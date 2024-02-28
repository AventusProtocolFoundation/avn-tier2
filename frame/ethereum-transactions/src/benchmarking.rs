//! # Ethereum transactions pallet
// Copyright 2020 Artos Systems (UK) Ltd.

//! ethereum transactions pallet benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

#[path = "ethereum_transaction.rs"]
pub mod ethereum_transaction;
use crate::ethereum_transaction::{EthTransactionType, PublishRootData, TransactionId};

use frame_benchmarking::{account, benchmarks};
use frame_system::{EventRecord, RawOrigin};
use pallet_avn::{self as avn};

pub const ROOT_HASH_BYTES: [u8; 32] = [
    135, 54, 201, 230, 113, 254, 88, 31, 228, 239, 70, 49, 17, 32, 56, 41, 125, 205, 236, 174, 22,
    62, 135, 36, 194, 129, 236, 232, 173, 148, 200, 195,
];

fn setup_eth_tx_and_dispatched_tx<T: Config>(number_of_validators: u32, number_of_txn_ids: u32) -> (
    T::AccountId, TransactionId, EthereumTransactionHash, <<T as avn::Config>::AuthorityId as RuntimeAppPublic>::Signature
) {
    // Setup validators
    let mnemonic: &str = "news slush supreme milk chapter athlete soap sausage put clutch what kitten";
    let mut validators: Vec<Validator<<T as pallet_avn::Config>::AuthorityId, T::AccountId>> = Vec::new();
    for i in 0..number_of_validators {
        let key = <T as avn::Config>::AuthorityId::generate_pair(Some(mnemonic.as_bytes().to_vec()));
        let account = account("dummy_account", i, i);
        validators.push(Validator::new(account, key));
    }
    avn::Validators::<T>::put(validators.clone());

    // Setup transaction ids
    let tx_ids = create_tx_ids::<T>(number_of_txn_ids);

    // Prepare results
    let submitter: T::AccountId = validators[0].account_id.clone();
    let candidate_tx_id = tx_ids[0];
    let eth_tx_hash: EthereumTransactionHash = H256::from([1; 32]);
    let signature = generate_signature::<T>();

    // Setup storages to pass the test
    add_tx_ids_to_account::<T>(&submitter, tx_ids.clone());
    add_eth_tx_candidate_to_candidate_tx_id::<T>(&submitter, candidate_tx_id);

    return (submitter, candidate_tx_id, eth_tx_hash, signature);
}

fn add_eth_tx_candidate_to_candidate_tx_id<T: Config>(account: &T::AccountId, candidate_tx_id: TransactionId) {
    let tx_id = 0u64;
    let from = Some(T::AccountToBytesConvert::into_bytes(&account.clone()));
    let quorum: u32 = 1;
    let eth_tx_candidate = EthTransactionCandidate::new(tx_id, from, EthTransactionType::Invalid, quorum);

    Repository::insert(candidate_tx_id, eth_tx_candidate);
}

fn add_tx_ids_to_account<T: Config>(account: &T::AccountId, candidate_tx_ids: Vec<TransactionId>) {
    let dispatch_data: Vec<DispatchedData::<T::BlockNumber>> = candidate_tx_ids.iter().map(|id| DispatchedData::new(*id, 0u32.into())).collect();
    DispatchedAvnTxIds::<T>::insert(account, dispatch_data);
}

fn create_tx_ids<T: Config>(number_of_txn_ids: u32) -> Vec<TransactionId> {
    let mut tx_ids: Vec<TransactionId> = Vec::new();
    for i in 0..number_of_txn_ids {
        tx_ids.push(i.into());
    }

    return tx_ids;
}

fn generate_signature<T: pallet_avn::Config>() -> <<T as avn::Config>::AuthorityId as RuntimeAppPublic>::Signature {
    let encoded_data = 0.encode();
    let authority_id = T::AuthorityId::generate_pair(None);
    let signature = authority_id.sign(&encoded_data).expect("able to make signature");

    return signature;
}

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

benchmarks! {
    set_transaction_id {
        let transaction_id: TransactionId = 1u64.into();
    }: _(RawOrigin::Root, transaction_id)
    verify {
        assert_eq!(Nonce::get(), transaction_id);
    }

    unreserve_transaction {
        let tx_id: TransactionId = 1;
        let transaction_type: EthTransactionType = EthTransactionType::PublishRoot(PublishRootData::new(ROOT_HASH_BYTES));
        // Insert value in the reserved list
        <ReservedTransactions>::insert(&transaction_type, tx_id);
    }: _(RawOrigin::Root, transaction_type.clone())
    verify {
        assert_eq!(false, <ReservedTransactions>::contains_key(&transaction_type));
        assert_eq!(true, <ReservedTransactions>::contains_key(&EthTransactionType::Discarded(tx_id)));
    }

    set_eth_tx_hash_for_dispatched_tx {
        let v in 1 .. MAX_VALIDATORS;
        let t in 1 .. MAX_TXS_PER_ACCOUNT;
        let (submitter, candidate_tx_id, eth_tx_hash, signature) = setup_eth_tx_and_dispatched_tx::<T>(v, t);
    }: _(RawOrigin::None, submitter, candidate_tx_id, eth_tx_hash, signature)
    verify {
        assert_eq!(true, Repository::contains_key(candidate_tx_id));
        assert_eq!(Repository::get(candidate_tx_id).get_eth_tx_hash(), Some(eth_tx_hash));
        assert_last_event::<T>(RawEvent::EthereumTransactionHashAdded(candidate_tx_id, eth_tx_hash).into());
    }

    set_publish_root_contract_storage {
        let contract_address = H160::from([1; 20]);
    }: set_publish_root_contract(RawOrigin::Root, contract_address.clone())
    verify {
        assert_eq!(PublishRootContract::get(), contract_address);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::*;
    use frame_support::assert_ok;

    #[test]
    fn benchmarks() {
        let mut ext = ExtBuilder::build_default().as_externality();

        ext.execute_with(|| {
            assert_ok!(test_benchmark_set_transaction_id::<TestRuntime>());
            assert_ok!(test_benchmark_unreserve_transaction::<TestRuntime>());
            assert_ok!(test_benchmark_set_eth_tx_hash_for_dispatched_tx::<TestRuntime>());
            assert_ok!(test_benchmark_set_publish_root_contract_storage::<TestRuntime>());
        });
    }
}
