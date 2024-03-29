#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}};

use codec::{Encode, Decode};
use sp_std::{prelude::*};
use sp_runtime::{
    traits::{Member, AtLeast32Bit},
    DispatchError,
    transaction_validity::{
        TransactionValidity,
        ValidTransaction,
        InvalidTransaction,
        TransactionPriority,
        TransactionSource,
    },
};
use sp_avn_common::{
    safe_add_block_numbers,
    safe_sub_block_numbers,
    event_types::Validator,
    calculate_two_third_quorum,
    offchain_worker_storage_lock:: {self as OcwLock, OcwOperationExpiration},
    IngressCounter
};

use frame_support::{decl_event, decl_storage, decl_module, decl_error, traits::Get, debug, dispatch::DispatchResult, ensure,
    weights::Weight};
use frame_system::{self as system, offchain::{SendTransactionTypes, SubmitTransaction}, ensure_none, ensure_root};
use sp_core::{H256, ecdsa};
use core::convert::TryInto;
use sp_application_crypto::RuntimeAppPublic;
use pallet_avn::{self as avn, Error as avn_error,
    vote::{
        VotingSessionData,
        VotingSessionManager,
        process_approve_vote,
        process_reject_vote,
        end_voting_period_validate_unsigned,
        approve_vote_validate_unsigned,
        reject_vote_validate_unsigned,
    }
};
use avn::AccountToBytesConverter;
use pallet_ethereum_transactions::{
    CandidateTransactionSubmitter,
    ethereum_transaction::{PublishRootData, EthAbiHelper, EthTransactionType, TransactionId}
};
use pallet_session::historical::IdentificationTuple;
use sp_staking::offence::ReportOffence;

pub mod offence;
use crate::offence::{SummaryOffence, SummaryOffenceType, create_and_report_summary_offence};

const NAME: &'static [u8; 7] = b"summary";
const UPDATE_BLOCK_NUMBER_CONTEXT: &'static [u8] = b"update_last_processed_block_number";
const ADVANCE_SLOT_CONTEXT: &'static [u8] = b"advance_slot";

// Error codes returned by validate unsigned methods
const ERROR_CODE_VALIDATOR_IS_NOT_PRIMARY: u8 = 10;
const ERROR_CODE_INVALID_ROOT_DATA: u8 = 20;
const ERROR_CODE_INVALID_ROOT_RANGE: u8 = 30;

// This value is used only when generating a signature for an empty root.
// Empty roots shouldn't be submitted to ethereum-transactions so we can use any value we want.
const EMPTY_ROOT_TRANSACTION_ID: TransactionId = 0;

// used in benchmarks and weights calculation only
const MAX_VALIDATOR_ACCOUNT_IDS: u32 = 10;
const MAX_OFFENDERS: u32 = 2; // maximum of offenders need to be less one third of minimum validators so the benchmark won't panic
const MAX_NUMBER_OF_ROOT_DATA_PER_RANGE: u32 = 2;

const MIN_SCHEDULE_PERIOD: u32 = 120; // 6 MINUTES
const MAX_SCHEDULE_PERIOD: u32 = 28800; // 1 DAY
const MIN_VOTING_PERIOD: u32 = 100; // 5 MINUTES
const MAX_VOTING_PERIOD: u32 = 28800; // 1 DAY
const DEFAULT_VOTING_PERIOD: u32 = 600; // 30 MINUTES

pub mod vote;
use crate::vote::*;

pub mod challenge;
use crate::challenge::*;

mod benchmarking;

// TODO: [TYPE: business logic][PRI: high][CRITICAL]
// Rerun benchmark in production and update both ./default_weights.rs file and /bin/node/runtime/src/weights/pallet_ethereum_transactions.rs file.
pub mod default_weights;
pub use default_weights::WeightInfo;

pub trait Config: SendTransactionTypes<Call<Self>> + system::Config + avn::Config + pallet_session::historical::Config {
    type Event: From<Event<Self>> + Into<<Self as system::Config>::Event>;

    /// A period (in block number) to detect when a validator failed to advance the current slot number
    type AdvanceSlotGracePeriod: Get<Self::BlockNumber>;

    /// Minimum age of block (in block number) to include in a tree.
    /// This will give grandpa a chance to finalise the blocks
    type MinBlockAge: Get<Self::BlockNumber>;

    type CandidateTransactionSubmitter: CandidateTransactionSubmitter<Self::AccountId>;

    type AccountToBytesConvert: pallet_avn::AccountToBytesConverter<Self::AccountId>;

    ///  A type that gives the pallet the ability to report offences
    type ReportSummaryOffence: ReportOffence<
        Self::AccountId,
        IdentificationTuple<Self>,
        SummaryOffence<IdentificationTuple<Self>>,
    >;

    /// The delay after which point things become suspicious. Default is 100.
    type FinalityReportLatency: Get<Self::BlockNumber>;

    /// Weight information for the extrinsics in this pallet.
    type WeightInfo: WeightInfo;
}

pub type AVN<T> = avn::Module::<T>;

decl_event!(
    pub enum Event<T> where
        <T as system::Config>::BlockNumber,
        <T as system::Config>::AccountId,
        IdentificationTuple = IdentificationTuple<T>,
        SummaryOffenceType = SummaryOffenceType,
        RootId = RootId<<T as system::Config>::BlockNumber>,
    {
        /// Schedule period and voting period are updated
        SchedulePeriodAndVotingPeriodUpdated(/*schedule period*/ BlockNumber, /*voting period*/ BlockNumber),
        /// Root hash of summary between from block number and to block number is calculated by a validator
        SummaryCalculated(/*from*/ BlockNumber, /*to*/ BlockNumber, /*summary:*/ H256, /*Validator*/ AccountId),
        /// Vote by a voter for a root id is added
        VoteAdded(/*Voter*/ AccountId, RootId, /*true = approve*/ bool),
        /// Voting for the root id is finished, true means the root is approved
        VotingEnded(RootId, /*true = root is approved*/ bool),
        /// A summary offence by a list of offenders is reported
        SummaryOffenceReported(SummaryOffenceType,/*offenders*/ Vec<IdentificationTuple>),
        /// A new slot between a range of blocks for a validator is advanced by an account
        SlotAdvanced(/*who advanced*/ AccountId, /*new slot*/ BlockNumber, /*slot validator*/ AccountId, /*slot end*/ BlockNumber),
        /// A summary created by a challengee is challenged by a challenger for a reason
        ChallengeAdded(SummaryChallengeReason, /*challenger*/ AccountId, /*challengee*/AccountId),
        /// An offence about a summary not be published by a challengee is reported
        SummaryNotPublishedOffence(
            /*challengee*/AccountId,
            /*slot number where no summary was published (aka void slot)*/ BlockNumber,
            /*slot where a block was last published*/ BlockNumber,
            /*block number for end of the void slot*/ BlockNumber,
            ),
    }
);

decl_error! {
    pub enum Error for Module<T: Config> {
        Overflow,
        ErrorCalculatingChosenValidator,
        ErrorConvertingBlockNumber,
        ErrorGettingSummaryDataFromService,
        InvalidSummaryRange,
        ErrorSubmittingTransaction,
        InvalidKey,
        ErrorSigning,
        InvalidHexString,
        InvalidUTF8Bytes,
        InvalidRootHashLength,
        SummaryPendingOrApproved,
        RootHasAlreadyBeenRegisteredForVoting,
        InvalidRoot,
        DuplicateVote,
        ErrorEndingVotingPeriod,
        ErrorSubmitCandidateTxnToTier1,
        VotingSessionIsNotValid,
        ErrorRecoveringPublicKeyFromSignature,
        ECDSASignatureNotValid,
        RootDataNotFound,
        InvalidChallenge,
        WrongValidator,
        GracePeriodElapsed,
        TooEarlyToAdvance,
        InvalidIngressCounter,
        SchedulePeriodIsTooShort,
        SchedulePeriodIsTooLong,
        VotingPeriodIsTooShort,
        VotingPeriodIsTooLong,
        VotingPeriodIsLessThanFinalityReportLatency,
        VotingPeriodIsEqualOrLongerThanSchedulePeriod,
    }
}

// Note for SYS-152 (see notes in fn end_voting)):
// A new instance of root_range should only be accepted into the system (record_summary_calculation) if:
// - there is no previous instance of that root_range in roots
// - if there is any such an instance, it does not exist in PendingApprovals and it is not validated
// It does not help to remove the root_range from Roots. If that were the case, we would lose the information the root
// has already been processed and so cannot be submitted (ie voted on) again.

decl_storage! {
    trait Store for Module<T: Config> as Summary {
        pub NextBlockToProcess get(fn get_next_block_to_process): T::BlockNumber;
        pub NextSlotAtBlock get(fn block_number_for_next_slot): T::BlockNumber;
        pub CurrentSlot get(fn current_slot): T::BlockNumber;
        pub CurrentSlotsValidator get(fn slot_validator): T::AccountId;
        pub SlotOfLastPublishedSummary get(fn last_summary_slot): T::BlockNumber;

        pub Roots: double_map hasher(blake2_128_concat) RootRange<T::BlockNumber>, hasher(blake2_128_concat) IngressCounter => RootData<T::AccountId>;
        pub VotesRepository get(fn get_vote): map hasher(blake2_128_concat) RootId<T::BlockNumber> => VotingSessionData<T::AccountId, T::BlockNumber>;
        pub PendingApproval get(fn get_pending_roots): map hasher(blake2_128_concat) RootRange<T::BlockNumber> => IngressCounter;

        /// The total ingresses of roots
        pub TotalIngresses get(fn get_ingress_counter): IngressCounter;

        /// A period (in block number) where summaries are calculated
        pub SchedulePeriod get(fn schedule_period) config(): T::BlockNumber;
        /// A period (in block number) where validators are allowed to vote on the validity of a root hash
        pub VotingPeriod get(fn voting_period) config(): T::BlockNumber;
    }
    add_extra_genesis {
        build(|config| {
            let mut schedule_period_in_blocks = config.schedule_period;
            if schedule_period_in_blocks == 0u32.into() {
                schedule_period_in_blocks = MIN_SCHEDULE_PERIOD.into();
            }
            assert!(Module::<T>::validate_schedule_period(schedule_period_in_blocks).is_ok(), "Schedule Period must be a valid value");
            <NextSlotAtBlock<T>>::put(schedule_period_in_blocks);
            <SchedulePeriod<T>>::put(schedule_period_in_blocks);

            let mut voting_period_in_blocks = config.voting_period;
            if voting_period_in_blocks == 0u32.into() {
                voting_period_in_blocks = MIN_VOTING_PERIOD.into();
            }
            assert!(Module::<T>::validate_voting_period(voting_period_in_blocks, schedule_period_in_blocks).is_ok(), "Voting Period must be a valid value");
            <VotingPeriod<T>>::put(voting_period_in_blocks);

            let maybe_first_validator = AVN::<T>::validators().into_iter().map(|v| v.account_id).nth(0);
            assert!(maybe_first_validator.is_some(), "You must add validators to run the AvN");

            <CurrentSlotsValidator<T>>::put(maybe_first_validator.expect("Validator is checked for none"));
		});
	}
}

decl_module! {
    pub struct Module<T: Config> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;

        /// # <weight>
        ///  DbWrites: `SchedulePeriod`, `VotingPeriod`: O(1)
        ///  Emit events: `SchedulePeriodAndVotingPeriodUpdated`: O(1)
        /// Total Complexity: O(1)
        /// # </weight>
        #[weight = <T as Config>::WeightInfo::set_periods()]
        pub fn set_periods(origin, schedule_period_in_blocks: T::BlockNumber, voting_period_in_blocks: T::BlockNumber) -> DispatchResult {
            ensure_root(origin)?;
            Self::validate_schedule_period(schedule_period_in_blocks)?;
            Self::validate_voting_period(voting_period_in_blocks, schedule_period_in_blocks)?;

            <SchedulePeriod<T>>::put(schedule_period_in_blocks);
            <VotingPeriod<T>>::put(voting_period_in_blocks);

            Self::deposit_event(Event::<T>::SchedulePeriodAndVotingPeriodUpdated(schedule_period_in_blocks, voting_period_in_blocks));
            Ok(())
        }

        /// # <weight>
        /// Keys: V - Number of validators accounts
        ///       R - Number of roots for a root range
        ///  DbReads: `TotalIngresses`, `VotesRepository`, 3 * `NextBlockToProcess`, `PendingApproval`: O(1)
        ///  DbWrites: `TotalIngresses`,`Roots`, `PendingApproval`, `VotesRepository`: O(1)
        ///  Check summary is not approved by searching Roots double map by its primary key: O(R)
        ///  avn pallet operations:
        ///     - DbReads: `Validators`: O(1)
        ///     - is_validator operation: O(V)
        ///  ethereum_transactions pallet operations:
        ///     - DbReads: `ReservedTransactions`, `Nonce`: O(1)
        ///     - DbWrites: `ReservedTransactions`, `Nonce`: O(1)
        ///  Emit events: `SummaryCalculated`: O(1)
        /// Total Complexity: O(1 + V + R)
        /// # </weight>
        #[weight = <T as Config>::WeightInfo::record_summary_calculation(
            MAX_VALIDATOR_ACCOUNT_IDS,
            MAX_NUMBER_OF_ROOT_DATA_PER_RANGE
        )]
        fn record_summary_calculation(
            origin,
            new_block_number: T::BlockNumber,
            root_hash: H256,
            ingress_counter: IngressCounter,
            validator: Validator<<T as avn::Config>::AuthorityId, T::AccountId>,
            _signature: <<T as avn::Config>::AuthorityId as RuntimeAppPublic>::Signature) -> DispatchResult
        {
            ensure_none(origin)?;
            ensure!(Self::get_ingress_counter() + 1 == ingress_counter, Error::<T>::InvalidIngressCounter);
            ensure!(AVN::<T>::is_validator(&validator.account_id), Error::<T>::InvalidKey);

            let root_range = RootRange::new(Self::get_next_block_to_process(), new_block_number);
            let root_id = RootId::new(root_range, ingress_counter);
            let expected_target_block = Self::get_target_block()?;
            let current_block_number = <system::Module<T>>::block_number();

            ensure!(Self::summary_is_neither_pending_nor_approved(&root_id.range), Error::<T>::SummaryPendingOrApproved);
            ensure!(!<VotesRepository<T>>::contains_key(root_id), Error::<T>::RootHasAlreadyBeenRegisteredForVoting);
            ensure!(new_block_number == expected_target_block, Error::<T>::InvalidSummaryRange);

            let quorum = calculate_two_third_quorum(AVN::<T>::validators().len() as u32);
            let voting_period_end = safe_add_block_numbers(current_block_number, Self::voting_period())
                .map_err(|_| Error::<T>::Overflow)?;

            let tx_id = if root_hash != Self::empty_root() {
                let publish_root = EthTransactionType::PublishRoot(PublishRootData::new(*root_hash.as_fixed_bytes()));
                Some(T::CandidateTransactionSubmitter::reserve_transaction_id(&publish_root)?)
            } else {
                None
            };

            TotalIngresses::put(ingress_counter);
            <Roots<T>>::insert(&root_id.range, ingress_counter, RootData::new(root_hash, validator.account_id.clone(), tx_id));
            <PendingApproval<T>>::insert(root_id.range, ingress_counter);
            <VotesRepository<T>>::insert(root_id, VotingSessionData::new(root_id.encode(), quorum, voting_period_end, current_block_number));

            Self::deposit_event(Event::<T>::SummaryCalculated(root_id.range.from_block, root_id.range.to_block, root_hash, validator.account_id));
            Ok(())
        }

        /// # <weight>
        /// Keys: V - Number of validators accounts
        ///  DbReads:  `Roots`: O(1)
        ///  DbWrites: `Roots`: O(1)
        ///  Convert data to eth compatible encoding: O(1)
        ///  Eth signature is valid operation: O(V)
        ///    - If eth signature is invalid: Create and report validators offence: O(1)
        ///  Get voting session: O(1)
        ///  Process approve vote: O(V)
        ///  Emit events: `VoteAdded`: O(1)
        /// Total Complexity: O(1 + V)
        /// # </weight>
        #[weight = <T as Config>::WeightInfo::approve_root_with_end_voting(MAX_VALIDATOR_ACCOUNT_IDS, MAX_OFFENDERS).max(
            <T as Config>::WeightInfo::approve_root_without_end_voting(MAX_VALIDATOR_ACCOUNT_IDS)
        )]
        fn approve_root(
            origin,
            root_id: RootId::<T::BlockNumber>,
            validator: Validator<<T as avn::Config>::AuthorityId, T::AccountId>,
            approval_signature: ecdsa::Signature,
            _signature: <T::AuthorityId as RuntimeAppPublic>::Signature) -> DispatchResult
        {
            ensure_none(origin)?;

            let root_data = Self::try_get_root_data(&root_id)?;
            let eth_encoded_data = Self::convert_data_to_eth_compatible_encoding(&root_data)?;
            if !AVN::<T>::eth_signature_is_valid(eth_encoded_data, &validator, &approval_signature) {
                create_and_report_summary_offence::<T>(
                    &validator.account_id,
                    &vec![validator.account_id.clone()],
                    SummaryOffenceType::InvalidSignatureSubmitted
                );
                return Err(avn_error::<T>::InvalidECDSASignature)?;
            };

            let voting_session = Self::get_root_voting_session(&root_id);

            process_approve_vote::<T>(&voting_session, validator.account_id.clone(), approval_signature)?;

            Self::deposit_event(RawEvent::VoteAdded(validator.account_id, root_id, true));

            // TODO [TYPE: weightInfo][PRI: medium]: Return accurate weight
            Ok(())
        }

        /// # <weight>
        /// Keys: V - Number of validators accounts
        ///       O - Number of offenders
        ///  DbReads:  `VotesRepository`, 2 * `Roots`, 2 * `PendingApproval`: O(1)
        ///  DbWrites: `VotesRepository`: O(1)
        ///  Emit Events: `VoteAdded`: O(1)
        ///  avn pallet operations:
        ///     DbReads: `Validators`: O(1)
        ///     Iterate validators items: O(V)
        ///  If end voting:
        ///     DbWrites: `PendingApproval`: O(1)
        ///     Emit Events: `VotingEnded`: O(1)
        ///     with approval:
        ///         DbReads: 9 * `VotesRepository`, 6 * `Roots`, 4 * `PendingApproval`: O(1)
        ///         DbWrites: `NextBlockToProcess`, `Roots`, `SlotOfLastPublishedSummary`: O(1)
        /// 	    create and report summary offence RejectedValidRoot operation: O(O)
        ///     with reject:
        ///         create and report summary offence CreatedInvalidRoot operation: O(O)
	    ///         create and report summary offence ApprovedInvalidRoot operation: O(O)
        /// Total Complexity: O(1 + V + O)
        /// # </weight>
        #[weight = <T as Config>::WeightInfo::reject_root_with_end_voting(MAX_VALIDATOR_ACCOUNT_IDS, MAX_OFFENDERS).max(
            <T as Config>::WeightInfo::reject_root_without_end_voting(MAX_VALIDATOR_ACCOUNT_IDS)
        )]
        fn reject_root(
            origin,
            root_id: RootId::<T::BlockNumber>,
            validator: Validator<<T as avn::Config>::AuthorityId, T::AccountId>,
            _signature: <T::AuthorityId as RuntimeAppPublic>::Signature) -> DispatchResult
        {
            ensure_none(origin)?;
            let voting_session = Self::get_root_voting_session(&root_id);
            process_reject_vote::<T>(&voting_session, validator.account_id.clone())?;

            Self::deposit_event(RawEvent::VoteAdded(validator.account_id, root_id, false));

            // TODO [TYPE: weightInfo][PRI: medium]: Return accurate weight
            Ok(())
        }

        /// # <weight>
        ///   DbReads: 2 * `PendingApproval`, 4 * `Roots`, 2 * `VotesRepository`: O(1)
        ///   DbWrites: `PendingApproval`: O(1)
        ///   Emit Event: `VotingEnded`: O(1)
        ///   if vote is approved:
        ///     DbReads: 2 * `VotesRepository`: O(1)
        ///     DbWrites: `NextBlockToProcess`, `Roots`, `SlotOfLastPublishedSummary`: O(1)
        ///     create and report summary offence: RejectedValidRoot: O(1)
        ///   else if vote is rejected:
        ///     create and report summary offence: CreatedInvalidRoot: O(1)
        ///     create and report summary offence: ApprovedInvalidRoot: O(1)
        /// Total Complexity: O(1)
        /// # </weight>
        #[weight = <T as Config>::WeightInfo::end_voting_period_with_rejected_valid_votes(MAX_OFFENDERS).max(
            <T as Config>::WeightInfo::end_voting_period_with_approved_invalid_votes(MAX_OFFENDERS)
        )]
        fn end_voting_period(
            origin,
            root_id: RootId::<T::BlockNumber>,
            validator: Validator<<T as avn::Config>::AuthorityId, T::AccountId>,
            _signature: <T::AuthorityId as RuntimeAppPublic>::Signature,
        ) -> DispatchResult {
            ensure_none(origin)?;
            //Event is deposited in end_voting because this function can get called from `approve_root` or `reject_root`
            Self::end_voting(validator.account_id, &root_id)?;

            // TODO [TYPE: weightInfo][PRI: medium]: Return accurate weight
            Ok(())
        }

        /// # <weight>
        ///  DbReads:  2 * `CurrentSlot`, 3 * `NextSlotAtBlock`,
        ///            2 * `CurrentSlotsValidator`, `SlotOfLastPublishedSummary`: O(1)
        ///  DbWrites: `CurrentSlot`, `CurrentSlotsValidator`, `NextSlotAtBlock`: O(1)
        ///  avn pallet calculate primary validator operations:
        ///     - DbReads: Validators: O(1)
        ///  Emit event: `SlotAdvanced`: O(1)
        ///  If no summary created in slot:
        ///     Create and report summary offence operation: O(1)
        ///     Emit event: `SummaryNotPublishedOffence`: O(1)
        /// Total Complexity: O(1)
        /// # </weight>
        #[weight = <T as Config>::WeightInfo::advance_slot_with_offence().max(
            <T as Config>::WeightInfo::advance_slot_without_offence()
        )]
        fn advance_slot(
            origin,
            validator: Validator<<T as avn::Config>::AuthorityId, T::AccountId>,
            _signature: <T::AuthorityId as RuntimeAppPublic>::Signature) -> DispatchResult
        {
            ensure_none(origin)?;

            Self::update_slot_number(validator)?;

            // TODO [TYPE: weightInfo][PRI: medium]: Return accurate weight
            Ok(())
        }

        /// # <weight>
        ///  DbReads: `NextBlockToProcess`, `NextSlotAtBlock`, `CurrentSlot`, `CurrentSlotsValidator`,
        ///           `SlotOfLastPublishedSummary`: O(1)
        ///  DbWrites:  `CurrentSlot`, `CurrentSlotsValidator`, `NextSlotAtBlock`: O(1)
        ///  Create and report summary offence operation: O(1)
        ///  Advance slot operation: O(1)
        ///  Emit event: `SummaryOffenceReported:SlotNotAdvanced`,`SummaryOffenceReported:NoSummaryCreated`,
        ///              `SummaryNotPublishedOffence`, `SlotAdvanced`, `ChallengeAdded`, : O(1)
        /// Total Complexity: O(1)
        /// # </weight>
        #[weight = <T as Config>::WeightInfo::add_challenge()]
        fn add_challenge(
            origin,
            challenge: SummaryChallenge<T::AccountId>,
            validator: Validator<T::AuthorityId, T::AccountId>,
            _signature: <T::AuthorityId as RuntimeAppPublic>::Signature,) -> DispatchResult
        {
            ensure_none(origin)?;
            ensure!(
                challenge.is_valid::<T>(Self::current_slot(), <frame_system::Module<T>>::block_number(), &challenge.challengee),
                Error::<T>::InvalidChallenge
            );
            // QUESTION: offence: do we slash the author of an invalid challenge?
            // I think it is probably too harsh. It may not be valid for timing reasons:
            // it arrived too early, or the slot has already moved and the validator changed

            let offender = challenge.challengee.clone();
            let challenge_type = match challenge.challenge_reason {
                SummaryChallengeReason::SlotNotAdvanced(_) => Some(SummaryOffenceType::SlotNotAdvanced),
                SummaryChallengeReason::Unknown => None,
            };

            // if this fails, it is a bug. All challenge types should have a corresponding offence type
            // except for Unknown which we should never produce
            ensure!(!challenge_type.is_none(), Error::<T>::InvalidChallenge);

            create_and_report_summary_offence::<T>(
                &validator.account_id,
                &vec![offender],
                challenge_type.expect("Already checked"));

            Self::update_slot_number(validator)?;

            Self::deposit_event(Event::<T>::ChallengeAdded(
                challenge.challenge_reason,
                challenge.challenger,
                challenge.challengee,
            ));

            Ok(())
        }

        fn offchain_worker(block_number: T::BlockNumber) {
            let setup_result = AVN::<T>::pre_run_setup(block_number, NAME.to_vec());
            if let Err(e) = setup_result {
                 match e {
                    _ if e == DispatchError::from(avn_error::<T>::OffchainWorkerAlreadyRun) => {();},
                    _ => {
                            debug::native::error!("💔️ Unable to run offchain worker: {:?}", e);
                        }
                };

                return ;
            }
            let this_validator = setup_result.expect("We have a validator");

            Self::advance_slot_if_required(block_number, &this_validator);
            Self::process_summary_if_required(block_number, &this_validator);
            cast_votes_if_required::<T>(block_number, &this_validator);
            end_voting_if_required::<T>(block_number, &this_validator);
            challenge_slot_if_required::<T>(block_number, &this_validator);
        }

        // Note: this "special" function will run during every runtime upgrade. Any complicated migration logic should be done in a
        // separate function so it can be tested properly.
        fn on_runtime_upgrade() -> Weight {
            let mut weight_write_counter = 0;
            frame_support::debug::RuntimeLogger::init();
            frame_support::debug::info!("ℹ️  Summary pallet data migration invoked");

            if Self::schedule_period() == 0u32.into() {
                frame_support::debug::info!("ℹ️  Updating SchedulePeriod to a default value of {} blocks", MAX_SCHEDULE_PERIOD);
                weight_write_counter += 1;
                <SchedulePeriod<T>>::put(<T as frame_system::Config>::BlockNumber::from(MAX_SCHEDULE_PERIOD));
            }

            if Self::voting_period() == 0u32.into() {
                frame_support::debug::info!("ℹ️  Updating VotingPeriod to a default value of {} blocks", DEFAULT_VOTING_PERIOD);
                weight_write_counter += 1;
                <VotingPeriod<T>>::put(<T as frame_system::Config>::BlockNumber::from(DEFAULT_VOTING_PERIOD));
            }

            return T::DbWeight::get().writes(weight_write_counter as Weight);
        }
    }
}

impl<T: Config> Module<T> {
    fn validate_schedule_period(schedule_period_in_blocks: T::BlockNumber) -> DispatchResult {
        ensure!(schedule_period_in_blocks >= MIN_SCHEDULE_PERIOD.into(), Error::<T>::SchedulePeriodIsTooShort);
        ensure!(schedule_period_in_blocks <= MAX_SCHEDULE_PERIOD.into(), Error::<T>::SchedulePeriodIsTooLong);
        Ok(())
    }

    fn validate_voting_period(voting_period_in_blocks: T::BlockNumber, schedule_period_in_blocks: T::BlockNumber) -> DispatchResult {
        ensure!(voting_period_in_blocks >= T::FinalityReportLatency::get(), Error::<T>::VotingPeriodIsLessThanFinalityReportLatency);
        ensure!(voting_period_in_blocks >= MIN_VOTING_PERIOD.into(), Error::<T>::VotingPeriodIsTooShort);
        ensure!(voting_period_in_blocks < schedule_period_in_blocks, Error::<T>::VotingPeriodIsEqualOrLongerThanSchedulePeriod);
        ensure!(voting_period_in_blocks <= MAX_VOTING_PERIOD.into(), Error::<T>::VotingPeriodIsTooLong);
        Ok(())
    }

    pub fn grace_period_elapsed(block_number: T::BlockNumber) -> bool {
        let diff = safe_sub_block_numbers::<T::BlockNumber>(block_number, Self::block_number_for_next_slot())
            .unwrap_or(0u32.into());
        return diff > T::AdvanceSlotGracePeriod::get();
    }

    // Check if this validator is allowed
    // the slot's validator is challenged if it does not advance the slot inside the challenge window
    // But this challenge will be checked later than when it was submitted, so it is possible storage has changed by then
    // To prevent the validator escape the challenge, we can allow it this change only inside the challenge window
    // Other validators can however move the slot after the challenge window
    pub fn validator_can_advance_slot(
        validator: &Validator<<T as avn::Config>::AuthorityId, T::AccountId>)
        -> DispatchResult
    {
        let current_block_number = <frame_system::Module<T>>::block_number();
        ensure!(current_block_number >= Self::block_number_for_next_slot(), Error::<T>::TooEarlyToAdvance);

        if Self::grace_period_elapsed(current_block_number) {
            if validator.account_id == Self::slot_validator() {
                return Err(Error::<T>::GracePeriodElapsed)?;
            }
        } else {
            if validator.account_id != Self::slot_validator() {
                return Err(Error::<T>::WrongValidator)?;
            }
        }

        Ok(())
    }

    pub fn update_slot_number(validator: Validator<<T as avn::Config>::AuthorityId, T::AccountId>) -> DispatchResult {
        Self::validator_can_advance_slot(&validator)?;
        // QUESTION: should we slash a validator who tries to advance the slot when it is not their turn?
        // this code is always called inside an unsigned transaction, so in consensus. We can raise offences here.
        Self::register_offence_if_no_summary_created_in_slot(&validator);

        let new_slot_number = safe_add_block_numbers::<T::BlockNumber>(
            Self::current_slot(),
            1u32.into())
        .map_err(|_| Error::<T>::Overflow)?;

        let new_validator_account_id = AVN::<T>::calculate_primary_validator(new_slot_number)?;

        let next_slot_start_block = safe_add_block_numbers::<T::BlockNumber>(
            Self::block_number_for_next_slot(),
            Self::schedule_period())
        .map_err(|_| Error::<T>::Overflow)?;

        <CurrentSlot<T>>::put(new_slot_number);
        <CurrentSlotsValidator<T>>::put(new_validator_account_id.clone());
        <NextSlotAtBlock<T>>::put(next_slot_start_block);

        Self::deposit_event(Event::<T>::SlotAdvanced(
            validator.account_id,
            new_slot_number,
            new_validator_account_id,
            next_slot_start_block));

        Ok(())
    }

    pub fn get_root_voting_session(root_id: &RootId<T::BlockNumber>)
        -> Box<dyn VotingSessionManager<T::AccountId, T::BlockNumber>>
    {
        return Box::new(RootVotingSession::<T>::new(root_id))
            as Box<dyn VotingSessionManager<T::AccountId, T::BlockNumber>>;
    }

    // This can be called by other validators to verify the root hash
    pub fn compute_root_hash(from_block: T::BlockNumber, to_block: T::BlockNumber) -> Result<H256, DispatchError>
    {
        let from_block_number: u32 = TryInto::<u32>::try_into(from_block).map_err(|_| Error::<T>::ErrorConvertingBlockNumber)?;
        let to_block_number: u32 = TryInto::<u32>::try_into(to_block).map_err(|_| Error::<T>::ErrorConvertingBlockNumber)?;

        let mut url_path = "roothash/".to_string();
        url_path.push_str(&from_block_number.to_string());
        url_path.push_str(&"/".to_string());
        url_path.push_str(&to_block_number.to_string());

        let response = AVN::<T>::get_data_from_service(url_path);

        if let Err(e) = response {
            debug::native::error!("💔️ Error getting summary data from external service: {:?}", e);
            return Err(Error::<T>::ErrorGettingSummaryDataFromService)?
        }

        let root_hash = Self::validate_response(response.expect("checked for error"))?;
        debug::native::trace!(target: "avn", "🥽 Calculated root hash {:?} for range [{:?}, {:?}]", &root_hash, &from_block_number, &to_block_number);

        return Ok(root_hash);
    }

    fn create_root_lock_name(block_number: T::BlockNumber) -> OcwLock::PersistentId{
        let mut name = b"create_summary::".to_vec();
        name.extend_from_slice(&mut block_number.encode());
        name
    }

    pub fn convert_data_to_eth_compatible_encoding(root_data: &RootData<T::AccountId>) -> Result<String, DispatchError> {
        let eth_description = EthAbiHelper::generate_ethereum_description_for_signature_request(
            &T::AccountToBytesConvert::into_bytes(&root_data.added_by),
            &EthTransactionType::PublishRoot(PublishRootData::new(*root_data.root_hash.as_fixed_bytes())),
            match root_data.tx_id {
                None => EMPTY_ROOT_TRANSACTION_ID,
                _ => *root_data.tx_id.as_ref().expect("Non-Empty roots have a reserved TransactionId"),
            },
        )
        .map_err(|_| Error::<T>::InvalidRoot)?;

        Ok(hex::encode(EthAbiHelper::generate_eth_abi_encoding_for_params_only(&eth_description)))
    }

    pub fn sign_root_for_ethereum(root_id: &RootId<T::BlockNumber>) -> Result<(String, ecdsa::Signature), DispatchError>
    {
        let root_data = Self::try_get_root_data(&root_id)?;
        let data = Self::convert_data_to_eth_compatible_encoding(&root_data)?;
        return Ok((data.clone(), AVN::<T>::request_ecdsa_signature_from_external_service(&data)?));
    }

    // TODO [Low Priority] Review if the lock period should be configurable
    fn lock_till_request_expires() -> OcwOperationExpiration {
        let avn_service_expiry_in_millisec = 300_000 as u32;
        let avn_block_generation_in_millisec = 3_000 as u32;
        let delay = 10 as u32;
        let lock_expiration_in_blocks = avn_service_expiry_in_millisec / avn_block_generation_in_millisec + delay;
        return OcwOperationExpiration::Custom(lock_expiration_in_blocks);
    }

    fn advance_slot_if_required(
        block_number: T::BlockNumber,
        this_validator: &Validator<<T as avn::Config>::AuthorityId, T::AccountId>)
    {
        if this_validator.account_id == Self::slot_validator() &&
            block_number >= Self::block_number_for_next_slot() {

            let result = Self::dispatch_advance_slot(this_validator);

            if let Err(e) = result {
                debug::native::warn!("💔️ Error starting a new summary creation slot: {:?}", e);
            }
        }
    }

    // called from OCW - no storage changes allowed here
    fn process_summary_if_required(
        block_number: T::BlockNumber,
        this_validator: &Validator<<T as avn::Config>::AuthorityId, T::AccountId>)
    {
        let target_block = Self::get_target_block();
        if target_block.is_err() {
            debug::native::error!("💔️ Error getting target block.");
            return;
        }
        let last_block_in_range = target_block.expect("Valid block number");

        let root_lock_name = Self::create_root_lock_name(last_block_in_range);
        let expiration = Self::lock_till_request_expires();

        if Self::can_process_summary(block_number, last_block_in_range, this_validator) &&
            OcwLock::set_lock_with_expiry(
                block_number,
                expiration,
                root_lock_name.clone()).is_ok()
        {
            debug::native::warn!("ℹ️  Processing summary for range {:?} - {:?}. Slot {:?}",
                Self::get_next_block_to_process(),
                last_block_in_range,
                Self::current_slot());

            let summary = Self::process_summary(last_block_in_range, this_validator);

            if let Err(e) = summary {
                debug::native::warn!("💔️ Error processing summary: {:?}", e);
            }

            // Ignore the remove storage lock error as the lock will be unlocked after the expiration period
            match OcwLock::remove_storage_lock(
                block_number,
                expiration,
                root_lock_name
            ) {
                Ok(_) => {},
                Err(e) => {debug::native::warn!("💔️ Error removing root lock: {:?}", e);}
            }
        }
    }

    fn register_offence_if_no_summary_created_in_slot(reporter: &Validator<T::AuthorityId, T::AccountId>) {
        if Self::last_summary_slot() < Self::current_slot() {

            let offender = Self::slot_validator();
            create_and_report_summary_offence::<T>(
                &reporter.account_id,
                &vec![offender],
                SummaryOffenceType::NoSummaryCreated);

            Self::deposit_event(RawEvent::SummaryNotPublishedOffence(
                Self::slot_validator(),
                Self::current_slot(),
                Self::last_summary_slot(),
                Self::block_number_for_next_slot()));
        }
    }

    // called from OCW - no storage changes allowed here
    fn can_process_summary(
        current_block_number: T::BlockNumber,
        last_block_in_range: T::BlockNumber,
        this_validator: &Validator<<T as avn::Config>::AuthorityId, T::AccountId>) -> bool
    {
        if OcwLock::is_locked(&Self::create_root_lock_name(last_block_in_range)) {
            return false;
        }

        let target_block_with_buffer = safe_add_block_numbers(last_block_in_range, T::MinBlockAge::get());

        if target_block_with_buffer.is_err() {
            debug::native::warn!("💔️ Error checking if we can process a summary for blocks {:?} to {:?}",
                current_block_number,
                last_block_in_range
            );

            return false;
        }
        let target_block_with_buffer = target_block_with_buffer.expect("Already checked");

        let root_range = RootRange::new(Self::get_next_block_to_process(), last_block_in_range);

        let is_slot_validator = this_validator.account_id == Self::slot_validator();
        let slot_is_active = current_block_number < Self::block_number_for_next_slot();
        let blocks_are_old_enough = current_block_number > target_block_with_buffer;

        return
            is_slot_validator &&
            slot_is_active &&
            blocks_are_old_enough &&
            Self::summary_is_neither_pending_nor_approved(&root_range);
    }

    // called from OCW - no storage changes allowed here
    fn process_summary(last_block_in_range: T::BlockNumber, validator: &Validator<<T as avn::Config>::AuthorityId, T::AccountId>)
        -> DispatchResult
    {
        let root_hash = Self::compute_root_hash(Self::get_next_block_to_process(), last_block_in_range)?;
        Self::record_summary(last_block_in_range, root_hash, validator)?;

        Ok(())
    }

    // called from OCW - no storage changes allowed here
    fn record_summary(
        last_processed_block_number: T::BlockNumber,
        root_hash: H256,
        validator: &Validator<<T as avn::Config>::AuthorityId, T::AccountId>) -> DispatchResult
    {
        let ingress_counter = Self::get_ingress_counter() + 1; // default value in storage is 0, so first root_hash has counter 1

        let signature = validator.key
            .sign(
                &(
                    UPDATE_BLOCK_NUMBER_CONTEXT,
                    root_hash,
                    ingress_counter,
                    last_processed_block_number
                ).encode())
            .ok_or(Error::<T>::ErrorSigning)?;

        debug::native::trace!(
            target: "avn",
            "🖊️  Worker records summary calculation: {:?} last processed block {:?} ingress: {:?}]",
            &root_hash,
            &last_processed_block_number,
            &ingress_counter
        );

        SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(
            Call::record_summary_calculation(last_processed_block_number, root_hash, ingress_counter, validator.clone(), signature).into()
        ).map_err(|_| Error::<T>::ErrorSubmittingTransaction)?;

        Ok(())
    }

    fn dispatch_advance_slot(validator: &Validator<<T as avn::Config>::AuthorityId, T::AccountId>) -> DispatchResult {
        let signature = validator.key
            .sign(&(ADVANCE_SLOT_CONTEXT, Self::current_slot()).encode())
            .ok_or(Error::<T>::ErrorSigning)?;

        SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(
            Call::advance_slot(validator.clone(), signature).into()
        ).map_err(|_| Error::<T>::ErrorSubmittingTransaction)?;

        Ok(())
    }

    fn get_target_block() -> Result<T::BlockNumber, Error<T>> {
        let end_block_number = safe_add_block_numbers::<T::BlockNumber>(Self::get_next_block_to_process(), Self::schedule_period())
            .map_err(|_| Error::<T>::Overflow)?;

        if Self::get_next_block_to_process() == 0u32.into() {
            return Ok(end_block_number);
        }

        Ok(safe_sub_block_numbers::<T::BlockNumber>(end_block_number, 1u32.into()).map_err(|_| Error::<T>::Overflow)?)
    }

    fn validate_response(response: Vec<u8>) -> Result<H256, Error<T>> {
        if response.len() != 64 {
            debug::native::error!("❌ Root hash is not valid: {:?}", response);
            return Err(Error::<T>::InvalidRootHashLength)?;
        }

        let root_hash = core::str::from_utf8(&response);
        if let Err(e) = root_hash {
            debug::native::error!("❌ Error converting root hash bytes to string: {:?}", e);
            return Err(Error::<T>::InvalidUTF8Bytes)?;
        }

        let mut data: [u8;32] = [0;32];
        hex::decode_to_slice(root_hash.expect("Checked for error"), &mut data[..]).map_err(|_| Error::<T>::InvalidHexString)?;

        return Ok(H256::from_slice(&data));
    }

    fn end_voting(reporter: T::AccountId, root_id: &RootId<T::BlockNumber>) -> DispatchResult {
        let voting_session = Self::get_root_voting_session(&root_id);

        ensure!(voting_session.is_valid(), Error::<T>::VotingSessionIsNotValid);

        let vote = Self::get_vote(root_id);
        ensure!(Self::can_end_vote(&vote), Error::<T>::ErrorEndingVotingPeriod);

        let root_is_approved = vote.is_approved();

        let root_data = Self::try_get_root_data(&root_id)?;
        if root_is_approved {
            if root_data.root_hash != Self::empty_root() {

                let result = T::CandidateTransactionSubmitter::submit_candidate_transaction_to_tier1(
                    EthTransactionType::PublishRoot(PublishRootData::new(*root_data.root_hash.as_fixed_bytes())),
                    *root_data.tx_id.as_ref().expect("Non empty roots have valid hash"),
                    root_data.added_by,
                    voting_session.state()?.confirmations,
                );

                if let Err(result) = result {
                    debug::native::error!("❌ Error Submitting Tx: {:?}", result);
                    Err(result)?
                }
                // There are a couple possible reasons for failure.
                // 1. We fail before sending to T1: likely a bug on our part
                // 2. Quorum mismatch. There is no guarantee that between accepting a root and submitting it to T1,
                // the tier2 session hasn't changed and with it the quorum, making ethereum-transactions reject it
                // In either case, we should not slash anyone.
            }
            // If we get here, then we did not get an error when submitting to T1.

            create_and_report_summary_offence::<T>(
                &reporter,
                &vote.nays,
                SummaryOffenceType::RejectedValidRoot);

            let next_block_to_process = safe_add_block_numbers::<T::BlockNumber>(root_id.range.to_block, 1u32.into())
                .map_err(|_| Error::<T>::Overflow)?;

            <NextBlockToProcess<T>>::put(next_block_to_process);
            <Roots<T>>::mutate(root_id.range, root_id.ingress_counter, |root| root.is_validated = true);
            <SlotOfLastPublishedSummary<T>>::put(Self::current_slot());
        } else {
            // We didn't get enough votes to approve this root

            let root_creator = root_data.added_by;
            create_and_report_summary_offence::<T>(
                &reporter,
                &vec![root_creator],
                SummaryOffenceType::CreatedInvalidRoot);

            create_and_report_summary_offence::<T>(
                &reporter,
                &vote.ayes,
                SummaryOffenceType::ApprovedInvalidRoot);
        }

        <PendingApproval<T>>::remove(root_id.range);

        // When we get here, the root's voting session has ended and it has been removed from PendingApproval
        // If the root was approved, it is now marked as validated. Otherwise, it stays false
        // If there was an error when submitting to T1, none of this happened and it is still pending and not validated
        // In either case, the whole voting history remains in storage

        // NOTE: when SYS-152 work is added here, root_range could exist several times in the voting history, since
        // a root_range that is rejected must eventually be submitted again. But at any given time, there should be a single
        // instance of root_range in the PendingApproval queue.
        // It is possible to keep several instances of root_range in the Roots repository. But that should not change the logic
        // in this area: we should still validate an approved (root_range, counter) and remove this pair from PendingApproval if no
        // errors occur.

        Self::deposit_event(Event::<T>::VotingEnded(*root_id, root_is_approved));

        Ok(())
    }

    fn can_end_vote(vote: &VotingSessionData<T::AccountId, T::BlockNumber>) -> bool {
        return vote.has_outcome() || <system::Module<T>>::block_number() >= vote.end_of_voting_period;
    }

    fn record_summary_validate_unsigned(_source: TransactionSource, call: &Call<T>) -> TransactionValidity {
        if let Call::record_summary_calculation(last_processed_block_number, root_hash, ingress_counter, validator, signature) = call {
            if validator.account_id != Self::slot_validator() {
                return InvalidTransaction::Custom(ERROR_CODE_VALIDATOR_IS_NOT_PRIMARY).into();
            }

            let signed_data = &(UPDATE_BLOCK_NUMBER_CONTEXT, root_hash, ingress_counter, last_processed_block_number);
            if !AVN::<T>::signature_is_valid(signed_data, &validator, signature) {
                return InvalidTransaction::BadProof.into();
            };

            return ValidTransaction::with_tag_prefix("Summary")
                .priority(TransactionPriority::max_value())
                .and_provides(vec![(UPDATE_BLOCK_NUMBER_CONTEXT, root_hash, ingress_counter).encode()])
                .longevity(64_u64)
                .propagate(true)
                .build();
        }

        return InvalidTransaction::Call.into();
    }

    fn advance_slot_validate_unsigned(_source: TransactionSource, call: &Call<T>) -> TransactionValidity {
        if let Call::advance_slot(validator, signature) = call {
            if validator.account_id != Self::slot_validator() {
                return InvalidTransaction::Custom(ERROR_CODE_VALIDATOR_IS_NOT_PRIMARY).into();
            }

            // QUESTION: slash here? If we check the signature validity first, then fail the check for slot_validator
            // we would prove someone tried to advance the slot outside their turn. Should this be slashable?

            let current_slot = Self::current_slot();
            let signed_data = &(ADVANCE_SLOT_CONTEXT, current_slot);
            if !AVN::<T>::signature_is_valid(signed_data, &validator, signature) {
                return InvalidTransaction::BadProof.into();
            };

            return ValidTransaction::with_tag_prefix("Summary")
                .priority(TransactionPriority::max_value())
                .and_provides(vec![(ADVANCE_SLOT_CONTEXT, current_slot).encode()])
                .longevity(64_u64)
                .propagate(true)
                .build();
        }

        return InvalidTransaction::Call.into();
    }

    fn empty_root() -> H256 {
        return H256::from_slice(&[0; 32]);
    }

    fn summary_is_neither_pending_nor_approved(root_range: &RootRange<T::BlockNumber>) -> bool {
        let has_been_approved = <Roots<T>>::iter_prefix_values(root_range).any(|root| root.is_validated);
        let is_pending = <PendingApproval<T>>::contains_key(root_range);

        return !is_pending && !has_been_approved;
    }

    fn try_get_root_data(root_id: &RootId<T::BlockNumber>) -> Result<RootData<T::AccountId>, Error<T>> {
        if <Roots<T>>::contains_key(root_id.range, root_id.ingress_counter) {
            return Ok(<Roots<T>>::get(root_id.range, root_id.ingress_counter));
        }

        Err(Error::<T>::RootDataNotFound)?
    }
}

impl<T: Config> frame_support::unsigned::ValidateUnsigned for Module<T> {
    type Call = Call<T>;

    fn validate_unsigned(source: TransactionSource, call: &Self::Call) -> TransactionValidity {
        if let Call::record_summary_calculation(_last_block, _root_hash, _ingress_counter, _validator, _signature) = call {
            return Self::record_summary_validate_unsigned(source, call);

        } else if let Call::end_voting_period(root_id, validator, signature) = call {
            let root_voting_session = Self::get_root_voting_session(root_id);
            return end_voting_period_validate_unsigned::<T>(&root_voting_session, validator, signature);

        } else if let Call::approve_root(root_id, validator, eth_signature, signature) = call {
            if !<Roots<T>>::contains_key(root_id.range, root_id.ingress_counter) {
                return InvalidTransaction::Custom(ERROR_CODE_INVALID_ROOT_RANGE).into();
            }

            let root_voting_session = Self::get_root_voting_session(root_id);

            let root_data = Self::try_get_root_data(&root_id)
                .map_err(|_| InvalidTransaction::Custom(ERROR_CODE_INVALID_ROOT_RANGE))?;

            let eth_encoded_data = Self::convert_data_to_eth_compatible_encoding(&root_data)
                .map_err(|_| InvalidTransaction::Custom(ERROR_CODE_INVALID_ROOT_DATA))?;

            return approve_vote_validate_unsigned::<T>(&root_voting_session, validator, eth_encoded_data.encode(), eth_signature, signature);

        } else if let Call::reject_root(root_id, validator, signature) = call {
            let root_voting_session = Self::get_root_voting_session(root_id);
            return reject_vote_validate_unsigned::<T>(&root_voting_session, validator, signature);

        } else if let Call::add_challenge(challenge, validator, signature) = call {
            return add_challenge_validate_unsigned::<T>(challenge, validator, signature);

        } else if let Call::advance_slot(_validator, _signature) = call {
            return Self::advance_slot_validate_unsigned(source, call);

        } else {
            return InvalidTransaction::Call.into();
        }
    }
}

#[derive(Encode, Decode, Default, Clone, Copy, PartialEq, Debug, Eq)]
pub struct RootId<BlockNumber: Member + AtLeast32Bit> {
    pub range: RootRange<BlockNumber>,
    pub ingress_counter: IngressCounter
}

impl<BlockNumber: Member + AtLeast32Bit> RootId<BlockNumber> {
    fn new(range: RootRange<BlockNumber>, ingress_counter: IngressCounter) -> Self {
        return RootId::<BlockNumber> {
            range,
            ingress_counter
        }
    }
}

#[derive(Encode, Decode, Default, Clone, Copy, PartialEq, Debug, Eq)]
pub struct RootRange<BlockNumber: Member + AtLeast32Bit> {
    pub from_block: BlockNumber,
    pub to_block: BlockNumber
}

impl<BlockNumber: Member + AtLeast32Bit> RootRange<BlockNumber> {
    fn new(from_block: BlockNumber, to_block: BlockNumber) -> Self {
        return RootRange::<BlockNumber> {
            from_block: from_block,
            to_block: to_block
        }
    }
}

#[derive(Encode, Decode, Default, Clone, PartialEq, Debug, Eq)]
pub struct RootData<AccountId: Member>  {
    pub root_hash: H256,
    pub added_by: AccountId,
    pub is_validated: bool, // This is set to true when 2/3 of validators approve it
    pub is_finalised: bool, // This is set to true when EthEvents confirms Tier1 has received the root
    pub tx_id: Option<TransactionId>, // This is the TransacionId that will be used to submit the tx
}

impl<AccountId: Member> RootData<AccountId> {
    fn new(root_hash: H256, added_by: AccountId, transaction_id: Option<TransactionId>) -> Self {
        return RootData::<AccountId> {
            root_hash: root_hash,
            added_by: added_by,
            is_validated: false,
            is_finalised: false,
            tx_id: transaction_id,
        }
    }
}

#[cfg(test)]
#[path = "tests/mock.rs"]
mod mock;

#[cfg(test)]
#[path = "../../avn/src/tests/extension_builder.rs"]
pub mod extension_builder;

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/tests_vote.rs"]
mod tests_vote;

#[cfg(test)]
#[path = "tests/tests_validate_unsigned.rs"]
mod tests_validate_unsigned;

#[cfg(test)]
#[path = "tests/tests_slot_logic.rs"]
mod tests_slots;

#[cfg(test)]
#[path = "tests/tests_challenge.rs"]
mod tests_challenge;

#[cfg(test)]
#[path = "tests/tests_set_periods.rs"]
mod tests_set_periods;

// TODO: Add unit tests for setting schedule period and voting period