# Using the lower_data rpc endpoint

## Pre-requisites

Before calling this end point to get details about a lower transaction you need to perform a successfull lower action on tier 2.
After you have lowered, take a note of the following:

  - The block number your transaction was added to
  - The index of your lower transaction in the block
  - The sender account address (in Base58 format)
  - The token that was lowered (the Ethereum address of the token)
  - The amount lowered and
  - The tier1 recipient address (Ethereum address)

## Getting the data

You can use your favorite client (`curl`, `postman`...) to send an RPC request. Postman example:

```
{
    "jsonrpc":"2.0",
    "id":1,
    "method":"lower_data",
    "params": [61, 120, 107, 2, "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY", "0x82Fea31F3f2bF9E555c9A27Aa4CfAF3ec2468C1e", 1, "0xa2BDA93504CC9dc25CCdCe3D7c610D53Fa21D77f"]
}
```

The order of the parameters are:
  1. from block
  2. to block
  3. block number
  4. transaction index
  5. sender account
  6. token
  7. amount
  8. tier1 recipient
