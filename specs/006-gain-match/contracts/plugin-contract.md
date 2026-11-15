# Contracts: Gain Match Automatique

**Feature**: `006-gain-match` | **Date**: 2026-11-14

## 1. DAW Parameter Contract (`nih-plug` VST3 / CLAP)

| Parameter ID | Rust Field | Type | Default | Automation / Persistence |
|---|---|---|---|---|
| `"gain_match"` | `OmorePunchParams::gain_match` | `BoolParam` (`"Gain Match"`) | `false` (`OFF`) | Exposed to DAW automation; persisted in plugin state; backward-compatible with existing presets (defaults to `false` if absent). |

Existing parameter IDs (`input_db`, `threshold_db`, `ratio`, `attack_ms`, `release_ms`, `release_mode`, `sc_hpf_hz`, `auto_makeup`, `makeup_db`, `eq_low_cut_hz`, `eq_mid_cut_db`, `eq_high_shelf_db`, `clipper_mode`, `ceiling_db`, `mix_pct`, `gr_meter_db`) MUST remain unchanged.

## 2. DSP Contract (`StereoPunchDsp::process_sample`)

- **Signal Chain Position**: Post-Parallel-Mix (step 8), immediately before final `sanitize_sample()`.
- **Channel Symmetry**: Identical linear multiplier `db_to_lin(self.gm_gain_db)` applied to both `mixed_l` and `mixed_r`.
- **Compensation Bounds**: `[-24.0, +24.0]` dB.
- **Silence Gate**: `-60.0 dBFS` (`1.0e-6` mean-square power) on `50.0 ms` input presence detector; freezes (`Hold`) RMS accumulators and `gm_gain_db` when input is below threshold.
- **Safety & Clip Latch**: Output passes through `sanitize_sample()` (`[-4.0, 4.0]`). `SampleOutput::clipped` is `true` if the internal clipper engaged **or** if `cfg.gain_match` is `true` and the post-trim signal exceeds `1.0` (`0 dBFS`).
- **Real-Time Guarantees**: No heap allocations, no blocking synchronization, no per-sample transcendental coefficient recomputation (`exp`, `sin`, `cos`, `tan`).

## 3. UI Contract (`draw_omore_punch_ui` — Section 4)

- **Location**: Right column, inside `4. GAIN REDUCTION & STEREO BUS METERING`, placed cleanly between the `GR` / `[Reset Peak]` header row and the vertical `In L/R`, `GR`, `Out L/R` meters.
- **Controls & Readout (100% ASCII, `monospace()`)**:
  - Toggle button: `[Gain Match: OFF]` (neutral fill `rgb(45, 48, 58)`) / `[Gain Match: ON]` (amber fill `rgb(195, 125, 20)`), wired with `setter.begin_set_parameter` / `set_parameter` / `end_set_parameter` on `&params.gain_match`.
  - Live compensation readout: formatted as `Trim: {:+.1} dB` after normalizing near-zero values (`if db.abs() < 0.05 { 0.0 } else { db }`) to avoid `-0.0 dB` (e.g. `Trim: +0.0 dB`, `Trim: -3.2 dB`), dimmed/neutral when `OFF` and amber/bright when `ON`.
- **Layout Integrity**: Must fit within `UI_WINDOW_WIDTH` (`820`) × `UI_WINDOW_HEIGHT` (`650`) with >= 6 px padding and zero widget overlap (verified by headless `egui` layout unit test).
