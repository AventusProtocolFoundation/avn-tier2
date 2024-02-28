const { ApiPromise, WsProvider, Keyring } = require('@polkadot/api');
const BN = require('bn.js');
const keyring = new Keyring({ type: 'sr25519' });
const path = require('path');
const fs = require('fs');
const simpleTransfer = require('./simpleTransfer.js');
const apiUtils = require('./utils/apiUtils.js');
const validator = require('./utils/validationUtils.js');

const DEV = new BN(1000000000000);
const DEFAULT_INITIAL_FUND = 1000000;
// Cost per Transfer:
// Tranfer 1G DEV costs about 4.5 DEV by default in Substrate
// 2.485 DEV fee + 1.988 DEV deposited to treasury
const COST_PER_TRANSFER = 4.5;
const DEFAULT_NODE_HOST = "127.0.0.1";
const DEFAULT_WS_PORT = 9944;

let keepWaiting = true;
let api, issuerSeed, issuerAddress, issuerAccountInfo, accountsFolderPath, initialAmount, nodeHost, wsPort;
let accounts = [];

function showUsage(){
  console.log(`
    This script sends funds to a list of accounts.
    Run as
      node initialiseAccountBalances.js [issuer account seed] [issuer account sr address] [accounts directory] [OPTIONAL: initial amount of DEV sending to each account] [OPTIONAL: node host address] [OPTIONAL: node ws port number]
    Examples of
      issuer account seed:                            0x2a3f54b2d3d483bbd30e1bbd5a2d38d82de8d67729801e6be38819445912cd9f
      issuer account sr address:                      5Fmo4xqnAcBrAXXRVzDvHpaaYVMtxcPbuzcFHxdSbrwN9cYZ
      accounts directory:                             ./keys/accounts/
      initial amount of DEV sending to each account:  1000000 (default is 1000000)
      node host address:                              127.0.0.1 (default is 127.0.0.1)
      node ws port number:                            9944 (default is 9944)
  `);
}

function getArguments(){
  const args = process.argv.slice(2);
  if(args.length < 3 || args.length > 6){
    console.log("Wrong number of parameters");
    showUsage();
    process.exit(1);
  }
  issuerSeed = args[0];
  // TODO [TYPE: refactoring][PRI: low]: Retrieve the issTODOuer account address by using keyring.addFromSeed
  issuerAddress = args[1];
  accountsFolderPath = args[2];
  initialAmount = DEV.mul(new BN(args[3] || DEFAULT_INITIAL_FUND));
  nodeHost = args[4] || DEFAULT_NODE_HOST;
  wsPort = args[5] || DEFAULT_WS_PORT;
}

function verifyArguments(){
  if(!validator.isValidSeed(issuerSeed, "Invalid Issuer Seed")
  || !validator.isValidAddress(issuerAddress, "Invalid Issuer Address")
  || !validator.isValidPath(accountsFolderPath, "Invalid path for accounts folder")
  || !validator.isValidValue(initialAmount, "Invalid Initial Amount")
  || !validator.isValidIP(nodeHost, "Invalid Host IP Address")
  || !validator.isValidPort(wsPort, "Invalid WS Port Number")){
    showUsage();
    process.exit(1);
  }
}

async function initialiseAPI(){
  const wsProvider = new WsProvider(`ws://${nodeHost}:${wsPort}`);
  api = await ApiPromise.create({ provider: wsProvider,
    types: {
      NFT: {
        "id": "H256",
        "owner": "AccountId"
      }
  }});

  // Retrieve the chain & node information via rpc calls
  const [chain, nodeName, nodeVersion] = await Promise.all([
    api.rpc.system.chain(),
    api.rpc.system.name(),
    api.rpc.system.version()
  ]);

  console.log(`You are connected to chain ${chain} using ${nodeName} v${nodeVersion}\n`);
}

async function verifyAmount(){
  issuerAccountInfo = await apiUtils.getAccountInfo(api, issuerAddress);
  const fundsRequired = (initialAmount.add(new BN(COST_PER_TRANSFER))).mul(new BN(accounts.length));
  if(issuerAccountInfo.freeBalance.lt(fundsRequired)){
    console.log("Issuer does not have enough DEV to send.");
    console.log(`Issuer Balance:  ${(new BN(issuerAccountInfo.freeBalance).div(DEV)).toString()} DEV`);
    console.log(`Requirement:     ${fundsRequired.div(DEV).toString()} DEV`);
    process.exit(1);
  }
}

async function setupAccounts(){
  let accountFiles = fs.readdirSync(accountsFolderPath);
  accountFiles.forEach(file => {
    let filePath = path.join(accountsFolderPath, file);
    let account = JSON.parse(fs.readFileSync(filePath));
    accounts.push(account);
  });
}

async function initialiseAccountBalances(){
  let nonce = new BN(issuerAccountInfo.nonce.toString());

  for(let i = 0; i < accounts.length; i++){
    let account = accounts[i];
    const accountInfo = await apiUtils.getAccountInfo(api, account.sr_address);
    // TODO [TYPE: refactoring][PRI: low]: calculate initial fund based on the estimate amount of all the transfers plus fees
    //       and only send initial fund to the account which has less amount than the inital fund
    account.initialBalance = accountInfo.freeBalance;
    console.log(`Sending fund ${initialAmount.toString()} to account${i}`);
    await simpleTransfer.signAndTransfer(issuerSeed, account.sr_address, initialAmount, api, nonce, keyring);
    nonce = nonce.add(new BN(1));
  }

  let allReceivedFund = false;
  while(!allReceivedFund && keepWaiting){
    for(let i = 0; i < accounts.length; i++){
      if(!accounts[i].receivedFund){
        const accountInfo = await apiUtils.getAccountInfo(api, accounts[i].sr_address);
        // TODO [TYPE: refactoring][PRI: low]: We should check transaction status are all finalized instead of checking the balances.
        //  As pervious executions of this script may leave some transactions in mempool,
        //  which can change the account balance during the waiting period
        accounts[i].receivedFund = accountInfo.freeBalance > accounts[i].initialBalance;
        if(accounts[i].receivedFund){
          console.log(`Transferred funds to account${i}`);
        }
      }
    }

    allReceivedFund = accounts.every(account => {
      return account.receivedFund;
    });
    // TODO [TYPE: refactoring][PRI: low]: Replace this by a setInterval instead of sleeping in a loop.
    await sleep(1000);
  }

  if(!keepWaiting){
    console.log("Some account(s) failed to receive initial fund from the issuer");
    process.exit(1);
  }
}

setTimeout(function(){ keepWaiting = false;}, 60000);

function sleep(ms){
  return new Promise(resolve => setTimeout(resolve, ms));
}

async function showAccountBalances(){
  console.log("\n****Available accounts****\n");
  for(let i = 0; i < accounts.length; i++){
    const accountInfo = await apiUtils.getAccountInfo(api, accounts[i].sr_address);
    console.log(`account${i} (${accounts[i].sr_address}): ${accountInfo.freeBalance.toString()}`);
  }
  console.log();
}

async function main (){
  getArguments();
  verifyArguments();
  await initialiseAPI();
  await setupAccounts();
  await verifyAmount();
  await initialiseAccountBalances();
  await showAccountBalances();
  process.exit(0);
}

main().catch(console.error);
