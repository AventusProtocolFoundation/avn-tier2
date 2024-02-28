//Copyright 2020 Artos Systems (UK) Ltd.

#![cfg(test)]

use crate::*;
use crate::common::*;
use crate::mock::*;
use crate::mock::Event as MockEvent;
use crate::mock::staking::Exposure;
use crate::mock::staking::StakingLedger;
use crate::extension_builder::ExtBuilder;
use pallet_balances::Error as BalancesError;
use pallet_staking::Error as StakingError;
use system::RawOrigin;
use sp_runtime::{assert_eq_error_rate, traits::BadOrigin, testing::{TestSignature, UintAuthorityId}};
use frame_support::{assert_ok, assert_noop};
use sp_io::crypto::{secp256k1_ecdsa_recover_compressed, secp256k1_ecdsa_recover};
use hex_literal::hex;
use substrate_test_utils::assert_eq_uvec;

fn bond_and_register_validator(mock_data: &MockData) {
    // Bonding first
    assert_ok!(ValidatorManager::bond(
       Origin::signed(mock_data.new_validator_id.clone()),
       mock_data.new_validator_id.clone(),
       ValidatorManager::min_validator_bond(),
       RewardDestination::Controller)
   );

   // Then validate
   assert_ok!(
       ValidatorManager::add_validator(
           RawOrigin::Root.into(),
           mock_data.new_validator_id.clone(),
           mock_data.validator_eth_public_key.clone(),
           ValidatorPrefs {
               commission: Perbill::from_percent(10),
               blocked: false
           }
       )
   );

   // advance by 1 era to activate the validator
   advance_era();
}

fn register_validator(mock_data: &MockData) -> DispatchResult {
    return ValidatorManager::add_validator(
        RawOrigin::Root.into(),
        mock_data.new_validator_id.clone(),
        mock_data.validator_eth_public_key.clone(),
        ValidatorPrefs {
            commission: Perbill::from_percent(10),
            blocked: false
        }
    );
}

fn bond(mock_data: &MockData) -> DispatchResult {
    return ValidatorManager::bond(
        Origin::signed(mock_data.new_validator_id.clone()),
        mock_data.new_validator_id.clone(),
        ValidatorManager::min_validator_bond(),
        RewardDestination::Controller
    );
}

#[test]
fn lydia_test_register_existing_validator() {
    let mut ext =  ExtBuilder::build_default().with_validators().as_externality();
    ext.execute_with(||{
        let mock_data = MockData::setup_valid();
        ValidatorManager::insert_to_validators(&mock_data.new_validator_id);

        assert_ok!(bond(&mock_data));

        let current_num_events = System::events().len();
        assert_noop!(register_validator(&mock_data), Error::<TestRuntime>::ValidatorAlreadyExists);

        // no Event has been deposited
        assert_eq!(System::events().len(), current_num_events);
    });
}

#[test]
fn lydia_test_register_validator_with_no_validators() {
    let mut ext =  ExtBuilder::build_default().as_externality();
    ext.execute_with(||{
        let mock_data = MockData::setup_valid();
        assert_ok!(bond(&mock_data));

        let current_num_events = System::events().len();
        assert_noop!(register_validator(&mock_data), Error::<TestRuntime>::NoValidators);

        // no Event has been deposited
        assert_eq!(System::events().len(), current_num_events);
    });
}

mod register_validator {
    use super::*;

    // TODO move MockData here and rename to Context

    fn run_preconditions(context: &MockData){
        assert_eq!(0, <ValidatorManager as Store>::ValidatorActions::iter().count());
        let validator_account_ids = ValidatorManager::validator_account_ids().expect("Should contain validators");
        assert_eq!(false, validator_account_ids.contains(&context.new_validator_id));
        assert_eq!(false, ValidatorManager::get_ethereum_public_key_if_exists(&context.new_validator_id).is_some());
    }

    fn find_validator_activation_action(data: &MockData, status: ValidatorsActionStatus) -> bool {
        let expected_eth_tx = EthTransactionType::ActivateValidator(
            ActivateValidatorData::new(
                <mock::TestRuntime as Config>::AccountToBytesConvert::into_bytes(&data.new_validator_id)
            )
        );
        return <ValidatorManager as Store>::ValidatorActions::iter().any(|(account_id, _ingress, action_data)| {
            action_data.status == status &&
            action_data.action_type == ValidatorsActionType::Activation &&
            account_id == data.new_validator_id &&
            action_data.reserved_eth_transaction == expected_eth_tx
        });
    }

    mod succeeds {
        use super::*;

        #[test]
        fn and_adds_validator() {
            let mut ext = ExtBuilder::build_default().with_validators().as_externality();
            ext.execute_with(||{
                let context = MockData::setup_valid();
                run_preconditions(&context);

                // Bonding first
                assert_ok!(bond(&context));

                // Result OK
                assert_ok!(
                    register_validator(&context)
                );
                // Upon completion validator has been added to ValidatorAccountIds storage
                assert!(ValidatorManager::validator_account_ids().unwrap().iter().any(|a| a == &context.new_validator_id));
                // ValidatorRegistered Event has been deposited
                assert_eq!(true, System::events().iter().any(|a| a.event == mock::Event::validators_manager(
                    crate::Event::<TestRuntime>::ValidatorRegistered(
                        context.new_validator_id,
                        context.validator_eth_public_key.clone())
                )));
                // ValidatorActivationStarted Event has not been deposited yet
                assert_eq!(false, System::events().iter().any(|a| a.event == mock::Event::validators_manager(
                    crate::Event::<TestRuntime>::ValidatorActivationStarted(context.new_validator_id)
                )));
                // But the activation action has been triggered
                assert_eq!(true, find_validator_activation_action(&context, ValidatorsActionStatus::AwaitingConfirmation));
            });
        }

        #[test]
        fn activation_dispatches_after_one_era() {
            let mut ext =  ExtBuilder::build_default().with_validators().as_externality();
            ext.execute_with(||{
                let context = MockData::setup_valid();
                run_preconditions(&context);

                bond_and_register_validator(&context);

                // The activation action has been sent
                assert_eq!(true, find_validator_activation_action(&context, ValidatorsActionStatus::Confirmed));
                // ValidatorActivationStarted Event has been deposited
                assert_eq!(true, System::events().iter().any(|a| a.event == mock::Event::validators_manager(
                    crate::Event::<TestRuntime>::ValidatorActivationStarted(context.new_validator_id)
                )));
            });
        }
    }
}

// Change these tests to accomodate the use of votes
#[allow(non_fmt_panics)]
mod remove_validator_public {
    use super::*;

    // tests for pub fn remove_validator(origin) -> DispatchResult {...}
    #[test]
    fn valid_case() {
        let mut ext =  ExtBuilder::build_default().with_validators().as_externality();
        ext.execute_with(||{

            //Prove this is an existing validator
            assert_eq_uvec!(validator_controllers(), vec![
                validator_id_1(), validator_id_2(), validator_id_3(),validator_id_4(),validator_id_5()
            ]);
            assert_eq!(pallet_staking::Validators::<TestRuntime>::contains_key(validator_id_3()), true);

            //Validator exists in the AVN
            assert_eq!(AVN::<TestRuntime>::is_validator(&validator_id_3()), true);

            //Remove the validator
            assert_ok!(ValidatorManager::remove_validator(RawOrigin::Root.into(), validator_id_3()));

            //Event emitted as expected
            assert!(System::events().iter().any(|a| a.event == mock::Event::validators_manager(
                crate::Event::<TestRuntime>::ValidatorDeregistered(validator_id_3())
            )));

            //Validator removed from validators manager
            assert_eq!(ValidatorManager::validator_account_ids().unwrap().iter().position(|&x| x == validator_id_3()), None);

            // Validator has been removed from the staking pallet
            assert_eq!(pallet_staking::Validators::<TestRuntime>::contains_key(validator_id_3()), false);

            //Validator is still in the session. Will be removed after 1 era.
            assert_eq_uvec!(validator_controllers(), vec![
                validator_id_1(), validator_id_2(), validator_id_3(),validator_id_4(),validator_id_5()
            ]);

            // advance 1 era
            advance_era();

            // Validator has been removed from the session
            assert_eq_uvec!(validator_controllers(), vec![validator_id_1(), validator_id_2(), validator_id_4(),validator_id_5()]);

            //Validator is also removed from the AVN
            assert_eq!(AVN::<TestRuntime>::is_validator(&validator_id_3()), false);
        });
    }

    #[test]
    fn regular_sender() {
        let mut ext =  ExtBuilder::build_default().with_validators().as_externality();
        ext.execute_with(||{
            assert_noop!(ValidatorManager::remove_validator(Origin::signed(validator_id_3()), validator_id_3()), BadOrigin);
            assert_eq!(System::events().len(), 0);
        });
    }

    #[test]
    fn unsigned_sender() {
        let mut ext =  ExtBuilder::build_default().with_validators().as_externality();
        ext.execute_with(||{
            assert_noop!(ValidatorManager::remove_validator(RawOrigin::None.into(), validator_id_3()), BadOrigin);
            assert_eq!(System::events().len(), 0);
        });
    }
}

mod remove_resigned_validator {
    use super::*;

    #[test]
    fn valid_case() {
        let mut ext =  ExtBuilder::build_default().with_validators().as_externality();
        ext.execute_with(||{
            let offender_validator_id = validator_id_3();

            assert_eq!(false, <ValidatorManager as Store>::ValidatorActions::iter_prefix_values(offender_validator_id)
                .any(|validators_action_data| validators_action_data.status == ValidatorsActionStatus::None)
            );
            let ingress_counter = <ValidatorManager as Store>::TotalIngresses::get() + 1;

            assert_eq!(false, <ValidatorManager as Store>::ValidatorActions::contains_key(offender_validator_id, ingress_counter));
            let mut validator_account_ids = ValidatorManager::validator_account_ids().expect("Should contain validators");
            assert_eq!(true, validator_account_ids.contains(&offender_validator_id));
            assert_eq!(true, ValidatorManager::get_ethereum_public_key_if_exists(&offender_validator_id).is_some());

            assert_ok!(ValidatorManager::remove_resigned_validator(&offender_validator_id));

            validator_account_ids = ValidatorManager::validator_account_ids().expect("Should contain validators");
            assert_eq!(true, <ValidatorManager as Store>::ValidatorActions::contains_key(offender_validator_id, ingress_counter));
            assert_eq!(false, validator_account_ids.contains(&offender_validator_id));
            // Public key should remain till the end of a session for resigned validators
            assert_eq!(true, ValidatorManager::get_ethereum_public_key_if_exists(&offender_validator_id).is_some());

            // Public dispatch method emits an event if removal is successful, but we are only calling the inner function here.
            assert_eq!(System::events().len(), 0);

            let ingress_counter = <ValidatorManager as Store>::TotalIngresses::get();

            assert_eq!(
                <ValidatorManager as Store>::ValidatorActions::get(offender_validator_id, ingress_counter).status,
                ValidatorsActionStatus::AwaitingConfirmation
            );

            // It takes 2 session for validators to be updated
            advance_session();
            advance_session();
            assert_eq!(
                ValidatorsActionStatus::Confirmed,
                <ValidatorManager as Store>::ValidatorActions::get(offender_validator_id, ingress_counter).status
            );
        });
    }

    #[test]
    fn non_validator() {
        let mut ext =  ExtBuilder::build_default().with_validators().as_externality();
        ext.execute_with(||{
            let validator_account_id = TestAccount::new([0u8; 32]).account_id();
            let original_validators = ValidatorManager::validator_account_ids();
            assert_ok!(ValidatorManager::remove_resigned_validator(&validator_account_id));
            // Caller of remove function has to emit event if removal is successful.
            assert_eq!(System::events().len(), 0);
            assert_eq!(ValidatorManager::validator_account_ids(), original_validators);
        });
    }

    #[test]
    fn validator_set_below_minimum_limit() {
        let mut ext = ExtBuilder::build_default().as_externality();
        ext.execute_with(||{
            let two_validators = vec![validator_id_1(), validator_id_2()];
            <ValidatorAccountIds<TestRuntime>>::put(two_validators.clone());
            assert_noop!(
                ValidatorManager::remove_resigned_validator(&validator_id_3()),
                Error::<TestRuntime>::MinimumValidatorsReached);
            assert_eq!(System::events().len(), 0);
            assert_eq!(ValidatorManager::validator_account_ids().unwrap(), two_validators);
        });
    }
}

mod remove_slashed_validator {
    use super::*;

    pub fn get_validator(index: AccountId) -> Validator<UintAuthorityId, AccountId> {
        Validator {
            account_id: index,
            key: UintAuthorityId(1),
        }
    }

    fn cast_votes_to_reach_quorum_and_end_vote(
        deregistration_id: &ActionId<AccountId>,
        validator: Validator<UintAuthorityId, AccountId>,
        signature: TestSignature)
    {
        let first_validator = get_validator(validator_id_1());
        let second_validator = get_validator(validator_id_2());
        let third_validator = get_validator(validator_id_3());
        let fourth_validator = get_validator(validator_id_4());
        ValidatorManager::record_approve_vote(
            deregistration_id,
            first_validator.account_id,
        );
        ValidatorManager::record_approve_vote(
            deregistration_id,
            second_validator.account_id,
        );
        ValidatorManager::record_approve_vote(
            deregistration_id,
            third_validator.account_id,
        );
        ValidatorManager::record_approve_vote(
            deregistration_id,
            fourth_validator.account_id,
        );
        assert_ok!(ValidatorManager::end_voting_period(RawOrigin::None.into(), *deregistration_id, validator, signature));
    }

    fn slash_validator(offender_validator_id: AccountId) {
        assert_ok!(ValidatorManager::remove_slashed_validator(&offender_validator_id));

        let ingress_counter = <ValidatorManager as Store>::TotalIngresses::get();
        let validator_account_ids = ValidatorManager::validator_account_ids().expect("Should contain validators");
        assert_eq!(false, validator_account_ids.contains(&offender_validator_id));
        assert_eq!(false, ValidatorManager::get_ethereum_public_key_if_exists(&offender_validator_id).is_some());

        // advance by 1 era to activate the validator
        advance_era();

        let deregistration_data = <ValidatorManager as Store>::ValidatorActions::get(offender_validator_id, ingress_counter);
        assert_eq!(deregistration_data.status, ValidatorsActionStatus::Confirmed);

        // vote and approve the slashing
        let deregistration_id = ActionId::new(offender_validator_id, ingress_counter);
        let submitter = get_validator(validator_id_2());
        let signature =  submitter.key.sign(&(CAST_VOTE_CONTEXT).encode()).unwrap();
        cast_votes_to_reach_quorum_and_end_vote(&deregistration_id, submitter, signature);

        // make sure the deregistration has been actioned
        let deregistration_data = <ValidatorManager as Store>::ValidatorActions::get(offender_validator_id, ingress_counter);
        assert_eq!(deregistration_data.status, ValidatorsActionStatus::Actioned);
    }

    #[test]
    fn valid_case() {
        let mut ext =  ExtBuilder::build_default().with_validators().as_externality();
        ext.execute_with(||{
            let offender_validator_id = validator_id_1();

            let mut validator_account_ids = ValidatorManager::validator_account_ids().expect("Should contain validators");
            assert_eq!(true, validator_account_ids.contains(&offender_validator_id));
            assert_eq!(true, ValidatorManager::get_ethereum_public_key_if_exists(&offender_validator_id).is_some());

            assert_ok!(ValidatorManager::remove_slashed_validator(&offender_validator_id));

            let ingress_counter = <ValidatorManager as Store>::TotalIngresses::get();

            assert_eq!(true, <ValidatorManager as Store>::ValidatorActions::contains_key(offender_validator_id, ingress_counter));

            validator_account_ids = ValidatorManager::validator_account_ids().expect("Should contain validators");
            assert_eq!(false, validator_account_ids.contains(&offender_validator_id));
            assert_eq!(false, ValidatorManager::get_ethereum_public_key_if_exists(&offender_validator_id).is_some());

            let event = mock::Event::validators_manager(
                crate::Event::<TestRuntime>::ValidatorSlashed(ActionId{
                    action_account_id: offender_validator_id,
                    ingress_counter: ingress_counter
                })
            );
            assert_eq!(true, ValidatorManager::event_emitted(&event));

            // It takes 2 session for validators to be updated
            advance_session();
            advance_session();
            assert!(<ValidatorManager as Store>::ValidatorActions::get(offender_validator_id, ingress_counter).status == ValidatorsActionStatus::Confirmed);
        });
    }

    #[test]
    fn succeeds_when_slashed_validator_registers_again() {
        let mut ext =  ExtBuilder::build_default().with_validators().as_externality();
        ext.execute_with(||{
            let mock_data = MockData::setup_valid();

            //Initial registration succeeds
            bond_and_register_validator(&mock_data);

            // Slash the validator and remove them
            slash_validator(mock_data.new_validator_id);

            // Register the validator again, after it has been slashed and removed. This time no need to bond
            assert_ok!(register_validator(&mock_data));
            // advance by 1 era to activate the validator
            advance_era();

            // Slash the validator and remove them again
            slash_validator(mock_data.new_validator_id);
        });
    }


    #[test]
    fn non_validator() {
        let mut ext =  ExtBuilder::build_default().with_validators().as_externality();
        ext.execute_with(||{
            let offender_validator_id = non_validator_id();
            assert_noop!(
                ValidatorManager::remove_slashed_validator(&offender_validator_id),
                Error::<TestRuntime>::SlashedValidatorIsNotFound
            );
        });
    }
}

#[test]
fn lydia_test_initial_validators_populated_from_genesis_config() {
    let mut ext =  ExtBuilder::build_default().with_validators().as_externality();
    ext.execute_with(||{
        assert_eq!(
            ValidatorManager::validator_account_ids().unwrap(),
            genesis_config_initial_validators().to_vec()
        );
    });
}

mod compress_public_key {
    use super::*;

    fn dummy_ecdsa_signature_as_bytes(r: [u8; 32], s: [u8; 32], v: [u8; 1]) -> [u8; 65] {
        let mut sig = Vec::new();
        sig.extend_from_slice(&r);
        sig.extend_from_slice(&s);
        sig.extend_from_slice(&v);

        let mut result = [0; 65];
        result.copy_from_slice(&sig[..]);
        return result;
    }

    mod returns_a_valid_public_key {
        use super::*;

        const MESSAGE: [u8;32] = [10; 32];

        #[test]
        fn for_a_recovered_key_from_a_signature_with_v27() {
            let r = [1; 32];
            let s = [2; 32];
            let v = [27];
            let ecdsa_signature = dummy_ecdsa_signature_as_bytes(r, s, v);

            let uncompressed_pub_key = secp256k1_ecdsa_recover(&ecdsa_signature, &MESSAGE).map_err(|_| ()).unwrap();
            let expected_pub_key = secp256k1_ecdsa_recover_compressed(&ecdsa_signature, &MESSAGE).map_err(|_| ()).unwrap();

            let calculated_pub_key = ValidatorManager::compress_eth_public_key(H512::from_slice(&uncompressed_pub_key));

            assert_eq!(ecdsa::Public::from_raw(expected_pub_key), calculated_pub_key);
        }

        #[test]
        fn for_a_recovered_key_from_a_signature_with_v28() {
            let r = [1; 32];
            let s = [2; 32];
            let v = [28];
            let ecdsa_signature = dummy_ecdsa_signature_as_bytes(r, s, v);

            let uncompressed_pub_key = secp256k1_ecdsa_recover(&ecdsa_signature, &MESSAGE).map_err(|_| ()).unwrap();
            let expected_pub_key = secp256k1_ecdsa_recover_compressed(&ecdsa_signature, &MESSAGE).map_err(|_| ()).unwrap();

            let calculated_pub_key = ValidatorManager::compress_eth_public_key(H512::from_slice(&uncompressed_pub_key));

            assert_eq!(ecdsa::Public::from_raw(expected_pub_key), calculated_pub_key);
        }

        #[test]
        fn for_a_recovered_key_from_a_different_signature() {
            let r = [7; 32];
            let s = [9; 32];
            let v = [27];
            let ecdsa_signature = dummy_ecdsa_signature_as_bytes(r, s, v);

            let uncompressed_pub_key = secp256k1_ecdsa_recover(&ecdsa_signature, &MESSAGE).map_err(|_| ()).unwrap();
            let expected_pub_key = secp256k1_ecdsa_recover_compressed(&ecdsa_signature, &MESSAGE).map_err(|_| ()).unwrap();

            let calculated_pub_key = ValidatorManager::compress_eth_public_key(H512::from_slice(&uncompressed_pub_key));

            assert_eq!(ecdsa::Public::from_raw(expected_pub_key), calculated_pub_key);
        }

        #[test]
        fn for_a_hard_coded_key() {
            // We must strip the `04` from the public key, otherwise it will not fit into a H512
            // This key is generated by running `scripts/eth/generate-ethereum-keys.js`
            let uncompressed_pub_key = hex!["8d5a0a0deb9db6775bcfe3f4d209efdb019e79682fd2bf81f1e325312dd1266ac9231db76588d67a7729c235ecd04a662dfb5d1bbfa19ebda5e601f3d373b5cf"];
            let expected_pub_key = hex!["038d5a0a0deb9db6775bcfe3f4d209efdb019e79682fd2bf81f1e325312dd1266a"];

            let calculated_pub_key = ValidatorManager::compress_eth_public_key(H512::from_slice(&uncompressed_pub_key));

            assert_eq!(ecdsa::Public::from_raw(expected_pub_key), calculated_pub_key);
        }
    }
}

mod era_payout {
    use super::*;

    #[test]
    fn valid_case() {
        let mut ext = ExtBuilder::build_default().as_externality();
        ext.execute_with(||{
            //There is no locked payment
            assert_eq!(ValidatorManager::locked_era_payout(), 0);

            // Set the pot to 100
            Balances::make_free_balance_be(&ValidatorManager::account_id(), 100);
            assert_eq!(ValidatorManager::pot(), 100);

            //This function is normally called at the end of an era
            ValidatorManager::era_payout(0, 0, 0);

            assert_eq!(ValidatorManager::locked_era_payout(), 100);
        });
    }

    #[test]
    fn error_event_raised() {
        let mut ext = ExtBuilder::build_default().as_externality();
        ext.execute_with(||{
            //There is no locked payment
            <LockedEraPayout<TestRuntime>>::put(200);
            Balances::make_free_balance_be(&ValidatorManager::account_id(), 100);
            assert_eq!(ValidatorManager::pot(), 100);

            //This function is normally called at the end of an era
            ValidatorManager::era_payout(0, 0, 0);

            //Assert error event thrown
            assert!(System::events().iter().any(|a| a.event == mock::Event::validators_manager(
                crate::Event::<TestRuntime>::NotEnoughFundsForEraPayment(ValidatorManager::pot())
            )));

            // clears all events
            System::reset_events();

            //If the chain generated more income, it can recover
            Balances::make_free_balance_be(&ValidatorManager::account_id(), 250);
            assert_eq!(ValidatorManager::pot(), 250);

            ValidatorManager::era_payout(0, 0, 0);

            //Assert no error event thrown
            assert_eq!(System::events().len(), 0usize);
            assert_eq!(ValidatorManager::locked_era_payout(), 250);

        });
    }
}

mod bond {
    use super::*;

    #[derive(Clone)]
    struct BondContext {
        origin: Origin,
        staker: Staker,
        value: BalanceOf<TestRuntime>,
        payee: RewardDestination<AccountId>,
    }

    impl Default for BondContext {
        fn default() -> Self {
            let staker: Staker = Default::default();
            BondContext {
                origin: Origin::signed(staker.stash.account_id()),
                staker,
                value: <ValidatorManager as Store>::MinUserBond::get(),
                payee: RewardDestination::Stash
            }
        }
    }

    impl BondContext {
        fn setup(&self) {
            Balances::make_free_balance_be(
                &self.staker.stash.account_id(),
                <ValidatorManager as Store>::MinUserBond::get()
            );
        }

        pub fn bonded_event_emitted(&self) -> bool {
            return System::events().iter().any(|e| {
                e.event == MockEvent::pallet_staking(crate::mock::staking::Event::<TestRuntime>::Bonded(self.staker.stash.account_id(), self.value))
            });
        }
    }

    #[test]
    fn succeeds_with_good_parameters() {
        let mut ext = ExtBuilder::build_default().with_validators().as_externality();
        ext.execute_with(|| {
            let context = &BondContext::default();
            context.setup();

            let stash_account_id = &context.staker.stash.account_id();
            let controller_account_id = &context.staker.controller.account_id();

            // Prior to bonding check that the staker is not taking part in staking
            assert_eq!(Staking::bonded(stash_account_id), None);

            assert_ok!(ValidatorManager::bond(
                context.origin.clone(),
                *controller_account_id,
                context.value,
                context.payee,
            ));

            // Event is emitted
            assert!(context.bonded_event_emitted());

            // The staker is now bonded. Key = stash, value = controller
            assert_eq!(Staking::bonded(stash_account_id).unwrap(), *controller_account_id);

            // The ledger is as expected. Total and active have the same value
            assert_eq!(
                Staking::ledger(&controller_account_id),
                Some(
                    StakingLedger {
                        stash: *stash_account_id,
                        total: context.value,
                        active: context.value,
                        unlocking: vec![],
                        claimed_rewards: vec![]
                    }
                )
            );

            // Free balance is not affected
            assert_eq!(Balances::free_balance(*stash_account_id), context.value);

            // We have locked up all the money we have
            assert_eq!(Balances::usable_balance(*stash_account_id), 0u128);
            assert_eq!(System::account(stash_account_id).data.misc_frozen, context.value);
            assert_eq!(System::account(stash_account_id).data.fee_frozen, context.value);

            // Transfer will fail because all the balance is locked
            assert_noop!(
                Balances::transfer(Origin::signed(*stash_account_id), context.staker.relayer, 1),
                BalancesError::<TestRuntime>::LiquidityRestrictions
            );
        });
    }

    mod fails_when {
        use super::*;

        #[test]
        fn extrinsic_is_unsigned() {
            let mut ext = ExtBuilder::build_default().with_validators().as_externality();
            ext.execute_with(|| {
                let context = &BondContext::default();
                context.setup();

                let controller_account_id = &context.staker.controller.account_id();

                assert_noop!(
                    ValidatorManager::bond(
                        RawOrigin::None.into(),
                        *controller_account_id,
                        context.value,
                        context.payee,
                    ),
                    BadOrigin
                );
            });
        }

        #[test]
        fn sender_does_not_have_enough_fund() {
            let mut ext = ExtBuilder::build_default().with_validators().as_externality();
            ext.execute_with(|| {
                let mut context = &mut BondContext::default();
                context.setup();

                let controller_account_id = &context.staker.controller.account_id();
                let min_validator_bond = <ValidatorManager as Store>::MinValidatorBond::get();
                let min_user_bond = <ValidatorManager as Store>::MinUserBond::get();
                context.value = min_validator_bond.min(min_user_bond) - 1;

                assert_noop!(
                    ValidatorManager::bond(
                        context.origin.clone(),
                        *controller_account_id,
                        context.value,
                        context.payee,
                    ),
                    Error::<TestRuntime>::InsufficientBond
                );
            });
        }
    }
}

mod add_validator {
    use super::*;

    #[derive(Clone)]
    struct AddValidatorContext {
        origin: Origin,
        staker: Staker,
        validator_eth_public_key: ecdsa::Public,
        preferences: ValidatorPrefs
    }

    impl Default for AddValidatorContext {
        fn default() -> Self {
            let staker: Staker = Default::default();
            AddValidatorContext {
                origin: RawOrigin::Root.into(),
                staker,
                validator_eth_public_key: ecdsa::Public::default(),
                preferences: ValidatorPrefs {
                    commission: <ValidatorManager as Store>::MaxCommission::get(),
                    blocked: false
                }
            }
        }
    }

    impl AddValidatorContext {
        fn setup(&self) {
            let stash = self.staker.stash.account_id();
            let controller = self.staker.controller.account_id();
            let bond_value = <ValidatorManager as Store>::MinValidatorBond::get();

            Balances::make_free_balance_be(&stash, bond_value);

            assert_ok!(ValidatorManager::bond(Origin::signed(stash), controller, bond_value, RewardDestination::Stash));
        }
    }

    #[test]
    fn succeeds_with_good_parameters() {
        let mut ext = ExtBuilder::build_default().with_validators().as_externality();
        ext.execute_with(|| {
            let context = &AddValidatorContext::default();
            context.setup();

            let stash_account_id = &context.staker.stash.account_id();
            let controller_account_id = &context.staker.controller.account_id();

            // Prior to bonding check that the staker is not taking part in staking
            assert_eq!(Staking::bonded(stash_account_id).unwrap(), *controller_account_id);

            assert_ok!(ValidatorManager::add_validator(
                context.origin.clone(),
                *controller_account_id,
                context.validator_eth_public_key.clone(),
                context.preferences.clone(),
            ));

            assert_eq!(true, ValidatorManager::validator_account_ids().unwrap().contains(stash_account_id));
            assert_eq!(ValidatorManager::get_validator_by_eth_public_key(context.validator_eth_public_key.clone()), *stash_account_id);
        });
    }

    mod fails_when {
        use super::*;

        #[test]
        fn extrinsic_is_unsigned() {
            let mut ext = ExtBuilder::build_default().with_validators().as_externality();
            ext.execute_with(|| {
                let context = &AddValidatorContext::default();
                context.setup();

                let stash_account_id = &context.staker.stash.account_id();
                let controller_account_id = &context.staker.controller.account_id();

                // Prior to bonding check that the staker is not taking part in staking
                assert_eq!(Staking::bonded(stash_account_id).unwrap(), *controller_account_id);

                assert_noop!(
                    ValidatorManager::add_validator(
                        RawOrigin::None.into(),
                        *controller_account_id,
                        context.validator_eth_public_key.clone(),
                        context.preferences.clone(),
                    ),
                    BadOrigin
                );
            });
        }

        #[test]
        fn no_validators() {
            let mut ext = ExtBuilder::build_default().as_externality();
            ext.execute_with(|| {
                let context = &AddValidatorContext::default();
                context.setup();

                let stash_account_id = &context.staker.stash.account_id();
                let controller_account_id = &context.staker.controller.account_id();

                // Prior to bonding check that the staker is not taking part in staking
                assert_eq!(Staking::bonded(stash_account_id).unwrap(), *controller_account_id);

                assert_noop!(
                    ValidatorManager::add_validator(
                        context.origin.clone(),
                        *controller_account_id,
                        context.validator_eth_public_key.clone(),
                        context.preferences.clone(),
                    ),
                    Error::<TestRuntime>::NoValidators
                );
            });
        }

        #[test]
        fn validator_eth_key_already_exists() {
            let mut ext = ExtBuilder::build_default().with_validators().as_externality();
            ext.execute_with(|| {
                let context = &AddValidatorContext::default();
                context.setup();

                let stash_account_id = &context.staker.stash.account_id();
                let controller_account_id = &context.staker.controller.account_id();

                // Prior to bonding check that the staker is not taking part in staking
                assert_eq!(Staking::bonded(stash_account_id).unwrap(), *controller_account_id);

                <<ValidatorManager as Store>::EthereumPublicKeys>::insert(context.validator_eth_public_key.clone(), stash_account_id);

                assert_noop!(
                    ValidatorManager::add_validator(
                        context.origin.clone(),
                        *controller_account_id,
                        context.validator_eth_public_key.clone(),
                        context.preferences.clone(),
                    ),
                    Error::<TestRuntime>::ValidatorEthKeyAlreadyExists
                );
            });
        }

        #[test]
        fn not_bonded() {
            let mut ext = ExtBuilder::build_default().with_validators().as_externality();
            ext.execute_with(|| {
                let context = &AddValidatorContext::default();

                let stash_account_id = &context.staker.stash.account_id();
                let controller_account_id = &context.staker.controller.account_id();

                assert_eq!(Staking::bonded(stash_account_id), None);

                assert_noop!(
                    ValidatorManager::add_validator(
                        context.origin.clone(),
                        *controller_account_id,
                        context.validator_eth_public_key.clone(),
                        context.preferences.clone(),
                    ),
                    Error::<TestRuntime>::NotController
                );
            });
        }

        #[test]
        fn validator_does_not_have_enough_bond() {
            let mut ext = ExtBuilder::build_default().with_validators().as_externality();
            ext.execute_with(|| {
                let context = &AddValidatorContext::default();
                context.setup();

                let stash_account_id = &context.staker.stash.account_id();
                let controller_account_id = &context.staker.controller.account_id();

                // Prior to bonding check that the staker is not taking part in staking
                assert_eq!(Staking::bonded(stash_account_id).unwrap(), *controller_account_id);

                // Increased the minimum validator bond, so the previously bonded amount is not valid to be a validator anymore
                <<ValidatorManager as Store>::MinValidatorBond>::put(ValidatorManager::min_validator_bond() + 1);

                assert_noop!(
                    ValidatorManager::add_validator(
                        context.origin.clone(),
                        *controller_account_id,
                        context.validator_eth_public_key.clone(),
                        context.preferences.clone(),
                    ),
                    Error::<TestRuntime>::InsufficientValidatorBond
                );
            });
        }

        #[test]
        fn validator_commission_is_too_high() {
            let mut ext = ExtBuilder::build_default().with_validators().as_externality();
            ext.execute_with(|| {
                let mut context = &mut AddValidatorContext::default();
                context.setup();

                let stash_account_id = &context.staker.stash.account_id();
                let controller_account_id = &context.staker.controller.account_id();
                context.preferences = ValidatorPrefs {
                    commission: Perbill::from_percent(<ValidatorManager as Store>::MaxCommission::get().deconstruct() + 1),
                    blocked: false
                };

                // Prior to bonding check that the staker is not taking part in staking
                assert_eq!(Staking::bonded(stash_account_id).unwrap(), *controller_account_id);

                assert_noop!(
                    ValidatorManager::add_validator(
                        context.origin.clone(),
                        *controller_account_id,
                        context.validator_eth_public_key.clone(),
                        context.preferences.clone(),
                    ),
                    Error::<TestRuntime>::ValidatorCommissionTooHigh
                );
            });
        }

        #[test]
        fn validator_already_exists() {
            let mut ext = ExtBuilder::build_default().with_validators().as_externality();
            ext.execute_with(|| {
                let context = &AddValidatorContext::default();
                context.setup();

                let stash_account_id = &context.staker.stash.account_id();
                let controller_account_id = &context.staker.controller.account_id();

                // Prior to bonding check that the staker is not taking part in staking
                assert_eq!(Staking::bonded(stash_account_id).unwrap(), *controller_account_id);

                <<ValidatorManager as Store>::ValidatorAccountIds>::append(&stash_account_id);

                assert_noop!(
                    ValidatorManager::add_validator(
                        context.origin.clone(),
                        *controller_account_id,
                        context.validator_eth_public_key.clone(),
                        context.preferences.clone(),
                    ),
                    Error::<TestRuntime>::ValidatorAlreadyExists
                );
            });
        }
    }
}

mod nominate {
    use super::*;

    #[derive(Clone)]
    struct NominateContext {
        origin: Origin,
        staker: Staker,
        amount: BalanceOf<TestRuntime>,
        targets: Vec<<<TestRuntime as system::Config>::Lookup as StaticLookup>::Source>
    }

    impl Default for NominateContext {
        fn default() -> Self {
            let staker: Staker = Default::default();
            NominateContext {
                origin: Origin::signed(staker.controller.account_id()),
                staker,
                targets: genesis_config_initial_validators().to_vec(),
                amount: <ValidatorManager as Store>::MinUserBond::get(),
            }
        }
    }

    impl NominateContext {
        fn setup(&self) {
            let stash = self.staker.stash.account_id();
            let controller = self.staker.controller.account_id();

            Balances::make_free_balance_be(&stash, self.amount);
            assert_ok!(ValidatorManager::bond(
                Origin::signed(stash), controller, self.amount, RewardDestination::Stash
            ));
        }

        pub fn nominated_event_emitted(&self) -> bool {
            return System::events().iter().any(|e| {
                e.event == mock::Event::validators_manager(crate::Event::<TestRuntime>::Nominated(
                    self.staker.stash.account_id(),
                    self.amount,
                    self.targets.len() as u32)
                )
            });
        }
    }

    fn sum_staker_exposure(era_index: EraIndex, staker: AccountId) -> u128 {
        let mut exposures: Vec<Exposure<AccountId, u128>> = vec![];
        exposures.push(Staking::eras_stakers(era_index, validator_id_1()));
        exposures.push(Staking::eras_stakers(era_index, validator_id_2()));
        exposures.push(Staking::eras_stakers(era_index, validator_id_3()));
        exposures.push(Staking::eras_stakers(era_index, validator_id_4()));
        exposures.push(Staking::eras_stakers(era_index, validator_id_5()));

        let mut sum = 0;
        exposures.into_iter().for_each(|e| {
            if e.others.len() as u32 > 0 {
                sum += e.others.iter().find(|o| o.who == staker).unwrap().value;
            }
        });

        return sum;
    }

    #[test]
    fn succeeds_with_good_parameters() {
        let mut ext = ExtBuilder::build_default().with_validators().as_externality();
        ext.execute_with(|| {
            let context = &NominateContext::default();
            context.setup();

            let stash_account_id = &context.staker.stash.account_id();

            // Prior to nominating check that the staker is not a nominator
            assert_eq!(Staking::nominators(stash_account_id), None);

            assert_ok!(ValidatorManager::nominate(context.origin.clone(), context.targets.clone()));

            // The staker is now a nominator
            assert_eq!(
                Staking::nominators(stash_account_id).unwrap().targets,
                genesis_config_initial_validators()
            );

            // Event is emitted
            assert!(context.nominated_event_emitted());

            let mut era_index = Staking::active_era().unwrap().index;

            // The nomination is not active yet
            let exposure = Staking::eras_stakers(era_index, validator_id_1());
            assert_eq_error_rate!(exposure.own, VALIDATOR_STAKE, 1000);
            assert_eq_error_rate!(exposure.total, VALIDATOR_STAKE, 1000);

            assert_eq!(sum_staker_exposure(era_index, *stash_account_id), 0);

            // advance the era
            advance_era();

            era_index = Staking::active_era().unwrap().index;

            // The exposure is set
            let new_exposure = Staking::eras_stakers(era_index, validator_id_2());
            assert_eq_error_rate!(new_exposure.own, VALIDATOR_STAKE, 1000);
            assert_eq_error_rate!(
                sum_staker_exposure(era_index, *stash_account_id),
                <ValidatorManager as Store>::MinUserBond::get(),
                2000
            );
        });
    }

    mod fails_when {
        use super::*;

        #[test]
        fn extrinsic_is_unsigned() {
            let mut ext = ExtBuilder::build_default().with_validators().as_externality();
            ext.execute_with(|| {
                let context = &NominateContext::default();
                context.setup();

                assert_noop!(ValidatorManager::nominate(RawOrigin::None.into(), context.targets.clone()), BadOrigin);
            });
        }

        #[test]
        fn sender_is_not_controller_account() {
            let mut ext = ExtBuilder::build_default().with_validators().as_externality();
            ext.execute_with(|| {
                let mut context = &mut NominateContext::default();
                context.setup();

                context.staker.controller = TestAccount::new([30u8; 32]);
                context.staker.controller_key_pair = context.staker.controller.key_pair();

                assert_noop!(
                    ValidatorManager::nominate(
                        Origin::signed(context.staker.controller.account_id()),
                        context.targets.clone()
                    ),
                    Error::<TestRuntime>::NotController
                );
            });
        }

        #[test]
        fn sender_does_not_have_enough_fund() {
            let mut ext = ExtBuilder::build_default().with_validators().as_externality();
            ext.execute_with(|| {
                let context = &mut NominateContext::default();
                context.setup();

                // Increased the minimum user bond, so the previously bonded amount is not valid to nominate anymore
                <<ValidatorManager as Store>::MinUserBond>::put(ValidatorManager::min_user_bond() + 1);

                assert_noop!(
                    ValidatorManager::nominate(
                        context.origin.clone(),
                        context.targets.clone()
                    ),
                    Error::<TestRuntime>::InsufficientFundsToNominateBond
                );
            });
        }

        #[test]
        fn sender_is_already_a_validator() {
            let mut ext = ExtBuilder::build_default().with_validators().as_externality();
            ext.execute_with(|| {
                let context = &NominateContext::default();
                context.setup();

                pallet_staking::Validators::<TestRuntime>::insert(
                    &context.staker.stash.account_id(),
                    ValidatorPrefs {
                        commission: Perbill::from_percent(10).clone(),
                        blocked: false,
                    }
                );

                assert_noop!(
                    ValidatorManager::nominate(
                        context.origin.clone(),
                        context.targets.clone()
                    ),
                    Error::<TestRuntime>::AlreadyValidating
                );
            });
        }
    }
}

mod update_validator_preference {
    use super::*;

    #[derive(Clone)]
    struct UpdateValidatorPreferenceContext {
        origin: Origin,
        staker: Staker,
        preference: ValidatorPrefs,
        value: BalanceOf<TestRuntime>,
    }

    impl Default for UpdateValidatorPreferenceContext {
        fn default() -> Self {
            let staker: Staker = Default::default();
            UpdateValidatorPreferenceContext {
                origin: RawOrigin::Root.into(),
                staker: staker,
                preference: ValidatorPrefs {
                    commission: Perbill::from_percent(25),
                    blocked: false
                },
                value: <ValidatorManager as Store>::MinUserBond::get(),
            }
        }
    }

    impl UpdateValidatorPreferenceContext {
        fn setup(&self) {
            let stash = self.staker.stash.account_id();
            let controller = self.staker.controller.account_id();

            Balances::make_free_balance_be(&stash, self.value);
            assert_ok!(ValidatorManager::bond(
                Origin::signed(stash), controller, self.value, RewardDestination::Stash
            ));
        }

        fn validator_preference_updated_event_emitted(&self) -> bool {
            return System::events().iter().any(|e| {
                e.event == mock::Event::validators_manager(crate::Event::<TestRuntime>::ValidatorPreferenceUpdated(
                    self.staker.stash.account_id(),
                    self.preference.commission,
                    self.preference.blocked
                ))
            });
        }
    }

    #[test]
    fn succeeds_with_good_parameters() {
        let mut ext = ExtBuilder::build_default().with_validators().as_externality();
        ext.execute_with(|| {
            let context = &UpdateValidatorPreferenceContext::default();
            context.setup();

            let stash_account_id = context.staker.stash.account_id();
            let controller_account_id = context.staker.controller.account_id();

            assert_ok!(ValidatorManager::update_validator_preference(
                context.origin.clone(),
                controller_account_id,
                context.preference.clone()
            ));

            assert!(pallet_staking::Ledger::<TestRuntime>::contains_key(&controller_account_id));
            assert_eq!(
                Staking::validators(&stash_account_id),
                context.preference
            );

            // Event is emitted
            assert!(context.validator_preference_updated_event_emitted());
        });
    }

    mod fails_when {
        use super::*;

        #[test]
        fn extrinsic_is_unsigned() {
            let mut ext = ExtBuilder::build_default().with_validators().as_externality();
            ext.execute_with(|| {
                let context = &UpdateValidatorPreferenceContext::default();
                context.setup();

                assert_noop!(
                    ValidatorManager::update_validator_preference(
                        RawOrigin::None.into(),
                        context.staker.controller.account_id(),
                        context.preference.clone()
                    ),
                    BadOrigin
                );
            });
        }

        #[test]
        fn sender_is_not_controller_account() {
            let mut ext = ExtBuilder::build_default().with_validators().as_externality();
            ext.execute_with(|| {
                let mut context = &mut UpdateValidatorPreferenceContext::default();
                context.setup();

                context.staker.controller = TestAccount::new([30u8; 32]);
                context.staker.controller_key_pair = context.staker.controller.key_pair();

                assert_noop!(
                    ValidatorManager::update_validator_preference(
                        context.origin.clone(),
                        context.staker.controller.account_id(),
                        context.preference.clone()
                    ),
                    Error::<TestRuntime>::NotController
                );
            });
        }

        #[test]
        fn validator_commission_is_too_high() {
            let mut ext = ExtBuilder::build_default().with_validators().as_externality();
            ext.execute_with(|| {
                let mut context = &mut UpdateValidatorPreferenceContext::default();
                context.setup();

                context.preference = ValidatorPrefs {
                    commission: Perbill::from_percent(<ValidatorManager as Store>::MaxCommission::get().deconstruct() + 1),
                    blocked: false
                };

                assert_noop!(
                    ValidatorManager::update_validator_preference(
                        context.origin.clone(),
                        context.staker.controller.account_id(),
                        context.preference.clone()
                    ),
                    Error::<TestRuntime>::ValidatorCommissionTooHigh
                );
            });
        }
    }
}

mod set_staking_configs {
    use super::*;

    #[derive(Clone)]
    struct SetStakingConfigsContext {
        origin: Origin,
        min_validator_bond: BalanceOf<TestRuntime>,
        min_user_bond: BalanceOf<TestRuntime>,
        max_commission: Perbill,
    }

    impl Default for SetStakingConfigsContext {
        fn default() -> Self {
            SetStakingConfigsContext {
                origin: RawOrigin::Root.into(),
                min_validator_bond: <ValidatorManager as Store>::MinValidatorBond::get(),
                min_user_bond: <ValidatorManager as Store>::MinUserBond::get(),
                max_commission: <ValidatorManager as Store>::MaxCommission::get(),
            }
        }
    }

    #[test]
    fn succeeds_with_good_parameters() {
        let mut ext = ExtBuilder::build_default().with_validators().as_externality();
        ext.execute_with(|| {
            let context = &SetStakingConfigsContext::default();

            assert_ok!(ValidatorManager::set_staking_configs(
                context.origin.clone(),
                (context.min_validator_bond + 1).into(),
                (context.min_user_bond + 1).into(),
                Perbill::from_percent(context.max_commission.deconstruct() - 1),
            ));

            assert_eq!(
                <ValidatorManager as Store>::MinValidatorBond::get(),
                context.min_validator_bond + 1
            );

            assert_eq!(
                <ValidatorManager as Store>::MinUserBond::get(),
                context.min_user_bond + 1
            );

            assert_eq!(
                <ValidatorManager as Store>::MaxCommission::get(),
                Perbill::from_percent(context.max_commission.deconstruct() - 1),
            );
        });
    }

    mod fails_when {
        use super::*;

        #[test]
        fn extrinsic_is_unsigned() {
            let mut ext = ExtBuilder::build_default().with_validators().as_externality();
            ext.execute_with(|| {
                let context = &SetStakingConfigsContext::default();

                assert_noop!(
                    ValidatorManager::set_staking_configs(
                        RawOrigin::None.into(),
                        context.min_validator_bond + 1,
                        context.min_user_bond + 1,
                        Perbill::from_percent(context.max_commission.deconstruct() - 1),
                    ),
                    BadOrigin
                );
            });
        }
    }
}

mod kick {
    use super::*;

    #[derive(Clone)]
    struct KickContext {
        origin: Origin,
        staker: Staker,
        who: Vec<<<TestRuntime as system::Config>::Lookup as StaticLookup>::Source>
    }

    impl Default for KickContext {
        fn default() -> Self {
            let staker: Staker = Default::default();
            KickContext {
                origin: RawOrigin::Root.into(),
                staker: staker.clone(),
                who: vec![staker.stash.account_id()]
            }
        }
    }

    impl KickContext {
        fn setup(&self) {
            let stash = self.staker.stash.account_id();
            let controller = self.staker.controller.account_id();
            let bond_amount = <ValidatorManager as Store>::MinUserBond::get();

            Balances::make_free_balance_be(&stash, bond_amount);
            assert_ok!(ValidatorManager::bond(
                Origin::signed(stash), controller, bond_amount, RewardDestination::Stash
            ));
            advance_era();

            assert_ok!(ValidatorManager::nominate(
                Origin::signed(controller),
                genesis_config_initial_validators().to_vec()
            ));
            advance_era();
        }

        fn kicked_event_emitted(&self) -> bool {
            return System::events().iter().any(|e| {
                e.event == MockEvent::pallet_staking(
                    crate::mock::staking::Event::<TestRuntime>::Kicked(
                        self.staker.stash.account_id(),
                        validator_id_1()
                    )
                )
            });
        }
    }

    #[test]
    fn succeeds_with_good_parameters() {
        let mut ext = ExtBuilder::build_default().with_validators().as_externality();
        ext.execute_with(|| {
            let context = &KickContext::default();
            context.setup();

            let nominator_to_kick = context.staker.stash.account_id();
            let validator1 = validator_id_1();

            let mut era_index = Staking::active_era().unwrap().index;
            let exposure = Staking::eras_stakers(era_index, validator1);
            assert_eq!(true, exposure.others.into_iter().any(|o| o.who == nominator_to_kick));

            assert_ok!(ValidatorManager::kick(
                context.origin.clone(),
                validator1,
                context.who.clone()
            ));

            advance_era();

            era_index = Staking::active_era().unwrap().index;
            let exposure = Staking::eras_stakers(era_index, validator1);
            assert_eq!(false, exposure.others.into_iter().any(|o| o.who == nominator_to_kick));

            // The kicked validator has been `swap_remove`ed from the nominator's target list
            assert_eq!(
                pallet_staking::Nominators::<TestRuntime>::get(&context.staker.stash.account_id()).unwrap().targets,
                vec![
                    validator_id_5(),
                    validator_id_2(),
                    validator_id_3(),
                    validator_id_4(),
                ]
            );

            // Event is emitted
            assert!(context.kicked_event_emitted());
        });
    }

    mod fails_when {
        use super::*;

        #[test]
        fn extrinsic_is_unsigned_by_root_user() {
            let mut ext = ExtBuilder::build_default().with_validators().as_externality();
            ext.execute_with(|| {
                let context = &KickContext::default();
                context.setup();

                let validator1 = validator_id_1();

                assert_noop!(
                    ValidatorManager::kick(
                        RawOrigin::None.into(),
                        validator1,
                        context.who.clone()
                    ),
                    BadOrigin
                );
            });
        }

        #[test]
        fn extrinsic_is_unsigned_by_controller_account() {
            let mut ext = ExtBuilder::build_default().with_validators().as_externality();
            ext.execute_with(|| {
                let context = &KickContext::default();
                context.setup();

                let unbonded_controller_account_id = TestAccount::new([100u8; 32]).account_id();

                assert_noop!(
                    ValidatorManager::kick(
                        context.origin.clone(),
                        unbonded_controller_account_id,
                        context.who.clone()
                    ),
                    StakingError::<TestRuntime>::NotController
                );
            });
        }

        #[test]
        fn era_election_is_not_closed_yet() {
            let mut ext = ExtBuilder::build_default().with_validators().as_externality();
            ext.execute_with(|| {
                let context = &KickContext::default();
                context.setup();

                let validator1 = validator_id_1();

                pallet_staking::EraElectionStatus::<TestRuntime>::put(ElectionStatus::Open(1));

                assert_noop!(
                    ValidatorManager::kick(
                        context.origin.clone(),
                        validator1,
                        context.who.clone()
                    ),
                    StakingError::<TestRuntime>::CallNotAllowed
                );
            });
        }
    }
}
