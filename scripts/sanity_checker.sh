#!/bin/bash
set -e

usage() {
  echo -e "\n\
    Script usage:\n\
      Test our code - not all of Substrate - optionally with coverage \n\
    Run as \n\
      ./test-our-code.sh [commands] \n\
    Commands: \n\
      grcov   Run tests with grcov coverage\n\
      tarp    Run tests with tarpaulin coverage\n\
      bench   Runs tests and benchmarks\n\
      todo    Print all the TODOs in these directories\n\
  \n"
  exit
}

runCommand() {
  echo
  echo "=== $1 ==="
  pushd $1
  $CMD
  popd
}

run() {
  runCommand frame/avn
  runCommand bin/node/cli/avn-service
  runCommand bin/node/rpc
  runCommand primitives/avn-common
  run_pallets_that_allow_benchmarks
}

run_pallets_that_allow_benchmarks() {
  runCommand frame/avn-finality-tracker
  runCommand frame/avn-offence-handler
  runCommand frame/ethereum-events
  runCommand frame/ethereum-transactions
  runCommand frame/nft-manager
  runCommand frame/summary
  runCommand frame/token-manager
  runCommand frame/validators-manager
  runCommand frame/avn-proxy
}

REPOSITORY_ROOT=$(dirname $(dirname $(readlink -f $0 || realpath $0)))
pushd $REPOSITORY_ROOT

if [ "$1" == "tarp" ]; then
  # See https://crates.io/crates/cargo-tarpaulin
  echo "*** WITH TARPAULIN"
  export CMD='cargo tarpaulin -b -o Html'
  run
  echo "*** See, eg frame/ethereum-events/tarpaulin-report.html\#frame/ethereum-events/src for results"
elif [ "$1" == "grcov" ]; then
    # See https://lib.rs/crates/grcov
    echo "*** WITH GRCOV"
    export CARGO_INCREMENTAL=0
    export RUSTFLAGS="-Zprofile -Ccodegen-units=1 -Copt-level=0 -Clink-dead-code -Coverflow-checks=off -Zpanic_abort_tests -Cpanic=abort"
    export RUSTDOCFLAGS="-Cpanic=abort"
    export CMD='cargo +nightly test'
    run
    grcov ./target/debug/ -s . -t html --llvm --branch --ignore-not-existing -o ./target/debug/coverage/
    echo "*** See target/debug/coverage/ for results"
elif [ "$1" == "bench" ]; then
  export CMD='cargo test --features runtime-benchmarks -- benchmarking'
  run_pallets_that_allow_benchmarks
elif [ "$1" == "todo" ]; then
  export CMD='git grep TODO'
  run
elif [ "$1" == "" ]; then
  export CMD='cargo test'
  run
else
  usage
fi

popd
