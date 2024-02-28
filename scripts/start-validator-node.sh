#!/bin/bash

set -e

usage() {
  echo -e "\n\
    Script usage: \n\
      Start or join a Multi-Node Local Validator Testnet \n\
    Run as \n\
      ./start-validator-node.sh [options] \n\
    Options: \n\
      -b, --bootnodes <BOOTNODES> \n\
               Bootnodes of the blockchain to join \n\
               Format: \n\
                 /ip4/<Boot node IP Address>/tcp/<Boot node port number>/p2p/<Boot node number> \n\
               Example: \n\
                 /ip4/127.0.0.1/tcp/30333/p2p/Qmf8bsJZyeRsGbPqiiSvgFS1XsCNbqmR8bpZVXkxDjJqGa \n\
      -c, --config <CONFIG_NAME> \n\
               Name of the JSON configuration file name, the default file is validators.json \n\
      -n, --node-index \n\
               Node index, bootstrap should be 0 \n\
      -g, --grandpa-off \n\
               Turn off grandpa finalisation \n\
      -r, --renew \n\
               Clean the blockchain database, same as purge-chain, and default setup is not to clean \n\
      -h, --help \n\
               Displays usage information \n\
  \n"
  exit
}

LYDIA_CONFIG="validator"
LYDIA_RENEW_DATABASE=0
LYDIA_GRANDPA=0
LYDIA_NODE_ID=1
LYDIA_KEYS_FOLDER="${LYDIA_KEYS_FOLDER:-$LYDIA_ROOT/keys/$(date +%Y%m%d%H%M%S)}"

# Parse command line arguments
while [[ "$#" -gt 0 ]]; do case $1 in
  -b|--bootnodes) LYDIA_BOOTNODES="$2"; shift;;
  -c|--config) LYDIA_CONFIG="$2"; shift;;
  -g|--grandpa-off) LYDIA_GRANDPA=1;;
  -r|--renew) LYDIA_RENEW_DATABASE=1;;
  -n|--node-id) LYDIA_NODE_ID="$2"; shift;;
  -h|--help) usage;;
  *) echo "Unknown parameter passed: $1"; usage; exit 1;;
esac; shift; done

LYDIA_NODE_INDEX=LYDIA_NODE_ID-1
LYDIA_ROOT=$(dirname $(dirname $(readlink -f $0 || realpath $0)))
LYDIA_CONF_DATA=`cat $LYDIA_ROOT/scripts/configs/$LYDIA_CONFIG.json`
LYDIA_BUILT_SOURCE=`echo $LYDIA_CONF_DATA | jq '.source' | tr -d \"`
LYDIA_BUILD_MODE=`echo $LYDIA_CONF_DATA | jq '.build' | tr -d \"`
LYDIA_CONF_BASE_PATH=`echo $LYDIA_CONF_DATA | jq '.base_path' | tr -d \"`
LYDIA_CONF_PORT=`echo $LYDIA_CONF_DATA | jq '.port' | tr -d \"`
LYDIA_CONF_WS_PORT=`echo $LYDIA_CONF_DATA | jq '.ws_port' | tr -d \"`
LYDIA_CONF_RPC_PORT=`echo $LYDIA_CONF_DATA | jq '.rpc_port' | tr -d \"`
LYDIA_CONF_AVN_PORT=`echo $LYDIA_CONF_DATA | jq '.avn_port' | tr -d \"`
LYDIA_CONF_TELEMETRY_URL=`echo $LYDIA_CONF_DATA | jq '.telemetry_url' | tr -d \"`
LYDIA_CONF_PROMETHEUS_PORT=`echo $LYDIA_CONF_DATA | jq '.prometheus_port' | tr -d \"`

# Clean the database if needed
if [ $LYDIA_RENEW_DATABASE == 1 ]; then
  echo "*************************************************************"
  echo "* Cleaning any cached block data for blockchain $LYDIA_CONFIG *"
  echo "*************************************************************"
  rm -rf $LYDIA_CONF_BASE_PATH/$LYDIA_CONFIG$LYDIA_NODE_ID/chains/local_testnet/db/
fi

runCommand="./target/$LYDIA_BUILD_MODE/$LYDIA_BUILT_SOURCE \
--base-path $LYDIA_CONF_BASE_PATH/$LYDIA_CONFIG$LYDIA_NODE_ID \
--chain $LYDIA_ROOT/scripts/specs/${LYDIA_CONFIG}SpecRaw.json \
--name $LYDIA_CONFIG$LYDIA_NODE_ID \
--port $((LYDIA_CONF_PORT+LYDIA_NODE_INDEX)) \
--ws-port $((LYDIA_CONF_WS_PORT+LYDIA_NODE_INDEX)) \
--rpc-port $((LYDIA_CONF_RPC_PORT+LYDIA_NODE_INDEX)) \
--avn-port $((LYDIA_CONF_AVN_PORT+LYDIA_NODE_INDEX)) \
--validator \
--prometheus-port $((LYDIA_CONF_PROMETHEUS_PORT+LYDIA_NODE_INDEX))"

if [[ -n $LYDIA_BOOTNODES ]]; then
  runCommand="$runCommand --bootnodes /ip4/127.0.0.1/tcp/$LYDIA_CONF_PORT/p2p/$LYDIA_BOOTNODES"
fi

if [[ $LYDIA_GRANDPA == 1 ]]; then
  runCommand="$runCommand --no-grandpa"
fi

echo "Running:"
echo "$runCommand"
$runCommand