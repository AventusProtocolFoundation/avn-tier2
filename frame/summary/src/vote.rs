#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{string::String};

use codec::{Encode, Decode};
use sp_std::{prelude::*};
use sp_avn_common::{
    event_types::Validator,
    offchain_worker_storage_lock:: {self as OcwLock}
};

use sp_core::ecdsa;
use sp_application_crypto::RuntimeAppPublic;
use frame_system::offchain::SubmitTransaction;
use frame_support::{dispatch::{DispatchResult, DispatchError}, debug, storage::{StorageMap, IterableStorageMap}};
use pallet_avn::{self as avn, vote::*};
use sp_std::fmt::Debug;

use super::{Config, Call};
use crate::{Module as Summary, Store, RootId, AVN};

pub const CAST_VOTE_CONTEXT: &'static [u8] = b"root_casting_vote";
pub const END_VOTING_PERIOD_CONTEXT: &'static [u8] = b"root_end_voting_period";
const MAX_VOTING_SESSIONS_RETURNED: usize = 5;

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, Debug)]
pub struct RootVotingSession<T: Config> {
    pub root_id: RootId<T::BlockNumber>
}

impl <T: Config> RootVotingSession<T> {
    pub fn new(root_id: &RootId<T::BlockNumber>) -> Self
    {
        return RootVotingSession::<T> {
            root_id: root_id.clone()
        };
    }
}

impl<T: Config> VotingSessionManager<T::AccountId, T::BlockNumber> for RootVotingSession<T>
{
    fn cast_vote_context(&self) -> &'static [u8] {
        return CAST_VOTE_CONTEXT;
    }

    fn end_voting_period_context(&self) -> &'static [u8] {
        return END_VOTING_PERIOD_CONTEXT;
    }

    fn state(&self) -> Result<VotingSessionData<T::AccountId, T::BlockNumber>, DispatchError> {
        if <Summary<T> as Store>::VotesRepository::contains_key(self.root_id) {
            return Ok(Summary::<T>::get_vote(self.root_id));
        }
        return Err(DispatchError::Other("Root Id is not found in votes repository"));
    }

    fn is_valid(&self) -> bool {
        let voting_session_data = self.state();
        let root_data_result = Summary::<T>::try_get_root_data(&self.root_id);
        let root_is_pending_approval = <Summary<T> as Store>::PendingApproval::contains_key(&self.root_id.range);
        let voting_session_exists_for_root = <Summary<T> as Store>::VotesRepository::contains_key(&self.root_id);

        if root_data_result.is_err() || !root_is_pending_approval || !voting_session_exists_for_root || voting_session_data.is_err() {
            return false;
        }

        let root_already_accepted = root_data_result.expect("already checked for error").is_validated;
        let pending_approval_root_ingress_counter = <Summary<T> as Store>::PendingApproval::get(self.root_id.range);
        let vote_is_for_correct_version_of_root_range = pending_approval_root_ingress_counter == self.root_id.ingress_counter;
        let voting_session_is_finalised = AVN::<T>::is_block_finalised(voting_session_data.expect("checked").created_at_block);

        return !root_already_accepted && vote_is_for_correct_version_of_root_range && voting_session_is_finalised;
    }

    fn is_active(&self) -> bool {
        let voting_session_data = self.state();
        return voting_session_data.is_ok() &&
            <frame_system::Module<T>>::block_number() < voting_session_data.expect("voting session data is ok").end_of_voting_period &&
            self.is_valid();
    }

    fn record_approve_vote(&self, voter: T::AccountId, approval_signature: ecdsa::Signature) {
        <Summary<T> as Store>::VotesRepository::mutate(&self.root_id, |vote| {
            vote.ayes.push(voter);
            vote.confirmations.push(approval_signature);
        })
    }

    fn record_reject_vote(&self, voter: T::AccountId) {
        <Summary<T> as Store>::VotesRepository::mutate(&self.root_id, |vote| vote.nays.push(voter));
    }

    fn end_voting_session(&self, sender: T::AccountId) -> DispatchResult {
        return Summary::<T>::end_voting(sender, &self.root_id);
    }
}

/***************************** Functions that run in an offchain worker context *****************************/

pub fn create_vote_lock_name<T: Config>(root_id: &RootId<T::BlockNumber>) -> OcwLock::PersistentId{
    let mut name = b"vote_summary::hash::".to_vec();
    name.extend_from_slice(&mut root_id.range.from_block.encode());
    name.extend_from_slice(&mut root_id.range.to_block.encode());
    name.extend_from_slice(&mut root_id.ingress_counter.encode());
    name
}

fn is_vote_in_transaction_pool<T: Config>(root_id: &RootId<T::BlockNumber>) -> bool {
    let persistent_data = create_vote_lock_name::<T>(root_id);
    return OcwLock::is_locked(&persistent_data);
}

pub fn cast_votes_if_required<T: Config>(
    block_number: T::BlockNumber,
    this_validator: &Validator<<T as avn::Config>::AuthorityId, T::AccountId>)
{
    let root_ids: Vec<RootId<T::BlockNumber>> = <Summary<T> as Store>::PendingApproval::iter()
        .filter(|(root_range, ingress_counter)| {
            let root_id = RootId::new(*root_range, *ingress_counter);
            root_can_be_voted_on::<T>(&root_id, &this_validator.account_id)
        })
        .take(MAX_VOTING_SESSIONS_RETURNED)
        .map(|(root_range, ingress_counter)| RootId::new(root_range, ingress_counter))
        .collect();

    // try to send 1 of MAX_VOTING_SESSIONS_RETURNED votes
    for root_id in root_ids {
        if OcwLock::set_lock_with_expiry(block_number, Summary::<T>::lock_till_request_expires(), create_vote_lock_name::<T>(&root_id)).is_err() {
            debug::native::trace!(target: "avn", "🤷 Unable to acquire local lock for root {:?}. Lock probably exists already", &root_id);
            continue;
        }
        let root_hash = Summary::<T>::compute_root_hash(root_id.range.from_block, root_id.range.to_block);
        if root_hash.is_err() {
            debug::native::error!("💔️ Error getting root hash while signing root id {:?} to vote", &root_id);
            continue;
        }

        let root_data = Summary::<T>::try_get_root_data(&root_id);
        if let Err(e) = root_data {
            debug::native::error!("💔️ Error getting root data while signing root id {:?} to vote. {:?}", &root_id, e);
            continue;
        }

        if root_hash.expect("has valid hash") == root_data.expect("checked for error").root_hash {
            if send_approve_vote::<T>(&root_id, this_validator).is_err() {
                // TODO: should we output any error message here?
                continue;
            }
        } else {
            if send_reject_vote::<T>(&root_id, this_validator).is_err() {
                // TODO: should we output any error message here?
                continue;
            }
        }

        return;
    }
}

pub fn end_voting_if_required<T: Config>(
    block_number: T::BlockNumber,
    this_validator: &Validator<<T as avn::Config>::AuthorityId, T::AccountId>)
{
    let root_ids: Vec<RootId<T::BlockNumber>> = <Summary<T> as Store>::PendingApproval::iter()
        .filter(|(root_range, ingress_counter)| {
            let root_id = RootId::new(*root_range, *ingress_counter);
            block_number > Summary::<T>::get_vote(root_id).end_of_voting_period
        })
        .take(MAX_VOTING_SESSIONS_RETURNED)
        .map(|(root_range, ingress_counter)| RootId::new(root_range, ingress_counter))
        .collect();

    for root_id in root_ids {
        let voting_session_data = Summary::<T>::get_root_voting_session(&root_id).state();
        if voting_session_data.is_err() {
            debug::native::error!("💔 Error getting voting session data with root id {:?} to end voting period", &root_id);
            return;
        }

        let voting_session_id = voting_session_data.expect("voting session data is ok").voting_session_id;
        let signature = match this_validator.key.sign(&(END_VOTING_PERIOD_CONTEXT, voting_session_id).encode())
        {
            Some(s) => s,
            _ => {
                    debug::native::error!("💔️ Error signing root id {:?} to end voting period", &root_id);
                    return;
                }
        };

        if let Err(e) = SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(Call::end_voting_period(
                            root_id.clone(),
                            this_validator.clone(),
                            signature).into()
        ) {
            debug::native::error!("💔️ Error sending transaction to end vote for root id {:?}: {:?}", root_id, e);
        }
    }
}

fn root_can_be_voted_on<T: Config>(root_id: &RootId<T::BlockNumber>, voter: &T::AccountId) -> bool {
    // There is an edge case here. If this is being run very close to `end_of_voting_period`, by the time the vote gets mined
    // It may be outside the voting window and get rejected.
    let root_voting_session = Summary::<T>::get_root_voting_session(root_id);
    let voting_session_data = root_voting_session.state();
    return  voting_session_data.is_ok() &&
        !voting_session_data.expect("voting session data is ok").has_voted(voter) &&
        !is_vote_in_transaction_pool::<T>(root_id) &&
        root_voting_session.is_active();
}

fn send_approve_vote<T: Config>(
    root_id: &RootId::<T::BlockNumber>,
    this_validator: &Validator<<T as avn::Config>::AuthorityId, T::AccountId>) -> Result<(), ()>
{
    let (eth_encoded_data, eth_signature) = Summary::<T>::sign_root_for_ethereum(&root_id).map_err(|_| ())?;

    let approve_vote_extrinsic_signature = sign_for_approve_vote_extrinsic::<T>(
        root_id,
        this_validator,
        eth_encoded_data,
        &eth_signature
    )?;

    debug::native::trace!(target: "avn", "🖊️  Worker sends approval vote for summary calculation: {:?}]", &root_id);

    if let Err(e) = SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(Call::approve_root(
                        root_id.clone(), this_validator.clone(), eth_signature, approve_vote_extrinsic_signature).into()
                    )
    {
        debug::native::error!("💔️ Error sending `approve vote transaction` for root id {:?}: {:?}", root_id, e);
        return Err(());
    }

    Ok(())
}

fn sign_for_approve_vote_extrinsic<T: Config>(
    root_id: &RootId::<T::BlockNumber>,
    this_validator: &Validator<<T as avn::Config>::AuthorityId, T::AccountId>,
    eth_encoded_data: String,
    eth_signature: &ecdsa::Signature) -> Result<<T::AuthorityId as RuntimeAppPublic>::Signature, ()>
{
    let voting_session_data = Summary::<T>::get_root_voting_session(&root_id).state();
    if voting_session_data.is_err() {
        debug::native::error!("💔 Error getting voting session data with root id {:?} to vote", &root_id);
        return Err(());
    }

    let voting_session_id = voting_session_data.expect("voting session data is ok").voting_session_id;
    let signature = this_validator.key.sign(
        &(
            CAST_VOTE_CONTEXT,
            voting_session_id,
            APPROVE_VOTE,
            eth_encoded_data.encode(),
            eth_signature.encode()
        ).encode()
    );

    if signature.is_none() {
        debug::native::error!("💔️ Error signing root id {:?} to vote", &root_id);
        return Err(());
    };

    return Ok(signature.expect("Signature is not empty if it gets here"));
}

fn send_reject_vote<T: Config>(
    root_id: &RootId::<T::BlockNumber>,
    this_validator: &Validator<<T as avn::Config>::AuthorityId, T::AccountId>) -> Result<(), ()>
{
    let voting_session_data = Summary::<T>::get_root_voting_session(&root_id).state();
    if voting_session_data.is_err() {
        debug::native::error!("💔 Error getting voting session data with root id {:?} to vote", &root_id);
        return Err(());
    }

    let voting_session_id = voting_session_data.expect("voting session data is ok").voting_session_id;
    let signature = this_validator.key.sign(
        &(
            CAST_VOTE_CONTEXT,
            voting_session_id,
            REJECT_VOTE
        ).encode()
    );

    if signature.is_none() {
        debug::native::error!("💔️ Error signing root id {:?} to vote", &root_id);
        return Err(());
    };

    debug::native::trace!(target: "avn", "🖊️  Worker sends reject vote for summary calculation: {:?}]", &root_id);

    if let Err(e) = SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(Call::reject_root(
                        root_id.clone(), this_validator.clone(), signature.expect("We have a signature")).into()
                    )
    {
        debug::native::error!("💔️ Error sending `reject vote transaction` for root id {:?}: {:?}", root_id, e);
        return Err(());
    }

    Ok(())
}
