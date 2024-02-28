// Copyright (C) 2020-2021 Aventus Network Services (UK) Limited.

pub mod constants {
    use frame_support::{parameter_types,
        weights::{
            Weight, RuntimeDbWeight,
            constants::{WEIGHT_PER_MILLIS, WEIGHT_PER_MICROS}
        }
    };

    parameter_types! {
        /// Importing a block with 0 txs takes ~77.8 ms -> ~78 ms
        pub const BlockExecutionWeight: Weight = 78 * WEIGHT_PER_MILLIS;
        /// Executing 10,001 System remarks (no-op) txs takes ~47.716 seconds -> ~4771 µs per tx
        /// TODO [TYPE: weightInfo][PRI: medium]: Fix bin/node/bench/src/import.rs sanity check assertion failure and update this value if needed
        pub const ExtrinsicBaseWeight: Weight = 4771 * WEIGHT_PER_MICROS;
        /// By default, AVN uses RocksDB, so this will be the weight used throughout
        /// the runtime.
        pub const RocksDbWeight: RuntimeDbWeight = RuntimeDbWeight {
            read: 40 * WEIGHT_PER_MICROS,   // ~39.7 µs -> ~40 µs @ 200,000 items
            write: 173 * WEIGHT_PER_MICROS, // ~172.4 µs -> ~173 µs @ 200,000 items
        };
	}
}