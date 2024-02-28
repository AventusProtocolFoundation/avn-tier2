const helper = require('./helper.js');
const { hexToU8a, u8aToHex, u8aConcat, hexStripPrefix } = require('@polkadot/util');
const { cryptoWaitReady } = require('@polkadot/util-crypto');
const { TypeRegistry } = require('@polkadot/types');

const registry = new TypeRegistry();

async function main() {
  await cryptoWaitReady();

  const signing_context = "authorization for add ethereum log operation";

  // Constants representing the data to sign. These values must match the values used in ethereum_events benchmarking
  const relayer = u8aToHex(helper.get_address_from_bytes("whitelisted_caller", 0, 0));
  const signer = helper.get_signer("kiss mule sheriff twice make bike twice improve rate quote draw enough");

  const event_type = '0x02'; // ValidEvents::NftMint
  const tx_hash = '0x0101010101010101010101010101010101010101010101010101010101010101';
  const sender_nonce = '0';

  //-----------------------------------------------------------------------------------------------------------------
  let add_ethereum_log_data = {
    context: signing_context,
    relayer: relayer,
    event_type: event_type,
    tx_hash: tx_hash,
    sender_nonce: sender_nonce
  };
  console.log("add_ethereum_log_data: ", add_ethereum_log_data);
  console.log();

  console.log("signer: ", signer.address);
  console.log();

  let encoded_data = helper.encode_signed_add_ethereum_log_data(add_ethereum_log_data);
  console.log("encoded_data:", encoded_data);
  console.log();

  let [add_ethereum_log_data_signature, ] = helper.sign_data(signer, encoded_data);
  console.log('Signature: ', add_ethereum_log_data_signature);
}

if (require.main === module) main();
