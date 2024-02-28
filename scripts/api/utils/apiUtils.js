async function getAccountInfo(api, accountAddress) {
  const [nonce, balances] = await api.query.system.account(accountAddress);
  return {nonce, freeBalance: balances.free};
}

module.exports = { getAccountInfo };