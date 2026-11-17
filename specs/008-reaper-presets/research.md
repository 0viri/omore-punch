# Research: Reaper Preset Support

**Feature**: `008-reaper-presets` | **Date**: 2026-11-15 | **Plan**: [plan.md](file:///home/tdavio/src/omore-punch/.worktrees/008-reaper-presets/specs/008-reaper-presets/plan.md)

## Research Findings & Root-Cause Analysis

Inspection of `src/lib.rs` and the pinned `nih_plug` wrappers (`nih-plug` commit `de42101`, `src/wrapper/state.rs`, `src/wrapper/vst3/{wrapper.rs,inner.rs}`, `src/wrapper/clap/wrapper.rs`, `src/params/float.rs`) identified four concrete gaps preventing reliable Reaper preset behavior across VST3 and CLAP:

### 1. Missing Host Parameter Resynchronization on State Load (CLAP & VST3)

- **Decision**: Intercept `CLAP_EXT_STATE` (`clap_plugin_state::load`) in `mod clap` (`ClapGainReductionBox`) and `IComponent::set_state` in `mod vst3` (`Vst3GainReductionProxy`) to notify Reaper immediately after successful state restoration, using re-entrancy guards (`AtomicBool`).
  - **CLAP**: After `inner_state.load(plugin, stream)` returns `true`, query `clap_host_params` (`CLAP_EXT_PARAMS`) from `clap_host::get_extension` and invoke `rescan(host, CLAP_PARAM_RESCAN_VALUES)` (flag `1 << 0`).
  - **VST3**: Store an upgraded `vst3_sys::VstPtr<dyn IComponentHandler>` in `Vst3GainReductionProxy::set_component_handler` (released in `terminate()` / `Drop`), and after `<Vst3Wrapper<OmorePunch> as IComponent>::set_state` returns `kResultOk`, release the mutex guard and invoke `handler.restart_component(RestartFlags::kParamValuesChanged as i32)`.
- **Rationale**:
  - In `nih_plug`, `ClapWrapper::ext_state_load` updates parameters and posts `Task::ParameterValuesChanged` (which only alerts the plugin's own editor), but never calls `clap_host_params::rescan(CLAP_PARAM_RESCAN_VALUES)` (unlike `set_state_object_from_gui` which posts `Task::RescanParamValues`).
  - Similarly, `Vst3Wrapper::set_state` calls `set_state_inner` which posts `Task::ParameterValuesChanged` to the editor, but never calls `IComponentHandler::restart_component(RestartFlags::kParamValuesChanged)` (unlike `set_state_object_from_gui`).
  - Without these host notifications, Reaper's host parameter cache, generic parameter UI, automation readouts, and preset dirty state can remain out of sync with the restored plugin state.
- **Alternatives considered**:
  - Forking/patching `nih_plug`: Rejected to avoid diverging from upstream `Cargo.lock` and adding external repo maintenance; both `mod clap` and `mod vst3` already wrap `ClapWrapper` and `Vst3Wrapper` cleanly in `src/lib.rs`.
  - Calling `restart_component` inside `IEditController::set_component_state`: Rejected because `set_component_state` is a no-op in `Vst3Wrapper` and may or may not follow `IComponent::set_state` depending on host sequence; notifying once after `IComponent::set_state` succeeds (with an `AtomicBool` re-entrancy guard) handles all VST3 state loads including `.rpl` and `.vstpreset`.

### 2. Incomplete State Validation, Missing-Parameter Fallback & Out-of-Range Clamping

- **Decision**: Implement `Plugin::filter_state(state: &mut PluginState)` on `OmorePunch` backed by a pure, unit-testable `sanitize_plugin_state(state: &mut PluginState)` helper in `src/lib.rs`.
- **Rationale**:
  - `nih_plug`'s `state::deserialize_object` (`src/wrapper/state.rs:184-232`) only iterates over keys present in `state.params`. If a partial or legacy preset (for example from v0.1.0/v0.2.0 before Drum Bus EQ, Auto-Release, or Gain Match existed) omits parameters, those parameters retain stale values from the previously loaded preset instead of falling back to defaults.
  - `FloatParam::set_plain_value` (`src/params/float.rs:208`) stores the incoming `plain` value directly without clamping (`let unmodulated_value = plain;`), allowing out-of-range or `NaN`/`Inf` values in malformed/edited presets to corrupt UI knob angles and normalized values.
  - Unknown parameter keys or unrecognized enum variant IDs trigger `nih_debug_assert_failure!` in debug builds (`assert_process_allocs` / debug runs).
  - `sanitize_plugin_state` normalizes every state object before `nih_plug` applies it:
    1. Ensures all 16 canonical parameter IDs exist (missing or wrong-variant entries are replaced with their documented defaults from `OmorePunchParams::default()`).
    2. Replaces non-finite floats with defaults and clamps all `FloatParam` values to `[min, max]`.
    3. Validates `ReleaseMode` and `ClipperMode` string IDs / variant indices (`0..=3` and `0..=2`), resetting invalid values to `"manual"` and `"soft_clip"`.
    4. Removes unknown parameter keys and strips legacy `"editor-state"` from `state.fields` while preserving backward compatibility for all valid legacy parameters.
- **Alternatives considered**:
  - Custom binary state serialization replacing `nih_plug`'s `PluginState` JSON: Rejected because it would break 100% backward compatibility with existing Reaper `.rpp` projects and `.rpl`/`.vstpreset` chunks saved by v0.1.x–v0.3.x (`FR-006`).

### 3. Transient Telemetry Latch Reset on Inactive Preset Loads

- **Decision**: Call `telemetry.clear_gain_reduction()` and `telemetry.reset_peaks()` unconditionally in both `ClapGainReductionBox` (`clap_state_load` on `true`) and `Vst3GainReductionProxy` (`IComponent::set_state` on `kResultOk`), in addition to `OmorePunch::initialize()`.
- **Rationale**:
  - `nih_plug`'s `set_state_inner` only calls `Plugin::initialize()` and `Plugin::reset()` when `current_buffer_config` is `Some(...)` (i.e., after audio activation). When a user loads or switches a preset while the plugin/track is inactive or before first playback, `OmorePunch::initialize()` is skipped, leaving stale `max_gr_db` and `clip_latched` values visible (`FR-008`).
- **Alternatives considered**:
  - Resetting telemetry inside `OmorePunch::filter_state`: Not possible because `filter_state` is a static associated function (`fn filter_state(state: &mut PluginState)`) with no `&self` access to `MeterTelemetry`.

### 4. Cross-OS Same-Format Portability & Stream Format

- **Decision**: Preserve `nih_plug`'s uncompressed UTF-8 JSON `PluginState` chunk format (and CLAP's 8-byte little-endian length prefix) without enabling `zstd` or OS-specific fields.
- **Rationale**:
  - `Cargo.toml` does not enable `zstd`, so both Linux x86_64 and Windows x86_64 emit identical UTF-8 JSON payloads with identical parameter IDs (`#[id = "..."]`), enum variant IDs (`#[id = "..."]`), `VST3_CLASS_ID = *b"OmorePunchBusV01"`, and `CLAP_ID = "local.omore-audio.omore-punch"`.
  - Reaper `.rpl` libraries and `.vstpreset` files embed these format-specific chunks directly and therefore round-trip identically between Linux and Windows within VST3 and within CLAP (`FR-002`, `SC-003`).
- **Alternatives considered**:
  - Custom cross-format `.rpl` converter: Out of scope per clarified `FR-002`.

### 5. Real-Time Safety & UI Refresh

- **Decision**: Keep `StereoPunchDsp`, `OmorePunch::process()`, `OmorePunch::initialize()`, and `OmorePunch::reset()` strictly allocation-free and lock-free; rely on existing 60 FPS `egui_ctx.request_repaint()` reading `OmorePunchParams` atomics directly.
- **Rationale**:
  - `OmorePunch::initialize()` and `OmorePunch::reset()` reset `StereoPunchDsp` states and `MeterTelemetry` atomics with zero heap allocations, so `set_state_inner`'s `process_wrapper(|| plugin.reset())` never trips `assert_process_allocs`.
  - `draw_omore_punch_ui` reads `params.*.value()` every frame without local shadow caches, so open and newly opened editor windows immediately display recalled presets (`FR-003`, `SC-002`).
  - `Vst3GainReductionProxy`'s `Mutex<Option<ComponentHandlerPtr>>` is only accessed on the main/UI thread (`set_component_handler`, `set_state`, `terminate`, `Drop`) and never from `process()`.
