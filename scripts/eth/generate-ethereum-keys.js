const { encodeAddress, base58Encode } = require('@polkadot/util-crypto');
const { hexToU8a } = require('@polkadot/util');
const { ethers, Wallet } = require('ethers');
const fs = require('fs');
const path = require('path');

function generateKeys(numberOfKeysToGenerate) {
  let result = []
  let privateKeys = []

  let generatedKeys = 0;

  while (generatedKeys < numberOfKeysToGenerate) {
    const keypair = Wallet.createRandom()

    if (privateKeys.includes(keypair.privateKey)) {
      continue;
    }

    let pubK = ethers.utils.computePublicKey(keypair.publicKey, true);
    let keys = {
      'privateKey': keypair.privateKey,
      'publicKey': pubK,
      'address': keypair.address,
      'uncompressedPublicKey': keypair.publicKey,
      'base58EncodedPublicKey': encodeAddress(hexToU8a(pubK))
    }
    result.push(keys)
    privateKeys.push(keys)

    generatedKeys += 1;
  }

  return {'keys' : result};
}


function showUsage() {
  console.log(`
    This script generates Ethereum compatible keys, in JSON format, that can be used to populate the chainspec.
    Run as
      node generate-ethereum-keys.js [number of keys to generate] [OPTIONAL - full output file. Default: ./ethKeys.json]
    Example
      node generate-ethereum-keys.js 5 ./eth/keys/output.json
  `);
}

function getArguments() {
  const args = process.argv.slice(2);
  if(![1,2].includes(args.length)) {
    console.log("Wrong number of parameters");
    showUsage();
    process.exit(1);
  }

  return {
    'numberOfKeysToGenerate': args[0],
    'outputPath': args[1] ? args[1] : './ethKeys.json'
  }
}

function run() {
  let args = getArguments();
  let generatedKeys = generateKeys(args.numberOfKeysToGenerate);
  fs.mkdirSync(path.parse(args.outputPath).dir, { recursive: true });
  fs.writeFileSync(args.outputPath, JSON.stringify(generatedKeys));

  console.log(`Generated ${args.numberOfKeysToGenerate} ethereum key pair${args.numberOfKeysToGenerate > 1 ? 's' : ''} at ${args.outputPath}`)
}

run()