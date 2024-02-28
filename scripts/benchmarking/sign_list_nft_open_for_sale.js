const helper = require('./helper.js');
const Web3Utils = require('web3-utils');
const { cryptoWaitReady, signatureVerify } = require('@polkadot/util-crypto');

async function main() {
  await cryptoWaitReady();

  const signing_context = 'authorization for list nft open for sale operation';

  // Setup for unit tests and benchmarking
  // const relayer = '0x0000000000000000000000000000000000000000000000000000000000000001';
  // const signer = helper.get_signer("kiss mule sheriff twice make bike twice improve rate quote draw enough");
  // const nft_id = '0x01';

  // Setup for test on dev network
  const relayer = '0x42ff87aa34da2ce561a0d63b7721916b21a6a025f53901c619d834c57d14a260'; // tier2PublicKeyHex
  const signer = helper.get_signer("category lens cage quantum true lunch group harbor viable verify among film"); // tier2SecretPhrase
  const nft_id = '55804896540906326079859184159455510192230894356597067587225434974184849922';

  const market = '0x01'; // NftSaleType::Ethereum
  const nonce = '0';

  //-----------------------------------------------------------------------------------------------------------------
  let list_nft_open_for_sale_data = {
    context: signing_context,
    relayer: relayer,
    nft_id: nft_id,
    market: market,
    nonce: nonce
  };
  console.log("list_nft_open_for_sale_data: ", list_nft_open_for_sale_data);
  console.log();

  console.log("signer: ", signer.address);
  console.log();

  let encoded_data = helper.encode_signed_list_nft_open_for_sale_data(list_nft_open_for_sale_data);
  console.log("encoded_data:", encoded_data);
  console.log();

  let [list_nft_open_for_sale_data_signature, ] = helper.sign_data(signer, encoded_data);
  console.log('Signature: ', list_nft_open_for_sale_data_signature);
}

if (require.main === module) main();
