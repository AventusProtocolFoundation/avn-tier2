#![cfg(test)]

use parking_lot::RwLock;
use codec::alloc::sync::Arc;
use crate::extension_builder::ExtBuilder;
use crate::mock::*;
use crate::*;
use frame_support::{assert_noop, assert_ok, assert_err};
use pallet_avn::Error as AvNError;
use sp_io::TestExternalities;
use sp_runtime::testing::{TestSignature, UintAuthorityId};
use sp_runtime::traits::BadOrigin;
use system::RawOrigin;
use sp_core::{offchain::{testing::{PoolState, OffchainState}}};

const VOTING_PERIOD_END: u64 = 12;
const QUORUM: u32 = 3;
const DEFAULT_INGRESS_COUNTER: IngressCounter = 0;

struct Context<'a> {
    pub validator: Validator<UintAuthorityId, AccountId>,
    pub action_id: ActionId<AccountId>,
    pub record_deregister_validator_calculation_signature: TestSignature,
    pub offchain_state: &'a Arc<RwLock<OffchainState>>,
}

fn setup_context(offchain_state: &Arc<RwLock<OffchainState>>) -> Context {
    let validator = get_validator(validator_id_1());
    let deregistered_validator = get_validator(validator_id_3());

    Context {
        action_id: ActionId::new(deregistered_validator.account_id, DEFAULT_INGRESS_COUNTER),
        validator: validator.clone(),
        record_deregister_validator_calculation_signature: generate_signature(validator, CAST_VOTE_CONTEXT),
        offchain_state,
    }
}

fn test_validator_count() -> u32 {
    return genesis_config_initial_validators().len() as u32;
}

pub fn get_test_validators() -> Vec<AccountId> {
    return genesis_config_initial_validators().to_vec();
}

pub fn get_validator(account_id: AccountId) -> Validator<UintAuthorityId, AccountId> {
    Validator {
        account_id: account_id,
        key: UintAuthorityId(1u64),
    }
}

pub fn get_validator_by_index(index: u32) -> Validator<UintAuthorityId, AccountId> {
    Validator {
        account_id: genesis_config_initial_validators()[index as usize],
        key: UintAuthorityId(index.into()),
    }
}

pub fn get_non_validator() -> Validator<UintAuthorityId, AccountId> {
    Validator {
        account_id: TestAccount::new([10u8; 32]).account_id(),
        key: UintAuthorityId(10),
    }
}

fn generate_signature(
    validator: Validator<UintAuthorityId, AccountId>,
    context: &[u8],
) -> TestSignature {
    validator
        .key
        .sign(&(context).encode())
        .expect("Signature is signed")
}

fn setup_ext_builder() -> (TestExternalities, Arc<RwLock<PoolState>>, Arc<RwLock<OffchainState>>) {
    let (ext, pool_state, offchain_state) = ExtBuilder::build_default()
        .with_validators()
        .for_offchain_worker()
        .as_externality_with_state();

        (ext, pool_state, offchain_state)
}

fn setup_voting_session(action_id: &ActionId<AccountId>) {
    let candidate_tx = EthTransactionType::DeregisterValidator(
        DeregisterValidatorData::new(<mock::TestRuntime as Config>::AccountToBytesConvert::into_bytes(&action_id.action_account_id))
    );

    ValidatorManager::insert_validators_action_data(action_id, candidate_tx);
    ValidatorManager::insert_pending_approval(action_id);
    ValidatorManager::create_voting_session(
        action_id,
        QUORUM,
        VOTING_PERIOD_END,
    );

    assert_eq!(
        ValidatorManager::get_vote(action_id).ayes.is_empty(),
        true
    );
    assert_eq!(
        ValidatorManager::get_vote(action_id).nays.is_empty(),
        true
    );
}

fn approve_validator_action(
    validator: &Validator<UintAuthorityId, AccountId>,
    context: &Context,
) -> DispatchResult {
    let eth_compatible_data = ValidatorManager::convert_data_to_eth_compatible_encoding(&context.action_id).unwrap();

    mock_response_of_get_ecdsa_signature(
        &mut context.offchain_state.write(),
        eth_compatible_data,
        Some(hex::encode([1;65].to_vec()).as_bytes().to_vec()),
    );

    let (_, approval_signature) = ValidatorManager::sign_validators_action_for_ethereum(&context.action_id).unwrap();
    set_mock_recovered_account_id(validator.account_id);

    ValidatorManager::approve_validator_action(
        RawOrigin::None.into(),
        context.action_id,
        validator.clone(),
        approval_signature,
        context.record_deregister_validator_calculation_signature.clone(),
    )
}

fn reject_validator_action(
    validator: &Validator<UintAuthorityId, AccountId>,
    context: &Context,
) -> DispatchResult {
    ValidatorManager::reject_validator_action(
        RawOrigin::None.into(),
        context.action_id,
        validator.clone(),
        context.record_deregister_validator_calculation_signature.clone(),
    )
}

fn vote_added_event_is_emitted_successfully(
    voter_account_id: &AccountId,
    action_id: &ActionId<AccountId>,
    is_approve: bool,
) -> bool {
    System::events()
        .iter()
        .any(
            |a| a.event == mock::Event::validators_manager(
                crate::Event::<TestRuntime>::VoteAdded(
                    *voter_account_id,
                    *action_id,
                    is_approve
                )
            )
        )
}

mod approve_vote {
    use super::*;

    mod succeeds_when {
        use super::*;

        #[test]
        fn one_validator_votes() {
            let (mut ext, _, offchain_state) = setup_ext_builder();

            ext.execute_with(|| {
                let context = setup_context(&offchain_state);
                setup_voting_session(&context.action_id);

                assert_ok!(approve_validator_action(&context.validator, &context));
            });
        }

        #[test]
        fn two_validators_vote_differently() {
            let (mut ext, _, offchain_state) = setup_ext_builder();

            ext.execute_with(|| {
                let context = setup_context(&offchain_state);
                let second_validator = get_validator(validator_id_2());
                setup_voting_session(&context.action_id);

                assert_ok!(reject_validator_action(&context.validator, &context));
                assert_ok!(approve_validator_action(&second_validator, &context));

                assert!(
                    vote_added_event_is_emitted_successfully(
                        &context.validator.account_id,
                        &context.action_id,
                        false,
                    )
                ); // TODO: Use constants to replace true/false
                assert!(
                    vote_added_event_is_emitted_successfully(
                        &second_validator.account_id,
                        &context.action_id,
                        true,
                    )
                );
            });
        }

        #[test]
        fn two_validators_vote_the_same() {
            let (mut ext, _, offchain_state) = setup_ext_builder();

            ext.execute_with(|| {
                let context = setup_context(&offchain_state);
                let second_validator = get_validator(validator_id_2());

                setup_voting_session(&context.action_id);

                assert_ok!(approve_validator_action(&context.validator, &context));
                assert_ok!(approve_validator_action(&second_validator, &context));

                assert!(
                    vote_added_event_is_emitted_successfully(
                        &context.validator.account_id,
                        &context.action_id,
                        true,
                    )
                );
                assert!(
                    vote_added_event_is_emitted_successfully(
                        &second_validator.account_id,
                        &context.action_id,
                        true,
                    )
                );
            });
        }
    }

    mod success_implies {
        use super::*;

        #[test]
        fn voter_account_id_is_in_ayes() {
            let (mut ext, _, offchain_state) = setup_ext_builder();

            ext.execute_with(|| {
                let context = setup_context(&offchain_state);
                setup_voting_session(&context.action_id);

                assert_ok!(approve_validator_action(&context.validator, &context));

                assert!(
                    ValidatorManager::get_vote(&context.action_id)
                        .ayes
                        .contains(&context.validator.account_id)
                );
            });
        }

        #[test]
        fn voter_account_id_is_not_in_nays() {
            let (mut ext, _, offchain_state) = setup_ext_builder();

            ext.execute_with(|| {
                let context = setup_context(&offchain_state);
                setup_voting_session(&context.action_id);

                assert_ok!(approve_validator_action(&context.validator, &context));

                assert!(
                    !ValidatorManager::get_vote(&context.action_id)
                        .nays
                        .contains(&context.validator.account_id)
                );
            });
        }

        #[test]
        fn event_is_emitted_correctly() {
            let (mut ext, _, offchain_state) = setup_ext_builder();

            ext.execute_with(|| {
                let context = setup_context(&offchain_state);
                setup_voting_session(&context.action_id);

                assert_ok!(approve_validator_action(&context.validator, &context));

                assert!(
                    vote_added_event_is_emitted_successfully(
                        &context.validator.account_id,
                        &context.action_id,
                        true,
                    )
                );
            });
        }
    }

    mod fails_when {
        use super::*;

        fn set_ecdsa_signature_verification_to_fail() {
            ETH_PUBLIC_KEY_VALID.with(|pk| { *pk.borrow_mut() = false; });
        }

        #[test]
        fn origin_is_signed() {
            let (mut ext, _, offchain_state) = setup_ext_builder();

            ext.execute_with(|| {
                let context = setup_context(&offchain_state);
                setup_voting_session(&context.action_id);

                let eth_compatible_data = ValidatorManager::convert_data_to_eth_compatible_encoding(
                    &context.action_id
                ).unwrap();

                mock_response_of_get_ecdsa_signature(
                    &mut context.offchain_state.write(),
                    eth_compatible_data,
                    Some(hex::encode([1;65].to_vec()).as_bytes().to_vec()),
                );

                let (_, approval_signature) = ValidatorManager::sign_validators_action_for_ethereum(
                    &context.action_id
                ).unwrap();

                assert_noop!(
                    ValidatorManager::approve_validator_action(
                        Origin::signed(Default::default()),
                        context.action_id,
                        context.validator,
                        approval_signature,
                        context.record_deregister_validator_calculation_signature
                    ),
                    BadOrigin
                );
            });
        }

        #[test]
        fn voter_is_invalid_validator() {
            let (mut ext, _, offchain_state) = setup_ext_builder();

            ext.execute_with(|| {
                let context = setup_context(&offchain_state);
                setup_voting_session(&context.action_id);

                let eth_compatible_data = ValidatorManager::convert_data_to_eth_compatible_encoding(
                    &context.action_id
                ).unwrap();

                mock_response_of_get_ecdsa_signature(
                    &mut context.offchain_state.write(),
                    eth_compatible_data,
                    Some(hex::encode([1;65].to_vec()).as_bytes().to_vec()),
                );

                let (_, approval_signature) = ValidatorManager::sign_validators_action_for_ethereum(
                    &context.action_id
                ).unwrap();

                let result = ValidatorManager::approve_validator_action(
                    RawOrigin::None.into(),
                    context.action_id,
                    get_non_validator(),
                    approval_signature,
                    context.record_deregister_validator_calculation_signature
                );

                // We can't use assert_noop here because we return an error after mutating storage
                assert_err!(result, AvNError::<TestRuntime>::InvalidECDSASignature);

            });
        }

        #[test]
        fn voting_session_is_not_open() {
            let (mut ext, _, offchain_state) = setup_ext_builder();

            ext.execute_with(|| {
                let context = setup_context(&offchain_state);
                setup_voting_session(&context.action_id);
                ValidatorManager::remove_voting_session(
                    &context.action_id,
                );

                // TODO [TYPE: test refactoring][PRI: LOW]: Refactor set_mock_recovered_account_id(validator.account_id);
                // out of approve_deregistration function, so assert_noop! macro can be used here.
                assert_eq!(
                    approve_validator_action(&context.validator, &context),
                    Err(AvNError::<TestRuntime>::InvalidVote.into()) // TODO: Use the right error code for this
                );
            });
        }

        #[test]
        fn voter_has_already_approved() {
            let (mut ext, _, offchain_state) = setup_ext_builder();

            ext.execute_with(|| {
                let context = setup_context(&offchain_state);
                setup_voting_session(&context.action_id);

                assert_ok!(approve_validator_action(&context.validator, &context));

                assert_noop!(
                    approve_validator_action(&context.validator, &context),
                    AvNError::<TestRuntime>::DuplicateVote
                );
            });
        }

        #[test]
        fn voter_has_already_rejected() {
            let (mut ext, _, offchain_state) = setup_ext_builder();

            ext.execute_with(|| {
                let context = setup_context(&offchain_state);
                setup_voting_session(&context.action_id);

                assert_ok!(reject_validator_action(&context.validator, &context));

                // TODO [TYPE: test refactoring][PRI: LOW]: Refactor set_mock_recovered_account_id(validator.account_id);
                // out of approve_deregistration function, so assert_noop! macro can be used here.
                assert_eq!(
                    approve_validator_action(&context.validator, &context),
                    Err(AvNError::<TestRuntime>::DuplicateVote.into())
                );
            });
        }

        #[test]
        fn a_bad_ecdsa_signature_is_used() {
            let (mut ext, _, offchain_state) = ExtBuilder::build_default()
                .with_validator_count(get_test_validators())
                .for_offchain_worker()
                .as_externality_with_state();

            ext.execute_with(|| {
                let context = setup_context(&offchain_state);
                setup_voting_session(&context.action_id);

                let eth_compatible_data = ValidatorManager::convert_data_to_eth_compatible_encoding(
                    &context.action_id
                ).unwrap();

                mock_response_of_get_ecdsa_signature(
                    &mut context.offchain_state.write(),
                    eth_compatible_data,
                    Some(hex::encode([1;65].to_vec()).as_bytes().to_vec()),
                );

                let (_, approval_signature) = ValidatorManager::sign_validators_action_for_ethereum(
                    &context.action_id
                ).unwrap();

                set_ecdsa_signature_verification_to_fail();

                let result = ValidatorManager::approve_validator_action(
                    RawOrigin::None.into(),
                    context.action_id,
                    context.validator.clone(),
                    approval_signature,
                    context.record_deregister_validator_calculation_signature
                );
                // We can't use assert_noop here because we return an error after mutating storage
                assert_err!(result, AvNError::<TestRuntime>::InvalidECDSASignature);

                // Check for offence
                assert_eq!(true, ValidatorManager::offence_reported(
                    context.validator.account_id,
                    test_validator_count(),
                    vec![context.validator.account_id,],
                    ValidatorOffenceType::InvalidSignatureSubmitted)
                );
            });
        }
    }
}

mod reject_vote {
    use super::*;

    mod succeeds_when {
        use super::*;

        #[test]
        fn one_validator_votes() {
            let (mut ext, _, offchain_state) = setup_ext_builder();

            ext.execute_with(|| {
                let context = setup_context(&offchain_state);
                setup_voting_session(&context.action_id);

                assert_ok!(reject_validator_action(&context.validator, &context));
            });
        }

        #[test]
        fn two_validators_vote_differently() {
            let (mut ext, _, offchain_state) = setup_ext_builder();

            ext.execute_with(|| {
                let context = setup_context(&offchain_state);
                let second_validator = get_validator(validator_id_2());
                setup_voting_session(&context.action_id);

                assert_ok!(approve_validator_action(&context.validator, &context));
                assert_ok!(reject_validator_action(&second_validator, &context));

                vote_added_event_is_emitted_successfully(
                    &context.validator.account_id,
                    &context.action_id,
                    true,
                ); // TODO: Use constants to replace true/false
                vote_added_event_is_emitted_successfully(
                    &second_validator.account_id,
                    &context.action_id,
                    false,
                );
            });
        }

        #[test]
        fn two_validators_vote_the_same() {
            let (mut ext, _, offchain_state) = setup_ext_builder();

            ext.execute_with(|| {
                let context = setup_context(&offchain_state);
                let second_validator = get_validator(validator_id_2());

                setup_voting_session(&context.action_id);

                assert_ok!(reject_validator_action(&context.validator, &context));
                assert_ok!(reject_validator_action(&second_validator, &context));

                vote_added_event_is_emitted_successfully(
                    &context.validator.account_id,
                    &context.action_id,
                    false,
                );
                vote_added_event_is_emitted_successfully(
                    &second_validator.account_id,
                    &context.action_id,
                    false,
                );
            });
        }
    }

    mod success_implies {
        use super::*;

        #[test]
        fn voter_account_id_is_in_nays() {
            let (mut ext, _, offchain_state) = setup_ext_builder();

            ext.execute_with(|| {
                let context = setup_context(&offchain_state);
                setup_voting_session(&context.action_id);

                assert_ok!(reject_validator_action(&context.validator, &context));

                assert!(
                    ValidatorManager::get_vote(&context.action_id)
                        .nays
                        .contains(&context.validator.account_id)
                );
            });
        }

        #[test]
        fn voter_account_id_is_not_in_ayes() {
            let (mut ext, _, offchain_state) = setup_ext_builder();

            ext.execute_with(|| {
                let context = setup_context(&offchain_state);
                setup_voting_session(&context.action_id);

                assert_ok!(reject_validator_action(&context.validator, &context));

                assert!(
                    !ValidatorManager::get_vote(&context.action_id)
                        .ayes
                        .contains(&context.validator.account_id)
                );
            });
        }

        #[test]
        fn event_is_emitted_correctly() {
            let (mut ext, _, offchain_state) = setup_ext_builder();

            ext.execute_with(|| {
                let context = setup_context(&offchain_state);
                setup_voting_session(&context.action_id);

                assert_ok!(reject_validator_action(&context.validator, &context));

                vote_added_event_is_emitted_successfully(
                    &context.validator.account_id,
                    &context.action_id,
                    false,
                );
            });
        }
    }

    mod fails_when {
        use super::*;

        #[test]
        fn origin_is_signed() {
            let (mut ext, _, offchain_state) = setup_ext_builder();

            ext.execute_with(|| {
                let context = setup_context(&offchain_state);
                setup_voting_session(&context.action_id);

                assert_noop!(
                    ValidatorManager::reject_validator_action(
                        Origin::signed(Default::default()),
                        context.action_id,
                        context.validator,
                        context.record_deregister_validator_calculation_signature
                    ),
                    BadOrigin
                );
            });
        }

        #[test]
        fn voter_is_invalid_validator() {
            let (mut ext, _, offchain_state) = setup_ext_builder();

            ext.execute_with(|| {
                let context = setup_context(&offchain_state);
                setup_voting_session(&context.action_id);

                assert_noop!(
                    ValidatorManager::reject_validator_action(
                        RawOrigin::None.into(),
                        context.action_id,
                        get_non_validator(),
                        context.record_deregister_validator_calculation_signature
                    ),
                    AvNError::<TestRuntime>::NotAValidator
                );
            });
        }

        #[test]
        fn voting_session_is_not_open() {
            let (mut ext, _, offchain_state) = setup_ext_builder();

            ext.execute_with(|| {
                let context = setup_context(&offchain_state);
                setup_voting_session(&context.action_id);
                ValidatorManager::remove_voting_session(
                    &context.action_id,
                );

                assert_noop!(
                    reject_validator_action(&context.validator, &context),
                    AvNError::<TestRuntime>::InvalidVote // TODO: Use the right error code for this
                );
            });
        }

        #[test]
        fn voter_has_already_rejected() {
            let (mut ext, _, offchain_state) = setup_ext_builder();

            ext.execute_with(|| {
                let context = setup_context(&offchain_state);
                setup_voting_session(&context.action_id);

                assert_ok!(reject_validator_action(&context.validator, &context));

                assert_noop!(
                    reject_validator_action(&context.validator, &context),
                    AvNError::<TestRuntime>::DuplicateVote
                );
            });
        }

        #[test]
        fn voter_has_already_approved() {
            let (mut ext, _, offchain_state) = setup_ext_builder();

            ext.execute_with(|| {
                let context = setup_context(&offchain_state);
                setup_voting_session(&context.action_id);

                assert_ok!(approve_validator_action(&context.validator, &context));

                assert_noop!(
                    reject_validator_action(&context.validator, &context),
                    AvNError::<TestRuntime>::DuplicateVote
                );
            });
        }
    }
}

mod multiple_successful_votes_imply {
    use super::*;

    #[test]
    fn ayes_includes_only_approvals() {
        let (mut ext, _, offchain_state) = setup_ext_builder();

        ext.execute_with(|| {
            let context = setup_context(&offchain_state);
            let second_validator = get_validator(validator_id_2());
            let third_validator = get_validator(validator_id_3());

            setup_voting_session(&context.action_id);

            assert_ok!(approve_validator_action(&context.validator, &context));
            assert_ok!(approve_validator_action(&second_validator, &context));
            assert_ok!(reject_validator_action(&third_validator, &context));

            // Approvals
            assert!(
                ValidatorManager::get_vote(&context.action_id)
                    .ayes
                    .contains(&context.validator.account_id)
            );
            assert!(
                ValidatorManager::get_vote(&context.action_id)
                    .ayes
                    .contains(&second_validator.account_id)
            );

            // Rejection
            assert!(
                !ValidatorManager::get_vote(&context.action_id)
                    .ayes
                    .contains(&third_validator.account_id)
            );
        });
    }

    #[test]
    fn nays_includes_only_rejections() {
        let (mut ext, _, offchain_state) = setup_ext_builder();

        ext.execute_with(|| {
            let context = setup_context(&offchain_state);
            let second_validator = get_validator(validator_id_2());
            let third_validator = get_validator(validator_id_3());

            setup_voting_session(&context.action_id);

            assert_ok!(approve_validator_action(&context.validator, &context));
            assert_ok!(approve_validator_action(&second_validator, &context));
            assert_ok!(reject_validator_action(&third_validator, &context));

            // Approvals
            assert!(
                !ValidatorManager::get_vote(&context.action_id)
                    .nays
                    .contains(&context.validator.account_id)
            );
            assert!(
                !ValidatorManager::get_vote(&context.action_id)
                    .nays
                    .contains(&second_validator.account_id)
            );

            // Rejection
            assert!(
                ValidatorManager::get_vote(&context.action_id)
                    .nays
                    .contains(&third_validator.account_id)
            );
        });
    }

    #[test]
    fn events_are_emitted_correctly() {
        let (mut ext, _, offchain_state) = setup_ext_builder();

        ext.execute_with(|| {
            let context = setup_context(&offchain_state);
            let second_validator = get_validator(validator_id_2());
            let third_validator = get_validator(validator_id_3());

            setup_voting_session(&context.action_id);

            assert_ok!(approve_validator_action(&context.validator, &context));
            assert_ok!(approve_validator_action(&second_validator, &context));
            assert_ok!(reject_validator_action(&third_validator, &context));

            vote_added_event_is_emitted_successfully(
                &context.validator.account_id,
                &context.action_id,
                true,
            );
            vote_added_event_is_emitted_successfully(
                &second_validator.account_id,
                &context.action_id,
                true,
            );
            vote_added_event_is_emitted_successfully(
                &third_validator.account_id,
                &context.action_id,
                false,
            );
        });
    }
}

mod end_voting_period {
    use super::*;

    fn end_voting_period(context: &Context) -> Result<(), DispatchError> {
        ValidatorManager::end_voting_period(
            RawOrigin::None.into(),
            context.action_id,
            context.validator.clone(),
            context
                .record_deregister_validator_calculation_signature
                .clone(),
        )
    }

    fn cast_votes_to_reach_quorum(action_id: &ActionId<AccountId>) {
        let first_validator = get_validator(validator_id_1());
        let second_validator = get_validator(validator_id_2());
        let third_validator = get_validator(validator_id_3());
        ValidatorManager::record_approve_vote(
            action_id,
            first_validator.account_id,
        );
        ValidatorManager::record_approve_vote(
            action_id,
            second_validator.account_id,
        );
        ValidatorManager::record_approve_vote(
            action_id,
            third_validator.account_id,
        );
    }

    mod succeeds_when {
        use super::*;

        mod a_vote_reached_quorum_and_that_implies {
            use super::*;

            #[test]
            fn end_voting_period_is_ok() {
                let (mut ext, _, offchain_state) = setup_ext_builder();

                ext.execute_with(|| {
                    let context = setup_context(&offchain_state);
                    setup_voting_session(
                        &context.action_id,
                    );
                    cast_votes_to_reach_quorum(&context.action_id);

                    assert_ok!(end_voting_period(&context));
                });
            }

            #[test]
            fn deregistered_validator_account_id_is_removed_from_pending_deregistrations() {
                let (mut ext, _, offchain_state) = setup_ext_builder();

                ext.execute_with(|| {
                    let context = setup_context(&offchain_state);
                    setup_voting_session(
                        &context.action_id,
                    );
                    cast_votes_to_reach_quorum(&context.action_id);

                    assert_ok!(end_voting_period(&context));
                    assert_eq!(false, <ValidatorManager as Store>::PendingApprovals::contains_key(&context.action_id.action_account_id));
                });
            }

            #[test]
            fn voting_ended_event_is_emitted_successfully() {
                let (mut ext, _, offchain_state) = setup_ext_builder();

                ext.execute_with(|| {
                    let context = setup_context(&offchain_state);
                    setup_voting_session(
                        &context.action_id,
                    );
                    cast_votes_to_reach_quorum(&context.action_id);

                    assert_ok!(end_voting_period(&context));
                    assert!(System::events().iter().any(|a| a.event
                        == mock::Event::validators_manager(
                            crate::Event::<TestRuntime>::VotingEnded(
                                context.action_id,
                                true
                            )
                        )));
                });
            }
        }

        mod end_of_voting_period_passed_and_that_implies {
            use super::*;

            #[test]
            fn end_voting_period_is_ok() {
                let (mut ext, _, offchain_state) = setup_ext_builder();

                ext.execute_with(|| {
                    let context = setup_context(&offchain_state);
                    setup_voting_session(
                        &context.action_id,
                    );
                    System::set_block_number(50);

                    assert_ok!(end_voting_period(&context));
                });
            }

            #[test]
            fn deregistered_validator_account_id_is_removed_from_pending_deregistrations() {
                let (mut ext, _, offchain_state) = setup_ext_builder();

                ext.execute_with(|| {
                    let context = setup_context(&offchain_state);
                    setup_voting_session(
                        &context.action_id,
                    );
                    System::set_block_number(50);

                    assert_ok!(end_voting_period(&context));
                    assert!(!<ValidatorManager as Store>::PendingApprovals::contains_key(&context.action_id.action_account_id));
                });
            }

            #[test]
            fn voting_ended_event_is_emitted_successfully() {
                let (mut ext, _, offchain_state) = setup_ext_builder();

                ext.execute_with(|| {
                    let context = setup_context(&offchain_state);
                    setup_voting_session(
                        &context.action_id,
                    );
                    System::set_block_number(50);

                    assert_ok!(end_voting_period(&context));
                    assert!(System::events().iter().any(|a| a.event
                        == mock::Event::validators_manager(
                            crate::Event::<TestRuntime>::VotingEnded(
                                context.action_id,
                                false
                            )
                        )));
                });
            }
        }
    }

    mod fails_when {
        use super::*;

        #[test]
        fn origin_is_signed() {
            let (mut ext, _, offchain_state) = setup_ext_builder();

            ext.execute_with(|| {
                let context = setup_context(&offchain_state);
                setup_voting_session(&context.action_id);
                cast_votes_to_reach_quorum(&context.action_id);

                assert_noop!(
                    ValidatorManager::end_voting_period(
                        Origin::signed(Default::default()),
                        context.action_id.clone(),
                        context.validator.clone(),
                        context
                            .record_deregister_validator_calculation_signature
                            .clone(),
                    ),
                    BadOrigin
                );
            });
        }

        #[test]
        fn voting_session_does_not_exist() {
            let (mut ext, _, offchain_state) = setup_ext_builder();

            ext.execute_with(|| {
                let context = setup_context(&offchain_state);
                setup_voting_session(
                    &context.action_id,
                );
                cast_votes_to_reach_quorum(&context.action_id);
                ValidatorManager::remove_voting_session(
                    &context.action_id,
                );

                assert_noop!(
                    end_voting_period(&context),
                    Error::<TestRuntime>::VotingSessionIsNotValid
                );
            });
        }

        #[test]
        fn cannot_end_vote() {
            let (mut ext, _, offchain_state) = setup_ext_builder();

            ext.execute_with(|| {
                let context = setup_context(&offchain_state);
                setup_voting_session(
                    &context.action_id,
                );

                assert_noop!(
                    end_voting_period(&context),
                    Error::<TestRuntime>::ErrorEndingVotingPeriod
                );
            });
        }

        #[test]
        fn submit_candidate_transaction_to_tier1_fails() {
            let (mut ext, _, offchain_state) = setup_ext_builder();

            ext.execute_with(|| {
                let context = setup_context(&offchain_state);
                let deregistration_id_2 = ActionId::new(
                    get_validator(validator_id_2()).account_id,
                    DEFAULT_INGRESS_COUNTER
                );
                setup_voting_session(&deregistration_id_2);
                cast_votes_to_reach_quorum(&deregistration_id_2);

                assert_noop!(
                    ValidatorManager::end_voting_period(
                        RawOrigin::None.into(),
                        deregistration_id_2,
                        context.validator.clone(),
                        context
                            .record_deregister_validator_calculation_signature
                            .clone(),
                    ),
                    Error::<TestRuntime>::ErrorSubmitCandidateTxnToTier1
                );
            });
        }
    }

    mod creates_offences_when {
        use super::*;

        pub fn advance_to_block_number(target_block_number: u64) {
            System::set_block_number(target_block_number);
        }

        mod deregistration_is_approved_and {
            use super::*;

            fn cast_approve_votes(action_id: &ActionId<AccountId>, vote_count: u32) {
                assert!(test_validator_count() >= vote_count);

                for validator in get_test_validators().into_iter().take(vote_count as usize) {
                    ValidatorManager::record_approve_vote(action_id, validator);
                }
            }

            #[test]
            fn one_validator_submits_nay_vote() {
                let (mut ext, _, offchain_state) = ExtBuilder::build_default()
                    .with_validator_count(get_test_validators())
                    .for_offchain_worker()
                    .as_externality_with_state();
                ext.execute_with(|| {
                    let context = setup_context(&offchain_state);
                    setup_voting_session(&context.action_id);
                    cast_approve_votes(&context.action_id, test_validator_count() - 1);

                    // cast single nay vote
                    let bad_validator = get_validator_by_index((test_validator_count() - 1).into());
                    ValidatorManager::record_reject_vote(
                        &context.action_id,
                        bad_validator.account_id,
                    );

                    advance_to_block_number(VOTING_PERIOD_END + 1);
                    assert_ok!(end_voting_period(&context));

                    // check single bad nay vote offence
                    assert_eq!(true, ValidatorManager::offence_reported(
                        context.validator.account_id,
                        test_validator_count(),
                        vec![bad_validator.account_id],
                        ValidatorOffenceType::RejectedValidAction)
                    );
                });
            }

            #[test]
            fn multiple_validators_submit_nay_vote() {
                let (mut ext, _, offchain_state) = ExtBuilder::build_default()
                    .with_validator_count(get_test_validators())
                    .for_offchain_worker()
                    .as_externality_with_state();
                ext.execute_with(|| {
                    let context = setup_context(&offchain_state);
                    setup_voting_session(&context.action_id);
                    cast_approve_votes(&context.action_id, test_validator_count() - 2);

                    // cast 2 nay votes
                    let bad_validator1 = get_validator_by_index((test_validator_count() - 2).into());
                    ValidatorManager::record_reject_vote(
                        &context.action_id,
                        bad_validator1.account_id,
                    );

                    let bad_validator2 = get_validator_by_index((test_validator_count() - 1).into());
                    ValidatorManager::record_reject_vote(
                        &context.action_id,
                        bad_validator2.account_id,
                    );

                    advance_to_block_number(VOTING_PERIOD_END + 1);
                    assert_ok!(end_voting_period(&context));

                    // check 2 bad nay vote offence
                    assert_eq!(true, ValidatorManager::offence_reported(
                        context.validator.account_id,
                        test_validator_count(),
                        vec![bad_validator1.account_id, bad_validator2.account_id],
                        ValidatorOffenceType::RejectedValidAction)
                    );
                });
            }
        }

        mod deregistration_is_rejected_and {
            use super::*;

            fn cast_reject_votes(action_id: &ActionId<AccountId>, vote_count: u32) {
                assert!(test_validator_count() >= vote_count);

                for validator in get_test_validators().into_iter().take(vote_count as usize) {
                    ValidatorManager::record_reject_vote(action_id, validator);
                }
            }

            #[test]
            fn one_validator_submits_aye_vote() {
                let (mut ext, _, offchain_state) = ExtBuilder::build_default()
                    .with_validator_count(get_test_validators())
                    .for_offchain_worker()
                    .as_externality_with_state();
                ext.execute_with(|| {
                    let context = setup_context(&offchain_state);
                    setup_voting_session(&context.action_id);
                    cast_reject_votes(&context.action_id, test_validator_count() - 1);

                    // cast single aye vote
                    let bad_validator = get_validator_by_index((test_validator_count() - 1).into());
                    ValidatorManager::record_approve_vote(
                        &context.action_id,
                        bad_validator.account_id,
                    );

                    advance_to_block_number(VOTING_PERIOD_END + 1);
                    assert_ok!(end_voting_period(&context));

                    // check single bad aye vote offence
                    assert_eq!(true, ValidatorManager::offence_reported(
                        context.validator.account_id,
                        test_validator_count(),
                        vec![bad_validator.account_id],
                        ValidatorOffenceType::ApprovedInvalidAction)
                    );
                });
            }

            #[test]
            fn multiple_validators_submit_aye_vote() {
                let (mut ext, _, offchain_state) = ExtBuilder::build_default()
                    .with_validator_count(get_test_validators())
                    .for_offchain_worker()
                    .as_externality_with_state();
                ext.execute_with(|| {
                    let context = setup_context(&offchain_state);
                    setup_voting_session(&context.action_id);
                    cast_reject_votes(&context.action_id, test_validator_count() - 2);

                    // cast 2 aye vote
                    let bad_validator1 = get_validator_by_index((test_validator_count() - 2).into());
                    ValidatorManager::record_approve_vote(
                        &context.action_id,
                        bad_validator1.account_id,
                    );

                    let bad_validator2 = get_validator_by_index((test_validator_count() - 1).into());
                    ValidatorManager::record_approve_vote(
                        &context.action_id,
                        bad_validator2.account_id,
                    );

                    advance_to_block_number(VOTING_PERIOD_END + 1);
                    assert_ok!(end_voting_period(&context));

                    // check 2 bad aye vote offence
                    assert_eq!(true, ValidatorManager::offence_reported(
                        context.validator.account_id,
                        test_validator_count(),
                        vec![bad_validator1.account_id, bad_validator2.account_id],
                        ValidatorOffenceType::ApprovedInvalidAction)
                    );
                });
            }
        }
    }
}
