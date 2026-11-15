# Data Model: Gain Match Automatique

**Feature**: `006-gain-match` | **Date**: 2026-11-14

## Entities & Fields (all inside `src/lib.rs`)

### 1. `DspConfig` (Block 1 — Pure DSP Configuration)
- **Existing fields**: `input_db`, `threshold_db`, `ratio`, `attack_ms`, `release_ms`, `release_mode`, `sc_hpf_hz`, `auto_makeup`, `makeup_db`, `eq_low_cut_hz`, `eq_mid_cut_db`, `eq_high_shelf_db`, `clipper_mode`, `ceiling_db`, `mix_pct`.
- **New field**:
  - `pub gain_match: bool` — Enables/disables post-processing level matching (`false` by default in `DspConfig::default()`).

### 2. `SampleOutput` (Block 1 — Per-Sample DSP Output)
- **Existing fields**: `out_l: f32`, `out_r: f32`, `gr_db: f32`, `clipped: bool`.
- **New field**:
  - `pub gain_match_db: f32` — Current smoothed gain-match trim applied in dB (clamped to `[-24.0, 24.0]`; `0.0` when inactive and settled).

### 3. `StereoPunchDsp` (Block 1 — Pure DSP Engine State)
- **New cached coefficients** (computed only on `Default` / `set_sample_rate`):
  - `gm_rms_coeff: f32` — 1-pole RMS power smoothing coefficient (`400.0 ms`).
  - `gm_presence_coeff: f32` — 1-pole input presence power smoothing coefficient (`50.0 ms`).
  - `gm_slew_coeff: f32` — 1-pole gain-match dB slew coefficient while active (`150.0 ms`).
  - `gm_off_slew_coeff: f32` — 1-pole gain-match dB return-to-zero slew coefficient when disabled (`35.0 ms`).
- **New state variables** (reset in `StereoPunchDsp::reset()`, sanitized against NaN/Inf/denormals):
  - `gm_in_ms: f32` — Smoothed stereo mean-square power of raw input (`0.0` initially).
  - `gm_post_ms: f32` — Smoothed stereo mean-square power of post-clipper/post-mix output (`0.0` initially).
  - `gm_presence_ms: f32` — Fast stereo mean-square input presence detector (`0.0` initially).
  - `pub gm_gain_db: f32` — Current smoothed gain-match trim in dB (`0.0` initially).

### 4. `MeterTelemetry` (Block 2 — Lock-Free Audio-to-UI Telemetry)
- **Existing fields**: `in_peak_l`, `in_peak_r`, `out_peak_l`, `out_peak_r`, `current_gr_db`, `max_gr_db`, `clip_latched`.
- **New field**:
  - `pub gain_match_db: AtomicU32` — Stores latest `f32` `dsp.gm_gain_db` via `f32::to_bits` / `Ordering::Relaxed`.

### 5. `OmorePunchParams` (Block 3 — Host-Exposed DAW Parameters)
- **New parameter**:
  - `#[id = "gain_match"] pub gain_match: BoolParam` — Display name `"Gain Match"`, default `false`, automatable and persisted in DAW project/preset state without altering existing parameter IDs.

## State Transitions (`gm_gain_db`)

1. **Cold Start / Reset**: `gm_in_ms = 0.0`, `gm_post_ms = 0.0`, `gm_presence_ms = 0.0`, `gm_gain_db = 0.0`.
2. **Active + Signal Present (`cfg.gain_match == true && gm_presence_ms >= 1.0e-6`)**:
   - `gm_in_ms` and `gm_post_ms` integrate raw input and pre-trim output stereo mean-square power (`400 ms`).
   - `target_db = (10.0 * (gm_in_ms.max(1.0e-12) / gm_post_ms.max(1.0e-12)).log10()).clamp(-24.0, 24.0)`.
   - `gm_gain_db` slews smoothly toward `target_db` (`150 ms`).
3. **Active + Silence / Break (`cfg.gain_match == true && gm_presence_ms < 1.0e-6`)**:
   - `gm_in_ms`, `gm_post_ms`, and `gm_gain_db` are held frozen (**Hold**), preventing noise-floor amplification or drift (`0.0 dB` if no valid signal was ever seen).
4. **Disabled (`cfg.gain_match == false`)**:
   - `gm_in_ms` and `gm_post_ms` still track active signal (`gm_presence_ms >= 1.0e-6`) at negligible cost so enabling Gain Match mid-playback starts from accurate RMS estimates.
   - `gm_gain_db` slews smoothly toward `0.0` (`35 ms`) and snaps to `0.0` once `abs < 1.0e-5`, ensuring click-free bypass and bit-exact unity output once settled.
