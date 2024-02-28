//! # Aventus Finality Tracker Pallet
//!
//! This pallet is responsible for tracking the latest finalised block and storing it on chain
//!
//! All validators are expected to periodically send their opinion of what is the latest finalised block, and this pallet
//! will select the highest finalised block seen by 2/3 or more of the validators.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}};

use codec::{Encode, Decode};
use sp_std::{prelude::*, cmp};
use sp_runtime::{DispatchError, traits::{Member, AtLeast32Bit, Zero},
    transaction_validity::{TransactionValidity, ValidTransaction, TransactionPriority, TransactionSource, InvalidTransaction},
    offchain::storage::StorageValueRef
};

use sp_application_crypto::RuntimeAppPublic;
use sp_avn_common::{event_types::Validator, offchain_worker_storage_lock:: {self as OcwLock}};
use frame_support::{decl_event, decl_storage, decl_module, decl_error, dispatch::DispatchResult, ensure, debug, traits::Get};
use frame_system::{self as system, offchain::{SendTransactionTypes, SubmitTransaction}, ensure_none};
use pallet_avn::{self as avn, Error as avn_error, FinalisedBlockChecker};

const NAME: &'static [u8; 12] = b"avn-finality";
const UPDATE_FINALISED_BLOCK_NUMBER_CONTEXT: &'static [u8] = b"update_finalised_block_number_signing_context";

const FINALISED_BLOCK_END_POINT: &str = "latest_finalised_block";

// used in benchmarks and weights calculation only
const MAX_VALIDATOR_ACCOUNT_IDS: u32 = 10;

pub type AVN<T> = avn::Module::<T>;

#[cfg(test)]
mod mock;

mod benchmarking;

// TODO: [TYPE: business logic][PRI: high][CRITICAL]
// Rerun benchmark in production and update both ./default_weights.rs file and /bin/node/runtime/src/weights/pallet_avn_finality_tracker.rs file.
pub mod default_weights;
pub use default_weights::WeightInfo;

pub trait Config: SendTransactionTypes<Call<Self>> + system::Config + avn::Config {
    /// Overarching event type
    type Event: From<Event<Self>> + Into<<Self as system::Config>::Event>;
    /// The number of block we can keep the calculated finalised block, before recalculating it again.
    type CacheAge: Get<Self::BlockNumber>;
    /// The interval, in block number, of sumbitting updates
    type SubmissionInterval: Get<Self::BlockNumber>;
    /// The delay after which point things become suspicious. Default is 100.
    type ReportLatency: Get<Self::BlockNumber>;
    /// Weight information for the extrinsics in this pallet.
    type WeightInfo: WeightInfo;
}

decl_event!(
    pub enum Event<T> where
        <T as system::Config>::BlockNumber,
    {
        /// BlockNumber is the new finalised block number
        FinalisedBlockUpdated(BlockNumber),
        /// BlockNumber is the last block number data was updated
        FinalisedBlockUpdateStalled(BlockNumber),
    }
);

decl_storage! {
	trait Store for Module<T: Config> as AvnFinalityTracker {
        // The latest finalised block number
        LatestFinalisedBlock get(fn latest_finalised_block_number): T::BlockNumber;
        // The block number where finalised block was last updated
        LastFinalisedBlockUpdate get(fn last_finalised_block_update): T::BlockNumber;
        // The block number where a finalised block was last submitted
        LastFinalisedBlockSubmission get(fn last_finalised_block_submission): T::BlockNumber;
        // Map of validator account ids and their reported finalised block numbers
        SubmittedBlockNumbers get(fn submissions): map hasher(blake2_128_concat) T::AccountId => SubmissionData<T::BlockNumber>;
	}
}

decl_error! {
	pub enum Error for Module<T: Config> {
        /// Finalized height above block number
        InvalidSubmission,
        ErrorGettingDataFromService,
        InvalidResponseType,
        ErrorDecodingResponse,
        ErrorSigning,
        ErrorSubmittingTransaction,
        SubmitterNotAValidator,
        NotAllowedToSubmitAtThisTime
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;

        /// # <weight>
        /// Key: V - Number of validators
        /// - DbReads: `LatestFinalisedBlock`, 3 * `SubmittedBlockNumbers`: O(1)
        /// - DbWrites: `LastFinalisedBlockSubmission`, `SubmittedBlockNumbers`: O(1)
        /// - avn pallet is_validator operation: O(V).
        /// Total Complexity: `O(1 + V)`
        /// # </weight>
        #[weight = <T as Config>::WeightInfo::submit_latest_finalised_block_number(MAX_VALIDATOR_ACCOUNT_IDS)]
        fn submit_latest_finalised_block_number(
            origin,
            new_finalised_block_number: T::BlockNumber,
            validator: Validator<<T as avn::Config>::AuthorityId, T::AccountId>,
            _signature: <<T as avn::Config>::AuthorityId as RuntimeAppPublic>::Signature) -> DispatchResult
        {
            ensure_none(origin)?;
            ensure!(AVN::<T>::is_validator(&validator.account_id), Error::<T>::SubmitterNotAValidator);
            ensure!(new_finalised_block_number > Self::latest_finalised_block_number(), Error::<T>::InvalidSubmission);
            ensure!(Self::is_submission_valid(&validator), Error::<T>::NotAllowedToSubmitAtThisTime);

            let current_block_number = <system::Module<T>>::block_number();
            let submission_data = SubmissionData::new(new_finalised_block_number, current_block_number);

            // No errors allowed below this line
            Self::record_submission(&validator.account_id, submission_data);
            LastFinalisedBlockSubmission::<T>::put(current_block_number);

            Ok(())
        }

        fn offchain_worker(block_number: T::BlockNumber) {
            let setup_result = AVN::<T>::pre_run_setup(block_number, NAME.to_vec());
            if let Err(e) = setup_result {
                match e {
                    _ if e == DispatchError::from(avn_error::<T>::OffchainWorkerAlreadyRun) => {();},
                    _ => {debug::native::error!("💔 Unable to run offchain worker: {:?}", e);}
                };

                return ;
            }
            let this_validator = setup_result.expect("We have a validator");

            Self::submit_finalised_block_if_required(&this_validator);
        }

        fn on_finalize() {
            Self::update_latest_finalised_block_if_required();
        }
    }
}

impl<T: Config> Module<T> {

    /// This function will only update the finalised block if there are 2/3rd or more submissions from distinct validators
    pub fn update_latest_finalised_block_if_required() {
        let quorum = AVN::<T>::calculate_two_third_quorum();
        let current_block_number = <system::Module<T>>::block_number();
        let last_finalised_block_submission = Self::last_finalised_block_submission();

        let quorum_is_reached = SubmittedBlockNumbers::<T>::iter().count() as u32 >= quorum;
        let block_is_stale = current_block_number > Self::last_finalised_block_update() + T::CacheAge::get();
        let new_submissions_available = last_finalised_block_submission > Self::last_finalised_block_update();

        let can_update = quorum_is_reached && block_is_stale && new_submissions_available;

        if can_update {
            let calculated_finalised_block = Self::calculate_finalised_block(quorum);

            if calculated_finalised_block > Self::latest_finalised_block_number() {
                LastFinalisedBlockUpdate::<T>::put(current_block_number);
                LatestFinalisedBlock::<T>::put(calculated_finalised_block);
                Self::deposit_event(Event::<T>::FinalisedBlockUpdated(calculated_finalised_block));
            }

            // check if there is something wrong with submissions in general and notify via an event
            if current_block_number - last_finalised_block_submission > T::ReportLatency::get() {
                Self::deposit_event(Event::<T>::FinalisedBlockUpdateStalled(last_finalised_block_submission));
            }
        }
    }

    /// This method assumes all validation (such as quorum) has passed before being called.
    fn calculate_finalised_block(quorum: u32) -> T::BlockNumber {
        let mut block_candidates = vec![];
        let mut removed_validators = vec![];

        for (validator_account_id, submission) in <SubmittedBlockNumbers<T>>::iter() {
            let validator_is_active = AVN::<T>::is_validator(&validator_account_id);

            if submission.finalised_block > <T as system::Config>::BlockNumber::zero() && validator_is_active {
                block_candidates.push(submission.finalised_block);
            }

            // Keep track and remove any inactive validators
            if !validator_is_active {
                removed_validators.push(validator_account_id);
            }
        }

        removed_validators.iter().for_each(|val| SubmittedBlockNumbers::<T>::remove(&val));

        block_candidates.sort();
        let can_be_ignored = block_candidates.len().saturating_sub(quorum as usize);
        return block_candidates[can_be_ignored];
    }

    fn record_submission(submitter: &T::AccountId, submission_data: SubmissionData<T::BlockNumber>)
    {
        if SubmittedBlockNumbers::<T>::contains_key(submitter) {
            SubmittedBlockNumbers::<T>::mutate(submitter, |data| *data = submission_data);
        } else {
            SubmittedBlockNumbers::<T>::insert(submitter, submission_data);
        }
    }

    fn is_submission_valid(submitter: &Validator<<T as avn::Config>::AuthorityId, T::AccountId>) -> bool {
        let has_submitted_before = SubmittedBlockNumbers::<T>::contains_key(&submitter.account_id);

        if has_submitted_before {
            let last_submission = Self::submissions(&submitter.account_id).submitted_at_block;
            return <system::Module<T>>::block_number() > last_submission + T::SubmissionInterval::get();
        }

        return true;
    }

    // Called from OCW, no storage changes allowed
    fn submit_finalised_block_if_required(this_validator: &Validator<<T as avn::Config>::AuthorityId, T::AccountId>) {
        if Self::can_submit_finalised_block(this_validator) == false {
            return;
        }

        let finalised_block_result = Self::get_finalised_block_from_external_service();
        if let Err(ref e) = finalised_block_result {
            debug::native::error!("💔 Error getting finalised block from external service: {:?}", e);
            return;
        }
        let calculated_finalised_block = finalised_block_result.expect("checked for errors");

        if calculated_finalised_block <= Self::latest_finalised_block_number() {
            // Only submit if the calculated value is greater than the current value
            return;
        }

        // send a transaction on chain with the latest finalised block data. We shouldn't have any sig re-use issue here
        // because new block number must be > current finalised block number
        let signature = this_validator.key
            .sign(&(UPDATE_FINALISED_BLOCK_NUMBER_CONTEXT, calculated_finalised_block).encode())
            .ok_or(Error::<T>::ErrorSigning);

        if let Err(ref e) = signature {
            debug::native::error!("💔 Error signing `submit finalised block` tranaction: {:?}", e);
            return;
        }

        let result = SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(
            Call::submit_latest_finalised_block_number(
                calculated_finalised_block,
                this_validator.clone(),
                signature.expect("checked for errors")
            ).into()
        ).map_err(|_| Error::<T>::ErrorSubmittingTransaction);

        if let Err(e) = result {
            debug::native::error!("💔 Error sending transaction to submit finalised block: {:?}", e);
            return;
        }

        Self::set_last_finalised_block_submission_in_local_storage(calculated_finalised_block);
    }

    // Called from OCW, no storage changes allowed
    fn can_submit_finalised_block(this_validator: &Validator<<T as avn::Config>::AuthorityId, T::AccountId>) -> bool
    {
        let has_submitted_before = SubmittedBlockNumbers::<T>::contains_key(&this_validator.account_id);

        if has_submitted_before {
            let last_submission_in_state = Self::submissions(&this_validator.account_id).submitted_at_block;
            let last_submission_in_local_storage = Self::get_last_finalised_block_submission_from_local_storage();
            let last_submission = cmp::max(last_submission_in_state, last_submission_in_local_storage);

            return <system::Module<T>>::block_number() > last_submission + T::SubmissionInterval::get();
        }

        return true;
    }

    // Called from OCW, no storage changes allowed
    fn get_finalised_block_from_external_service() -> Result<T::BlockNumber, Error<T>>
    {
        let response = AVN::<T>::get_data_from_service(FINALISED_BLOCK_END_POINT.to_string())
            .map_err(|_| Error::<T>::ErrorGettingDataFromService)?;

        let finalised_block_bytes = hex::decode(&response).map_err(|_| Error::<T>::InvalidResponseType)?;
        let finalised_block = u32::decode(&mut &finalised_block_bytes[..]).map_err(|_| Error::<T>::ErrorDecodingResponse)?;
        let latest_finalised_block_number = T::BlockNumber::from(finalised_block);

        return Ok(latest_finalised_block_number);
    }

    fn get_persistent_local_storage_name() -> OcwLock::PersistentId{
        return b"last_finalised_block_submission::".to_vec();
    }

    // TODO: Try to move to offchain_worker_storage_locks
    // Called from an OCW, no state changes allowed
    fn get_last_finalised_block_submission_from_local_storage() -> T::BlockNumber {
        let local_stoage_key = Self::get_persistent_local_storage_name();
        let stored_value: Option<Option<T::BlockNumber>> = StorageValueRef::persistent(&local_stoage_key).get();
        let last_finalised_block_submission = match stored_value {
            // If the value is found
            Some(Some(block)) => block,
            // In every other case return 0.
            _ => <T as system::Config>::BlockNumber::zero()
        };

        return last_finalised_block_submission;
    }

    // TODO: Try to move to offchain_worker_storage_locks
    // Called from an OCW, no state changes allowed
    fn set_last_finalised_block_submission_in_local_storage(last_submission: T::BlockNumber) {
        const INVALID_VALUE: () = ();

        let local_stoage_key = Self::get_persistent_local_storage_name();
        let val = StorageValueRef::persistent(&local_stoage_key);
        let result = val.mutate(|last_run: Option<Option<T::BlockNumber>>| {
            match last_run {
                Some(Some(block)) if block >= last_submission => Err(INVALID_VALUE),
                _ => Ok(last_submission)
            }
        });

        match result {
            Err(INVALID_VALUE) => {
                debug::native::warn!("Attempt to update local storage with invalid value {:?}", last_submission);
            },
            Ok(Err(_)) => {
                debug::native::error!("💔 Error updating local storage with latest submission: {:?}", last_submission);
            },
            _ => {}
        }
    }
}

impl<T: Config> frame_support::unsigned::ValidateUnsigned for Module<T> {
    type Call = Call<T>;

    fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
        if let Call::submit_latest_finalised_block_number(finalised_block, validator, signature) = call {

            let signed_data = &(UPDATE_FINALISED_BLOCK_NUMBER_CONTEXT, finalised_block);
            if !AVN::<T>::signature_is_valid(signed_data, &validator, signature) {
                return InvalidTransaction::BadProof.into();
            };

            ValidTransaction::with_tag_prefix("AvnFinalityTracker")
                .priority(TransactionPriority::max_value())
                .and_provides(vec![(finalised_block, validator).encode()])
                .longevity(10) // after 10 block we have to revalidate this transaction
                .propagate(true)
                .build()
        } else {
            return InvalidTransaction::Call.into();
        }
    }
}

impl<T: Config> FinalisedBlockChecker<T::BlockNumber> for Module<T> {
	fn is_finalised(block_number: T::BlockNumber) -> bool {
        return Self::latest_finalised_block_number() >= block_number;
    }
}

#[derive(Encode, Decode, Default, Clone, Copy, PartialEq, Debug, Eq)]
pub struct SubmissionData<BlockNumber: Member + AtLeast32Bit> {
    pub finalised_block: BlockNumber,
    pub submitted_at_block: BlockNumber
}

impl<BlockNumber: Member + AtLeast32Bit> SubmissionData<BlockNumber> {
    fn new(finalised_block: BlockNumber, submitted_at_block: BlockNumber) -> Self {
        return SubmissionData::<BlockNumber> {
            finalised_block,
            submitted_at_block
        }
    }
}