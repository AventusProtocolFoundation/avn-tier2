const helper = require('./helper.js');
const { u8aToHex, hexStripPrefix } = require('@polkadot/util');
const { cryptoWaitReady } = require('@polkadot/util-crypto');

async function main() {
  await cryptoWaitReady();

  const signing_context = 'authorization for transfer operation'

  // Constants representing the data to sign. These values must match the values used in TokenManager benchmarking
  const relayer = u8aToHex(helper.get_address_from_bytes("whitelisted_caller", 0, 0));
  const signer = helper.get_signer("news slush supreme milk chapter athlete soap sausage put clutch what kitten");
  const recipient = u8aToHex(helper.get_address_from_bytes("to", 2, 2));
  const token = '0x1414141414141414141414141414141414141414';
  const amount = 1000;
  const nonce = 0;

  //-----------------------------------------------------------------------------------------------------------------

  let encoded_data_for_proxy = helper.encode_signed_transfer_signature_data({
    context: signing_context,
    relayer: relayer,
    from: u8aToHex(signer.publicKey),
    to: recipient,
    token: token,
    amount: amount,
    nonce: nonce
  });

  let [proxy_signature, ] = helper.sign_data(signer, encoded_data_for_proxy);
  console.log(`Proxy signature: ${hexStripPrefix(proxy_signature)}\n`);

  let encoded_data_for_signed_transfer = helper.encode_signed_transfer_signature_data({
    context: signing_context,
    relayer: u8aToHex(signer.publicKey),
    from: u8aToHex(signer.publicKey),
    to: recipient,
    token: token,
    amount: amount,
    nonce: nonce
  });

  let [transfer_signature, ] = helper.sign_data(signer, encoded_data_for_signed_transfer);
  console.log(`Signed transfer signature: ${hexStripPrefix(transfer_signature)}\n`);
}

if (require.main === module) main();
