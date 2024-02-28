#!/bin/bash

keystore_path=${ETHK_OUTPUT_PATH:-"$HOME/.local/share/avn-node/chains/dev/keystore"}
# Omit 0x pre-fix in these values
eth_private_key=""
eth_public_address=""

# The keystore file must be all lowercase
eth_public_address=${eth_public_address,,}
mkdir -p ${keystore_path}

# 6574686b is ethk in hex + public address with 0x omitted
echo \"${eth_private_key}\" > ${keystore_path}/6574686b${eth_public_address}
