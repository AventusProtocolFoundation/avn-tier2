#!/bin/bash

usage() {
  echo -e "\n\
    Script usage: \n\
      Join a Multi-Node Local Testnet with configurations stored in join-chain.json file\n\
    Run as \n\
      ./join-chain.sh [options] \n\
    Options: \n\
      -a, --as        <ACCOUNT_NAME>    Account name used to join the network, and default value is bob \n\
      -b, --bootnodes <BOOTNODES>       Bootnodes of the blockchain to join \n\
                                        Format: \n\
                                          /ip4/<Boot node IP Address>/tcp/<Boot node port number>/p2p/<Boot node number> \n\
                                        Example: \n\
                                          /ip4/127.0.0.1/tcp/30333/p2p/Qmf8bsJZyeRsGbPqiiSvgFS1XsCNbqmR8bpZVXkxDjJqGa \n\
      -c, --chain     <CHAIN_NAME>      Blockchain name to join, this is also the name of the JSON configuration file name, \n\
                                        and default file is join-default.json \n\
      -g, --grafana-port <GRAFANA_PORT> Start a server with grafana port number to serve Grafana metrics. \n\
      -r, --renew                       Clean the blockchain database, same as purge-chain, and default setup is not to clean \n\
      -s, --source    <SOURCE_NAME>     Source to start the network with, and default value is substrate \n\
      -p, --port      <PORT>            Specify p2p protocol TCP port \n\
      -h, --help                        Displays usage information \n\
  \n"
  exit
}

LYDIA_ACCOUNT="bob"
LYDIA_CHAIN="join-default"
LYDIA_BUILT_SOURCE="avn-node"
LYDIA_BUILD_MODE="release"
LYDIA_RENEW_DATABASE=0

while [[ "$#" -gt 0 ]]; do case $1 in
  -a|--as) LYDIA_ACCOUNT="$2"; shift;;
  -b|--bootnodes) LYDIA_BOOTNODES="$2"; shift;;
  -c|--chain) LYDIA_CHAIN="$2"; shift;;
  -g|--grafana-port) LYDIA_CONF_GRAFANA_PORT="$2"; shift;;
  -r|--renew) LYDIA_RENEW_DATABASE=1;;
  -s|--source) LYDIA_BUILT_SOURCE="$2"; shift;;
  -p|--port) LYDIA_CONF_PORT="$2"; shift;;
  -h|--help) usage;;
  *) echo "Unknown parameter passed: $1"; usage; exit 1;;
esac; shift; done

if [[ -z "$LYDIA_CONF_PORT" ]]; then
  $LYDIA_CONF_PORT=$(cat ./configs/$LYDIA_CHAIN.json | jq '.port' | tr -d \")
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

if [ ! -z $LYDIA_BOOTNODES ]; then
  LYDIA_BOOTNODES="--bootnodes ${LYDIA_BOOTNODES/localhost/127.0.0.1}"
fi

echo "************************************************"
echo "* Joining a blockchain ${LYDIA_CHAIN} *"
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
--grafana-external \
$LYDIA_BOOTNODES"

echo "Running:"
echo "$runCommand"
$runCommand