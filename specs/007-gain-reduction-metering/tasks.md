# Tasks: DAW Gain Reduction Metering

**Input**: Design documents from `specs/007-gain-reduction-metering/`
**Prerequisites**: `plan.md` (required), `spec.md` (required for user stories), `research.md`, `data-model.md`, `contracts/host-gr-metering-abi.md`, `quickstart.md`

**Tests**: Inclus et obligatoires selon le Principe V de la Constitution (`cargo test --release` et vérification numérique dans `src/lib.rs`).

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story. Because Omore Punch follows a strict single-file architecture (`src/lib.rs`, Principe I), tasks editing `src/lib.rs` execute sequentially while documentation updates (`CHANGELOG.md`) can run in parallel.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup (Shared Verification Baseline)

**Purpose**: Confirm clean baseline before modifying the monolithic plugin file

- [X] T001 Verify baseline build, existing DSP unit tests (`cargo test --release`), and strict ASCII compliance (`LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs`) in `src/lib.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core lock-free host GR projection and per-instance telemetry capture shared by all user stories

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [X] T002 Add `MeterTelemetry::host_gain_reduction_db(&self) -> f64` (atomic `Relaxed` load of `current_gr_db`, returning exact `0.0` if non-finite or `<= 0.0`, otherwise clamped to `[-24.0, 0.0]`) and `MeterTelemetry::clear_gain_reduction(&self)` in `src/lib.rs`
- [X] T003 Update `OmorePunch::current_gain_adjustment_db(&self) -> f64` to delegate to `self.telemetry.host_gain_reduction_db()` and record each new instance's `Arc<MeterTelemetry>` into a `thread_local!` slot in `OmorePunch::default()` for multi-instance safe wrapper binding in `src/lib.rs`

**Checkpoint**: Foundation ready — user story implementation can now begin

---

## Phase 3: User Story 1 - Affichage temps réel de la réduction de gain sur la tranche de console du DAW (Priority: P1) 🎯 MVP

**Goal**: Expose instantaneous compressor gain reduction (`0.0 dB` to `-24.0 dB`) to the host DAW via CLAP `clap.gain-adjustment-metering/0` and VST3 `PreSonus::IGainReductionInfo`, matching the internal meter within 1 dB even when the plugin editor window is closed and regardless of makeup gain or clipper activity.

**Independent Test**: Build VST3 and CLAP bundles, verify ABI contract unit tests (`clap.gain-adjustment-metering/0` lookup + VST3 `query_interface` for `IGainReductionInfo`), and validate the yellow downward GR meter on Reaper MCP/TCP track strips.

### Implementation & Tests for User Story 1

- [X] T004 [US1] Replace `nih_export_clap!(OmorePunch);` with a local CLAP export module (`ClapPluginGainAdjustmentMetering`, `#[repr(C)] ClapGainReductionBox`, `clap_entry`, and `get_extension` serving `"clap.gain-adjustment-metering/0"` with lock-free `get` reading `telemetry.host_gain_reduction_db()`) in `src/lib.rs`
- [X] T005 [US1] Replace `nih_export_vst3!(OmorePunch);` with a local VST3 export module defining `#[com_interface("8E3C292C-9592-4F9D-B259-0B1E100E4198")] IGainReductionInfo`, `#[VST3(implements(IComponent, IEditController, IAudioProcessor, IMidiMapping, INoteExpressionController, IProcessContextRequirements, IUnitInfo, IGainReductionInfo))] Vst3GainReductionProxy`, `Factory`, and `GetPluginFactory`/`ModuleEntry`/`InitDll` entry points in `src/lib.rs`
- [X] T006 [US1] Add unit tests validating CLAP `clap.gain-adjustment-metering/0` extension lookup, multi-instance isolation, VST3 `query_interface` for `IID_IGAIN_REDUCTION_INFO` from `IComponent`/`IEditController` returning `[-24.0, 0.0] dB`, and live host GR updates while `OmorePunch::editor()` is never instantiated (closed UI) in `src/lib.rs`
- [X] T007 [US1] Add DSP unit test verifying exact alignment (`< 0.01 dB` delta) between `host_gain_reduction_db()` and internal `env_gr_db`, `-24.0 dB` clamping on extreme peaks, and independence from makeup gain and output clipper in `src/lib.rs`

**Checkpoint**: User Story 1 (MVP) is complete and independently testable across both VST3 and CLAP formats

---

## Phase 4: User Story 2 - Isolation totale vis-à-vis de la liste des paramètres et de l'automation (Priority: P2)

**Goal**: Remove `gr_meter_db` from exposed plugin parameters so zero gain reduction metering parameters appear in DAW control lists or automation lanes.

**Independent Test**: Inspect `OmorePunchParams` parameter map, CLAP `clap_plugin_params::count`, and VST3 `IEditController::getParameterCount` to confirm `gr_meter_db` is absent.

### Implementation & Tests for User Story 2

- [X] T008 [US2] Remove `gr_meter_db: FloatParam` (`#[id = "gr_meter_db"]`) from `OmorePunchParams` and its initialization in `OmorePunchParams::default()` in `src/lib.rs`
- [X] T009 [US2] Remove the UI parameter synchronization (`setter.set_parameter(&params.gr_meter_db, ...)`) from `draw_omore_punch_ui` in `src/lib.rs`
- [X] T010 [US2] Add unit test confirming no `"gr_meter_db"` or `"Gain Reduction (dB)"` parameter is exposed in `OmorePunchParams`, CLAP `ext_params_count`, or VST3 `get_parameter_count` in `src/lib.rs`

**Checkpoint**: User Stories 1 and 2 work independently with a clean DAW parameter list

---

## Phase 5: User Story 3 - Retour à zéro au repos, en Bypass et cohérence en mix parallèle (Priority: P2)

**Goal**: Guarantee clean return to `0.0 dB` during silence, below-threshold signal, Bypass/reset/host deactivation, raw detector GR parity when `mix_pct < 100%`, and consistent response across `44.1 kHz` to `192 kHz` and variable buffer sizes.

**Independent Test**: Run unit tests for `reset()`, CLAP `stop_processing`/`deactivate`, VST3 `set_processing(false)`/`set_active(false)`, silence decay, `mix_pct` (`0%`, `50%`, `100%`), and sample rates `44100..192000 Hz` with buffer sizes `32..2048`.

### Implementation & Tests for User Story 3

- [X] T011 [US3] Call `self.telemetry.clear_gain_reduction()` inside `OmorePunch::reset(&mut self)` and `OmorePunch::initialize(...)`, keeping `MeterTelemetry::reset_peaks(&self)` scoped strictly to `max_gr_db` and `clip_latched` so UI `[Reset Peak]` never zeroes live `current_gr_db` in `src/lib.rs`
- [X] T012 [US3] Clear `telemetry.current_gr_db` to `0.0` in CLAP lifecycle hooks (`stop_processing`, `deactivate`, `reset`) and VST3 lifecycle hooks (`IAudioProcessor::set_processing(false)`, `IComponent::set_active(false)`) in `src/lib.rs`
- [X] T013 [US3] Add unit tests covering exact `0.0 dB` return on silence, `NaN`/`Inf`/`-0.0`, `reset()`, and CLAP/VST3 bypass/deactivation hooks, parity across `mix_pct` (`0%`, `50%`, `100%`), and consistency across `44100..192000 Hz` with buffer sizes `32..2048` in `src/lib.rs`

**Checkpoint**: All user stories are independently functional and verified

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, real-time safety audit, and CI Quality Gates

- [X] T014 [P] Add an entry under `## [Unreleased]` describing DAW Gain Reduction Metering (VST3 & CLAP) in `CHANGELOG.md`
- [X] T015 Audit `process()`, `host_gain_reduction_db()`, `clap_gain_adjustment_get()`, and `IGainReductionInfo::get_gain_reduction_value_in_db()` in `src/lib.rs` against Constitution Principles III & IV (zero heap allocation, zero locks, non-finite sanitization)
- [X] T016 Run full Quality Gates (`LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs`, `cargo test --release`, `cargo run --package xtask --release -- bundle omore-punch --release` verifying `target/bundled/omore-punch.vst3` and `target/bundled/omore-punch.clap`) and validate against `specs/007-gain-reduction-metering/quickstart.md`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately.
- **Foundational (Phase 2)**: Depends on Phase 1 — blocks all user stories.
- **User Story 1 (Phase 3)**: Depends on Phase 2 — delivers the core MVP (CLAP & VST3 DAW GR metering).
- **User Story 2 (Phase 4)**: Depends on Phase 2 (and logically follows US1 in `src/lib.rs` so host metering works before removing `gr_meter_db`).
- **User Story 3 (Phase 5)**: Depends on Phase 3 (extends the CLAP & VST3 wrappers with lifecycle reset hooks and full edge-case tests).
- **Polish (Phase 6)**: Depends on Phases 3, 4, and 5 (`T014` in `CHANGELOG.md` can run in parallel with `T015`).

### Parallel Opportunities

- Because all Rust plugin code lives in the single monolithic file `src/lib.rs` (Constitution Principle I), edits to `src/lib.rs` (`T002`–`T013`, `T015`) are executed sequentially.
- `T014` (`CHANGELOG.md`) modifies a separate file and is marked `[P]` to run in parallel with `T015` before the final gate verification `T016`.

---

## Parallel Example: Final Polish

```text
Parallel batch before T016:
- T014 [P] Add entry under ## [Unreleased] in CHANGELOG.md
- T015 Audit process() and metering callbacks in src/lib.rs
Then:
- T016 Run ASCII check, cargo test --release, and xtask bundle
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1 (Setup baseline check) and Phase 2 (Foundational `MeterTelemetry::host_gain_reduction_db` + thread-local instance binding).
2. Complete Phase 3 (User Story 1: CLAP `clap.gain-adjustment-metering/0` + VST3 `PreSonus::IGainReductionInfo` + ABI & DSP tests).
3. **STOP and VALIDATE**: Run `cargo test --release` and bundle VST3/CLAP to confirm Reaper displays the gain reduction meter.

### Incremental Delivery

1. Setup + Foundational → Lock-free GR readout and per-instance binding ready.
2. Add User Story 1 → Core CLAP/VST3 DAW GR metering operational (MVP).
3. Add User Story 2 → Remove `gr_meter_db` parameter so DAW control/automation lists remain clean.
4. Add User Story 3 → Lifecycle `0.0 dB` reset guarantees, `Mix < 100%` parity, and multi-sample-rate/buffer verification.
5. Polish → `CHANGELOG.md` update, RT safety audit, and full CI Quality Gates.
