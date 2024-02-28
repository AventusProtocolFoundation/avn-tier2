const helper = require('./helper.js');
const { u8aToHex, hexStripPrefix } = require('@polkadot/util');
const { cryptoWaitReady } = require('@polkadot/util-crypto');
const BN = require('bn.js');

async function main() {
  await cryptoWaitReady();

  const BOND_SIG_CONTEXT = 'authorization for bond operation';
  const BOND_EXTRA_SIG_CONTEXT = 'authorization for bond extra operation';
  const PAYOUT_SIG_CONTEXT = 'authorization for signed payout stakers operation';
  const UNBOND_SIG_CONTEXT = 'authorization for unbond operation';
  const SET_PAYEE_SIG_CONTEXT = 'authorization for set payee operation';

  const user_bond_amount = new BN("10000000000000000000000"); //10,000AVT
  const nonce = 0;
  const defaultPayee = 'Stash';

  // Constants representing the data to sign. These values must match the values used in TokenManager benchmarking
  const staker = helper.get_signer("kiss mule sheriff twice make bike twice improve rate quote draw enough");
  const relayer = helper.get_signer("category lens cage quantum true lunch group harbor viable verify among film");

  //-----------------------------------------------------------------------------------------------------------------
  let signature = undefined;

  //Bond
  let encoded_bond = helper.encode_bond_signature_data({
    context: BOND_SIG_CONTEXT,
    amount: user_bond_amount.toString(),
    relayer: u8aToHex(relayer.publicKey),
    nonce,
    controller: u8aToHex(staker.publicKey),
    payee: defaultPayee
  });

  [signature, ] = helper.sign_data(staker, encoded_bond);
  console.log(`Bond signature: ${hexStripPrefix(signature)}\n`);

  //Bond extra
  let encoded_bond_extra = helper.encode_bond_extra_signature_data({
    context: BOND_EXTRA_SIG_CONTEXT,
    amount: new BN("500000000"),
    relayer: u8aToHex(relayer.publicKey),
    nonce
  });

  [signature, ] = helper.sign_data(staker, encoded_bond_extra);
  console.log(`Bond extra signature: ${hexStripPrefix(signature)}\n`);

  //Unbond
  let encoded_unstake = helper.encode_unbond_signature_data({
    context: UNBOND_SIG_CONTEXT,
    amount: new BN("500000000"),
    relayer: u8aToHex(relayer.publicKey),
    nonce
  });

  [signature, ] = helper.sign_data(staker, encoded_unstake);
  console.log(`Unbond signature: ${hexStripPrefix(signature)}\n`);

  //Set payee
  let encoded_payee = helper.encode_set_payee_signature_data({
    context: SET_PAYEE_SIG_CONTEXT,
    payee: "Controller",
    relayer: u8aToHex(relayer.publicKey),
    nonce
  });

  [signature, ] = helper.sign_data(staker, encoded_payee);
  console.log(`Payee signature: ${hexStripPrefix(signature)}\n`);

  //Payout stakers
  let encoded_payout_stakers = helper.encode_payout_stakers_signature_data({
    context: PAYOUT_SIG_CONTEXT,
    eraIndex: new BN("0"), //Change me to 1 for the actual benchmark and 0 for tests
    relayer: u8aToHex(relayer.publicKey),
    nonce
  });

  [signature, ] = helper.sign_data(staker, encoded_payout_stakers);
  console.log(`Payout stakers signature: ${hexStripPrefix(signature)}\n`);

}

if (require.main === module) main();
