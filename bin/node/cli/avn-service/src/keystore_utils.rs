use sp_avn_common::ETHEREUM_SIGNING_KEY;
use sc_keystore::LocalKeystore;
use tide::Error as TideError;
use web3::{Web3, transports::Http};
use crate::{server_error, web3_utils::get_nonce_from_ethereum};
use std::{str::FromStr, path::PathBuf, fs::{File, OpenOptions}, io::{Read, Write, Seek, SeekFrom}};

const NONCE_FILE_NAME: &str = "eth_wallet_nonce";

///For this function to work, the name of the keystore file must be a valid Ethereum address
pub fn get_eth_address_bytes_from_keystore(keystore: &LocalKeystore) -> Result<Vec<u8>, TideError> {
    let addresses = keystore.raw_public_keys(ETHEREUM_SIGNING_KEY)
        .map_err(|_| server_error(format!("Error getting public key from keystore for {:?}", ETHEREUM_SIGNING_KEY)))?;

    if addresses.len() == 0 {
        Err(server_error(format!("No keys found in the keystore for {:?}", ETHEREUM_SIGNING_KEY)))?
    }

    return Ok(addresses[0].clone());
}

pub fn get_priv_key(keystore: &LocalKeystore, eth_address: &Vec<u8>) -> Result<[u8; 32], TideError> {
    let priv_key = keystore.key_phrase_by_type(eth_address, ETHEREUM_SIGNING_KEY)
        .map_err(|_| server_error(format!("Error getting private key from keystore for {:?}", ETHEREUM_SIGNING_KEY)))?;
    let priv_key_bytes = hex::decode(priv_key)
        .map_err(|_| server_error("Error decoding private key to bytes".to_string()))?;

    let mut key: [u8; 32] = Default::default();
    key.copy_from_slice(&priv_key_bytes[0..32]);
    return Ok(key);
}

pub async fn setup_nonce_storage(web3: &Option<&Web3<Http>>, keystore: &LocalKeystore) -> Result<(), TideError> {
    let nonce_file = get_nonce_file(keystore)?;

    let eth_address = get_eth_address_bytes_from_keystore(keystore)?;
    let ethereum_nonce: u64 = get_nonce_from_ethereum(web3.unwrap(), &eth_address).await?.low_u64();

    write_nonce_to_file(nonce_file, ethereum_nonce)?;

    return Ok(());
}

pub fn get_nonce(keystore: &LocalKeystore) -> Result<u64, TideError> {
    let nonce_file = get_nonce_file(keystore)?;
    return read_nonce_from_file(&nonce_file);
}

pub fn set_nonce(keystore: &LocalKeystore, nonce: u64) -> Result<(), TideError> {
    let nonce_file = get_nonce_file(keystore)?;
    write_nonce_to_file(nonce_file, nonce)?;

    Ok(())
}

pub fn increment_nonce(keystore: &LocalKeystore) -> Result<(), TideError> {
    let nonce_file = get_nonce_file(keystore)?;
    let current_nonce = read_nonce_from_file(&nonce_file)?;

    write_nonce_to_file(nonce_file, current_nonce + 1)?;

    Ok(())
}

fn get_nonce_file(keystore: &LocalKeystore) -> Result<File, TideError> {
    let path = nonce_file_path(keystore).expect("Already checked");

    let nonce_file = OpenOptions::new().read(true).write(true).create(true).open(path);
    if let Err(e) = nonce_file {
        return Err(server_error(format!("Error opening nonce database: {:?}", e)));
    }

    return Ok(nonce_file.expect("Already checked"));
}

fn read_nonce_from_file(mut nonce_file: &File) -> Result<u64, TideError> {
    let mut nonce = String::new();
    nonce_file.read_to_string(&mut nonce)
        .map_err(|e| server_error(format!("Error reading nonce from db: {:?}", e)))?;

    nonce = nonce.replace("\n", "");
    if nonce.is_empty() {
        nonce = "0".to_string();
    }

    return Ok(u64::from_str(&nonce).map_err(|_| server_error(format!("Invalid nonce value in database: {:?}", nonce)))?);
}

fn write_nonce_to_file(mut nonce_file: File, nonce: u64) -> Result<(), TideError> {
    nonce_file.seek(SeekFrom::Start(0)).unwrap();
    serde_json::to_writer(&nonce_file, &nonce)
        .map_err(|e| server_error(format!("Error writing to nonce DB: {:?}", e)))?;
    nonce_file.flush().map_err(|e| server_error(format!("Error writing to nonce DB: {:?}", e)))?;

    Ok(())
}

fn nonce_file_path(keystore: &LocalKeystore) -> Result<PathBuf, TideError> {
    if keystore.path().is_none() {
        return Err(server_error("Keystore not setup correctly".to_string()));
    }

    let mut buf: PathBuf = keystore.path().clone().expect("Already checked");
    buf.push(NONCE_FILE_NAME.to_string());
    Ok(buf)
}
