#!/usr/bin/env bash
set -euo pipefail

echo "=== [Omore Punch] Execution des tests unitaires DSP ==="
cargo test --release

echo "=== [Omore Punch] Bundling VST3 & CLAP (Release) ==="
cargo run --package xtask --release -- bundle omore-punch --release

echo "=== [Omore Punch] Binaires generes dans target/bundled/ ==="
ls -la target/bundled/

# Copier les bundles localement dans ~/.clap et ~/.vst3
mkdir -p "$HOME/.clap" "$HOME/.vst3"
rm -rf "$HOME/.clap/omore-punch.clap" "$HOME/.vst3/omore-punch.vst3"
cp -r target/bundled/omore-punch.clap "$HOME/.clap/"
cp -r target/bundled/omore-punch.vst3 "$HOME/.vst3/"

echo "Termine ! Plugins installes dans ~/.vst3/omore-punch.vst3 et ~/.clap/omore-punch.clap"
# Ce script ne fait aucune operation Git : les commits se font separement (GEMINI.md).
