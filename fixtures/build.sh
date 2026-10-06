#!/bin/bash
set -e

cd "$(dirname "$0")/dummy-contract"
cargo build --target wasm32-unknown-unknown --release
