#!/bin/bash

# Download ICRC-1 ledger and index canisters (used for vUSD)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DIR=target/ic

if [ ! -d "$DIR" ]; then
  mkdir -p "$DIR"
fi

VERSION="ledger-suite-icrc-2025-06-19"
BASE_URL="https://github.com/dfinity/ic/releases/download/$VERSION"

# Ledger
"$SCRIPT_DIR/download-immutable.sh" "$BASE_URL/ic-icrc1-ledger.wasm.gz" "$DIR"/ledger.wasm.gz
gunzip --force "$DIR"/ledger.wasm.gz
"$SCRIPT_DIR/download-immutable.sh" "$BASE_URL/ledger.did" "$DIR"/ledger.did

# Index
"$SCRIPT_DIR/download-immutable.sh" "$BASE_URL/ic-icrc1-index-ng.wasm.gz" "$DIR"/index.wasm.gz
gunzip --force "$DIR"/index.wasm.gz
"$SCRIPT_DIR/download-immutable.sh" "$BASE_URL/index-ng.did" "$DIR"/index.did
