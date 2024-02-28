//! # Ethereum transactions pallet
// Copyright 2020 Artos Systems (UK) Ltd.

//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::weights::{Weight, constants::RocksDbWeight as DbWeight};
use sp_std::marker::PhantomData;

pub trait WeightInfo {
    fn set_transaction_id() -> Weight;
    fn unreserve_transaction() -> Weight;
    fn set_eth_tx_hash_for_dispatched_tx(v: u32, t: u32, ) -> Weight;
    fn set_publish_root_contract() -> Weight;
}

/// Weights for pallet_ethereum_transactions
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn set_transaction_id() -> Weight {
		(5_240_000 as Weight)
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn unreserve_transaction() -> Weight {
		(34_729_000 as Weight)
			.saturating_add(DbWeight::get().reads(1 as Weight))
			.saturating_add(DbWeight::get().writes(2 as Weight))
	}
	fn set_eth_tx_hash_for_dispatched_tx(v: u32, t: u32, ) -> Weight {
		(133_973_000 as Weight)
			.saturating_add((434_000 as Weight).saturating_mul(v as Weight))
			.saturating_add((35_000 as Weight).saturating_mul(t as Weight))
			.saturating_add(DbWeight::get().reads(3 as Weight))
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
    fn set_publish_root_contract() -> Weight {
		(5_660_000 as Weight)
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
}

// For backwards compatibility and tests
impl crate::WeightInfo for () {
	fn set_transaction_id() -> Weight {
		(5_240_000 as Weight)
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn unreserve_transaction() -> Weight {
		(34_729_000 as Weight)
			.saturating_add(DbWeight::get().reads(1 as Weight))
			.saturating_add(DbWeight::get().writes(2 as Weight))
	}
	fn set_eth_tx_hash_for_dispatched_tx(v: u32, t: u32, ) -> Weight {
		(133_973_000 as Weight)
			.saturating_add((434_000 as Weight).saturating_mul(v as Weight))
			.saturating_add((35_000 as Weight).saturating_mul(t as Weight))
			.saturating_add(DbWeight::get().reads(3 as Weight))
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
    fn set_publish_root_contract() -> Weight {
		(5_660_000 as Weight)
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
}
