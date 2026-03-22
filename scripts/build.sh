#!/usr/bin/env bash

cargo build --locked --target wasm32-unknown-unknown --release -p minter

gzip -c target/wasm32-unknown-unknown/release/minter.wasm >target/wasm32-unknown-unknown/release/minter.wasm.gz

./scripts/download.ledger.sh
