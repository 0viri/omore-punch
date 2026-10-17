#!/usr/bin/env bash
set -euo pipefail

echo "=== [Omore Punch] Execution des tests unitaires DSP ==="
cargo test --release

echo "=== [Omore Punch] Bundling VST3 & CLAP (Release) ==="
cargo run --package xtask --release -- bundle omore-punch --release

echo "=== [Omore Punch] Binaires generes dans target/bundled/ ==="
ls -la target/bundled/

# Copier les bundles
#mkdir -p ~/.clap ~/.vst3
cp -r target/bundled/omore-punch.clap ~/.clap/
cp -r target/bundled/omore-punch.vst3 ~/.vst3/