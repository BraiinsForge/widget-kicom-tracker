#!/usr/bin/env bash
# The formatting, Clippy and test checks of `nix flake check`, without Nix.
set -euo pipefail

root=$(cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"

cargo fmt -- --check
cargo clippy --locked --target wasm32-unknown-unknown -- -D warnings
cargo test --locked --lib
