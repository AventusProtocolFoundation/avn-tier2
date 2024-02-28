const { ApiPromise, WsProvider, Keyring } = require('@polkadot/api');
const BN = require('bn.js');
const keyring = new Keyring({ type: 'sr25519' });
const path = require('path');
const fs = require('fs');
const apiUtils = require('./utils/apiUtils.js');
const validator = require('./utils/validationUtils.js');

//https://github.com/paritytech/substrate/blob/a4404bc1410ff599dd66b78de1244532b2854f97/primitives/transaction-pool/src/pool.rs#L60
let txState = {"Ready": 0, "Broadcast": 0, "Future": 0, "Finalized": 0, "FinalityTimeout": 0, "Invalid": 0, "Dropped": 0, "Usurped": 0, "InBlock": 0, "Retracted": 0};
let nodesFile, accountsFolderPath, transactionsPerAccountPerRound, roundInterval, executionPeriod;
let accounts = [];
let accountsNonces = [];
let nodeAPIs = [];
let unsubs = [];
let totalTxCounts = [];
let txReceiptCounts = [];
let txStates = [];
let sendTransactionsIntervalId;
let showTransactionsStatusIntervalId;
let timeStartSendingTxns;

const DEV = new BN(1000000000000);
const SECOND = 1000;
const TRANSFER_AMOUNT = DEV.mul(new BN(1));
const DEFAULT_TRANSACTIONS_PER_ACCOUNT_PER_ROUND = 100;
const DEFAULT_ROUND_INTERVAL = 1;
const DEFAULT_EXECUTION_PERIOD = 60;
const SHOW_STATUS_DELAY = 60 * SECOND;
const summaryFilePath = path.join(__dirname, `results/${(new Date()).toISOString().slice(0,19).replace(/[:]/g, '-')}.json`);

function showUsage() {
  console.log(`
    This script generates transactions, sends funds between multiple accounts, and balances the load across multiple
    nodes in a number of rounds within an execution period. It writes some summary statistics to ./scripts/api/results/.
    Run as
      node generateTransactions.js [nodes JSON file] [accounts directory] [OPTIONAL: number of transactions per
      account per round] [OPTIONAL: round interval in second] [OPTIONAL: execution period in second]
    Examples of
      nodes json file:                                ./scripts/configs/nodes.json
      accounts directory:                             ./keys/accounts/
      number of transactions per account per round:   100 (default is 100)
      round interval in second:                       1 (default is 1)
      execution period in second:                     60 (default is 60)
  `);
}
// TODO [TYPE: refactoring][PRI: low]: Allow each optional argument to be optional by itself
// Currently, if the third argument is to be ommitted, the fourth and fifth will be misinterpreted
function getArguments() {
  const args = process.argv.slice(2);
  if(args.length < 2 || args.length > 5) {
    console.log("Wrong number of parameters");
    showUsage();
    process.exit(1);
  }
  nodesFile = args[0];
  accountsFolderPath = args[1];
  transactionsPerAccountPerRound = args[2] || DEFAULT_TRANSACTIONS_PER_ACCOUNT_PER_ROUND;
  roundInterval = (args[3] || DEFAULT_ROUND_INTERVAL) * SECOND;
  executionPeriod = (args[4] || DEFAULT_EXECUTION_PERIOD) * SECOND;
}

function verifyArguments() {
  if(!validator.isValidPath(nodesFile, "Nodes JSON File Not Found")
  || !validator.isValidPath(accountsFolderPath, "Invalid Accounts Directory")
  || !validator.isValidValue(transactionsPerAccountPerRound, "Invalid Number of Transactions per Round")
  || !validator.isValidValue(roundInterval, "Invalid Rounds Interval")
  || !validator.isValidValue(executionPeriod, "Invalid Execution Period")) {
    showUsage();
    process.exit(1);
  }
}

function addState(state, nodeIndex) {
  if(state == "Finalized" || state == "Invalid") {
    txReceiptCounts[nodeIndex]++;
  }
  txStates[nodeIndex][state]++;
}

function showStates(nodeIndex) {
  console.log(`\n****Node${nodeIndex} Status****\n`);
  for(let key in txState) {
    console.log(`  - ${key}: ${txStates[nodeIndex][key]}`);
  }
  console.log(`Total Transaction Count:     ${totalTxCounts[nodeIndex]}`);
  console.log(`Transaction Receipt Count:   ${txReceiptCounts[nodeIndex]}`);
}

function showSummary(txnsSummary) {
  console.log(`\n****Transaction Status Summary: ${txnsSummary.Label} in ${txnsSummary.TimeTakenInSeconds} seconds****`);
  for(let key in txState) {
    console.log(`  - ${key}: ${txnsSummary.TransactionStates[key]}`);
  }
  console.log(`Total Transaction Count:     ${txnsSummary.TransactionsSent}`);
  console.log(`Transaction Receipt Count:   ${txnsSummary.TransactionReceiptsReceived}`);
}

async function initialiseAPI(nodeHost, wsPort, nodeIndex) {
  const wsProvider = new WsProvider(`ws://${nodeHost}:${wsPort}`);
  const api = await ApiPromise.create({ provider: wsProvider,
    types: {
      NFT: {
        "id": "H256",
        "owner": "AccountId"
      }
  }});

  //TODO [TYPE: refactoring][PRI: low]: Extract polkadot api logic to a separate module

  // Retrieve the chain & node information via rpc calls
  const [chain, nodeName, nodeVersion] = await Promise.all([
    api.rpc.system.chain(),
    api.rpc.system.name(),
    api.rpc.system.version()
  ]);

  unsubs.push(await api.rpc.chain.subscribeNewHeads((header) => {
    console.log(`Node${nodeIndex} - Block #${header.number}  (${header.hash}) Mined.`);
  }));

  console.log(`Node${nodeIndex} - You are connected to chain ${chain} at ${nodeHost}:${wsPort} using ${nodeName} v${nodeVersion}\n`);

  return api;
}

async function setupAccounts() {
  let accountFiles = fs.readdirSync(accountsFolderPath);
  accountFiles.forEach(file => {
    let filePath = path.join(accountsFolderPath, file);
    let account = JSON.parse(fs.readFileSync(filePath));
    accounts.push(account);
  });
}

async function initialiseNodeAPIs() {
  var nodeAddresses = JSON.parse(fs.readFileSync(nodesFile));
  for(let i = 0; i < nodeAddresses.length; i++) {
    const api = await initialiseAPI(nodeAddresses[i].host, nodeAddresses[i].wsPort, i);
    nodeAPIs.push(api);
  }
}

async function showAccountBalances(api) {
  console.log("*********************Available accounts*****************************************");
  for(let i = 0; i < accounts.length; i++) {
    const accountInfo = await apiUtils.getAccountInfo(api, accounts[i].sr_address);
    console.log(`account${i} (${accounts[i].sr_address}): ${accountInfo.freeBalance.toString()}`);
  }
  console.log("********************************************************************************");
}

async function sendTransactionsThroughNode(nodeIndex) {
    const numberOfSenderAccounts = accounts.length - 1;
    const receiverAccountIndex = numberOfSenderAccounts;
    const totalTxnsSendToNode = numberOfSenderAccounts * transactionsPerAccountPerRound;
    console.time(`${totalTxnsSendToNode} transactions sent to node${nodeIndex} in`);

    for(let i = 0; i < numberOfSenderAccounts; i++) {
      for(let j = 0; j < transactionsPerAccountPerRound; j++) {
        let nonce = accountsNonces[i];
        totalTxCounts[nodeIndex] = totalTxCounts[nodeIndex] + 1;
        let transaction = await nodeAPIs[nodeIndex].tx.balances.transfer(accounts[receiverAccountIndex].sr_address, TRANSFER_AMOUNT);
        transaction.signAndSend(keyring.addFromUri(accounts[i].secret_seed), {nonce}, ({status}) => {
          addState(status.type, nodeIndex);
        });
        accountsNonces[i] = nonce.add(new BN(1));
      }
    }

    console.timeEnd(`${totalTxnsSendToNode} transactions sent to node${nodeIndex} in`);
}

async function sendTransactions() {
  totalTxCounts = new Array(nodeAPIs.length).fill(0);
  txReceiptCounts = new Array(nodeAPIs.length).fill(0);
  txStates = new Array(nodeAPIs.length);
  nodeAPIs.forEach((api, nodeIndex) => {
    txStates[nodeIndex] = JSON.parse(JSON.stringify(txState));
  });

  accountsNonces = new Array(accounts.length);
  for(let i = 0; i < accounts.length; i++) {
    accountsNonces[i] = (await apiUtils.getAccountInfo(nodeAPIs[0], accounts[i].sr_address)).nonce;
  }

  let roundNumber = 1;
  timeStartSendingTxns = Date.now();

  sendTransactionsIntervalId = setInterval(async () => {
    if(roundNumber != 1)
      summarise(`After Round ${roundNumber-1}`);
    console.log(`\n-------------------- Round${roundNumber} ------------------`);
    for(let nodeIndex = 0; nodeIndex < nodeAPIs.length; nodeIndex++) {
      await sendTransactionsThroughNode(nodeIndex);
    }
    console.log(`----------------------------------------------\n`);
    roundNumber++;
  }, roundInterval);
}

function turnSendTransactionsOff() {
  setTimeout(() => {
    clearInterval(sendTransactionsIntervalId);
    summarise("After Last Round");
  }, executionPeriod);
}

function turnShowStatusOff() {
  setTimeout(() => {
    clearInterval(showTransactionsStatusIntervalId);
    nodeAPIs.forEach((api, nodeIndex) => {
      unsubs[nodeIndex]();
      showStates(nodeIndex);
    });
    summarise("After Waiting for Cooldown Period");
    process.exit(0);
  }, executionPeriod + SHOW_STATUS_DELAY);
}

async function summarise(label){
  let txnsInfo = getTxnsInfo();
  let summary = {
    Label: label,
    TimeTakenInSeconds: (Date.now() - timeStartSendingTxns) / 1000.0,
    TransactionsSent: txnsInfo.transactionsSent,
    TransactionReceiptsReceived: txnsInfo.transactionReceiptsReceived,
    TransactionStates: txnsInfo.transactionStates
  };
  showSummary(summary);
  saveSummary(summary);
}

function getTxnsInfo(){
  let transactionStates = JSON.parse(JSON.stringify(txState));
  Object.keys(txState).forEach(state => {
    txStates.forEach(nodeTxState => transactionStates[state] += nodeTxState[state]);
  });
  return {
    transactionsSent: totalTxCounts.reduce((a,b) => a+b),
    transactionReceiptsReceived: txReceiptCounts.reduce((a,b) => a+b),
    transactionStates: transactionStates
  };
}

function saveSummary(summary) {
  try{
    let summaries = [];
    if(fs.existsSync(summaryFilePath)){
      summaries=JSON.parse(fs.readFileSync(summaryFilePath));
    }
    summaries.push(summary);
    fs.writeFileSync(summaryFilePath, JSON.stringify(summaries));
  } catch (err) {
    console.log(err);
  }
}

async function showNodesStatus() {
  showTransactionsStatusIntervalId = setInterval(async () => {
    for (let nodeIndex = 0; nodeIndex < nodeAPIs.length; nodeIndex++) {
      await nodeAPIs[nodeIndex].rpc.author.pendingExtrinsics((extrinsics) => {
        console.log(`Node${nodeIndex} has ${extrinsics.length} pending extrinsics in the pool and waiting for ${totalTxCounts[nodeIndex] - txReceiptCounts[nodeIndex]} transaction receipts.`);
      });
    }
  }, 1 * SECOND);
}

async function main() {
  getArguments();
  verifyArguments();
  await setupAccounts();
  await initialiseNodeAPIs();
  await showAccountBalances(nodeAPIs[0]);
  turnSendTransactionsOff();
  turnShowStatusOff();
  await sendTransactions();
  await showNodesStatus();
}

main().catch(console.error);
