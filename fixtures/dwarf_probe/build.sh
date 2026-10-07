#!/bin/bash
# Rebuild the committed artifacts. Run from this directory after changing src/lib.rs, then
# commit the two `.wasm` files: the tests read them with `include_bytes!`, so what is committed
# must match what this script produces.
set -e
cd "$(dirname "$0")"

cargo build --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/dwarf_probe.wasm ./dwarf_probe.wasm

# The same source with debug info off, which is what a default release build ships. Used by the
# tests that assert a DWARF-free binary is reported rather than silently accepted.
sed -i.bak 's/^debug = 1$/debug = false/' Cargo.toml
cargo build --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/dwarf_probe.wasm ./dwarf_probe_no_debug.wasm
mv Cargo.toml.bak Cargo.toml
