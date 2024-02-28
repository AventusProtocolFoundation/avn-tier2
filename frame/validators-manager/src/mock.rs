//Copyright 2020 Artos Systems (UK) Ltd.

use crate::{self as validators_manager, *};
use crate::extension_builder::ExtBuilder;
use frame_support::{parameter_types, BasicExternalities, traits::{OnFinalize, OnInitialize}};
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup, ConvertInto},
    testing::{Header, UintAuthorityId, TestXt},
    Perbill,
    curve::PiecewiseLinear,
};
use sp_avn_common::{
    event_types::{EthEvent, EthEventId, ValidEvents, AddedValidatorData},
    avn_tests_helpers::ethereum_converters::*
};
use sp_core::{H256, Public, sr25519, Pair, offchain::testing::{OffchainState, PendingRequest}};
use std::cell::RefCell;
use pallet_timestamp as timestamp;
use hex_literal::hex;
use hex::FromHex;
pub use pallet_staking::{self as staking, EraIndex, StakerStatus};
use pallet_balances as balances;
use sp_staking::{SessionIndex, offence::{ReportOffence, OffenceError}};
use avn::FinalisedBlockChecker;
use sp_avn_common::event_types::EventData;
use pallet_avn_proxy::ProvableProxy;

pub fn validator_id_1() -> AccountId { TestAccount::new([1u8; 32]).account_id() }
pub fn validator_id_2() -> AccountId { TestAccount::new([2u8; 32]).account_id() }
pub fn validator_id_3() -> AccountId { TestAccount::new([3u8; 32]).account_id() }
pub fn validator_id_4() -> AccountId { TestAccount::new([4u8; 32]).account_id() }
pub fn validator_id_5() -> AccountId { TestAccount::new([5u8; 32]).account_id() }
pub fn non_validator_id() -> AccountId { TestAccount::new([100u8; 32]).account_id() }
pub fn sender() -> AccountId { validator_id_3() }
pub fn genesis_config_initial_validators() -> [AccountId; 5] { [
    validator_id_1(), validator_id_2(), validator_id_3(),validator_id_4(),validator_id_5()
] }

pub const REGISTERING_VALIDATOR_TIER1_ID: u128 = 200;
pub const EXISTENTIAL_DEPOSIT: u64 = 0;

const MOCK_ETH_PUBLIC_KEY: &str = "026f39ae48cacc934a04e0ee8b8e34d5d17ef4d85f93951c32ae15c91ea3b48a7d";
const MOCK_T2_PUBLIC_KEY_BYTES: [u8; 32] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 185, 106, 148, 112, 79, 84, 62, 242];

pub const INIT_TIMESTAMP: u64 = 30_000;
pub const BLOCK_TIME: u64 = 1000;
pub const VALIDATOR_STAKE: u128 = 5_000_000_000_000_000_000_000u128; //5000AVT
pub const USER_STAKE: u128 = 100_000_000_000_000_000_000u128; //100AVT

pub type Extrinsic = TestXt<Call, ()>;
pub type BlockNumber = <TestRuntime as system::Config>::BlockNumber;
pub type ValidatorId = <TestRuntime as session::Config>::ValidatorId;
pub type FullIdentification = AccountId;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;
type Block = frame_system::mocking::MockBlock<TestRuntime>;
pub type Signature = sr25519::Signature;
pub type AccountId = <Signature as Verify>::Signer;

// TODO: Refactor this struct to be reused in all tests
#[derive(Clone)]
pub struct TestAccount {
    pub seed: [u8; 32]
}

impl TestAccount {
    pub fn new(seed: [u8; 32]) -> Self {
        TestAccount {
            seed: seed
        }
    }

    pub fn from_bytes(seed: &[u8]) -> Self {
        let mut seed_bytes: [u8; 32] = Default::default();
        seed_bytes.copy_from_slice(&seed[0..32]);
        TestAccount {
            seed: seed_bytes
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

frame_support::construct_runtime!(
    pub enum TestRuntime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Module, Call, Config, Storage, Event<T>},
        Session: pallet_session::{Module, Call, Storage, Event, Config<T>},
        Staking: pallet_staking::{Module, Call, Storage, Config<T>, Event<T>},
        Balances: pallet_balances::{Module, Call, Storage, Config<T>, Event<T>},
        Timestamp: pallet_timestamp::{Module, Call, Storage, Inherent},
        AVN: pallet_avn::{Module, Storage},
        AvnProxy: pallet_avn_proxy::{Module, Call, Storage, Event<T>},
        ValidatorManager: validators_manager::{Module, Call, Storage, Event<T>, Config<T>},
    }
);

use frame_system as system;
use pallet_session as session;

impl ValidatorManager {
    pub fn insert_pending_approval(action_id: &ActionId<AccountId>) {
        <<ValidatorManager as Store>::PendingApprovals>::insert(action_id.action_account_id, action_id.ingress_counter);
    }

    pub fn remove_pending_approval(action_id: &ActionId<AccountId>) {
        <<ValidatorManager as Store>::PendingApprovals>::remove(action_id.action_account_id);
    }

    pub fn get_voting_session_for_deregistration(action_id: &ActionId<AccountId>) -> VotingSessionData<AccountId, BlockNumber> {
        <ValidatorManager as Store>::VotesRepository::get(action_id)
    }

    pub fn create_voting_session(
        action_id: &ActionId<AccountId>,
        quorum: u32,
        voting_period_end: u64,
    ) {
        <<ValidatorManager as Store>::VotesRepository>::insert(
            action_id,
            VotingSessionData::new(action_id.encode(), quorum, voting_period_end, 0),
        );
    }

    pub fn insert_validators_action_data(action_id: &ActionId<AccountId>, reserved_eth_tx: EthTransactionType) {
        <<ValidatorManager as Store>::ValidatorActions>::insert(
            action_id.action_account_id,
            action_id.ingress_counter,
            ValidatorsActionData::new(
                ValidatorsActionStatus::AwaitingConfirmation,
                sender(),
                INITIAL_TRANSACTION_ID,
                ValidatorsActionType::Resignation,
                reserved_eth_tx
            )
        );
    }

    pub fn remove_voting_session(action_id: &ActionId<AccountId>) {
        <<ValidatorManager as Store>::VotesRepository>::remove(action_id);
    }

    pub fn record_approve_vote(action_id: &ActionId<AccountId>, voter: AccountId) {
        <<ValidatorManager as Store>::VotesRepository>::mutate(action_id, |vote| vote.ayes.push(voter));
    }

    pub fn record_reject_vote(action_id: &ActionId<AccountId>, voter: AccountId) {
        <<ValidatorManager as Store>::VotesRepository>::mutate(action_id, |vote| vote.nays.push(voter));
    }

    pub fn event_emitted(event: &Event) -> bool {
        return System::events().iter().any(|a| a.event == *event);
    }

    pub fn create_mock_identification_tuple(account_id: AccountId) -> (AccountId, AccountId) {
        return (account_id, account_id);
    }

    pub fn emitted_event_for_offence_of_type(offence_type: ValidatorOffenceType) -> bool {
        return System::events().iter().any(|e|
            Self::event_matches_offence_type(&e.event, offence_type.clone()));
    }

    pub fn event_matches_offence_type(event: &Event, this_type: ValidatorOffenceType) -> bool {
        return matches!(event,
            Event::validators_manager(
                crate::Event::<TestRuntime>::OffenceReported(offence_type, _)
            )
            if this_type == *offence_type
        );
    }

    pub fn get_offence_record() -> Vec<(Vec<ValidatorId>, Offence)> {
        return OFFENCES.with(|o| o.borrow().to_vec());
    }

    pub fn offence_reported(
            reporter: AccountId,
            validator_count: u32,
            offenders: Vec<ValidatorId>,
            offence_type: ValidatorOffenceType
    ) -> bool {
        let offences = Self::get_offence_record();

        return offences.iter().any(|o|
            Self::offence_matches_criteria(
                o,
                vec![reporter],
                validator_count,
                offenders.iter().map(|v| Self::create_mock_identification_tuple(*v)).collect(),
                offence_type.clone()
            ));
    }

    fn offence_matches_criteria(
            this_report: &(Vec<ValidatorId>, Offence),
            these_reporters: Vec<ValidatorId>,
            this_count: u32,
            these_offenders: Vec<(ValidatorId, FullIdentification)>,
            this_type: ValidatorOffenceType,
    ) -> bool {
        return matches!(
            this_report,
            (
                reporters,
                ValidatorOffence {
                    session_index: _,
                    validator_set_count,
                    offenders,
                    offence_type}
            )
            if these_reporters == *reporters
            && this_count == *validator_set_count
            && these_offenders == *offenders
            && this_type == *offence_type
        );
    }
}

parameter_types! {
    pub const VotingPeriod: u64 = 2;
    pub const ValidatorsManagerModuleId: ModuleId = ModuleId(*b"av/vamgr");
}

impl Config for TestRuntime {
    type Call = Call;
    type Event = Event;
    type ProcessedEventsChecker = Self;
    type VotingPeriod = VotingPeriod;
    type AccountToBytesConvert = AVN;
    type CandidateTransactionSubmitter = Self;
    type ReportValidatorOffence = OffenceHandler;
    type ValidatorRegistrationNotifier = Self;
    type ModuleId = ValidatorsManagerModuleId;
    type Public = AccountId;
    type Signature = Signature;
    type WeightInfo = ();
}

impl<LocalCall> system::offchain::SendTransactionTypes<LocalCall> for TestRuntime
where
    Call: From<LocalCall>,
{
    type OverarchingCall = Call;
    type Extrinsic = Extrinsic;
}

parameter_types! {
    pub const BlockHashCount: u64 = 250;
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
    type AccountData = balances::AccountData<u128>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
}

impl avn::Config for TestRuntime {
    type AuthorityId = UintAuthorityId;
    type EthereumPublicKeyChecker = Self;
    type NewSessionHandler = ValidatorManager;
    type DisabledValidatorChecker = ValidatorManager;
    type FinalisedBlockChecker = Self;
}

parameter_types! {
    pub const ExistentialDeposit: u64 = EXISTENTIAL_DEPOSIT;
}

impl balances::Config for TestRuntime {
    type MaxLocks = ();
    type Balance = u128;
    type DustRemoval = ();
    type Event = Event;
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
}

parameter_types! {
	pub const MinimumPeriod: u64 = 3;
}

impl timestamp::Config for TestRuntime {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
}

pallet_staking_reward_curve::build! {
	const REWARD_CURVE: PiecewiseLinear<'static> = curve!(
		min_inflation: 0_025_000u64,
		max_inflation: 0_100_000,
		ideal_stake: 0_500_000,
		falloff: 0_050_000,
		max_piece_count: 40,
		test_precision: 0_005_000,
	);
}

impl staking::SessionInterface<AccountId> for TestRuntime {
    fn disable_validator(validator: &AccountId) -> Result<bool, ()> {
        return <session::Module<TestRuntime>>::disable(validator);
    }

    fn validators() -> Vec<AccountId> {
        return <session::Module<TestRuntime>>::validators();
    }

    fn prune_historical_up_to(_up_to: SessionIndex) { }
}

    parameter_types! {
    pub const SessionsPerEra: SessionIndex = 3;
    pub const BondingDuration: EraIndex = 3;
    pub const SlashDeferDuration: EraIndex = 0;
    pub const AttestationPeriod: u64 = 100;
    pub const RewardCurve: &'static PiecewiseLinear<'static> = &REWARD_CURVE;
    pub const MaxNominatorRewardedPerValidator: u32 = 256;
    pub const ElectionLookahead: u64 = 0;
    pub const StakingUnsignedPriority: u64 = u64::max_value() / 2;
    }

    impl staking::Config for TestRuntime {
    type RewardRemainder = ();
    type CurrencyToVote = frame_support::traits::U128CurrencyToVote;
    type Event = Event;
    type Currency = Balances;
    type Slash = ();
    type ValidatorReward = PositiveImbalanceHandler<Self>;
    type NominatorReward = PositiveImbalanceHandler<Self>;
    type SessionsPerEra = SessionsPerEra;
    type BondingDuration = BondingDuration;
    type SlashDeferDuration = SlashDeferDuration;
    type SlashCancelOrigin = system::EnsureRoot<Self::AccountId>;
    type SessionInterface = Self;
    type UnixTime = Timestamp;
    type EraPayout = ValidatorManager;
    type MaxNominatorRewardedPerValidator = MaxNominatorRewardedPerValidator;
    type NextNewSession = Session;
    type ElectionLookahead = ElectionLookahead;
    type Call = Call;
    type UnsignedPriority = StakingUnsignedPriority;
    type MaxIterations = ();
    type MinSolutionScoreBump = ();
    type OffchainSolutionWeightLimit = ();
    type WeightInfo = ();
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
            Call::ValidatorManager(validators_manager::Call::signed_bond(proof, _, _, _)) => return Some(proof.clone()),
            Call::ValidatorManager(validators_manager::Call::signed_nominate(proof, _)) => return Some(proof.clone()),
            Call::ValidatorManager(validators_manager::Call::signed_rebond(proof, _)) => return Some(proof.clone()),
            Call::ValidatorManager(validators_manager::Call::signed_payout_stakers(proof, _)) => return Some(proof.clone()),
            Call::ValidatorManager(validators_manager::Call::signed_set_controller(proof, _)) => return Some(proof.clone()),
            Call::ValidatorManager(validators_manager::Call::signed_set_payee(proof, _)) => return Some(proof.clone()),
            Call::ValidatorManager(validators_manager::Call::signed_withdraw_unbonded(proof, _)) => return Some(proof.clone()),
            Call::ValidatorManager(validators_manager::Call::signed_unbond(proof, _)) => return Some(proof.clone()),
            Call::ValidatorManager(validators_manager::Call::signed_bond_extra(proof, _)) => return Some(proof.clone()),
            _ => None
        }
    }
}

impl InnerCallValidator for TestAvnProxyConfig {
    type Call = Call;

    fn signature_is_valid(call: &Box<Self::Call>) -> bool {
        match **call {
            Call::ValidatorManager(..) => {
                return ValidatorManager::signature_is_valid(call);
            },
            _ => false
        }
    }
}

impl CandidateTransactionSubmitter<AccountId> for TestRuntime {
    fn submit_candidate_transaction_to_tier1(
        candidate_type: EthTransactionType,
        _tx_id: TransactionId,
        submitter: AccountId,
        _signatures: Vec<ecdsa::Signature>,
    ) -> DispatchResult {
        let validator_t2_pub_key_used_in_unit_tests: [u8; 32] = <mock::TestRuntime as Config>::AccountToBytesConvert::into_bytes(&validator_id_3());
        let validator_t2_pub_key_used_in_benchmarks: [u8; 32] = MOCK_T2_PUBLIC_KEY_BYTES;
        let candidate_pub_key: [u8; 32] = <mock::TestRuntime as Config>::AccountToBytesConvert::into_bytes(&get_registered_validator_id());

        if submitter == get_registered_validator_id()
        || candidate_type == EthTransactionType::SlashValidator(SlashValidatorData::new(candidate_pub_key))
        || candidate_type == EthTransactionType::DeregisterValidator(DeregisterValidatorData::new(validator_t2_pub_key_used_in_unit_tests))
        || candidate_type == EthTransactionType::DeregisterValidator(DeregisterValidatorData::new(validator_t2_pub_key_used_in_benchmarks))
        {
            return Ok(());
        }

        Err(Error::<TestRuntime>::ErrorSubmitCandidateTxnToTier1.into())
    }

    fn reserve_transaction_id(_candidate_type: &EthTransactionType) -> Result<TransactionId, DispatchError> {
        let value = MOCK_TX_ID.with(|tx_id| {*tx_id.borrow()});
        MOCK_TX_ID.with(|tx_id| { *tx_id.borrow_mut() += 1; });
        return Ok(value);
    }
}

pub struct TestSessionManager;
// TODO [TYPE: test][PRI: low]: this mock is empty. Implement if needed in the tests.
impl session::SessionManager<AccountId> for TestSessionManager {
    fn new_session(new_index: SessionIndex) -> Option<Vec<AccountId>> {
        Staking::new_session(new_index);
        ValidatorManager::validator_account_ids()
    }
    fn end_session(end_index: SessionIndex) {
        Staking::end_session(end_index)
    }
    fn start_session(start_index: SessionIndex) {
        Staking::start_session(start_index)
    }
}

parameter_types! {
    pub const Period: u64 = 5;
    pub const Offset: u64 = 0;
    // TODO [TYPE: review][PRI: low]: inspect this value
    pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(33);
}

impl session::Config for TestRuntime {
    // TODO: This might need this value pallet_session::historical::NoteHistoricalRoot<TestRuntime, TestSessionManager>;
    type SessionManager = TestSessionManager;
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

impl pallet_session::historical::Config for TestRuntime {
	type FullIdentification = AccountId;
	type FullIdentificationOf = ConvertInto;
}

impl pallet_session::historical::SessionManager<AccountId, AccountId> for TestSessionManager {
    fn new_session(new_index: SessionIndex) -> Option<Vec<(AccountId, AccountId)>> {
        Staking::new_session(new_index);

        VALIDATORS.with(|l| l
            .borrow_mut()
            .take()
            .map(|validators| {
                validators.iter().map(|v| (*v, *v)).collect()
            })
        )
    }

    fn end_session(end_index: SessionIndex) {
        Staking::end_session(end_index)
    }

    fn start_session(start_index: SessionIndex) {
        Staking::start_session(start_index)
    }
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

impl FinalisedBlockChecker<BlockNumber> for TestRuntime {
    fn is_finalised(_block_number: BlockNumber) -> bool { true }
}

/// An extrinsic type used for tests.
type IdentificationTuple = (AccountId, AccountId);
type Offence = crate::ValidatorOffence<IdentificationTuple>;

pub fn get_registered_validator_id() -> AccountId {
    let topic_receiver = &MockData::get_validator_token_topics()[3];
    return TestAccount::from_bytes(topic_receiver.as_slice()).account_id();
}

pub const INITIAL_TRANSACTION_ID: TransactionId = 0;

thread_local! {
    static PROCESSED_EVENTS: RefCell<Vec<EthEventId>> = RefCell::new(vec![]);

    pub static VALIDATORS: RefCell<Option<Vec<AccountId>>> = RefCell::new(Some(vec![
        validator_id_1(),
        validator_id_2(),
        validator_id_3(),
        validator_id_4(),
        validator_id_5(),
    ]));

    static MOCK_TX_ID: RefCell<TransactionId> = RefCell::new(INITIAL_TRANSACTION_ID);

    pub static ETH_PUBLIC_KEY_VALID: RefCell<bool> = RefCell::new(true);

    pub static OFFENCES: RefCell<Vec<(Vec<AccountId>, Offence)>> = RefCell::new(vec![]);
}

impl ProcessedEventsChecker for TestRuntime {
    fn check_event(event_id: &EthEventId) -> bool {
        return PROCESSED_EVENTS.with(|l| l.borrow_mut().iter().any(|event| event == event_id));
    }
}

// TODO: Do we need to test the ECDSA sig verification logic here? If so, replace this with a call to the pallet's
// get_validator_for_eth_public_key method and update the tests to use "real" signatures
impl EthereumPublicKeyChecker<AccountId> for TestRuntime {
    fn get_validator_for_eth_public_key(eth_public_key: &ecdsa::Public) -> Option<AccountId> {
        if !<ValidatorManager as Store>::EthereumPublicKeys::contains_key(eth_public_key) {
            return None;
        }

        return Some(<ValidatorManager as Store>::EthereumPublicKeys::get(eth_public_key));
    }
}

pub fn set_mock_recovered_account_id(account_id: AccountId) {
    let eth_public_key = sp_core::ecdsa::Public::from_raw(<[u8; 33]>::from_hex(MOCK_ETH_PUBLIC_KEY).unwrap());
    <ValidatorManager as Store>::EthereumPublicKeys::insert(eth_public_key, account_id);
}

impl ValidatorRegistrationNotifier<ValidatorId> for TestRuntime {
    fn on_validator_registration(_validator_id: &ValidatorId) {}
}

fn initial_validators_public_keys() -> Vec<ecdsa::Public> {
    return vec![
        Public::from_slice(&hex!["03471b4c1012dddf4d494c506a098c7b1b719b20bbb177b1174f2166f953c29503"]),
        Public::from_slice(&hex!["0292a73ad9488b934fd04cb31a0f50634841f7105a5b4a8538e4bfa06aa477bed6"]),
        Public::from_slice(&hex!["03c5527886d8e09ad1fededd3231f890685d2d5345385d54181269f80c8926ff8e"]),
        Public::from_slice(&hex!["020e7593c534411f6f0e2fb91340751ada34ee5986f70b300443be17844416b28b"]),
        Public::from_slice(&hex!["02fde5665a2cb42863fb312fb527f2b02110997fc6865df583ca4324be137b7894"]),
    ];
}

impl ExtBuilder {
    pub fn with_validators(mut self) -> Self {
        let validator_account_ids = VALIDATORS.with(|l| l.borrow_mut().take().unwrap());
        BasicExternalities::execute_with_storage(&mut self.storage, || {
            for ref k in &validator_account_ids {
                frame_system::Module::<TestRuntime>::inc_providers(k);
            }
        });

        let _ = pallet_balances::GenesisConfig::<TestRuntime> {
            balances: validator_account_ids.iter().map(|x| {
                (x.clone(), VALIDATOR_STAKE + USER_STAKE)
            }).collect(),
        }.assimilate_storage(&mut self.storage);

        let _ = staking::GenesisConfig::<TestRuntime> {
            validator_count: validator_account_ids.len() as u32 * 2,
            minimum_validator_count: validator_account_ids.len() as u32,
            history_depth: 84,
            stakers: validator_account_ids.iter().map(|x| {
                (x.clone(), x.clone(), VALIDATOR_STAKE, StakerStatus::Validator)
            }).collect(),
            ..Default::default()
        }
        .assimilate_storage(&mut self.storage);

        let _ = validators_manager::GenesisConfig::<TestRuntime> {
            validators: validator_account_ids
                .iter().map(|v| v.clone())
                .zip(initial_validators_public_keys().iter().map(|pk| pk.clone()))
                .collect::<Vec<_>>(),
            min_validator_bond: VALIDATOR_STAKE,
            validator_max_commission: Perbill::from_percent(25),
            min_user_bond: USER_STAKE
        }
        .assimilate_storage(&mut self.storage);

        let _ = session::GenesisConfig::<TestRuntime> {
            keys: validator_account_ids
                .into_iter()
                .enumerate()
                .map(|(i, v)| (v, v, UintAuthorityId((i as u32).into())))
                .collect(),
        }.assimilate_storage(&mut self.storage);

        BasicExternalities::execute_with_storage(&mut self.storage, || {
            System::set_block_number(1);
            Session::on_initialize(1);
            Staking::on_initialize(1);
            Timestamp::set_timestamp(INIT_TIMESTAMP);
        });

        self
    }

    pub fn with_validator_count(self, validators: Vec<AccountId>) -> Self {
        assert!(validators.len() <= initial_validators_public_keys().len());

        VALIDATORS.with(|l| *l.borrow_mut() = Some(validators));

        return self.with_validators();
    }
}

pub struct MockData{
    pub event: EthEvent,
    pub validator_data: AddedValidatorData,
    pub new_validator_id: AccountId,
    pub validator_eth_public_key: ecdsa::Public
}

impl MockData {
    pub fn setup_valid() -> Self {
        let event_id = EthEventId {
            signature: ValidEvents::AddedValidator.signature(),
            transaction_hash: H256::random(),
        };
        let data = Some(LogDataHelper::get_validator_data(REGISTERING_VALIDATOR_TIER1_ID));
        let topics = MockData::get_validator_token_topics();
        let validator_data = AddedValidatorData::parse_bytes(data.clone() ,topics.clone()).unwrap();
        let new_validator_id = TestAccount::from_bytes(validator_data.t2_address.clone().as_bytes()).account_id();
        let _ = Balances::make_free_balance_be(&new_validator_id, VALIDATOR_STAKE);
        MockData{
            validator_data: validator_data.clone(),
            event: EthEvent {
                event_data: EventData::LogAddedValidator(validator_data.clone()),
                event_id: event_id.clone(),
            },
            new_validator_id: new_validator_id,
            validator_eth_public_key: ValidatorManager::compress_eth_public_key(validator_data.eth_public_key)
        }

    }

    pub fn get_validator_token_topics() -> Vec<Vec<u8>> {
        let topic_event_signature = LogDataHelper::get_topic_32_bytes(10);
        let topic_sender_lhs = LogDataHelper::get_topic_32_bytes(15);
        let topic_sender_rhs = LogDataHelper::get_topic_32_bytes(25);
        let topic_receiver = LogDataHelper::get_topic_32_bytes(30);
        return vec![topic_event_signature, topic_sender_lhs, topic_sender_rhs, topic_receiver];
    }
}

impl ValidatorManager {
    pub fn insert_to_validators(to_insert: &AccountId){
        <ValidatorAccountIds<TestRuntime>>::append(to_insert.clone());
    }
}

/// LogData Helper struct that converts values to topics and data
// TODO [TYPE: refactoring][PRI: low] We should consolidate the different versions of these functions and make one helper that can be used everywhere
pub struct LogDataHelper {}

impl LogDataHelper {
    pub fn get_validator_data(deposit: u128) -> Vec<u8> {
        return into_32_be_bytes(&deposit.to_le_bytes());
    }

    pub fn get_topic_32_bytes(n: u8) -> Vec<u8> {
        return vec![n; 32];
    }
}

// Progress to the given block, triggering session and era changes as we progress.
// This will finalize the previous block, initialize up to the given block, essentially simulating
// a block import/propose process where we first initialize the block, then execute some stuff (not
// in the function), and then finalize the block.
pub fn run_to_block(n: BlockNumber) {
    Staking::on_finalize(System::block_number());
    for b in (System::block_number() + 1)..=n {
        System::set_block_number(b);
        Session::on_initialize(b);
        Staking::on_initialize(b);
        Timestamp::set_timestamp(System::block_number() * BLOCK_TIME + INIT_TIMESTAMP);
        if b != n {
            Staking::on_finalize(System::block_number());
        }
    }
}

// Progresses from the current block number (whatever that may be) to the `P * session_index + 1`.
pub fn start_session(session_index: SessionIndex) {
    let end: u64 = if Offset::get().is_zero() {
        (session_index as u64) * Period::get()
    } else {
        Offset::get() + (session_index.saturating_sub(1) as u64) * Period::get()
    };
    run_to_block(end);
    // session must have progressed properly.
    assert_eq!(
        Session::current_index(), session_index,
        "current session index = {}, expected = {}", Session::current_index(), session_index,
    );
}

// Go one session forward.
pub(crate) fn advance_session() {
    let current_index = Session::current_index();
    start_session(current_index + 1);
}

// Go one era forward.
pub fn advance_era() {
    start_session(Session::current_index() + <TestRuntime as staking::Config>::SessionsPerEra::get());
}

pub fn advance_era_to_pass_bonding_period() {
    const BONDING_DURATION: u32 = <TestRuntime as pallet_staking::Config>::BondingDuration::get();
    for _era in 0..BONDING_DURATION {
        advance_era();
    }
}

pub(crate) fn validator_controllers() -> Vec<AccountId> {
	Session::validators()
		.into_iter()
		.map(|s| Staking::bonded(&s).expect("no controller for validator"))
		.collect()
}

pub fn mock_response_of_get_ecdsa_signature(
    state: &mut OffchainState,
    data_to_sign: String,
    response: Option<Vec<u8>>,
) {
    let mut url = "http://127.0.0.1:2020/eth/sign/".to_string();
    url.push_str(&data_to_sign);

    state.expect_request(PendingRequest {
        method: "GET".into(),
        uri: url.into(),
        response: response,
        sent: true,
        ..Default::default()
    });
}