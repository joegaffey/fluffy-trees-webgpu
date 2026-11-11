#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
cargo build --release --target wasm32-unknown-unknown
wasm-bindgen target/wasm32-unknown-unknown/release/fluffy_trees.wasm \
  --out-dir pkg --target web
echo "Built pkg/. Serve with: python3 -m http.server 8080"
