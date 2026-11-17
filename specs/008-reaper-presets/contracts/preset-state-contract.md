# Contract: Host Preset State & Synchronization Protocol

**Feature**: `008-reaper-presets` | **Date**: 2026-11-15

This contract defines the external behavioral guarantees between **Omore Punch** and **Reaper** (for both VST3 and CLAP on Linux x86_64 and Windows x86_64).

## 1. Serialized Preset Payload Contract

- **Format**: UTF-8 JSON `PluginState` (`version`, `params`, `fields`), identical across Linux and Windows.
- **Plugin Identifiers** (unchanged for 100% backward compatibility):
  - **VST3**: `VST3_CLASS_ID = *b"OmorePunchBusV01"`
  - **CLAP**: `CLAP_ID = "local.omore-audio.omore-punch"`
- **Parameter Completeness**: All 16 parameter IDs in `OmorePunchParams` are serialized with unnormalized plain values (floats, booleans, and stable enum variant string IDs).
- **Forward / Backward Compatibility (`FR-006`)**:
  - Any valid `PluginState` saved by earlier versions of Omore Punch MUST deserialize without error; parameters absent from legacy payloads MUST assume their documented default values rather than retaining stale values from a prior preset.
  - Any out-of-range float MUST be clamped to `[min, max]`; any non-finite float MUST fall back to default.
  - Any unknown parameter key or invalid enum/boolean value MUST be stripped or reset to default without asserting/panicking in debug or release builds.
  - Legacy `"editor-state"` entries in `fields` MUST NOT alter the fixed `820x650` editor window size.

## 2. CLAP Host Protocol Contract (`CLAP_EXT_STATE` + `CLAP_EXT_PARAMS`)

- **`clap_plugin_state::save(plugin, stream)`**:
  - Writes 8-byte little-endian payload length followed by UTF-8 JSON `PluginState`; returns `true` on success.
- **`clap_plugin_state::load(plugin, stream)`**:
  - Delegates deserialization + `OmorePunch::filter_state` to `ClapWrapper`.
  - Returns `false` cleanly on null pointers, truncated streams, or invalid JSON without mutating audio safety.
  - On `true`:
    1. Immediately resets `MeterTelemetry` (`current_gr_db = 0.0`, `max_gr_db = 0.0`, `clip_latched = false`) regardless of whether audio processing is active.
    2. Queries `clap_host_params` (`"clap.params"`) from `clap_host` and calls `rescan(host, CLAP_PARAM_RESCAN_VALUES)` (guarded against re-entrancy and null function pointers).
- **Reset to Default**: `clap_plugin_params::get_info` exposes documented `default_value` for all 16 parameters so Reaper's parameter reset restores exact defaults.

## 3. VST3 Host Protocol Contract (`IComponent` + `IEditController`)

- **`IComponent::get_state(stream)`**:
  - Writes UTF-8 JSON `PluginState` to `IBStream`; returns `kResultOk` on success.
- **`IComponent::set_state(stream)`**:
  - Delegates deserialization + `OmorePunch::filter_state` to `Vst3Wrapper`.
  - Returns `kResultFalse` cleanly on null/empty/corrupt streams without crashing or producing non-finite audio.
  - On `kResultOk`:
    1. Immediately resets `MeterTelemetry` (`current_gr_db = 0.0`, `max_gr_db = 0.0`, `clip_latched = false`) regardless of whether `set_active(true)` / `setup_processing` has occurred.
    2. If `IComponentHandler` was registered via `IEditController::set_component_handler`, drops any internal mutex lock first and invokes `restart_component(RestartFlags::kParamValuesChanged as i32)` (guarded against re-entrancy).
- **`IPluginBase::terminate()` / `Drop`**:
  - Releases the stored `IComponentHandler` COM reference cleanly to avoid reference leaks or dangling callbacks.
- **Reset to Default**: `IEditController::get_parameter_info` exposes accurate `default_normalized_value` for all 16 parameters.

## 4. Real-Time & UI Invariants

- Audio thread (`process()`, `StereoPunchDsp`) performs zero heap allocations, zero locks, and always sanitizes outputs to finite values in `[-4.0, 4.0]`.
- `egui` UI reads `OmorePunchParams` atomics directly on each frame (`60 FPS`), requiring no extra editor reopen when presets change while open or closed (`FR-003`, `SC-002`).
