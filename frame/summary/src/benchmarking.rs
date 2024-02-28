//! # Summary pallet
// Copyright 2020 Artos Systems (UK) Ltd.

//! summary pallet benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::{account, benchmarks};
use frame_system::{Module as System, EventRecord, RawOrigin};
use pallet_avn::{self as avn};
use hex_literal::hex;

pub type AVN<T> = avn::Module::<T>;
pub const ROOT_HASH_BYTES: [u8; 32] = [
    135, 54, 201, 230, 113, 254, 88, 31, 228, 239, 70, 49, 17, 32, 56, 41, 125, 205, 236, 174, 22,
    62, 135, 36, 194, 129, 236, 232, 173, 148, 200, 195,
];

fn setup_publish_root_voting<T: Config>(validators: Vec<Validator<<T as pallet_avn::Config>::AuthorityId, T::AccountId>>) -> (
    Validator<T::AuthorityId, T::AccountId>,
    RootId::<T::BlockNumber>,
    ecdsa::Signature,
    <T::AuthorityId as RuntimeAppPublic>::Signature,
    u32
) {
    let sender: Validator<T::AuthorityId, T::AccountId> = validators[validators.len() - (1 as usize)].clone();
    let root_id: RootId<T::BlockNumber> = RootId::new(RootRange::new(0u32.into(), 60u32.into()), 1);
    let approval_signature: ecdsa::Signature = ecdsa::Signature::from_slice(&hex!("3a0490e7d4325d3baa39b3011284e9758f9e370477e6b9e98713b2303da7427f71919f2757f62a01909391aeb3e89991539fdcb2d02ad45f7c64eb129c96f37100")).into();
    let signature: <T::AuthorityId as RuntimeAppPublic>::Signature = generate_signature::<T>();
    let quorum = setup_voting_session::<T>(&root_id);

    (sender, root_id, approval_signature, signature, quorum)
}

fn setup_voting_session<T: Config>(root_id: &RootId<T::BlockNumber>) -> u32 {
    PendingApproval::<T>::insert(
        root_id.range.clone(),
        root_id.ingress_counter
    );

    let quorum = calculate_two_third_quorum(AVN::<T>::validators().len() as u32);
    let voting_period_end = safe_add_block_numbers(
        <system::Module<T>>::block_number(),
        VotingPeriod::<T>::get()
    );
    let current_block_number: T::BlockNumber = 0u32.into();
    VotesRepository::<T>::insert(
        root_id,
        VotingSessionData::<T::AccountId, T::BlockNumber>::new(root_id.encode(), quorum, voting_period_end.expect("already checked"), current_block_number),
    );

    return quorum;
}

fn setup_approval_votes<T: Config>(
    validators: &Vec<Validator<<T as pallet_avn::Config>::AuthorityId, T::AccountId>>,
    number_of_votes: u32,
    root_id: &RootId<T::BlockNumber>
) {
    setup_votes::<T>(validators, number_of_votes, root_id, true);
}

fn setup_reject_votes<T: Config>(
    validators: &Vec<Validator<<T as pallet_avn::Config>::AuthorityId, T::AccountId>>,
    number_of_votes: u32,
    root_id: &RootId<T::BlockNumber>
) {
    setup_votes::<T>(validators, number_of_votes, root_id, false);
}

fn setup_votes<T: Config>(
    validators: &Vec<Validator<<T as pallet_avn::Config>::AuthorityId, T::AccountId>>,
    number_of_votes: u32,
    root_id: &RootId<T::BlockNumber>,
    is_approval: bool
) {
    for i in 0 .. validators.len() {
        if i < (number_of_votes as usize) {
            let approval_signature: ecdsa::Signature = generate_ecdsa_signature::<T>(validators[i].key.clone(), i as u64);
            match is_approval {
                true => VotesRepository::<T>::mutate(root_id, |vote| {
                    vote.ayes.push(validators[i].account_id.clone());
                    vote.confirmations.push(approval_signature.clone());
                }),
                false => VotesRepository::<T>::mutate(root_id, |vote|
                    vote.nays.push(validators[i].account_id.clone())
                )
            }
        }
    }
}

fn generate_ecdsa_signature<T: pallet_avn::Config>(key: <T as pallet_avn::Config>::AuthorityId, msg: u64) -> ecdsa::Signature {
    let sr25519_signature= key.sign(&msg.encode()).expect("able to make signature").encode();

    let mut signature_bytes: [u8; 65] = [0u8; 65];
    let start = if sr25519_signature.len() <= 65 { 65 - sr25519_signature.len() } else { 0 };
    signature_bytes[start..].copy_from_slice(&sr25519_signature);

    return ecdsa::Signature::from_slice(&signature_bytes);
}

fn advance_block<T: Config>(number: T::BlockNumber) {
	let now = System::<T>::block_number();
	System::<T>::set_block_number(now + number);
}

fn setup_validators<T: Config>(number_of_validator_account_ids: u32) -> Vec<Validator<<T as pallet_avn::Config>::AuthorityId, T::AccountId>>
{
    let mnemonic: &str = "basic anxiety marine match castle rival moral whisper insane away avoid bike";
    let mut validators: Vec<Validator<<T as pallet_avn::Config>::AuthorityId, T::AccountId>> = Vec::new();
    for i in 0..number_of_validator_account_ids {
        let account = account("dummy_validator", i, i);
        let key = <T as avn::Config>::AuthorityId::generate_pair(Some(mnemonic.as_bytes().to_vec()));
        validators.push(Validator::new(account, key));
    }

    // setup sender account id and key
    let sender_index = validators.len() - (1 as usize);
    let sender: Validator<T::AuthorityId, T::AccountId> = validators[sender_index].clone();
    let mut account_bytes: [u8; 32] = [0u8; 32];
    account_bytes.copy_from_slice(&hex!("be5ddb1579b72e84524fc29e78609e3caf42e85aa118ebfe0b0ad404b5bdd25f"));
    let account_id = T::AccountId::decode(&mut &account_bytes.encode()[..]).unwrap();
    validators[sender_index] = Validator::new(account_id, sender.key);

    // Setup validators in avn pallet
    avn::Validators::<T>::put(validators.clone());

    return validators;
}

fn setup_roots<T: Config>(number_of_roots: u32, account_id: T::AccountId, start_ingress_counter: IngressCounter) {
    for i in 0 .. number_of_roots + 1 {
        Roots::<T>::insert(
            RootRange::new(0u32.into(), 60u32.into()),
            start_ingress_counter + i as IngressCounter,
            RootData::new(H256::from([0u8; 32]), account_id.clone(), None)
        );
    }
}

fn setup_record_summary_calculation<T: Config>()-> (
    T::BlockNumber,
    H256,
    IngressCounter,
    <<T as avn::Config>::AuthorityId as RuntimeAppPublic>::Signature
) {
    let new_block_number: T::BlockNumber = SchedulePeriod::<T>::get();
    let root_hash = H256::from(ROOT_HASH_BYTES);
    let ingress_counter: IngressCounter = 100u64.into();
    TotalIngresses::put(ingress_counter - 1);

    let signature: <T::AuthorityId as RuntimeAppPublic>::Signature = generate_signature::<T>();

    (new_block_number, root_hash, ingress_counter, signature)
}

fn generate_signature<T: pallet_avn::Config>() -> <<T as avn::Config>::AuthorityId as RuntimeAppPublic>::Signature {
    let encoded_data = 0.encode();
    let authority_id = T::AuthorityId::generate_pair(None);
    let signature = authority_id.sign(&encoded_data).expect("able to make signature");
    return signature;
}

fn assert_last_event<T: Config>(generic_event: <T as Config>::Event) {
    assert_last_nth_event::<T>(generic_event, 1);
}

fn assert_last_nth_event<T: Config>(generic_event: <T as Config>::Event, n: u32) {
    let events = frame_system::Module::<T>::events();
    let system_event: <T as frame_system::Config>::Event = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len().saturating_sub(n as usize)];
    assert_eq!(event, &system_event);
}

benchmarks! {
    set_periods {
        let new_schedule_period: T::BlockNumber = 200u32.into();
        let new_voting_period: T::BlockNumber = 150u32.into();
    }: _(RawOrigin::Root, new_schedule_period, new_voting_period)
    verify {
        assert_eq!(SchedulePeriod::<T>::get(), new_schedule_period);
        assert_eq!(VotingPeriod::<T>::get(), new_voting_period);
    }

    record_summary_calculation {
        let v in 3 .. MAX_VALIDATOR_ACCOUNT_IDS;
        let r in 1 .. MAX_NUMBER_OF_ROOT_DATA_PER_RANGE;

        let validators = setup_validators::<T>(v);
        let validator = validators[validators.len() - (1 as usize)].clone();
        let (new_block_number, root_hash, ingress_counter, signature) = setup_record_summary_calculation::<T>();
        setup_roots::<T>(r, validator.account_id.clone(), ingress_counter);
        let next_block_to_process = NextBlockToProcess::<T>::get();
    }: _(RawOrigin::None, new_block_number, root_hash, ingress_counter, validator.clone(), signature)
    verify {
        let range = RootRange::new(next_block_to_process, new_block_number);
        let root = Roots::<T>::get(range, ingress_counter);

        assert_eq!(TotalIngresses::get(), ingress_counter);
        assert!(PendingApproval::<T>::contains_key(range));
        assert_eq!(true, VotesRepository::<T>::contains_key(RootId::new(range, ingress_counter)));
        assert_last_event::<T>(RawEvent::SummaryCalculated(
            next_block_to_process,
            new_block_number,
            root_hash,
            validator.account_id
        ).into());
    }

    approve_root_with_end_voting {
        let v in 60 .. MAX_VALIDATOR_ACCOUNT_IDS;
        let o in 1 .. MAX_OFFENDERS;

        let mut validators = setup_validators::<T>(v);
        let (sender, root_id, approval_signature, signature, quorum) = setup_publish_root_voting::<T>(validators.clone());
        validators.remove(validators.len() - (1 as usize)); // Avoid setting up sender to approve vote automatically

        setup_roots::<T>(1, sender.account_id.clone(), root_id.ingress_counter);

        // Setup votes more than quorum to trigger end voting period
        let number_of_votes = quorum;
        setup_approval_votes::<T>(&validators, number_of_votes, &root_id);

        let mut reject_voters = validators.clone();
        reject_voters.reverse();
        setup_reject_votes::<T>(&reject_voters, o, &root_id);

        CurrentSlot::<T>::put::<T::BlockNumber>(3u32.into());
    }: approve_root(RawOrigin::None, root_id, sender.clone(), approval_signature.clone(), signature)
    verify {
        let vote = VotesRepository::<T>::get(&root_id);
        assert_eq!(true, vote.ayes.contains(&sender.account_id));
        assert_eq!(true, vote.confirmations.contains(&approval_signature));

        assert_eq!(true, NextBlockToProcess::<T>::get() == root_id.range.to_block + 1u32.into());
        assert_eq!(true, Roots::<T>::get(root_id.range, root_id.ingress_counter).is_validated);
        assert_eq!(true, SlotOfLastPublishedSummary::<T>::get() == CurrentSlot::<T>::get());
        assert_eq!(false, PendingApproval::<T>::contains_key(&root_id.range));

        // TODO: Fix error executing runtime benchmark
        // let vote = VotesRepository::<T>::get(&root_id);
        // assert_last_nth_event::<T>(RawEvent::SummaryOffenceReported(
        //         SummaryOffenceType::RejectedValidRoot,
        //         create_offenders_identification::<T>(&vote.nays)
        //     ).into(),
        //     3
        // );

        assert_last_nth_event::<T>(
            RawEvent::VotingEnded(
                root_id.clone(),
                true
            ).into(),
            2
        );

        assert_last_event::<T>(RawEvent::VoteAdded(
                sender.account_id.clone(),
                root_id,
                true
            ).into()
        );
    }

    approve_root_without_end_voting {
        let v in 3 .. MAX_VALIDATOR_ACCOUNT_IDS;

        let validators = setup_validators::<T>(v);
        let (sender, root_id, approval_signature, signature, quorum) = setup_publish_root_voting::<T>(validators.clone());
        setup_roots::<T>(1, sender.account_id.clone(), root_id.ingress_counter - 1);

        CurrentSlot::<T>::put::<T::BlockNumber>(3u32.into());
    }: approve_root(RawOrigin::None, root_id, sender.clone(), approval_signature.clone(), signature)
    verify {
        let vote = VotesRepository::<T>::get(&root_id);
        assert_eq!(true, vote.ayes.contains(&sender.account_id));
        assert_eq!(true, vote.confirmations.contains(&approval_signature));

        assert_eq!(false, NextBlockToProcess::<T>::get() == root_id.range.to_block + 1u32.into());
        assert_eq!(false, Roots::<T>::get(root_id.range, root_id.ingress_counter).is_validated);
        assert_eq!(false, SlotOfLastPublishedSummary::<T>::get() == CurrentSlot::<T>::get());
        assert_eq!(true, PendingApproval::<T>::contains_key(&root_id.range));

        assert_last_event::<T>(RawEvent::VoteAdded(
            sender.account_id,
            root_id.clone(),
            true
        ).into());
    }

    reject_root_with_end_voting {
        let v in 60 .. MAX_VALIDATOR_ACCOUNT_IDS;
        let o in 1 .. MAX_OFFENDERS;

        let mut validators = setup_validators::<T>(v);
        let (sender, root_id, _, signature, quorum) = setup_publish_root_voting::<T>(validators.clone());
        validators.remove(validators.len() - (1 as usize)); // Avoid setting up sender to reject vote automatically

        setup_roots::<T>(1, sender.account_id.clone(), root_id.ingress_counter);

        // Setup votes more than quorum to trigger end voting period
        let reject_voters = quorum;
        setup_reject_votes::<T>(&validators, reject_voters, &root_id);

        let mut approve_voters = validators.clone();
        approve_voters.reverse();
        setup_approval_votes::<T>(&approve_voters, o, &root_id);
    }: reject_root(RawOrigin::None, root_id.clone(), sender.clone(), signature)
    verify {
        assert_eq!(false, NextBlockToProcess::<T>::get() == root_id.range.to_block + 1u32.into());
        assert_eq!(false, Roots::<T>::get(root_id.range, root_id.ingress_counter).is_validated);
        assert_eq!(false, SlotOfLastPublishedSummary::<T>::get() == CurrentSlot::<T>::get() + 1u32.into());

        assert_eq!(false, PendingApproval::<T>::contains_key(&root_id.range));

        // TODO: Fix error executing runtime benchmark
        // let root_data = Roots::<T>::get(root_id.range, root_id.ingress_counter);
        // assert_last_nth_event::<T>(RawEvent::SummaryOffenceReported(
        //         SummaryOffenceType::CreatedInvalidRoot,
        //         create_offenders_identification::<T>(&vec![root_data.added_by])
        //     ).into(),
        //     4
        // );

        // TODO: Fix error executing runtime benchmark
        // let vote = VotesRepository::<T>::get(&root_id);
        // assert_last_nth_event::<T>(RawEvent::SummaryOffenceReported(
        //         SummaryOffenceType::ApprovedInvalidRoot,
        //         create_offenders_identification::<T>(&vote.ayes)
        //     ).into(),
        //     3
        // );

        assert_last_nth_event::<T>(
            RawEvent::VotingEnded(
                root_id.clone(),
                false
            ).into(),
            2
        );

        assert_last_event::<T>(RawEvent::VoteAdded(
            sender.account_id,
            root_id.clone(),
            false
        ).into());
    }

    reject_root_without_end_voting {
        let v in 3 .. MAX_VALIDATOR_ACCOUNT_IDS;

        let mut validators = setup_validators::<T>(v);
        let (sender, root_id, _, signature, quorum) = setup_publish_root_voting::<T>(validators.clone());
        validators.remove(validators.len() - (1 as usize)); // Avoid setting up sender to reject vote automatically

        setup_roots::<T>(1, sender.account_id.clone(), root_id.ingress_counter);
    }: reject_root(RawOrigin::None, root_id.clone(), sender.clone(), signature)
    verify {
        assert_eq!(false, NextBlockToProcess::<T>::get() == root_id.range.to_block + 1u32.into());
        assert_eq!(false, Roots::<T>::get(root_id.range, root_id.ingress_counter).is_validated);
        assert_eq!(false, SlotOfLastPublishedSummary::<T>::get() == CurrentSlot::<T>::get() + 1u32.into());

        assert_eq!(true, PendingApproval::<T>::contains_key(&root_id.range));

        assert_last_event::<T>(RawEvent::VoteAdded(
            sender.account_id,
            root_id.clone(),
            false
        ).into());
    }

    end_voting_period_with_rejected_valid_votes {
        let o in 1 .. MAX_OFFENDERS;

        let number_of_validators = MAX_VALIDATOR_ACCOUNT_IDS;
        let validators = setup_validators::<T>(number_of_validators);
        let (sender, root_id, _, signature, quorum) = setup_publish_root_voting::<T>(validators.clone());
        setup_roots::<T>(1, sender.account_id.clone(), root_id.ingress_counter);

        let current_slot_number: T::BlockNumber = 3u32.into();
        CurrentSlot::<T>::put(current_slot_number);

        // Setup votes more than quorum to trigger end voting period
        let number_of_approval_votes = quorum;
        setup_approval_votes::<T>(&validators, number_of_approval_votes, &root_id);

        // setup offenders votes
        let (_, offenders) = validators.split_at(quorum as usize);
        let number_of_reject_votes = o;
        setup_reject_votes::<T>(&offenders.to_vec(), number_of_reject_votes, &root_id);
    }: end_voting_period(RawOrigin::None, root_id.clone(), sender.clone(), signature)
    verify {
        assert_eq!(true, NextBlockToProcess::<T>::get() == root_id.range.to_block + 1u32.into());
        assert_eq!(true, Roots::<T>::get(root_id.range, root_id.ingress_counter).is_validated);
        assert_eq!(true, SlotOfLastPublishedSummary::<T>::get() == CurrentSlot::<T>::get());
        assert_eq!(false, PendingApproval::<T>::contains_key(&root_id.range));

        // TODO: Fix error executing runtime benchmark
        // let vote = VotesRepository::<T>::get(&root_id);
        // assert_last_nth_event::<T>(RawEvent::SummaryOffenceReported(
        //         SummaryOffenceType::RejectedValidRoot,
        //         create_offenders_identification::<T>(&vote.nays)
        //     ).into(),
        //     2
        // );

        assert_last_event::<T>(
            RawEvent::VotingEnded(
                root_id.clone(),
                true
            ).into()
        );
    }

    end_voting_period_with_approved_invalid_votes {
        let o in 1 .. MAX_OFFENDERS;

        let number_of_validators = MAX_VALIDATOR_ACCOUNT_IDS;
        let validators = setup_validators::<T>(number_of_validators);
        let (sender, root_id, _, signature, quorum) = setup_publish_root_voting::<T>(validators.clone());
        setup_roots::<T>(1, sender.account_id.clone(), root_id.ingress_counter);

        let current_slot_number: T::BlockNumber = 3u32.into();
        CurrentSlot::<T>::put(current_slot_number);

        // Setup votes more than quorum to trigger end voting period
        let number_of_reject_votes = quorum;
        setup_reject_votes::<T>(&validators, number_of_reject_votes, &root_id);

        // setup offenders votes
        let (_, offenders) = validators.split_at(quorum as usize);
        let number_of_approval_votes = o;
        setup_approval_votes::<T>(&offenders.to_vec(), number_of_approval_votes, &root_id);
    }: end_voting_period(RawOrigin::None, root_id.clone(), sender.clone(), signature)
    verify {
        assert_eq!(false, NextBlockToProcess::<T>::get() == root_id.range.to_block + 1u32.into());
        assert_eq!(false, Roots::<T>::get(root_id.range, root_id.ingress_counter).is_validated);
        assert_eq!(false, SlotOfLastPublishedSummary::<T>::get() == CurrentSlot::<T>::get());
        assert_eq!(false, PendingApproval::<T>::contains_key(&root_id.range));

        // TODO: Fix error executing runtime benchmark
        // let vote = VotesRepository::<T>::get(&root_id);
        // assert_last_nth_event::<T>(RawEvent::SummaryOffenceReported(
        //         SummaryOffenceType::RejectedValidRoot,
        //         create_offenders_identification::<T>(&vote.ayes)
        //     ).into(),
        //     2
        // );

        assert_last_event::<T>(
            RawEvent::VotingEnded(
                root_id.clone(),
                false
            ).into()
        );
    }

    advance_slot_with_offence {
        let number_of_validators = MAX_VALIDATOR_ACCOUNT_IDS;
        let validators = setup_validators::<T>(number_of_validators);
        let (sender, _, _, signature, quorum) = setup_publish_root_voting::<T>(validators.clone());

        advance_block::<T>(SchedulePeriod::<T>::get());
        CurrentSlotsValidator::<T>::put(sender.account_id.clone());

        // Create an offence: last published summary slot number < current slot number
        let old_slot_number: T::BlockNumber = 2u32.into();
        CurrentSlot::<T>::put(old_slot_number);

        let last_summary_slot: T::BlockNumber = 1u32.into();
        SlotOfLastPublishedSummary::<T>::put(last_summary_slot);

        let old_new_slot_start = NextSlotAtBlock::<T>::get();
    }: advance_slot(RawOrigin::None, sender.clone(), signature)
    verify {
        let new_slot_number = CurrentSlot::<T>::get();
        let new_validator = CurrentSlotsValidator::<T>::get();
        let new_slot_start = NextSlotAtBlock::<T>::get();

        assert_eq!(new_slot_number, old_slot_number + 1u32.into());
        assert_eq!(false, new_validator == sender.account_id.clone());
        assert_last_event::<T>(RawEvent::SlotAdvanced(
            sender.account_id.clone(),
            new_slot_number,
            new_validator,
            new_slot_start
        ).into());

        assert_last_nth_event::<T>(
            RawEvent::SummaryNotPublishedOffence(
                sender.account_id.clone(),
                old_slot_number,
                last_summary_slot,
                old_new_slot_start
            ).into(),
            2
        );

        // TODO: assert_emitted_event_for_offence_of_type(SummaryOffenceType::SlotNotAdvanced);
    }

    advance_slot_without_offence {
        let number_of_validators = MAX_VALIDATOR_ACCOUNT_IDS;
        let validators = setup_validators::<T>(number_of_validators);
        let (sender, _, _, signature, _) = setup_publish_root_voting::<T>(validators.clone());

        advance_block::<T>(SchedulePeriod::<T>::get());
        CurrentSlotsValidator::<T>::put(sender.account_id.clone());

        let old_slot_number = CurrentSlot::<T>::get();
    }: advance_slot(RawOrigin::None, sender.clone(), signature)
    verify {
        let new_slot_number = CurrentSlot::<T>::get();
        let new_validator = CurrentSlotsValidator::<T>::get();
        let new_slot_start = NextSlotAtBlock::<T>::get();

        assert_eq!(new_slot_number, old_slot_number + 1u32.into());
        assert_eq!(false, new_validator == sender.account_id.clone());
        assert_last_event::<T>(RawEvent::SlotAdvanced(
            sender.account_id,
            new_slot_number,
            new_validator,
            new_slot_start
        ).into());
    }

    add_challenge {
        let number_of_validators = 4;
        let validators = setup_validators::<T>(number_of_validators);
        let (sender, _, _, signature, _) = setup_publish_root_voting::<T>(validators.clone());

        let current_block_number = SchedulePeriod::<T>::get() + T::MinBlockAge::get();
        let next_slot_at_block: T::BlockNumber = current_block_number - T::AdvanceSlotGracePeriod::get() - 1u32.into();
        let current_slot_number: T::BlockNumber = 3u32.into();
        let slot_number_to_challenge_as_u32: u32 = AVN::<T>::convert_block_number_to_u32(current_slot_number).expect("valid u32 value");

        advance_block::<T>(current_block_number);
        NextSlotAtBlock::<T>::put(next_slot_at_block);
        CurrentSlot::<T>::put(current_slot_number);
        SlotOfLastPublishedSummary::<T>::put(current_slot_number - 1u32.into());
        CurrentSlotsValidator::<T>::put(validators[1].account_id.clone());

        let challenge: SummaryChallenge<T::AccountId> = SummaryChallenge {
            challenge_reason: SummaryChallengeReason::SlotNotAdvanced(slot_number_to_challenge_as_u32),
            challenger: sender.account_id.clone(),
            challengee: validators[1].account_id.clone()
        };
    }: _(RawOrigin::None, challenge.clone(), sender.clone(), signature)
    verify {
        let new_slot_number = CurrentSlot::<T>::get();
        let new_validator = CurrentSlotsValidator::<T>::get();
        let new_slot_start = NextSlotAtBlock::<T>::get();

        assert_eq!(new_slot_number, current_slot_number + 1u32.into());

        // TODO: Fix error executing runtime benchmark
        // assert_last_nth_event::<T>(RawEvent::SummaryOffenceReported(
        //         SummaryOffenceType::SlotNotAdvanced,
        //         create_offenders_identification::<T>(&vec![validators[1].account_id.clone()])
        //     ).into(),
        //     5
        // );

        // TODO: Fix error executing runtime benchmark
        // assert_last_nth_event::<T>(RawEvent::SummaryOffenceReported(
        //         SummaryOffenceType::NoSummaryCreated,
        //         create_offenders_identification::<T>(&vec![validators[1].account_id.clone()])
        //     ).into(),
        //     4
        // );

        assert_last_nth_event::<T>(RawEvent::SummaryNotPublishedOffence(
                validators[1].account_id.clone(),
                current_slot_number,
                current_slot_number - 1u32.into(),
                next_slot_at_block
            ).into(),
            3
        );

        assert_last_nth_event::<T>(RawEvent::SlotAdvanced(
            sender.account_id,
            new_slot_number,
            new_validator,
            new_slot_start
            ).into(),
            2
        );

        assert_last_event::<T>(
            RawEvent::ChallengeAdded(
                challenge.challenge_reason.clone(),
                challenge.challenger,
                challenge.challengee
            ).into()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::*;
    use frame_support::assert_ok;
    use crate::extension_builder::ExtBuilder;

    #[test]
    fn benchmarks() {
        let mut ext = ExtBuilder::build_default().as_externality();

        ext.execute_with(|| {
            assert_ok!(test_benchmark_set_periods::<TestRuntime>());
            assert_ok!(test_benchmark_record_summary_calculation::<TestRuntime>());
            assert_ok!(test_benchmark_end_voting_period_with_rejected_valid_votes::<TestRuntime>());
            assert_ok!(test_benchmark_end_voting_period_with_approved_invalid_votes::<TestRuntime>());
            assert_ok!(test_benchmark_advance_slot_with_offence::<TestRuntime>());
            assert_ok!(test_benchmark_advance_slot_without_offence::<TestRuntime>());

            // TODO: SYS-1976 Fix 'InvalidECDSASignature'
            // assert_ok!(test_benchmark_approve_root_with_end_voting::<TestRuntime>());
            // assert_ok!(test_benchmark_approve_root_without_end_voting::<TestRuntime>());

            // TODO: SYS-1977 Fix 'InvalidVote'
            // assert_ok!(test_benchmark_reject_root_with_end_voting::<TestRuntime>());
            // assert_ok!(test_benchmark_reject_root_without_end_voting::<TestRuntime>());

            // TODO: SYS-1978 Fix `attempt to subtract with overflow`
            // assert_ok!(test_benchmark_add_challenge::<TestRuntime>());
        });
    }
}