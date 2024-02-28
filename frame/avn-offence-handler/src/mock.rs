//Copyright 2021 Aventus Systems (UK) Ltd.

use crate::{self as avn_offence_handler, *};
use crate::extension_builder::ExtBuilder;
use frame_support::{parameter_types, BasicExternalities,
    dispatch::{DispatchResult, DispatchError}
};
use frame_system as system;
use std::cell::RefCell;
use sp_core::H256;
use sp_runtime::{
    testing::{Header, UintAuthorityId},
    traits::{BlakeTwo256, IdentityLookup, ConvertInto},
    Perbill,
};
use pallet_session as session;

pub const VALIDATOR_ID_1: u64 = 1;
pub const VALIDATOR_ID_2: u64 = 2;
pub const VALIDATOR_ID_CAN_CAUSE_SLASH_ERROR: u64 = 3;

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
        AVN: pallet_avn::{Module, Storage},
        AvnOffenceHandler: avn_offence_handler::{Module, Call, Storage, Event<T>}
    }
);

pub type ValidatorId = <TestRuntime as session::Config>::ValidatorId;

impl Config for TestRuntime {
    type Event = Event;
    type Enforcer = Self;
    type WeightInfo = ();
}

impl pallet_session::historical::Config for TestRuntime {
	type FullIdentification = u64;
	type FullIdentificationOf = ConvertInto;
}

impl pallet_avn::Config for TestRuntime {
    type AuthorityId = UintAuthorityId;
    type EthereumPublicKeyChecker = ();
    type NewSessionHandler = ();
    type DisabledValidatorChecker = ();
    type FinalisedBlockChecker = ();
}

pub struct TestSessionManager;
// TODO [TYPE: test][PRI: low]: this mock is empty. Implement if needed in the tests.
impl session::SessionManager<u64> for TestSessionManager {
	fn new_session(_new_index: SessionIndex) -> Option<Vec<u64>> {
		None
	}
    fn end_session(_: SessionIndex) {}
    fn start_session(_: SessionIndex) {}
}

parameter_types! {
    pub const Period: u64 = 1;
    pub const Offset: u64 = 0;
    // TODO [TYPE: review][PRI: low]: inspect this value
    pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(33);
}

impl session::Config for TestRuntime {
    type SessionManager = TestSessionManager;
    type Keys = UintAuthorityId;
    type ShouldEndSession = session::PeriodicSessions<Period, Offset>;
    type SessionHandler = (AVN, );
    type Event = Event;
    type ValidatorId = u64;
    type ValidatorIdOf = ConvertInto;
    type DisabledValidatorsThreshold = DisabledValidatorsThreshold;
    type NextSessionRotation = session::PeriodicSessions<Period, Offset>;
    type WeightInfo = ();
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
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = Event;
    type BlockHashCount = BlockHashCount;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = ();
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
}

impl Enforcer<ValidatorId> for TestRuntime {
    fn slash_validator(slashed_validator_id: &ValidatorId) -> DispatchResult {
        if slashed_validator_id == &VALIDATOR_ID_CAN_CAUSE_SLASH_ERROR {
            return Err(DispatchError::Other("Slash validator failed"));
        }
        Ok(())
    }
}

thread_local! {
    static VALIDATORS: RefCell<Option<Vec<u64>>> = RefCell::new(Some(vec![
        VALIDATOR_ID_1,
        VALIDATOR_ID_2,
        VALIDATOR_ID_CAN_CAUSE_SLASH_ERROR,
    ]));
}

impl ExtBuilder {
    pub fn with_validators(mut self) -> Self {
        let validators: Vec<u64> = VALIDATORS.with(|l| l.borrow_mut().take().unwrap());
        BasicExternalities::execute_with_storage(&mut self.storage, || {
            for ref k in &validators {
                frame_system::Module::<TestRuntime>::inc_providers(k);
            }
        });
        let _ = session::GenesisConfig::<TestRuntime> {
            keys: validators
                .into_iter()
                .map(|v| (v, v, UintAuthorityId(v)))
                .collect(),
        }
        .assimilate_storage(&mut self.storage);
        self
    }
}

impl AvnOffenceHandler {
    pub fn enable_offence() {
        <SlashingEnabled>::put(true);
    }

    pub fn disable_offence() {
        <SlashingEnabled>::put(false);
    }
}
