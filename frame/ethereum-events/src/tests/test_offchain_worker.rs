// Copyright 2020 Artos Systems (UK) Ltd.

#![cfg(test)]

use crate::*;
use crate::mock::*;
use crate::Call;
use frame_support::{assert_noop, unsigned::ValidateUnsigned};
use sp_core::hash::H256;
use sp_runtime::{DispatchError, testing::{UintAuthorityId, TestSignature}, transaction_validity::TransactionValidityError};
use system::{RawOrigin};
use core::convert::TryInto;
use sp_avn_common::{
    event_types::{
        AddedValidatorData,
        EthEventCheckResult,
        EventData,
        CheckResult
    },
    avn_tests_helpers::ethereum_converters::*
};

#[derive(Clone)]
pub struct MockData {
    pub block_number: u64,
    pub check_result: CheckResult,
    pub event_id: EthEventId,
    pub event_data: EventData,
    pub eth_event_check_result: EthEventCheckResult<<mock::TestRuntime as frame_system::Config>::BlockNumber,AccountId>,
    pub validator: Validator<UintAuthorityId, AccountId>,
    pub signature: <AuthorityId as RuntimeAppPublic>::Signature,
    pub checked_by: AccountId,
    pub min_challenge_votes: u32,
}

impl MockData {
pub fn get_valid_added_validator_data() -> AddedValidatorData {
        let data = Self::get_data();

        let topic1 = vec![10;32];
        let topic2_lhs = vec![15;32];
        let topic2_rhs = vec![25;32];
        let topic3 = vec![30;32];
        let topics = vec![topic1, topic2_lhs, topic2_rhs, topic3];

        return AddedValidatorData::parse_bytes(Some(data), topics).unwrap();
    }

    fn get_invalid_added_validator_data() -> AddedValidatorData {
        let data = Self::get_data();

        let topic1 = vec![0;32];
        let topic2_lhs = topic1.clone();
        let topic2_rhs = topic1.clone();
        let topic3 = topic1.clone();
        let topics = vec![topic1, topic2_lhs, topic2_rhs, topic3];

        return AddedValidatorData::parse_bytes(Some(data), topics).unwrap();
    }

    fn get_data() -> Vec<u8> {
        return into_32_be_bytes(&10000u32.to_le_bytes());
    }

    fn setup() -> Self {
        System::set_block_number(2);
        let validator = EthereumEvents::validators()[0].clone();
        let block_number = 4;
        let check_result = CheckResult::Ok;
        let event_id = EthEventId {
            signature: ValidEvents::AddedValidator.signature(),
            transaction_hash: H256::from([1; 32]),
        };
        let event_data = EventData::LogAddedValidator(Self::get_valid_added_validator_data());
        let checked_by = validator.account_id.clone();
        let min_challenge_votes = 1;

        MockData {
            block_number: block_number,
            checked_by: checked_by.clone(),
            check_result: check_result.clone(),
            event_id: event_id.clone(),
            event_data: event_data.clone(),
            eth_event_check_result: EthEventCheckResult::new(
                block_number,
                check_result,
                &event_id,
                &event_data,
                checked_by,
                block_number + EVENT_CHALLENGE_PERIOD,
                min_challenge_votes
            ),
            validator: validator,
            signature: TestSignature(0, vec![]),
            min_challenge_votes: min_challenge_votes,
        }
    }
}

#[test]
fn submit_checkevent_result_should_return_expected_result_when_input_is_valid() {
    let mut ext = ExtBuilder::build_default()
        .with_validators()
        .as_externality();
    ext.execute_with(||{
        let mock_data = MockData::setup();
        <UncheckedEvents<TestRuntime>>::append(&(mock_data.event_id.clone(), DEFAULT_INGRESS_COUNTER, 0));

        assert_eq!(EthereumEvents::unchecked_events().len(), 1);
        assert_eq!(EthereumEvents::events_pending_challenge().len(), 0);

        let submit_checkevent_result = EthereumEvents::submit_checkevent_result(
            RawOrigin::None.into(),
            mock_data.eth_event_check_result.clone(),
            DEFAULT_INGRESS_COUNTER,
            mock_data.signature.clone(),
            mock_data.validator.clone()
        );

        assert!(submit_checkevent_result.is_ok());
        assert_eq!(EthereumEvents::unchecked_events().len(), 0);
        assert_eq!(EthereumEvents::events_pending_challenge().len(), 1);

        //MockData::setup() sets the block number to 2
        assert!(EthereumEvents::events_pending_challenge().contains(
            &(mock_data.eth_event_check_result.clone(), DEFAULT_INGRESS_COUNTER, 2)
        ));
        assert!(System::events().iter().any(|a| a.event == mock::Event::pallet_ethereum_events(
            crate::Event::<TestRuntime>::EventValidated(
                mock_data.event_id.clone(),
                mock_data.check_result.clone(),
                mock_data.checked_by.clone())
        )));
    });
}

#[test]
fn submit_checkevent_result_should_return_error_when_request_is_signed() {
    let mut ext = ExtBuilder::build_default()
        .with_validators()
        .as_externality();
    ext.execute_with(||{
        let mock_data = MockData::setup();
        <UncheckedEvents<TestRuntime>>::append(&(mock_data.event_id.clone(), DEFAULT_INGRESS_COUNTER, 0));

        assert_noop!(
            EthereumEvents::submit_checkevent_result(
                Origin::signed(Default::default()),
                mock_data.eth_event_check_result.clone(),
                DEFAULT_INGRESS_COUNTER,
                mock_data.signature.clone(),
                mock_data.validator.clone()
            ),
            DispatchError::BadOrigin
        );
        assert!(!System::events().iter().any(|a| a.event == mock::Event::pallet_ethereum_events(
            crate::Event::<TestRuntime>::EventValidated(
                mock_data.event_id.clone(),
                mock_data.check_result.clone(),
                mock_data.checked_by.clone())
        )));
    });
}

#[test]
fn submit_checkevent_result_should_return_error_when_validator_key_is_invalid() {
    let mut ext = ExtBuilder::build_default()
        .with_validators()
        .as_externality();
    ext.execute_with(||{
        let mock_data = MockData::setup();
        let invalid_validator_key = account_id_0();
        let not_authorised_check_result = EthEventCheckResult::new(
            mock_data.block_number,
            mock_data.check_result.clone(),
            &mock_data.event_id,
            &mock_data.event_data,
            invalid_validator_key,
            mock_data.block_number - 1,
            mock_data.min_challenge_votes
        );
        <UncheckedEvents<TestRuntime>>::append(&(mock_data.event_id.clone(), DEFAULT_INGRESS_COUNTER, 0));

        assert_noop!(
            EthereumEvents::submit_checkevent_result(
                RawOrigin::None.into(),
                not_authorised_check_result,
                DEFAULT_INGRESS_COUNTER,
                mock_data.signature.clone(),
                mock_data.validator.clone()
            ),
            Error::<TestRuntime>::InvalidKey
        );
        assert!(!System::events().iter().any(|a| a.event == mock::Event::pallet_ethereum_events(
            crate::Event::<TestRuntime>::EventValidated(
                mock_data.event_id.clone(),
                mock_data.check_result.clone(),
                mock_data.checked_by.clone())
        )));
    });
}

#[test]
fn submit_checkevent_result_should_return_error_when_event_log_never_been_added() {
    let mut ext = ExtBuilder::build_default()
        .with_validators()
        .as_externality();
    ext.execute_with(||{
        let mock_data = MockData::setup();
        let event_id_not_in_unchecked = EthEventId {
            signature: ValidEvents::AddedValidator.signature(),
            transaction_hash: H256::from([9; 32]),
        };
        let event_check_result_not_in_unchecked = EthEventCheckResult::new(
            mock_data.block_number,
            mock_data.check_result.clone(),
            &event_id_not_in_unchecked,
            &mock_data.event_data,
            mock_data.checked_by.clone(),
            mock_data.block_number - 1,
            mock_data.min_challenge_votes
        );
        <UncheckedEvents<TestRuntime>>::append(&(mock_data.event_id.clone(), DEFAULT_INGRESS_COUNTER, 0));

        assert_noop!(
            EthereumEvents::submit_checkevent_result(
                RawOrigin::None.into(),
                event_check_result_not_in_unchecked.clone(),
                DEFAULT_INGRESS_COUNTER,
                mock_data.signature.clone(),
                mock_data.validator.clone()
            ),
            Error::<TestRuntime>::MissingEventToCheck
        );
        assert!(!System::events().iter().any(|a| a.event == mock::Event::pallet_ethereum_events(
            crate::Event::<TestRuntime>::EventValidated(
                mock_data.event_id.clone(),
                mock_data.check_result.clone(),
                mock_data.checked_by.clone())
        )));
    });
}

#[test]
fn submit_checkevent_result_should_return_error_when_challenge_window_overflow() {
    let mut ext = ExtBuilder::build_default()
        .with_validators()
        .as_externality();
    ext.execute_with(||{
        let mock_data = MockData::setup();
        System::set_block_number(u64::max_value());
        <UncheckedEvents<TestRuntime>>::append(&(mock_data.event_id.clone(), DEFAULT_INGRESS_COUNTER, 0));

        assert_noop!(
            EthereumEvents::submit_checkevent_result(
                RawOrigin::None.into(),
                mock_data.eth_event_check_result.clone(),
                DEFAULT_INGRESS_COUNTER,
                mock_data.signature.clone(),
                mock_data.validator.clone()
            ),
            Error::<TestRuntime>::Overflow
        );
        assert!(!System::events().iter().any(|a| a.event == mock::Event::pallet_ethereum_events(
            crate::Event::<TestRuntime>::EventValidated(
                mock_data.event_id.clone(),
                mock_data.check_result.clone(),
                mock_data.checked_by.clone())
        )));
    });
}

#[test]
fn process_event_should_return_expected_result_when_challenge_fails() {
    let mut ext = ExtBuilder::build_default()
        .with_validators()
        .as_externality();
    ext.execute_with(||{
        let mock_data = MockData::setup();

        // Add a new event, whose checked result is OK, into the EventsPendingChallenge collection
        <EventsPendingChallenge<TestRuntime>>::append((mock_data.eth_event_check_result.clone(), DEFAULT_INGRESS_COUNTER, 0));

        // Set block number to be ready for processing the event
        System::set_block_number(mock_data.eth_event_check_result.ready_for_processing_after_block + 1);

        assert_eq!(EthereumEvents::events_pending_challenge().len(), 1);
        assert!(!<ProcessedEvents>::contains_key(&mock_data.event_id));

        let process_event_result = EthereumEvents::process_event(
            RawOrigin::None.into(),
            mock_data.event_id.clone(),
            DEFAULT_INGRESS_COUNTER,
            mock_data.validator.clone(),
            mock_data.signature.clone()
        );

        assert!(process_event_result.is_ok());
        assert_eq!(EthereumEvents::events_pending_challenge().len(), 0);
        assert!(<ProcessedEvents>::contains_key(&mock_data.event_id));
        assert!(System::events().iter().any(|a| a.event == mock::Event::pallet_ethereum_events(
            crate::Event::<TestRuntime>::EventProcessed(
                mock_data.event_id.clone(),
                mock_data.validator.account_id,
                true)
        )));
        assert!(!System::events().iter().any(|a| a.event == mock::Event::pallet_ethereum_events(
            crate::Event::<TestRuntime>::ChallengeSucceeded(
                mock_data.event_id.clone(),
                mock_data.check_result.clone())
        )));

        // TODO [TYPE: test][PRI: medium]: Test TestRuntime::ProcessedEventHandler::on_event_processed is triggered
    });
}

#[test]
fn process_event_should_return_expected_result_when_challenge_is_successful() {
    let mut ext = ExtBuilder::build_default()
        .with_validators()
        .as_externality();
    ext.execute_with(||{
        let mock_data = MockData::setup();

        let _ = <Challenges<TestRuntime>>::insert(mock_data.event_id.clone(), vec![
            EthereumEvents::validators()[1].account_id.clone(),
            EthereumEvents::validators()[2].account_id.clone()
        ]);

        let invalid_check_result = EthEventCheckResult::new(
            mock_data.block_number,
            CheckResult::Invalid,
            &mock_data.event_id,
            &mock_data.event_data,
            mock_data.checked_by.clone(),
            mock_data.block_number - 1,
            mock_data.min_challenge_votes
        );

        // Add a new event, whose checked result is Invalid, into the EventsPendingChallenge collection
        <EventsPendingChallenge<TestRuntime>>::append((invalid_check_result.clone(), DEFAULT_INGRESS_COUNTER, 0));

        // Set block number to be ready for processing the event
        System::set_block_number(mock_data.eth_event_check_result.ready_for_processing_after_block + 1);

        assert_eq!(EthereumEvents::events_pending_challenge().len(), 1);
        assert!(!<ProcessedEvents>::contains_key(&mock_data.event_id));

        let process_event_result = EthereumEvents::process_event(
            RawOrigin::None.into(),
            mock_data.event_id.clone(),
            DEFAULT_INGRESS_COUNTER,
            mock_data.validator.clone(),
            mock_data.signature.clone()
        );

        assert!(process_event_result.is_ok());
        assert_eq!(EthereumEvents::events_pending_challenge().len(), 0);
        assert!(!<ProcessedEvents>::contains_key(&mock_data.event_id));
        assert!(System::events().iter().any(|a| a.event == mock::Event::pallet_ethereum_events(
            crate::Event::<TestRuntime>::EventProcessed(
                mock_data.event_id.clone(),
                mock_data.validator.account_id,
                false)
        )));
        assert!(System::events().iter().any(|a| a.event == mock::Event::pallet_ethereum_events(
            crate::Event::<TestRuntime>::ChallengeSucceeded(
                mock_data.event_id.clone(),
                CheckResult::Invalid)
        )));
        // TODO [TYPE: test][PRI: high][JIRA: 348]: Test TestRuntime::ProcessedEventHandler::on_event_processed is triggered
        // Test once if possible, in a way that handles this todo and the previous one
    });
}

#[test]
fn process_event_should_return_error_when_request_is_signed() {
    let mut ext = ExtBuilder::build_default()
        .with_validators()
        .as_externality();
    ext.execute_with(||{
        let mock_data = MockData::setup();

        <EventsPendingChallenge<TestRuntime>>::append((mock_data.eth_event_check_result.clone(), DEFAULT_INGRESS_COUNTER, 0));
        System::set_block_number(mock_data.eth_event_check_result.ready_for_processing_after_block);

        assert_noop!(
            EthereumEvents::process_event(
                Origin::signed(Default::default()),
                mock_data.event_id.clone(),
                DEFAULT_INGRESS_COUNTER,
                mock_data.validator,
                mock_data.signature
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn process_event_should_return_error_when_validator_key_is_invalid() {
    let mut ext = ExtBuilder::build_default()
        .with_validators()
        .as_externality();
    ext.execute_with(||{
        let mock_data = MockData::setup();
        let invalid_validator = Validator::new(account_id_0(), UintAuthorityId(0));

        <EventsPendingChallenge<TestRuntime>>::append((mock_data.eth_event_check_result.clone(), DEFAULT_INGRESS_COUNTER, 0));
        System::set_block_number(mock_data.eth_event_check_result.ready_for_processing_after_block);

        assert_noop!(
            EthereumEvents::process_event(
                RawOrigin::None.into(),
                mock_data.event_id.clone(),
                DEFAULT_INGRESS_COUNTER,
                invalid_validator,
                mock_data.signature
            ),
            Error::<TestRuntime>::InvalidKey
        );
    });
}

#[test]
fn process_event_should_return_error_when_event_not_found_in_pending_challenge_event() {
    let mut ext = ExtBuilder::build_default()
        .with_validators()
        .as_externality();
    ext.execute_with(||{
        let mock_data = MockData::setup();

        System::set_block_number(mock_data.eth_event_check_result.ready_for_processing_after_block);

        assert_eq!(EthereumEvents::events_pending_challenge().len(), 0);

        assert_noop!(
            EthereumEvents::process_event(
                RawOrigin::None.into(),
                mock_data.event_id.clone(),
                DEFAULT_INGRESS_COUNTER,
                mock_data.validator,
                mock_data.signature
            ),
            Error::<TestRuntime>::PendingChallengeEventNotFound
        );
    });
}

#[test]
fn process_event_should_return_error_when_event_is_still_in_challenge_window() {
    let mut ext = ExtBuilder::build_default()
        .with_validators()
        .as_externality();
    ext.execute_with(||{
        let mock_data = MockData::setup();

        <EventsPendingChallenge<TestRuntime>>::append((mock_data.eth_event_check_result.clone(), DEFAULT_INGRESS_COUNTER, 0));
        let block_number_within_challenge_window = mock_data.eth_event_check_result.ready_for_processing_after_block - 1;
        System::set_block_number(block_number_within_challenge_window);

        assert_noop!(
            EthereumEvents::process_event(
                RawOrigin::None.into(),
                mock_data.event_id.clone(),
                DEFAULT_INGRESS_COUNTER,
                mock_data.validator,
                mock_data.signature
            ),
            Error::<TestRuntime>::InvalidEventToProcess
        );
    });
}

#[test]
fn validate_unsigned_with_submit_checkevent_result_call_should_return_error_when_event_not_in_unchecked_events() {
    eth_events_test_with_validators().execute_with(||{
        let mock_data = MockData::setup();
        let transaction_call = Call::submit_checkevent_result(
            mock_data.eth_event_check_result.clone(),
            DEFAULT_INGRESS_COUNTER,
            mock_data.signature,
            mock_data.validator
        );

        assert_noop!(
            EthereumEvents::validate_unsigned(TransactionSource::Local, &transaction_call),
            TransactionValidityError::Invalid(InvalidTransaction::Custom(ERROR_CODE_EVENT_NOT_IN_UNCHECKED))
        );
    });
}

#[test]
fn validate_unsigned_with_submit_checkevent_result_call_should_return_error_when_event_data_is_invalid() {
    eth_events_test_with_validators().execute_with(||{
        let mock_data = MockData::setup();
        let invalid_added_validator_data = MockData::get_invalid_added_validator_data();
        let check_result_with_invalid_event_data = EthEventCheckResult::new(
            mock_data.block_number,
            mock_data.check_result,
            &mock_data.event_id,
            &EventData::LogAddedValidator(invalid_added_validator_data),
            mock_data.checked_by,
            mock_data.block_number - 1,
            mock_data.min_challenge_votes
        );
        let transaction_call = Call::submit_checkevent_result(
            check_result_with_invalid_event_data,
            DEFAULT_INGRESS_COUNTER,
            mock_data.signature,
            mock_data.validator
        );
        <UncheckedEvents<TestRuntime>>::append(&(mock_data.event_id.clone(), DEFAULT_INGRESS_COUNTER, 0));

        assert_noop!(
            EthereumEvents::validate_unsigned(TransactionSource::Local, &transaction_call),
            TransactionValidityError::Invalid(InvalidTransaction::Custom(ERROR_CODE_INVALID_EVENT_DATA))
        );
    });
}

#[test]
fn validate_unsigned_with_submit_checkevent_result_call_should_return_error_when_validator_is_not_primary() {
    eth_events_test_with_validators().execute_with(||{
        let mock_data = MockData::setup();
        let block_number = EthereumEvents::validators().len().try_into().unwrap(); // 3 keys in total
        System::set_block_number(block_number);
        let checked_by = EthereumEvents::validators()[2].account_id.clone(); // the 3rd validator
        let check_result_by_non_primary_validator = EthEventCheckResult::new(
            block_number, // the 1st validator is primary
            mock_data.check_result,
            &mock_data.event_id,
            &mock_data.event_data,
            checked_by,
            mock_data.block_number - 1,
            mock_data.min_challenge_votes
        );
        let transaction_call = Call::submit_checkevent_result(
            check_result_by_non_primary_validator,
            DEFAULT_INGRESS_COUNTER,
            mock_data.signature,
            mock_data.validator
        );
        <UncheckedEvents<TestRuntime>>::append(&(mock_data.event_id.clone(), DEFAULT_INGRESS_COUNTER, 0));

        assert_noop!(
            EthereumEvents::validate_unsigned(TransactionSource::Local, &transaction_call),
            TransactionValidityError::Invalid(InvalidTransaction::Custom(ERROR_CODE_VALIDATOR_NOT_PRIMARY))
        );
    });
}

#[test]
fn validate_unsigned_with_submit_checkevent_result_call_should_return_error_when_signature_is_invalid() {
    eth_events_test_with_validators().execute_with(||{
        let mock_data = MockData::setup();
        let transaction_call = Call::submit_checkevent_result(
            mock_data.eth_event_check_result.clone(),
            DEFAULT_INGRESS_COUNTER,
            TestSignature(0, vec![]), // Invalid signature
            mock_data.validator
        );
        <UncheckedEvents<TestRuntime>>::append(&(mock_data.event_id.clone(), DEFAULT_INGRESS_COUNTER, 0));

        assert_noop!(
            EthereumEvents::validate_unsigned(TransactionSource::Local, &transaction_call),
            TransactionValidityError::Invalid(InvalidTransaction::BadProof)
        );
    });
}