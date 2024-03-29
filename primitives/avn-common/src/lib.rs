#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{string::String};

use codec::{Encode, Decode, Codec};
use sp_std::{vec::Vec, boxed::Box};
use sp_core::{H160, crypto::KeyTypeId, ecdsa};
use sp_runtime::traits::{AtLeast32Bit, Member, Dispatchable};
use sp_io::{EcdsaVerifyError, hashing::keccak_256, crypto::secp256k1_ecdsa_recover_compressed};

pub mod event_types;
pub mod offchain_worker_storage_lock;
#[path = "tests/helpers.rs"]
pub mod avn_tests_helpers;

/// Ingress counter type for a counter that can sign the same message with a different signature each time
pub type IngressCounter = u64;

/// Key type for AVN pallet. dentified as `avnk`.
pub const AVN_KEY_ID: KeyTypeId = KeyTypeId(*b"avnk");
/// Key type for signing ethereum compatible signatures, built-in. Identified as `ethk`.
pub const ETHEREUM_SIGNING_KEY: KeyTypeId = KeyTypeId(*b"ethk");
/// Ethereum prefix
pub const ETHEREUM_PREFIX: &'static [u8] = b"\x19Ethereum Signed Message:\n32";

/// Local storage key to access the external service's port number
pub const EXTERNAL_SERVICE_PORT_NUMBER_KEY: &'static [u8; 15] = b"avn_port_number";
/// Default port number the external service runs on.
pub const DEFAULT_EXTERNAL_SERVICE_PORT_NUMBER: &str = "2020";

#[derive(Debug)]
pub enum ECDSAVerificationError {
    InvalidSignature,
    InvalidValueForV,
    InvalidValueForRS,
    InvalidMessageFormat,
    BadSignature
}


// Struct that holds the information about an Ethereum transaction
// See https://github.com/ethereum/wiki/wiki/JSON-RPC#parameters-22
#[derive(Encode, Decode, Clone, PartialEq, Debug, Eq, Default)]
pub struct EthTransaction{
    pub from: [u8;32],
    pub to: H160,
    pub data: Vec<u8>,
}

impl EthTransaction {
    pub fn new(
        from: [u8;32],
        to: H160,
        data: Vec<u8>,
    ) -> Self {
            return EthTransaction {
            from: from,
            to: to,
            data: data,
        };
    }
}

#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, Default, Debug)]
pub struct Proof<Signature, AccountId> {
    pub signer: AccountId,
    pub relayer: AccountId,
    pub signature: Signature,
}

pub trait CallDecoder {
    // The type that represents an account id defined in the trait (T::AccountId)
    type AccountId;

    // The type that represents a signature
    type Signature;

    // The type used to throw an error (Error<T>)
    type Error;

    /// The type which encodes the call to be decoded.
    type Call;

    fn get_proof(call: &Self::Call) -> Result<Proof<Self::Signature, Self::AccountId>, Self::Error>;
}

// ======================================== Proxy validation ==========================================

pub trait InnerCallValidator {
    type Call: Dispatchable;

    fn signature_is_valid(_call: &Box<Self::Call>) -> bool { false }
}

pub fn safe_add_block_numbers<BlockNumber: Member + Codec + AtLeast32Bit>(left: BlockNumber, right: BlockNumber)
    -> Result<BlockNumber, ()>
{
    Ok(
        left.checked_add(&right).ok_or(())?.into()
    )
}

pub fn safe_sub_block_numbers<BlockNumber: Member + Codec + AtLeast32Bit>(left: BlockNumber, right: BlockNumber)
    -> Result<BlockNumber, ()>
{
    Ok(
        left.checked_sub(&right).ok_or(())?.into()
    )
}

pub fn calculate_two_third_quorum(total_num_of_validators: u32) -> u32 {
    if total_num_of_validators < 3 {
        return total_num_of_validators;
    } else {
        return (2 * total_num_of_validators / 3) + 1;
    }
}

pub fn recover_public_key_from_ecdsa_signature(
    signature: ecdsa::Signature,
    message: String) -> Result<ecdsa::Public, ECDSAVerificationError>
{
    match secp256k1_ecdsa_recover_compressed(&signature.into(), &hash_with_ethereum_prefix(message)?)
    {
        Ok(pubkey) => {
            return Ok(ecdsa::Public::from_raw(pubkey));
        },
        Err(EcdsaVerifyError::BadRS) => {
            return Err(ECDSAVerificationError::InvalidValueForRS);
        },
        Err(EcdsaVerifyError::BadV) => {
            return Err(ECDSAVerificationError::InvalidValueForV);
        },
        Err(EcdsaVerifyError::BadSignature) => {
            return Err(ECDSAVerificationError::BadSignature);
        }
    }
}

pub fn hash_with_ethereum_prefix(hex_message: String) -> Result<[u8; 32], ECDSAVerificationError> {
    let message_bytes = hex::decode(hex_message.trim_start_matches("0x"))
        .map_err(|_| ECDSAVerificationError::InvalidMessageFormat)?;

    let mut prefixed_message = ETHEREUM_PREFIX.to_vec();
    let hashed_message = keccak_256(&message_bytes);
    prefixed_message.append(&mut hashed_message.to_vec());
    Ok(keccak_256(&prefixed_message))
}