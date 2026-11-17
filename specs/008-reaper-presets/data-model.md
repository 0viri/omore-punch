# Data Model: Reaper Preset Support

**Feature**: `008-reaper-presets` | **Date**: 2026-11-15 | **Spec**: [spec.md](file:///home/tdavio/src/omore-punch/.worktrees/008-reaper-presets/specs/008-reaper-presets/spec.md)

## Entities

### 1. `PresetState` (`nih_plug::prelude::PluginState`)

Canonical serialized snapshot persisted by Reaper (`.rpp` project chunks, `.rpl` preset libraries, `.vstpreset` files, and CLAP host state streams).

| Field | Type | Description | Validation / Compatibility Rules |
| --- | --- | --- | --- |
| `version` | `String` | Plugin version string (`env!("CARGO_PKG_VERSION")`) at save time; may be empty in legacy chunks | Preserved for diagnostics; missing/empty or older versions remain 100% loadable |
| `params` | `BTreeMap<String, ParamValue>` | Map of stable string parameter IDs to unnormalized plain values | Must contain all 16 canonical keys after `filter_state`; unknown keys stripped; missing/invalid-type keys filled with defaults |
| `fields` | `BTreeMap<String, String>` | Arbitrary persisted non-parameter fields | Currently unused (`{}`); legacy `"editor-state"` key stripped cleanly so window dimensions remain fixed |

### 2. Canonical Parameter Schema (16 Parameters in `OmorePunchParams`)

Every parameter below is normalized and bounded by `sanitize_plugin_state` during `OmorePunch::filter_state`:

| Parameter ID | Type | Default | Valid Domain | Mapping / Notes |
| --- | --- | --- | --- | --- |
| `input_db` | `F32` | `0.0` | `[-12.0, 12.0]` | Clamped; non-finite -> `0.0` |
| `threshold_db` | `F32` | `0.0` | `[-40.0, 0.0]` | Clamped; non-finite -> `0.0` |
| `ratio` | `F32` | `4.0` | `[1.0, 20.0]` | Clamped; non-finite -> `4.0` |
| `attack_ms` | `F32` | `3.0` | `[0.1, 100.0]` | Clamped; non-finite -> `3.0` |
| `release_ms` | `F32` | `80.0` | `[10.0, 500.0]` | Clamped; non-finite -> `80.0` |
| `release_mode` | `String` (or `I32`) | `"manual"` | `"manual"` \| `"auto_tight"` \| `"auto_punch"` \| `"auto_glue"` (or `I32(0..=3)`) | Normalized to canonical `String` ID; invalid -> `"manual"` |
| `sc_hpf_hz` | `F32` | `20.0` | `[20.0, 250.0]` | Clamped; non-finite -> `20.0` |
| `auto_makeup` | `Bool` | `true` | `true` \| `false` | Wrong type -> `true` |
| `makeup_db` | `F32` | `0.0` | `[-12.0, 24.0]` | Clamped; non-finite -> `0.0` |
| `eq_low_cut_hz` | `F32` | `20.0` | `[20.0, 160.0]` | Clamped; non-finite -> `20.0` |
| `eq_mid_cut_db` | `F32` | `0.0` | `[-12.0, 0.0]` | Clamped; non-finite -> `0.0` |
| `eq_high_shelf_db` | `F32` | `0.0` | `[-6.0, 12.0]` | Clamped; non-finite -> `0.0` |
| `clipper_mode` | `String` (or `I32`) | `"soft_clip"` | `"soft_clip"` \| `"hard_clip"` \| `"bypass_clip"` (or `I32(0..=2)`) | Normalized to canonical `String` ID; invalid -> `"soft_clip"` |
| `ceiling_db` | `F32` | `0.0` | `[-6.0, 0.0]` | Clamped; non-finite -> `0.0` |
| `mix_pct` | `F32` | `100.0` | `[0.0, 100.0]` | Clamped; non-finite -> `100.0` |
| `gain_match` | `Bool` | `false` | `true` \| `false` | Wrong type -> `false` |

### 3. `TransientTelemetryState` (`MeterTelemetry`)

Runtime-only atomic telemetry (never serialized into presets):

| Field | Type | Behavior on Preset Load / Switch (`FR-008`) |
| --- | --- | --- |
| `current_gr_db` | `AtomicU32` | Reset to `0.0` (`clear_gain_reduction()`) |
| `max_gr_db` | `AtomicU32` | Reset to `0.0` (`reset_peaks()`) |
| `clip_latched` | `AtomicBool` | Reset to `false` (`reset_peaks()`) |
| `in_peak_l/r`, `out_peak_l/r`, `gain_match_db` | `AtomicU32` | Runtime audio-block decay / DSP state (`dsp.reset()` clears `gm_gain_db`) |

### 4. Host Wrapper Sync State

- **CLAP (`ClapGainReductionBox`)**:
  - `host: *const clap_host`
  - `inner_state: AtomicPtr<ClapPluginState>` (cached on `CLAP_EXT_STATE` lookup)
  - `rescanning_state: AtomicBool` (re-entrancy guard around `clap_host_params::rescan(CLAP_PARAM_RESCAN_VALUES)`)
- **VST3 (`Vst3GainReductionProxy`)**:
  - `component_handler: Mutex<Option<ComponentHandlerPtr>>` (main/UI-thread COM pointer captured in `set_component_handler`, released in `terminate` and `Drop`)
  - `restarting_params: AtomicBool` (re-entrancy guard around `IComponentHandler::restart_component(RestartFlags::kParamValuesChanged as i32)`)

## State Transitions on Preset Recall

```text
Host triggers state load (VST3 IComponent::set_state / CLAP clap_plugin_state::load)
  │
  ├─► Malformed binary/JSON stream ──► Wrapper returns error (`kResultFalse` / `false`); existing state & audio unaffected
  │
  └─► Valid JSON PluginState
        │
        ├─► `OmorePunch::filter_state(&mut state)` (`sanitize_plugin_state`)
        │     - Fill missing params with defaults (backward compatibility)
        │     - Clamp out-of-range floats, replace NaN/Inf with defaults
        │     - Normalize/validate enum values, strip unknown keys & legacy `editor-state`
        │
        ├─► `nih_plug` `deserialize_object` updates all 16 params & resets smoothers
        │
        ├─► If active (`buffer_config.is_some()`): `initialize()` + `reset()` resets `StereoPunchDsp`
        │
        ├─► Proxy post-load hook (active OR inactive):
        │     - `telemetry.clear_gain_reduction()` + `telemetry.reset_peaks()` (`max_gr_db = 0.0`, `clip_latched = false`)
        │     - Notify host parameter values changed (CLAP `rescan(CLAP_PARAM_RESCAN_VALUES)` / VST3 `restart_component(kParamValuesChanged)`)
        │
        └─► `egui` UI reads `params.*.value()` on next frame (`60 FPS`) -> immediate visual sync
```
