const helper = require('./helper.js');
const { u8aToHex, hexStripPrefix } = require('@polkadot/util');
const { cryptoWaitReady, signatureVerify } = require('@polkadot/util-crypto');

async function main() {
  await cryptoWaitReady();

  const signing_context = 'authorization for lower operation'

  const signer = helper.get_signer("news slush supreme milk chapter athlete soap sausage put clutch what kitten");
  const relayer = u8aToHex(signer.publicKey); //'0x7851f9b2488bbcb6c5dcd11486c2af03b6a2dd47f5a6313a00c2683fea73822e';
  const token = '0x1414141414141414141414141414141414141414';
  const amount = 1000;
  const t1_recipient = '0xafdf36201bf70F1232111b5c6a9a424558755134';
  const nonce = 0;

  //-----------------------------------------------------------------------------------------------------------------

  let lower_data = {
    context: signing_context,
    relayer: relayer,
    from: u8aToHex(signer.publicKey),
    token: token,
    amount: amount,
    t1_recipient: t1_recipient,
    nonce: nonce
  };
  console.log("lower_data: ", lower_data);
  console.log();

  console.log("signer: ", signer.address);
  console.log();

  let encoded_data = helper.encode_signed_lower_signature_data(lower_data);
  console.log("encoded_data:", encoded_data);
  console.log();

  let [lower_signature, ] = helper.sign_data(signer, encoded_data);
  console.log('Signature: ', lower_signature);
}

if (require.main === module) main();
