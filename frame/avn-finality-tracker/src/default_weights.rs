//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::weights::{Weight, constants::RocksDbWeight as DbWeight};
use sp_std::marker::PhantomData;

pub trait WeightInfo {
    fn submit_latest_finalised_block_number(v: u32, ) -> Weight;
}

/// Weights for pallet_finality_tracker
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn submit_latest_finalised_block_number(v: u32, ) -> Weight {
		(61_566_000 as Weight)
			.saturating_add((390_000 as Weight).saturating_mul(v as Weight))
			.saturating_add(DbWeight::get().reads(3 as Weight))
			.saturating_add(DbWeight::get().writes(2 as Weight))
	}
}

// For backwards compatibility and tests
impl crate::WeightInfo for () {
	fn submit_latest_finalised_block_number(v: u32, ) -> Weight {
		(77_727_000 as Weight)
			.saturating_add((454_000 as Weight).saturating_mul(v as Weight))
			.saturating_add(DbWeight::get().reads(3 as Weight))
			.saturating_add(DbWeight::get().writes(2 as Weight))
	}
}
