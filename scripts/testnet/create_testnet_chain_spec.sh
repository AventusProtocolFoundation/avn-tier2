#!/bin/bash

avn_tier2_dir=""
release_dir=`pwd`

eth_accounts_file="${release_dir}/ethereum-keys.json"
eth_events_file="${release_dir}/ethereum-events.json"
avn_binaries_dir="${release_dir}/avn-binaries/"

set -e

# Step 1: Create session keys
if [ ! -d "${release_dir}/session_keys" ]; then
    echo "Creating Session keys"
    mkdir -p ${release_dir}/session_keys
    ${avn_tier2_dir}/scripts/generate-keys.sh -c 10 -o ${release_dir}/session_keys/ --binary ${avn_binaries_dir}/subkey
fi

######
read -p "Do the tier1 deployemnt at this point. After it is done, add a config file containing the addresses and an ethereum events file \
with initial lifts & the initialise AvN transaction in the processed list."

######

if [[ ! -f contracts.json ]] ; then
    echo contracts info missing. Exiting.
    exit
fi

# Step 2: Create keystore files
if [ ! -d "${release_dir}/keystore" ]; then
    echo "Creating keystore"
    mkdir -p ${release_dir}/keystore
    ${avn_tier2_dir}/scripts/generate-keystores.sh -k ${release_dir}/session_keys/ --base-path ${release_dir}/keystore/ --ethereum-keys-file ${eth_accounts_file}
fi

# Step 3: Create "public info" files
if [ ! -d "${release_dir}/chain_spec_accounts" ]; then
    echo "Creating chain-spec accounts"
    mkdir -p ${release_dir}/chain_spec_accounts
    cp ${release_dir}/session_keys/* ${release_dir}/chain_spec_accounts

    # For Testnet we don't need replacing the account section as we used the original accounts for a deployment
    # validator_sr_puk=()
    # validator_sr_address=()
    # # Validator 1
    # validator_sr_puk+=("<Replace with [account_sr_puk_key] from keys file>")
    # validator_sr_address+=("<Replace with [account_sr_address] from keys file>")
    # # Validator 2
    # validator_sr_puk+=("<Replace with [account_sr_puk_key] from keys file>")
    # validator_sr_address+=("<Replace with [account_sr_address] from keys file>")
    # # Validator 3
    # validator_sr_puk+=("<Replace with [account_sr_puk_key] from keys file>")
    # validator_sr_address+=("<Replace with [account_sr_address] from keys file>")
    # # Validator 4
    # validator_sr_puk+=("<Replace with [account_sr_puk_key] from keys file>")
    # validator_sr_address+=("<Replace with [account_sr_address] from keys file>")
    # # Validator 5
    # validator_sr_puk+=("<Replace with [account_sr_puk_key] from keys file>")
    # validator_sr_address+=("<Replace with [account_sr_address] from keys file>")
    # # Validator 6
    # validator_sr_puk+=("<Replace with [account_sr_puk_key] from keys file>")
    # validator_sr_address+=("<Replace with [account_sr_address] from keys file>")
    # # Validator 7
    # validator_sr_puk+=("<Replace with [account_sr_puk_key] from keys file>")
    # validator_sr_address+=("<Replace with [account_sr_address] from keys file>")
    # # Validator 8
    # validator_sr_puk+=("<Replace with [account_sr_puk_key] from keys file>")
    # validator_sr_address+=("<Replace with [account_sr_address] from keys file>")
    # # Validator 9
    # validator_sr_puk+=("<Replace with [account_sr_puk_key] from keys file>")
    # validator_sr_address+=("<Replace with [account_sr_address] from keys file>")
    # # Validator 10
    # validator_sr_puk+=("<Replace with [account_sr_puk_key] from keys file>")
    # validator_sr_address+=("<Replace with [account_sr_address] from keys file>")

    # count=0
    for session_key in ${release_dir}/chain_spec_accounts/*.json
    do
        result=`cat $session_key | jq '. | del(.["account_secret_phrase", "account_secret_seed", "authdisc_secret_phrase", "authdisc_secret_seed", "imonline_secret_phrase", "imonline_secret_seed", "avn_secret_phrase", "avn_secret_seed", "gran_secret_phrase", "gran_secret_seed", "babe_secret_phrase", "babe_secret_seed"])'`
        # result2=`echo $result | jq '(.account_sr_puk_key, .account_sr_acc_id) |= '\"${validator_sr_puk[$count]}\"'' | jq '(.account_sr_address) |= '\"${validator_sr_address[$count]}\"''`
        echo $result > ${session_key}
        # count=$((count+1))
    done
fi

# Step 4: Create the ethereum-events file
# - Initial lift
# - Initialise AvN transaction Hash: to be added as a AddValidatorLog

# Step 5 Contract address & sudo
# TestNet AVT token contract
avt_contract=`cat contracts.json | jq .avtContractAddress | tr -d '"'`
# TestNet AvnValidatorsManager
avn_validators_manager=`cat contracts.json | jq .validatorsManagerAddress | tr -d '"'`
# TestNet AvnFTScalingManager
avn_ft_scaling_manager=`cat contracts.json | jq .avnFTScalingManagerAddress | tr -d '"'`

tier2_sudo_account_puk=""
tier2_sudo_account_address=""

# Step 6: Create chainspec

command="${avn_tier2_dir}/scripts/generate-chain-spec.sh -c avnTestNet --staging \
    -k ${release_dir}/chain_spec_accounts/ \
    -o ${release_dir} \
    --lift-contract ${avn_ft_scaling_manager} \
    --publish-root-contract ${avn_validators_manager} \
    --validators-contract ${avn_validators_manager} \
    --avt-contract ${avt_contract} \
    --eth-keys-file ${eth_accounts_file} \
    --ethereum-events ${eth_events_file} \
    --binary ${avn_binaries_dir}/avn-node \
    --sudo \"${tier2_sudo_account_address}\""

#   --bootnode
#   --overwrite-initial-funds   we shouldn't use this.
#   --default-sudo              we specify sudo account, this isn't needed
#   --quorum-factor             No need to change
#   --event-challenge-period    2 hours default value

# Generate the chain-spec
echo ${command}
${command}

