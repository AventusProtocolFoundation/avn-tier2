//! # AVN Offence Handler Pallet
//!
//! This pallet provides functionality to call ethereum transaction to slash the offender.
//! and implements the OnOffenceHandler trait defined in sp_staking.

#![cfg_attr(not(feature = "std"), no_std)]

use sp_std::prelude::*;
use frame_support::{decl_error, decl_event, decl_module, decl_storage, dispatch::DispatchResult};
use frame_system::{self as system, ensure_root};
use sp_runtime::{Perbill};
use sp_staking::{
    offence::{OffenceDetails, OnOffenceHandler},
    SessionIndex,
};
use pallet_avn::{Enforcer, ValidatorRegistrationNotifier};
use pallet_session::{self as session, historical::IdentificationTuple};

#[cfg(test)]
mod mock;

#[cfg(test)]
#[path = "../../avn/src/tests/extension_builder.rs"]
pub mod extension_builder;

#[cfg(test)]
mod tests;

mod benchmarking;

// TODO: [TYPE: business logic][PRI: high][CRITICAL]
// Rerun benchmark in production and update both ./default_weights.rs file and /bin/node/runtime/src/weights/pallet_avn_offence_handler.rs file.
pub mod default_weights;
pub use default_weights::WeightInfo;

pub trait Config: system::Config + session::historical::Config {
    /// Overarching event type
    type Event: From<Event<Self>> + Into<<Self as system::Config>::Event>;
    /// A trait responsible for punishing malicious validators
    type Enforcer: Enforcer<<Self as session::Config>::ValidatorId>;
    /// Weight information for the extrinsics in this pallet.
    type WeightInfo: WeightInfo;
}

decl_event!(
    pub enum Event<T>
    where
        ValidatorId = <T as session::Config>::ValidatorId,
    {
        /// One validator has been reported.
        ReportedOffence(/*offender: */ ValidatorId),
        /// True if slashing is enable, otherwise False
        SlashingConfigurationUpdated(bool),
    }
);

decl_error! {
    pub enum Error for Module<T: Config> {
    }
}

decl_storage! {
    trait Store for Module<T: Config> as AvnOffenceHandler {
        /// A false value means the offence for the validator was not applied successfully.
        pub ReportedOffenders get(fn get_reported_offender): map hasher(blake2_128_concat) T::ValidatorId => bool;
        /// A flag to control if slashing is enabled
        pub SlashingEnabled get(fn can_slash): bool = false;
    }
}

decl_module! {
    pub struct Module<T: Config> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;

        /// # <weight>
        /// - DbWrites: `SlashingEnabled`: O(1)
        /// - Emit event: `SlashingConfigurationUpdated`: O(1)
        /// Total Complexity: `O(1)`
        /// # </weight>
        #[weight = <T as Config>::WeightInfo::configure_slashing()]
        pub fn configure_slashing(origin, enabled: bool) -> DispatchResult {
            let _sender = ensure_root(origin)?;
            SlashingEnabled::put(enabled);

            Self::deposit_event(Event::<T>::SlashingConfigurationUpdated(enabled));
            Ok(())
        }
    }
}

impl<T: Config> Module<T> {
    pub fn setup_for_new_validator(new_validator_id: &<T as session::Config>::ValidatorId) {
        <ReportedOffenders<T>>::remove(new_validator_id);
    }
}

impl<Res: Default, T: Config> OnOffenceHandler<T::AccountId, IdentificationTuple<T>, Res> for Module<T>
{
    // This function must not error because failed offences will be retried forever.
    fn on_offence(
        offenders: &[OffenceDetails<T::AccountId, IdentificationTuple<T>>], // A list containing both current offenders and previous offenders
        _slash_fraction: &[Perbill],
        _session: SessionIndex,
    ) -> Result<Res, ()> {
        offenders.iter()
            .filter(|&detail|{
                !<ReportedOffenders<T>>::contains_key(&detail.offender.0)
            })
            .for_each(|detail|{
                let offender_account_id = &detail.offender.0;
                Self::deposit_event(Event::<T>::ReportedOffence(offender_account_id.clone()));

                let mut result: bool = false;

                if Self::can_slash() {
                    result = T::Enforcer::slash_validator(&offender_account_id.clone()).is_ok();
                }

                <ReportedOffenders<T>>::insert(offender_account_id.clone(), result);
            });

        // TODO: Return the actual weight used here
        return Ok(Default::default());
    }

    /// Can an offence be reported now or not.
    fn can_report() -> bool {
        true // on_offence can be reported at any time for now
    }
}

impl<T:Config> ValidatorRegistrationNotifier<<T as session::Config>::ValidatorId> for Module<T> {
    fn on_validator_registration(validator_id: &<T as session::Config>::ValidatorId) {
        Self::setup_for_new_validator(validator_id);
    }
}
