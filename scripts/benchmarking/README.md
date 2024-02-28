# Setup

Run `npm install` from this directory

## Running the script

Run `node main.js`.

The script should output 2 signatures for the proxy call and the signed transfer call. You can use these signatures in `TokenManager -> benchmarking.rs`. The `0x` prefix is removed to allow users to copy paste the output without any modifications.

## Updating the data to sign

To make changes to the data to sign, open main.js and update the constant values to match the actual data in `TokenManager -> benchmarking.rs`.
