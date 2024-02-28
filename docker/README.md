## Create an AvN private chain using docker-compose
## Prerequisites
### Install Docker engine or Docker Desktop
 - https://docs.docker.com/engine/install/
 - https://docs.docker.com/engine/install/linux-postinstall/
 - docker compose [1.29.1](https://docs.docker.com/compose/install/) or newer

###
To avoid having multiple configurations with replicated code we use the extend feature of docker-compose:
 - https://docs.docker.com/compose/extends/

That way the local and QA setup are re-using most part of the setup, with QA being the main reference point.

### docker config file
Under docker/config folder you can find a sample config file (env.sample). This file sets 3 environment variables:
 - `image_tag`: the tag of the docker image to use or build.
 - `ethereum_node_url`: with default value ```http://172.17.0.1:8545/```. 172.17.0.1 is the host machine address in a linux environment.
 - `COMPOSE_PROJECT_NAME`: The project name that docker-compose will use.
Do not change the sample configuration, instead create a new one with the values you want and pass it as part of the --env-file parameteres.
### avn-node release build (for local development)
A release build under avn-tier2/target. [avn-tier2 README.md](../README.md) has instructions on how to create one.
### avn-tier1
The scripts assume that avn-tier1 is deployed & initialised in the `ethereum_node_url` endpoint.
### avn-scripts
The output of avn-scripts configuration should exists in `avn-scripts-output` folder
This is the expected output for a staging configuration:
```avn-scripts-output/
avn-scripts-output/.gitignore
avn-scripts-output/AvnValidator0
avn-scripts-output/AvnValidator0/runValidator0.sh
avn-scripts-output/AvnValidator0/set_audi_sessionKey.sh
avn-scripts-output/AvnValidator0/set_avnk_sessionKey.sh
avn-scripts-output/AvnValidator0/set_babe_sessionKey.sh
avn-scripts-output/AvnValidator0/set_ethk_sessionKey.sh
avn-scripts-output/AvnValidator0/set_gran_sessionKey.sh
avn-scripts-output/AvnValidator0/set_imon_sessionKey.sh
avn-scripts-output/AvnValidator1
avn-scripts-output/AvnValidator1/runValidator1.sh
avn-scripts-output/AvnValidator1/set_audi_sessionKey.sh
avn-scripts-output/AvnValidator1/set_avnk_sessionKey.sh
avn-scripts-output/AvnValidator1/set_babe_sessionKey.sh
avn-scripts-output/AvnValidator1/set_ethk_sessionKey.sh
avn-scripts-output/AvnValidator1/set_gran_sessionKey.sh
avn-scripts-output/AvnValidator1/set_imon_sessionKey.sh
avn-scripts-output/AvnValidator2
avn-scripts-output/AvnValidator2/runValidator2.sh
avn-scripts-output/AvnValidator2/set_audi_sessionKey.sh
avn-scripts-output/AvnValidator2/set_avnk_sessionKey.sh
avn-scripts-output/AvnValidator2/set_babe_sessionKey.sh
avn-scripts-output/AvnValidator2/set_ethk_sessionKey.sh
avn-scripts-output/AvnValidator2/set_gran_sessionKey.sh
avn-scripts-output/AvnValidator2/set_imon_sessionKey.sh
avn-scripts-output/AvnValidator3
avn-scripts-output/AvnValidator3/runValidator3.sh
avn-scripts-output/AvnValidator3/set_audi_sessionKey.sh
avn-scripts-output/AvnValidator3/set_avnk_sessionKey.sh
avn-scripts-output/AvnValidator3/set_babe_sessionKey.sh
avn-scripts-output/AvnValidator3/set_ethk_sessionKey.sh
avn-scripts-output/AvnValidator3/set_gran_sessionKey.sh
avn-scripts-output/AvnValidator3/set_imon_sessionKey.sh
avn-scripts-output/AvnValidator4
avn-scripts-output/AvnValidator4/runValidator4.sh
avn-scripts-output/AvnValidator4/set_audi_sessionKey.sh
avn-scripts-output/AvnValidator4/set_avnk_sessionKey.sh
avn-scripts-output/AvnValidator4/set_babe_sessionKey.sh
avn-scripts-output/AvnValidator4/set_ethk_sessionKey.sh
avn-scripts-output/AvnValidator4/set_gran_sessionKey.sh
avn-scripts-output/AvnValidator4/set_imon_sessionKey.sh
avn-scripts-output/bootnode-key
avn-scripts-output/chainspec.json
avn-scripts-output/chainspecRaw.json
avn-scripts-output/tier1GenesisConfig.json

```
## Network setup
Under `docker/bin/` folder where you will find the familiar wrapper scripts for docker.

You will need to setup avn-tier1 deployment & create a local chain with avn-scripts. A template input configuration for avn-scripts:

```
{
  "avnBinaryPath": "<avn-tier2>/target/release/avn-node",
  "validatorKeysFilePath": "<avn-tier2>/docker/data/avn-keys/local_network_keys.json",
  "numberOfValidators":  5,
  "chainType":  "local",
  "preFund":  true,
  "sudoAddress":  null,
  "web3ProviderUrl":  "http://localhost:8545",
  "abiPath": "<avn-tier1>/build/contracts",
  "tier1SudoPrivateKey": "0xdeaf8f08e8272bce5cc8258d4af0585f9b0436e83637b05b639dbc5b3c4ef7a0",
  "contractAddressesFilePath": "<avn-tier1>/contractAddresses.json",
  "outputPath":  "<avn-tier2>/avn-scripts-output/"
}
```
The output might create some subfolder, but you want the chainspec to be directly under the `avn-scripts-output` folder:
```
...
avn-scripts-output/chainspecRaw.json
avn-scripts-output/.gitignore
...
```
Once that is in place, you have to build the docker images. For a local setup run:
```
bin/build
```
QA setups should use the remote images directly and skip the build step.
```
bin/setup-keystore
```
To stop the network:
```
bin/stop
```
To stop the network and remove all data:
```
bin/remove-docker-volumes
```

## QA Network

### AWS ECR access (for qa setup)
To access the ECR repositories, you will need to have installed the `aws-cli` tool and configured to access the `UAT Testnet` account
- https://docs.aws.amazon.com/cli/latest/userguide/cli-chap-configure.html

To access the registry you need to log in to ECR with this command:
```
aws ecr get-login-password --region eu-west-1 | docker login --username AWS --password-stdin 372459114472.dkr.ecr.eu-west-1.amazonaws.com
```

The images will be pulled automatically when needed.

Update the contents of `config/env.dev` with the values from `config/env.sample`
Then update the `ethereum_node_url` value to the ethereum endpoint you want to use, and `image_tag` to the one you want to use.
After that you can start the network with the scripts under `bin`.

### ganache accounts
env.dev config and ganache-data folder have ethk files generated for 5 accounts, for a ganache instance that is initialized with this mnemonic:
```
"lady sad two vacuum rail siren barrel convince rare helmet wagon approve"
```
Which is the same as the one we use in avn-tier1: run.sh.
