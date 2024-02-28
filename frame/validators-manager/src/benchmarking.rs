//! # Validators Manager pallet
// Copyright 2020 Artos Systems (UK) Ltd.

// validators manager pallet benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::benchmark_utils::*;
use crate::Module as ValidatorsManager;
use frame_benchmarking::{account, benchmarks, whitelist_account};
use frame_system::{EventRecord, RawOrigin};
use sp_core::H256;
use hex_literal::hex;

pub type Staking<T> = pallet_staking::Module::<T>;
pub const AMOUNT: u128 = 10_000_000_000_000_000_000_000u128;
pub const SEED: u32 = 0;

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

fn get_signed_bond_proof<T: Config>(signer: &T::AccountId) -> Proof<T::Signature, T::AccountId> {
    // Signatures are generated using the script in `scripts/benchmarking/sign_staking_signature.js`.
    #[cfg(test)]
    let signature = hex!("3455fb403424e5523ed076a8eb6e6c4e7d63567579e93d06287f36f9301dcc15651083ea2dca212e16c1296247132b4003c5510952b1f86cf241544d619c1489");
    #[cfg(not(test))]
    let signature = hex!("3e4ef30ba23f9501773be344baa18a1a4297a6262c05b7cd7e2c16421ed8df6ec921581ed871c59023331f51e1fdfe6691789ee9f19375340b0c0eefbd893582");

    return get_proof::<T>(signer, &signature);
}

fn get_signed_bond_extra_proof<T: Config>(signer: &T::AccountId) -> Proof<T::Signature, T::AccountId> {
    // Signature is generated using the script in `scripts/benchmarking/sign_staking_signature.js`.
    let signature = hex!("22c095f95372f10da3dffe9c14f1169a7e3b86d6e2be333bcb26b99fa36f3b1f3b79ea9546f6ca6c90f924094d093af53a6177506a3292d6a0199220b791358e");
    return get_proof::<T>(signer, &signature);
}

fn get_signed_unbond_proof<T: Config>(signer: &T::AccountId) -> Proof<T::Signature, T::AccountId> {
    // Signature is generated using the script in `scripts/benchmarking/sign_staking_signature.js`.
    let signature = hex!("9a1b95f2cdf6048012486093a4ca280b5fab1762152419273c01df431b9ae223c1050281d9dc5b48f93f629ec2e107f743ef1a3a0fcf386a5430b5746607e483");
    return get_proof::<T>(signer, &signature);
}

fn get_set_payee_proof<T: Config>(signer: &T::AccountId) -> Proof<T::Signature, T::AccountId> {
    // Signature is generated using the script in `scripts/benchmarking/sign_staking_signature.js`.
    let signature = hex!("08309990d74e69053939f9c1340594c8e89992d6430c1a6fef53991691e5b526c7ef78d0aa3803a148bef09b21278d2ba27f769de457e0e0f302e207325b4b80");
    return get_proof::<T>(signer, &signature);
}

fn get_payout_staker_proof<T: Config>(signer: &T::AccountId) -> Proof<T::Signature, T::AccountId> {
    // Signatures are generated using the script in `scripts/benchmarking/sign_staking_signature.js`.
    #[cfg(test)]
    let signature = hex!("3089536a44455f454f921d95bdb4213efd84ce5187d76734028fc026cd312f5c3416cdbced29407b68950fe30b8670a4f6d9f8cd9a3df45263783414d1717580");
    #[cfg(not(test))]
    let signature = hex!("fc9b96f7886ba85c7c68c50d65e33174c911711556e7ec20d8ea8cbd5b02f77dc0ffc2c3fab3e69d7ff78409ba63964182f37f8c3d228e123d14725effca6287");

    return get_proof::<T>(signer, &signature);
}

fn get_proof<T: Config>(signer: &T::AccountId, signature: &[u8]) -> Proof<T::Signature, T::AccountId> {
    let relayer = create_funded_relayer::<T>();
    return Proof {
        signer: signer.clone(),
        relayer,
        signature: sp_core::sr25519::Signature::from_slice(signature).into()
    };
}

// Create a funded user
pub fn create_funded_staker<T: Config>() -> T::AccountId {
    // This is generated from scripts/benchmarking/sign_staking_signature.js
    let stash_account_raw: H256 = H256(hex!("482eae97356cdfd3b12774db1e5950471504d28b89aa169179d6c0527a04de23"));
    let staker = T::AccountId::decode(&mut stash_account_raw.as_bytes()).expect("valid account id");

    fund_account::<T>(&staker);
    return staker;
}

pub fn create_funded_relayer<T: Config>() -> T::AccountId {
    // This is generated from scripts/benchmarking/sign_staking_signature.js
    let controller_account_raw: H256 = H256(hex!("ba7e4480c1cdd7bfd51faa3c98228b0d5dead1c0cff148be5645fe8207d94c17"));
    let relayer = T::AccountId::decode(&mut controller_account_raw.as_bytes()).expect("valid account id");

    fund_account::<T>(&relayer);
    return relayer;
}

pub fn create_funded_user<T: Config>(string: &'static str, n: u32) -> T::AccountId {
    let user = account(string, n, SEED);
    fund_account::<T>(&user);
    return user;
}

pub fn fund_account<T: Config>(account: &T::AccountId) {
    if let Ok(amount) = get_amount::<T>() {
        T::Currency::make_free_balance_be(account, amount);
        T::Currency::issue(amount);
    }
}

pub fn get_amount<T: Config>() -> Result<BalanceOf<T>, &'static str> {
    return <BalanceOf<T> as TryFrom<u128>>::try_from(AMOUNT).or_else(|_| Err("Error converting amount"));
}

// Create a stash and controller pair.
pub fn create_bonded_staker<T: Config>(destination: RewardDestination<T::AccountId>)
    -> Result<(T::AccountId, T::AccountId), &'static str>
{
    let staker = create_funded_staker::<T>();
    let amount = get_amount::<T>()?;
    return bond_funds::<T>(staker.clone(), staker, amount / 2u32.into(), destination);
}

pub fn create_bonded_user<T: Config>(n: u32, destination: RewardDestination<T::AccountId>)
    -> Result<(T::AccountId, T::AccountId), &'static str>
{
    let stash = create_funded_user::<T>("stash", n);
    let controller = create_funded_user::<T>("controller", n);
    let amount = get_amount::<T>()?;
    return bond_funds::<T>(stash, controller, amount / 2u32.into(), destination);
}

fn bond_funds<T: Config>(stash: T::AccountId, controller: T::AccountId, amount: BalanceOf<T>, destination: RewardDestination<T::AccountId>)
    -> Result<(T::AccountId, T::AccountId), &'static str>
{
    let controller_lookup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(controller.clone());
    Staking::<T>::bond(RawOrigin::Signed(stash.clone()).into(), controller_lookup, amount, destination)?;
    return Ok((stash, controller))
}

pub fn create_stash_and_dead_controller<T: Config>(n: u32, destination: RewardDestination<T::AccountId>,)
    -> Result<(T::AccountId, T::AccountId), &'static str>
{
    let stash = create_funded_user::<T>("stash", n);
    // controller has no funds
    let controller: T::AccountId = account("controller", n, SEED);
    let amount = get_amount::<T>()?;
    return bond_funds::<T>(stash, controller, amount / 2u32.into(), destination);
}

benchmarks! {
    add_validator {
        let validators = setup_validators::<T>(DEFAULT_MINIMUM_VALIDATORS_COUNT as u32);
        let new_validator: T::AccountId = account("dummy_new_validator", 0, 0);
        let new_validator_lookup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(new_validator.clone());

        let validator_eth_public_key: ecdsa::Public = ecdsa::Public::default();
        let preferences = ValidatorPrefs {
            commission: MaxCommission::get(),
            blocked: false
        };

        let bond_value = MinValidatorBond::<T>::get();
        T::Currency::make_free_balance_be(&new_validator, bond_value);
        let _ = ValidatorsManager::<T>::bond(
            RawOrigin::Signed(new_validator.clone()).into(),
            new_validator_lookup,
            bond_value,
            RewardDestination::Stash
        );
        let controller_account_id = T::AccountId::decode(&mut &new_validator.encode()[..]).unwrap();
    }: _(RawOrigin::Root, controller_account_id, validator_eth_public_key.clone(), preferences.clone())
    verify {
        assert_eq!(true, ValidatorAccountIds::<T>::get().unwrap().contains(&new_validator));
        assert_eq!(EthereumPublicKeys::<T>::get(validator_eth_public_key.clone()), new_validator);
    }

    remove_validator {
        let v in (DEFAULT_MINIMUM_VALIDATORS_COUNT as u32 + 1) .. MAX_VALIDATOR_ACCOUNT_IDS;

        let validators = setup_validators::<T>(v);
        let validator_to_remove: T::AccountId = account("dummy_validator", 0, 0);
        let validator_to_remove_lookup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(validator_to_remove.clone());

        let bond_value = MinValidatorBond::<T>::get();
        T::Currency::make_free_balance_be(&validator_to_remove, bond_value);
        let _ = ValidatorsManager::<T>::bond(
            RawOrigin::Signed(validator_to_remove.clone()).into(),
            validator_to_remove_lookup,
            bond_value,
            RewardDestination::Stash
        );
        let controller_account_id = T::AccountId::decode(&mut &validator_to_remove.encode()[..]).unwrap();
    }: _(RawOrigin::Root, controller_account_id.clone())
    verify {
        assert_eq!(ValidatorAccountIds::<T>::get().unwrap().iter().position(|validator_account_id| *validator_account_id == controller_account_id), None);
        assert_last_event::<T>(RawEvent::ValidatorDeregistered(controller_account_id.clone()).into());
        assert_eq!(true, ValidatorActions::<T>::contains_key(controller_account_id, TotalIngresses::get()));
    }

    approve_action_with_end_voting {
        let v in (DEFAULT_MINIMUM_VALIDATORS_COUNT as u32 + 1) .. MAX_VALIDATOR_ACCOUNT_IDS;

        let mut validators = setup_validators::<T>(v);
        let (sender, action_id, approval_signature, signature, quorum) = setup_action_voting::<T>(validators.clone());
        validators.remove(validators.len() - (1 as usize)); // Avoid setting up sender to approve vote automatically

        // Setup votes more than quorum to trigger end voting period
        let number_of_votes = quorum;
        setup_approval_votes::<T>(&validators, number_of_votes, &action_id);
    }: approve_validator_action(RawOrigin::None, action_id.clone(), sender.clone(), approval_signature.clone(), signature)
    verify {
        // Approve vote is added
        assert_eq!(true, VotesRepository::<T>::get(action_id.clone()).ayes.contains(&sender.account_id.clone()));
        assert_eq!(true, VotesRepository::<T>::get(action_id.clone()).confirmations.contains(&approval_signature));

        // Voting period is ended
        assert_eq!(ValidatorActions::<T>::get(&action_id.action_account_id.clone(), action_id.ingress_counter).status, ValidatorsActionStatus::Actioned);
        assert_eq!(false, PendingApprovals::<T>::contains_key(&action_id.action_account_id.clone()));

        // Events are emitted
        assert_last_nth_event::<T>(
            RawEvent::VotingEnded(
                action_id.clone(),
                (Box::new(
                    ValidatorManagementVotingSession::<T>::new(&action_id.clone())
                ) as Box<dyn VotingSessionManager<T::AccountId, T::BlockNumber>>).state()?.is_approved()
            ).into(),
            2
        );
        assert_last_event::<T>(RawEvent::VoteAdded(sender.account_id, action_id.clone(), APPROVE_VOTE).into());
    }

    approve_action_without_end_voting {
        let v in (DEFAULT_MINIMUM_VALIDATORS_COUNT as u32 + 1) .. MAX_VALIDATOR_ACCOUNT_IDS;
        let validators = setup_validators::<T>(v);
        let (sender, action_id, approval_signature, signature, _) = setup_action_voting::<T>(validators);
    }: approve_validator_action(RawOrigin::None, action_id.clone(), sender.clone(), approval_signature.clone(), signature)
    verify {
        // Approve vote is added
        assert_eq!(true, VotesRepository::<T>::get(action_id.clone()).ayes.contains(&sender.account_id.clone()));
        assert_eq!(true, VotesRepository::<T>::get(action_id.clone()).confirmations.contains(&approval_signature));

        // Voting period is not ended
        assert_eq!(ValidatorActions::<T>::get(&action_id.action_account_id.clone(), action_id.ingress_counter).status, ValidatorsActionStatus::AwaitingConfirmation);
        assert_eq!(true, PendingApprovals::<T>::contains_key(&action_id.action_account_id.clone()));

        // Event is emitted
        assert_last_event::<T>(RawEvent::VoteAdded(sender.account_id, action_id.clone(), APPROVE_VOTE).into());
    }

    reject_action_with_end_voting {
        let v in (DEFAULT_MINIMUM_VALIDATORS_COUNT as u32 + 1) .. MAX_VALIDATOR_ACCOUNT_IDS;

        let mut validators = setup_validators::<T>(v);
        let (sender, action_id, _, signature, quorum) = setup_action_voting::<T>(validators.clone());
        validators.remove(validators.len() - (1 as usize)); // Avoid setting up sender to reject vote automatically

        // Setup votes more than quorum to trigger end voting period
        let number_of_votes = quorum;
        setup_reject_votes::<T>(&validators, number_of_votes, &action_id);
    }: reject_validator_action(RawOrigin::None, action_id.clone(), sender.clone(), signature)
    verify {
        // Reject vote is added
        assert_eq!(true, VotesRepository::<T>::get(action_id.clone()).nays.contains(&sender.account_id.clone()));

        // Voting period is ended, but deregistration is not actioned
        assert_eq!(
            ValidatorActions::<T>::get(
                &action_id.action_account_id.clone(),
                action_id.ingress_counter
            ).status,
            ValidatorsActionStatus::AwaitingConfirmation);
        assert_eq!(false, PendingApprovals::<T>::contains_key(&action_id.action_account_id.clone()));

        // Events are emitted
        assert_last_nth_event::<T>(
            RawEvent::VotingEnded(
                action_id.clone(),
                (Box::new(
                    ValidatorManagementVotingSession::<T>::new(&action_id.clone())
                ) as Box<dyn VotingSessionManager<T::AccountId, T::BlockNumber>>).state()?.is_approved()
            ).into(),
            2
        );
        assert_last_event::<T>(RawEvent::VoteAdded(sender.account_id, action_id.clone(), REJECT_VOTE).into());
    }

    reject_action_without_end_voting {
        let v in (DEFAULT_MINIMUM_VALIDATORS_COUNT as u32 + 1) .. MAX_VALIDATOR_ACCOUNT_IDS;

        let validators = setup_validators::<T>(v);
        let (sender, action_id, _, signature, _) = setup_action_voting::<T>(validators);
    }: reject_validator_action(RawOrigin::None, action_id.clone(), sender.clone(), signature)
    verify {
        // Reject vote is added
        assert_eq!(true, VotesRepository::<T>::get(action_id.clone()).nays.contains(&sender.account_id.clone()));

        // Voting period is not ended
        assert_eq!(
            ValidatorActions::<T>::get(
                &action_id.action_account_id.clone(),
                action_id.ingress_counter
            ).status,
            ValidatorsActionStatus::AwaitingConfirmation
        );
        assert_eq!(true, PendingApprovals::<T>::contains_key(&action_id.action_account_id.clone()));

        // Event is emitted
        assert_last_event::<T>(RawEvent::VoteAdded(sender.account_id, action_id.clone(), REJECT_VOTE).into());
    }

    end_voting_period_with_rejected_valid_actions {
        let o in 1 .. MAX_OFFENDERS; // maximum of offenders need to be less one third of minimum validators so the benchmark won't panic

        let number_of_validators = MAX_VALIDATOR_ACCOUNT_IDS;
        let validators = setup_validators::<T>(number_of_validators);
        let (sender, action_id, _, signature, quorum) = setup_action_voting::<T>(validators.clone());

        // Setup votes more than quorum to trigger end voting period
        let number_of_approval_votes = quorum;
        setup_approval_votes::<T>(&validators, number_of_approval_votes, &action_id);

        // setup offenders votes
        let (_, offenders) = validators.split_at(quorum as usize);
        let number_of_reject_votes = o;
        setup_reject_votes::<T>(&offenders.to_vec(), number_of_reject_votes, &action_id);
    }: end_voting_period(RawOrigin::None, action_id.clone(), sender.clone(), signature)
    verify {
        // Voting period is ended, and deregistration is actioned
        assert_eq!(
            ValidatorActions::<T>::get(
                &action_id.action_account_id.clone(),
                action_id.ingress_counter
            ).status,
            ValidatorsActionStatus::Actioned);
        assert_eq!(false, PendingApprovals::<T>::contains_key(&action_id.action_account_id.clone()));

        // Events are emitted
        assert_last_event::<T>(RawEvent::VotingEnded(
            action_id.clone(),
            (Box::new(
                ValidatorManagementVotingSession::<T>::new(&action_id.clone())
            ) as Box<dyn VotingSessionManager<T::AccountId, T::BlockNumber>>).state()?.is_approved()).into()
        );
    }

    end_voting_period_with_approved_invalid_actions {
        let o in 1 .. MAX_OFFENDERS; // maximum of offenders need to be less one third of minimum validators so the benchmark won't panic

        let number_of_validators = MAX_VALIDATOR_ACCOUNT_IDS;
        let validators = setup_validators::<T>(number_of_validators);
        let (sender, action_id, _, signature, quorum) = setup_action_voting::<T>(validators.clone());

        // Setup votes more than quorum to trigger end voting period
        let number_of_reject_votes = quorum;
        setup_reject_votes::<T>(&validators, number_of_reject_votes, &action_id);

        // setup offenders votes
        let (_, offenders) = validators.split_at(quorum as usize);
        let number_of_approval_votes = o;
        setup_approval_votes::<T>(&offenders.to_vec(), number_of_approval_votes, &action_id);
    }: end_voting_period(RawOrigin::None, action_id.clone(), sender.clone(), signature)
    verify {
        // Voting period is ended, but deregistration is not actioned
        assert_eq!(
            ValidatorActions::<T>::get(
                &action_id.action_account_id.clone(),
                action_id.ingress_counter
            ).status,
            ValidatorsActionStatus::AwaitingConfirmation);
        assert_eq!(false, PendingApprovals::<T>::contains_key(&action_id.action_account_id.clone()));

        // Events are emitted
        assert_last_event::<T>(RawEvent::VotingEnded(
            action_id.clone(),
            (Box::new(
                ValidatorManagementVotingSession::<T>::new(&action_id.clone())
            ) as Box<dyn VotingSessionManager<T::AccountId, T::BlockNumber>>).state()?.is_approved()).into()
        );
    }

    bond {
        let staker = create_funded_staker::<T>();
        let staker_lookup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(staker.clone());
        let reward_destination = RewardDestination::Staked;
        let amount = get_amount::<T>()?;
        whitelist_account!(staker);
    }: _(RawOrigin::Signed(staker.clone()), staker_lookup, amount, reward_destination)
    verify {
        assert!(pallet_staking::Bonded::<T>::contains_key(&staker));
        assert!(pallet_staking::Ledger::<T>::contains_key(&staker));
    }

    // Worst case scenario, MAX_NOMINATIONS
    nominate {
        let n in 1 .. pallet_staking::MAX_NOMINATIONS as u32;
        let (stash, controller) = create_bonded_user::<T>(n + 1, Default::default())?;
        let validators = create_validators::<T>(n)?;
        whitelist_account!(controller);
    }: _(RawOrigin::Signed(controller), validators)
    verify {
        assert!(pallet_staking::Nominators::<T>::contains_key(stash));
    }

    signed_bond {
        let staker = create_funded_staker::<T>();
        let staker_lookup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(staker.clone());
        let reward_destination = RewardDestination::Stash;
        let amount = get_amount::<T>()?;
        let proof: Proof<T::Signature, T::AccountId> = get_signed_bond_proof::<T>(&staker);
        whitelist_account!(staker);
    }: _(RawOrigin::Signed(staker.clone()), proof, staker_lookup, amount, reward_destination)
    verify {
        assert!(pallet_staking::Bonded::<T>::contains_key(&staker));
        assert!(pallet_staking::Ledger::<T>::contains_key(&staker));
    }

    signed_bond_extra {
        let (stash, controller) = create_bonded_staker::<T>(Default::default())?;
        let amount = 500_000_000u32.into();
        let ledger = pallet_staking::Ledger::<T>::get(&controller).ok_or("ledger not created before")?;
        let original_bonded: BalanceOf<T> = ledger.active;
        let proof: Proof<T::Signature, T::AccountId> = get_signed_bond_extra_proof::<T>(&stash);
        whitelist_account!(stash);
    }: _(RawOrigin::Signed(stash), proof, amount)
    verify {
        let ledger = pallet_staking::Ledger::<T>::get(&controller).ok_or("ledger not created after")?;
        let new_bonded: BalanceOf<T> = ledger.active;
        assert!(original_bonded < new_bonded);
    }

    signed_unbond {
        let (_, controller) = create_bonded_staker::<T>(Default::default())?;
        let amount = 500_000_000u32.into();
        let ledger = pallet_staking::Ledger::<T>::get(&controller).ok_or("ledger not created before")?;
        let original_bonded: BalanceOf<T> = ledger.active;
        let proof: Proof<T::Signature, T::AccountId> = get_signed_unbond_proof::<T>(&controller);
        whitelist_account!(controller);
    }: _(RawOrigin::Signed(controller.clone()), proof, amount)
    verify {
        let ledger = pallet_staking::Ledger::<T>::get(&controller).ok_or("ledger not created after")?;
        let new_bonded: BalanceOf<T> = ledger.active;
        assert!(original_bonded > new_bonded);
    }

    signed_set_payee {
        let (stash, controller) = create_bonded_staker::<T>(Default::default())?;
        assert_eq!(pallet_staking::Payee::<T>::get(&stash), RewardDestination::Staked);
        let proof: Proof<T::Signature, T::AccountId> = get_set_payee_proof::<T>(&controller);
        whitelist_account!(controller);
    }: _(RawOrigin::Signed(controller), proof, RewardDestination::Controller)
    verify {
        assert_eq!(pallet_staking::Payee::<T>::get(&stash), RewardDestination::Controller);
    }

    set_staking_configs {
        let previous_min_validator_bond = MinValidatorBond::<T>::get();
        let previous_min_user_bond = MinUserBond::<T>::get();
        let previous_max_commission = MaxCommission::get();

        let new_min_validator_bond = previous_min_validator_bond + 1u32.into();
        let new_min_user_bond = previous_min_user_bond + 2u32.into();
        let new_max_commission = Perbill::from_percent(99);
    }: _(RawOrigin::Root, new_min_validator_bond, new_min_user_bond, new_max_commission)
    verify {
        assert_eq!(new_min_validator_bond, MinValidatorBond::<T>::get());
        assert_eq!(new_min_user_bond, MinUserBond::<T>::get());
        assert_eq!(new_max_commission, MaxCommission::get());

        assert!(new_min_validator_bond > previous_min_validator_bond);
        assert!(new_min_user_bond > previous_min_user_bond);
        assert!(new_max_commission > previous_max_commission);
	}

    update_validator_preference {
        MaxCommission::put(Perbill::from_percent(25));
        let validator = create_known_validator::<T>()?;
        let previous_validator_prefs = pallet_staking::Validators::<T>::get(&validator);
        let new_validator_prefs = ValidatorPrefs {
			commission: Perbill::from_percent(1),
			blocked: true
		};
    }: _(RawOrigin::Root, validator.clone(), new_validator_prefs.clone())
    verify {
        let current_pref = pallet_staking::Validators::<T>::get(validator);
        assert_eq!(current_pref, new_validator_prefs);
        assert!(current_pref.commission != previous_validator_prefs.commission);
        assert!(current_pref.blocked != previous_validator_prefs.blocked);
    }

    signed_payout_all_validators_and_stakers {
        let validators_count = 10;
        let n in 1 .. <T as pallet_staking::Config>::MaxNominatorRewardedPerValidator::get() as u32;
        let (new_validators, nominators) = create_validators_with_nominators_for_era::<T>(validators_count, n)?;

        let current_era = pallet_staking::CurrentEra::get().unwrap();
        let mut validators_balance_before = Vec::new();
        let mut nominator_balances_before = Vec::new();

        for validator in new_validators.iter() {
            let validator_account_id = T::Lookup::lookup(validator.clone())?;
            // Give Era Points
            Staking::<T>::reward_by_ids(vec![(validator_account_id.clone(), 10)]);
            validators_balance_before.push(T::Currency::free_balance(&validator_account_id));
        }

        for (stash, _) in nominators.iter() {
            nominator_balances_before.push(T::Currency::free_balance(&stash));
        }

        // Create reward pool
        let total_payout = T::Currency::minimum_balance()
        .saturating_add(1u32.into())
        .saturating_mul(100u32.into())
        .saturating_mul(1000000000u32.into())
        .saturating_mul(1000000000u32.into()); // we cannot multiply by 20 zeros because u32 is not big enough
        <pallet_staking::ErasValidatorReward<T>>::insert(current_era, total_payout);

        let (signer, _) = create_bonded_staker::<T>(Default::default())?;
        let proof: Proof<T::Signature, T::AccountId> = get_payout_staker_proof::<T>(&signer);
    }: signed_payout_stakers(RawOrigin::Signed(signer), proof, current_era)
    verify {
        for (validator, balance_before) in new_validators.iter().zip(validators_balance_before.iter()) {
            let balance_after = T::Currency::free_balance(&T::Lookup::lookup(validator.clone())?);
            ensure!(balance_before < &balance_after, "Balance of validator controller should have increased after payout.");
        }
        // All validators have the same list of nominators
        for ((stash, _), balance_before) in nominators.iter().zip(nominator_balances_before.iter()) {
            let balance_after = T::Currency::free_balance(&stash);
            ensure!(balance_before < &balance_after, "Balance of nominator stash should have increased after payout.");
        }

        assert_last_event::<T>(RawEvent::PayoutCompleted(current_era, validators_balance_before.len() as u32).into());
    }

    // TODO: Benchmark the following. For now they have been assigned values set in substrate plus some additional weight
    // signed_nominate { }
    // signed_withdraw_unbonded { }
    // signed_set_controller { }
    // signed_rebond
    // kick
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::*;
    use crate::extension_builder::ExtBuilder;
    use frame_support::assert_ok;

    #[test]
    fn benchmarks() {
        let mut ext = ExtBuilder::build_default().as_externality();

        ext.execute_with(|| {
            assert_ok!(test_benchmark_add_validator::<TestRuntime>());
            assert_ok!(test_benchmark_remove_validator::<TestRuntime>());
            assert_ok!(test_benchmark_reject_action_with_end_voting::<TestRuntime>());
            assert_ok!(test_benchmark_reject_action_without_end_voting::<TestRuntime>());
            assert_ok!(test_benchmark_end_voting_period_with_approved_invalid_actions::<TestRuntime>());

            // TODO: SYS-1979 Fix `InvalidECDSASignature`
            // assert_ok!(test_benchmark_approve_action_with_end_voting::<TestRuntime>());
            // assert_ok!(test_benchmark_approve_action_without_end_voting::<TestRuntime>());

            // TODO: SYS-1980 Fix `ErrorSubmitCandidateTxnToTier1`
            // assert_ok!(test_benchmark_end_voting_period_with_rejected_valid_actions::<TestRuntime>());

            assert_ok!(test_benchmark_bond::<TestRuntime>());
            assert_ok!(test_benchmark_nominate::<TestRuntime>());
            assert_ok!(test_benchmark_update_validator_preference::<TestRuntime>());
            assert_ok!(test_benchmark_set_staking_configs::<TestRuntime>());
            assert_ok!(test_benchmark_signed_bond::<TestRuntime>());
            assert_ok!(test_benchmark_signed_bond_extra::<TestRuntime>());
            assert_ok!(test_benchmark_signed_unbond::<TestRuntime>());
            assert_ok!(test_benchmark_signed_set_payee::<TestRuntime>());
            assert_ok!(test_benchmark_signed_payout_all_validators_and_stakers::<TestRuntime>());
        });
    }
}