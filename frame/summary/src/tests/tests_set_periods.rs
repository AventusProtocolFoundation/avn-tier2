// Copyright 2021 Aventus (UK) Ltd.
#![cfg(test)]

use crate::mock::*;
use crate::*;
use crate::extension_builder::ExtBuilder;
use frame_support::{assert_ok, assert_noop};
use sp_runtime::traits::BadOrigin;
use system::RawOrigin;

mod test_set_periods {
    use super::*;

    struct Context {
        origin: Origin,
        schedule_period: BlockNumber,
        new_schedule_period: BlockNumber,
        voting_period: BlockNumber,
        new_voting_period: BlockNumber,
    }

    impl Default for Context {
        fn default() -> Self {
            Context {
                origin: RawOrigin::Root.into(),
                schedule_period: 160,
                new_schedule_period: 200,
                voting_period: 100,
                new_voting_period: 150,
            }
        }
    }

    impl Context {
        fn dispatch_set_schedule_period(&self) -> DispatchResult {
            return Summary::set_periods(self.origin.clone(), self.new_schedule_period.clone(), self.voting_period.clone());
        }

        fn dispatch_set_voting_period(&self) -> DispatchResult {
            return Summary::set_periods(self.origin.clone(), self.schedule_period.clone(), self.new_voting_period.clone());
        }
    }

    mod successful_cases {
        use super::*;

        #[test]
        fn update_schedule_period() {
            let mut ext = ExtBuilder::build_default()
                .with_validators()
                .with_genesis_config()
                .as_externality();
            ext.execute_with(||{
                let context = Context::default();
                assert_ne!(context.new_schedule_period, Summary::schedule_period());

                assert_ok!(context.dispatch_set_schedule_period());
                assert_eq!(context.new_schedule_period, Summary::schedule_period());
            });
        }

        #[test]
        fn update_voting_period() {
            let mut ext = ExtBuilder::build_default()
                .with_validators()
                .with_genesis_config()
                .as_externality();
            ext.execute_with(||{
                let context = Context::default();
                assert_ne!(context.new_voting_period, Summary::voting_period());

                assert_ok!(context.dispatch_set_voting_period());
                assert_eq!(context.new_voting_period, Summary::voting_period());
            });
        }
    }

    mod fails_when {
        use super::*;

        mod set_schedule_period {
            use super::*;

            #[test]
            fn origin_is_not_root() {
                let mut ext = ExtBuilder::build_default()
                    .with_validators()
                    .with_genesis_config()
                    .as_externality();
                ext.execute_with(||{
                    let context: Context = Context {
                        origin: Origin::signed(Default::default()),
                        ..Default::default()
                    };

                    assert_noop!(context.dispatch_set_schedule_period(), BadOrigin);
                    assert_ne!(context.new_schedule_period, Summary::schedule_period());
                });
            }

            #[test]
            fn origin_is_unsigned() {
                let mut ext = ExtBuilder::build_default()
                    .with_validators()
                    .with_genesis_config()
                    .as_externality();
                ext.execute_with(||{
                    let context: Context = Context {
                        origin: RawOrigin::None.into(),
                        ..Default::default()
                    };

                    assert_noop!(context.dispatch_set_schedule_period(), BadOrigin);
                    assert_ne!(context.new_schedule_period, Summary::schedule_period());
                });
            }

            #[test]
            fn less_than_minimum_value_should_fail() {
                let mut ext = ExtBuilder::build_default()
                    .with_validators()
                    .with_genesis_config()
                    .as_externality();
                ext.execute_with(||{
                    let context: Context = Context {
                        new_schedule_period: (MIN_SCHEDULE_PERIOD - 1).into(),
                        ..Default::default()
                    };

                    assert_noop!(context.dispatch_set_schedule_period(), Error::<TestRuntime>::SchedulePeriodIsTooShort);
                    assert_ne!(context.new_schedule_period, Summary::schedule_period());
                });
            }

            #[test]
            fn greater_than_maximum_value_should_fail() {
                let mut ext = ExtBuilder::build_default()
                    .with_validators()
                    .with_genesis_config()
                    .as_externality();
                ext.execute_with(||{
                    let context: Context = Context {
                        new_schedule_period: (MAX_SCHEDULE_PERIOD + 1).into(),
                        ..Default::default()
                    };

                    assert_noop!(context.dispatch_set_schedule_period(), Error::<TestRuntime>::SchedulePeriodIsTooLong);
                    assert_ne!(context.new_schedule_period, Summary::schedule_period());
                });
            }
        }

        mod set_voting_period {
            use super::*;

            #[test]
            fn origin_is_not_root() {
                let mut ext = ExtBuilder::build_default()
                    .with_validators()
                    .with_genesis_config()
                    .as_externality();
                ext.execute_with(||{
                    let context: Context = Context {
                        origin: Origin::signed(Default::default()),
                        ..Default::default()
                    };

                    assert_noop!(context.dispatch_set_voting_period(), BadOrigin);
                    assert_ne!(context.new_voting_period, Summary::voting_period());
                });
            }

            #[test]
            fn origin_is_unsigned() {
                let mut ext = ExtBuilder::build_default()
                    .with_validators()
                    .with_genesis_config()
                    .as_externality();
                ext.execute_with(||{
                    let context: Context = Context {
                        origin: RawOrigin::None.into(),
                        ..Default::default()
                    };

                    assert_noop!(context.dispatch_set_voting_period(), BadOrigin);
                    assert_ne!(context.new_voting_period, Summary::voting_period());
                });
            }

            #[test]
            fn less_than_report_latency_should_fail() {
                let mut ext = ExtBuilder::build_default()
                    .with_validators()
                    .with_genesis_config()
                    .as_externality();
                ext.execute_with(||{
                    let context: Context = Context {
                        new_voting_period: (<TestRuntime as Config>::FinalityReportLatency::get() - 1).into(),
                        ..Default::default()
                    };

                    assert_noop!(context.dispatch_set_voting_period(), Error::<TestRuntime>::VotingPeriodIsLessThanFinalityReportLatency);
                    assert_ne!(context.new_voting_period, Summary::voting_period());
                });
            }

            #[test]
            fn less_than_minimum_value_should_fail() {
                let mut ext = ExtBuilder::build_default()
                    .with_validators()
                    .with_genesis_config()
                    .as_externality();
                ext.execute_with(||{
                    let context: Context = Context {
                        new_voting_period: (MIN_VOTING_PERIOD - 1).into(),
                        ..Default::default()
                    };

                    assert_noop!(context.dispatch_set_voting_period(), Error::<TestRuntime>::VotingPeriodIsTooShort);
                    assert_ne!(context.new_voting_period, Summary::voting_period());
                });
            }

            #[test]
            fn equal_to_schedule_period_should_fail() {
                let mut ext = ExtBuilder::build_default()
                    .with_validators()
                    .with_genesis_config()
                    .as_externality();
                ext.execute_with(||{
                    let context: Context = Context {
                        new_voting_period: (Summary::schedule_period()).into(),
                        ..Default::default()
                    };

                    assert_noop!(context.dispatch_set_voting_period(), Error::<TestRuntime>::VotingPeriodIsEqualOrLongerThanSchedulePeriod);
                    assert_ne!(context.new_voting_period, Summary::voting_period());
                });
            }

            #[test]
            fn greater_than_schedule_period_should_fail() {
                let mut ext = ExtBuilder::build_default()
                    .with_validators()
                    .with_genesis_config()
                    .as_externality();
                ext.execute_with(||{
                    let context: Context = Context {
                        new_voting_period: (Summary::schedule_period() + 1).into(),
                        ..Default::default()
                    };

                    assert_noop!(context.dispatch_set_voting_period(), Error::<TestRuntime>::VotingPeriodIsEqualOrLongerThanSchedulePeriod);
                    assert_ne!(context.new_voting_period, Summary::voting_period());
                });
            }
        }
    }
}