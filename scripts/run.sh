#!/bin/bash

usage() {
  echo -e "\n\
    Script usage:\n\
      Run commands to build, test or start a blockchain network locally \n\
    Run as \n\
      ./run.sh [commands] \n\
    Commands: \n\
      build    Build the project in release mode \n\
      test     Run all the tests for the node runtime \n\
      deploy   Start a fresh blockchain node locally \n\
  \n"
  exit
}

# Find the root of the repository
LYDIA_ROOT=$(dirname $(dirname $(readlink -f $0 || realpath $0)))

if [ "$1" == "build" ]; then
  pushd $LYDIA_ROOT/bin/node/
  cargo build --release
  popd
elif [ "$1" == "test" ]; then
  pushd $LYDIA_ROOT/bin/node/runtime/
  cargo test
  popd
elif [ "$1" == "deploy" ]; then
  pushd $LYDIA_ROOT
  if [[ ! -z "$2" ]] && [ "$2" = "purge" ]; then
    ./target/release/avn-node purge-chain --dev -y
  fi
  ./target/release/avn-node --dev
  popd
else
  usage
fi