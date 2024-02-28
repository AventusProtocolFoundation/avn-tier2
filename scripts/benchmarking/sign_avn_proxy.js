const helper = require('./helper.js');
const { hexToU8a, u8aToHex, u8aConcat, hexStripPrefix } = require('@polkadot/util');
const { cryptoWaitReady } = require('@polkadot/util-crypto');
const { TypeRegistry } = require('@polkadot/types');

const registry = new TypeRegistry();

async function main() {
  await cryptoWaitReady();

  const signing_context = 'authorization for proxy payment'

  // Constants representing the data to sign. These values must match the values used in avnProxy benchmarking
  const relayer = u8aToHex(helper.get_address_from_bytes("whitelisted_caller", 0, 0));
  const signer = helper.get_signer("kiss mule sheriff twice make bike twice improve rate quote draw enough");

  console.log("signer:", u8aToHex(signer.publicKey))
  console.log("relayer:", relayer)

  const proof = {
    signer: u8aToHex(signer.publicKey),
    relayer: relayer,
    signature: {
        Sr25519: "0xa6350211fcdf1d7f0c79bf0a9c296de17449ca88a899f0cd19a70b07513fc107b7d34249dba71d4761ceeec2ed6bc1305defeb96418e6869e6b6199ed0de558e"
    }
  };

  const recipient = relayer;
  const amount = 10;
  const payment_nonce = 0;

  //-----------------------------------------------------------------------------------------------------------------

  let encoded_data = encode_payment_authorisation_data({
    context: signing_context,
    recipient,
    amount,
    payment_nonce,
    proof
  });

  let [payment_signature, ] = helper.sign_data(signer, encoded_data);
  console.log(`Payment signature: ${hexStripPrefix(payment_signature)}\n`);
}

function encode_payment_authorisation_data(params) {
    console.log(JSON.stringify(params, null, 2));

    const context = registry.createType('Text', params.context);
    const recipient = registry.createType('AccountId', hexToU8a(params.recipient));
    const amount = registry.createType('Balance', params.amount);
    const payment_nonce = registry.createType('u64', params.payment_nonce);
    const proof = encodeProxyTokenTransferProof(params.proof);

    const encoded_params = u8aConcat(
      context.toU8a(false),
      proof,
      recipient.toU8a(true),
      amount.toU8a(true),
      payment_nonce.toU8a(true)
    );

    let result = u8aToHex(encoded_params);

    return result;
}

function encodeProxyTokenTransferProof(params) {
    const signer = registry.createType('AccountId', params.signer)
    const relayer = registry.createType('AccountId', params.relayer)
    const signature = registry.createType('MultiSignature', params.signature)
    return u8aConcat(signer.toU8a(true), relayer.toU8a(true), signature.toU8a(false))
}

if (require.main === module) main();
