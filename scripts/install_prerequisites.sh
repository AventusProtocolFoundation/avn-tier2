#!/bin/bash
rustup toolchain install 1.55.0 && rustup toolchain install nightly
rustup target add wasm32-unknown-unknown --toolchain nightly
rustup default 1.55.0-x86_64-unknown-linux-gnu
# Use this if you want to use the latest toolchains.
# run: rustup update nightly && rustup update stable
# run: rustup target add wasm32-unknown-unknown --toolchain nightly
