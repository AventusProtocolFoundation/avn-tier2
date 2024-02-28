const BN = require('bn.js');
const validator = require('./utils/validationUtils.js');

function verifyTransactionDetails(fromSeed, toAddress, amount) {
  if(!validator.isValidSeed(fromSeed, "Invalid Sender Seed")
  || !validator.isValidAddress(toAddress, "Invalid Recipient Address")
  || !validator.isValidValue(amount, "Invalid Amount")){
    process.exitCode(1);
  }
}

async function signAndTransfer(fromSeed, toAddress, amount, api, nonce, keyring) {
  verifyTransactionDetails(fromSeed, toAddress, amount);
  let fromAccount = keyring.addFromUri(fromSeed);
  await api.tx.balances.transfer(toAddress, amount.toString()).signAndSend(fromAccount, {nonce});
}

module.exports = { signAndTransfer };
