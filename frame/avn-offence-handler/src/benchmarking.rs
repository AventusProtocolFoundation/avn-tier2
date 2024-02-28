//! # Avn offence handler pallet
// Copyright 2020 Artos Systems (UK) Ltd.

//! avn-offence-handler pallet benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::benchmarks;
use frame_system::{EventRecord, RawOrigin};

fn assert_last_event<T: Config>(generic_event: <T as Config>::Event) {
    let events = frame_system::Module::<T>::events();
    let system_event: <T as frame_system::Config>::Event = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len().saturating_sub(1 as usize)];
    assert_eq!(event, &system_event);
}

benchmarks! {
    configure_slashing {
        let enabled = true;
    }: _(RawOrigin::Root, enabled)
    verify {
        assert_eq!(SlashingEnabled::get(), enabled);
        assert_last_event::<T>(
            RawEvent::SlashingConfigurationUpdated(enabled).into()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::*;
    use frame_support::assert_ok;
    use crate::extension_builder::ExtBuilder;

    #[test]
    fn benchmarks() {
        let mut ext = ExtBuilder::build_default().as_externality();

        ext.execute_with(|| {
            assert_ok!(test_benchmark_configure_slashing::<TestRuntime>());
        });
    }
}