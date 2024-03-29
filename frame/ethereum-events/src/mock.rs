// Copyright 2020 Artos Systems (UK) Ltd.

#![cfg(test)]

use std::cell::RefCell;
use sp_core::{H256, crypto::KeyTypeId, sr25519, Pair};
use frame_support::{parameter_types, weights::{Weight}, BasicExternalities};
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup, ConvertInto},
    testing::{Header, UintAuthorityId, TestXt},
    Perbill
};

use frame_system as system;
use pallet_session as session;
use pallet_avn_proxy::ProvableProxy;
use hex_literal::hex;

use parking_lot::{RwLock};
use codec::{
    alloc::sync::{Arc}
};
use sp_core::offchain::{
    OffchainExt,
    TransactionPoolExt,
    testing::{TestOffchainExt, PoolState, OffchainState, TestTransactionPoolExt, PendingRequest},
};
use sp_staking::{SessionIndex, offence::{ReportOffence, OffenceError}};

use sp_io::TestExternalities;
use sp_avn_common::event_types::EthEvent;
use avn::FinalisedBlockChecker;

use crate::{self as pallet_ethereum_events, *};

#[allow(dead_code)]
pub type Signature = sr25519::Signature;
pub type AccountId = <Signature as Verify>::Signer;
pub type BlockNumber = <TestRuntime as system::Config>::BlockNumber;
pub type AuthorityId = <TestRuntime as avn::Config>::AuthorityId;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;
type Block = frame_system::mocking::MockBlock<TestRuntime>;

frame_support::construct_runtime!(
    pub enum TestRuntime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Module, Call, Config, Storage, Event<T>},
        Session: pallet_session::{Module, Call, Storage, Event, Config<T>},
        Balances: pallet_balances::{Module, Call, Storage, Config<T>, Event<T>},
        AVN: pallet_avn::{Module, Storage},
        AvnProxy: pallet_avn_proxy::{Module, Call, Storage, Event<T>},
        EthereumEvents: pallet_ethereum_events::{Module, Call, Storage, Event<T>, Config<T>},
    }
);

pub fn account_id_0() -> AccountId { TestAccount::new([0u8; 32]).account_id() }
pub fn validator_id_1() -> AccountId { TestAccount::new([1u8; 32]).account_id() }
pub fn validator_id_2() -> AccountId { TestAccount::new([2u8; 32]).account_id() }
pub fn validator_id_3() -> AccountId { TestAccount::new([3u8; 32]).account_id() }
pub fn account_id_1() -> AccountId { validator_id_1() }
pub fn checked_by() -> AccountId { TestAccount::new([10u8; 32]).account_id() }

// TODO: Refactor this struct to be reused in all tests
pub struct TestAccount {
    pub seed: [u8; 32]
}

impl TestAccount {
    pub fn new(seed: [u8; 32]) -> Self {
        TestAccount {
            seed: seed
        }
    }

    pub fn account_id(&self) -> AccountId {
        return AccountId::decode(&mut self.key_pair().public().to_vec().as_slice()).unwrap();
    }

    pub fn key_pair(&self) -> sr25519::Pair {
        return sr25519::Pair::from_seed(&self.seed);
    }
}

pub fn sign(signer: &sr25519::Pair, message_to_sign: &[u8]) -> Signature {
    return Signature::from(signer.sign(message_to_sign));
}

pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"dumy");

pub const GOOD_STATUS: &str = "0x1";
pub const GOOD_BLOCK_CONFIRMATIONS: u64 = 2;
pub const QUORUM_FACTOR: u32 = 3;
pub const EVENT_CHALLENGE_PERIOD: BlockNumber = 2;
pub const EXISTENTIAL_DEPOSIT: u64 = 0;

pub mod crypto {
    use super::KEY_TYPE;
    use sp_runtime::app_crypto::{app_crypto, sr25519};
    app_crypto!(sr25519, KEY_TYPE);
}

pub type Extrinsic = TestXt<Call, ()>;

impl Config for TestRuntime {
    type Call = Call;
    type Event = Event;
    type ProcessedEventHandler = Self;
    type MinEthBlockConfirmation = MinEthBlockConfirmation;
    type ReportInvalidEthereumLog = OffenceHandler;
    type Public = AccountId;
    type Signature = Signature;
    type WeightInfo = ();
}

impl<LocalCall> frame_system::offchain::SendTransactionTypes<LocalCall> for TestRuntime where
	Call: From<LocalCall>,
{
	type OverarchingCall = Call;
	type Extrinsic = Extrinsic;
}


parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub const MinEthBlockConfirmation: u64 = 2;
}

impl system::Config for TestRuntime {
    type BaseCallFilter = ();
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type Origin = Origin;
    type Call = Call;
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = Event;
    type BlockHashCount = BlockHashCount;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<u128>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
}

parameter_types! {
    pub const Period: u64 = 1;
    pub const Offset: u64 = 0;
    pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(33);
}

thread_local! {
    // validator accounts (aka public addresses, public keys-ish)
    pub static VALIDATORS: RefCell<Option<Vec<AccountId>>> = RefCell::new(Some(vec![
        validator_id_1(),
        validator_id_2(),
        validator_id_3(),
    ]));

    pub static PROCESS_EVENT_SUCCESS: RefCell<bool> = RefCell::new(true);
}

impl avn::Config for TestRuntime {
    type AuthorityId = UintAuthorityId;
    type EthereumPublicKeyChecker = ();
    type NewSessionHandler = ();
    type DisabledValidatorChecker = ();
    type FinalisedBlockChecker = Self;
}

pub struct TestSessionManager;
impl session::SessionManager<AccountId> for TestSessionManager {
    fn new_session(_new_index: SessionIndex) -> Option<Vec<AccountId>> {
        VALIDATORS.with(|l| l.borrow_mut().take())
    }
    fn end_session(_: SessionIndex) {}
    fn start_session(_: SessionIndex) {}
}

impl session::Config for TestRuntime {
    type SessionManager = pallet_session::historical::NoteHistoricalRoot<TestRuntime, TestSessionManager>;
    type Keys = UintAuthorityId;
    type ShouldEndSession = session::PeriodicSessions<Period, Offset>;
    type SessionHandler = (AVN, );
    type Event = Event;
    type ValidatorId = AccountId;
    type ValidatorIdOf = ConvertInto;
    type DisabledValidatorsThreshold = DisabledValidatorsThreshold;
    type NextSessionRotation = session::PeriodicSessions<Period, Offset>;
    type WeightInfo = ();
}

parameter_types! {
    pub const ExistentialDeposit: u64 = EXISTENTIAL_DEPOSIT;
}

impl pallet_balances::Config for TestRuntime {
    type MaxLocks = ();
    type Balance = u128;
    type DustRemoval = ();
    type Event = Event;
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
}

impl pallet_session::historical::Config for TestRuntime {
    type FullIdentification = AccountId;
    type FullIdentificationOf = ConvertInto;
}

impl pallet_session::historical::SessionManager<AccountId, AccountId> for TestSessionManager {
    fn new_session(_new_index: SessionIndex) -> Option<Vec<(AccountId, AccountId)>> {
        VALIDATORS.with(|l| l
            .borrow_mut()
            .take()
            .map(|validators| {
                validators.iter().map(|v| (*v, *v)).collect()
            })
        )
    }
    fn end_session(_: SessionIndex) {}
    fn start_session(_: SessionIndex) {}
}

impl ProcessedEventHandler for TestRuntime {
    fn on_event_processed(_event: &EthEvent) -> DispatchResult {
        match PROCESS_EVENT_SUCCESS.with(|pk| {*pk.borrow()}) {
            true => return Ok(()),
            _ => Err(Error::<TestRuntime>::InvalidEventToProcess)?
        }
    }
}

impl FinalisedBlockChecker<BlockNumber> for TestRuntime {
    fn is_finalised(_block_number: BlockNumber) -> bool { true }
}

/// An extrinsic type used for tests.
type IdentificationTuple = (AccountId, AccountId);
type Offence = crate::InvalidEthereumLogOffence<IdentificationTuple>;

thread_local! {
    pub static OFFENCES: RefCell<Vec<(Vec<AccountId>, Offence)>> = RefCell::new(vec![]);
}

/// A mock offence report handler.
pub struct OffenceHandler;
impl ReportOffence<AccountId, IdentificationTuple, Offence> for OffenceHandler {
    fn report_offence(reporters: Vec<AccountId>, offence: Offence) -> Result<(), OffenceError> {
        OFFENCES.with(|l| l.borrow_mut().push((reporters, offence)));
        Ok(())
    }

    fn is_known_offence(_offenders: &[IdentificationTuple], _time_slot: &SessionIndex) -> bool {
        false
    }
}

pub static CUSTOM_VALIDATOR_MANAGER_CONTRACT: H160 = H160(hex!("11111AAAAA22222BBBBB11111AAAAA22222BBBBB"));
pub static CUSTOM_LIFTING_CONTRACT: H160 = H160(hex!("33333CCCCC44444DDDDD33333CCCCC44444DDDDD"));

#[allow(dead_code)]
pub const INDEX_RESULT: usize = 2;
#[allow(dead_code)]
pub const INDEX_RESULT_LOGS: usize = 9;
#[allow(dead_code)]
pub const INDEX_RESULT_STATUS: usize = 10;
#[allow(dead_code)]
pub const INDEX_EVENT_ADDRESS: usize = 5;
#[allow(dead_code)]
pub const INDEX_DATA: usize = 6;
#[allow(dead_code)]
pub const INDEX_TOPICS: usize = 7;

pub const DEFAULT_INGRESS_COUNTER: IngressCounter = 100;
pub const FIRST_INGRESS_COUNTER: IngressCounter = 1;
pub const DEFAULT_BLOCK: u64 = 1;
pub const CHECKED_AT_BLOCK: u64 = 0;
pub const MIN_CHALLENGE_VOTES: u32 = 1;

impl EthereumEvents {

    pub fn has_events_to_check() -> bool {
        return <UncheckedEvents<TestRuntime>>::get().is_empty() == false;
    }

    pub fn setup_mock_ethereum_contracts_address() {
        <ValidatorManagerContractAddress>::put(CUSTOM_VALIDATOR_MANAGER_CONTRACT);
        <LiftingContractAddress>::put(CUSTOM_LIFTING_CONTRACT);
    }

    pub fn set_ingress_counter(new_value: IngressCounter) {
        <TotalIngresses>::put(new_value);
    }

    pub fn insert_to_unchecked_events(to_insert: &EthEventId, ingress_counter: IngressCounter){
        <UncheckedEvents<TestRuntime>>::append((to_insert.clone(), ingress_counter, 0));
        Self::set_ingress_counter(ingress_counter);
    }

    pub fn populate_events_pending_challenge(checked_by: &AccountId, num_of_events: u8) -> IngressCounter {
        let from = Self::events_pending_challenge().len() as u8;
        let to = from + num_of_events;
        let block_number = EVENT_CHALLENGE_PERIOD;
        let min_challenge_votes = 0;

        for i in from..to {
            let ingress_counter = (i + 1) as IngressCounter;
            Self::insert_to_events_pending_challenge(
                block_number,
                CheckResult::Unknown,
                &Self::get_event_id(i),
                ingress_counter,
                &EventData::EmptyEvent,
                checked_by.clone(),
                block_number - 1,
                min_challenge_votes
            );
        }
        // returns the first ingress counter
        return (from + 1) as IngressCounter;
    }

    pub fn insert_to_events_pending_challenge_compact(
        block_number: u64,
        event_info: &EthEventCheckResult<<TestRuntime as system::Config>::BlockNumber, AuthorityId>,
        checked_by: AccountId)
    {
        Self::insert_to_events_pending_challenge(
            block_number,
            event_info.result.clone(),
            &event_info.event.event_id,
            DEFAULT_INGRESS_COUNTER,
            &event_info.event.event_data,
            checked_by.clone(),
            block_number + 4,
            20
        );
    }

    pub fn insert_to_events_pending_challenge(
        id: u64,
        result: CheckResult,
        event_id: &EthEventId,
        ingress_counter: u64,
        event_data: &EventData,
        checked_by: AccountId,
        checked_at_block: u64,
        min_challenge_votes: u32)
    {
        let to_insert = EthEventCheckResult::new(id, result, &event_id, event_data, checked_by, checked_at_block, min_challenge_votes);
        <EventsPendingChallenge<TestRuntime>>::append((to_insert, ingress_counter, 0));
    }

    pub fn get_event_id(seed: u8) -> EthEventId {
        return EthEventId {
            signature: ValidEvents::AddedValidator.signature(),
            transaction_hash: H256::from([seed; 32]),
        };
    }

    pub fn insert_to_processed_events(to_insert: &EthEventId){
        <ProcessedEvents>::insert(to_insert.clone(), true);
    }

    pub fn has_events_to_validate() -> bool {
        return !<EventsPendingChallenge<TestRuntime>>::get().is_empty();
    }

    pub fn validators() -> Vec<Validator<AuthorityId, AccountId>> {
        return AVN::active_validators();
    }

    pub fn is_primary(
        block_number: <TestRuntime as system::Config>::BlockNumber,
        validator: &AccountId) -> Result<bool, avn_error<TestRuntime>>
    {
        return AVN::is_primary(block_number, validator);
    }

    pub fn get_validator_for_current_node() -> Option<Validator<AuthorityId, AccountId>> {
        return AVN::get_validator_for_current_node();
    }

    pub fn event_emitted(event: &Event) -> bool {
        return System::events().iter().any(|a| a.event == *event);
    }
}

impl pallet_avn_proxy::Config for TestRuntime {
    type Event = Event;
    type Call = Call;
    type Currency = Balances;
    type Public = AccountId;
    type Signature = Signature;
    type ProxyConfig = TestAvnProxyConfig;
    type WeightInfo = ();
}

// Test Avn proxy configuration logic
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Encode, Decode, Debug)]
pub struct TestAvnProxyConfig { }
impl Default for TestAvnProxyConfig { fn default() -> Self { TestAvnProxyConfig { } }}

impl ProvableProxy<Call, Signature, AccountId> for TestAvnProxyConfig {
    fn get_proof(call: &Call) -> Option<Proof<Signature, AccountId>> {
        match call {
            Call::EthereumEvents(pallet_ethereum_events::Call::signed_add_ethereum_log(proof, _, _)) => return Some(proof.clone()),
            _ => None
        }
    }
}

impl InnerCallValidator for TestAvnProxyConfig {
    type Call = Call;

    fn signature_is_valid(call: &Box<Self::Call>) -> bool {
        match **call {
            Call::EthereumEvents(..) => {
                return EthereumEvents::signature_is_valid(call);
            },
            _ => false
        }
    }
}

// TODO [TYPE: test refactoring][PRI: low]: remove this function, when tests in session_handler_tests and test_challenges are fixed
#[allow(dead_code)]
pub fn eth_events_test_with_validators() -> TestExternalities {

    let mut ext = ExtBuilder::build_default()
        .with_validators()
        .for_offchain_worker()
        .as_externality();

    ext.execute_with(|| System::set_block_number(1));
    return ext;
}

#[allow(dead_code)]
pub fn keys_setup_return_good_validator() -> Validator<AuthorityId, AccountId> {
    let validators = EthereumEvents::validators(); // Validators are tuples (UintAuthorityId(int), int)
    assert_eq!(validators[0], Validator {account_id: validator_id_1(), key:UintAuthorityId(0)});
    assert_eq!(validators[2], Validator {account_id: validator_id_3(), key:UintAuthorityId(2)});
    assert_eq!(validators.len(), 3);

    // AuthorityId type for TestRuntime is UintAuthorityId
    let keys: Vec<UintAuthorityId> = validators.into_iter().map(|v| v.key).collect();
    UintAuthorityId::set_all_keys(keys); // Keys in the setup are either () or (1,2,3). See VALIDATORS.
    let current_node_validator = EthereumEvents::get_validator_for_current_node().unwrap(); // filters validators() to just those corresponding to this validator
    assert_eq!(current_node_validator.key, UintAuthorityId(0));
    assert_eq!(current_node_validator.account_id, validator_id_1());

    assert_eq!(current_node_validator, Validator {
        account_id: validator_id_1(),
        key: UintAuthorityId(0)
    });

    return current_node_validator;
}

#[allow(dead_code)]
pub fn bad_authority() -> Validator<AuthorityId, AccountId> {
    let validator = Validator {
        account_id: TestAccount::new([0u8; 32]).account_id(),
        key: UintAuthorityId(0),
    };

    return validator;
}

#[allow(dead_code)]
pub fn test_json(tx_hash: &H256, event_signature: &H256, contract_address: &H160, log_data: &str, event_topics: &str, status: &str, num_confirmations: u64) -> Vec<u8> {
    let json = format!("
    {{
        \"id\": 1,
        \"jsonrpc\": \"2.0\",
        \"result\": {{
            \"transactionHash\": \"{}\",
            \"transactionIndex\": \"0x0\",
            \"blockHash\": \"0x5536c9e671fe581fe4ef4631112038297dcdecae163e8724c281ece8ad94c8c3\",
            \"blockNumber\": \"0x2e\",
            \"from\": \"0x3a629a342f842d2e548a372742babf288816da4e\",
            \"to\": \"0x604dd282e3fbe35f40f84405f90965821483827f\",
            \"gasUsed\": \"0x6a4b\",
            \"cumulativeGasUsed\": \"0x6a4b\",
            \"contractAddress\": null,
            \"logs\": [
                {{
                    \"logIndex\": \"0x0\",
                    \"transactionIndex\": \"0x0\",
                    \"transactionHash\": \"0x9ad4d46054b0495fa38e8418263c6107ecb4ffd879675372613edf39af898dcb\",
                    \"blockHash\": \"0x5536c9e671fe581fe4ef4631112038297dcdecae163e8724c281ece8ad94c8c3\",
                    \"blockNumber\": \"0x2e\",
                    \"address\": \"{}\",
                    \"data\": \"{}\",
                    \"topics\": [
                        \"{}\",
                        \"{}\"

                    ],
                    \"type\": \"mined\"
                }}
            ],
            \"status\": \"{}\",
            \"logsBloom\": \"0x00000100000000000000000000000000000000000000000000000000000100000000000000000000000000000000400000000000000000000000010000000000000000000000000000000000000000000000000001020000000000000000040000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000100000000000000000000000000000000000000000000000000001000000000000000000000000000000000800000000000000000\",
            \"v\": \"0x1c\",
            \"r\": \"0x8823b54a06401fed57e03ac54b1a4cf81091dc1e44192b9a87ce4f4b9c56d454\",
            \"s\": \"0x842e06a5258c4337148bc677f0b5ca343a8dfda597fb92f540ce443fd2bf340\"
        }},
        \"num_confirmations\": {}
    }}
    ",
        format!("{:?}", tx_hash),
        format!("{:?}", contract_address),
        log_data,
        format!("{:?}", event_signature),
        event_topics,
        status,
        num_confirmations
    );
    return json.into_bytes();
}

#[allow(dead_code)]
pub fn inject_ethereum_node_response(state: &mut OffchainState, tx_hash: &H256, expected_response: Option<Vec<u8>>) {
    state.expect_request(PendingRequest {
        method: "GET".into(),
        uri: format!("http://127.0.0.1:2020/eth/events/{:?}",tx_hash).into(),
        response: expected_response,
        headers: vec![],
        sent: true,
        ..Default::default()
    });
}

pub fn simulate_http_response(
    offchain_state: &Arc<RwLock<OffchainState>>,
    unchecked_event: &EthEventId,
    status: &str,
    confirmations: u64) {

    let log_data = "0x0000000000000000000000000000000000000000000000000000000005f5e100";
    let event_topics = "0x00000000000000000000000023aaf097c241897060c0a6b8aae61af5ea48cea3\",
                      \"0x689d5b000758030ea25304346869b002a345e7647ec5784b8af986e24e971303\",
                      \"0x0000000000000000000000000000000000000000000000000000000000000001";
    inject_ethereum_node_response(
        &mut offchain_state.write(),
        &unchecked_event.transaction_hash,
        Some(
            test_json(
                &unchecked_event.transaction_hash,
                &unchecked_event.signature,
                &EthereumEvents::validator_manager_contract_address(),
                log_data,
                event_topics,
                status,
                confirmations
            )
        )
    );
}


// ==========================================================

pub const VALIDATORS_MANAGER_CONTRACT: [u8;20] = [8u8;20];
pub const LIFTING_CONTRACT: [u8;20] = [9u8;20];
pub static NFT_CONTRACT: [u8;20] = [10u8;20];

pub const INITIAL_LIFTS: [[u8;32];4] = [
    [10u8;32],
    [11u8;32],
    [12u8;32],
    [13u8;32]
];

pub const INITIAL_PROCESSED_EVENTS: [[u8;32];3] = [
    [15u8;32],
    [16u8;32],
    [17u8;32]
];

pub fn create_initial_processed_events() -> Vec<(EthEventId, bool)> {
    let initial_processed_events = INITIAL_PROCESSED_EVENTS.iter().map(|x| {
        (
            EthEventId {
                signature: ValidEvents::AddedValidator.signature(),
                transaction_hash: H256::from(x),
            },
            true
        )
    }).collect::<Vec<(EthEventId, bool)>>();
    assert_eq!(INITIAL_PROCESSED_EVENTS.len(), initial_processed_events.len());
    return initial_processed_events;
}

pub struct ExtBuilder {
    storage: sp_runtime::Storage,
    offchain_state: Option<Arc<RwLock<OffchainState>>>,
    pool_state: Option<Arc<RwLock<PoolState>>>,
    txpool_extension: Option<TestTransactionPoolExt>,
    offchain_extension: Option<TestOffchainExt>,
    offchain_registered: bool,
}

#[allow(dead_code)]
impl ExtBuilder {
    pub fn build_default() -> Self {
        let storage = pallet_ethereum_events::GenesisConfig::<TestRuntime>{
            quorum_factor: QUORUM_FACTOR,
            event_challenge_period: EVENT_CHALLENGE_PERIOD,
            ..Default::default()
        }.build_storage().unwrap();

        Self {
            storage: storage,
            pool_state: None,
            offchain_state: None,
            txpool_extension: None,
            offchain_extension: None,
            offchain_registered: false,
        }
    }

    #[allow(dead_code)]
    pub fn with_genesis_config(mut self) -> Self {
        let _ = pallet_ethereum_events::GenesisConfig::<TestRuntime> {
            validator_manager_contract_address: H160::from(VALIDATORS_MANAGER_CONTRACT),
            lifting_contract_address: H160::from(LIFTING_CONTRACT),
            nft_t1_contracts: vec![(H160::from(NFT_CONTRACT), ())],
            processed_events: vec![],
            lift_tx_hashes: vec![],
            quorum_factor: QUORUM_FACTOR,
            event_challenge_period: EVENT_CHALLENGE_PERIOD,
        }.assimilate_storage(&mut self.storage);
        self
    }

    pub fn with_genesis_and_initial_lifts(mut self) -> Self {
        let _ = pallet_ethereum_events::GenesisConfig::<TestRuntime> {
            validator_manager_contract_address: H160::from(VALIDATORS_MANAGER_CONTRACT),
            lifting_contract_address: H160::from(LIFTING_CONTRACT),
            nft_t1_contracts: vec![(H160::from(NFT_CONTRACT), ())],
            processed_events: create_initial_processed_events(),
            lift_tx_hashes: vec![
                H256::from(INITIAL_LIFTS[0]),
                H256::from(INITIAL_LIFTS[1]),
                H256::from(INITIAL_LIFTS[2]),
                H256::from(INITIAL_LIFTS[3]),
            ],
            quorum_factor: QUORUM_FACTOR,
            event_challenge_period: EVENT_CHALLENGE_PERIOD,
        }.assimilate_storage(&mut self.storage);
        self
    }

    pub fn invalid_config_with_zero_validator_threshold(mut self) -> Self {
        let _ = pallet_ethereum_events::GenesisConfig::<TestRuntime> {
            quorum_factor: 0,
            event_challenge_period: EVENT_CHALLENGE_PERIOD,
            ..Default::default()
        }.assimilate_storage(&mut self.storage);
        self
    }

    #[allow(dead_code)]
    pub fn with_validators(mut self) -> Self {
        let validators: Vec<AccountId> = VALIDATORS.with(|l| l.borrow_mut().take().unwrap());

        BasicExternalities::execute_with_storage(&mut self.storage, || {
            for ref k in &validators {
                frame_system::Module::<TestRuntime>::inc_providers(k);
            }
        });

        let _ = pallet_session::GenesisConfig::<TestRuntime> {
            keys: validators
                .into_iter()
                .enumerate()
                .map(|(i, v)| (v, v, UintAuthorityId((i as u32).into())))
            .collect(),
        }.assimilate_storage(&mut self.storage);
        self
    }

    #[allow(dead_code)]
    pub fn for_offchain_worker(mut self) -> Self {
        assert!(!self.offchain_registered);
        let (offchain, offchain_state) = TestOffchainExt::new();
        let (pool, pool_state) = TestTransactionPoolExt::new();
        self.txpool_extension = Some(pool);
        self.offchain_extension = Some(offchain);
        self.pool_state = Some(pool_state);
        self.offchain_state = Some(offchain_state);
        self.offchain_registered = true;
        self
    }

    #[allow(dead_code)]
    pub fn as_externality(self) -> sp_io::TestExternalities {
        let mut ext = sp_io::TestExternalities::from(self.storage);
        // Events do not get emitted on block 0, so we increment the block here
        ext.execute_with(|| System::set_block_number(1));
        ext
    }

    #[allow(dead_code)]
    pub fn as_externality_with_state(self) -> (
        TestExternalities,
        Arc<RwLock<PoolState>>,
        Arc<RwLock<OffchainState>>
    ) {
        assert!(self.offchain_registered);
        let mut ext = sp_io::TestExternalities::from(self.storage);
        ext.register_extension(OffchainExt::new(self.offchain_extension.unwrap()));
        ext.register_extension(TransactionPoolExt::new(self.txpool_extension.unwrap()));
        assert!(self.pool_state.is_some());
        assert!(self.offchain_state.is_some());
        ext.execute_with(|| System::set_block_number(1));
        (ext, self.pool_state.unwrap(), self.offchain_state.unwrap())
    }
}
