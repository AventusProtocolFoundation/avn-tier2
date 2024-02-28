# initialiseAccountBalances
```
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
```

# generateTransactions
```
This script generates transactions, sends funds between multiple accounts, and balances the load across multiple nodes in a number of rounds within an execution period. It writes some summary statistics to ./scripts/api/results/.
Run as
  node generateTransactions.js [nodes JSON file] [accounts directory] [OPTIONAL: number of transactions per account per round] [OPTIONAL: round interval in second] [OPTIONAL: execution period in second]
Examples of
  nodes json file:                                ./scripts/configs/nodes.json
  accounts directory:                             ./keys/accounts/
  number of transactions per account per round:   100 (default is 100)
  round interval in second:                       1 (default is 1)
  execution period in second:                     60 (default is 60)
```

 # parseGrafanaData
 This script collapses raw data collected from Grafana, for better analysis.
 It expects to receive csv files in the format exported by Grafana. It produces a similar file in a similar format, with all null values collapsed and without the time column.
 The result is created in a file named `out_<input filename>`

## To run:
 
 Options:
 - node parseGrafanaData.js <csv file> 
 - node parseGrafanaData.js --test
