// Copyright 2020 Artos Systems (UK) Ltd.

use crate::mock::*;
use crate::mock::extension_builder::ExtBuilder;
use sp_runtime::{testing::UintAuthorityId};
use sp_avn_common::event_types::Validator;

fn change_validators_good() {
    VALIDATORS.with(|v| {
        let mut v = v.borrow_mut();
        *v = Some(vec![1, 2]);
        Some(v.clone())
    });

    advance_session_and_force_new_validators();
}

fn change_validators_empty() {
    VALIDATORS.with(|v| {
        let mut v = v.borrow_mut();
        *v = Some(vec![]);
        Some(v.clone())
    });

    advance_session_and_force_new_validators();
}

fn advance_session_no_validators_change() {
    VALIDATORS.with(|v| {
        let mut v = v.borrow_mut();
        *v = None;
        Some(v.clone())
    });

    advance_session_and_force_new_validators();
}

fn advance_session_and_force_new_validators() {
    // need to do it twice for the change to take effect
    advance_session();
    advance_session();
}

fn advance_session() {
    let now = System::block_number().max(1);
    System::set_block_number(now + 1);
    Session::rotate_session();
    assert_eq!(Session::current_index(), (now / Period::get()) as u32);
}

#[test]
//* good case: keys have been imported in the ethereum-events pallet
fn keys_populated_correctly_on_genesis() {
    let mut ext = ExtBuilder::build_default()
        .with_validators()
        .as_externality();
    ext.execute_with(||{
        assert!(AVN::validators() == vec![
            Validator {account_id:1, key:UintAuthorityId(1)},
            Validator {account_id:2, key:UintAuthorityId(2)},
            Validator {account_id:3, key:UintAuthorityId(3)}
        ]);
    });
}

#[test]
#[ignore]
//* edge case: handle an empty set of validatorId and keys
fn empty_validators_on_genesis() {
    // Substrate doesn't allow empty validators on genesis
}

#[test]
// *changed is true but with the same validators: keys list has not changed
fn keys_populated_correctly_new_session_same_validators_change() {
    let mut ext = ExtBuilder::build_default()
        .with_validators()
        .as_externality();
    ext.execute_with(||{
        assert!(AVN::validators() == vec![
            Validator {account_id:1, key:UintAuthorityId(1)},
            Validator {account_id:2, key:UintAuthorityId(2)},
            Validator {account_id:3, key:UintAuthorityId(3)}
        ]);

        advance_session();

        assert!(AVN::validators() == vec![
            Validator {account_id:1, key:UintAuthorityId(1)},
            Validator {account_id:2, key:UintAuthorityId(2)},
            Validator {account_id:3, key:UintAuthorityId(3)}
        ]);
    });
}

#[test]
// * changed is true: Ensure that the keys have been updated
fn keys_populated_correctly_new_session_with_good_change() {
    let mut ext = ExtBuilder::build_default()
        .with_validators()
        .as_externality();
    ext.execute_with(||{
        assert!(AVN::validators() == vec![
            Validator {account_id:1, key:UintAuthorityId(1)},
            Validator {account_id:2, key:UintAuthorityId(2)},
            Validator {account_id:3, key:UintAuthorityId(3)}
        ]);

        change_validators_good();

        assert!(AVN::validators() == vec![
            Validator {account_id:1, key:UintAuthorityId(1)},
            Validator {account_id:2, key:UintAuthorityId(2)}
        ]);
    });
}

#[test]
// * changed is true: Ensure that the keys have been updated
fn keys_populated_correctly_new_session_with_empty_change() {
    let mut ext = ExtBuilder::build_default()
        .with_validators()
        .as_externality();
    ext.execute_with(||{
        assert!(AVN::validators() == vec![
            Validator {account_id:1, key:UintAuthorityId(1)},
            Validator {account_id:2, key:UintAuthorityId(2)},
            Validator {account_id:3, key:UintAuthorityId(3)}
        ]);

        change_validators_empty();

        assert!(AVN::validators() == vec![]);
    });
}

#[test]
// * changed is false: keys list has not changed
fn keys_populated_correctly_new_session_with_no_change() {
    let mut ext = ExtBuilder::build_default()
        .with_validators()
        .as_externality();
    ext.execute_with(||{
        assert!(AVN::validators() == vec![
            Validator {account_id:1, key:UintAuthorityId(1)},
            Validator {account_id:2, key:UintAuthorityId(2)},
            Validator {account_id:3, key:UintAuthorityId(3)}
        ]);

        advance_session_no_validators_change();

        assert!(AVN::validators() == vec![
            Validator {account_id:1, key:UintAuthorityId(1)},
            Validator {account_id:2, key:UintAuthorityId(2)},
            Validator {account_id:3, key:UintAuthorityId(3)}
        ]);
    });
}

#[test]
#[ignore]
fn test_on_disabled() {
    // Not implemented in our pallet
}