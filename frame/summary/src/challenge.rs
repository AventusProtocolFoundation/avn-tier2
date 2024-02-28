#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Encode, Decode};
use sp_std::{prelude::*};
use sp_std::fmt::Debug;
use sp_application_crypto::RuntimeAppPublic;
use sp_runtime::{
    traits::Member,
    transaction_validity::{
        TransactionValidity,
        ValidTransaction,
        InvalidTransaction,
        TransactionPriority,
    }
};
use sp_avn_common::event_types::Validator;
use frame_support::{storage::StorageValue, debug};
use frame_system::offchain::SubmitTransaction;

use super::{Config, OcwLock, OcwOperationExpiration as OcwLockExpiry};
use crate::{Module as Summary, Call, Store, AVN};

pub const CHALLENGE_CONTEXT: &'static [u8] = b"root_challenge";
pub const UNKNOWN_CHALLENGE_REASON: u8 = 10;

pub type SlotNumber = u32;

#[derive(Encode, Decode, Clone, PartialEq, Debug, Eq)]
pub enum SummaryChallengeReason {
    /// The slot has not been advanced
    SlotNotAdvanced(SlotNumber),

    /// Default challenge reason
    Unknown,
}

#[derive(Encode, Decode, Default, Clone, PartialEq, Debug)]
pub struct SummaryChallenge<AccountId: Member> {
    pub challenge_reason: SummaryChallengeReason,
    pub challenger: AccountId,
    pub challengee: AccountId,
}

impl<AccountId: Member> SummaryChallenge<AccountId> {
    pub fn new(challenge_reason: SummaryChallengeReason, challenger: AccountId, challengee: AccountId) -> Self
    {
        return SummaryChallenge::<AccountId> {
            challenge_reason,
            challenger,
            challengee,
        };
    }

    /// Validates the challenge and returns true if it's correct.
    pub fn is_valid<T: Config>(&self, current_slot_number: T::BlockNumber, current_block_number: T::BlockNumber, challengee: &T::AccountId) -> bool
    {
        match self.challenge_reason {
            SummaryChallengeReason::SlotNotAdvanced(slot_number_to_challenge) => {
                return  T::BlockNumber::from(slot_number_to_challenge) == current_slot_number &&
                        Summary::<T>::grace_period_elapsed(current_block_number) &&
                        *challengee == <Summary<T> as Store>::CurrentSlotsValidator::get();
            },
            _ => false
        }
    }
}

impl Default for SummaryChallengeReason {
    fn default() -> Self { SummaryChallengeReason::Unknown }
}

pub fn add_challenge_validate_unsigned<T: Config>(
    challenge: &SummaryChallenge<T::AccountId>,
    validator: &Validator<T::AuthorityId, T::AccountId>,
    signature: &<T::AuthorityId as RuntimeAppPublic>::Signature) -> TransactionValidity
{
    if challenge.challenge_reason == SummaryChallengeReason::Unknown {
        return InvalidTransaction::Custom(UNKNOWN_CHALLENGE_REASON).into();
    }

    if !AVN::<T>::signature_is_valid(&(CHALLENGE_CONTEXT, challenge), &validator, signature) {
        return InvalidTransaction::BadProof.into();
    };

    return ValidTransaction::with_tag_prefix("summary_challenge")
            .priority(TransactionPriority::max_value())
            .and_provides(vec![(CHALLENGE_CONTEXT, challenge, validator).encode()])
            .longevity(64_u64)
            .propagate(true)
            .build();
}

pub fn challenge_slot_if_required<T: Config>(
    offchain_worker_block_number: T::BlockNumber,
    this_validator: &Validator<T::AuthorityId, T::AccountId>)
{
    let slot_number: T::BlockNumber = <Summary<T> as Store>::CurrentSlot::get();
    let slot_as_u32 = AVN::<T>::convert_block_number_to_u32(slot_number);
    if let Err(_) = slot_as_u32 {
        debug::native::error!("💔 Error converting block number: {:?} into u32", slot_number);
        return;
    }

    let challenge = SummaryChallenge::new(
        SummaryChallengeReason::SlotNotAdvanced(slot_as_u32.expect("Checked for error")),
        this_validator.account_id.clone(),
        <Summary<T> as Store>::CurrentSlotsValidator::get()
    );

    if can_challenge::<T>(&challenge, this_validator, offchain_worker_block_number) {
        let _ = send_challenge_transaction::<T>(&challenge, this_validator, offchain_worker_block_number);
    }
}

fn can_challenge<T: Config>(
    challenge: &SummaryChallenge<T::AccountId>,
    this_validator: &Validator<T::AuthorityId, T::AccountId>,
    ocw_block_number: T::BlockNumber) -> bool
{
    if OcwLock::is_locked(&challenge_lock_name::<T>(challenge)) {
        return false;
    }

    let is_chosen_validator = AVN::<T>::is_primary(ocw_block_number, &this_validator.account_id)
            .unwrap_or_else(|_| false);

    let grace_period_elapsed =  Summary::<T>::grace_period_elapsed(ocw_block_number);

    return is_chosen_validator && grace_period_elapsed;
}

fn send_challenge_transaction<T: Config>(
    challenge: &SummaryChallenge<T::AccountId>,
    this_validator: &Validator<T::AuthorityId, T::AccountId>,
    ocw_block_number: T::BlockNumber) -> Result<(), ()>
{
    let signature = this_validator.key.sign(&(CHALLENGE_CONTEXT, challenge).encode());

    if signature.is_none() {
        debug::native::error!("💔 Error signing challenge: {:?}", &challenge);
        return Err(());
    };

    if let Err(e) = SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(Call::add_challenge(
                        challenge.clone(), this_validator.clone(), signature.expect("We have a signature")).into()
                    )
    {
        debug::native::error!("💔 Error sending `challenge transaction`: {:?}. Error: {:?}", &challenge, e);
        return Err(());
    }

    // Add a lock to record the fact that we have sent a challenge.
    if let Err(()) = OcwLock::set_lock_with_expiry(ocw_block_number, OcwLockExpiry::Fast, challenge_lock_name::<T>(challenge)) {
        debug::native::warn!("ℹ️  Error adding a lock for `challenge transaction`: {:?}.", &challenge);
    }

    Ok(())
}

pub fn challenge_lock_name<T: Config>(challenge: &SummaryChallenge<T::AccountId>) -> OcwLock::PersistentId{
    let mut name = b"challenge_summary::slot::".to_vec();
    name.extend_from_slice(&mut challenge.encode());
    name
}
