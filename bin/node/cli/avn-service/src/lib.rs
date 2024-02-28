///Web-server helper for lydia backend.
use std::marker::PhantomData;
use codec::{Encode, Decode};
use hex::FromHex;
use sp_core::{ecdsa::Signature, hashing::keccak_256};
use sp_avn_common::{EthTransaction, DEFAULT_EXTERNAL_SERVICE_PORT_NUMBER};
use sp_runtime::{traits::{Block as BlockT}};
use sc_keystore::LocalKeystore;

use sc_client_api::{UsageProvider, client::BlockBackend};
use std::time::Instant;

pub use std::sync::{Arc, Mutex};

use secp256k1::SecretKey;
use tide::{http::StatusCode, Error as TideError};
use web3::{Web3, types::{Bytes, TransactionReceipt}, transports};
use ethereum_types::H256;
use node_rpc::extrinsic_utils::get_latest_finalised_block;

pub mod web3_utils;
pub mod keystore_utils;
pub mod summary_utils;

use crate::{web3_utils::*};
use crate::{keystore_utils::*};
use crate::{summary_utils::*};


#[derive(Clone)]
pub struct Config<Block: BlockT, ClientT: BlockBackend<Block> + UsageProvider<Block>> {
    pub keystore: Arc<LocalKeystore>,
    pub avn_port: Option<String>,
    pub eth_node_url: String,
    pub web3_mutex: Arc<Mutex<Option<Web3<transports::Http>>>>,
    pub client: Arc<ClientT>,
    pub _block: PhantomData<Block>,
}

#[derive(Debug, serde::Serialize)]
struct Response {
    pub result: serde_json::Value,
    pub num_confirmations: u64
}

pub fn server_error(message: String) -> TideError {
    return TideError::from_str(StatusCode::InternalServerError, format!("ℹ️  {:?}", message));
}

pub fn hash_with_ethereum_prefix(data_to_sign: Vec<u8>) -> [u8; 32] {
    // T1 Solidity code expects "packed" encoding of the signed message & prefix so we concatenate
    let mut prefixed_message = b"\x19Ethereum Signed Message:\n32".to_vec();
    let hashed_message = keccak_256(&data_to_sign);
    prefixed_message.append(&mut hashed_message.to_vec());
    keccak_256(&prefixed_message)
}

pub fn to_bytes32(data: String) -> Result<[u8; 32], TideError> {
    let mut data = data.to_lowercase();
    if data.starts_with("0x") {
        data = data[2..].into();
    }

    return <[u8; 32]>::from_hex(data.clone())
        .map_or_else(
            |_| Err(server_error(format!("Error converting to bytes32: {:?}", data))),
            |bytes32| Ok(bytes32)
        );
}

fn get_tx_receipt_json(receipt: TransactionReceipt, current_block_number: u64) -> Result<String, TideError> {
    let response = Response {
        result: serde_json::to_value(&receipt)
            .map_err(|_| TideError::from_str(StatusCode::Ok, "❗Eth response is not a valid JSON".to_string()))?,
        num_confirmations:  current_block_number - receipt.block_number.unwrap_or(Default::default()).as_u64()
    };

    let json_response = serde_json::to_string(&response)
        .map_err(|_| server_error("Error serialising response".to_string()))?;

    return Ok(json_response);
}

async fn send_tx(web3: &Web3<transports::Http>,
    keystore: &LocalKeystore,
    send_request: &EthTransaction,
    sender_eth_address: &Vec<u8>,
    priv_key: [u8; 32]) -> Result<web3::types::H256, TideError>
{
    let tx = build_raw_transaction(web3, &keystore, send_request, &sender_eth_address).await.map_err(|e| {
        log::error!("💔 Error building raw transaction: {:?}", e);
        server_error(format!("Error building raw transaction: {:?}", e))})?;

    let signed_tx = tx.sign(&H256::from_slice(&priv_key), &get_chain_id(web3).await?);

    Ok(send_raw_transaction(web3, Bytes::from(signed_tx)).await?)
}

#[tokio::main]
async fn send_main<Block: BlockT, ClientT>(mut req: tide::Request<Arc<Config<Block, ClientT>>>) -> Result<String, TideError>
    where ClientT: BlockBackend<Block> + UsageProvider<Block> + Send + Sync + 'static
{
    log::info!("ℹ️ avn-service send Request");
    let post_body = req.body_bytes().await?;
    let send_request = &EthTransaction::decode(&mut &post_body[..])?;
    // TODO wrap this in a getter function that does the check and returns the mutex
    if let Ok(mutex_web3) = &req.state().web3_mutex.lock() {
        if mutex_web3.is_none() {
            return Err(server_error("Web3 connection not setup".to_string()));
        }
        let web3 = mutex_web3.as_ref().expect("Already checked");
        let keystore = &req.state().keystore;

        let my_eth_address = get_eth_address_bytes_from_keystore(&keystore)?;
        let my_priv_key = get_priv_key(&keystore, &my_eth_address)?;

        let mut tx_hash = send_tx(&web3, keystore, send_request, &my_eth_address, my_priv_key).await;

        if let Err(e) = tx_hash {
            if e.to_string().find("the tx doesn't have the correct nonce").is_some() {
                let ethereum_nonce: u64 = get_nonce_from_ethereum(web3, &my_eth_address).await?.low_u64();
                set_nonce(&keystore, ethereum_nonce)?;

                tx_hash = send_tx(&web3, keystore, send_request, &my_eth_address, my_priv_key).await;
            } else {
                return Err(server_error(format!("Error sending transaction to ethereum: {:?}", e)));
            }
        }

        let tx_hash = tx_hash.map_err(|e| server_error(format!("Error sending transaction to ethereum: {:?}", e)))?;

        increment_nonce(&keystore)?;

        Ok(hex::encode(tx_hash))
    }
    else {
        Err(TideError::from_str(StatusCode::FailedDependency, "Failed to get web3"))
    }
}

#[tokio::main]
async fn root_hash_main<Block: BlockT, ClientT>(req: tide::Request<Arc<Config<Block, ClientT>>>) -> Result<String, TideError>
    where ClientT: BlockBackend<Block> + UsageProvider<Block> + Send + Sync + 'static
{
    log::info!("ℹ️ avn-service eth events");
    let tx_hash: H256 = H256::from_slice(
        &to_bytes32(
            req.param("txHash").map_err(|_| TideError::from_str(
                StatusCode::BadRequest,
                "💔 txHash is not a valid transaction hash".to_string()))?.to_string()
        )?
    );

    // TODO wrap this in a getter function that does the check and returns the mutex
    if let Ok(mutex_web3) = &req.state().web3_mutex.lock() {
        if mutex_web3.is_none() {
            return Err(server_error("Web3 connection not setup".to_string()));
        }
        let web3 = mutex_web3.as_ref().expect("Already checked");

        let current_block_number = web3_utils::get_current_block_number(web3).await?;
        let maybe_receipt = web3_utils::get_tx_receipt(web3, tx_hash).await?;
        match maybe_receipt {
            None => Err(TideError::from_str(StatusCode::Ok, "❗Transaction receipt is empty".to_string())),
            Some(receipt) => Ok(get_tx_receipt_json(receipt, current_block_number)?)
        }
    }
    else {
        Err(TideError::from_str(StatusCode::FailedDependency, "Failed to get web3"))
    }
}

pub async fn start<Block: BlockT, ClientT>(mut config: Config<Block, ClientT>) where
    ClientT: BlockBackend<Block> + UsageProvider<Block> + Send + Sync + 'static
 {
    let web3 = setup_web3_connection(&config.eth_node_url);

    if web3.is_none() {
        log::error!("💔 Error creating a web3 connection. URL is not valid {:?}", &config.eth_node_url);
        return;
    }


    if let Err(e) = setup_nonce_storage(&web3.as_ref(), &config.keystore).await {
        log::error!("💔 Error setting up nonce storage {:?}", e);
        return;
    }

    config.web3_mutex = Arc::new(Mutex::new(web3));

    let port = format!("127.0.0.1:{}", &config.avn_port.clone().unwrap_or_else(|| DEFAULT_EXTERNAL_SERVICE_PORT_NUMBER.to_string()));

    let mut app = tide::with_state(Arc::<Config<Block, ClientT>>::from(config));

    app.at("/eth/sign/:data_to_sign").get(|req: tide::Request<Arc<Config<Block, ClientT>>>| async move {
        log::info!("ℹ️ avn-service sign Request");
        let keystore = &req.state().keystore;
        let data_to_sign: Vec<u8> = hex::decode(req.param("data_to_sign")?.trim_start_matches("0x"))
            .map_err(|e| server_error(format!("Error converting data_to_sign into hex string {:?}", e)))?;

        let hashed_message = hash_with_ethereum_prefix(data_to_sign);

        let my_eth_address = get_eth_address_bytes_from_keystore(keystore)?;
        let my_priv_key = get_priv_key(keystore, &my_eth_address)?;

        let secret = SecretKey::parse(&my_priv_key)?;
        let message = secp256k1::Message::parse(&hashed_message);
        let signature: Signature = secp256k1::sign(&message, &secret).into();

        Ok(hex::encode(signature.encode()))
    });

    app.at("/eth/send").post(|req: tide::Request<Arc<Config<Block, ClientT>>>| async move {
        // Methods that require web3 must be run within the tokio runtime (#[tokio::main])
        return send_main(req);
    });

    app.at("/eth/events/:txHash").get(|req: tide::Request<Arc<Config<Block, ClientT>>>| async move {
        // Methods that require web3 must be run within the tokio runtime (#[tokio::main])
        return root_hash_main(req);
    });

    app.at("/roothash/:from_block/:to_block").get(|req: tide::Request<Arc<Config<Block, ClientT>>>| async move {
        log::info!("ℹ️ avn-service roothash");
        // We cannot use a number bigger than a u32, but with block times of 3 sec it would take about
        // 408 years before we reach it so i think we can live with it for now.
        let from_block_number:u32 = req.param("from_block")?.parse()?;
        let to_block_number:u32 = req.param("to_block")?.parse()?;

        let extrinsics_start_time = Instant::now();

        let extrinsics = get_extrinsics::<Block, ClientT>(&req, from_block_number, to_block_number)?;
        let extrinsics_duration = extrinsics_start_time.elapsed();
        log::info!("⏲️  get_extrinsics on block range [{:?}, {:?}] time: {:?}", from_block_number, to_block_number, extrinsics_duration);

        if extrinsics.len() > 0 {
            let root_hash_start_time = Instant::now();
            let root_hash = generate_tree_root(extrinsics)?;
            let root_hash_duration = root_hash_start_time.elapsed();
            log::info!("⏲️  generate_tree_root on block range [{:?}, {:?}] time: {:?}", from_block_number, to_block_number, root_hash_duration);

            return Ok(hex::encode(root_hash));
        }

        // the tree is empty
        Ok(hex::encode([0; 32]))
    });

    app.at("/latest_finalised_block").get(|req: tide::Request<Arc<Config<Block, ClientT>>>| async move {
        log::info!("ℹ️ avn-service latest finalised block");
        let finalised_block_number = get_latest_finalised_block(&req.state().client);
        Ok(hex::encode(finalised_block_number.encode()))
    });

    app.listen(port)
        .await
        .map_err(|e| log::error!("avn-service error: {}", e))
        .unwrap_or(());
}
