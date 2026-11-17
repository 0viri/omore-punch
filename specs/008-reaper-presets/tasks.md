# Tasks: Reaper Preset Support

**Input**: Design documents from `/specs/008-reaper-presets/`
**Prerequisites**: `plan.md` (required), `spec.md` (required), `research.md`, `data-model.md`, `contracts/preset-state-contract.md`, `quickstart.md`

**Tests**: Unit tests are mandatory per Omore Punch Constitution Principle V and are included for each user story and foundational state sanitization.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing. Because all plugin code lives in the single monolithic file `src/lib.rs` (Constitution Principle I), edits to `src/lib.rs` are ordered sequentially without false `[P]` markers.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files or read-only checks, no dependencies on incomplete tasks)
- **[Story]**: Which user story this task belongs to (`[US1]`, `[US2]`, `[US3]`)
- Exact file paths are included in every task description.

---

## Phase 1: Setup (Shared Baseline)

**Purpose**: Verify the feature worktree builds and passes baseline quality gates before modifying `src/lib.rs`.

- [X] T001 Verify baseline `cargo test --release` and pure ASCII check (`LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs`) in `/home/tdavio/src/omore-punch/.worktrees/008-reaper-presets`

---

## Phase 2: Foundational (State Sanitization & Backward Compatibility)

**Purpose**: Ensure every `PluginState` loaded by Reaper (from `.rpp`, `.rpl`, `.vstpreset`, or CLAP state) is sanitized, bounded, and 100% backward-compatible with earlier Omore Punch versions before `nih_plug` applies parameters.

**⚠️ CRITICAL**: Must be complete before User Story 1–3 wrapper integration tests can pass.

- [X] T002 Implement pure `sanitize_plugin_state(state: &mut PluginState)` helper in Block 3 of `src/lib.rs` enforcing all 16 canonical parameter defaults and bounds (`input_db` `[-12.0, 12.0]` default `0.0`, `threshold_db` `[-40.0, 0.0]` default `0.0`, `ratio` `[1.0, 20.0]` default `4.0`, `attack_ms` `[0.1, 100.0]` default `3.0`, `release_ms` `[10.0, 500.0]` default `80.0`, `release_mode` `"manual"|"auto_tight"|"auto_punch"|"auto_glue"` or `0..=3` normalized to `"manual"`, `sc_hpf_hz` `[20.0, 250.0]` default `20.0`, `auto_makeup` `Bool` default `true`, `makeup_db` `[-12.0, 24.0]` default `0.0`, `eq_low_cut_hz` `[20.0, 160.0]` default `20.0`, `eq_mid_cut_db` `[-12.0, 0.0]` default `0.0`, `eq_high_shelf_db` `[-6.0, 12.0]` default `0.0`, `clipper_mode` `"soft_clip"|"hard_clip"|"bypass_clip"` or `0..=2` normalized to `"soft_clip"`, `ceiling_db` `[-6.0, 0.0]` default `0.0`, `mix_pct` `[0.0, 100.0]` default `100.0`, `gain_match` `Bool` default `false`), replacing non-finite floats with defaults, stripping unknown keys, and removing legacy `"editor-state"` from `state.fields`
- [X] T003 Implement `fn filter_state(state: &mut PluginState)` inside `impl Plugin for OmorePunch` in `src/lib.rs` delegating to `sanitize_plugin_state(state)`
- [X] T004 Add unit tests `test_filter_state_fills_missing_legacy_params`, `test_filter_state_clamps_out_of_range_and_nan`, and `test_filter_state_normalizes_enums_and_strips_unknowns` in `src/lib.rs` (`mod tests`)

**Checkpoint**: `OmorePunch::filter_state` guarantees safe, backward-compatible parameter restoration for all 16 parameters.

---

## Phase 3: User Story 1 - Save and Recall User Presets in Reaper (Priority: P1) 🎯 MVP

**Goal**: Enable reliable saving and recalling of user presets (and project chunks) via Reaper's FX preset menu in both CLAP and VST3 formats with full host parameter synchronization.

**Independent Test**: Save a custom 16-parameter state via CLAP `clap_plugin_state::save` and VST3 `IComponent::get_state`, mutate parameters, restore via `load` / `set_state`, and verify all 16 parameters and host resync notifications (`CLAP_PARAM_RESCAN_VALUES` / `RestartFlags::kParamValuesChanged`) fire cleanly.

- [X] T005 [US1] Add `ClapPluginState` and `ClapHostParams` C-ABI definitions plus `host: *const clap_host`, `inner_state: AtomicPtr<ClapPluginState>`, and `rescanning_state: AtomicBool` to `ClapGainReductionBox` in `mod clap` of `src/lib.rs`, wrapping `CLAP_EXT_STATE` so `clap_state_load` calls `clap_host_params::rescan(host, CLAP_PARAM_RESCAN_VALUES)` on `true`
- [X] T006 [US1] Add `ComponentHandlerPtr` (`VstPtr<dyn IComponentHandler>`), `component_handler: Mutex<Option<ComponentHandlerPtr>>`, and `restarting_params: AtomicBool` to `Vst3GainReductionProxy` in `mod vst3` of `src/lib.rs`, capturing the handler in `set_component_handler`, releasing it in `terminate`/`Drop`, and calling `restart_component(RestartFlags::kParamValuesChanged as i32)` (after dropping the mutex guard) when `IComponent::set_state` returns `kResultOk`
- [X] T007 [US1] Add CLAP state round-trip, host `rescan(CLAP_PARAM_RESCAN_VALUES)`, `clap_plugin_params::get_info` default-value consistency, and malformed-stream test `test_clap_wrapper_state_roundtrip_and_host_sync` in `src/lib.rs` (`mod tests`)
- [X] T008 [US1] Add VST3 COM `IComponent::{get_state, set_state}` round-trip, `IComponentHandler::restart_component(kParamValuesChanged)`, `IEditController::get_parameter_info` default-value / `set_param_normalized` reset consistency, and malformed-stream test `test_vst3_proxy_state_roundtrip_and_host_sync` in `src/lib.rs` (`mod tests`)

**Checkpoint**: User Story 1 is complete and independently testable for both CLAP and VST3.

---

## Phase 4: User Story 2 - Reset to Default and Switch Between Presets During Playback (Priority: P2)

**Goal**: Ensure live A/B preset switching during playback is glitch-free, resets `Max GR` peak hold and `CLIP` latch automatically on every preset load (active or inactive), and restores documented defaults cleanly.

**Independent Test**: Switch presets 10 times consecutively during active `OmorePunch` audio processing (verifying finite clamped output in `[-4.0, 4.0]`), verify `max_gr_db == 0.0` and `clip_latched == false` after CLAP/VST3 state load even when inactive, and verify default parameter restoration.

- [X] T009 [US2] Call `telemetry.clear_gain_reduction()` and `telemetry.reset_peaks()` unconditionally on successful `clap_state_load` in `mod clap` and `IComponent::set_state` in `mod vst3` of `src/lib.rs` (covering inactive instances where `initialize()` is skipped)
- [X] T010 [US2] Extend `test_clap_wrapper_state_roundtrip_and_host_sync` and `test_vst3_proxy_state_roundtrip_and_host_sync` in `src/lib.rs` (`mod tests`) to assert `max_gr_db == 0.0` and `clip_latched == false` after inactive preset load (`FR-008`)
- [X] T011 [US2] Add `test_live_preset_switching_stability` in `src/lib.rs` (`mod tests`) exercising 10 consecutive preset switches during active stereo drum-bus processing (applying each sanitized `PluginState` to `OmorePunchParams` with smoother reset + `initialize` + `reset` before each `process()` block) and reset-to-default restoration with finite output in `[-4.0, 4.0]`

**Checkpoint**: User Stories 1 and 2 both pass independently.

---

## Phase 5: User Story 3 - Import, Export, and Cross-Platform Preset Compatibility (Priority: P3)

**Goal**: Guarantee deterministic, OS-independent UTF-8 JSON `PluginState` chunks for same-format `.rpl` / `.vstpreset` portability between Linux and Windows.

**Independent Test**: Verify serialized state chunks contain only canonical UTF-8 JSON parameters without OS-specific fields (`editor-state` absent) and round-trip legacy v0.2/v0.3 payloads identically.

- [X] T012 [US3] Add `test_preset_json_portability_and_backward_compat` in `src/lib.rs` (`mod tests`) verifying deterministic UTF-8 JSON state serialization, absence of `editor-state` in serialized `fields`, and exact round-trip of all 16 parameters plus legacy v0.2/v0.3 payloads

**Checkpoint**: All 3 user stories are independently verified by unit tests.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Changelog update, CI quality gates, bundle generation, and manual Reaper verification.

- [X] T013 [P] Add an entry under `## [Unreleased]` in `CHANGELOG.md` documenting Reaper preset support (VST3/CLAP host parameter resync, backward-compatible state sanitization, and telemetry latch reset)
- [X] T014 Run pure ASCII gate (`LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs`), `cargo test --release`, and `cargo run --package xtask --release -- bundle omore-punch --release` in `/home/tdavio/src/omore-punch/.worktrees/008-reaper-presets`
- [X] T015 Validate end-to-end preset workflows in Reaper (VST3 & CLAP) per `specs/008-reaper-presets/quickstart.md`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — starts immediately.
- **Phase 2 (Foundational)**: Depends on T001 — BLOCKS Phases 3–5.
- **Phase 3 (US1 — MVP)**: Depends on Phase 2 (`filter_state` active before testing wrapper round-trips).
- **Phase 4 (US2)**: Depends on Phase 3 (`ClapGainReductionBox` & `Vst3GainReductionProxy` state hooks in place).
- **Phase 5 (US3)**: Depends on Phase 2 (can run after Phase 2 or in sequence after Phase 4 in `src/lib.rs`).
- **Phase 6 (Polish)**: Depends on Phases 3–5 completion (`T013` in `CHANGELOG.md` can run in parallel `[P]` with `src/lib.rs` test tasks).

### Parallel Opportunities

- `T013` (`CHANGELOG.md`) can be edited in parallel with `src/lib.rs` tasks.
- Within `src/lib.rs`, tasks are executed sequentially to avoid merge conflicts in the monolithic source file.

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1 (`T001`) + Phase 2 (`T002`–`T004`).
2. Complete Phase 3 (`T005`–`T008`) to deliver working Reaper preset save/recall in both CLAP and VST3.
3. Validate independently via `cargo test --release`.

### Incremental Delivery

1. Phase 2 -> Backward-compatible state sanitization ready.
2. Phase 3 (US1) -> Reaper save/recall/project persistence working (MVP!).
3. Phase 4 (US2) -> Live playback switching, inactive telemetry latch reset, and default reset verified.
4. Phase 5 (US3) -> Cross-OS JSON portability verified.
5. Phase 6 -> `CHANGELOG.md`, ASCII gate, `cargo test --release`, `xtask bundle`, and Reaper quickstart check.
