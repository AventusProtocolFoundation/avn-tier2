const helper = require('./helper.js');
const Web3Utils = require('web3-utils');
const { cryptoWaitReady, signatureVerify } = require('@polkadot/util-crypto');

async function main() {
  await cryptoWaitReady();

  const signing_context = 'authorization for transfer fiat nft operation';

  // Setup for test on dev network
  const relayer = '0x42ff87aa34da2ce561a0d63b7721916b21a6a025f53901c619d834c57d14a260'; // tier2PublicKeyHex
  const signer = helper.get_signer("category lens cage quantum true lunch group harbor viable verify among film"); // tier2SecretPhrase
  const nft_id = '65190117265976402131816099285775849115350340885182090231319076206100919253070';

  const t2_transfer_to_public_key = '0x50283d901d56054c52bb68b5c04316056c8be5e0a1dd506977ce9bde58d97e50';
  const nonce = '1';

  //-----------------------------------------------------------------------------------------------------------------
  let transfer_nft_data = {
    context: signing_context,
    relayer: relayer,
    nft_id: nft_id,
    t2_transfer_to_public_key: t2_transfer_to_public_key,
    nonce: nonce
  };
  console.log("transfer_nft_data: ", transfer_nft_data);
  console.log();

  console.log("signer: ", signer.address);
  console.log();

  let encoded_data = helper.encode_signed_transfer_fiat_nft_data(transfer_nft_data);
  console.log("encoded_data:", encoded_data);
  console.log();

  let [transfer_nft_data_signature, ] = helper.sign_data(signer, encoded_data);
  console.log('Signature: ', transfer_nft_data_signature);
}

if (require.main === module) main();
