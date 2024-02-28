//! # Ethereum event checker Pallet
//!
//! This pallet provides functionality to get ethereum events.

#![cfg_attr(not(feature = "std"), no_std)]

// TODO [TYPE: review][PRI: low]: Find a way of not using strings directly in the runtime. (probably irrelevant)
#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}};
use simple_json2::json::JsonValue;
use sp_std::{prelude::*, cmp};
use frame_support::{
    Parameter,
    decl_event,
    decl_storage,
    decl_module,
    decl_error,
    dispatch::DispatchResult,
    ensure,
    debug,
    traits::{Get, IsSubType},
    weights::Weight,
    dispatch::DispatchResultWithPostInfo,
};
use frame_system::{self as system, ensure_signed, ensure_none, ensure_root,
    offchain::{SendTransactionTypes, SubmitTransaction}
};
use sp_core::{H256, H160};
use sp_runtime::{
    DispatchError,
    offchain::{http, Duration, storage::StorageValueRef},
    traits::{CheckedAdd, Dispatchable, Hash, IdentifyAccount, Member, Verify, Zero},
    transaction_validity::{
        TransactionValidity,
        ValidTransaction,
        InvalidTransaction,
        TransactionPriority,
        TransactionSource
    },
};

use simple_json2::{self as json};
use sp_application_crypto::RuntimeAppPublic;
use codec::{Encode, Decode};
use sp_avn_common::{
    event_types::{
        EthEventId, EthEventCheckResult, CheckResult, ValidEvents, ChallengeReason, Challenge, ProcessedEventHandler, Validator,
        EventData, AddedValidatorData, LiftedData, NftMintData, NftTransferToData, NftCancelListingData, NftEndBatchListingData
    },
    IngressCounter,
    Proof,
    InnerCallValidator
};

use pallet_session::historical::IdentificationTuple;
use sp_staking::offence::ReportOffence;

use pallet_avn::{self as avn, Error as avn_error};
pub mod offence;
use crate::offence::{InvalidEthereumLogOffence, EthereumLogOffenceType, create_and_report_invalid_log_offence};

pub mod event_parser;
use crate::event_parser::{get_events, find_event, get_data, get_topics, get_status, get_num_confirmations};

pub type AVN<T> = avn::Module::<T>;

const VALIDATED_EVENT_LOCAL_STORAGE: &'static [u8; 28] = b"eth_events::validated_events";

const NAME: &'static [u8; 20] = b"eth_events::last_run";

const ERROR_CODE_EVENT_NOT_IN_UNCHECKED: u8 = 0;
const ERROR_CODE_INVALID_EVENT_DATA: u8 = 1;
const ERROR_CODE_IS_PRIMARY_HAS_ERROR: u8 = 2;
const ERROR_CODE_VALIDATOR_NOT_PRIMARY: u8 = 3;
const ERROR_CODE_EVENT_NOT_IN_PENDING_CHALLENGES: u8 = 4;

const MINIMUM_EVENT_CHALLENGE_PERIOD: u32 = 60;

pub const SIGNED_ADD_ETHEREUM_LOG_CONTEXT: &'static [u8] = b"authorization for add ethereum log operation";

#[cfg(test)]
mod mock;

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/session_handler_tests.rs"]
mod session_handler_tests;

#[cfg(test)]
#[path = "tests/test_offchain_worker_calls.rs"]
mod test_offchain_worker_calls;

#[path = "tests/test_offchain_worker.rs"]
mod test_offchain_worker;

#[path = "tests/test_process_event.rs"]
mod test_process_event;

#[path = "tests/test_parse_event.rs"]
mod test_parse_event;

#[cfg(test)]
#[path = "tests/test_challenges.rs"]
mod test_challenges;

#[cfg(test)]
#[path = "tests/test_set_ethereum_contract.rs"]
mod test_set_ethereum_contract;

#[cfg(test)]
#[path = "tests/test_set_event_challenge_period.rs"]
mod test_set_event_challenge_period;

#[cfg(test)]
#[path = "tests/test_initial_events.rs"]
mod test_initial_events;

#[cfg(test)]
#[path = "tests/test_ethereum_logs.rs"]
mod tests_ethereum_logs;

#[cfg(test)]
#[path = "tests/test_proxy_signed_add_ethereum_logs.rs"]
mod test_proxy_signed_add_ethereum_logs;

mod benchmarking;

// TODO: [TYPE: business logic][PRI: high][CRITICAL]
// Rerun benchmark in production and update both ./default_weights.rs file and /bin/node/runtime/src/weights/pallet_ethereum_events.rs file.
pub mod default_weights;
pub use default_weights::WeightInfo;

#[derive(Encode, Decode, Clone, PartialEq, Debug, Eq)]
pub enum EthereumContracts {
    ValidatorsManager,
    Lifting,
    NftMarketplace,
}

pub trait ProcessedEventsChecker {
    fn check_event(event_id: &EthEventId) -> bool;
}

const SUBMIT_CHECKEVENT_RESULT_CONTEXT: &'static [u8] = b"submit_checkevent_result";
const CHALLENGE_EVENT_CONTEXT: &'static [u8] = b"challenge_event";
const PROCESS_EVENT_CONTEXT: &'static [u8] = b"process_event";

const MAX_NUMBER_OF_VALIDATORS_ACCOUNTS: u32 = 10;
const MAX_NUMBER_OF_UNCHECKED_EVENTS: u32 = 5;
const MAX_NUMBER_OF_EVENTS_PENDING_CHALLENGES: u32 = 5;
const MAX_CHALLENGES: u32 = 10;

// Public interface of this pallet
pub trait Config: SendTransactionTypes<Call<Self>> + system::Config + avn::Config + pallet_session::historical::Config {
    type Event: From<Event<Self>> + Into<<Self as system::Config>::Event>;

    type Call: Parameter + Dispatchable<Origin=<Self as frame_system::Config>::Origin> + IsSubType<Call<Self>> + From<Call<Self>>;

    type ProcessedEventHandler: ProcessedEventHandler;

    /// Minimum number of blocks that have passed after an ethereum transaction has been mined
    type MinEthBlockConfirmation: Get<u64>;

    ///  A type that gives the pallet the ability to report offences
    type ReportInvalidEthereumLog: ReportOffence<
            Self::AccountId,
            IdentificationTuple<Self>,
            InvalidEthereumLogOffence<IdentificationTuple<Self>>,
        >;

    /// A type that can be used to verify signatures
    type Public: IdentifyAccount<AccountId = Self::AccountId>;

    /// The signature type used by accounts/transactions.
    type Signature: Verify<Signer = Self::Public> + Member + Decode + Encode + From<sp_core::sr25519::Signature>;

    /// Weight information for the extrinsics in this pallet.
    type WeightInfo: WeightInfo;
}

decl_event!(
    pub enum Event<T> where
        <T as system::Config>::BlockNumber,
        AccountId = <T as system::Config>::AccountId,
        Relayer = <T as system::Config>::AccountId,
        Hash = <T as system::Config>::Hash,
        IdentificationTuple = IdentificationTuple<T>,
        EthereumLogOffenceType = EthereumLogOffenceType
    {
        // T1 Event added to the pending queue
        /// EthereumEventAdded(EthEventId, AddedBy, T1 contract address)
        EthereumEventAdded(EthEventId, AccountId, H160),
        // T1 Event's validity checked (does it exist?)
        /// EventValidated(EthEventId, CheckResult, ValidatedBy)
        EventValidated(EthEventId, CheckResult, AccountId),
        /// EventProcessed(EthEventId, Processor, Outcome)
        EventProcessed(EthEventId, AccountId, bool),
        /// EventChallenged(EthEventId, Challenger, ChallengeReason)
        EventChallenged(EthEventId, AccountId, ChallengeReason),
        /// ChallengeSucceeded(T1 event, CheckResult)
        ChallengeSucceeded(EthEventId, CheckResult),
        /// OffenceReported(OffenceType, Offenders)
        OffenceReported(EthereumLogOffenceType, Vec<IdentificationTuple>),
        /// EventAccepted(EthEventId)
        EventAccepted(EthEventId),
        /// EventRejected(EthEventId, CheckResult, HasSuccessfullChallenge)
        EventRejected(EthEventId, CheckResult, bool),
        /// EventChallengePeriodUpdated(EventChallengePeriodInBlocks)
        EventChallengePeriodUpdated(BlockNumber),
        CallDispatched(Relayer, Hash),
        /// NFT related Ethereum event was added(EthEventId, AddedBy)
        NftEthereumEventAdded(EthEventId, AccountId),
    }
);

decl_error! {
	pub enum Error for Module<T: Config> {
        DuplicateEvent,
        MissingEventToCheck,
        UnrecognizedEventSignature,
        EventParsingFailed,
        ErrorSigning,
        ErrorSubmittingTransaction,
        InvalidKey,
        PendingChallengeEventNotFound,
        InvalidEventToChallenge,
        Overflow,
        DuplicateChallenge,
        ErrorSavingValidationToLocalDB,
        MalformedHash,
        InvalidEventToProcess,
        ChallengingOwnEvent,
        InvalidContractAddress,
        InvalidContractType,
        InvalidEventChallengePeriod,
        SenderIsNotSigner,
        UnauthorizedTransaction,
        UnauthorizedSignedAddEthereumLogTransaction,
	}
}

decl_storage! {
    trait Store for Module<T: Config> as EthereumEvents {
        // TODO [TYPE: refactoring][PRI: low]: replace these contract addresses by a map.
        // (note: low value. This is simple to use, and there are few contracts)
        pub ValidatorManagerContractAddress get(fn validator_manager_contract_address) config(): H160;
        pub LiftingContractAddress get(fn lifting_contract_address) config(): H160;
        // Progress of T1 events onto T2

        // TODO: Replace this with the one defined in pallet_avn.
        pub TotalIngresses get(fn ingress_counter): IngressCounter;

        pub UncheckedEvents get(fn unchecked_events): Vec<(EthEventId, IngressCounter, T::BlockNumber)>;
        pub EventsPendingChallenge get(fn events_pending_challenge):
            Vec<(EthEventCheckResult<T::BlockNumber, T::AccountId>, IngressCounter, T::BlockNumber)>;

        // Should be a set as requires quick access but Substrate doesn't support sets: they recommend using a bool HashMap.
        // This map holds all events that have been processed, regardless of the outcome of the execution of the events.
        pub ProcessedEvents get(fn processed_events) config(): map hasher(blake2_128_concat) EthEventId => bool;

        pub Challenges get(fn challenges): map hasher(blake2_128_concat) EthEventId => Vec<T::AccountId>;

        /// The factor of the total validators specifying the threshold for successful challenges
        pub QuorumFactor get(fn quorum_factor) config(): u32;

        /// A period (in block number) where an event cannot be processed
        pub EventChallengePeriod get(fn event_challenge_period) config(): T::BlockNumber;

        /// A map containing the tier1 contracts for NFT marketplaces
        pub NftT1Contracts get(fn nft_t1_contracts) config(): map hasher(blake2_128_concat) H160 => ();

        /// An account nonce that represents the number of proxy transactions from this account
        pub ProxyNonces get(fn proxy_nonce): map hasher(blake2_128_concat) T::AccountId => u64;

        /// Track the version of this storage. Mainly used for storage migration.
        StorageVersion: Releases;
    }
    add_extra_genesis {
        config(lift_tx_hashes): Vec<H256>;
        build(|config: &GenesisConfig<T>| {
            let unchecked_lift_events = config.lift_tx_hashes.iter()
                .map(|&tx_hash| {
                    let ingress_counter = Module::<T>::get_next_ingress_counter();
                    return (
                        EthEventId {
                            signature: ValidEvents::Lifted.signature(),
                            transaction_hash: tx_hash,
                        },
                        ingress_counter,
                        <T as system::Config>::BlockNumber::zero()
                    );
                })
                .collect::<Vec<(EthEventId, IngressCounter, T::BlockNumber)>>();
            <UncheckedEvents<T>>::put(unchecked_lift_events);
            assert_ne!(config.quorum_factor, 0, "Quorum factor cannot be 0");
        });
    }
}


decl_module! {
    pub struct Module<T: Config> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;


        // # <weight>
        // Keys: U - Number of unchecked events
        //       E - Number of pending challenge events
        //   DbReads: `ValidatorManagerContractAddress`, `ProcessedEvents`, `TotalIngresses`: O(1)
        //   DbWrites: `TotalIngresses`, `UncheckedEvents`: O(1)
        //   Iterate UncheckedEvents vector: O(U)
        //   Iterate EventsPendingChallenge vector: O(E)
        //   Emitted Event: `EthereumEventAdded`: O(1)
        // Total Complexity: O(1 + U + E)
        // # </weights>
        /// This extrinsic is being deprecated. Use add_ethereum_log
        // We need to maintain this till SYS-888 is resolved. After that it can be removed.
        #[weight = <T as Config>::WeightInfo::add_validator_log(
            MAX_NUMBER_OF_UNCHECKED_EVENTS,
            MAX_NUMBER_OF_EVENTS_PENDING_CHALLENGES
        )]
        pub fn add_validator_log(origin, tx_hash: H256) -> DispatchResult {
            let account_id = ensure_signed(origin)?;
            ensure!(&tx_hash != &H256::zero(), Error::<T>::MalformedHash);

            // TODO [TYPE: weightInfo][PRI: medium]: Return accurate weight
            return Self::add_event(ValidEvents::AddedValidator, tx_hash, account_id);
        }

        // # <weight>
        // Keys: U - Number of unchecked events
        //       E - Number of pending challenge events
        //   DbReads: `LiftingContractAddress`, `ProcessedEvents`, `TotalIngresses`: O(1)
        //   DbWrites: `TotalIngresses`, `UncheckedEvents`: O(1)
        //   Iterate UncheckedEvents vector: O(U)
        //   Iterate EventsPendingChallenge vector: O(E)
        //   Emitted Event: `EthereumEventAdded`: O(1)
        // Total Complexity: O(1 + U + E)
        // # </weights>
        /// This extrinsic is being deprecated. Use add_ethereum_log
        // We need to maintain this till SYS-888 is resolved. After that it can be removed.
        #[weight = <T as Config>::WeightInfo::add_lift_log(
            MAX_NUMBER_OF_UNCHECKED_EVENTS,
            MAX_NUMBER_OF_EVENTS_PENDING_CHALLENGES
        )]
        pub fn add_lift_log(origin, tx_hash: H256) -> DispatchResult {
            let account_id = ensure_signed(origin)?;
            ensure!(&tx_hash != &H256::zero(), Error::<T>::MalformedHash);

            // TODO [TYPE: weightInfo][PRI: medium]: Return accurate weight
            return Self::add_event(ValidEvents::Lifted, tx_hash, account_id);
        }

        /// # <weight>
        /// Keys: V - number of validators
        ///       U - number of unchecked events
        ///   DbReads: `QuorumFactor`: O(1)
        ///   DbWrites: `EventsPendingChallenge`, `UncheckedEvents`: O(1)
        ///   avn pallet is_validator operation: O(V)
        ///   Iterate unchecked_events vector operation: O(U)
        ///   Emitted event: `EventValidated`: O(1)
        /// Total Complexity: O(1 + V + U)
        /// # </weight>
        #[weight = <T as Config>::WeightInfo::submit_checkevent_result(
            MAX_NUMBER_OF_VALIDATORS_ACCOUNTS,
            MAX_NUMBER_OF_UNCHECKED_EVENTS
        )]
        fn submit_checkevent_result(
            origin,
            result: EthEventCheckResult<T::BlockNumber, T::AccountId>,
            ingress_counter: u64,
            // Signature and structural validation is already done in validate unsigned so no need to do it here
            // This is not used, but we must have this field so it can be used in the logic of validate_unsigned
            _signature: <T::AuthorityId as RuntimeAppPublic>::Signature,
            _validator: Validator<T::AuthorityId, T::AccountId>) -> DispatchResult
        {
            ensure_none(origin)?;
            ensure!(Self::is_validator(&result.checked_by), Error::<T>::InvalidKey);

            let event_index = Self::unchecked_events().iter().position(
                |(event, counter, _)| event == &result.event.event_id && counter == &ingress_counter);
            if let Some(event_index) = event_index {
                let current_block = <system::Module<T>>::block_number();
                let mut result = result;
                result.ready_for_processing_after_block = current_block
                    .checked_add(&Self::event_challenge_period())
                    .ok_or(Error::<T>::Overflow)?
                    .into();
                result.min_challenge_votes = (AVN::<T>::active_validators().len() as u32) / Self::quorum_factor();

                // Insert first and remove
                <EventsPendingChallenge<T>>::mutate(|pending_events|
                    pending_events.push((result.clone(), ingress_counter, current_block))
                );

                <UncheckedEvents<T>>::mutate(|events| events.remove(event_index));

                Self::deposit_event(Event::<T>::EventValidated(result.event.event_id, result.result, result.checked_by));
            } else {
                Err(Error::<T>::MissingEventToCheck)?
            }

            // TODO [TYPE: weightInfo][PRI: medium]: Return accurate weight
            Ok(())
        }

        /// # <weight>
        /// Keys: E - number of events pending challenge
        ///       V - number of validators
        ///   DbWrites: `EventsPendingChallenge`, `ProcessedEvents`, `QuorumFactor`, `Challenges`
        ///   Iterate EventsPendingChallenge vector: O(E)
        ///   avn pallet operations:
        ///     - is_validator operation: O(V)
        ///     - DbReads: `Validators`: O(1)
        ///   If challenge is successful:
        ///     - Create and report invalid log offence: O(1)
        ///   Emitted event: `EventProcessed`, `ChallengeSucceeded`, : O(1)
        /// Total Complexity: O(1 + E + V)
        /// #</weight>
        #[weight = <T as Config>::WeightInfo::process_event_with_successful_challenge(
                MAX_NUMBER_OF_VALIDATORS_ACCOUNTS,
                MAX_NUMBER_OF_EVENTS_PENDING_CHALLENGES
            ).max(<T as Config>::WeightInfo::process_event_without_successful_challenge(
                MAX_NUMBER_OF_VALIDATORS_ACCOUNTS,
                MAX_NUMBER_OF_EVENTS_PENDING_CHALLENGES
            )
        )]
        fn process_event(origin,
            event_id: EthEventId,
            _ingress_counter: IngressCounter, // this is not used in this function, but is added here so that `_signature` can use this value to become different from previous calls.
            validator: Validator<T::AuthorityId, T::AccountId>,
            // Signature and structural validation is already done in validate unsigned so no need to do it here
            _signature: <T::AuthorityId as RuntimeAppPublic>::Signature) -> DispatchResultWithPostInfo
        {
            ensure_none(origin)?;
            ensure!(Self::is_validator(&validator.account_id), Error::<T>::InvalidKey);

            let event_index = Self::get_pending_event_index(&event_id)?;
            // Not using the passed in `checked` to be sure the details have not been changed
            let (validated, _ingress_counter, _) = &Self::events_pending_challenge()[event_index];

            ensure!(
                <system::Module<T>>::block_number() > validated.ready_for_processing_after_block,
                Error::<T>::InvalidEventToProcess
            );

            let successful_challenge = Self::is_challenge_successful(validated);

            // Once an event is added to the `ProcessedEvents` set, it cannot be processed again.
            // If there is a successfull challenge on an `Invalid` event, it means the event should
            // have been valid so DO NOT add it to the processed set to allow the event to be processed again in the future.
            let event_was_declared_invalid = validated.result == CheckResult::Invalid;
            let event_can_be_resubmitted = event_was_declared_invalid && successful_challenge;
            if !event_can_be_resubmitted {
                <ProcessedEvents>::insert(event_id.clone(), true);
            }
            <EventsPendingChallenge<T>>::mutate(|pending_events| pending_events.remove(event_index));
            // TODO: Remove this event's challenges from the Challenges map too.
            Self::deposit_event(Event::<T>::EventProcessed(event_id.clone(), validator.account_id.clone(), !successful_challenge));

            if successful_challenge {
                Self::deposit_event(Event::<T>::ChallengeSucceeded(event_id.clone(), validated.result.clone()));

                // Now report the offence of the validator who submitted the check
                create_and_report_invalid_log_offence::<T>(
                    &validator.account_id,
                    &vec![validated.checked_by.clone()],
                    EthereumLogOffenceType::IncorrectValidationResultSubmitted,
                );
            } else {
                // SYS-536 report the offence for the people who challenged
                create_and_report_invalid_log_offence::<T>(
                    &validator.account_id,
                    &Self::challenges(event_id.clone()),
                    EthereumLogOffenceType::ChallengeAttemptedOnValidResult,
                );
            }

            if validated.result == CheckResult::Ok && !successful_challenge {
                // Let everyone know we have processed an event.
                T::ProcessedEventHandler::on_event_processed(&validated.event)?;

                Self::deposit_event(Event::<T>::EventAccepted(event_id));
            } else {
                Self::deposit_event(Event::<T>::EventRejected(event_id, validated.result.clone(), successful_challenge));
            }

            let final_weight = if successful_challenge {
                <T as Config>::WeightInfo::process_event_with_successful_challenge(
                    MAX_NUMBER_OF_VALIDATORS_ACCOUNTS,
                    MAX_NUMBER_OF_EVENTS_PENDING_CHALLENGES
                )
            } else {
                <T as Config>::WeightInfo::process_event_without_successful_challenge(
                    MAX_NUMBER_OF_VALIDATORS_ACCOUNTS,
                    MAX_NUMBER_OF_EVENTS_PENDING_CHALLENGES
                )
            };

            // TODO [TYPE: weightInfo][PRI: medium]: Return accurate weight
            Ok(Some(final_weight).into())
        }

        /// # <weight>
        /// Keys: V - number of validators accounts
        ///       E - number of events pending challenge
        ///       C - number of challenges
        ///   DbReads: `EventsPendingChallenge`, 2* `Challenges`: O(1)
        ///   avn pallet operations:
        ///     - is_validator operation: O(V)
        ///     - DbReads: `Validators`: O(1)
        ///   Iterate `EventsPendingChallenge` operation: O(E)
        ///   Iterate `Challenges` operation: O(C)
        ///   DbWrites: `Challenges`: O(1)
        ///   Emitted Event: `EventChallenged`: O(1)
        /// Total Complexity: O(1 + V + E + C)
        /// #</weight>
        #[weight = <T as Config>::WeightInfo::challenge_event(
            MAX_NUMBER_OF_VALIDATORS_ACCOUNTS,
            MAX_NUMBER_OF_EVENTS_PENDING_CHALLENGES,
            MAX_CHALLENGES
        )]
        fn challenge_event(origin,
            challenge: Challenge<T::AccountId>,
            ingress_counter: IngressCounter,
            _signature: <T::AuthorityId as RuntimeAppPublic>::Signature,
            _validator: Validator<T::AuthorityId, T::AccountId>) -> DispatchResult
        {
            ensure_none(origin)?;
            ensure!(Self::is_validator(&challenge.challenged_by), Error::<T>::InvalidKey);

            let events_pending_challenge = Self::events_pending_challenge();
            let checked = events_pending_challenge
                .iter()
                .filter(|(e, counter, _)| e.event.event_id == challenge.event_id && *counter == ingress_counter)
                .map(|(event, _counter, _) | event)
                .last(); // returns the most recent occurrence of event_id (in the unexpected case there is more than one)
            ensure!(checked.is_some(), Error::<T>::InvalidEventToChallenge);
            ensure!(checked.expect("Not None").checked_by != challenge.challenged_by, Error::<T>::ChallengingOwnEvent);

            // TODO [TYPE: business logic][PRI: medium][CRITICAL][JIRA: 349]: Make sure the challenge period has not passed
            // Note: the current block number can be different to the block_number the offchain worker was invoked in

            if <Challenges<T>>::contains_key(&challenge.event_id) {
                ensure!(
                    !Self::challenges(challenge.event_id.clone())
                    .iter()
                    .any(|challenger| challenger == &challenge.challenged_by),
                     Error::<T>::DuplicateChallenge
                );

                <Challenges<T>>::mutate(challenge.event_id.clone(), |prev_challenges| {
                   prev_challenges.push(challenge.challenged_by.clone());
                });

            } else {
                <Challenges<T>>::insert(challenge.event_id.clone(), vec![challenge.challenged_by.clone()]);
            }

            Self::deposit_event(Event::<T>::EventChallenged(
                challenge.event_id,
                challenge.challenged_by,
                challenge.challenge_reason));

            // TODO [TYPE: weightInfo][PRI: medium]: Return accurate weight
            Ok(())
        }

        // # <weight>
        // Keys: U - Number of unchecked events
        //       E - Number of pending challenge events
        //   DbReads: ContractAddress, `ProcessedEvents`, `TotalIngresses`: O(1)
        //   DbWrites: `TotalIngresses`, `UncheckedEvents`: O(1)
        //   Iterate UncheckedEvents vector: O(U)
        //   Iterate EventsPendingChallenge vector: O(E)
        //   Emitted Event: `EthereumEventAdded`: O(1)
        // Total Complexity: O(1 + U + E)
        // # </weights>
        /// Submits an ethereum transaction hash into the chain
        #[weight = <T as Config>::WeightInfo::add_ethereum_log(
            MAX_NUMBER_OF_UNCHECKED_EVENTS,
            MAX_NUMBER_OF_EVENTS_PENDING_CHALLENGES
        )]
        pub fn add_ethereum_log(origin, event_type: ValidEvents, tx_hash: H256) -> DispatchResult {
            let account_id = ensure_signed(origin)?;
            ensure!(&tx_hash != &H256::zero(), Error::<T>::MalformedHash);

            // TODO [TYPE: weightInfo][PRI: medium]: Return accurate weight
            return Self::add_event(event_type, tx_hash, account_id);
        }

        // # <weight>
        // Keys: U - Number of unchecked events
        //       E - Number of pending challenge events
        //   DbReads: `ProxyNonces`, ContractAddress, `ProcessedEvents`, `TotalIngresses`: O(1)
        //   DbWrites: `ProxyNonces`, `TotalIngresses`, `UncheckedEvents`: O(1)
        //   Iterate UncheckedEvents vector: O(U)
        //   Iterate EventsPendingChallenge vector: O(E)
        //   Emitted Event: `EthereumEventAdded`: O(1)
        // Total Complexity: O(1 + U + E)
        // # </weight>
        #[weight = <T as Config>::WeightInfo::signed_add_ethereum_log(
            MAX_NUMBER_OF_UNCHECKED_EVENTS,
            MAX_NUMBER_OF_EVENTS_PENDING_CHALLENGES
        )]
        pub fn signed_add_ethereum_log(origin,
            proof: Proof<T::Signature, T::AccountId>,
            event_type: ValidEvents,
            tx_hash: H256
        ) -> DispatchResult {
            let sender = ensure_signed(origin)?;
            ensure!(sender == proof.signer, Error::<T>::SenderIsNotSigner);
            ensure!(&tx_hash != &H256::zero(), Error::<T>::MalformedHash);

            let sender_nonce = Self::proxy_nonce(&sender);
            let signed_payload = Self::encode_signed_add_ethereum_log_params(&proof, &event_type, &tx_hash, sender_nonce);
            ensure!(Self::verify_signature(&proof, &signed_payload.as_slice()).is_ok(),
                Error::<T>::UnauthorizedSignedAddEthereumLogTransaction);

            <ProxyNonces<T>>::mutate(&sender, |n| *n += 1);

            // TODO [TYPE: weightInfo][PRI: medium]: Return accurate weight
            return Self::add_event(event_type, tx_hash, sender);
        }

        // # <weight>
        //  - DbReads: 'NftT1Contracts' access or one of 'ValidatorManagerContractAddress'/'LiftingContractAddress' : O(1)
        //  - DbWrites: 'NftT1Contracts' access or one of 'ValidatorManagerContractAddress'/'LiftingContractAddress' : O(1)
        //  - Total Complexity: O(1)
        // # </weights>
        /// Sets the address for ethereum contracts
        #[weight = <T as Config>::WeightInfo::set_ethereum_contract_map_storage().max(<T as Config>::WeightInfo::set_ethereum_contract_storage())]
        pub fn set_ethereum_contract(origin, contract_type: EthereumContracts, contract_address: H160) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(&contract_address != &H160::zero(), Error::<T>::InvalidContractAddress);

            match contract_type {
                EthereumContracts::ValidatorsManager => <ValidatorManagerContractAddress>::put(contract_address),
                EthereumContracts::Lifting => <LiftingContractAddress>::put(contract_address),
                EthereumContracts::NftMarketplace => <NftT1Contracts>::insert(contract_address, ()),
            };

            Ok(())
        }

        /// Set Ethereum event challenge period in number of blocks
        //
        // # <weight>
        //   DbWrites: EventChallengePeriod: O(1)
        //   Emitted Event: EventChallengePeriodUpdated: O(1)
        //  - Total Complexity: O(1)
        // # </weights>
        #[weight = <T as Config>::WeightInfo::set_event_challenge_period()]
        pub fn set_event_challenge_period(origin, event_challenge_period_in_blocks: T::BlockNumber) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(event_challenge_period_in_blocks >= MINIMUM_EVENT_CHALLENGE_PERIOD.into(), Error::<T>::InvalidEventChallengePeriod);
            EventChallengePeriod::<T>::put(event_challenge_period_in_blocks);
            Self::deposit_event(Event::<T>::EventChallengePeriodUpdated(event_challenge_period_in_blocks));
            Ok(())
        }

        /// Offchain Worker entry point.
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

            // Only primary validators can check and process events
            let is_primary = AVN::<T>::is_primary(block_number, &this_validator.account_id);
            if is_primary.is_err() {
                debug::native::error!("Error checking if validator can check result");
                return
            }

            // =============================== Main Logic ===========================
            if is_primary.expect("Already checked for error.") {
                Self::try_check_event(block_number, &this_validator);
                Self::try_process_event(block_number, &this_validator);
            } else {
                Self::try_validate_event(block_number, &this_validator);
            }
        }

         // Note: this "special" function will run during every runtime upgrade. Any complicated migration logic should be done in a
        // separate function so it can be tested properly.
        fn on_runtime_upgrade() -> Weight {
            if StorageVersion::get() == Releases::Unknown {
                StorageVersion::put(Releases::V2_0_0);
                return migrations::migrate_to_multi_nft_contract::<T>()
            }

            return 0;
        }
    }
}

// implement offchain worker sub-functions
impl<T: Config> Module<T> {
    fn try_check_event(block_number: T::BlockNumber, validator: &Validator<T::AuthorityId, T::AccountId>) {
        let event_to_check = Self::get_events_to_check_if_required();

        if let Some(event_to_check) = event_to_check {
            debug::native::info!("** Checking events");

            let result = Self::check_event_and_submit_result(
                block_number,
                &event_to_check.0,
                event_to_check.1,
                validator);
            if let Err(e) = result {
                debug::native::error!("Error checking for events: {:#?}", e);
            }
        }
    }

    fn try_process_event(block_number: T::BlockNumber, validator: &Validator<T::AuthorityId, T::AccountId>) {
        if let Some((event_to_process, ingress_counter, _)) = Self::get_next_event_to_process(block_number) {
            debug::native::info!("** Processing events");

            let result = Self::send_event(event_to_process, ingress_counter, validator);
            if let Err(e) = result {
                debug::native::error!("Error processing events: {:#?}", e);
            }
        }
    }

    fn try_validate_event(block_number: T::BlockNumber, validator: &Validator<T::AuthorityId, T::AccountId>) {
        if let Some((event_to_validate, ingress_counter, _)) = Self::get_next_event_to_validate(&validator.account_id) {
            debug::native::info!("** Validating events");

            let result = Self::validate_event(block_number, event_to_validate, ingress_counter, validator);
            if let Err(e) = result {
                debug::native::error!("Error validating events: {:#?}", e);
            }
        }
    }
}

impl<T: Config> Module<T> {

    fn is_challenge_successful(validated: &EthEventCheckResult<T::BlockNumber, T::AccountId>) -> bool {
        let required_challenge_votes = (AVN::<T>::active_validators().len() as u32) / Self::quorum_factor();
        let total_num_of_challenges = Self::challenges(validated.event.event_id.clone()).len() as u32;

        return total_num_of_challenges > cmp::max(validated.min_challenge_votes, required_challenge_votes);
    }

    fn get_pending_event_index(event_id: &EthEventId) -> Result<usize, Error<T>> {
        // `rposition: there should be at most one occurrence of this event,
        // but in case there is more, we pick the most recent one
        let event_index = Self::events_pending_challenge().iter().rposition(
            |(pending, _counter, _)| *event_id == pending.event.event_id);
        ensure!(event_index.is_some(), Error::<T>::PendingChallengeEventNotFound);
        return Ok(event_index.expect("Checked for error"));
    }

    fn parse_tier1_event(event_id: EthEventId, data: Option<Vec<u8>>, topics: Vec<Vec<u8>>) -> Result<EventData, Error<T>> {
        // TODO [TYPE: refactoring][PRI: low]: change the error in parse_event to be some standard type, or just ignore the error reason
        // Beware of circular dependencies. Ideally, we want all errors to be of one type (our Error enum)
        if event_id.signature == ValidEvents::AddedValidator.signature() {
            let event_data = <AddedValidatorData>::parse_bytes(data, topics)
                .map_err(|e| {
                    debug::native::warn!("Error parsing T1 AddedValidator Event: {:#?}", e);
                    Error::<T>::EventParsingFailed
                })?;

            return Ok( EventData::LogAddedValidator(event_data) );
        } else if event_id.signature == ValidEvents::Lifted.signature() {
            let event_data = <LiftedData>::parse_bytes(data, topics)
                .map_err(|e| {
                    debug::native::warn!("Error parsing T1 Lifted Event: {:#?}", e);
                    Error::<T>::EventParsingFailed
                })?;
            return Ok( EventData::LogLifted(event_data) );
        } else if event_id.signature == ValidEvents::NftMint.signature() {
            let event_data = <NftMintData>::parse_bytes(data, topics)
                .map_err(|e| {
                    debug::native::warn!("Error parsing T1 AvnMintTo Event: {:#?}", e);
                    Error::<T>::EventParsingFailed
                })?;
            return Ok( EventData::LogNftMinted(event_data) );
        } else if event_id.signature == ValidEvents::NftTransferTo.signature() {
            let event_data = <NftTransferToData>::parse_bytes(data, topics)
                .map_err(|e| {
                    debug::native::warn!("Error parsing T1 AvnTransferTo Event: {:#?}", e);
                    Error::<T>::EventParsingFailed
                })?;
            return Ok( EventData::LogNftTransferTo(event_data) );
        } else if event_id.signature == ValidEvents::NftCancelListing.signature() {
            let event_data = <NftCancelListingData>::parse_bytes(data, topics)
                .map_err(|e| {
                    debug::native::warn!("Error parsing T1 AvnCancelNftListing Event: {:#?}", e);
                    Error::<T>::EventParsingFailed
                })?;
            return Ok( EventData::LogNftCancelListing(event_data) );
        } else if event_id.signature == ValidEvents::NftEndBatchListing.signature() {
            let event_data = <NftEndBatchListingData>::parse_bytes(data, topics)
                .map_err(|e| {
                    debug::native::warn!("Error parsing T1 AvnCancelNftBatchListing Event: {:#?}", e);
                    Error::<T>::EventParsingFailed
                })?;
            return Ok( EventData::LogNftEndBatchListing(event_data) );
        } else {
            return Err(Error::<T>::UnrecognizedEventSignature);
        }
    }

    fn get_events_to_check_if_required() -> Option<(EthEventId, IngressCounter, T::BlockNumber)> {
        if Self::unchecked_events().is_empty() {
            return None;
        }

        return Self::unchecked_events()
            .into_iter()
            .filter(|e| AVN::<T>::is_block_finalised(e.2))
            .nth(0);
    }

    fn get_next_event_to_validate(validator_account_id: &T::AccountId) ->
        Option<(EthEventCheckResult<T::BlockNumber, T::AccountId>, IngressCounter, T::BlockNumber)> {

        let storage = StorageValueRef::persistent(VALIDATED_EVENT_LOCAL_STORAGE);

        let validated_events = storage.get::<Vec<EthEventId>>();
        let node_has_never_validated_events = match validated_events {
            Some(Some(_)) => false,
            _ => true
        };

        return Self::events_pending_challenge()
            .into_iter()
            .filter(|(checked, _counter, submitted_at_block)|
                Self::can_validate_this_event(checked, validator_account_id, validated_events.as_ref(), node_has_never_validated_events) &&
                AVN::<T>::is_block_finalised(*submitted_at_block)
            )
            .nth(0);
    }

    fn can_validate_this_event(
        checked: &EthEventCheckResult<T::BlockNumber, T::AccountId>,
        validator_account_id: &T::AccountId,
        validated_events: Option<&Option<Vec<EthEventId>>>,
        node_has_never_validated_events: bool) -> bool
    {
        if checked.checked_by == *validator_account_id { return false; }
        if node_has_never_validated_events { return true; }

        let node_has_not_validated_this_event = |event_id| {
            !validated_events
                .expect("Checked for error").as_ref()
                .expect("Checked for error").as_slice()
                .contains(event_id)
        };

        return node_has_not_validated_this_event(&checked.event.event_id);
    }

    fn get_next_event_to_process(block_number: T::BlockNumber)
        -> Option<(EthEventCheckResult<T::BlockNumber, T::AccountId>, IngressCounter, T::BlockNumber)>
    {
        return Self::events_pending_challenge()
            .into_iter()
            .filter(|(checked, _counter, submitted_at_block)|
                block_number > checked.ready_for_processing_after_block && AVN::<T>::is_block_finalised(*submitted_at_block))
            .last();
    }

    fn send_event(
        checked: EthEventCheckResult<T::BlockNumber, T::AccountId>,
        ingress_counter: IngressCounter,
        validator: &Validator<T::AuthorityId, T::AccountId>) -> Result<(), Error<T>>
    {
        let signature = validator.key
            .sign(&(PROCESS_EVENT_CONTEXT, &checked.event.event_id, ingress_counter).encode())
            .ok_or(Error::<T>::ErrorSigning)?;

        SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(
            Call::process_event(checked.event.event_id, ingress_counter, validator.clone(), signature).into()
        ).map_err(|_| Error::<T>::ErrorSubmittingTransaction)?;

        Ok(())
    }

    fn check_event_and_submit_result(
        block_number: T::BlockNumber,
        event_id: &EthEventId,
        ingress_counter: IngressCounter,
        validator: &Validator<T::AuthorityId, T::AccountId>) -> Result<(), Error<T>>
    {
        let result = Self::check_event(block_number, event_id, validator);
        if result.result == CheckResult::HttpErrorCheckingEvent {
            debug::native::info!("Http error checking event, skipping check");
            return Ok(());
        }

        if result.result == CheckResult::InsufficientConfirmations {
            debug::native::info!("Event is not old enough, skipping check");
            return Ok(());
        }

        let signature = validator.key
            .sign(&(SUBMIT_CHECKEVENT_RESULT_CONTEXT, &result, ingress_counter).encode())
            .ok_or(Error::<T>::ErrorSigning)?;
        SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(
            Call::submit_checkevent_result(result, ingress_counter, signature, validator.clone()).into()
        ).map_err(|_| Error::<T>::ErrorSubmittingTransaction)?;

        debug::native::info!("Check result submitted successfully");
        Ok(())
    }

    fn validate_event(
        block_number: T::BlockNumber,
        checked: EthEventCheckResult<T::BlockNumber, T::AccountId>,
        ingress_counter: IngressCounter,
        validator: &Validator<T::AuthorityId, T::AccountId>) -> Result<(), Error<T>>
    {
        let validated = Self::check_event(block_number, &checked.event.event_id, validator);
        if validated.result == CheckResult::HttpErrorCheckingEvent {
            debug::native::info!("Http error validating event, not challenging");
            return Ok(());
        }

        Self::save_validated_event_in_local_storage(checked.event.event_id.clone())?;

        // Note: Any errors after saving to local storage will mean the event will not be validated again
        let challenge = Self::get_challenge_if_required(checked, validated, validator.account_id.clone());
        if let Some(challenge) = challenge {
            let signature = validator.key
                .sign(&(CHALLENGE_EVENT_CONTEXT, &challenge, ingress_counter).encode())
                .ok_or(Error::<T>::ErrorSigning)?;
            // TODO [TYPE: business logic][PRI: medium][CRITICAL][JIRA: 349]: Allow for this event to be resubmitted if it fails here
            SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(
                Call::challenge_event(challenge, ingress_counter, signature, validator.clone()).into()
            ).map_err(|_| Error::<T>::ErrorSubmittingTransaction)?;

            debug::native::info!("Validation result submitted successfully");
        }

        Ok(())
    }

    fn get_challenge_if_required(
        checked: EthEventCheckResult<T::BlockNumber, T::AccountId>,
        validated: EthEventCheckResult<T::BlockNumber, T::AccountId>,
        validator_account_id: T::AccountId) -> Option<Challenge<T::AccountId>>
    {
        if checked.event.event_id != validated.event.event_id {
            debug::native::info!("Checked and validated have different event id's, not challenging");
            return None;
        }

        if (validated.result == checked.result && validated.event.event_data == checked.event.event_data) ||
           (validated.result == CheckResult::Invalid && checked.result == CheckResult::Invalid) {
            debug::native::info!("Validation matches original check, not challenging");
            return None;
        }

        let challenge_reason = match validated {
            EthEventCheckResult { result: CheckResult::Ok, .. } => {
                if checked.result == CheckResult::Ok {
                    ChallengeReason::IncorrectEventData
                } else {
                    ChallengeReason::IncorrectResult
                }
            },
            EthEventCheckResult { result: CheckResult::Invalid, .. } if checked.result == CheckResult::Ok =>
                ChallengeReason::IncorrectResult,
            _ => ChallengeReason::Unknown // We shouldn't get here but in case we do, set it to Unknown
        };

        if challenge_reason == ChallengeReason::Unknown {
            return None;
        }

        return Some(Challenge::new(checked.event.event_id, challenge_reason, validator_account_id));
    }

    fn save_validated_event_in_local_storage(event_id: EthEventId) -> Result<(), Error<T>> {
        let storage = StorageValueRef::persistent(VALIDATED_EVENT_LOCAL_STORAGE);
        let result = storage.mutate(|events: Option<Option<Vec<EthEventId>>>| {
            match events {
                Some(Some(mut events)) => {
                    events.push(event_id);
                    Ok(events)
                },
                None => Ok(vec![event_id]),
                _ => Err(()),
            }
        });

        if let Ok(Ok(_)) = result {
            return Ok(());
        }

        Err(Error::<T>::ErrorSavingValidationToLocalDB)
    }

    fn check_event(
        block_number: T::BlockNumber,
        event_id: &EthEventId,
        validator: &Validator<T::AuthorityId, T::AccountId>) -> EthEventCheckResult<T::BlockNumber, T::AccountId>
    {
        // Make an external HTTP request to fetch the event.
        // Note this call will block until response is received.
        let body = Self::fetch_event(event_id);

        // analyse the body to see if the event exists and is correctly formed
        return Self::compute_result(block_number, body, event_id, &validator.account_id);
    }

    // This function must not panic!!.
    // The outcome of the check must be reported back, even if the check fails
    fn compute_result(
        block_number: T::BlockNumber,
        response_body: Result<Vec<u8>, http::Error>,
        event_id: &EthEventId,
        validator_account_id: &T::AccountId) -> EthEventCheckResult<T::BlockNumber, T::AccountId>
    {
        let ready_after_block: T::BlockNumber = 0u32.into();
        let invalid_result = EthEventCheckResult::new(
            ready_after_block,
            CheckResult::Invalid,
            event_id,
            &EventData::EmptyEvent,
            validator_account_id.clone(),
            block_number,
            Default::default());

        // check if the body has been received successfully
        if let Err(e) = response_body {
            debug::native::error!("Http error fetching event: {:?}", e);
            return EthEventCheckResult::new(
                ready_after_block,
                CheckResult::HttpErrorCheckingEvent,
                event_id,
                &EventData::EmptyEvent,
                validator_account_id.clone(),
                block_number,
                Default::default());
        }

        let body = response_body.expect("Checked for error.");
        let response_body_str = &core::str::from_utf8(&body);
        if let Err(e) = response_body_str {
            debug::native::error!("❌ Invalid response from ethereum: {:?}", e);
            return invalid_result;
        }

        let response_json = json::parse_json(response_body_str.expect("Checked for error"));
        if let Err(e) = response_json {
            debug::native::error!("❌ Response from ethereum is not valid json: {:?}", e);
            return invalid_result;
        }
        let response = response_json.expect("Checked for error.");

        let status = get_status(&response);
        if let Err(e) = status {
            debug::native::error!("❌ Unable to extract transaction status from response: {:?}", e);
            return invalid_result;
        }

        let status = status.expect("Already checked");
        if status != 1 {
            debug::native::error!("❌ This was not executed successfully on Ethereum");
            return invalid_result;
        }

        let events = get_events(&response);
        if let Err(e) = events {
            debug::native::error!("❌ Unable to extract events from response: {:?}", e);
            return invalid_result;
        }

        let (event, contract_address) = find_event(&events.expect("Checked for error."), event_id.signature)
            .map_or_else(|| (&JsonValue::Null, H160::zero()), |(e, c)| (e, c));
        if event.is_null() || contract_address == H160::zero() {
            debug::native::error!("❌ Unable to find event");
            return invalid_result;
        }

        if Self::is_event_contract_valid(&contract_address, event_id) == false {
            debug::native::error!("❌ Event contract address {:?} is not recognised", contract_address);
            return invalid_result;
        }

        let data = get_data(event);
        if let Err(e) = data {
            debug::native::error!("❌ Unable to extract event data from response: {:?}", e);
            return invalid_result;
        }

        let topics = get_topics(event);
        if let Err(e) = topics {
            debug::native::error!("❌ Unable to extract topics from response: {:?}", e);
            return invalid_result;
        }

        let event_data = Self::parse_tier1_event(
            event_id.clone(),
            data.expect("Checked for error."),
            topics.expect("Checked for error."));

        if let Err(e) = event_data {
            debug::native::error!("❌ Unable to parse event data: {:?}", e);
            return invalid_result;
        }

        let num_confirmations = get_num_confirmations(&response);
        if let Err(e) = num_confirmations {
            debug::native::error!("❌ Unable to extract confirmations from response: {:?}", e);
            return invalid_result;
        }

        let num_confirmations = num_confirmations.expect("Checked already");
        if num_confirmations < <T as Config>::MinEthBlockConfirmation::get() {
            debug::native::error!("❌ There aren't enough confirmations for this event. Current confirmations: {:?}", num_confirmations);
            return EthEventCheckResult::new(
                ready_after_block,
                CheckResult::InsufficientConfirmations,
                event_id,
                &EventData::EmptyEvent,
                validator_account_id.clone(),
                block_number,
                Default::default());
        }

        return EthEventCheckResult::new(
            ready_after_block,
            CheckResult::Ok,
            event_id,
            &event_data.expect("Checked for error."),
            validator_account_id.clone(),
            block_number,
            Default::default());
    }

    fn fetch_event(event_id: &EthEventId) -> Result<Vec<u8>, http::Error> {
        let deadline = sp_io::offchain::timestamp().add(Duration::from_millis(2_000));
        let external_service_port_number = AVN::<T>::get_external_service_port_number();

        let mut url = String::from("http://127.0.0.1:");
        url.push_str(&external_service_port_number);
        url.push_str(&"/eth/events/0x".to_string());
        url.push_str(&hex::encode(&event_id.transaction_hash.as_bytes()));

        let request = http::Request::get(&url);
        let pending = request
            .deadline(deadline)
            .send()
            .map_err(|_| http::Error::IoError)?;

        let response = pending.try_wait(deadline).map_err(|_| http::Error::DeadlineReached)??;

        if response.code != 200 {
            debug::native::error!("Unexpected status code: {}", response.code);
            return Err(http::Error::Unknown);
        }

        Ok(response.body().collect::<Vec<u8>>())
    }

    fn event_exists_in_system(event_id: &EthEventId) -> bool {
        return <ProcessedEvents>::contains_key(&event_id) ||
                Self::unchecked_events().iter().any(|(event, _, _)| event == event_id) ||
                Self::events_pending_challenge().iter().any(|(event, _counter, _)| &event.event.event_id == event_id);
    }
    /// Adds an event: tx_hash must be a nonzero hash
    fn add_event(event_type: ValidEvents, tx_hash: H256, sender: T::AccountId) -> DispatchResult {
        let event_id = EthEventId {
            signature: event_type.signature(),
            transaction_hash: tx_hash,
        };

        ensure!(!Self::event_exists_in_system(&event_id), Error::<T>::DuplicateEvent);

        let ingress_counter = Self::get_next_ingress_counter();
        <UncheckedEvents<T>>::append((event_id.clone(), ingress_counter, <frame_system::Module<T>>::block_number()));

        if event_type.is_nft_event() {
            Self::deposit_event(Event::<T>::NftEthereumEventAdded(event_id, sender));
        } else {
            let eth_contract_address: H160 = Self::get_contract_address_for_non_nft_event(&event_type)
                .or_else(|| Some(H160::zero())).expect("Always return a default value");
            Self::deposit_event(Event::<T>::EthereumEventAdded(event_id, sender, eth_contract_address));
        }

        Ok(())
    }

    fn get_contract_address_for_non_nft_event(event_type: &ValidEvents) -> Option<H160> {
        match event_type {
            ValidEvents::AddedValidator => Some(Self::validator_manager_contract_address()),
            ValidEvents::Lifted => Some(Self::lifting_contract_address()),
            _ => None
        }
    }

    fn is_event_contract_valid(contract_address: &H160, event_id: &EthEventId) -> bool {
        let event_type = ValidEvents::try_from(&event_id.signature);
        if let Some(event_type) = event_type {
            if event_type.is_nft_event() {
                return <NftT1Contracts>::contains_key(contract_address);
            }

            let non_nft_contract_address = Self::get_contract_address_for_non_nft_event(&event_type);
            return non_nft_contract_address.is_some()
                && non_nft_contract_address.expect("checked for none") == *contract_address;
        }

        return false;
    }

    fn data_signature_is_valid<D: Encode>(
        data: &D,
        validator: &Validator<T::AuthorityId, T::AccountId>,
        signature: &<T::AuthorityId as RuntimeAppPublic>::Signature) -> bool
    {
        // verify that the incoming (unverified) pubkey is actually a validator
        if !Self::is_validator(&validator.account_id) {
            return false;
        }

        // check signature (this is expensive so we do it last).
        let signature_valid = data.using_encoded(|encoded_data| {
            validator.key.verify(&encoded_data, &signature)
        });

        return signature_valid;
    }

    fn is_validator(account_id: &T::AccountId) -> bool {
        return AVN::<T>::active_validators().into_iter().any(|v| v.account_id == *account_id);
    }

    fn verify_signature(
        proof: &Proof<T::Signature, T::AccountId>,
        signed_payload: &[u8]
    ) -> Result<(), Error<T>> {

        match proof.signature.verify(
            signed_payload,
            &proof.signer
        ) {
            true => Ok(()),
            false => Err(<Error<T>>::UnauthorizedTransaction.into()),
        }
    }

    fn encode_signed_add_ethereum_log_params(
        proof: &Proof<T::Signature, T::AccountId>,
        event_type: &ValidEvents,
        tx_hash: &H256,
        sender_nonce: u64) -> Vec<u8>
    {
        return (SIGNED_ADD_ETHEREUM_LOG_CONTEXT, proof.relayer.clone(), event_type, tx_hash, sender_nonce).encode();
    }

    fn get_encoded_call_param(call: &<T as Config>::Call) -> Option<(&Proof<T::Signature, T::AccountId>, Vec<u8>)> {
        let call = match call.is_sub_type() {
            Some(call) => call,
            None => return None,
        };

        match call {
            Call::signed_add_ethereum_log(proof, event_type, tx_hash) => {
                let sender_nonce = Self::proxy_nonce(&proof.signer);
                let encoded_data = Self::encode_signed_add_ethereum_log_params(proof, event_type, tx_hash, sender_nonce);
                return Some((proof, encoded_data));
            },

            _ => return None
        }
    }

    pub fn get_next_ingress_counter() -> IngressCounter {
        let ingress_counter = Self::ingress_counter() + 1; // default value in storage is 0, so first root_hash has counter 1
        TotalIngresses::put(ingress_counter);
        return ingress_counter;
    }
}

// Transactions sent by the validator nodes to report the result of checking an event is free
// Instead we will validate the signature before we allow it to get to the mempool
impl<T: Config> frame_support::unsigned::ValidateUnsigned for Module<T> {
    // https://substrate.dev/rustdocs/master/sp_runtime/traits/trait.ValidateUnsigned.html
    type Call = Call<T>;

    // TODO [TYPE: security][PRI: high][JIRA: 152][CRITICAL]: Are we open to transaction replay attacks, or signature re-use?
    fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
        if let Call::submit_checkevent_result(result, ingress_counter, signature, validator) = call {
            if !Self::unchecked_events().iter().any(
                |(event, counter, _)| event == &result.event.event_id && counter == ingress_counter) {
                return InvalidTransaction::Custom(ERROR_CODE_EVENT_NOT_IN_UNCHECKED).into();
            }

            if !result.event.event_data.is_valid() {
                return InvalidTransaction::Custom(ERROR_CODE_INVALID_EVENT_DATA).into();
            }

            if AVN::<T>::is_primary(result.checked_at_block, &result.checked_by)
                .map_err(|_| InvalidTransaction::Custom(ERROR_CODE_IS_PRIMARY_HAS_ERROR))? == false
            {
                return InvalidTransaction::Custom(ERROR_CODE_VALIDATOR_NOT_PRIMARY).into();
            }

            if validator.account_id != result.checked_by {
                return InvalidTransaction::BadProof.into();
            }

            if !Self::data_signature_is_valid(&(SUBMIT_CHECKEVENT_RESULT_CONTEXT, result, ingress_counter), &validator, signature) {
                return InvalidTransaction::BadProof.into();
            };

            ValidTransaction::with_tag_prefix("EthereumEvents")
                .priority(TransactionPriority::max_value())
                .and_provides(vec![("check",
                    result.event.event_id.hashed(<T as system::Config>::Hashing::hash)
                    ).encode()]
                )
                .longevity(64_u64)
                .propagate(true)
                .build()
        } else if let Call::process_event(event_id, ingress_counter, validator, signature) = call {
            if !Self::events_pending_challenge().iter().any(|(pending, counter, _)|
                &pending.event.event_id == event_id && counter == ingress_counter)
            {
                return InvalidTransaction::Custom(ERROR_CODE_EVENT_NOT_IN_PENDING_CHALLENGES).into();
            }

            // TODO [TYPE: security][PRI: high][CRITICAL][JIRA: 350]: Check if `validator` is a primary. Beware of using the current block_number() because it may not be the
            // same as what triggered the offchain worker.
            if !Self::data_signature_is_valid(&(PROCESS_EVENT_CONTEXT, &event_id, ingress_counter), validator, signature) {
                return InvalidTransaction::BadProof.into();
            };

            ValidTransaction::with_tag_prefix("EthereumEvents")
                .priority(TransactionPriority::max_value())
                .and_provides(vec![("process",
                    event_id.hashed(<T as system::Config>::Hashing::hash)
                    ).encode()]
                )
                .longevity(64_u64)
                .propagate(true)
                .build()
        } else if let Call::challenge_event(challenge, ingress_counter, signature, validator) = call {
            if !Self::events_pending_challenge().iter().any(|(pending, counter, _)|
                pending.event.event_id == challenge.event_id && ingress_counter == counter) {
                return InvalidTransaction::Custom(ERROR_CODE_EVENT_NOT_IN_PENDING_CHALLENGES).into();
            }

            // TODO [TYPE: business logic][PRI: medium][CRITICAL][JIRA: 351]: Make sure the challenge period has not passed
            // Note: the current block number can be different to the block_number the offchain worker was invoked in so
            // by the time the tx gets here the window may have passed.

            if validator.account_id != challenge.challenged_by {
                return InvalidTransaction::BadProof.into();
            }

            if !Self::data_signature_is_valid(&(CHALLENGE_EVENT_CONTEXT, challenge, ingress_counter), &validator, signature) {
                return InvalidTransaction::BadProof.into();
            };

            ValidTransaction::with_tag_prefix("EthereumEvents")
                .priority(TransactionPriority::max_value())
                .and_provides(vec![("challenge",
                    challenge.challenged_by.clone(),
                    challenge.event_id.hashed(<T as system::Config>::Hashing::hash)
                    ).encode()]
                )
                .longevity(64_u64)
                .propagate(true)
                .build()
        } else {
            return InvalidTransaction::Call.into();
        }
    }
}

impl<T: Config> ProcessedEventsChecker for Module<T> {
    fn check_event(event_id: &EthEventId) -> bool {
        return <ProcessedEvents>::contains_key(event_id);
    }
}

impl ProcessedEventsChecker for () {
    fn check_event(_event_id: &EthEventId) -> bool {
        return false;
    }
}

impl<T: Config> InnerCallValidator for Module<T> {
    type Call = <T as Config>::Call;

    fn signature_is_valid(call: &Box<Self::Call>) -> bool {
        if let Some((proof, signed_payload)) = Self::get_encoded_call_param(call) {
            return Self::verify_signature(&proof, &signed_payload.as_slice()).is_ok();
        }

        return false;
    }
}

// A value placed in storage that represents the current version of the EthereumEvents pallet storage. This value
// is used by the `on_runtime_upgrade` logic to determine whether we run its storage migration logic.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq)]
enum Releases {
    Unknown,
    V2_0_0,
    V3_0_0,
}

impl Default for Releases {
    fn default() -> Self {
        Releases::Unknown
    }
}

pub mod migrations {
    use super::*;
    use frame_support::{Blake2_128Concat, migration::StorageKeyIterator};
    pub type MarketplaceId = u32;

    pub fn migrate_to_multi_nft_contract<T: Config>() -> frame_support::weights::Weight {
        frame_support::debug::RuntimeLogger::init();
        frame_support::debug::info!("ℹ️  Ethereum events pallet data migration invoked");

        let mut consumed_weight = T::DbWeight::get().reads_writes(1, 1);

        for (_, address) in StorageKeyIterator::<MarketplaceId, H160, Blake2_128Concat>::
            new(b"EthereumEvents", b"NftContractAddresses").drain()
        {
            //Insert the address into the new storage item
            <NftT1Contracts>::insert(address, ());

            //update weight
            consumed_weight = consumed_weight.saturating_add(T::DbWeight::get().reads_writes(0, 1));
        }

        frame_support::debug::info!("ℹ️  Migrated Ethereum event's NFT contract addresses successfully");
        return consumed_weight;
    }
}