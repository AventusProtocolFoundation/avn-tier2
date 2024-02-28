// Copyright 2020 Artos Systems (UK) Ltd.

#![cfg(test)]

use crate::{self as avn_finality_tracker, *};
use sp_core::H256;
use frame_support::{parameter_types};
use sp_runtime::{traits::{BlakeTwo256, IdentityLookup}, testing::{Header, UintAuthorityId, TestXt}};
use frame_system as system;

pub type Extrinsic = TestXt<Call, ()>;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;
type Block = frame_system::mocking::MockBlock<TestRuntime>;

frame_support::construct_runtime!(
    pub enum TestRuntime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Module, Call, Config, Storage, Event<T>},
        AVN: pallet_avn::{Module, Storage},
        AvnFinalityTracker: avn_finality_tracker::{Module, Call, Storage, Event<T>}
    }
);

parameter_types! {
    pub const CacheAge: u64 = 10;
    pub const SubmissionInterval: u64 = 5;
    pub const ReportLatency: u64 = 1000;
}

impl Config for TestRuntime {
    type Event = Event;
    type CacheAge = CacheAge;
    type SubmissionInterval = SubmissionInterval;
    type ReportLatency = ReportLatency;
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

impl avn::Config for TestRuntime {
    type AuthorityId = UintAuthorityId;
    type EthereumPublicKeyChecker = ();
    type NewSessionHandler = ();
    type DisabledValidatorChecker = ();
    type FinalisedBlockChecker = ();
}

impl<LocalCall> frame_system::offchain::SendTransactionTypes<LocalCall> for TestRuntime where
    Call: From<LocalCall>,
{
    type OverarchingCall = Call;
    type Extrinsic = Extrinsic;
}

pub struct ExtBuilder {
    storage: sp_runtime::Storage,
}

impl ExtBuilder {
    pub fn build_default() -> Self {
        let storage = system::GenesisConfig::default()
            .build_storage::<TestRuntime>()
            .unwrap();
        Self {
            storage: storage,
        }
    }

    pub fn as_externality(self) -> sp_io::TestExternalities {
        let mut ext = sp_io::TestExternalities::from(self.storage);
        // Events do not get emitted on block 0, so we increment the block here
        ext.execute_with(|| System::set_block_number(1));
        ext
    }
}

