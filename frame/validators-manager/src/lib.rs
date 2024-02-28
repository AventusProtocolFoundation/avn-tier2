//! # Validators manager Pallet
//!
//! This pallet provides functionality to add/remove validators.
//!
//! The pallet is based on the Substrate session pallet and implements related traits for session
//! management when validators are added or removed.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}};

use sp_std::prelude::*;
use frame_support::{decl_event, decl_storage, decl_module, decl_error, dispatch::{DispatchResult, DispatchResultWithPostInfo,
    DispatchErrorWithPostInfo}, ensure, debug, traits::{Currency, Get, OnUnbalanced, Imbalance,
    ExistenceRequirement::KeepAlive, WithdrawReasons, IsSubType}, weights::{Weight, GetDispatchInfo}, transactional, Parameter};
use frame_system::{self as system, offchain::SendTransactionTypes, ensure_signed, ensure_none, ensure_root, RawOrigin};
use pallet_session::{self as session, Config as SessionConfig};
use sp_runtime::{traits::{Convert, Member, AccountIdConversion, Saturating, CheckedSub, CheckedAdd, Zero, Bounded,
    StaticLookup, Dispatchable, IdentifyAccount, Verify}, DispatchError, Perbill, ModuleId, transaction_validity::{
    TransactionValidity, InvalidTransaction, TransactionSource}
};

use sp_core::{ecdsa, H512};
use codec::{Encode, Decode};
use sp_application_crypto::RuntimeAppPublic;
use sp_avn_common::{safe_add_block_numbers, calculate_two_third_quorum, event_types::Validator, IngressCounter, Proof,
    InnerCallValidator};
use pallet_ethereum_events::{ProcessedEventsChecker};
use pallet_avn::{self as avn, Error as avn_error, AccountToBytesConverter, NewSessionHandler, EthereumPublicKeyChecker,
    DisabledValidatorChecker, ValidatorRegistrationNotifier, Enforcer,
    vote::{
        VotingSessionData,
        VotingSessionManager,
        process_approve_vote,
        process_reject_vote,
        end_voting_period_validate_unsigned,
        approve_vote_validate_unsigned,
        reject_vote_validate_unsigned,
        APPROVE_VOTE,
        REJECT_VOTE,
    }
};
use pallet_ethereum_transactions::{CandidateTransactionSubmitter,
    ethereum_transaction::{ActivateValidatorData, DeregisterValidatorData, EthAbiHelper, EthTransactionType, TransactionId, SlashValidatorData}
};

use pallet_staking::{ValidatorPrefs, EraPayout, RewardDestination, WeightInfo as StakingWeightInfo, ElectionStatus, EraIndex};
use pallet_session::historical::IdentificationTuple;
use sp_staking::offence::ReportOffence;
use core::convert::TryFrom;

pub(crate) const LOG_TARGET: &'static str = "validatorsManager";

pub mod vote;
use crate::vote::*;
pub mod offence;
use crate::offence::{ValidatorOffence, ValidatorOffenceType, create_and_report_validators_offence};
pub mod proxy_helper;
use crate::proxy_helper::*;

pub trait Config: SendTransactionTypes<Call<Self>>
    + system::Config + session::Config
    + avn::Config
    + pallet_staking::Config
    + pallet_session::historical::Config
{
    /// Overarching event type
    type Event: From<Event<Self>> + Into<<Self as system::Config>::Event>;
    /// A trait that allows to subscribe to notifications triggered when ethereum event processes an event
    type ProcessedEventsChecker: ProcessedEventsChecker;
    /// A period (in block number) where validators are allowed to vote
    type VotingPeriod: Get<Self::BlockNumber>;
    /// A trait that allows pallets to submit transactions to Ethereum
    type CandidateTransactionSubmitter: CandidateTransactionSubmitter<Self::AccountId>;
    /// A trait that allows converting between accountIds <-> public keys
    type AccountToBytesConvert: AccountToBytesConverter<Self::AccountId>;
    /// A trait that allows extra work to be done during validator registration
    type ValidatorRegistrationNotifier: ValidatorRegistrationNotifier<<Self as session::Config>::ValidatorId>;
    ///  A type that gives the pallet the ability to report offences
    type ReportValidatorOffence: ReportOffence<
            Self::AccountId,
            IdentificationTuple<Self>,
            ValidatorOffence<IdentificationTuple<Self>>,
        >;
    /// The validator manager's module id, used for deriving its sovereign account ID.
    type ModuleId: Get<ModuleId>;

    /// The overarching call type.
    type Call: Parameter
        + Dispatchable<Origin=<Self as frame_system::Config>::Origin>
        + IsSubType<Call<Self>>
        + From<Call<Self>>
        + GetDispatchInfo;

    /// A type that can be used to verify signatures
    type Public: IdentifyAccount<AccountId = Self::AccountId>;

    /// The signature type used by accounts/transactions.
    type Signature: Verify<Signer = Self::Public> + Member + Decode + Encode + From<sp_core::sr25519::Signature>;

    /// Weight information for the extrinsics in this pallet.
    type WeightInfo: WeightInfo;
}

#[derive(Copy, Clone, Eq, PartialEq, Encode, Decode, Debug)]
pub enum ValidatorsActionStatus {
    /// Validator enters this state immediately within removal extrinsic, ready for session confirmation
    AwaitingConfirmation,
    /// Validator enters this state within session handler, ready for signing and sending to T1
    Confirmed,
    /// Validator enters this state once T1 action request is sent, ready to be removed from hashmap
    Actioned,
    /// Validator enters this state once T1 event processed
    None,
}

#[derive(Copy, Clone, Eq, PartialEq, Encode, Decode, Debug)]
pub enum ValidatorsActionType {
    /// Validator has asked to leave voluntarily
    Resignation,
    /// Validator is being forced to leave due to a malicious behaviour
    Slashed,
    /// Validator activates himself after he joins an active session
    Activation,
    /// Default value
    Unknown,
}

impl ValidatorsActionType {
    fn is_deregistration(&self) -> bool {
        match self {
            ValidatorsActionType::Resignation => true,
            ValidatorsActionType::Slashed => true,
            _ => false
        }
    }
}

#[derive(Encode, Decode, Default, Clone, PartialEq, Debug)]
pub struct ValidatorsActionData<AccountId: Member> {
    pub status: ValidatorsActionStatus,
    pub primary_validator: AccountId,
    pub eth_transaction_id: TransactionId,
    pub action_type: ValidatorsActionType,
    pub reserved_eth_transaction: EthTransactionType,
}

impl<AccountId: Member> ValidatorsActionData<AccountId> {
    fn new(
        status: ValidatorsActionStatus,
        primary_validator: AccountId,
        eth_transaction_id: TransactionId,
        action_type: ValidatorsActionType,
        reserved_eth_transaction: EthTransactionType) -> Self
    {
        return ValidatorsActionData::<AccountId> {
            status,
            primary_validator,
            eth_transaction_id,
            action_type,
            reserved_eth_transaction,
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tests_voting_deregistration;

#[cfg(test)]
#[path = "proxy_tests/bond_tests.rs"]
mod bond_tests;

#[cfg(test)]
#[path = "proxy_tests/bond_extra_tests.rs"]
mod bond_extra_tests;

#[cfg(test)]
#[path = "proxy_tests/unbond_tests.rs"]
mod unbond_tests;

#[cfg(test)]
#[path = "proxy_tests/withdraw_unbonded_tests.rs"]
mod withdraw_unbonded_tests;

#[cfg(test)]
#[path = "proxy_tests/rebond_tests.rs"]
mod rebond_tests;

#[cfg(test)]
#[path = "proxy_tests/nominate_tests.rs"]
mod nominate_tests;

#[cfg(test)]
#[path = "proxy_tests/set_controller_tests.rs"]
mod set_controller_tests;

#[cfg(test)]
#[path = "proxy_tests/set_payee_tests.rs"]
mod set_payee_tests;

#[cfg(test)]
#[path = "proxy_tests/staking_reward_tests.rs"]
mod staking_reward_tests;

#[cfg(test)]
#[path = "proxy_tests/common.rs"]
mod common;

#[cfg(test)]
#[path = "../../avn/src/tests/extension_builder.rs"]
pub mod extension_builder;

#[cfg(test)]
mod mock;

#[cfg(any(feature = "runtime-benchmarks"))]
pub mod benchmark_utils;
#[cfg(any(feature = "runtime-benchmarks"))]
pub mod benchmarking;

// TODO: [TYPE: business logic][PRI: high][CRITICAL]
// Rerun benchmark in production and update both ./default_weights.rs file and /bin/node/runtime/src/weights/pallet_ethereum_transactions.rs file.
pub mod default_weights;
pub use default_weights::WeightInfo;

// used in benchmarks and weights calculation only
const MAX_VALIDATOR_ACCOUNT_IDS: u32 = 10;
const MAX_OFFENDERS: u32 = 2;

// TODO [TYPE: review][PRI: medium]: if needed, make this the default value to a configurable option.
// See MinimumValidatorCount in Staking pallet as a reference
const DEFAULT_MINIMUM_VALIDATORS_COUNT : usize = 2;
const NAME: &'static [u8; 17] = b"validatorsManager";

// Error codes returned by validate unsigned methods
const ERROR_CODE_INVALID_DEREGISTERED_VALIDATOR: u8 = 10;

pub const SIGNED_BOND_CONTEXT: &'static [u8] = b"authorization for bond operation";
pub const SIGNED_NOMINATOR_CONTEXT: &'static [u8] = b"authorization for nominate operation";

pub type AVN<T> = avn::Module::<T>;
type BalanceOf<T> = <<T as pallet_staking::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type PositiveImbalanceOf<T> = <<T as pallet_staking::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::PositiveImbalance;
type NegativeImbalanceOf<T> = <<T as pallet_staking::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;
type CurrencyOf<T> = <T as pallet_staking::Config>::Currency;

decl_event!(
    pub enum Event<T> where
        ValidatorId = <T as system::Config>::AccountId,
        ActionId = ActionId<<T as system::Config>::AccountId>,
        EthKey = ecdsa::Public, // The ethereum public key
        IdentificationTuple = IdentificationTuple<T>,
        ValidatorOffenceType = ValidatorOffenceType,
        Balance = BalanceOf<T>,
    {
        ValidatorRegistered(ValidatorId, EthKey),
        ValidatorDeregistered(ValidatorId),
        ValidatorActivationStarted(ValidatorId),
        VoteAdded(/*Voter*/ ValidatorId, ActionId, /*true = approve*/ bool),
        VotingEnded(ActionId, /*true = deregistration is approved*/ bool),
        ValidatorActionConfirmed(ActionId),
        ValidatorSlashed(ActionId),
        OffenceReported(ValidatorOffenceType,/*offenders*/ Vec<IdentificationTuple>),
        /// Some funds have been deposited in the reward pot.
        RewardPotDeposit(Balance),
        /// Some funds have been withdrawn from the reward pot.
        RewardPotWithdrawal(Balance),
        /// Validator address that triggered the update, commission rate and flag for refusing nominations
        ValidatorPreferenceUpdated(ValidatorId, Perbill, bool),
        /// A nomination was registered. [Nominator Address, total nomination, number of validators to nominate]
        Nominated(ValidatorId, Balance, u32),
        /// We don't have enough to cover the reward payment, we have an error here. (Balance: total pot value)
        NotEnoughFundsForEraPayment(Balance),
        /// PayoutCompleted(EraIndex, Number of validators paid)
        PayoutCompleted(EraIndex, u32),
    }
);

decl_error! {
    pub enum Error for Module<T: Config> {
        NoTier1EventForAddingValidator,
        NoTier1EventForRemovingValidator,
        NoValidators,
        ValidatorAlreadyExists,
        InvalidIngressCounter,
        MinimumValidatorsReached,
        ErrorEndingVotingPeriod,
        VotingSessionIsNotValid,
        ErrorSubmitCandidateTxnToTier1,
        ErrorCalculatingPrimaryValidator,
        ErrorGeneratingEthDescription,
        ValidatorsActionDataNotFound,
        RemovalAlreadyRequested,
        ErrorConvertingAccountIdToValidatorId,
        SlashedValidatorIsNotFound,
        InsufficientValidatorBond,
        ValidatorCommissionTooHigh,
        /// Sender is not a registered controller account
        NotController,
        /// Stash does not have enough funds to nominate
        InsufficientBond,
        /// An active validator cannot nominate
        AlreadyValidating,
        /// User does not have enough funds to nominate
        InsufficientFundsToNominateBond,
        /// The ethereum public key of this validator alredy exists
        ValidatorEthKeyAlreadyExists,
        /// Proxy transaction is not authorised
        UnauthorizedProxyTransaction,
        /// The signer of the proof and the sender do not match
        SenderIsNotSigner,
        UnauthorizedSignedBondTransaction,
        UnauthorizedSignedNominateTransaction,
        UnauthorizedSignedRebondTransaction,
        UnauthorizedSignedPayoutStakersTransaction,
        UnauthorizedSignedSetControllerTransaction,
        UnauthorizedSignedSetPayeeTransaction,
        UnauthorizedSignedWithdrawUnbondedTransaction,
        UnauthorizedSignedUnbondTransaction,
        UnauthorizedSignedBondExtraTransaction
    }
}

decl_storage! {
	trait Store for Module<T: Config> as ValidatorsManager {
        pub ValidatorAccountIds get(fn validator_account_ids): Option<Vec<T::AccountId>>;
        pub ValidatorActions: double_map hasher(blake2_128_concat) T::AccountId, hasher(blake2_128_concat) IngressCounter =>
            ValidatorsActionData<T::AccountId>;
        pub VotesRepository get(fn get_vote): map hasher(blake2_128_concat) ActionId<T::AccountId> =>
            VotingSessionData<T::AccountId, T::BlockNumber>;
        pub PendingApprovals get(fn get_pending_actions): map hasher(blake2_128_concat) T::AccountId =>
            IngressCounter;
        pub EthereumPublicKeys get(fn get_validator_by_eth_public_key): map hasher(blake2_128_concat) ecdsa::Public => T::AccountId;
        pub TotalIngresses get(fn get_ingress_counter): IngressCounter;
        pub MinValidatorBond  get(fn min_validator_bond) config(): BalanceOf<T>;
        pub MaxCommission  get(fn validator_max_commission) config(): Perbill;
        pub MinUserBond  get(fn min_user_bond) config(): BalanceOf<T>;
        pub FailedRewardPayments get(fn faild_payments): map hasher(blake2_128_concat) BalanceOf<T> => bool;
        /// Storage value that holds the total amount of payouts we are waiting to take out of this pallet's pot.
        pub LockedEraPayout get(fn locked_era_payout): BalanceOf<T>;
        /// An account nonce that represents the number of proxy transactions from this account
        pub ProxyNonces get(fn proxy_nonce): map hasher(blake2_128_concat) T::AccountId => u64;
		StorageVersion: Releases;
    }
    add_extra_genesis {
        config(validators): Vec<(T::AccountId, ecdsa::Public)>;
        build(|config: &GenesisConfig<T>| {
            for (validator_controller_account_id, eth_public_key) in &config.validators {
                assert!(
                    Module::<T>::validator_genesis_registration_is_valid(validator_controller_account_id).is_ok(),
                    "Controller is not registered in the staking pallet or it does not have the minimum bond");

                let validator_stash_account_id = pallet_staking::Module::<T>::ledger(&validator_controller_account_id)
                    .expect("The Assert makes sure there is an entry for the controller").stash;

                <ValidatorAccountIds<T>>::append(&validator_stash_account_id);
                <EthereumPublicKeys<T>>::insert(eth_public_key, validator_stash_account_id);
            }
        });
    }
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;

        // TODO: [TYPE: business logic][PRI: high][CRITICAL][JIRA: 356] Specify weights for all the extrinsics
        ///
        /// # <weight>
        /// Keys: V: number of validator account Ids
        /// - DbReads: `TotalIngresses`, `ValidatorAccountIds`, `ValidatorAction`, `avn::Validators`: O(1)
        /// - DbWrites: `TotalIngresses`, `ValidatorAccountIds`, `ValidatorAction`: O(1)
        /// - Account id to bytes conversion includes encoding and copy operations: O(1)
        /// - Iterate ValidatorAccountIds to find validator index: O(V)
        /// - Ethereum transactions pallet reserve transaction id operation: O(1)
        ///     - DbReads: `ethereum-transactions::ReservedTransactions, Nonce`
        ///     - DbWrites: `ethereum-transactions::ReservedTransactions`
        ///     - DbMutates: `ethereum-transactions::Nonce`
        /// - swap_remove: `ValidatorAccountIds`: O(1)
        /// - Calling staking pallet function: O(fn_chill)
        /// - Emit an event: O(1)
        /// - Complexity: `O(V + 1 + fn_chill)`
        /// # </weight>
        #[weight = <T as Config>::WeightInfo::remove_validator(MAX_VALIDATOR_ACCOUNT_IDS)]
        pub fn remove_validator(origin, validator_controller_account_id: T::AccountId) -> DispatchResult {
            let _ = ensure_root(origin)?;

            // TODO [TYPE: security][PRI: low][CRITICAL][JIRA: 66]: ensure that we have authorization from the whole of T2?
            // This is part of the package to implement validator removals, slashing and the economics around that
            Self::remove_resigned_validator(&validator_controller_account_id)?;

            // remove the validator from the staking pallet
            pallet_staking::Module::<T>::chill(
                <T as frame_system::Config>::Origin::from(RawOrigin::Signed(validator_controller_account_id.clone()))
            )?;

            Self::deposit_event(RawEvent::ValidatorDeregistered(validator_controller_account_id));

            // TODO [TYPE: weightInfo][PRI: medium]: Refund unused weights
            Ok(())
        }

        /// # <weight>
        /// Keys: V: number of validators
        ///  - Convert data to eth compatible encoding operation: O(1)
        ///  - Eth signature is valid operation: O(V)
        ///    - If eth signature is invalid: Create and report validators offence: O(1)
        ///  - Get voting session: O(1)
        ///  - Process approve vote: O(V)
        ///  - Emit an event: O(1)
        /// - Total Complexity: `O(V + 1)`
        /// # </weight>
        #[weight = <T as Config>::WeightInfo::approve_action_with_end_voting(MAX_VALIDATOR_ACCOUNT_IDS)]
        fn approve_validator_action(
            origin,
            action_id: ActionId<T::AccountId>,
            validator: Validator<T::AuthorityId, T::AccountId>,
            approval_signature: ecdsa::Signature,
            _signature: <T::AuthorityId as RuntimeAppPublic>::Signature) -> DispatchResult
        {
            ensure_none(origin)?;

            let eth_encoded_data = Self::convert_data_to_eth_compatible_encoding(&action_id)?;
            if !AVN::<T>::eth_signature_is_valid(eth_encoded_data, &validator, &approval_signature) {
                create_and_report_validators_offence::<T>(
                    &validator.account_id,
                    &vec![validator.account_id.clone()],
                    ValidatorOffenceType::InvalidSignatureSubmitted,
                );
                return Err(avn_error::<T>::InvalidECDSASignature)?;
            };

            let voting_session = Self::get_voting_session(&action_id);

            process_approve_vote::<T>(&voting_session, validator.account_id.clone(), approval_signature)?;

            Self::deposit_event(RawEvent::VoteAdded(validator.account_id, action_id, APPROVE_VOTE));

            // TODO [TYPE: weightInfo][PRI: medium]: Refund unused weights
            Ok(())
        }

        /// # <weight>
        /// Keys: V: number of validators
        ///  - Get voting session: O(1)
        ///  - Process reject vote: O(V)
        ///  - Emit an event: O(1)
        /// - Total Complexity: `O(V + 1)`
        /// # </weight>
        #[weight = <T as Config>::WeightInfo::reject_action_with_end_voting(MAX_VALIDATOR_ACCOUNT_IDS)]
        fn reject_validator_action(
            origin,
            action_id: ActionId<T::AccountId>,
            validator: Validator<T::AuthorityId, T::AccountId>,
            _signature: <T::AuthorityId as RuntimeAppPublic>::Signature) -> DispatchResult
        {
            ensure_none(origin)?;
            let voting_session = Self::get_voting_session(&action_id);

            process_reject_vote::<T>(&voting_session, validator.account_id.clone())?;

            Self::deposit_event(RawEvent::VoteAdded(validator.account_id, action_id, REJECT_VOTE));

            // TODO [TYPE: weightInfo][PRI: medium]: Refund unused weights
            Ok(())
        }

        /// # <weight>
        /// Keys: O: number of offenders accounts
        ///  - DbReads: 5 * `VotesRepository`, 2 * `ValidatorAction`, `PendingApprovals`: O(1)
        ///  - DbWrites: `PendingApprovals`, `ValidatorAction`: O(1)
        ///  - ethereum-transactions pallet operation:
        ///     - DbReads: 2 * `ReservedTransactions`, 2 * `Repository`, `DispatchedAvnTxIds`: O(1)
        ///     - DbWrites: `Repository`, `ReservedTransactions`, `DispatchedAvnTxIds`: O(1)
        ///     - Emit evet: `TransactionReadyToSend`: O(1)
        ///  - Create offenders identity: O(O)
        ///  - Emit events: `OffenceReported`, `VotingEnded`: O(1)
        /// - Total Complexity: `O(O + 1)`
        /// # </weight>
        #[weight = <T as Config>::WeightInfo::end_voting_period_with_rejected_valid_actions(MAX_OFFENDERS)
            .max(<T as Config>::WeightInfo::end_voting_period_with_approved_invalid_actions(MAX_OFFENDERS))]
        fn end_voting_period(
            origin,
            action_id: ActionId<T::AccountId>,
            validator: Validator<T::AuthorityId, T::AccountId>,
            _signature: <T::AuthorityId as RuntimeAppPublic>::Signature) -> DispatchResult
        {
            ensure_none(origin)?;
            //Event is deposited in end_voting because this function can get called from `approve_validator_action`
            //or `reject_validator_action`
            Self::end_voting(validator.account_id, &action_id)?;

            // TODO [TYPE: weightInfo][PRI: medium]: Refund unused weights
            Ok(())
        }

        fn offchain_worker(block_number: T::BlockNumber) {
            let setup_result = AVN::<T>::pre_run_setup(block_number, NAME.to_vec());
            if let Err(e) = setup_result {
                match e {
                    _ if e == DispatchError::from(avn_error::<T>::OffchainWorkerAlreadyRun) => {();},
                    _ => {
                            debug::native::error!("💔 Unable to run offchain worker: {:?}", e);
                        }
                };

                return ;
            }
            let this_validator = setup_result.expect("We have a validator");

            cast_votes_if_required::<T>(block_number, &this_validator);
            end_voting_if_required::<T>(block_number, &this_validator);
        }

        /// Sudo function to convert a bonded account to become a validator.
        /// This will call the validate method in the staking pallet.
        /// [transactional]: this makes `add_validator` behave like an ethereum transaction (atomic tx). No need to use VFWL.
        /// see here for more info: https://github.com/paritytech/substrate/issues/10806
        ///
        /// # <weight>
        ///   DbReads: ValidatorAccountIds, EthereumPublicKeys, MinValidatorBond, MaxCommission, TotalIngresses: O(1)
        ///     pallet_staking: Ledger: O(1)
        ///     pallet_avn: Validators: O(1)
        ///     pallet_ethereum_transactions: ReservedTransactions, Nonce: O(1)
        ///   DbWrites: ValidatorAccountIds, EthereumPublicKeys, TotalIngresses, ValidatorActions: O(1)
        ///     pallet_ethereum_transactions: ReservedTransactions, Nonce: O(1)
        ///     pallet_avn_offence_handler: ReportedOffenders: O(1)
        ///   Call pallet_staking extrinsic validate: O(fn_validate)
        ///  Total complexity: O(1 + fn_validate)
        /// # </weight>
        #[weight = <T as Config>::WeightInfo::add_validator()]
        #[transactional]
        pub fn add_validator(
            origin,
            controller_account_id: T::AccountId,
            validator_eth_public_key: ecdsa::Public,
            preferences: ValidatorPrefs) -> DispatchResult
        {
            ensure_root(origin)?;
            let validator_account_ids = Self::validator_account_ids().or_else(|| Some(vec![])).expect("empty vec");
            ensure!(validator_account_ids.len() > 0, Error::<T>::NoValidators);
            ensure!(!<EthereumPublicKeys<T>>::contains_key(&validator_eth_public_key), Error::<T>::ValidatorEthKeyAlreadyExists);

            let ledger = pallet_staking::Module::<T>::ledger(&controller_account_id).ok_or(Error::<T>::NotController)?;

            // latest version (3.0+) of substrate has implemented this. We only need this here in the short term.
            ensure!(ledger.active >= Self::min_validator_bond(), Error::<T>::InsufficientValidatorBond);
            let stash = &ledger.stash;

            // ensure their commission is correct.
            ensure!(preferences.commission <= Self::validator_max_commission(), Error::<T>::ValidatorCommissionTooHigh);
            ensure!(!validator_account_ids.contains(stash), Error::<T>::ValidatorAlreadyExists);

            pallet_staking::Module::<T>::validate(
                <T as frame_system::Config>::Origin::from(RawOrigin::Signed(controller_account_id.clone())),
                preferences
            )?;

            Self::register_validator(stash, &validator_eth_public_key)?;

            <ValidatorAccountIds<T>>::append(stash);
            <EthereumPublicKeys<T>>::insert(validator_eth_public_key, stash);

            Ok(())
        }

        #[weight = <T as Config>::WeightInfo::bond()]
        pub fn bond(
            origin,
            controller: <T::Lookup as StaticLookup>::Source,
            #[compact] value: BalanceOf<T>,
            payee: RewardDestination<T::AccountId>) -> DispatchResult
        {
            let stash = ensure_signed(origin)?;
            ensure!(value >= Self::min_validator_bond().min(Self::min_user_bond()), Error::<T>::InsufficientBond);

            pallet_staking::Module::<T>::bond(
                <T as frame_system::Config>::Origin::from(RawOrigin::Signed(stash)), controller, value, payee
            )?;

            Ok(())
        }

        #[weight = <T as Config>::WeightInfo::signed_bond()]
        pub fn signed_bond(
            origin,
            proof: Proof<T::Signature, T::AccountId>,
            controller: <T::Lookup as StaticLookup>::Source,
            #[compact] value: BalanceOf<T>,
            payee: RewardDestination<T::AccountId>) -> DispatchResult
        {
            let stash = ensure_signed(origin)?;
            ensure!(stash == proof.signer, Error::<T>::SenderIsNotSigner);
            ensure!(value >= Self::min_validator_bond().min(Self::min_user_bond()), Error::<T>::InsufficientBond);

            let sender_nonce = Self::proxy_nonce(&stash);
            let signed_payload = encode_signed_bond_params::<T>(&proof, &controller, &value, &payee, sender_nonce);
            ensure!(Self::verify_signature(&proof, &signed_payload.as_slice()).is_ok(),
                Error::<T>::UnauthorizedSignedBondTransaction);

            pallet_staking::Module::<T>::bond(
                <T as frame_system::Config>::Origin::from(RawOrigin::Signed(stash.clone())), controller, value, payee
            )?;

            <ProxyNonces<T>>::mutate(&stash, |n| *n += 1);
            Ok(())
        }

        #[weight = <T as Config>::WeightInfo::nominate(targets.len() as u32)]
        pub fn nominate(origin, targets: Vec<<T::Lookup as StaticLookup>::Source>) -> DispatchResult {
            let controller = ensure_signed(origin)?;
            let ledger = pallet_staking::Module::<T>::ledger(&controller).ok_or(Error::<T>::NotController)?;
            ensure!(ledger.active >= Self::min_user_bond(), Error::<T>::InsufficientFundsToNominateBond);

            let stash = &ledger.stash;

            if pallet_staking::Validators::<T>::contains_key(&stash) {
                Err(Error::<T>::AlreadyValidating)?
            }

            let number_of_validators_nominated = targets.len() as u32;
            pallet_staking::Module::<T>::nominate(
                <T as frame_system::Config>::Origin::from(RawOrigin::Signed(controller)), targets
            )?;

            Self::deposit_event(RawEvent::Nominated(ledger.stash, ledger.active, number_of_validators_nominated));

            Ok(())
        }

        #[weight = <T as pallet_staking::Config>::WeightInfo::nominate(targets.len() as u32)
        .saturating_add(T::DbWeight::get().reads_writes(4, 1))
        .saturating_add(40_000_000)]
        pub fn signed_nominate(
            origin,
            proof: Proof<T::Signature, T::AccountId>,
            targets: Vec<<T::Lookup as StaticLookup>::Source>) -> DispatchResult
        {
            let controller = ensure_signed(origin)?;
            ensure!(controller == proof.signer, Error::<T>::SenderIsNotSigner);

            let ledger = pallet_staking::Module::<T>::ledger(&controller).ok_or(Error::<T>::NotController)?;
            ensure!(ledger.active >= Self::min_user_bond(), Error::<T>::InsufficientFundsToNominateBond);

            let sender_nonce = Self::proxy_nonce(&controller);
            let signed_payload = encode_signed_nominate_params::<T>(&proof, &targets, sender_nonce);
            ensure!(Self::verify_signature(&proof, &signed_payload.as_slice()).is_ok(),
                Error::<T>::UnauthorizedSignedNominateTransaction);

            let stash = &ledger.stash;

            if pallet_staking::Validators::<T>::contains_key(&stash) {
                Err(Error::<T>::AlreadyValidating)?
            }

            let number_of_validators_nominated = targets.len() as u32;
            pallet_staking::Module::<T>::nominate(
                <T as frame_system::Config>::Origin::from(RawOrigin::Signed(controller.clone())), targets
            )?;

            <ProxyNonces<T>>::mutate(&controller, |n| *n += 1);
            Self::deposit_event(RawEvent::Nominated(ledger.stash, ledger.active, number_of_validators_nominated));

            Ok(())
        }

        #[weight = <T as Config>::WeightInfo::update_validator_preference()]
        pub fn update_validator_preference(origin, controller: T::AccountId, preference: ValidatorPrefs) -> DispatchResult
        {
            ensure_root(origin)?;
            let ledger = pallet_staking::Module::<T>::ledger(&controller).ok_or(Error::<T>::NotController)?;

            // ensure their commission is correct.
            ensure!(preference.commission <= Self::validator_max_commission(), Error::<T>::ValidatorCommissionTooHigh);

            pallet_staking::Validators::<T>::mutate(&ledger.stash, |p| *p = preference.clone());
            Self::deposit_event(RawEvent::ValidatorPreferenceUpdated(ledger.stash, preference.commission, preference.blocked));

            Ok(())
        }

        /// Update the various staking configurations .
        ///
        /// * `min_user_bond`: The minimum active bond needed to be a nominator.
        /// * `min_validator_bond`: The minimum active bond needed to be a validator.
        /// * `max_commission`: The maximum amount of commission that each validators must maintain.
        ///
        /// Origin must be Root to call this function.
        ///
        /// NOTE: Existing nominators and validators will not be affected by this update.
        #[weight = <T as Config>::WeightInfo::set_staking_configs()]
        pub fn set_staking_configs(
            origin,
            min_validator_bond: BalanceOf<T>,
            min_user_bond: BalanceOf<T>,
            max_commission: Perbill) -> DispatchResult
        {
            ensure_root(origin)?;
            MinValidatorBond::<T>::set(min_validator_bond);
            MinUserBond::<T>::set(min_user_bond);
            MaxCommission::set(max_commission);
            Ok(())
        }

        /// Add some extra amount that have appeared in the stash `free_balance` into the balance up
        /// for staking.
        ///
        /// Use this if there are additional funds in your stash account that you wish to bond.
        /// Unlike [`bond`] or [`unbond`] this function does not impose any limitation on the amount
        /// that can be added.
        ///
        /// The dispatch origin for this call must be _Signed_ by the stash, not the controller
        #[weight = <T as Config>::WeightInfo::signed_bond_extra()]
        pub fn signed_bond_extra(
            origin,
            proof: Proof<T::Signature, T::AccountId>,
            #[compact] max_additional: BalanceOf<T>) -> DispatchResult
        {
            let stash = ensure_signed(origin)?;
            ensure!(stash == proof.signer, Error::<T>::SenderIsNotSigner);

            let sender_nonce = Self::proxy_nonce(&stash);
            let signed_payload = encode_signed_bond_extra_params::<T>(&proof, &max_additional, sender_nonce);
            ensure!(Self::verify_signature(&proof, &signed_payload.as_slice()).is_ok(),
                Error::<T>::UnauthorizedSignedBondExtraTransaction);

            pallet_staking::Module::<T>::bond_extra(
                <T as frame_system::Config>::Origin::from(RawOrigin::Signed(stash.clone())), max_additional
            )?;

            <ProxyNonces<T>>::mutate(&stash, |n| *n += 1);
            Ok(())
        }

        /// Schedule a portion of the stash to be unlocked ready for transfer out after the bond
        /// period ends. If this leaves an amount actively bonded less than
        /// T::Currency::minimum_balance(), then it is increased to the full amount.
        ///
        /// Once the unlock period is done, you can call `withdraw_unbonded` to actually move
        /// the funds out of management ready for transfer.
        ///
        /// No more than a limited number of unlocking chunks (see `MAX_UNLOCKING_CHUNKS`)
        /// can co-exists at the same time. In that case, [`Call::withdraw_unbonded`] needs
        /// to be called first to remove some of the chunks (if possible).
        ///
        /// The dispatch origin for this call must be _Signed_ by the controller, not the stash.
        #[weight = <T as Config>::WeightInfo::signed_unbond()]
        pub fn signed_unbond(
            origin,
            proof: Proof<T::Signature, T::AccountId>,
            #[compact] value: BalanceOf<T>) -> DispatchResult
        {
            let controller = ensure_signed(origin)?;
            ensure!(controller == proof.signer, Error::<T>::SenderIsNotSigner);

            let sender_nonce = Self::proxy_nonce(&controller);
            let signed_payload = encode_signed_unbond_params::<T>(&proof, &value, sender_nonce);
            ensure!(Self::verify_signature(&proof, &signed_payload.as_slice()).is_ok(),
                Error::<T>::UnauthorizedSignedUnbondTransaction);

            pallet_staking::Module::<T>::unbond(
                <T as frame_system::Config>::Origin::from(RawOrigin::Signed(controller.clone())),
                value
            )?;

            <ProxyNonces<T>>::mutate(&controller, |n| *n += 1);
            Ok(())
        }

        /// Remove any unlocked chunks from the `unlocking` queue from our management.
        /// This essentially frees up that balance to be used by the stash account to do
        /// whatever it wants. The dispatch origin for this call must be _Signed_ by the controller
        #[weight = <T as pallet_staking::Config>::WeightInfo::withdraw_unbonded_kill(*num_slashing_spans)
        .saturating_add(T::DbWeight::get().reads_writes(1, 1))
        .saturating_add(40_000_000)]
        pub fn signed_withdraw_unbonded(
            origin,
            proof: Proof<T::Signature, T::AccountId>,
            num_slashing_spans: u32) -> DispatchResultWithPostInfo
        {
            let controller = ensure_signed(origin)?;
            ensure!(controller == proof.signer, Error::<T>::SenderIsNotSigner);

            let sender_nonce = Self::proxy_nonce(&controller);
            let signed_payload = encode_signed_withdraw_unbonded_params::<T>(&proof, &num_slashing_spans, sender_nonce);
            ensure!(Self::verify_signature(&proof, &signed_payload.as_slice()).is_ok(),
                Error::<T>::UnauthorizedSignedWithdrawUnbondedTransaction);

            let withdraw_unbonded_weight = pallet_staking::Module::<T>::withdraw_unbonded(
                <T as frame_system::Config>::Origin::from(RawOrigin::Signed(controller.clone())),
                num_slashing_spans
            )?;

            <ProxyNonces<T>>::mutate(&controller, |n| *n += 1);

            Ok(Some(withdraw_unbonded_weight.actual_weight.or_else(|| Some(Weight::zero())).expect("Has default value")
                .saturating_add(T::DbWeight::get().reads_writes(1, 1))
                .saturating_add(40_000_000)).into()
            )
        }

        /// (Re-)set the payment target for a controller.
        /// Effects will be felt at the beginning of the next era.
        /// The dispatch origin for this call must be _Signed_ by the controller, not the stash.
        #[weight = <T as Config>::WeightInfo::signed_set_payee()]
        pub fn signed_set_payee(
            origin,
            proof: Proof<T::Signature, T::AccountId>,
            payee: RewardDestination<T::AccountId>) -> DispatchResult
        {
            let controller = ensure_signed(origin)?;
            ensure!(controller == proof.signer, Error::<T>::SenderIsNotSigner);

            let sender_nonce = Self::proxy_nonce(&controller);
            let signed_payload = encode_signed_set_payee_params::<T>(&proof, &payee, sender_nonce);
            ensure!(Self::verify_signature(&proof, &signed_payload.as_slice()).is_ok(),
                Error::<T>::UnauthorizedSignedSetPayeeTransaction);

            pallet_staking::Module::<T>::set_payee(
                <T as frame_system::Config>::Origin::from(RawOrigin::Signed(controller.clone())),
                payee
            )?;

            <ProxyNonces<T>>::mutate(&controller, |n| *n += 1);
            Ok(())
        }

        /// (Re-)set the controller of a stash.
        /// Effects will be felt at the beginning of the next era.
        /// The dispatch origin for this call must be _Signed_ by the stash, not the controller.
        #[weight = <T as pallet_staking::Config>::WeightInfo::set_controller()
        .saturating_add(T::DbWeight::get().reads_writes(1, 1))
        .saturating_add(40_000_000)]
        pub fn signed_set_controller(
            origin,
            proof: Proof<T::Signature, T::AccountId>,
            controller: <T::Lookup as StaticLookup>::Source) -> DispatchResult
        {
            let stash = ensure_signed(origin)?;
            ensure!(stash == proof.signer, Error::<T>::SenderIsNotSigner);

            let sender_nonce = Self::proxy_nonce(&stash);
            let signed_payload = encode_signed_set_controller_params::<T>(&proof, &controller, sender_nonce);
            ensure!(Self::verify_signature(&proof, &signed_payload.as_slice()).is_ok(),
                Error::<T>::UnauthorizedSignedSetControllerTransaction);

            pallet_staking::Module::<T>::set_controller(
                <T as frame_system::Config>::Origin::from(RawOrigin::Signed(stash.clone())),
                controller
            )?;

            <ProxyNonces<T>>::mutate(&stash, |n| *n += 1);
            Ok(())
        }

        /// Pay out all the stakers behind a single validator for a single era.
        /// - `validator_stash` is the stash account of the validator. Their nominators, up to
        ///   `T::MaxNominatorRewardedPerValidator`, will also receive their rewards.
        /// - `era` may be any era between `[current_era - history_depth; current_era]`.
        /// The origin of this call must be _Signed_. Any account can call this function, even if
        /// it is not one of the stakers.
        #[weight = <T as Config>::WeightInfo::signed_payout_all_validators_and_stakers(
            <T as pallet_staking::Config>::MaxNominatorRewardedPerValidator::get())
        ]
        #[transactional]
        pub fn signed_payout_stakers(
            origin,
            proof: Proof<T::Signature, T::AccountId>,
            era: EraIndex) -> DispatchResultWithPostInfo
        {
            let sender = ensure_signed(origin)?;
            ensure!(sender == proof.signer, Error::<T>::SenderIsNotSigner);

            let sender_nonce = Self::proxy_nonce(&sender);
            let signed_payload = encode_signed_payout_stakers_params::<T>(&proof, &era, sender_nonce);
            ensure!(Self::verify_signature(&proof, &signed_payload.as_slice()).is_ok(),
                Error::<T>::UnauthorizedSignedPayoutStakersTransaction);

            let validator_account_ids = Self::validator_account_ids().ok_or(Error::<T>::NoValidators)?;
            let validators_len: u32 = validator_account_ids.len() as u32;
            // Track the actual weight of each of the batch calls.
            let base_weight: Weight = (Weight::zero()
                .saturating_add(T::DbWeight::get().reads_writes(2, 0))
                .saturating_add(40_000_000)).into();
            let mut weight: Weight = 0;

            //TODO: If we have more than 10 validators we need to revisit this logic because it will have a performance issue
            for (_index, validator_stash) in validator_account_ids.into_iter().enumerate() {
                let result = pallet_staking::Module::<T>::payout_stakers(
                    <T as frame_system::Config>::Origin::from(RawOrigin::Signed(sender.clone())),
                    validator_stash,
                    era
                );

                weight = weight.saturating_add(
                    <T as pallet_staking::Config>::WeightInfo::payout_stakers_alive_staked(
                        <T as pallet_staking::Config>::MaxNominatorRewardedPerValidator::get()
                    )
                );

                result.map_err(|err| {
                    // Return the actual used weight + base_weight of this call.
                    let post_info = Some(base_weight + weight).into();
                    return DispatchErrorWithPostInfo { post_info, error: err.into() }                    ;
                })?;
            };

            <ProxyNonces<T>>::mutate(&sender, |n| *n += 1);
            Self::deposit_event(Event::<T>::PayoutCompleted(era, validators_len));
            Ok(Some(base_weight + weight).into())
        }

        /// Rebond a portion of the stash scheduled to be unlocked. The dispatch origin must be signed by the controller
        #[weight = <T as pallet_staking::Config>::WeightInfo::rebond(pallet_staking::MAX_UNLOCKING_CHUNKS as u32)
        .saturating_add(T::DbWeight::get().reads_writes(1, 1))
        .saturating_add(40_000_000)]
        pub fn signed_rebond(
            origin,
            proof: Proof<T::Signature, T::AccountId>,
            #[compact] value: BalanceOf<T>) -> DispatchResultWithPostInfo
        {
            let controller = ensure_signed(origin)?;
            ensure!(controller == proof.signer, Error::<T>::SenderIsNotSigner);

            let sender_nonce = Self::proxy_nonce(&controller);
            let signed_payload = encode_signed_rebond_params::<T>(&proof, &value, sender_nonce);
            ensure!(Self::verify_signature(&proof, &signed_payload.as_slice()).is_ok(),
                Error::<T>::UnauthorizedSignedRebondTransaction);

            let rebond_weight = pallet_staking::Module::<T>::rebond(
                <T as frame_system::Config>::Origin::from(RawOrigin::Signed(controller.clone())),
                value
            )?;

            <ProxyNonces<T>>::mutate(&controller, |n| *n += 1);

            return Ok(Some(rebond_weight.actual_weight.or_else(|| Some(Weight::zero())).expect("Has default value")
                .saturating_add(T::DbWeight::get().reads_writes(1, 1))
                .saturating_add(40_000_000)).into()
            );
        }

        /// Remove the given nominations from the calling validator.
        /// Effects will be felt at the beginning of the next era.
        /// The dispatch origin for this call must be _Root_.
        #[weight = <T as pallet_staking::Config>::WeightInfo::kick(who.len() as u32)
        .saturating_add(40_000)]
        pub fn kick(origin, controller_account_id: T::AccountId, who: Vec<<T::Lookup as StaticLookup>::Source>) -> DispatchResult
        {
            ensure_root(origin)?;

            pallet_staking::Module::<T>::kick(
                <T as frame_system::Config>::Origin::from(RawOrigin::Signed(controller_account_id.clone())),
                who
            )?;
            Ok(())
        }
    }
}

impl<T: Config> Module<T> {

    pub fn validator_genesis_registration_is_valid(controller: &T::AccountId) -> Result<(), Error<T>> {
        let ledger = pallet_staking::Module::<T>::ledger(&controller).ok_or(Error::<T>::NotController)?;
        ensure!(ledger.active >= Self::min_validator_bond(), Error::<T>::InsufficientValidatorBond);
        Ok(())
    }

    pub fn get_voting_session(
        action_id: &ActionId<T::AccountId>) -> Box<dyn VotingSessionManager<T::AccountId, T::BlockNumber>>
    {
        return Box::new(
            ValidatorManagementVotingSession::<T>::new(action_id)
        ) as Box<dyn VotingSessionManager<T::AccountId, T::BlockNumber>>;
    }

    pub fn sign_validators_action_for_ethereum(action_id: &ActionId<T::AccountId>) -> Result<(String, ecdsa::Signature), DispatchError>
    {
        let data = Self::convert_data_to_eth_compatible_encoding(action_id)?;
        return Ok((data.clone(), AVN::<T>::request_ecdsa_signature_from_external_service(&data)?));
    }

    pub fn convert_data_to_eth_compatible_encoding(action_id: &ActionId<T::AccountId>) -> Result<String, DispatchError> {
        let validators_action_data = Self::try_get_validators_action_data(action_id)?;
        let eth_description = EthAbiHelper::generate_ethereum_description_for_signature_request(
            &T::AccountToBytesConvert::into_bytes(&validators_action_data.primary_validator),
            &validators_action_data.reserved_eth_transaction,
            validators_action_data.eth_transaction_id
        )
        .map_err(|_| Error::<T>::ErrorGeneratingEthDescription)?;

        Ok(hex::encode(EthAbiHelper::generate_eth_abi_encoding_for_params_only(&eth_description)))
    }

    fn try_get_validators_action_data(action_id: &ActionId<T::AccountId>) -> Result<ValidatorsActionData<T::AccountId>, Error<T>> {
        if <ValidatorActions<T>>::contains_key(&action_id.action_account_id, action_id.ingress_counter) {
            return Ok(<ValidatorActions<T>>::get(&action_id.action_account_id, action_id.ingress_counter));
        }

        Err(Error::<T>::ValidatorsActionDataNotFound)?
    }

    fn end_voting(sender: T::AccountId, action_id: &ActionId<T::AccountId>) -> DispatchResult {
        let voting_session = Self::get_voting_session(&action_id);

        ensure!(voting_session.is_valid(), Error::<T>::VotingSessionIsNotValid);

        let vote = voting_session.state()?;

        ensure!(Self::can_end_vote(&vote), Error::<T>::ErrorEndingVotingPeriod);

        let deregistration_is_approved = vote.is_approved();

        if deregistration_is_approved {
            let validators_action_data = Self::try_get_validators_action_data(action_id)?;

            let result = T::CandidateTransactionSubmitter::submit_candidate_transaction_to_tier1(
                validators_action_data.reserved_eth_transaction,
                validators_action_data.eth_transaction_id,
                validators_action_data.primary_validator,
                voting_session.state()?.confirmations,
            );

            if let Err(result) = result {
                debug::native::error!("❌ Error Submitting Tx: {:?}", result);
                Err(result)?
            }

            create_and_report_validators_offence::<T>(
                &sender,
                &vote.nays,
                ValidatorOffenceType::RejectedValidAction,
            );

            <ValidatorActions<T>>::mutate(
                &action_id.action_account_id,
                action_id.ingress_counter,
                |validators_action_data| validators_action_data.status = ValidatorsActionStatus::Actioned
            );
        } else {
            // We didn't get enough votes to approve this deregistration
            create_and_report_validators_offence::<T>(
                &sender,
                &vote.ayes,
                ValidatorOffenceType::ApprovedInvalidAction,
            );
        }

        <PendingApprovals<T>>::remove(&action_id.action_account_id);

        Self::deposit_event(Event::<T>::VotingEnded(action_id.clone(), deregistration_is_approved));

        Ok(())
    }

    fn can_end_vote(vote: &VotingSessionData<T::AccountId, T::BlockNumber>) -> bool {
        return vote.has_outcome() || <system::Module<T>>::block_number() >= vote.end_of_voting_period ;
    }

    /// Helper function to help us fail early if any of the data we need is not available for the registration & activation
    fn prepare_registration_data(validator_id: &T::AccountId)
        -> Result<(T::ValidatorId, T::AccountId, EthTransactionType, TransactionId), DispatchError> {

        let new_validator_id = <T as SessionConfig>::ValidatorIdOf::convert(validator_id.clone())
            .ok_or(Error::<T>::ErrorConvertingAccountIdToValidatorId)?;
        let eth_tx_sender = AVN::<T>::calculate_primary_validator(<system::Module<T>>::block_number())
            .map_err(|_| Error::<T>::ErrorCalculatingPrimaryValidator)?;
        let eth_transaction_type = EthTransactionType::ActivateValidator(
            ActivateValidatorData::new(T::AccountToBytesConvert::into_bytes(&validator_id))
        );
        let tx_id = T::CandidateTransactionSubmitter::reserve_transaction_id(&eth_transaction_type)?;

        Ok((new_validator_id, eth_tx_sender, eth_transaction_type, tx_id))
    }

    fn start_activation_for_registered_validator(
        registered_validator: &T::AccountId,
        eth_tx_sender: T::AccountId,
        eth_transaction_type: EthTransactionType,
        tx_id: TransactionId
    ) {
        let ingress_counter = Self::get_ingress_counter() + 1;

        TotalIngresses::put(ingress_counter);
        <ValidatorActions<T>>::insert(
            registered_validator,
            ingress_counter,
            ValidatorsActionData::new(
                ValidatorsActionStatus::AwaitingConfirmation,
                eth_tx_sender,
                tx_id,
                ValidatorsActionType::Activation,
                eth_transaction_type
            ),
        );
    }

    fn register_validator(stash_account_id: &T::AccountId, eth_public_key: &ecdsa::Public) -> DispatchResult {
        let (new_validator_id, eth_tx_sender, eth_transaction_type, tx_id) = Self::prepare_registration_data(stash_account_id)?;

        Self::start_activation_for_registered_validator(stash_account_id, eth_tx_sender, eth_transaction_type, tx_id);
        T::ValidatorRegistrationNotifier::on_validator_registration(&new_validator_id);

        Self::deposit_event(RawEvent::ValidatorRegistered(stash_account_id.clone(), eth_public_key.clone()));

        Ok(())
    }

    /// We assume the full public key doesn't have the `04` prefix
    #[allow(dead_code)]
    pub fn compress_eth_public_key(full_public_key: H512) -> ecdsa::Public {
        let mut compressed_public_key = [0u8; 33];

        // Take bytes 0..32 from the full plublic key ()
        compressed_public_key[1..=32].copy_from_slice(&full_public_key.0[0..32]);
        // If the last byte of the full public key is even, prefix compresssed public key with 2, otherwise prefix with 3
        compressed_public_key[0] = if full_public_key.0[63] % 2 == 0 { 2u8 } else { 3u8 };

        return ecdsa::Public::from_raw(compressed_public_key);
    }

    fn remove(
        validator_id: &T::AccountId,
        ingress_counter: IngressCounter,
        action_type: ValidatorsActionType,
        eth_transaction_type: EthTransactionType) -> DispatchResult
    {
        let mut validator_account_ids = Self::validator_account_ids().ok_or(Error::<T>::NoValidators)?;

        ensure!(Self::get_ingress_counter() + 1 == ingress_counter, Error::<T>::InvalidIngressCounter);
        ensure!(validator_account_ids.len() > DEFAULT_MINIMUM_VALIDATORS_COUNT, Error::<T>::MinimumValidatorsReached);
        ensure!(!<ValidatorActions<T>>::contains_key(validator_id, ingress_counter), Error::<T>::RemovalAlreadyRequested);

        let maybe_validator_index = validator_account_ids.iter().position(|v| v == validator_id);
        if maybe_validator_index.is_none() {
            // exit early if deregistration is not in the system. As dicussed, we don't want to give any feedback if the validator is not found.
            return Ok(());
        }

        let index_of_validator_to_remove = maybe_validator_index.expect("checked for none already");

        // TODO: decide if this is the best way to handle this
        let eth_tx_sender = AVN::<T>::calculate_primary_validator(<system::Module<T>>::block_number())
            .map_err(|_| Error::<T>::ErrorCalculatingPrimaryValidator)?;

        let tx_id = T::CandidateTransactionSubmitter::reserve_transaction_id(&eth_transaction_type)?;

        TotalIngresses::put(ingress_counter);
        <ValidatorActions<T>>::insert(
            validator_id,
            ingress_counter,
            ValidatorsActionData::new(ValidatorsActionStatus::AwaitingConfirmation, eth_tx_sender, tx_id, action_type, eth_transaction_type),
        );
        validator_account_ids.swap_remove(index_of_validator_to_remove);
        <ValidatorAccountIds<T>>::put(validator_account_ids);

        Ok(())
    }

    fn remove_ethereum_public_key_if_required(validator_id: &T::AccountId) {
        let public_key_to_remove = Self::get_ethereum_public_key_if_exists(&validator_id);
        if let Some(public_key_to_remove) = public_key_to_remove {
            <EthereumPublicKeys<T>>::remove(public_key_to_remove);
        }
    }

    fn get_ethereum_public_key_if_exists(account_id: &T::AccountId) -> Option<ecdsa::Public> {
        return <EthereumPublicKeys<T>>::iter()
            .filter(|(_, acc)| acc == account_id)
            .map(|(pk, _)| pk)
            .nth(0);
    }

    fn validator_permanently_removed(
        active_validators: &Vec<Validator<T::AuthorityId, T::AccountId>>,
        disabled_validators: &Vec<T::AccountId>,
        deregistered_validator: &T::AccountId) -> bool
    {
        // if the validator exists in either vectors then they have not been removed from the session
        return !active_validators.iter().any(|v| &v.account_id == deregistered_validator) &&
               !disabled_validators.iter().any(|v| v == deregistered_validator);
    }

    fn remove_slashed_validator(slashed_validator_id: &<T as SessionConfig>::ValidatorId) -> DispatchResult {
        let slashed_validator = &T::AccountToBytesConvert::try_from_any(slashed_validator_id.encode())?;

        if !AVN::<T>::is_validator(slashed_validator) {
            return Err(Error::<T>::SlashedValidatorIsNotFound)?;
        }

        let candidate_tx = EthTransactionType::SlashValidator(
            SlashValidatorData::new(T::AccountToBytesConvert::into_bytes(slashed_validator))
        );

        let ingress_counter = Self::get_ingress_counter() + 1;
        Self::remove(slashed_validator, ingress_counter, ValidatorsActionType::Slashed, candidate_tx)?;
        AVN::<T>::remove_validator_from_active_list(slashed_validator);

        Self::remove_ethereum_public_key_if_required(&slashed_validator);

        Self::deposit_event(Event::<T>::ValidatorSlashed(ActionId{
            action_account_id: slashed_validator.clone(),
            ingress_counter: ingress_counter
        }));

        return Ok(());
    }

    fn remove_resigned_validator(resigned_validator: &T::AccountId) -> DispatchResult {
        let candidate_tx = EthTransactionType::DeregisterValidator(
            DeregisterValidatorData::new(T::AccountToBytesConvert::into_bytes(resigned_validator))
        );
        let ingress_counter = Self::get_ingress_counter() + 1;
        return Self::remove(resigned_validator, ingress_counter, ValidatorsActionType::Resignation, candidate_tx);
    }

    fn can_setup_voting_to_activate_validator(
        validators_action_data: &ValidatorsActionData<T::AccountId>,
        action_account_id: &T::AccountId,
        active_validators: &Vec<Validator<T::AuthorityId, T::AccountId>>
    ) -> bool {
        return validators_action_data.status == ValidatorsActionStatus::AwaitingConfirmation &&
            validators_action_data.action_type == ValidatorsActionType::Activation &&
            active_validators.iter().any(|v| &v.account_id == action_account_id);
    }

    fn setup_voting_to_activate_validator(
        ingress_counter: IngressCounter,
        validator_to_activate: &T::AccountId,
        quorum: u32,
        voting_period_end: T::BlockNumber
    ){
        <ValidatorActions<T>>::mutate(&validator_to_activate, ingress_counter, |validators_action_data|
            validators_action_data.status = ValidatorsActionStatus::Confirmed);

        <PendingApprovals<T>>::insert(&validator_to_activate, ingress_counter);

        let action_id = ActionId::new(validator_to_activate.clone(), ingress_counter);
        <VotesRepository<T>>::insert(
            action_id.clone(),
            VotingSessionData::new(
                action_id.encode(),
                quorum,
                voting_period_end,
                <system::Module<T>>::block_number()
            )
        );
        Self::deposit_event(RawEvent::ValidatorActivationStarted(validator_to_activate.clone()));
    }

    fn deregistration_state_is_active(status: ValidatorsActionStatus) -> bool {
        return vec![ValidatorsActionStatus::AwaitingConfirmation, ValidatorsActionStatus::Confirmed].contains(&status);
    }

    fn has_active_slash(validator_account_id: &T::AccountId) -> bool {
        return <ValidatorActions<T>>::iter_prefix_values(validator_account_id).any(|validators_action_data| {
            validators_action_data.action_type == ValidatorsActionType::Slashed &&
            Self::deregistration_state_is_active(validators_action_data.status)
        });
    }

    /// The account ID of the validators manager pot.
    /// This actually does computation. If you need to keep using it, then make sure you cache the
    /// value and only call this once.
    pub fn account_id() -> T::AccountId {
        T::ModuleId::get().into_account()
    }

    /// The total amount of funds stored in this pallet
    pub fn pot() -> BalanceOf<T> {
        // Must never be less than 0 but better be safe.
        CurrencyOf::<T>::free_balance(&Self::account_id()).saturating_sub(CurrencyOf::<T>::minimum_balance())
    }

    fn verify_signature(proof: &Proof<T::Signature, T::AccountId>, signed_payload: &[u8]) -> Result<(), Error<T>> {
        match proof.signature.verify(signed_payload, &proof.signer) {
            true => Ok(()),
            false => Err(<Error<T>>::UnauthorizedProxyTransaction.into()),
        }
    }
}

#[derive(Encode, Decode, Default, Clone, Copy, PartialEq, Debug, Eq)]
pub struct ActionId<AccountId: Member> {
    pub action_account_id: AccountId,
    pub ingress_counter: IngressCounter
}

impl<AccountId: Member> ActionId<AccountId> {
    fn new(action_account_id: AccountId, ingress_counter: IngressCounter) -> Self {
        return ActionId::<AccountId> {
            action_account_id,
            ingress_counter
        }
    }
}

impl<T: Config> NewSessionHandler<T::AuthorityId, T::AccountId> for Module<T> {
    fn on_genesis_session(_validators: &Vec<Validator<T::AuthorityId, T::AccountId>>) { }

    fn on_new_session(
        _changed: bool,
        active_validators: &Vec<Validator<T::AuthorityId, T::AccountId>>,
        disabled_validators: &Vec<T::AccountId>)
    {
        if <ValidatorActions<T>>::iter().count() > 0 {
            let quorum = calculate_two_third_quorum(AVN::<T>::validators().len() as u32);
            let voting_period_end = safe_add_block_numbers(<system::Module<T>>::block_number(), T::VotingPeriod::get());

            if let Err(e) = voting_period_end {
                debug::native::error!("💔 Unable to calculate voting period end: {:?}", e);
                return;
            }

            for (action_account_id, ingress_counter, validators_action_data) in <ValidatorActions<T>>::iter() {
                // TODO: Investigate if can_setup_voting_to_activate_validator can be used for deregistration as well
                if validators_action_data.status == ValidatorsActionStatus::AwaitingConfirmation &&
                    validators_action_data.action_type.is_deregistration() &&
                   Self::validator_permanently_removed(&active_validators, &disabled_validators, &action_account_id)
                {
                    <ValidatorActions<T>>::mutate(&action_account_id, ingress_counter, |validators_action_data|
                        validators_action_data.status = ValidatorsActionStatus::Confirmed);

                    <PendingApprovals<T>>::insert(&action_account_id, ingress_counter);

                    Self::remove_ethereum_public_key_if_required(&action_account_id);

                    let action_id = ActionId::new(action_account_id, ingress_counter);
                    <VotesRepository<T>>::insert(
                        action_id.clone(),
                        VotingSessionData::new(
                            action_id.encode(),
                            quorum,
                            voting_period_end.expect("already checked"),
                            <system::Module<T>>::block_number()
                        )
                    );

                    Self::deposit_event(RawEvent::ValidatorActionConfirmed(action_id));
                } else if Self::can_setup_voting_to_activate_validator(&validators_action_data, &action_account_id, active_validators) {
                    Self::setup_voting_to_activate_validator(
                        ingress_counter,
                        &action_account_id,
                        calculate_two_third_quorum(active_validators.len() as u32),
                        voting_period_end.expect("already checked")
                    );
                }
            }
        }
    }
}

impl Default for ValidatorsActionStatus {
    fn default() -> Self { ValidatorsActionStatus::None }
}

impl Default for ValidatorsActionType {
    fn default() -> Self { ValidatorsActionType::Unknown }
}

impl<T: Config> frame_support::unsigned::ValidateUnsigned for Module<T> {
    type Call = Call<T>;

    fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
        if let Call::end_voting_period(deregistered_validator, validator, signature) = call {
            let voting_session = Self::get_voting_session(deregistered_validator);
            return end_voting_period_validate_unsigned::<T>(&voting_session, validator, signature);

        } else if let Call::approve_validator_action(action_id, validator, eth_signature, signature) = call {
            if !<ValidatorActions<T>>::contains_key(&action_id.action_account_id, action_id.ingress_counter) {
                return InvalidTransaction::Custom(ERROR_CODE_INVALID_DEREGISTERED_VALIDATOR).into();
            }

            let voting_session = Self::get_voting_session(action_id);
            let eth_encoded_data = Self::convert_data_to_eth_compatible_encoding(action_id)
                .map_err(|_| InvalidTransaction::Custom(ERROR_CODE_INVALID_DEREGISTERED_VALIDATOR))?;
            return approve_vote_validate_unsigned::<T>(&voting_session, validator, eth_encoded_data.encode(), eth_signature, signature);

        } else if let Call::reject_validator_action(deregistered_validator, validator, signature) = call {
            let voting_session = Self::get_voting_session(deregistered_validator);
            return reject_vote_validate_unsigned::<T>(&voting_session, validator, signature);

        } else {
            return InvalidTransaction::Call.into();
        }
    }
}

impl<T: Config> EthereumPublicKeyChecker<T::AccountId> for Module<T> {
    fn get_validator_for_eth_public_key(eth_public_key: &ecdsa::Public) -> Option<T::AccountId> {
        if !<EthereumPublicKeys<T>>::contains_key(eth_public_key) {
            return None;
        }

        return Some(Self::get_validator_by_eth_public_key(eth_public_key));
    }
}

impl<T: Config> DisabledValidatorChecker<T::AccountId> for Module<T> {
    fn is_disabled(validator_account_id: &T::AccountId) -> bool {
        return Self::has_active_slash(validator_account_id);
    }
}

impl<T: Config> Enforcer<<T as session::Config>::ValidatorId> for Module<T> {
    fn slash_validator(slashed_validator_id: &<T as session::Config>::ValidatorId) -> DispatchResult {
        return Self::remove_slashed_validator(slashed_validator_id);
    }
}

impl<T: Config> EraPayout<BalanceOf<T>> for Module<T> {
    /*
    era_payout example:
        Era 0 reward = 10 AVT
            ** Pot = 10
            ** payout = 10
            ** LockedEraPayout = 10

        Era 1 reward = 5 AVT
            ** Pot = 15
            ** payout = 5
            ** LockedEraPayout = 15
    */
    fn era_payout(_total_staked: BalanceOf<T>, _total_issuance: BalanceOf<T>, _era_millis: u64) -> (BalanceOf<T>, BalanceOf<T>)
    {
        let pot = Self::pot();
        let mut payout = pot.checked_sub(&Self::locked_era_payout()).or_else(|| {
            frame_support::debug::native::error!(
                target: LOG_TARGET,
                "💔 💔 Error calculating era payout. Not enough funds in pot."
            );

            //This is a bit strange but since we are dealing with money, log it.
            Module::<T>::deposit_event(RawEvent::NotEnoughFundsForEraPayment(pot));
            Some(BalanceOf::<T>::zero())
        }).expect("We have a default value");

        <LockedEraPayout<T>>::mutate(|lp| {
            *lp = lp.checked_add(&payout).or_else(|| {
                frame_support::debug::native::error!(
                    target: LOG_TARGET,
                    "💔 💔 Error - locked_era_payout overflow. Reducing era payout"
                );
                // In the unlikely event where the value will overflow the LockedEraPayout, return the difference to avoid errors
                payout = BalanceOf::<T>::max_value().saturating_sub(Self::locked_era_payout());
                Some(BalanceOf::<T>::max_value())
            }).expect("We have a default value");
        });

        // make sure the second parameter is ALWAYS 0
        (payout, 0u32.into())
    }
}

pub struct PositiveImbalanceHandler<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> OnUnbalanced<PositiveImbalanceOf<T>> for PositiveImbalanceHandler<T> {
    // This function will be called when a validator or a nominator is paid rewards.
    // The action of paying creates a positive imbalance in the chain, which means we need to deduct this amount
    // from somewhere to restore the balance
    fn on_nonzero_unbalanced(imbalance: PositiveImbalanceOf<T>) {
        let numeric_amount: BalanceOf<T> = imbalance.peek().into();

        // settle will take money out of the pot
        if let Err(err) = <<T as pallet_staking::Config>::Currency>::settle(
            &Module::<T>::account_id(),
            imbalance,
            WithdrawReasons::TRANSFER,
            KeepAlive)
        {
            frame_support::debug::native::error!(
                target: LOG_TARGET,
                "💔 💔 Error withdrawing money from ValidatorsManager pallet to pay for staking rewards"
            );

            // Store the amounts we failed to withdraw so we can fix this manually
            <FailedRewardPayments<T>>::insert(numeric_amount, false);
            drop(err);

            // exit function here
            return;
        }

        // Update storage with the amount we paid
        <LockedEraPayout<T>>::mutate(|p| {
            *p = p.saturating_sub(numeric_amount.into());
        });

        Module::<T>::deposit_event(RawEvent::RewardPotWithdrawal(numeric_amount));
    }
}

impl<T: Config> OnUnbalanced<NegativeImbalanceOf<T>> for Module<T> {
    // This function will be called when a fee paying transaction is executed and gas or tip is taken
    fn on_nonzero_unbalanced(amount: NegativeImbalanceOf<T>) {
        let numeric_amount = amount.peek();

        // Must resolve into existing but better to be safe.
        let _ = CurrencyOf::<T>::resolve_creating(&Module::<T>::account_id(), amount);

        // Instead of raising an event, add a trace log. Otherwise the event will be emitted for all paying transactions
        frame_support::debug::native::trace!(target: LOG_TARGET, "Deposited {:?} in the reward pot", numeric_amount);
    }
}

impl<T: Config> InnerCallValidator for Module<T> {
    type Call = <T as Config>::Call;

    fn signature_is_valid(call: &Box<Self::Call>) -> bool {
        if let Some((proof, signed_payload)) = get_encoded_call_param::<T>(call) {
            return Self::verify_signature(&proof, &signed_payload.as_slice()).is_ok();
        }

        return false;
    }
}

// A value placed in storage that represents the current version of the Staking storage. This value
// is used by the `on_runtime_upgrade` logic to determine whether we run storage migration logic.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq)]
enum Releases {
    Unknown,
    V2_0_0
}

impl Default for Releases {
    fn default() -> Self {
        Releases::V2_0_0
    }
}
