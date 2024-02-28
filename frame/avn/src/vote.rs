#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}};

use codec::{Encode, Decode};
use sp_std::{prelude::*};
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
use sp_core::ecdsa;
use frame_support::{dispatch::{DispatchResult, DispatchError}, ensure};
use sp_application_crypto::RuntimeAppPublic;

use super::{Config, Error};
use crate::{Module as AVN};

pub const APPROVE_VOTE_IS_NOT_VALID: u8 = 2;
pub const REJECT_VOTE_IS_NOT_VALID: u8 = 3;
pub const VOTE_SESSION_IS_NOT_VALID: u8 = 4;
pub const VOTING_SESSION_DATA_IS_NOT_FOUND: u8 = 5;
pub const APPROVE_VOTE: bool = true;
pub const REJECT_VOTE: bool = false;

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, Debug)]
pub struct VotingSessionData<AccountId, BlockNumber> {
    /// The unique identifier for this voting session
    pub voting_session_id: Vec<u8>,
    /// The number of approval votes that are needed to reach an outcome.
    pub threshold: u32,
    /// The current set of voters that approved it.
    pub ayes: Vec<AccountId>,
    /// The current set of voters that rejected it.
    pub nays: Vec<AccountId>,
    /// The hard end time of this vote.
    pub end_of_voting_period: BlockNumber,
    /// The confirmations collected from the aye votes
    pub confirmations: Vec<ecdsa::Signature>,
    /// The block number this session was created on
    pub created_at_block: BlockNumber
}

impl<AccountId: Member, BlockNumber: Member> VotingSessionData<AccountId, BlockNumber> {
    pub fn new(id: Vec<u8>, threshold: u32, end_of_voting_period: BlockNumber, created_at_block: BlockNumber) -> Self
    {
        return VotingSessionData::<AccountId, BlockNumber> {
            voting_session_id: id,
            threshold: threshold,
            ayes: Vec::new(),
            nays: Vec::new(),
            end_of_voting_period: end_of_voting_period,
            confirmations: Vec::new(),
            created_at_block: created_at_block,
        };
    }

    pub fn has_outcome(&self) -> bool {
        let threshold: usize = self.threshold as usize;
        return self.ayes.len() >= threshold || self.nays.len() >= threshold;
    }

    // The vote has been accepted (positive votes surpass or match the negative ones)
    pub fn is_approved(&self) -> bool {
        return self.ayes.len() >= self.threshold as usize;
    }

    // The voter has already cast a vote for this vote subject in this session
    pub fn has_voted(&self, voter: &AccountId) -> bool {
        return self.ayes.contains(voter) || self.nays.contains(voter);
    }
}

pub trait VotingSessionManager<AccountId, BlockNumber> {

    fn cast_vote_context(&self) -> &'static [u8];

    fn end_voting_period_context(&self) -> &'static [u8];

    fn state(&self) -> Result<VotingSessionData<AccountId, BlockNumber>, DispatchError>;

    // The voting session has been created and the vote subject is in the correct state (for example, pending a vote)
    fn is_valid(&self) -> bool;

    // The session is valid (is_valid) AND has not ended yet
    fn is_active(&self) -> bool;

    fn record_approve_vote(
        &self,
        voter: AccountId,
        approval_signature: ecdsa::Signature
    );

    fn record_reject_vote(
        &self,
        voter: AccountId
    );

    fn end_voting_session(&self, sender: AccountId) -> DispatchResult;
}

pub fn process_approve_vote<T: Config>(
    voting_session: &Box<dyn VotingSessionManager::<T::AccountId, T::BlockNumber>>,
    voter: T::AccountId,
    approval_signature: ecdsa::Signature) -> DispatchResult
{
    validate_vote::<T>(voting_session, &voter)?;
    voting_session.record_approve_vote(voter.clone(), approval_signature);
    end_voting_if_outcome_reached::<T>(voting_session, voter)?;
    Ok(())
}

pub fn validate_vote<T: Config>(
    voting_session: &Box<dyn VotingSessionManager::<T::AccountId, T::BlockNumber>>,
    voter: &T::AccountId) -> DispatchResult
{
    ensure!(AVN::<T>::is_validator(voter), Error::<T>::NotAValidator);
    ensure!(voting_session.is_active(), Error::<T>::InvalidVote);
    ensure!(!voting_session.state()?.has_voted(voter), Error::<T>::DuplicateVote);
    Ok(())
}

pub fn process_reject_vote<T: Config>(
    voting_session: &Box<dyn VotingSessionManager::<T::AccountId, T::BlockNumber>>,
    voter: T::AccountId) -> DispatchResult
{
    validate_vote::<T>(voting_session, &voter)?;
    voting_session.record_reject_vote(voter.clone());
    end_voting_if_outcome_reached::<T>(voting_session, voter)?;
    Ok(())
}

fn end_voting_if_outcome_reached<T: Config>(
    voting_session: &Box<dyn VotingSessionManager::<T::AccountId, T::BlockNumber>>,
    voter: T::AccountId) -> DispatchResult
{
    if voting_session.state()?.has_outcome() && voting_session.is_active() {
        voting_session.end_voting_session(voter)?;
    }

    Ok(())
}

pub fn end_voting_period_validate_unsigned<T: Config>(
    voting_session: &Box<dyn VotingSessionManager::<T::AccountId, T::BlockNumber>>,
    validator: &Validator<T::AuthorityId, T::AccountId>,
    signature: &<T::AuthorityId as RuntimeAppPublic>::Signature) -> TransactionValidity
{
    if !voting_session.is_valid() {
        return InvalidTransaction::Custom(VOTE_SESSION_IS_NOT_VALID).into();
    }

    let voting_session_data = voting_session.state();
    if voting_session_data.is_err() {
        return InvalidTransaction::Custom(VOTING_SESSION_DATA_IS_NOT_FOUND).into();
    }

    let voting_session_id = voting_session_data.expect("voting session data is ok").voting_session_id;

    if !AVN::<T>::signature_is_valid(&(voting_session.end_voting_period_context(), voting_session_id.clone()), &validator, signature) {
        return InvalidTransaction::BadProof.into();
    };

    return ValidTransaction::with_tag_prefix("vote")
            .priority(TransactionPriority::max_value())
            .and_provides(vec![
                (
                    voting_session.end_voting_period_context(),
                    voting_session_id,
                    validator
                ).encode()
            ])
            .longevity(64_u64)
            .propagate(true)
            .build()
}

/// Unlike SR signatures, `_eth_signature` is not validated here and MUST be validated in the public dispatch method being called.
/// ECDSA sig validation is different because we cannot raise an offence (and mutate storage) from validate_unsigned.
pub fn approve_vote_validate_unsigned<T: Config>(
    voting_session: &Box<dyn VotingSessionManager::<T::AccountId, T::BlockNumber>>,
    validator: &Validator<T::AuthorityId, T::AccountId>,
    eth_encoded_data: Vec<u8>,
    eth_signature: &ecdsa::Signature,
    signature: &<T::AuthorityId as RuntimeAppPublic>::Signature) -> TransactionValidity
{
    if validate_vote::<T>(&voting_session, &validator.account_id).is_err() {
        return InvalidTransaction::Custom(APPROVE_VOTE_IS_NOT_VALID).into();
    }

    let voting_session_data = voting_session.state();
    if voting_session_data.is_err() {
        return InvalidTransaction::Custom(VOTING_SESSION_DATA_IS_NOT_FOUND).into();
    }

    let voting_session_id = voting_session_data.expect("voting session data is ok").voting_session_id;

    if !AVN::<T>::signature_is_valid(
        &(
            voting_session.cast_vote_context(),
            &voting_session_id,
            APPROVE_VOTE,
            eth_encoded_data,
            eth_signature.encode(),
        ),
        &validator,
        signature
    )
    {
        return InvalidTransaction::BadProof.into();
    };

    return ValidTransaction::with_tag_prefix("vote")
            .priority(TransactionPriority::max_value())
            .and_provides(vec![(
                voting_session.cast_vote_context(),
                voting_session_id,
                APPROVE_VOTE,
                validator).encode()
            ])
            .longevity(64_u64)
            .propagate(true)
            .build()
}

pub fn reject_vote_validate_unsigned<T: Config>(
    voting_session: &Box<dyn VotingSessionManager::<T::AccountId, T::BlockNumber>>,
    validator: &Validator<T::AuthorityId, T::AccountId>,
    signature: &<T::AuthorityId as RuntimeAppPublic>::Signature) -> TransactionValidity
{
    // TODO: Check if we can end the vote here
    if validate_vote::<T>(&voting_session, &validator.account_id).is_err() {
        return InvalidTransaction::Custom(REJECT_VOTE_IS_NOT_VALID).into();
    }

    let voting_session_data = voting_session.state();
    if voting_session_data.is_err() {
        return InvalidTransaction::Custom(VOTING_SESSION_DATA_IS_NOT_FOUND).into();
    }

    let voting_session_id = voting_session_data.expect("voting session data is ok").voting_session_id;

    if !AVN::<T>::signature_is_valid(&(voting_session.cast_vote_context(), &voting_session_id, REJECT_VOTE), &validator, signature) {
        return InvalidTransaction::BadProof.into();
    };

    return ValidTransaction::with_tag_prefix("vote")
            .priority(TransactionPriority::max_value())
            .and_provides(vec![(
                voting_session.cast_vote_context(),
                voting_session_id,
                REJECT_VOTE,
                validator).encode()
            ])
            .longevity(64_u64)
            .propagate(true)
            .build()
}
