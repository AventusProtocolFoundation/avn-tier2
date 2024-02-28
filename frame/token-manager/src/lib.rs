// This file is part of Aventus.
// Copyright (C) 2022 Aventus Network Services (UK) Ltd.

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! # Token manager pallet

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use core::convert::{TryFrom, TryInto};
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage,
    dispatch::DispatchResultWithPostInfo,
    ensure,
    traits::{Currency, ExistenceRequirement, Imbalance, IsSubType, WithdrawReasons},
    weights::GetDispatchInfo,
    Parameter,
};
use frame_system::{self as system, ensure_signed};
use pallet_avn::{self as avn};
use pallet_ethereum_events::{self as ethereum_events, ProcessedEventsChecker};
use sp_avn_common::{
    event_types::{EthEvent, EventData, ProcessedEventHandler},
    CallDecoder, InnerCallValidator, Proof,
};
use sp_core::{H160, H256};
use sp_runtime::{
    traits::{AtLeast32Bit, CheckedAdd, Dispatchable, Hash, IdentifyAccount, Member, Verify, Zero},
    DispatchResult,
};
use sp_std::prelude::*;

type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type PositiveImbalanceOf<T> = <<T as Config>::Currency as Currency<
    <T as frame_system::Config>::AccountId,
>>::PositiveImbalance;

mod benchmarking;

pub mod default_weights;
pub use default_weights::WeightInfo;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod test_proxying_signed_transfer;

#[cfg(test)]
mod test_proxying_signed_lower;

#[cfg(test)]
mod test_common_cases;

#[cfg(test)]
mod test_avt_tokens;

#[cfg(test)]
mod test_non_avt_tokens;

pub const SIGNED_TRANSFER_CONTEXT: &'static [u8] = b"authorization for transfer operation";
pub const SIGNED_LOWER_CONTEXT: &'static [u8] = b"authorization for lower operation";

pub trait Config: system::Config + avn::Config {
    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as system::Config>::Event>;

    /// The overarching call type.
    type Call: Parameter
        + Dispatchable<Origin = <Self as frame_system::Config>::Origin>
        + IsSubType<Call<Self>>
        + From<Call<Self>>
        + GetDispatchInfo;

    /// Currency type for lifting
    type Currency: Currency<Self::AccountId>;

    /// The units in which we record balances of tokens others than AVT
    type TokenBalance: Member + Parameter + AtLeast32Bit + Default + Copy;

    /// The type of token identifier
    /// (a H160 because this is an Ethereum address)
    type TokenId: Parameter + Default + Copy + From<H160>;

    type ProcessedEventsChecker: ProcessedEventsChecker;

    /// A type that can be used to verify signatures
    type Public: IdentifyAccount<AccountId = Self::AccountId>;

    /// The signature type used by accounts/transactions.
    type Signature: Verify<Signer = Self::Public>
        + Member
        + Decode
        + Encode
        + From<sp_core::sr25519::Signature>;

    type WeightInfo: WeightInfo;
}

decl_event!(
    pub enum Event<T>
    where
        RecipientAccountId = <T as system::Config>::AccountId,
        SenderAccountId = <T as system::Config>::AccountId,
        Relayer = <T as system::Config>::AccountId,
        EthTxHash = H256,
        Hash = <T as system::Config>::Hash,
        AmountLifted = BalanceOf<T>,
        TokenBalance = <T as Config>::TokenBalance,
        TokenId = <T as Config>::TokenId,
        AmountLowered = u128,
        T1Recipient = H160,
    {
        AVTLifted(RecipientAccountId, AmountLifted, EthTxHash),
        TokenLifted(TokenId, RecipientAccountId, TokenBalance, EthTxHash),
        TokenTransferred(TokenId, SenderAccountId, RecipientAccountId, TokenBalance),
        CallDispatched(Relayer, Hash),
        TokenLowered(
            TokenId,
            SenderAccountId,
            RecipientAccountId,
            AmountLowered,
            T1Recipient,
        ),
    }
);

decl_error! {
    pub enum Error for Module<T: Config> {
        NoTier1EventForLogLifted,
        AmountOverflow,
        DepositFailed,
        LowerFailed,
        AmountIsZero,
        InsufficientSenderBalance,
        TransactionNotSupported,
        SenderNotValid,
        UnauthorizedTransaction,
        UnauthorizedProxyTransaction,
        UnauthorizedSignedTransferTransaction,
        UnauthorizedSignedLowerTransaction,
        ErrorConvertingAccountId,
        ErrorConvertingTokenBalance,
        ErrorConvertingToBalance,
    }
}

decl_storage! {
    trait Store for Module<T: Config> as TokenManager {
        /// The number of units of tokens held by any given account.
        pub Balances get(fn balance): map hasher(blake2_128_concat) (T::TokenId, T::AccountId) => T::TokenBalance;

        /// An account nonce that represents the number of transfers from this account
        /// It is shared for all tokens held by the account
        pub Nonces get(fn nonce): map hasher(blake2_128_concat) T::AccountId => u64;

        /// An account without a known private key, that can send transfers (eg Lowering transfers) but from which no one can send funds. Tokens sent to this account are effectively destroyed.
        pub LowerAccountId get(fn lower_account_id) config(): H256;

        /// The ethereum address of the AVT contract. Default value is the Rinkeby address
        pub AVTTokenContract get(fn avt_token_contract) config(): H160;
    }
}

decl_module! {
    pub struct Module<T: Config> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;

        /// This extrinsic allows relayer to dispatch a `signed_transfer` or `signed_lower` call for a sender
        ///
        /// As a general rule, every function that can be proxied should follow this convention:
        /// - its first argument (after origin) should be a public verification key and a signature
        ///
        /// # <weight>
        /// - One get proof operation: O(1)
        /// - One hash of operation: O(1)
        /// - One signed transfer operation: O(1)
        /// - One event emitted: O(1)
        /// Total Complexity: `O(1)`
        /// # </weight>
        #[weight = T::WeightInfo::proxy_with_non_avt_token().saturating_add(call.get_dispatch_info().weight)]
        pub fn proxy(origin, call: Box<<T as Config>::Call>) -> DispatchResult {
            let relayer = ensure_signed(origin)?;

            let proof = Self::get_proof(&*call)?;
            ensure!(relayer == proof.relayer, Error::<T>::UnauthorizedProxyTransaction);

            let call_hash: T::Hash = T::Hashing::hash_of(&call);
            call.dispatch(frame_system::RawOrigin::Signed(proof.signer).into()).map(|_| ()).map_err(|e| e.error)?;
            Self::deposit_event(RawEvent::CallDispatched(relayer, call_hash));
            Ok(())
        }

        /// Transfer an amount of token with token_id from sender to receiver with a proof
        ///
        /// # <weight>
        /// - Db reads:   one `Nonces`, two `Balances`: O(1)
        /// - Db mutates: one `Nonces`, two `Balances`: O(1)
        /// - One codec encode operation: O(1).
        /// - One signature verification operation: O(1).
        /// - One event emitted: O(1).
        /// Total Complexity: `O(1)`
        /// # <weight>
        #[weight = T::WeightInfo::signed_transfer()]
        pub fn signed_transfer(
            origin,
            proof: Proof<T::Signature, T::AccountId>,
            from: T::AccountId,
            to: T::AccountId,
            token_id: T::TokenId,
            amount: T::TokenBalance,
        ) -> DispatchResult {

            let sender = ensure_signed(origin)?;
            ensure!(sender == from, Error::<T>::SenderNotValid);
            let sender_nonce = Self::nonce(&sender);

            let signed_payload = Self::encode_signed_transfer_params(&proof, &from, &to, &token_id, &amount, sender_nonce);

            ensure!(Self::verify_signature(&proof, &signed_payload.as_slice()).is_ok(),
                Error::<T>::UnauthorizedSignedTransferTransaction);

            Self::settle_transfer(&token_id, &from, &to, &amount)?;

            Self::deposit_event(RawEvent::TokenTransferred(token_id, from, to, amount));
            Ok(())
        }

        /// Lower an amount of token from tier2 to tier1
        ///
        /// # <weight>
        /// Key: W (currency withdraw operation),
        /// - Two storage read to get the lower account id: O(1).
        /// - One codec decode operation: O(1).
        /// - One settle lower operation:
        ///     AVT Token
        ///         - One currency withdraw operation: O(W).
        ///     Non AVT Token
        ///         - One storage read of an account balance: O(1).
        ///         - One storage mutate of an account balance: O(1).
        /// - One event emitted: O(1).
        /// Total Complexity: `O(1 + W)`
        /// # </weight>
        #[weight = T::WeightInfo::lower_avt_token().max(T::WeightInfo::lower_non_avt_token())]
        pub fn lower(
            origin,
            from: T::AccountId,
            token_id: T::TokenId,
            amount: u128,
            t1_recipient: H160 // the receiver address on tier1
        ) -> DispatchResultWithPostInfo
        {
            let sender = ensure_signed(origin)?;
            ensure!(sender == from, Error::<T>::SenderNotValid);
            ensure!(amount != 0, Error::<T>::AmountIsZero);

            let to_account_id = T::AccountId::decode(&mut Self::lower_account_id().as_bytes()).map_err(|_| Error::<T>::ErrorConvertingAccountId)?;

            Self::settle_lower(&from, token_id, amount)?;

            Self::deposit_event(RawEvent::TokenLowered(token_id, from, to_account_id, amount, t1_recipient));

            let final_weight = if token_id == Self::avt_token_contract().into() {
                T::WeightInfo::lower_avt_token()
            } else {
                T::WeightInfo::lower_non_avt_token()
            };

            Ok(Some(final_weight).into())
        }

        /// Lower an amount of token from tier2 to tier1 by a relayer
        ///
        /// # <weight>
        /// Key: W (currency withdraw operation),
        /// - DbReads: 2 * LowerAccountId, Nonce: O(1).
        /// - DbWrites: Nonce: O(1).
        /// - One codec encode operation: O(1).
        /// - One codec decode operation: O(1).
        /// - One signature verification operation: O(1).
        /// - One settle lower operation:
        ///     AVT Token
        ///         - One currency withdraw operation: O(W).
        ///     Non AVT Token
        ///         - One storage read of an account balance: O(1).
        ///         - One storage mutate of an account balance: O(1).
        /// - One event emitted: O(1).
        /// Total Complexity: `O(1 + W)`
        /// # </weight>
        #[weight = T::WeightInfo::signed_lower_avt_token().max(T::WeightInfo::signed_lower_non_avt_token())]
        pub fn signed_lower(
            origin,
            proof: Proof<T::Signature, T::AccountId>,
            from: T::AccountId,
            token_id: T::TokenId,
            amount: u128,
            t1_recipient: H160 // the receiver address on tier1
        ) -> DispatchResultWithPostInfo
        {
            let sender = ensure_signed(origin)?;
            ensure!(sender == from, Error::<T>::SenderNotValid);
            ensure!(amount != 0, Error::<T>::AmountIsZero);

            let sender_nonce = Self::nonce(&sender);
            let signed_payload = Self::encode_signed_lower_params(&proof, &from, &token_id, &amount, &t1_recipient, sender_nonce);

            ensure!(Self::verify_signature(&proof, &signed_payload.as_slice()).is_ok(),
                Error::<T>::UnauthorizedSignedLowerTransaction);

            let to_account_id = T::AccountId::decode(&mut Self::lower_account_id().as_bytes()).map_err(|_| Error::<T>::ErrorConvertingAccountId)?;

            Self::settle_lower(&from, token_id, amount)?;

            Self::deposit_event(RawEvent::TokenLowered(token_id, from, to_account_id, amount, t1_recipient));

            let final_weight = if token_id == Self::avt_token_contract().into() {
                T::WeightInfo::signed_lower_avt_token()
            } else {
                T::WeightInfo::signed_lower_non_avt_token()
            };

            Ok(Some(final_weight).into())
        }
    }
}

impl<T: Config> Module<T> {
    fn settle_transfer(
        token_id: &T::TokenId,
        from: &T::AccountId,
        to: &T::AccountId,
        amount: &T::TokenBalance,
    ) -> DispatchResult {
        if *token_id == Self::avt_token_contract().into() {
            // First convert TokenBalance to u128
            let amount_u128 = TryInto::<u128>::try_into(*amount)
                .map_err(|_| Error::<T>::ErrorConvertingTokenBalance)?;
            // Then convert to Balance
            let transfer_amount = <BalanceOf<T> as TryFrom<u128>>::try_from(amount_u128)
                .or_else(|_error| Err(Error::<T>::ErrorConvertingToBalance))?;

            T::Currency::transfer(from, to, transfer_amount, ExistenceRequirement::KeepAlive)?;
        } else {
            let sender_balance = Self::balance((token_id, from));
            ensure!(
                sender_balance >= *amount,
                Error::<T>::InsufficientSenderBalance
            );

            if from != to {
                // If we are transfering to ourselves, we need to be careful when reading the balance because
                // `Self::balance((token_id, from))` == `Self::balance((token_id, to))` hence the if statement.
                let receiver_balance = Self::balance((token_id, to));
                ensure!(
                    receiver_balance.checked_add(amount).is_some(),
                    Error::<T>::AmountOverflow
                );
            }

            <Balances<T>>::mutate((token_id, from), |balance| *balance -= *amount);

            <Balances<T>>::mutate((token_id, to), |balance| *balance += *amount);
        }

        <Nonces<T>>::mutate(from, |n| *n += 1);

        Ok(())
    }

    fn lift(event: &EthEvent) -> DispatchResult {
        if let EventData::LogLifted(d) = &event.event_data {
            let event_id = &event.event_id;
            let recipient_account_id = T::AccountId::decode(&mut d.receiver_address.as_bytes())
                .expect("32 bytes will always decode into an AccountId");

            let event_validity = T::ProcessedEventsChecker::check_event(event_id);
            ensure!(event_validity, Error::<T>::NoTier1EventForLogLifted);

            if d.amount == 0 {
                Err(Error::<T>::AmountIsZero)?
            }

            if d.token_contract == Self::avt_token_contract() {
                return Self::update_avt_balance(
                    event_id.transaction_hash,
                    recipient_account_id,
                    d.amount,
                );
            }

            return Self::update_token_balance(
                event_id.transaction_hash,
                d.token_contract.into(),
                recipient_account_id,
                d.amount,
            );
        }

        // Event handled or it is not for us, in which case ignore it.
        Ok(())
    }

    fn settle_lower(from: &T::AccountId, token_id: T::TokenId, amount: u128) -> DispatchResult {
        if token_id == Self::avt_token_contract().into() {
            let lower_amount = <BalanceOf<T> as TryFrom<u128>>::try_from(amount)
                .or_else(|_error| Err(Error::<T>::AmountOverflow))?;
            // Note: Keep account alive when balance is lower than existence requirement,
            //       so the SystemNonce will not be reset just in case if any logic relies on the SystemNonce.
            //       However all zero AVT account balances will be kept in our runtime storage
            let imbalance = T::Currency::withdraw(
                &from,
                lower_amount,
                WithdrawReasons::TRANSFER,
                ExistenceRequirement::KeepAlive,
            )?;

            if imbalance.peek() == BalanceOf::<T>::zero() {
                Err(Error::<T>::LowerFailed)?
            }

            // Decreases the total issued AVT when this negative imbalance is dropped
            // so that total issued AVT becomes equal to total supply once again.
            drop(imbalance);
        } else {
            let lower_amount = <T::TokenBalance as TryFrom<u128>>::try_from(amount)
                .or_else(|_error| Err(Error::<T>::AmountOverflow))?;
            let sender_balance = Self::balance((token_id, from));
            ensure!(
                sender_balance >= lower_amount,
                Error::<T>::InsufficientSenderBalance
            );

            <Balances<T>>::mutate((token_id, from), |balance| *balance -= lower_amount);
        }

        <Nonces<T>>::mutate(from, |n| *n += 1);

        Ok(())
    }

    fn update_token_balance(
        transaction_hash: H256,
        token_id: T::TokenId,
        recipient_account_id: T::AccountId,
        raw_amount: u128,
    ) -> DispatchResult {
        let amount = <T::TokenBalance as TryFrom<u128>>::try_from(raw_amount)
            .or_else(|_error| Err(Error::<T>::AmountOverflow))?;

        if <Balances<T>>::contains_key((token_id, &recipient_account_id)) {
            Self::increment_token_balance(token_id, &recipient_account_id, &amount)?;
        } else {
            <Balances<T>>::insert((token_id, &recipient_account_id), amount);
        }

        Self::deposit_event(Event::<T>::TokenLifted(
            token_id,
            recipient_account_id,
            amount,
            transaction_hash,
        ));

        Ok(())
    }

    fn update_avt_balance(
        transaction_hash: H256,
        recipient_account_id: T::AccountId,
        raw_amount: u128,
    ) -> DispatchResult {
        let amount = <BalanceOf<T> as TryFrom<u128>>::try_from(raw_amount)
            .or_else(|_error| Err(Error::<T>::AmountOverflow))?;

        // Drop the imbalance caused by depositing amount into the recipient account without a corresponding deduction
        // If the recipient account does not exist, deposit_creating function will create a new one.
        let imbalance: PositiveImbalanceOf<T> =
            T::Currency::deposit_creating(&recipient_account_id, amount);

        if imbalance.peek() == BalanceOf::<T>::zero() {
            Err(Error::<T>::DepositFailed)?
        }

        // Increases the total issued AVT when this positive imbalance is dropped
        // so that total issued AVT becomes equal to total supply once again.
        drop(imbalance);

        Self::deposit_event(Event::<T>::AVTLifted(
            recipient_account_id,
            amount,
            transaction_hash,
        ));

        Ok(())
    }

    fn increment_token_balance(
        token_id: T::TokenId,
        recipient_account_id: &T::AccountId,
        amount: &T::TokenBalance,
    ) -> DispatchResult {
        let current_balance = Self::balance((token_id, recipient_account_id));
        let new_balance = current_balance
            .checked_add(amount)
            .ok_or(Error::<T>::AmountOverflow)?;

        <Balances<T>>::mutate((token_id, recipient_account_id), |balance| {
            *balance = new_balance
        });

        Ok(())
    }

    fn verify_signature(
        proof: &Proof<T::Signature, T::AccountId>,
        signed_payload: &[u8],
    ) -> Result<(), Error<T>> {
        match proof.signature.verify(signed_payload, &proof.signer) {
            true => Ok(()),
            false => Err(<Error<T>>::UnauthorizedTransaction.into()),
        }
    }

    fn encode_signed_transfer_params(
        proof: &Proof<T::Signature, T::AccountId>,
        from: &T::AccountId,
        to: &T::AccountId,
        token_id: &T::TokenId,
        amount: &T::TokenBalance,
        sender_nonce: u64,
    ) -> Vec<u8> {
        return (
            SIGNED_TRANSFER_CONTEXT,
            proof.relayer.clone(),
            from,
            to,
            token_id,
            amount,
            sender_nonce,
        )
            .encode();
    }

    fn encode_signed_lower_params(
        proof: &Proof<T::Signature, T::AccountId>,
        from: &T::AccountId,
        token_id: &T::TokenId,
        amount: &u128,
        t1_recipient: &H160,
        sender_nonce: u64,
    ) -> Vec<u8> {
        return (
            SIGNED_LOWER_CONTEXT,
            proof.relayer.clone(),
            from,
            token_id,
            amount,
            t1_recipient,
            sender_nonce,
        )
            .encode();
    }

    fn get_encoded_call_param(
        call: &<T as Config>::Call,
    ) -> Option<(&Proof<T::Signature, T::AccountId>, Vec<u8>)> {
        let call = match call.is_sub_type() {
            Some(call) => call,
            None => return None,
        };

        match call {
            Call::signed_transfer(proof, from, to, token_id, amount) => {
                let sender_nonce = Self::nonce(&proof.signer);
                let encoded_data = Self::encode_signed_transfer_params(
                    proof,
                    from,
                    to,
                    token_id,
                    amount,
                    sender_nonce,
                );

                return Some((proof, encoded_data));
            }
            Call::signed_lower(proof, from, token_id, amount, t1_recipient) => {
                let sender_nonce = Self::nonce(&proof.signer);
                let encoded_data = Self::encode_signed_lower_params(
                    proof,
                    from,
                    token_id,
                    amount,
                    t1_recipient,
                    sender_nonce,
                );

                return Some((proof, encoded_data));
            }
            _ => return None,
        }
    }
}

impl<T: Config + ethereum_events::Config> ProcessedEventHandler for Module<T> {
    fn on_event_processed(event: &EthEvent) -> DispatchResult {
        return Self::lift(event);
    }
}

impl<T: Config> CallDecoder for Module<T> {
    type AccountId = T::AccountId;
    type Signature = <T as Config>::Signature;
    type Error = Error<T>;
    type Call = <T as Config>::Call;

    fn get_proof(
        call: &Self::Call,
    ) -> Result<Proof<Self::Signature, Self::AccountId>, Self::Error> {
        let call = match call.is_sub_type() {
            Some(call) => call,
            None => return Err(Error::TransactionNotSupported),
        };

        match call {
            Call::signed_transfer(proof, _from, _to, _token_id, _amount) => {
                return Ok(proof.clone())
            }
            Call::signed_lower(proof, _from, _token_id, _amount, _t1_recipient) => {
                return Ok(proof.clone())
            }
            _ => return Err(Error::TransactionNotSupported),
        }
    }
}

impl<T: Config> InnerCallValidator for Module<T> {
    type Call = <T as Config>::Call;

    fn signature_is_valid(call: &Box<Self::Call>) -> bool {
        if let Some((proof, signed_payload)) = Self::get_encoded_call_param(call) {
            return Self::verify_signature(&proof, &signed_payload.as_slice()).is_ok();
        }

        return false;
    }
}
