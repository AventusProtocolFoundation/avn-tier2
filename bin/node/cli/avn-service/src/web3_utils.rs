use tide::Error as TideError;
use web3::{Web3, transports::Http, types::{CallRequest, TransactionReceipt, Bytes, H160, U256}};
use sp_avn_common::EthTransaction;
use ethereum_tx_sign::{RawTransaction};
use ethereum_types;
use sc_keystore::LocalKeystore;
pub use std::sync::{Arc, MutexGuard};
use crate::{server_error, keystore_utils::get_nonce};

pub fn setup_web3_connection(url: &String) -> Option<Web3<Http>> {
    let transport_init_result = web3::transports::Http::new(url);

    if transport_init_result.is_err() {
        return None;
    }
    let transport = transport_init_result.expect("Already checked");
    return Some(web3::Web3::new(transport));
}

pub async fn get_nonce_from_ethereum(web3: &Web3<Http>, sender_eth_address: &Vec<u8>) -> Result<U256, TideError> {
    if sender_eth_address.len() != 20 {
        return Err(server_error(format!("sender address ({:?}) is not a valid Ethereum address", sender_eth_address)));
    }

    return Ok(
        web3.eth()
            .transaction_count(H160::from_slice(sender_eth_address), None)
            .await
            .map_err(|_| server_error("Error getting nonce from Ethereum".to_string()))?
    );
}

/// Note: this is called by the signer which has different ethereum types to web3
pub async fn build_raw_transaction(
    web3: &Web3<Http>,
    keystore: &LocalKeystore,
    send_request: &EthTransaction,
    sender_eth_address: &Vec<u8>) -> Result<RawTransaction, TideError>
{
    let recipient = send_request.to.as_bytes();
    let gas_price = web3.eth()
        .gas_price()
        .await
        .map_err(|_| server_error("Error getting gas price".to_string()))?;

    let nonce = get_nonce(keystore)?;
    let maybe_gas_estimate = estimate_gas(web3, gas_price, sender_eth_address, recipient, &send_request.data).await;

    if let Err(ref gas_estimate_error) = maybe_gas_estimate {
        log::error!("💔 Error estimating gas (this may be due to the transaction failing on Ethereum) {:?}", gas_estimate_error);
        return Err(server_error("Error estimating gas".to_string()));
    }

    let gas_estimate = maybe_gas_estimate.expect("Checked for errors");

    Ok(
        RawTransaction {
            nonce: to_eth_u256(nonce.into()),
            to: Some(ethereum_types::H160::from_slice(recipient)),
            value: ethereum_types::U256::zero(),
            gas_price: to_eth_u256(gas_price),
            gas: to_eth_u256(gas_estimate),
            data: send_request.data.clone()
        }
    )
}

pub async fn get_chain_id(web3: &Web3<Http>) -> Result<u64, TideError> {
    Ok(
        web3
            .eth()
            .chain_id()
            .await
            .map_err(|_| server_error("Error getting chain Id".to_string()))?
            .as_u64()
    )
}

async fn estimate_gas(web3: &Web3<Http>, gas_price: U256, sender: &Vec<u8>, recipient: &[u8], data: &Vec<u8>) -> Result<U256, TideError>
{
    let call_request = CallRequest {
        from: Some(H160::from_slice(&sender)),
        to: Some(H160::from_slice(recipient)),
        gas: None,
        gas_price: Some(gas_price),
        value: Some(U256::zero()),
        data: Some(Bytes(data.to_vec())),
    };

    Ok(
        web3.eth()
            .estimate_gas(call_request.clone(), None)
            .await
            .map_err(|e| server_error(format!("Error estimating gas for data: {:?}\nerror: {:?}", call_request, e)))?
    )
}

pub async fn get_current_block_number(web3: &Web3<Http>) -> Result<u64, TideError>
{
    Ok(
        web3.eth()
            .block_number()
            .await
            .map_err(|_| server_error("Error getting block number".to_string()))?
            .as_u64()
    )
}

pub async fn get_tx_receipt(web3: &Web3<Http>, tx_hash: ethereum_types::H256) -> Result<Option<TransactionReceipt>, TideError>
{
    Ok(
        web3.eth()
            .transaction_receipt(web3::types::H256(tx_hash.0))
            .await
            .map_err(|_| server_error("Error getting tx receipt".to_string()))?
    )
}

pub async fn send_raw_transaction(web3: &Web3<Http>, tx: Bytes) -> Result<web3::types::H256, TideError> {
    Ok(
        web3.eth()
            .send_raw_transaction(tx)
            .await
            .map_err(|_| server_error("Error sending raw transaction".to_string()))?
    )
}

///This function is needed because the library to sign raw transactions uses different ethereum types to Web3
fn to_eth_u256(value: U256) -> ethereum_types::U256 {
    return ethereum_types::U256(value.0);
}