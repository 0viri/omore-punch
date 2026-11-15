# Quickstart & Validation Guide: Gain Match Automatique

**Feature**: `006-gain-match` | **Date**: 2026-11-14

## Prerequisites

- Working directory: `/home/tdavio/src/omore-punch/.worktrees/006-gain-match` (branch `feat/006-gain-match`).
- Rust toolchain with `cargo` installed.

## Automated Quality Gates

1. **Pure ASCII check on `src/lib.rs`** (Constitution Principle II):
   ```bash
   LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs
   ```
   Expected: empty output, exit code `1` (no non-ASCII bytes).

2. **DSP & UI Unit Tests in Release Mode** (Constitution Principle V):
   ```bash
   cargo test --release
   ```
   Expected: all existing tests pass plus new Gain Match tests verifying:
   - Level matching convergence within `<= 0.5 dB` under compression, Drum Bus EQ boost/cut, and clipping.
   - Identical L/R gain application (0.0 dB stereo balance shift).
   - Silence / noise-floor `Hold` behavior (0.0 dB at cold start, held trim during breaks below -60 dBFS).
   - Clamping within `[-24.0, +24.0]` dB and final safety clamp `[-4.0, 4.0]`.
   - Smooth ON/OFF toggling without abrupt step discontinuities.
   - Coexistence with `Auto Makeup` and `Makeup Gain`.
   - Headless `egui` layout fit inside `820x650` with Gain Match `OFF` and `ON`.

3. **VST3 & CLAP Bundle Build** (Constitution Platform Gate):
   ```bash
   cargo run --package xtask --release -- bundle omore-punch --release
   ```
   Expected: `target/bundled/omore-punch.vst3` and `target/bundled/omore-punch.clap` built cleanly without warnings/errors.

## Manual DAW Verification (Reaper)

1. Load `omore-punch` on a stereo drum bus track in Reaper.
2. Verify `[Gain Match: OFF]` and `Trim: +0.0 dB` appear in Section 4 (`4. GAIN REDUCTION & STEREO BUS METERING`) with no overlap.
3. Dial heavy compression (`Threshold -18 dB`, `Ratio 6:1`), `High Shelf +6 dB`, or `Soft Clip` (`Ceiling -4 dB`), then toggle `[Gain Match: ON]`.
4. Confirm `In L/R` and `Out L/R` perceived loudness match, `Trim` updates smoothly in dB, and toggling `ON`/`OFF` produces no audible click.
5. Pause playback / feed silence and confirm `Trim` freezes cleanly without boosting background noise.
