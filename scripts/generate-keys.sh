#!/bin/bash

set -e

usage() {
  echo -e "\n\
    Script usage: \n\
      Use Subkey 2.0.0 to generate a number of sr25519 and ed25519 key pairs, and store each key pair with their address, \n\
      secret phrase and seed into a file named [sr25519_address].json within keys/[yyyymmddHHMMSS] folder. Stash key is also \n\
      generated if the user wants to create a stash account. This account holds some funds to become a validator or \n\
      nominator in Nominated Proof-of-Stake(NPoS) algorithm, which is used by Substrate node to select validators.\n\
      See https://substrate.dev/docs/en/conceptual/cryptography/keys for more details. \n\
    Run as \n\
      ./generate-keys.sh [options] \n\
    Options: \n\
      -c, --count <KEY_COUNT> Number of key pairs to generate \n\
      -b, --binary <path>     Path to subkey binary, default value is target/release/avn-node \n\
      -o, --output            Output folder \n\
      -h, --help              Displays usage information \n\
  \n"
  exit
}

AVN_KEYS_COUNT=1

while [[ "$#" -gt 0 ]]; do case $1 in
  -c|--count) AVN_KEYS_COUNT="$2"; shift;;
  -b|--binary) bin_path=$(readlink -f $2 || realpath $2); shift;;
  -o|--output) AVN_KEYS_FOLDER="$2"; shift;;
  -h|--help) usage;;
  *) echo "Unknown parameter passed: $1"; usage; exit 1;;
esac; shift; done
AVN_ROOT=$(dirname $(dirname $(readlink -f $0 || realpath $0)))
AVN_SUBKEY_BIN=${bin_path:-$AVN_ROOT/target/release/subkey}
AVN_KEYS_FOLDER=${AVN_KEYS_FOLDER:-$AVN_ROOT/keys/$(date +%Y%m%d%H%M%S)}
mkdir -p $AVN_KEYS_FOLDER
for (( i=1; i<=$AVN_KEYS_COUNT; i++ ))
do
  while read -r line
  do
    if [[ $line == *"Secret phrase"* ]]; then
      BABE_SECRET_PHRASE=$(echo "$line" | cut -d'`' -f 2)
    elif [[ $line == *"Secret seed"* ]]; then
      BABE_SECRET_SEED=`echo $(echo "$line" | cut -d':' -f 2 ) | sed 's/ *$//g'`
    elif [[ $line == *"Public key (hex)"* ]]; then
      BABE_SR25519_PUBLIC_KEY=`echo $(echo "$line" | cut -d':' -f 2 ) | sed 's/ *$//g'`
    elif [[ $line == *"Account ID"* ]]; then
      BABE_SR25519_ACCOUNT_ID=`echo $(echo "$line" | cut -d':' -f 2 ) | sed 's/ *$//g'`
    elif [[ $line == *"SS58 Address"* ]]; then
      BABE_SR25519_SS58_ADDRESS=`echo $(echo "$line" | cut -d':' -f 2 ) | sed 's/ *$//g'`
    fi
  done < <($AVN_SUBKEY_BIN generate --scheme Sr25519)

  while read -r line
  do
    if [[ $line == *"Secret phrase"* ]]; then
      GRAN_SECRET_PHRASE=$(echo "$line" | cut -d'`' -f 2)
    elif [[ $line == *"Secret seed"* ]]; then
      GRAN_SECRET_SEED=`echo $(echo "$line" | cut -d':' -f 2 ) | sed 's/ *$//g'`
    elif [[ $line == *"Public key (hex)"* ]]; then
      GRAN_ED25519_PUBLIC_KEY=`echo $(echo "$line" | cut -d':' -f 2 ) | sed 's/ *$//g'`
    elif [[ $line == *"Account ID"* ]]; then
      GRAN_ED25519_ACCOUNT_ID=`echo $(echo "$line" | cut -d':' -f 2 ) | sed 's/ *$//g'`
    elif [[ $line == *"SS58 Address"* ]]; then
      GRAN_ED25519_SS58_ADDRESS=`echo $(echo "$line" | cut -d':' -f 2 ) | sed 's/ *$//g'`
    fi
  done < <($AVN_SUBKEY_BIN generate --scheme Ed25519)

  while read -r line
  do
    if [[ $line == *"Secret phrase"* ]]; then
      AVN_SECRET_PHRASE=$(echo "$line" | cut -d'`' -f 2)
    elif [[ $line == *"Secret seed"* ]]; then
      AVN_SECRET_SEED=`echo $(echo "$line" | cut -d':' -f 2 ) | sed 's/ *$//g'`
    elif [[ $line == *"Public key (hex)"* ]]; then
      AVN_SR25519_PUBLIC_KEY=`echo $(echo "$line" | cut -d':' -f 2 ) | sed 's/ *$//g'`
    elif [[ $line == *"Account ID"* ]]; then
      AVN_SR25519_ACCOUNT_ID=`echo $(echo "$line" | cut -d':' -f 2 ) | sed 's/ *$//g'`
    elif [[ $line == *"SS58 Address"* ]]; then
      AVN_SR25519_SS58_ADDRESS=`echo $(echo "$line" | cut -d':' -f 2 ) | sed 's/ *$//g'`
    fi
  done < <($AVN_SUBKEY_BIN generate --scheme Sr25519)

  while read -r line
  do
    if [[ $line == *"Secret phrase"* ]]; then
      IMONLINE_SECRET_PHRASE=$(echo "$line" | cut -d'`' -f 2)
    elif [[ $line == *"Secret seed"* ]]; then
      IMONLINE_SECRET_SEED=`echo $(echo "$line" | cut -d':' -f 2 ) | sed 's/ *$//g'`
    elif [[ $line == *"Public key (hex)"* ]]; then
      IMONLINE_SR25519_PUBLIC_KEY=`echo $(echo "$line" | cut -d':' -f 2 ) | sed 's/ *$//g'`
    elif [[ $line == *"Account ID"* ]]; then
      IMONLINE_SR25519_ACCOUNT_ID=`echo $(echo "$line" | cut -d':' -f 2 ) | sed 's/ *$//g'`
    elif [[ $line == *"SS58 Address"* ]]; then
      IMONLINE_SR25519_SS58_ADDRESS=`echo $(echo "$line" | cut -d':' -f 2 ) | sed 's/ *$//g'`
    fi
  done < <($AVN_SUBKEY_BIN generate --scheme Sr25519)

  while read -r line
  do
    if [[ $line == *"Secret phrase"* ]]; then
      AUTHDISC_SECRET_PHRASE=$(echo "$line" | cut -d'`' -f 2)
    elif [[ $line == *"Secret seed"* ]]; then
      AUTHDISC_SECRET_SEED=`echo $(echo "$line" | cut -d':' -f 2 ) | sed 's/ *$//g'`
    elif [[ $line == *"Public key (hex)"* ]]; then
      AUTHDISC_SR25519_PUBLIC_KEY=`echo $(echo "$line" | cut -d':' -f 2 ) | sed 's/ *$//g'`
    elif [[ $line == *"Account ID"* ]]; then
      AUTHDISC_SR25519_ACCOUNT_ID=`echo $(echo "$line" | cut -d':' -f 2 ) | sed 's/ *$//g'`
    elif [[ $line == *"SS58 Address"* ]]; then
      AUTHDISC_SR25519_SS58_ADDRESS=`echo $(echo "$line" | cut -d':' -f 2 ) | sed 's/ *$//g'`
    fi
  done < <($AVN_SUBKEY_BIN generate --scheme Sr25519)

  while read -r line
  do
    if [[ $line == *"Secret phrase"* ]]; then
      ACCOUNT_SECRET_PHRASE=$(echo "$line" | cut -d'`' -f 2)
    elif [[ $line == *"Secret seed"* ]]; then
      ACCOUNT_SECRET_SEED=`echo $(echo "$line" | cut -d':' -f 2 ) | sed 's/ *$//g'`
    elif [[ $line == *"Public key (hex)"* ]]; then
      ACCOUNT_SR25519_PUBLIC_KEY=`echo $(echo "$line" | cut -d':' -f 2 ) | sed 's/ *$//g'`
    elif [[ $line == *"Account ID"* ]]; then
      ACCOUNT_SR25519_ACCOUNT_ID=`echo $(echo "$line" | cut -d':' -f 2 ) | sed 's/ *$//g'`
    elif [[ $line == *"SS58 Address"* ]]; then
      ACCOUNT_SR25519_SS58_ADDRESS=`echo $(echo "$line" | cut -d':' -f 2 ) | sed 's/ *$//g'`
    fi
  done < <($AVN_SUBKEY_BIN generate --scheme Sr25519)

  jq -n --arg babe_secret_phrase "$BABE_SECRET_PHRASE" \
        --arg babe_secret_seed $BABE_SECRET_SEED \
        --arg babe_sr_puk_key $BABE_SR25519_PUBLIC_KEY \
        --arg babe_sr_acc_id $BABE_SR25519_ACCOUNT_ID \
        --arg babe_sr_address $BABE_SR25519_SS58_ADDRESS \
        --arg gran_secret_phrase "$GRAN_SECRET_PHRASE" \
        --arg gran_ed_puk_key $GRAN_ED25519_PUBLIC_KEY \
        --arg gran_secret_seed $GRAN_SECRET_SEED \
        --arg gran_ed_acc_id $GRAN_ED25519_ACCOUNT_ID \
        --arg gran_ed_address $GRAN_ED25519_SS58_ADDRESS \
        --arg avn_secret_phrase "$AVN_SECRET_PHRASE" \
        --arg avn_secret_seed $AVN_SECRET_SEED \
        --arg avn_sr_puk_key $AVN_SR25519_PUBLIC_KEY \
        --arg avn_sr_acc_id $AVN_SR25519_ACCOUNT_ID \
        --arg avn_sr_address $AVN_SR25519_SS58_ADDRESS \
        --arg imonline_secret_phrase "$IMONLINE_SECRET_PHRASE" \
        --arg imonline_secret_seed $IMONLINE_SECRET_SEED \
        --arg imonline_sr_puk_key $IMONLINE_SR25519_PUBLIC_KEY \
        --arg imonline_sr_acc_id $IMONLINE_SR25519_ACCOUNT_ID \
        --arg imonline_sr_address $IMONLINE_SR25519_SS58_ADDRESS \
        --arg authdisc_secret_phrase "$AUTHDISC_SECRET_PHRASE" \
        --arg authdisc_secret_seed $AUTHDISC_SECRET_SEED \
        --arg authdisc_sr_puk_key $AUTHDISC_SR25519_PUBLIC_KEY \
        --arg authdisc_sr_acc_id $AUTHDISC_SR25519_ACCOUNT_ID \
        --arg authdisc_sr_address $AUTHDISC_SR25519_SS58_ADDRESS \
        --arg account_secret_phrase "$ACCOUNT_SECRET_PHRASE" \
        --arg account_secret_seed $ACCOUNT_SECRET_SEED \
        --arg account_sr_puk_key $ACCOUNT_SR25519_PUBLIC_KEY \
        --arg account_sr_acc_id $ACCOUNT_SR25519_ACCOUNT_ID \
        --arg account_sr_address $ACCOUNT_SR25519_SS58_ADDRESS \
        '{ "babe_secret_phrase":$babe_secret_phrase,
          "babe_secret_seed":$babe_secret_seed,
          "babe_sr_puk_key":$babe_sr_puk_key,
          "babe_sr_acc_id":$babe_sr_acc_id,
          "babe_sr_address":$babe_sr_address,
          "gran_secret_phrase":$gran_secret_phrase,
          "gran_secret_seed":$gran_secret_seed,
          "gran_ed_puk_key":$gran_ed_puk_key,
          "gran_ed_acc_id":$gran_ed_acc_id,
          "gran_ed_address":$gran_ed_address,
          "avn_secret_phrase":$avn_secret_phrase,
          "avn_secret_seed":$avn_secret_seed,
          "avn_sr_puk_key":$avn_sr_puk_key,
          "avn_sr_acc_id":$avn_sr_acc_id,
          "avn_sr_address":$avn_sr_address,
          "imonline_secret_phrase":$imonline_secret_phrase,
          "imonline_secret_seed":$imonline_secret_seed,
          "imonline_sr_puk_key":$imonline_sr_puk_key,
          "imonline_sr_acc_id":$imonline_sr_acc_id,
          "imonline_sr_address":$imonline_sr_address,
          "authdisc_secret_phrase":$authdisc_secret_phrase,
          "authdisc_secret_seed":$authdisc_secret_seed,
          "authdisc_sr_puk_key":$authdisc_sr_puk_key,
          "authdisc_sr_acc_id":$authdisc_sr_acc_id,
          "authdisc_sr_address":$authdisc_sr_address,
          "account_secret_phrase":$account_secret_phrase,
          "account_secret_seed":$account_secret_seed,
          "account_sr_puk_key":$account_sr_puk_key,
          "account_sr_acc_id":$account_sr_acc_id,
          "account_sr_address":$account_sr_address,
        }' > $AVN_KEYS_FOLDER/$BABE_SR25519_SS58_ADDRESS.json

  echo "Key pair ${i}:"
  echo "BABE key ss58 address: $BABE_SR25519_SS58_ADDRESS"
  echo "GRANDPA ed25519 ss58 address: $GRAN_ED25519_SS58_ADDRESS"
  echo "IMONLINE sr25519 ss58 address: $IMONLINE_SR25519_SS58_ADDRESS"
  echo "AUTHDISCOVERY sr25519 ss58 address: $AUTHDISC_SR25519_SS58_ADDRESS"
  echo "AVN sr25519 ss58 address: $AVN_SR25519_SS58_ADDRESS"
  echo "---------------------------------------------------------------"
done

echo "Generated key pairs saved in $AVN_KEYS_FOLDER/"
