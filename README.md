# Avn-tier2
A decentralised, scalable, anonymous non-fungible token blockchain
[![avn-node build & release](https://github.com/Aventus-Network-Services/avn-tier2/actions/workflows/node-release.yml/badge.svg)](https://github.com/Aventus-Network-Services/avn-tier2/actions/workflows/node-release.yml)

Content List:
  * [Substrate Tutorial](#a-quick-tutorial-about-substrate)
  * [Sync a node with the testnet](TestnetSync.md)
  * [Developer mode](#run-node-in-development-node-mode)
  * Create your own private network
    * [Using Docker](docker/README.md)
    * [Using Scripts](#using-scripts)
  * [Setup Grafana Dashboard](dashboard/README.md)
  * [Generate Keys](#generate-keys)
    * [Ethereum keys](#ethereum-keys)
    * [Substrate keys](#substrate-keys)
  * [Generate Chain Spec with Custom Keys](#generate-chain-spec-with-custom-keys)
  * [Generate Keystore Files](#generate-keystore-files)
  * [Generate Transactions using API Scripts](scripts/api/README.md)
  * [Create a Release](#create-a-release)
  * [Upgrade a chain](#upgrade-a-chain)
  * Deployment
    * [AWS Resources Deployment Scripts](scripts/deploy/aws/README.md)
    * [AvN testnet deployment scripts](scripts/deploy/README.md)
    * [Updating substrate](Updating.md)
    * [Custom RPC endpoint](/bin/node/rpc/README.md#using-the-lower_data-rpc-endpoint)
  * Benchmarking
    * [1. Design and Implement Benchmarking for pallet extrinsics](#1-design-and-implement-benchmarking-for-pallet-extrinsics)
    * [2. Build and Benchmarking Runtime](#2-build-and-benchmarking-runtime)
    * [3. Benchmarking RocketDbWeight](#3-benchmarking-rocketdbweight)
    * [4. Benchmarking BlockExecutionWeight](#4-benchmarking-blockexecutionweight)
    * [5. Benchmarking ExtrinsicBaseWeight](#5-benchmarking-extrinsicbaseweight)
    * [6. Update weights with benchmarking results](#6-update-weights-with-benchmarking-results)
    * [7. Archive benchmarking results](#7-archive-benchmarking-results)

## A quick tutorial about Substrate
https://drive.google.com/file/d/1JcPe_q0bEdd6M6rb47FDCShQcnCTThwV/view?usp=sharing

Substrate is used as a library, and it is added as a git submodule to the project, under the substrate folder. To checkout the submodule code, run:
```
git submodule init
git submodule update --remote
```

## Run node in development node mode

Validator nodes need to be configured & connected with an ethereum node.

* See [Ethereum keys](#ethereum-keys) section on how to create the `ethk` key which is a prerequisite.
* Provide the path to the ethereum node. This can be configured from the command line with the ethereum-node-url.

  ganache:
  ```
  avn-node --dev --ethereum-node-url http://127.0.0.1:8545
  ```
  infura (rinkeby):
  ```
  avn-node --dev --ethereum-node-url https://rinkeby.infura.io/v3/a150e1f998cd4562a0f4f45b0964d72b
  ```
  note: The above infura link is an example. You can use any infura link you prefer.
## Create your own private network



### Using Scripts

#### Prerequisites
  + JQ: a lightweight and flexible command-line JSON processor
    ```
    // Run update command to update package repositories and get latest package information.
    sudo apt-get update -y
    // Run the install command with -y flag to quickly install the packages and dependencies.
    sudo apt-get install -y jq
    ```
  + Install Rust: https://www.rust-lang.org/tools/install
  + Install Rust prerequisites:
    ```
    scripts/install_prerequisites.sh
    ```
    **note**: Due to a [bug](https://github.com/rust-lang/rust/issues/77653) in latest version of Rust, we use a specific version of rust nightly and wasm. If you have a newer version installed than 2020-10-06 then set ```WASM_BUILD_TOOLCHAIN="nightly-2020-10-06"``` in your environment.

#### run.sh
  ```
  ./scripts/run.sh build

  Script usage:
    Run commands to build, test or start a blockchain network locally
  Run as
    ./run.sh [commands]
  Commands:
    build    Build the project in release mode
    test     Run all the tests for the node runtime
    deploy   Start a Single Node Development Chain
  ```

#### start-chain.sh
  ```
  ./scripts/start-chain.sh -a alice -p 30333

  Script usage:
    Start Multi-Node Local Testnet and generate a spec JSON file for reviewing or editing, and a spec raw file to share with the other nodes for them to join
  Run as
    ./start-chain.sh [options]
  Options:
    -a, --as           <ACCOUNT_NAME> Account name to start the network, and default value is alice
    -c, --chain        <CHAIN_NAME>   Blockchain name to start, this is also the name of the JSON configuration file name,
                                      and default file is start-default.json
    -g, --grafana-port <GRAFANA_PORT> Start a server with grafana port number to serve Grafana metrics.
    -r, --renew                       Clean the blockchain database, same as purge-chain, and default setup is not to clean
    -s, --source       <SOURCE_NAME>  Source to start the network with, and default value is substrate
    -p, --port         <PORT>         Specify p2p protocol TCP port
    -h, --help                        Displays usage information
  ```

#### join-chain.sh
  ```
  ./scripts/join-chain.sh -a bob -p 30334

  Script usage:
    Start a Multi-Node Local Testnet
  Run as
    ./join-chain.sh [options]
  Options:
    -a, --as          <ACCOUNT_NAME>  Account name used to join the network, and default value is bob
    -b, --bootnodes   <BOOTNODES>     Bootnodes of the blockchain to join
                                      Format:
                                        /ip4/<Boot node IP Address>/tcp/<Boot node port number>/p2p/<Boot node number>
                                      Example:
                                        /ip4/127.0.0.1/tcp/30333/p2p/Qmf8bsJZyeRsGbPqiiSvgFS1XsCNbqmR8bpZVXkxDjJqGa
    -c, --chain       <CHAIN_NAME>    Blockchain name to join, this is also the name of the JSON configuration file name,
                                      and default file is join-default.json
    -g, --grafana-port <GRAFANA_PORT> Start a server with grafana port number to serve Grafana metrics
    -r, --renew                       Clean the blockchain database, same as purge-chain, and default setup is not to clean
    -s, --source      <SOURCE_NAME>   Source to start the network with, and default value is substrate
    -p, --port        <PORT>          Specify p2p protocol TCP port
    -h, --help                        Displays usage information
  ```

  ## Generate Keys
  ### Ethereum keys
  Validator nodes need an ethereum key pair in their keystore, . There are two ways to create this:
  - Through an rpc call [(tutorial)](https://substrate.dev/docs/en/tutorials/start-a-private-network/customchain#add-keys-to-keystore)
  - Generation of the keystore file. Use the `generate-ethk.sh` script, after defining `eth_private_key` and `eth_public_address` variables with the account you want to use. By default it creates the file in the keystore for the `dev` chain, but `ETHK_OUTPUT_PATH` variable can be used to overwrite the output path.
  ### Substrate keys
  In order to generate a test network, we need a number of test accounts with unique keys. The following script uses Subkey 2.0.0 to generate the key pairs.
  ```
  ./scripts/generate-keys.sh -c 3 -o keys/validators
  ./scripts/generate-keys.sh -c 5 -o keys/accounts

  Script usage:
    Use Subkey 2.0.0 to generate a number of sr25519 and ed25519 key pairs, and store each key pair with their address, secret phrase and seed into a file named [sr25519_address].json within keys/[yyyymmddHHMMSS] folder. Stash key is also generated if the user wants to create a stash account. This account holds some funds to become a validator or nominator in Nominated Proof-of-Stake(NPoS) algorithm, which is used by Substrate node to select validators. See https://substrate.dev/docs/en/conceptual/cryptography/keys for more details.
  Run as
    ./generate-keys.sh [options]
  Options:
    -c, --count <KEY_COUNT> Number of key pairs to generate
    -h, --help              Displays usage information
  ```

  ## Generate Chain Spec with Custom Keys
  Once the keys are generated, they can be used in chain spec. Use the following script to generate chain spec with specified key files.

  ```
  ./scripts/generate-chain-spec.sh -c validator -o scripts/specs/ -k keys/validators/ --ethereum-events <ethereum-events.json>

    Script usage:
      This script generates a chain specification file and a chain specification raw file containing account information
      specified in the key files.
    Run as
      ./generate-chain-spec.sh [options]
    Options:
      -c, --chain-name <CHAIN_NAME>     Name of the blockchain, this name will be used in the chain spec file and chain spec
                                        raw file names
      -b, --binary <path>               Path to avn-node binary, default value is target/release/avn-node
      -o, --output                      Output folder
      -k, --key-files <KEY_FILES>       Either a key file name or a directory containing key files generated by using
                                        generate-key.sh script
      -e, --eth-keys-file <KEYS_FILE>   Key file containing Ethereum key pairs in json format
      --staging                         Generate a staging chain-spec
      --lift-contract                   Ethereum contract used by the avn-tier2 for lift operations
      --publish-root-contract           Ethereum contract used by the avn-tier2 for publish-root operations
      --validators-contract             Ethereum contract used by the avn-tier2 for validators operations
      -b, --bootnode                    Specify a bootnode to be added in the spec
      --ethereum-events                 Ethereum events file with events that we want to inject to the chain-spec generation
      --overwrite-initial-funds         Overwrite the initial funds of an account from 0 to 1000 AVT. Should NOT be used in any chain-spec used in production
      -h, --help                        Displays usage information
  ```
  The `ethereum-events.json` file must contains all the ethereum transactions that are registrations for the initial validators of the chain in the `processed_events` array. If not, then those events will be able to be processed again, leading to an inconsistent state of the chain.
  Additionally you can specify lift ethereum transactions that you wish to be processed upon the start of the chain in the `initial_lift_events` array. When generating a chain, by default all accounts have 0 AVT. The only way to fund them AVT is with a lift transaction. Because no-one has AVT no-one can pay the transaction fees for an operation. We provide this initial lift mechanism to bypass this issue, where we pre-load some lift transactions that we want the chain to process. For testing purposes you can use the `--overwrite-initial-funds` flag to change initial funding from 0 to 1000.

  These are a mandatory step when generating a testnet and production chain. A [sample](scripts/configs/eth-events.json.sample) file is provided as a template.

  `ethereum-events.json` **must** be a valid json file.

  ## Generate Keystore Files
  Once the keys are generated, they can be used to generate keystore files. Use the following script to generate keystore files with specified key files.
  ```
  ./scripts/generate-keystores.sh -b /tmp/validators -k keys/validators -n validator
  ./scripts/generate-keystores.sh -b /tmp/accounts -k keys/accounts/ -n account

    Script usage:
      This script generates a pair of keystore files into the node keystore, one file named with 6772616e as prefix, which is
      gran in hex, followed by ed_puk_key without the '0x' prefix. A second file is created with 62616265 as prefix, which is babe in hex,
      followed by sr_puk_key without the '0x' prefix. Both files take the secret phrase with "" as their contents.
    Run as
      ./generate-keystores.sh [options]
    Options:
      -b, --base-path <PATH>      Specify custom base path
      -k, --key-file  <KEY_FILE>  A key file or a directory containing multiple key files generated by using
                                  generate-key.sh script
      -n, --node-name <NAME>      Specify custom node name
      --staging                   Generate a staging chain-spec
      -h, --help                  Displays usage information
  ```
  ## Run Validator Nodes
  NB: If you have followed the examples above and generated a chain spec and keystore then use the 'start-validator-node.sh' script as follows:
  ```
  ./scripts/start-validator-node.sh -r

    Script usage: \n\
      Start or join a Multi-Node Local Validator Testnet
    Run as \n\
      ./start-validator-node.sh [options]
    Options: \n\
      -b, --bootnodes   <BOOTNODES> Bootnodes of the blockchain to join
                                    Format:
                                      /ip4/<Boot node IP Address>/tcp/<Boot node port number>/p2p/<Boot node number>
                                    Example:
                                      /ip4/127.0.0.1/tcp/30333/p2p/Qmf8bsJZyeRsGbPqiiSvgFS1XsCNbqmR8bpZVXkxDjJqGa
      -c, --config <CONFIG_NAME>    Name of the JSON configuration file name, the default file is validators.json
      -n, --node index              Node index, bootstrap should be 0
      -g, --grandpa-off             Turn off grandpa finalisation
      -r, --renew                   Clean the blockchain database, same as purge-chain, and default setup is not to clean
      -h, --help                    Displays usage information
  ```
  and look for the line in the log output which specifies the local node identity, for example:
  ```
  Local node identity is: QmTQc5RwDiT5Q8ReNL1TeMHa1TdMkPTMt9G7WyHBm7g8YW
  ```
  Then use that local node identity with the script to join subsequent nodes:
  ```
  ./scripts/start-validator-node.sh -n 2 -b QmTQc5RwDiT5Q8ReNL1TeMHa1TdMkPTMt9G7WyHBm7g8YW -r
  ./scripts/start-validator-node.sh -n 3 -b QmTQc5RwDiT5Q8ReNL1TeMHa1TdMkPTMt9G7WyHBm7g8YW -r
  ```

  ## Create a Release
  To generate a release for AvN you will need to:
  - identify the commit that will be used for the release
  - create a tag with this format: `v*.*.*` i.e. `v1.0.0`, `v20.15.10`
  - push the tag to github

  After that github will trigger the release workflow and generate a draft release for the tag used, with the build artifacts attached, if no errors occur. If you wish to publish the release, you can do so from the project [releases](https://github.com/artosSystems/avn-tier2/releases). These releases are only visible to those with access to the repository. The workflow can be extended to publish the release to a different, public repository where we can control what gets published (Releases & Documentation).

  If you are doing a release, read the [release guide](https://docs.google.com/document/d/1XOSTaaINe66cNwctsu1Q85eRnWStHRAtuT7Jnt6Etb8/edit#) to identify any follow up steps.
```
  # Create tag for v1.2.3 on commit c4ab1fe
  git tag v1.2.3 c4ab1fe
  # Push the commit to github
  git push origin v1.2.3
```
**Important Note:** Upon release github automatically creates archives with all the source code attached. Internally this uses `git archive`. A rule has been added in [.gitattributes](.gitattributes) file to exclude all files from the archive.

## Upgrade a chain

Use the node_runtime.compact.wasm build artifact from a release and use these [instructions](https://substrate.dev/docs/en/tutorials/upgrade-a-chain/sudo-upgrade) to perform a forkless upgrade.

## Benchmarking

  ### 1. Design and Implement Benchmarking for pallet extrinsics

  1.1 Analyse the extrinsic to identify key factors and its complexity using Big O notations, and document it within the description for the extrinsic.

  1.2 Create benchmarking.rs file or update the existing file's benchmarks macro to cover all worst paths for the extrinsic.

  1.3 Add unit test or update existing test functions to assert the new functions created in the benchmarks macro.

  1.4 Run the benchmark tests like below:

  ```
    cargo test --features runtime-benchmarks bench
  ```

  References:
  - [Sub0 Online: Benchmarking Deep Dive by Shawn Tabrizi](https://www.youtube.com/watch?v=i3zW4wGexAc)
  - [Substrate Benchmarking Documentation](https://www.shawntabrizi.com/substrate-graph-benchmarks/docs/#/)
  - [Substrate Weight and Fees](https://www.shawntabrizi.com/substrate/substrate-weight-and-fees/)

  ### 2. Build and Benchmarking Runtime

  2.1 Update the pallet's cargo.toml file and:
  - Update all dependencies required in pallet's benchmarking.rs to be `optional = true`. `pallet-avt` dependency is a popular one.
  - Add these optional dependencies to `runtime-benchmarks` feature as:
    ```
    runtime-benchmarks = [
      "frame-benchmarking",
      "frame-support/runtime-benchmarks",
      "frame-system/runtime-benchmarks",
      "pallet-avt",
    ]
    ```
    Note: Don't check in these changes to master as it may fail the build.

  2.2 Build binaries with `runtime-benchmarks` feature:
  ```
    cd bin/node/cli && cargo build --release --features runtime-benchmarks
  ```

  2.3 Run benchmarking against a pallet and its extrinsics locally.
  e.g. to cover all the extrinsics in token-manager pallet run:
  ```
  cd ../../.. && ./target/release/avn-node benchmark \
  --chain dev \
  --execution=wasm \
  --wasm-execution=compiled \
  --pallet pallet_token_manager \
  --extrinsic "*" \
  --steps 50 \
  --repeat 20 \
  --heap-pages=4096 \
  --template=./.maintain/frame-weight-template.hbs \
  --output pallet_token_manager.rs
  ```
  This will output benchmarking result in plain text, and also a pallet_token_manager.rs file containing a WeightInfo implementation with a full list of functions defined in the pallet's benchmarking.rs file.

  2.4 Upload avn-node binary built with runtime-benchmarks feature to a production environment, such as an AWS EC2 instance. Then run benchmarking from there.

  ### 3. Benchmarking RocketDbWeight
  Upload node-bench binary to the production environment, run the following commands and save the output.

  ```
  cargo run --release -p node-bench -- ::trie::read::large
  cargo run --release -p node-bench -- ::trie::write::large
  ```

  ### 4. Benchmarking BlockExecutionWeight
  Upload node-bench binary to the production environment, run the following commands and save the output.
  ```
  cargo run --release -p node-bench -- ::node::import::wasm::sr25519::noop::rocksdb::empty
  ```

  ### 5. Benchmarking ExtrinsicBaseWeight
  Upload node-bench binary to the production environment, run the following commands and save the output.
  ```
  cargo run --release -p node-bench -- ::node::import::wasm::sr25519::noop::rocksdb::custom --transactions 10000
  ```
  Note: This execution panics as bin/node/bench/src/import.rs sanity check assertion fails. We can temperally comment that assertion out when block type is `BlockType::Noop` for the moment in order to execute a successful benchmarking.

  ### 6. Update weights with benchmarking results

  Please make sure the WeightInfo for all pallets and extrinsics are calibrated in a production environment before pushing into master. As different hardware may have different speed of reading and writing to their storage and memories, the local tested result may not be correct in a production node.

  Weights files to create or update:
  - bin/node/runtime/src/weights/mod.rs
  - bin/node/runtime/src/weights/[my_pallet].rs
  - frame/[my-pallet]/src/default_weights.rs
  - frame/avn/src/weights.rs

  ### 7. Archive Benchmarking Results

  Benchmarking activities and outputs are achieved and logged in our shared drive: `~/Tech/Benchmarking` with the following file structure:
  ```
  Tech
  └───Benchmarks
      │   Benchmarking Logs   <- Logs of each benchmarking executions, include DbWeight/BlockExecutionWeight/ExtrinsicBaseWeight weights
      └───2021-02-07_1500
          └───Output          <- Execution print out files
          └───WeightInfo      <- Generated WeightInfo files
  ```