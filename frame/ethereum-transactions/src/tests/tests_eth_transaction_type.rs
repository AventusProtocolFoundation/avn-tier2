#![cfg(test)]

use ethabi::{Function, Token, Param, ParamType};
use crate::ethereum_transaction::*;

pub const ROOT_HASH: [u8; 32] = [3; 32];
pub const T2_PUBLIC_KEY: [u8; 32] = [4; 32];

pub fn generate_publish_root_data(root_hash: [u8; 32]) -> PublishRootData {
    PublishRootData {
        root_hash: root_hash,
    }
}

fn generate_deregister_validator_data(t2_public_key: [u8; 32]) -> DeregisterValidatorData {
    DeregisterValidatorData {
        t2_public_key: t2_public_key,
    }
}

fn generate_publish_root_eth_txn_desc(root_hash: [u8; 32]) -> EthTransactionDescription {
    EthTransactionDescription {
        function_call: Function {
            name: String::from("publishRoot"),
            inputs: vec![Param {
                name: String::from("_rootHash"),
                kind: ParamType::FixedBytes(32),
            }],
            outputs: Vec::<Param>::new(),
            constant: false,
        },
        call_values: vec![Token::FixedBytes(root_hash.to_vec())],
    }
}

fn generate_deregister_validator_eth_txn_desc(t2_public_key: [u8; 32]) -> EthTransactionDescription {
    EthTransactionDescription {
        function_call: Function {
            name: String::from("deregisterValidator"),
            inputs: vec![Param {
                name: String::from("_targetT2PublicKey"),
                kind: ParamType::FixedBytes(32),
            }],
            outputs: Vec::<Param>::new(),
            constant: false,
        },
        call_values: vec![Token::FixedBytes(t2_public_key.to_vec())],
    }
}

// EthTransactionType tests
mod eth_transaction_type {
    use super::*;

    fn generate_publish_root_eth_txn_type(root_hash: [u8; 32]) -> EthTransactionType {
        EthTransactionType::PublishRoot(
            generate_publish_root_data(root_hash)
        )
    }

    fn generate_deregister_validator_eth_txn_type(t2_public_key: [u8; 32]) -> EthTransactionType {
        EthTransactionType::DeregisterValidator(
            generate_deregister_validator_data(t2_public_key)
        )
    }

    fn generate_unsupported_eth_txn_type() -> EthTransactionType {
        EthTransactionType::Invalid
    }

    mod to_abi {
        use super::*;

        mod succeeds_when {
            use super::*;

            #[test]
            fn txn_is_publish_root() {
                let publish_root_eth_txn_type = generate_publish_root_eth_txn_type(ROOT_HASH);
                let publish_root_eth_txn_desc = generate_publish_root_eth_txn_desc(ROOT_HASH);

                let result = publish_root_eth_txn_type.to_abi();

                assert!(result.is_ok(), "Unsupported ethereum transaction type!");
                assert_eq!(result.unwrap(), publish_root_eth_txn_desc);
            }

            #[test]
            fn txn_is_deregister_validator() {
                let deregister_validator_eth_txn_type = generate_deregister_validator_eth_txn_type(T2_PUBLIC_KEY);
                let deregister_validator_eth_txn_desc = generate_deregister_validator_eth_txn_desc(T2_PUBLIC_KEY);

                let result = deregister_validator_eth_txn_type.to_abi();

                assert!(result.is_ok(), "Unsupported ethereum transaction type!");
                assert_eq!(result.unwrap(), deregister_validator_eth_txn_desc);
            }
        }

        #[test]
        fn fails_when_txn_is_invalid() {
            let unsupported_eth_txn_type = generate_unsupported_eth_txn_type();

            assert!(unsupported_eth_txn_type.to_abi().is_err(), "Ethererum transaction type is valid");
        }
    }
}

// PublishRootData tests
mod publish_root_data {
    use super::*;

    #[test]
    fn new_succeeds() {
        let expected_publish_root_data = generate_publish_root_data(ROOT_HASH);

        assert_eq!(PublishRootData::new(ROOT_HASH), expected_publish_root_data);
    }

    #[test]
    fn to_abi_succeeds() {
        let publish_root_data = generate_publish_root_data(ROOT_HASH);
        let expected_eth_transaction_desc = generate_publish_root_eth_txn_desc(ROOT_HASH);

        assert_eq!(publish_root_data.to_abi(), expected_eth_transaction_desc);
    }
}

// DeregisterValidatorData tests
mod deregister_validator_data {
    use super::*;

    #[test]
    fn new_succeeds() {
        let expected_deregister_validator_data = generate_deregister_validator_data(T2_PUBLIC_KEY);

        assert_eq!(DeregisterValidatorData::new(T2_PUBLIC_KEY), expected_deregister_validator_data);
    }

    #[test]
    fn to_abi_succeeds() {
        let deregister_validator_data = generate_deregister_validator_data(T2_PUBLIC_KEY);
        let expected_eth_transaction_desc = generate_deregister_validator_eth_txn_desc(T2_PUBLIC_KEY);

        assert_eq!(deregister_validator_data.to_abi(), expected_eth_transaction_desc);
    }
}
