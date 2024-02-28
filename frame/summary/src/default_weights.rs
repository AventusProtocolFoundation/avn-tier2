//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::weights::{Weight, constants::RocksDbWeight as DbWeight};
use sp_std::marker::PhantomData;

pub trait WeightInfo {
    fn set_periods() -> Weight;
    fn record_summary_calculation(v: u32, r: u32, ) -> Weight;
    fn approve_root_with_end_voting(v: u32, o: u32, ) -> Weight;
    fn approve_root_without_end_voting(v: u32, ) -> Weight;
    fn reject_root_with_end_voting(v: u32, o: u32, ) -> Weight;
    fn reject_root_without_end_voting(v: u32, ) -> Weight;
    fn end_voting_period_with_rejected_valid_votes(o: u32, ) -> Weight;
    fn end_voting_period_with_approved_invalid_votes(o: u32, ) -> Weight;
    fn advance_slot_with_offence() -> Weight;
    fn advance_slot_without_offence() -> Weight;
    fn add_challenge() -> Weight;
}

/// Weights for pallet_summary
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn set_periods() -> Weight {
		(38_520_000 as Weight)
			.saturating_add(DbWeight::get().writes(2 as Weight))
	}
	fn record_summary_calculation(v: u32, r: u32, ) -> Weight {
		(207_472_000 as Weight)
			// Standard Error: 356_000
			.saturating_add((232_000 as Weight).saturating_mul(v as Weight))
			// Standard Error: 2_498_000
			.saturating_add((2_744_000 as Weight).saturating_mul(r as Weight))
			.saturating_add(DbWeight::get().reads(10 as Weight))
			.saturating_add(DbWeight::get().writes(6 as Weight))
	}
	fn approve_root_with_end_voting(v: u32, o: u32, ) -> Weight {
		(896_879_000 as Weight)
			// Standard Error: 272_000
			.saturating_add((5_777_000 as Weight).saturating_mul(v as Weight))
			// Standard Error: 13_605_000
			.saturating_add((103_691_000 as Weight).saturating_mul(o as Weight))
			.saturating_add(DbWeight::get().reads(12 as Weight))
			.saturating_add(DbWeight::get().reads((3 as Weight).saturating_mul(o as Weight)))
			.saturating_add(DbWeight::get().writes(7 as Weight))
			.saturating_add(DbWeight::get().writes((2 as Weight).saturating_mul(o as Weight)))
	}
	fn approve_root_without_end_voting(_v: u32, ) -> Weight {
		(598_889_000 as Weight)
			.saturating_add(DbWeight::get().reads(6 as Weight))
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn reject_root_with_end_voting(v: u32, o: u32, ) -> Weight {
		(715_903_000 as Weight)
			// Standard Error: 236_000
			.saturating_add((2_440_000 as Weight).saturating_mul(v as Weight))
			// Standard Error: 11_847_000
			.saturating_add((80_118_000 as Weight).saturating_mul(o as Weight))
			.saturating_add(DbWeight::get().reads(13 as Weight))
			.saturating_add(DbWeight::get().reads((3 as Weight).saturating_mul(o as Weight)))
			.saturating_add(DbWeight::get().writes(6 as Weight))
			.saturating_add(DbWeight::get().writes((2 as Weight).saturating_mul(o as Weight)))
	}
	fn reject_root_without_end_voting(v: u32, ) -> Weight {
		(186_068_000 as Weight)
			// Standard Error: 676_000
			.saturating_add((1_792_000 as Weight).saturating_mul(v as Weight))
			.saturating_add(DbWeight::get().reads(5 as Weight))
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn end_voting_period_with_rejected_valid_votes(o: u32, ) -> Weight {
		(350_681_000 as Weight)
			// Standard Error: 8_642_000
			.saturating_add((78_349_000 as Weight).saturating_mul(o as Weight))
			.saturating_add(DbWeight::get().reads(11 as Weight))
			.saturating_add(DbWeight::get().reads((3 as Weight).saturating_mul(o as Weight)))
			.saturating_add(DbWeight::get().writes(6 as Weight))
			.saturating_add(DbWeight::get().writes((2 as Weight).saturating_mul(o as Weight)))
	}
	fn end_voting_period_with_approved_invalid_votes(o: u32, ) -> Weight {
		(483_724_000 as Weight)
			// Standard Error: 3_842_000
			.saturating_add((84_524_000 as Weight).saturating_mul(o as Weight))
			.saturating_add(DbWeight::get().reads(13 as Weight))
			.saturating_add(DbWeight::get().reads((3 as Weight).saturating_mul(o as Weight)))
			.saturating_add(DbWeight::get().writes(5 as Weight))
			.saturating_add(DbWeight::get().writes((2 as Weight).saturating_mul(o as Weight)))
	}
	fn advance_slot_with_offence() -> Weight {
		(375_563_000 as Weight)
			.saturating_add(DbWeight::get().reads(14 as Weight))
			.saturating_add(DbWeight::get().writes(7 as Weight))
	}
	fn advance_slot_without_offence() -> Weight {
		(121_141_000 as Weight)
			.saturating_add(DbWeight::get().reads(6 as Weight))
			.saturating_add(DbWeight::get().writes(3 as Weight))
	}
	fn add_challenge() -> Weight {
		(426_183_000 as Weight)
			.saturating_add(DbWeight::get().reads(14 as Weight))
			.saturating_add(DbWeight::get().writes(7 as Weight))
	}
}

// For backwards compatibility and tests
impl crate::WeightInfo for () {
	fn set_periods() -> Weight {
		(18_132_000 as Weight)
			.saturating_add(DbWeight::get().writes(2 as Weight))
	}
	fn record_summary_calculation(v: u32, r: u32, ) -> Weight {
		(245_074_000 as Weight)
			.saturating_add((924_000 as Weight).saturating_mul(v as Weight))
			.saturating_add((21_782_000 as Weight).saturating_mul(r as Weight))
			.saturating_add(DbWeight::get().reads(9 as Weight))
			.saturating_add(DbWeight::get().reads((1 as Weight).saturating_mul(r as Weight)))
			.saturating_add(DbWeight::get().writes(6 as Weight))
	}
	fn approve_root_with_end_voting(v: u32, o: u32, ) -> Weight {
		(284_878_000 as Weight)
			.saturating_add((14_912_000 as Weight).saturating_mul(v as Weight))
			.saturating_add((172_974_000 as Weight).saturating_mul(o as Weight))
			.saturating_add(DbWeight::get().reads(13 as Weight))
			.saturating_add(DbWeight::get().reads((3 as Weight).saturating_mul(o as Weight)))
			.saturating_add(DbWeight::get().writes(8 as Weight))
			.saturating_add(DbWeight::get().writes((1 as Weight).saturating_mul(o as Weight)))
	}
	fn approve_root_without_end_voting(v: u32, ) -> Weight {
		(655_370_000 as Weight)
			.saturating_add((1_133_000 as Weight).saturating_mul(v as Weight))
			.saturating_add(DbWeight::get().reads(5 as Weight))
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn reject_root_with_end_voting(v: u32, o: u32, ) -> Weight {
		(197_696_000 as Weight)
			.saturating_add((11_656_000 as Weight).saturating_mul(v as Weight))
			.saturating_add((178_744_000 as Weight).saturating_mul(o as Weight))
			.saturating_add(DbWeight::get().reads(14 as Weight))
			.saturating_add(DbWeight::get().reads((3 as Weight).saturating_mul(o as Weight)))
			.saturating_add(DbWeight::get().writes(6 as Weight))
			.saturating_add(DbWeight::get().writes((1 as Weight).saturating_mul(o as Weight)))
	}
	fn reject_root_without_end_voting(v: u32, ) -> Weight {
		(238_446_000 as Weight)
			.saturating_add((451_000 as Weight).saturating_mul(v as Weight))
			.saturating_add(DbWeight::get().reads(4 as Weight))
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn end_voting_period_with_rejected_valid_votes(o: u32, ) -> Weight {
		(571_479_000 as Weight)
			.saturating_add((173_113_000 as Weight).saturating_mul(o as Weight))
			.saturating_add(DbWeight::get().reads(12 as Weight))
			.saturating_add(DbWeight::get().reads((3 as Weight).saturating_mul(o as Weight)))
			.saturating_add(DbWeight::get().writes(7 as Weight))
			.saturating_add(DbWeight::get().writes((1 as Weight).saturating_mul(o as Weight)))
	}
	fn end_voting_period_with_approved_invalid_votes(o: u32, ) -> Weight {
		(987_512_000 as Weight)
			.saturating_add((173_005_000 as Weight).saturating_mul(o as Weight))
			.saturating_add(DbWeight::get().reads(14 as Weight))
			.saturating_add(DbWeight::get().reads((3 as Weight).saturating_mul(o as Weight)))
			.saturating_add(DbWeight::get().writes(5 as Weight))
			.saturating_add(DbWeight::get().writes((1 as Weight).saturating_mul(o as Weight)))
	}
	fn advance_slot_with_offence() -> Weight {
		(670_464_000 as Weight)
			.saturating_add(DbWeight::get().reads(15 as Weight))
			.saturating_add(DbWeight::get().writes(7 as Weight))
	}
	fn advance_slot_without_offence() -> Weight {
		(193_472_000 as Weight)
			.saturating_add(DbWeight::get().reads(5 as Weight))
			.saturating_add(DbWeight::get().writes(3 as Weight))
	}
	fn add_challenge() -> Weight {
		(635_534_000 as Weight)
			.saturating_add(DbWeight::get().reads(15 as Weight))
			.saturating_add(DbWeight::get().writes(7 as Weight))
	}
}
