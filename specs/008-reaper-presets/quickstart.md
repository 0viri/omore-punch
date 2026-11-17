# Quickstart Validation Guide: Reaper Preset Support

**Feature**: `008-reaper-presets` | **Date**: 2026-11-15 | **Spec**: [spec.md](file:///home/tdavio/src/omore-punch/.worktrees/008-reaper-presets/specs/008-reaper-presets/spec.md)

## 1. Automated Verification (CI & Local)

Run from the repository worktree root (`/home/tdavio/src/omore-punch/.worktrees/008-reaper-presets`):

1. **Pure ASCII gate (`src/lib.rs`)**:
   ```bash
   LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs
   ```
   Expected: empty output (exit code 1).

2. **Release unit tests (DSP + state sanitization + VST3/CLAP wrapper round-trips)**:
   ```bash
   cargo test --release
   ```
   Expected: all existing DSP/UI tests plus the new preset tests pass (`test_filter_state_*`, `test_vst3_proxy_state_roundtrip_and_host_sync`, `test_clap_wrapper_state_roundtrip_and_host_sync`, `test_live_preset_switching_stability`).

3. **VST3 & CLAP bundle build**:
   ```bash
   cargo run --package xtask --release -- bundle omore-punch --release
   ```
   Expected: `target/bundled/omore-punch.vst3` and `target/bundled/omore-punch.clap` are generated.

## 2. Manual End-to-End Reaper Validation (Linux & Windows)

Perform on both **VST3** and **CLAP** builds of Omore Punch in Reaper (and optionally repeat with a debug bundle to verify `assert_process_allocs` never fires):

1. **Save & Recall (US1)**:
   - Insert Omore Punch on a drum bus track.
   - Change all 16 controls (knobs, `Release Mode`, `Auto Makeup`, `Clipper Mode`, `Gain Match`) to non-default values.
   - Click `+` -> `Save preset...` (e.g. `Drum Punch Test A`).
   - Change all controls to different values (and save `Drum Punch Test B`).
   - Recall `Drum Punch Test A` from Reaper's preset dropdown (both with the editor window **open** and **closed**, then reopened).
   - Expected: all knobs, toggles, numeric readouts, Reaper generic UI (`UI` button), and audio processing match `Drum Punch Test A` immediately.

2. **Project Persistence (US1)**:
   - Save the `.rpp` project, close Reaper, reopen the project.
   - Expected: Omore Punch restores exact parameter state and preset selection.

3. **Live Playback Switching & Reset to Default (US2)**:
   - Loop a drum bus audio clip in Reaper.
   - Drive the compressor/clipper until `Max GR > 0 dB` and `[CLIP: ACTIVE]` latches.
   - Switch between `Drum Punch Test A` and `Drum Punch Test B` 10 times consecutively while playing.
   - Expected: no dropouts, no auto-mute, smooth transition, and `Max GR` + `CLIP` reset automatically on each preset switch (`FR-008`).
   - Trigger `+` -> `Reset to factory default`.
   - Expected: all 16 parameters return to documented defaults (`0 dB` trim/threshold, `4:1`, `3 ms`, `80 ms`, `Manual`, `20 Hz` SC HPF, `Auto Makeup: ON`, `0 dB` makeup, EQ flat/`20 Hz`, `Soft Clip`, `0 dB` ceiling, `100%` mix, `Gain Match: OFF`).

4. **Cross-OS Library Export / Import (US3)**:
   - Export `.rpl` (and `.vstpreset` for VST3) on Linux, import into the same format on Windows (and vice versa).
   - Expected: all presets appear and recall identically.
