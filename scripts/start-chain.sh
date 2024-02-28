#!/bin/bash

set -e

usage() {
  echo -e "\n\
    Script usage: \n\
      Start a Multi-Node Local Testnet generate a spec JSON file for reviewing or editing, \n\
      and a spec raw file to share with the other nodes for them to join \n\
    Run as \n\
      ./start-chain.sh [options] \n\
    Options: \n\
      -a, --as <ACCOUNT_NAME> \n\
               Account name to start the network, and default value is alice \n\
      -c, --chain <CHAIN_NAME> \n\
               Blockchain name to start, this is also the name of the JSON configuration file name, \n\
               and default file is start-default.json \n\
      -g, --grafana-port <GRAFANA_PORT> \n\
               Start a server with grafana port number to serve Grafana metrics. \n\
      -p, --port <PORT> \n\
               Specify p2p protocol TCP port \n\
      -r, --renew \n\
               Clean the blockchain database, same as purge-chain, and default setup is not to clean \n\
      -s, --source <SOURCE_NAME> \n\
               Source to start the network with, and default value is substrate \n\
      -h, --help \n\
               Displays usage information \n\
  \n"
  exit
}

LYDIA_ACCOUNT="alice"
LYDIA_BUILT_SOURCE="substrate"
LYDIA_CHAIN="start-default"
LYDIA_RENEW_DATABASE=0
# TODO [TYPE: refactoring][PRI: low]: Make this configurable
LYDIA_BUILD_MODE="release"

while [[ "$#" -gt 0 ]]; do case $1 in
  -a|--as) LYDIA_ACCOUNT="$2"; shift;;
  -c|--chain) LYDIA_CHAIN="$2"; shift;;
  -g|--grafana-port) LYDIA_CONF_GRAFANA_PORT="$2"; shift;;
  -r|--renew) LYDIA_RENEW_DATABASE=1;;
  -s|--source) LYDIA_BUILT_SOURCE="$2"; shift;;
  -p|--port) LYDIA_CONF_PORT="$2"; shift;;
  -h|--help) usage;;
  *) echo "Unknown parameter passed: $1"; usage; exit 1;;
esac; shift; done

if [[ -z "$LYDIA_CONF_PORT" ]]; then
  LYDIA_CONF_PORT=$(cat ./configs/$LYDIA_CHAIN.json | jq '.port' | tr -d \")
fi

if [[ -z "$LYDIA_CONF_GRAFANA_PORT" ]]; then
  LYDIA_CONF_GRAFANA_PORT=$(cat ./configs/$LYDIA_CHAIN.json | jq '.grafana_port' | tr -d \")
fi

# TODO [TYPE: refactoring][PRI: low]: Load variable values from configuration to a separate function
LYDIA_CONF_BASE_PATH=$(cat ./configs/$LYDIA_CHAIN.json | jq '.base_path' | tr -d \")
LYDIA_CONF_CHAIN=$(cat ./configs/$LYDIA_CHAIN.json | jq '.chain' | tr -d \")
LYDIA_CONF_WS_PORT=$(cat ./configs/$LYDIA_CHAIN.json | jq '.ws_port' | tr -d \")
LYDIA_CONF_RPC_PORT=$(cat ./configs/$LYDIA_CHAIN.json | jq '.rpc_port' | tr -d \")
LYDIA_CONF_TELEMETRY_URL=$(cat ./configs/$LYDIA_CHAIN.json | jq '.telemetry_url' | tr -d \")

echo "**********************************************************"
echo "* Cleaning any cached data for blockchain ${LYDIA_CHAIN} *"
echo "**********************************************************"

if [ $LYDIA_RENEW_DATABASE == 1 ]; then
  rm -fr $LYDIA_CONF_BASE_PATH$LYDIA_ACCOUNT
fi

echo "*******************************************************"
echo "* Generating spec files for blockchain ${LYDIA_CHAIN} *"
echo "*******************************************************"
pushd specs
../../target/${LYDIA_BUILD_MODE}/${LYDIA_BUILT_SOURCE} build-spec --chain local > ${LYDIA_CHAIN}Spec.json
../../target/${LYDIA_BUILD_MODE}/${LYDIA_BUILT_SOURCE} build-spec --chain ${LYDIA_CHAIN}Spec.json --raw > ${LYDIA_CHAIN}SpecRaw.json
popd

echo "************************************************"
echo "* Initialising a new blockchain ${LYDIA_CHAIN} *"
echo "************************************************"

# TODO [TYPE: refactoring][PRI: medium]: make validator option configurable
runCommand="../target/$LYDIA_BUILD_MODE/$LYDIA_BUILT_SOURCE \
--base-path $LYDIA_CONF_BASE_PATH$LYDIA_ACCOUNT \
--chain $LYDIA_CONF_CHAIN \
--$LYDIA_ACCOUNT \
--port $LYDIA_CONF_PORT \
--ws-port $LYDIA_CONF_WS_PORT \
--rpc-port $LYDIA_CONF_RPC_PORT \
--telemetry-url $LYDIA_CONF_TELEMETRY_URL \
--validator \
--grafana-port $LYDIA_CONF_GRAFANA_PORT \
--grafana-external"

echo "Running:"
echo "$runCommand"
$runCommand