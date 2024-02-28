//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::weights::{Weight, constants::RocksDbWeight as DbWeight};
use sp_std::marker::PhantomData;

pub trait WeightInfo {
    fn configure_slashing() -> Weight;
}

/// Weights for pallet_avn_offence_handler
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn configure_slashing() -> Weight {
		(34_750_000 as Weight)
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
}

impl crate::WeightInfo for () {
	fn configure_slashing() -> Weight {
		(34_750_000 as Weight)
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
}
