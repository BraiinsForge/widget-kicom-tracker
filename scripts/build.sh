#!/usr/bin/env bash
# Build the widget's NAR with cargo and python3 (with jsonschema); Nix is not needed.
set -euo pipefail

root=$(cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
target=${CARGO_TARGET_DIR:-$root/target}

cargo build --locked --release --target wasm32-unknown-unknown
# The packaging files and the extractor come from the bmc-main checkout cargo made for the SDK.
upstream=$(cargo metadata --locked --format-version 1 | python3 -c '
import json, pathlib, sys
sdk = next(p for p in json.load(sys.stdin)["packages"] if p["name"] == "bmc-wasm-sdk")
print(pathlib.Path(sdk["manifest_path"]).parents[2])')
cargo install --locked --quiet --path "$upstream/bmc-wasm-assets" \
    --root "$target/host" --target-dir "$target/host"
release=$target/wasm32-unknown-unknown/release
python3 scripts/package.py \
    --wasm "$release/kicom_tracker.wasm" \
    --artifact-root "$release/deps" \
    --extractor "$target/host/bin/bmc-wasm-assets" \
    --upstream "$upstream" \
    --upstream-revision "$(git -C "$upstream" rev-parse HEAD)" \
    --out "$target/nar"
