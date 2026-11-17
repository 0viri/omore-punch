# Implementation Plan: Reaper Preset Support

**Branch**: `feat/008-reaper-presets` | **Date**: 2026-11-15 | **Spec**: [spec.md](file:///home/tdavio/src/omore-punch/.worktrees/008-reaper-presets/specs/008-reaper-presets/spec.md)

**Input**: Feature specification from `/specs/008-reaper-presets/spec.md`

## Summary

Make Reaper user presets (save, recall, switch during playback, reset to default, project reload, `.rpl`/`.vstpreset` export/import across Linux and Windows within each plugin format) work reliably in both VST3 and CLAP builds of Omore Punch.

The technical approach stays self-contained inside `src/lib.rs` (zero `nih_plug` fork and zero `Cargo.toml` dependency changes):
1. Implement `Plugin::filter_state` (`sanitize_plugin_state`) on `OmorePunch` to guarantee 100% backward compatibility for legacy/partial presets (filling omitted parameters with defaults), clamp out-of-range/non-finite float values, validate enum values, and strip unknown keys / legacy `"editor-state"`.
2. Wrap `CLAP_EXT_STATE` (`clap_plugin_state::load`) in `mod clap` (`ClapGainReductionBox`) so successful state loads immediately reset `MeterTelemetry` (`Max GR` & `CLIP`) even when inactive and notify Reaper via `clap_host_params::rescan(CLAP_PARAM_RESCAN_VALUES)`.
3. Intercept `IComponent::set_state` in `mod vst3` (`Vst3GainReductionProxy`) using a captured `IComponentHandler` reference so successful state loads immediately reset `MeterTelemetry` even when inactive and notify Reaper via `IComponentHandler::restart_component(RestartFlags::kParamValuesChanged)`.
4. Add comprehensive unit tests covering state sanitization, live playback preset switching, and real VST3/CLAP wrapper state round-trips + host callbacks.

## Technical Context

**Language/Version**: Rust (Edition 2021)

**Primary Dependencies**: `nih_plug` (rev `de42101`, with `assert_process_allocs`), `nih_plug_egui`, `atomic_float` 1.1

**Storage**: Host-managed plugin state chunks (UTF-8 JSON `PluginState` via VST3 `IBStream` and CLAP `clap_istream`/`clap_ostream`)

**Testing**: `cargo test --release` (Rust unit tests in `src/lib.rs::tests`, including mock VST3 `IBStream`/`IComponentHandler` and mock CLAP `clap_host`/`clap_istream`/`clap_ostream` harnesses) + manual Reaper validation

**Target Platform**: Reaper on Linux x86_64 (Debian 13) and Windows 11 x86_64; VST3 and CLAP formats

**Project Type**: Audio DSP Plugin (`cdylib` + `lib` bundled into `.vst3` and `.clap` via `xtask`)

**Performance Goals**: Real-time safe audio thread (`process()` lock-free and allocation-free); UI parameter sync within one 60 FPS `egui` refresh frame

**Constraints**: Monolithic `src/lib.rs`; 100% pure ASCII in `src/lib.rs`; zero heap allocations/locks in `process()`/`StereoPunchDsp`; zero breaking changes to existing parameter IDs, VST3 Class ID, or CLAP ID

**Scale/Scope**: 16 parameters in `OmorePunchParams`, 2 plugin wrapper layers (`mod clap`, `mod vst3`), single-file scope (`src/lib.rs`)

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Pre-Design Status | Post-Design Status | Notes |
| --- | --- | --- | --- |
| I. Monolithic Single-File Architecture | PASS | PASS | All logic (`sanitize_plugin_state`, `OmorePunch::filter_state`, `mod clap` state wrapper, `mod vst3` state/handler hook, unit tests) stays strictly in `src/lib.rs` in its designated blocks. |
| II. Pure ASCII Source | PASS | PASS | All code, comments, and test strings in `src/lib.rs` remain pure ASCII (`LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs`). |
| III. Real-Time Audio Thread Safety | PASS | PASS | `process()` and `StereoPunchDsp` are untouched. `OmorePunch::initialize()`/`reset()` remain allocation-free and lock-free. `Vst3GainReductionProxy`'s `Mutex` is accessed only on main/UI COM lifecycle methods, never in `process()`. |
| IV. Signal Sanitization & Safety Clamp | PASS | PASS | `sanitize_sample()` on audio output remains intact; `sanitize_plugin_state` additionally blocks `NaN`/`Inf`/out-of-range values at state deserialization time. |
| V. Mandatory DSP Unit Tests | PASS | PASS | Existing DSP tests stay intact; new tests cover state sanitization, live playback preset switching stability, and VST3/CLAP state round-trips + telemetry reset. |
| VI. Single-Source Versioning | PASS | PASS | Continues reading `env!("CARGO_PKG_VERSION")`; no hard-coded version strings in `src/lib.rs`. |

## Project Structure

### Documentation (this feature)

```text
specs/008-reaper-presets/
├── plan.md                              # This file
├── research.md                          # Phase 0 root-cause & design decisions
├── data-model.md                        # Phase 1 state schema & transition model
├── quickstart.md                        # Phase 1 automated & Reaper validation guide
└── contracts/
    └── preset-state-contract.md         # Host/plugin preset & sync contracts
```

### Source Code (repository root)

```text
src/
└── lib.rs
    ├── Block 1 (StereoPunchDsp)         # Unchanged (already real-time safe & resettable)
    ├── Block 2 (MeterTelemetry)         # Unchanged (clear_gain_reduction / reset_peaks reused)
    ├── Block 3 (OmorePunchParams / OmorePunch)
    │   ├── sanitize_plugin_state()      # New pure state validator/migrator helper
    │   ├── Plugin::filter_state()       # Calls sanitize_plugin_state(&mut state)
    │   ├── mod clap (ClapGainReductionBox)
    │   │   └── CLAP_EXT_STATE wrapper   # Telemetry reset + clap_host_params::rescan(VALUES)
    │   └── mod vst3 (Vst3GainReductionProxy)
    │       └── IComponent::set_state    # Telemetry reset + restart_component(kParamValuesChanged)
    ├── Block 4 (draw_omore_punch_ui)    # Unchanged (reads params atomics every 60 FPS frame)
    └── mod tests                        # New unit tests for filter_state, CLAP/VST3 state & live switching
```

**Structure Decision**: Single monolithic `src/lib.rs` following Constitution Principle I; no new crates, modules, or Cargo dependencies are required.

## Complexity Tracking

No Constitution violations; table omitted.
