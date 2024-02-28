//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::weights::{Weight, constants::RocksDbWeight as DbWeight};
use sp_std::marker::PhantomData;

pub trait WeightInfo {
    fn add_validator_log(u: u32, e: u32, ) -> Weight;
    fn add_lift_log(u: u32, e: u32, ) -> Weight;
    fn add_ethereum_log(u: u32, e: u32, ) -> Weight;
    fn submit_checkevent_result(v: u32, u: u32, ) -> Weight;
    fn process_event_with_successful_challenge(v: u32, e: u32, ) -> Weight;
    fn process_event_without_successful_challenge(v: u32, e: u32, ) -> Weight;
    fn challenge_event(v: u32, e: u32, c: u32, ) -> Weight;
    fn set_ethereum_contract_map_storage() -> Weight;
    fn set_ethereum_contract_storage() -> Weight;
    fn set_event_challenge_period() -> Weight;
    fn signed_add_ethereum_log(u: u32, e: u32, ) -> Weight;
}

/// Weights for pallet_ethereum_events
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn add_validator_log(u: u32, e: u32, ) -> Weight {
		(92_365_000 as Weight)
			.saturating_add((275_000 as Weight).saturating_mul(u as Weight))
			.saturating_add((2_034_000 as Weight).saturating_mul(e as Weight))
			.saturating_add(DbWeight::get().reads(5 as Weight))
			.saturating_add(DbWeight::get().writes(2 as Weight))
	}
	fn add_lift_log(u: u32, e: u32, ) -> Weight {
		(93_637_000 as Weight)
			.saturating_add((306_000 as Weight).saturating_mul(u as Weight))
			.saturating_add((1_906_000 as Weight).saturating_mul(e as Weight))
			.saturating_add(DbWeight::get().reads(5 as Weight))
			.saturating_add(DbWeight::get().writes(2 as Weight))
	}
	fn add_ethereum_log(u: u32, e: u32, ) -> Weight {
		(124_700_000 as Weight)
			.saturating_add((814_000 as Weight).saturating_mul(u as Weight))
			.saturating_add((4_063_000 as Weight).saturating_mul(e as Weight))
			.saturating_add(DbWeight::get().reads(5 as Weight))
			.saturating_add(DbWeight::get().writes(2 as Weight))
	}
	fn submit_checkevent_result(v: u32, u: u32, ) -> Weight {
		(113_171_000 as Weight)
			.saturating_add((1_260_000 as Weight).saturating_mul(v as Weight))
			.saturating_add((1_479_000 as Weight).saturating_mul(u as Weight))
			.saturating_add(DbWeight::get().reads(5 as Weight))
			.saturating_add(DbWeight::get().writes(2 as Weight))
	}
	fn process_event_with_successful_challenge(v: u32, e: u32, ) -> Weight {
		(366_866_000 as Weight)
			.saturating_add((1_712_000 as Weight).saturating_mul(v as Weight))
			.saturating_add((7_267_000 as Weight).saturating_mul(e as Weight))
			.saturating_add(DbWeight::get().reads(13 as Weight))
			.saturating_add(DbWeight::get().writes(6 as Weight))
	}
	fn process_event_without_successful_challenge(v: u32, e: u32, ) -> Weight {
		(361_399_000 as Weight)
			.saturating_add((2_081_000 as Weight).saturating_mul(v as Weight))
			.saturating_add((6_280_000 as Weight).saturating_mul(e as Weight))
			.saturating_add(DbWeight::get().reads(13 as Weight))
			.saturating_add(DbWeight::get().writes(6 as Weight))
	}
	fn challenge_event(v: u32, e: u32, c: u32, ) -> Weight {
		(108_232_000 as Weight)
			.saturating_add((566_000 as Weight).saturating_mul(v as Weight))
			.saturating_add((1_598_000 as Weight).saturating_mul(e as Weight))
			.saturating_add((375_000 as Weight).saturating_mul(c as Weight))
			.saturating_add(DbWeight::get().reads(3 as Weight))
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn set_ethereum_contract_map_storage() -> Weight {
		(10_870_000 as Weight)
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn set_ethereum_contract_storage() -> Weight {
		(5_960_000 as Weight)
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn set_event_challenge_period() -> Weight {
		(47_310_000 as Weight)
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn signed_add_ethereum_log(_u: u32, e: u32, ) -> Weight {
		(238_774_000 as Weight)
			// Standard Error: 421_000
			.saturating_add((1_040_000 as Weight).saturating_mul(e as Weight))
			.saturating_add(DbWeight::get().reads(6 as Weight))
			.saturating_add(DbWeight::get().writes(3 as Weight))
	}
}

// For backwards compatibility and tests
impl crate::WeightInfo for () {
	fn add_validator_log(u: u32, e: u32, ) -> Weight {
		(92_365_000 as Weight)
			.saturating_add((275_000 as Weight).saturating_mul(u as Weight))
			.saturating_add((2_034_000 as Weight).saturating_mul(e as Weight))
			.saturating_add(DbWeight::get().reads(5 as Weight))
			.saturating_add(DbWeight::get().writes(2 as Weight))
	}
	fn add_lift_log(u: u32, e: u32, ) -> Weight {
		(93_637_000 as Weight)
			.saturating_add((306_000 as Weight).saturating_mul(u as Weight))
			.saturating_add((1_906_000 as Weight).saturating_mul(e as Weight))
			.saturating_add(DbWeight::get().reads(5 as Weight))
			.saturating_add(DbWeight::get().writes(2 as Weight))
	}
	fn add_ethereum_log(u: u32, e: u32, ) -> Weight {
		(124_700_000 as Weight)
			.saturating_add((814_000 as Weight).saturating_mul(u as Weight))
			.saturating_add((4_063_000 as Weight).saturating_mul(e as Weight))
			.saturating_add(DbWeight::get().reads(5 as Weight))
			.saturating_add(DbWeight::get().writes(2 as Weight))
	}
	fn submit_checkevent_result(v: u32, u: u32, ) -> Weight {
		(113_171_000 as Weight)
			.saturating_add((1_260_000 as Weight).saturating_mul(v as Weight))
			.saturating_add((1_479_000 as Weight).saturating_mul(u as Weight))
			.saturating_add(DbWeight::get().reads(5 as Weight))
			.saturating_add(DbWeight::get().writes(2 as Weight))
	}
	fn process_event_with_successful_challenge(v: u32, e: u32, ) -> Weight {
		(366_866_000 as Weight)
			.saturating_add((1_712_000 as Weight).saturating_mul(v as Weight))
			.saturating_add((7_267_000 as Weight).saturating_mul(e as Weight))
			.saturating_add(DbWeight::get().reads(13 as Weight))
			.saturating_add(DbWeight::get().writes(6 as Weight))
	}
	fn process_event_without_successful_challenge(v: u32, e: u32, ) -> Weight {
		(361_399_000 as Weight)
			.saturating_add((2_081_000 as Weight).saturating_mul(v as Weight))
			.saturating_add((6_280_000 as Weight).saturating_mul(e as Weight))
			.saturating_add(DbWeight::get().reads(13 as Weight))
			.saturating_add(DbWeight::get().writes(6 as Weight))
	}
	fn challenge_event(v: u32, e: u32, c: u32, ) -> Weight {
		(108_232_000 as Weight)
			.saturating_add((566_000 as Weight).saturating_mul(v as Weight))
			.saturating_add((1_598_000 as Weight).saturating_mul(e as Weight))
			.saturating_add((375_000 as Weight).saturating_mul(c as Weight))
			.saturating_add(DbWeight::get().reads(3 as Weight))
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn set_ethereum_contract_map_storage() -> Weight {
		(10_870_000 as Weight)
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn set_ethereum_contract_storage() -> Weight {
		(5_960_000 as Weight)
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn set_event_challenge_period() -> Weight {
		(47_310_000 as Weight)
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn signed_add_ethereum_log(_u: u32, e: u32, ) -> Weight {
		(238_774_000 as Weight)
			// Standard Error: 421_000
			.saturating_add((1_040_000 as Weight).saturating_mul(e as Weight))
			.saturating_add(DbWeight::get().reads(6 as Weight))
			.saturating_add(DbWeight::get().writes(3 as Weight))
	}
}