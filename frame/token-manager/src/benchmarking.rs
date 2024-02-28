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

//! token manager pallet benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use sp_core::sr25519;
use sp_runtime::RuntimeAppPublic;

pub const AVT_TOKEN_CONTRACT: H160 = H160(hex!("405dF1B38510C455Ef81500A3dC7e9ae599e18f6"));

fn assert_last_event<T: Config>(generic_event: <T as Config>::Event) {
    assert_last_nth_event::<T>(generic_event, 1);
}

fn assert_last_nth_event<T: Config>(generic_event: <T as Config>::Event, n: u32) {
    let events = frame_system::Module::<T>::events();
    let system_event: <T as frame_system::Config>::Event = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len().saturating_sub(n as usize)];
    assert_eq!(event, &system_event);
}

fn into_bytes<T: Config>(account: &<T as avn::Config>::AuthorityId) -> [u8; 32] {
    let bytes = account.encode();
    let mut vector: [u8; 32] = Default::default();
    vector.copy_from_slice(&bytes[0..32]);
    return vector;
}

struct Transfer<T: Config> {
    relayer: T::AccountId,
    from: T::AccountId,
    to: T::AccountId,
    token_id: T::TokenId,
    amount: T::TokenBalance,
    nonce: u64,
}

impl<T: Config> Transfer<T> {
    fn new(token_id: H160) -> Self {
        let mnemonic: &str =
            "news slush supreme milk chapter athlete soap sausage put clutch what kitten";
        let key_pair =
            <T as avn::Config>::AuthorityId::generate_pair(Some(mnemonic.as_bytes().to_vec()));
        let account_bytes = into_bytes::<T>(&key_pair);
        let account_id = T::AccountId::decode(&mut &account_bytes.encode()[..]).unwrap();

        let index = 2;
        let seed = 2;
        return Transfer {
            relayer: whitelisted_caller(),
            from: account_id,
            to: account("to", index, seed),
            token_id: token_id.into(),
            amount: 1000u32.into(),
            nonce: 0,
        };
    }

    fn setup(self) -> Self {
        Balances::<T>::insert((self.token_id, self.from.clone()), self.amount);
        Nonces::<T>::insert(self.from.clone(), self.nonce);
        return self;
    }

    fn generate_signed_transfer_call(&self, signature: &[u8]) -> <T as Config>::Call {
        let proof: Proof<T::Signature, T::AccountId> = self.get_proof(&self.relayer, signature);
        return Call::signed_transfer(
            proof,
            self.from.clone(),
            self.to.clone(),
            self.token_id,
            self.amount,
        )
        .into();
    }

    fn get_proof(
        &self,
        relayer: &T::AccountId,
        signature: &[u8],
    ) -> Proof<T::Signature, T::AccountId> {
        return Proof {
            signer: self.from.clone(),
            relayer: relayer.clone(),
            signature: sr25519::Signature::from_slice(signature).into(),
        };
    }
}

struct Lower<T: Config> {
    from_account_id: T::AccountId,
    lower_account: H256,
    lower_account_id: T::AccountId,
    amount: u32,
    non_avt_token_id: T::TokenId,
    t1_recipient: H160,
}

impl<T: Config> Lower<T> {
    fn new() -> Self {
        let mnemonic: &str =
            "news slush supreme milk chapter athlete soap sausage put clutch what kitten";
        let key_pair =
            <T as avn::Config>::AuthorityId::generate_pair(Some(mnemonic.as_bytes().to_vec()));
        let account_bytes = into_bytes::<T>(&key_pair);
        let from_account_id: T::AccountId =
            T::AccountId::decode(&mut &account_bytes.encode()[..]).unwrap();
        let lower_account: H256 = H256(hex!(
            "000000000000000000000000000000000000000000000000000000000000dead"
        ));
        let lower_account_id =
            T::AccountId::decode(&mut lower_account.as_bytes()).expect("valid lower account id");
        let non_avt_token_id: T::TokenId =
            H160(hex!("1414141414141414141414141414141414141414")).into();
        let t1_recipient: H160 = H160(hex!("afdf36201bf70F1232111b5c6a9a424558755134"));

        Lower {
            from_account_id: from_account_id,
            lower_account: lower_account,
            lower_account_id: lower_account_id,
            amount: 1000,
            non_avt_token_id: non_avt_token_id,
            t1_recipient: t1_recipient,
        }
    }

    fn setup(self) -> Self {
        // setup AVT token contract
        AVTTokenContract::put(AVT_TOKEN_CONTRACT);

        // setup non avt balance
        let lower_amount: T::TokenBalance = self.amount.into();
        Balances::<T>::insert(
            (self.non_avt_token_id, self.from_account_id.clone()),
            lower_amount,
        );

        // setup avt balance
        T::Currency::make_free_balance_be(&self.from_account_id, self.amount.into());

        // setup lower account id
        LowerAccountId::put(self.lower_account);

        self
    }

    fn get_proof(
        &self,
        relayer_account_id: &T::AccountId,
        signature: &[u8],
    ) -> Proof<T::Signature, T::AccountId> {
        return Proof {
            signer: self.from_account_id.clone(),
            relayer: relayer_account_id.clone(),
            signature: sr25519::Signature::from_slice(signature).into(),
        };
    }
}

benchmarks! {
    proxy_with_non_avt_token {
        let signature = &hex!("a6350211fcdf1d7f0c79bf0a9c296de17449ca88a899f0cd19a70b07513fc107b7d34249dba71d4761ceeec2ed6bc1305defeb96418e6869e6b6199ed0de558e");
        let token_id = H160(hex!("1414141414141414141414141414141414141414"));
        let transfer: Transfer<T> = Transfer::new(token_id).setup();
        let call: <T as Config>::Call = transfer.generate_signed_transfer_call(signature);
        let boxed_call: Box<<T as Config>::Call> = Box::new(call);
        let call_hash: T::Hash = T::Hashing::hash_of(&boxed_call);
    }: proxy(RawOrigin::<T::AccountId>::Signed(transfer.relayer.clone()), boxed_call)
    verify {
        assert_eq!(Balances::<T>::get((transfer.token_id, transfer.from.clone())), 0u32.into());
        assert_eq!(Balances::<T>::get((transfer.token_id, transfer.to.clone())), transfer.amount);
        assert_eq!(Nonces::<T>::get(transfer.from.clone()), transfer.nonce + 1);
        assert_eq!(Nonces::<T>::get(transfer.to.clone()), 0);

        assert_last_event::<T>(RawEvent::CallDispatched(transfer.relayer.clone(), call_hash).into());
        assert_last_nth_event::<T>(RawEvent::TokenTransferred(
            transfer.token_id.clone(),
            transfer.from.clone(),
            transfer.to.clone(),
            transfer.amount
        ).into(), 2);
    }

    signed_transfer {
        let signature = &hex!("a875c83f0709276ffd87bf401d1563bd8bcabcfda24ebb51170b72d4cd5edd6e3816f56712fa4df421260447ff483f69bcdb5a55f6356c3ffedace7fee12288e");
        let token_id = H160(hex!("1414141414141414141414141414141414141414"));
        let transfer: Transfer<T> = Transfer::new(token_id).setup();
        let proof: Proof<T::Signature, T::AccountId> = transfer.get_proof(&transfer.from, signature);
    }: _ (
            RawOrigin::<T::AccountId>::Signed(transfer.from.clone()),
            proof,
            transfer.from.clone(),
            transfer.to.clone(),
            transfer.token_id,
            transfer.amount
        )
    verify {
        assert_eq!(Balances::<T>::get((transfer.token_id, transfer.from.clone())), 0u32.into());
        assert_eq!(Balances::<T>::get((transfer.token_id, transfer.to.clone())), transfer.amount);
        assert_eq!(Nonces::<T>::get(transfer.from.clone()), transfer.nonce + 1);
        assert_eq!(Nonces::<T>::get(transfer.to.clone()), 0);

        assert_last_event::<T>(RawEvent::TokenTransferred(
            transfer.token_id.clone(),
            transfer.from.clone(),
            transfer.to.clone(),
            transfer.amount
        ).into());
    }

    lower_avt_token {
        let lower: Lower<T> = Lower::new().setup();
    }: lower(
        RawOrigin::<T::AccountId>::Signed(lower.from_account_id.clone()),
        lower.from_account_id.clone(),
        AVT_TOKEN_CONTRACT.into(),
        lower.amount.into(),
        lower.t1_recipient
    )
    verify {
        assert_eq!(T::Currency::free_balance(&lower.from_account_id), 0u32.into());
        assert_last_event::<T>(RawEvent::TokenLowered(
            AVT_TOKEN_CONTRACT.into(),
            lower.from_account_id.clone(),
            lower.lower_account_id,
            lower.amount.into(),
            lower.t1_recipient
        ).into());
    }

    lower_non_avt_token {
        let lower: Lower<T> = Lower::new().setup();
    }: lower(
        RawOrigin::<T::AccountId>::Signed(lower.from_account_id.clone()),
        lower.from_account_id.clone(),
        lower.non_avt_token_id,
        lower.amount.into(),
        lower.t1_recipient
    )
    verify {
        assert_eq!(Balances::<T>::get((lower.non_avt_token_id, lower.from_account_id.clone())), 0u32.into());
        assert_last_event::<T>(RawEvent::TokenLowered(
            lower.non_avt_token_id,
            lower.from_account_id,
            lower.lower_account_id,
            lower.amount.into(),
            lower.t1_recipient
        ).into());
    }

    signed_lower_avt_token {
        let signature = &hex!("b45101430be66705c27cc38f2a090ff9750f213e108cec0aa92a5180e1d72a22e397e3ae8c58cc63b36cc32b338a60aa07d701b0e23a03e637bf32de578c8784");
        let lower: Lower<T> = Lower::new().setup();
        let proof: Proof<T::Signature, T::AccountId> = lower.get_proof(&lower.from_account_id, signature);
    }: signed_lower(
        RawOrigin::<T::AccountId>::Signed(lower.from_account_id.clone()),
        proof,
        lower.from_account_id.clone(),
        AVT_TOKEN_CONTRACT.into(),
        lower.amount.into(),
        lower.t1_recipient
    )
    verify {
        assert_eq!(T::Currency::free_balance(&lower.from_account_id), 0u32.into());
        assert_last_event::<T>(RawEvent::TokenLowered(
            AVT_TOKEN_CONTRACT.into(),
            lower.from_account_id.clone(),
            lower.lower_account_id,
            lower.amount.into(),
            lower.t1_recipient
        ).into());
    }

    signed_lower_non_avt_token {
        let signature = &hex!("82f8b0f7270a6b1c6221789a5b3192f557e8d9d9973f6fdd051762de3ef3b9396f8a5c3b86a62d6ff7934181112b6f2d9dd976d42226cb3258a5b61d5b43838e");
        let lower: Lower<T> = Lower::new().setup();
        let proof: Proof<T::Signature, T::AccountId> = lower.get_proof(&lower.from_account_id, signature);
    }: signed_lower(
        RawOrigin::<T::AccountId>::Signed(lower.from_account_id.clone()),
        proof,
        lower.from_account_id.clone(),
        lower.non_avt_token_id,
        lower.amount.into(),
        lower.t1_recipient
    )
    verify {
        assert_eq!(Balances::<T>::get((lower.non_avt_token_id, lower.from_account_id.clone())), 0u32.into());
        assert_last_event::<T>(RawEvent::TokenLowered(
            lower.non_avt_token_id,
            lower.from_account_id,
            lower.lower_account_id,
            lower.amount.into(),
            lower.t1_recipient
        ).into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::*;
    use frame_support::assert_ok;

    #[test]
    fn benchmarks() {
        let mut ext = ExtBuilder::build_default().with_balances().as_externality();

        ext.execute_with(|| {
            assert_ok!(test_benchmark_proxy_with_non_avt_token::<TestRuntime>());
            assert_ok!(test_benchmark_signed_transfer::<TestRuntime>());
            assert_ok!(test_benchmark_lower_avt_token::<TestRuntime>());
            assert_ok!(test_benchmark_lower_non_avt_token::<TestRuntime>());
            assert_ok!(test_benchmark_signed_lower_avt_token::<TestRuntime>());
            assert_ok!(test_benchmark_signed_lower_non_avt_token::<TestRuntime>());
        });
    }
}
