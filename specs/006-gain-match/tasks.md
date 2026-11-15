# Tasks: Gain Match Automatique

**Input**: Design documents from `specs/006-gain-match/`
**Prerequisites**: `plan.md` (required), `spec.md` (required for user stories), `research.md`, `data-model.md`, `contracts/plugin-contract.md`, `quickstart.md`

**Tests**: Included per Constitution Principle V (mandatory numerical DSP unit tests and UI layout verification in `src/lib.rs`).

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story within the single-file plugin architecture (`src/lib.rs`).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story this task belongs to (`US1`, `US2`, `US3`)
- All plugin code targets `src/lib.rs` (Constitution Principle I); tasks modifying `src/lib.rs` execute sequentially to avoid file conflicts.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Verify baseline build, tests, and ASCII compliance in the `feat/006-gain-match` worktree before modifying plugin code.

- [X] T001 Verify clean baseline in `/home/tdavio/src/omore-punch/.worktrees/006-gain-match`: run `LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs` and `cargo test --release` in `src/lib.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Add core data-model fields, cached DSP coefficients, atomic telemetry, and host DAW parameter skeleton required by all user stories.

**CRITICAL**: No user story behavior can be verified until this phase is complete.

- [X] T002 Add `pub gain_match: bool` (default `false`) to `DspConfig` and `pub gain_match_db: f32` (default `0.0`) to `SampleOutput`, and update existing explicit `DspConfig` struct literals in `src/lib.rs`
- [X] T003 Add Gain Match constants (`GAIN_MATCH_RMS_MS = 400.0`, `GAIN_MATCH_PRESENCE_MS = 50.0`, `GAIN_MATCH_SLEW_MS = 150.0`, `GAIN_MATCH_TOGGLE_SLEW_MS = 35.0`, `GAIN_MATCH_NOISE_FLOOR_POWER = 1.0e-6`, `GAIN_MATCH_MIN_DB = -24.0`, `GAIN_MATCH_MAX_DB = 24.0`), cached coefficients (`gm_rms_coeff`, `gm_presence_coeff`, `gm_slew_coeff`, `gm_off_slew_coeff`), and state fields (`gm_in_ms`, `gm_post_ms`, `gm_presence_ms`, `pub gm_gain_db`) to `StereoPunchDsp`, computing coefficients only in `Default` / `set_sample_rate()` and clearing state in `StereoPunchDsp::reset()` in `src/lib.rs`
- [X] T004 Add `pub gain_match_db: AtomicU32` to `MeterTelemetry` and `#[id = "gain_match"] pub gain_match: BoolParam` (`"Gain Match"`, default `false`) to `OmorePunchParams`, and wire `cfg.gain_match` plus `telemetry.gain_match_db` in `OmorePunch::process()` in `src/lib.rs`

**Checkpoint**: Project compiles cleanly with `cargo test --release` and preserves 100% of existing behavior when `gain_match` is `false`.

---

## Phase 3: User Story 1 - Egalisation automatique du volume entree/sortie apres traitement (Priority: P1) 🎯 MVP

**Goal**: Measure stereo-linked raw input RMS vs post-Clipper/post-Mix RMS and apply an identical smoothed compensation gain (`[-24.0, +24.0]` dB) to both channels at the very end of the processing chain before `sanitize_sample()`.

**Independent Test**: Feed stereo test and drum-like signals through compression, Drum Bus EQ boost/cut, `Auto Makeup`/`Makeup Gain`, and Output Clipper with `gain_match: true`, verifying output RMS converges within `<= 0.5 dB` of raw input RMS, L/R stereo balance shift is `0.0 dB`, and trim stays bounded in `[-24.0, +24.0]` dB with final safety clamp `[-4.0, 4.0]`.

### Implementation & Tests for User Story 1

- [X] T005 [US1] Implement feed-forward stereo RMS integration (`gm_in_ms` on raw `(dry_l, dry_r)` before `input_db`, `gm_post_ms` on `(mixed_l, mixed_r)` post-Clipper/post-Mix), target calculation `10.0 * (gm_in_ms.max(1.0e-12) / gm_post_ms.max(1.0e-12)).log10()` clamped to `[-24.0, 24.0]`, active slew (`gm_slew_coeff`), identical L/R output scaling, post-trim `> 1.0` `clipped` flag evaluation, and `sanitize_sample()` in `StereoPunchDsp::process_sample()` in `src/lib.rs`
- [X] T006 [US1] Add numerical DSP unit tests in `mod tests` of `src/lib.rs` verifying: (1) RMS match `<= 0.5 dB` under compression, Drum Bus EQ boost/cut, and Soft/Hard Clipping, (2) identical L/R gain (`0.0 dB` balance shift), (3) `Auto Makeup` and `Makeup Gain` coexistence, and (4) `[-24.0, +24.0]` dB trim clamping + `[-4.0, 4.0]` safety clamp

**Checkpoint**: User Story 1 (core post-processing Gain Match DSP) is complete and verified by unit tests.

---

## Phase 4: User Story 2 - Activation/Desactivation fluide et affichage de la compensation appliquee (Priority: P2)

**Goal**: Ensure smooth click-free ON/OFF transitions returning to unity (`0.0 dB`) when disabled, and expose `[Gain Match: OFF]` / `[Gain Match: ON]` toggle plus live `Trim: +0.0 dB` readout in Section 4 (`4. GAIN REDUCTION & STEREO BUS METERING`) of the UI.

**Independent Test**: Toggle `gain_match` between `true` and `false` during continuous playback verifying smooth sample-to-sample gain transition without step clicks and exact `0.0 dB` settlement when `OFF`, and run the headless `egui` layout test verifying Section 4 renders without overlap inside `820x650` (with >= 6 px margin).

### Implementation & Tests for User Story 2

- [X] T007 [US2] Update `StereoPunchDsp::process_sample()` in `src/lib.rs` so when `cfg.gain_match == false`, `gm_in_ms` and `gm_post_ms` still track active signal (for seamless mid-playback activation) while `gm_gain_db` slews smoothly toward `0.0` via `gm_off_slew_coeff` and snaps to `0.0` when `gm_gain_db.abs() < 1.0e-5`
- [X] T008 [US2] Add `[Gain Match: OFF]` / `[Gain Match: ON]` toggle button (using `begin_set_parameter`/`set_parameter`/`end_set_parameter` on `&params.gain_match`) and live `Trim: {:+.1} dB` monospace ASCII readout (normalizing `abs() < 0.05` to `0.0` so `-0.0 dB` never displays) in Section 4 (`4. GAIN REDUCTION & STEREO BUS METERING`) of `draw_omore_punch_ui()` in `src/lib.rs`, adjusting `UI_METER_HEIGHT` (e.g. `384.0`) if needed so the right column stays within `UI_WINDOW_HEIGHT` (`650`)
- [X] T009 [US2] Add DSP unit tests for smooth ON/OFF toggle slewing (bounded sample-to-sample delta, return to `0.0 dB` when `OFF`) and extend headless `egui` layout tests (`assert_ui_fits` with `gain_match` `false` and `true`) in `mod tests` of `src/lib.rs`

**Checkpoint**: User Stories 1 and 2 work together; DAW automation, smooth toggle, and Section 4 UI rendering are verified.

---

## Phase 5: User Story 3 - Preservation des transitoires et comportement sur les silences (Priority: P3)

**Goal**: Protect Gain Match against background noise amplification and transient pumping by gating RMS and gain updates with a `50 ms` input presence detector (`-60 dBFS` = `1.0e-6` power) that holds (`Hold`) the last valid trim during silence.

**Independent Test**: Feed cold silence (`-70 dBFS`), active percussive drum bursts, and mid-stream breaks (`< -60 dBFS`) into `StereoPunchDsp` with `gain_match: true`, verifying `gm_gain_db` remains `0.0 dB` at cold start, holds its converged value during breaks without drifting toward `+24 dB`, and preserves percussive transient punch.

### Implementation & Tests for User Story 3

- [X] T010 [US3] Implement `50 ms` stereo mean-square presence detector (`gm_presence_ms`) with `NaN`/`Inf`/denormal guards and `-60.0 dBFS` (`1.0e-6`) threshold gating in `StereoPunchDsp::process_sample()` in `src/lib.rs` so `gm_in_ms`, `gm_post_ms`, and `gm_gain_db` freeze (`Hold`) whenever `gm_presence_ms < 1.0e-6`
- [X] T011 [US3] Add DSP unit tests in `mod tests` of `src/lib.rs` verifying: (1) cold-start silence/noise (`-70 dBFS`) keeps `gm_gain_db == 0.0`, (2) mid-stream silence below `-60 dBFS` holds the converged trim without drifting toward `+24.0 dB` or amplifying noise, and (3) percussive drum bursts preserve transient crest without rapid gain pumping

**Checkpoint**: All 3 user stories (P1, P2, P3) are complete and independently verified.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Changelog update, real-time safety audit, and full CI-equivalent quality gates.

- [X] T012 [P] Add an `Added` entry for Gain Match under `## [Unreleased]` in `CHANGELOG.md`
- [X] T013 Audit all edits in `StereoPunchDsp::process_sample()` and `OmorePunch::process()` against Constitution Principles III & IV (zero heap allocations, zero locks, cached coefficients, `NaN`/`Inf`/denormal protection, `sanitize_sample()`) in `src/lib.rs`
- [X] T014 Run all quality gates from `/home/tdavio/src/omore-punch/.worktrees/006-gain-match`: `LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs` (empty output), `cargo test --release` (all tests pass), and `cargo run --package xtask --release -- bundle omore-punch --release` (`omore-punch.vst3` and `omore-punch.clap` generated in `target/bundled/`)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: Starts immediately (`T001`).
- **Foundational (Phase 2)**: Depends on `T001` (`T002` → `T003` → `T004`). Blocks all user stories.
- **User Story 1 (Phase 3 - P1 🎯 MVP)**: Depends on Phase 2 (`T005` → `T006`).
- **User Story 2 (Phase 4 - P2)**: Depends on Phase 3 (`T007` → `T008` → `T009`) because all edits target `src/lib.rs` and `T007` builds on `process_sample()` Gain Match logic.
- **User Story 3 (Phase 5 - P3)**: Depends on Phase 4 (`T010` → `T011`) because all edits target `src/lib.rs`.
- **Polish (Phase 6)**: `T012` can run in parallel with Phase 5 (`CHANGELOG.md` is a separate file); `T013` → `T014` run after `T011` and `T012`.

### Parallel Opportunities

- Because Omore Punch follows a strict single-file plugin architecture (`src/lib.rs`, Constitution Principle I), Rust implementation tasks in `src/lib.rs` run sequentially to avoid merge/edit conflicts.
- `T012` (`CHANGELOG.md`) is in a separate file and marked `[P]` so it can be done in parallel with DSP/UI tasks.

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1 (`T001`) + Phase 2 (`T002`–`T004`).
2. Complete Phase 3 (`T005`–`T006`).
3. Validate core RMS level matching (`<= 0.5 dB`) independently via `cargo test --release`.

### Incremental Delivery

1. Phase 2 lays down the parameter, telemetry, and DSP state foundation with zero impact when `gain_match == false`.
2. Phase 3 (`US1`) delivers core post-Clipper Gain Match DSP + numerical verification.
3. Phase 4 (`US2`) delivers click-free ON/OFF slewing and the Section 4 UI toggle + `Trim` readout + headless layout check.
4. Phase 5 (`US3`) delivers `-60 dBFS` presence gating (`Hold` on silence) + transient preservation tests.
5. Phase 6 finalizes `CHANGELOG.md`, real-time safety review, ASCII check, Release tests, and VST3/CLAP bundling.
