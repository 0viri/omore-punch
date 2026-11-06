# Omore Punch Constitution

## Core Principles

### I. Monolithic Single-File Architecture

All plugin code (DSP engine, parameters, atomic telemetry, `egui` UI, VST3/CLAP exports and unit
tests) MUST live in the single file `src/lib.rs`. Plugin submodules and additional plugin source
files are forbidden. Build tooling (`xtask/`, `build.sh`) is not plugin code and is outside this
rule.

`src/lib.rs` is organized in four isolated blocks, and new code MUST join the block it belongs to:

1. `StereoPunchDsp` and `DspConfig`: the pure DSP engine. This block MUST NOT depend on the UI or
   on the host wrapper.
2. `MeterTelemetry`: non-blocking communication from the audio thread to the UI.
3. `OmorePunchParams` and `OmorePunch`: the nih-plug `Plugin`, `ClapPlugin` and `Vst3Plugin`
   implementations.
4. `draw_omore_punch_ui`: the `egui` interface.

Rationale: a single file keeps the real-time audit, the unit tests and AI-assisted maintenance
simple, and a host-independent DSP block can be tested in isolation.

### II. Pure ASCII Source

`src/lib.rs` MUST contain only ASCII characters: no emoji, no accented letters and no other
non-ASCII Unicode, in code, comments and UI strings alike. The check
`LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs` MUST print nothing; CI runs it on pushes
and pull requests to `main`.

Rationale: non-ASCII glyphs render as empty boxes in `egui` on Linux, while an ASCII-only UI
renders identically on Debian 13 and Windows 11. Markdown documentation is outside this rule.

### III. Real-Time Audio Thread Safety (NON-NEGOTIABLE)

All code reachable from `process()`, including the whole of `StereoPunchDsp`, MUST be real-time
safe:

- **No heap allocation:** `Vec`, `String`, `Box`, `format!`, `.to_string()` and `.clone()` of
  heap-allocated types are forbidden. The nih-plug feature `assert_process_allocs` MUST stay
  enabled in `Cargo.toml`.
- **No blocking:** `Mutex`, `RwLock` and blocking channels are forbidden. Communication between
  the audio thread and the UI MUST go through nih-plug parameters or `MeterTelemetry`
  (`AtomicU32` through `f32::to_bits`/`f32::from_bits`, and `AtomicBool`, with
  `Ordering::Relaxed`).
- **Precomputed coefficients:** `.exp()`, `.sin()`, `.cos()` and `.tan()` MUST NOT be evaluated
  per sample while their inputs (`sample_rate`, `attack_ms`, `release_ms`, filter frequencies
  such as `sc_hpf_hz`) are unchanged. Coefficients (`attack_coeff`, `release_coeff`,
  `sc_hpf_coeff`) MUST be cached and recomputed only when one of their inputs changes.

`assert_process_allocs` only fires in debug builds, inside nih-plug's process wrapper, so
`cargo test --release` does not detect allocations. Every change to `process()` or
`StereoPunchDsp` MUST therefore be reviewed against the forbidden constructs above.

Rationale: an allocation, a lock or an expensive per-sample call on the audio thread can cause
audio dropouts in Reaper.

### IV. Signal Sanitization & Safety Clamp (NON-NEGOTIABLE)

Every input sample, every output sample and every internal state variable (envelope, filter state
such as the sidechain HPF) MUST be protected against `NaN`, `Inf` and denormals. The final output
MUST pass through `sanitize_sample()`, which replaces any non-finite value with `0.0` and clamps
the signal to `[-4.0, 4.0]` (about +12 dBFS). A unit test MUST keep covering this behaviour
(currently `test_nan_inf_and_safety_clamp`).

Rationale: a single non-finite or runaway sample can trigger Reaper's automatic track muting, and
it propagates to every processor downstream of the drum bus.

### V. Mandatory DSP Unit Tests

DSP behaviour MUST be verified by unit tests in the `tests` module of `src/lib.rs`. The existing
tests (stereo-linked compression, sidechain HPF, clipper ceiling, dry/wet mix, sanitization and
safety clamp) MUST keep passing. Any new or modified DSP feature MUST add or update at least one
test that checks its numerical behaviour, as `test_sidechain_hpf_kick_immunity` did for the
sidechain HPF in v0.2.0. `cargo test --release` MUST pass on Linux and Windows before any change
lands on `main` and before any release.

Rationale: DSP regressions are hard to hear in isolation; numerical tests on synthetic signals
catch them before a bundle reaches Reaper.

### VI. Single-Source Versioning

The version number MUST be defined only in `Cargo.toml`. Plugin metadata and the `egui` header
MUST read it through `env!("CARGO_PKG_VERSION")`; a hard-coded version string anywhere in
`src/lib.rs` is forbidden.

Rationale: a single source of truth keeps the version reported to the host and the version shown
in the UI identical.

## Platform, Format & GUI Constraints

- **Stack:** Rust (edition 2021) with `nih_plug` and `nih_plug_egui`.
- **Formats:** every build MUST export both VST3 (`nih_export_vst3!`) and CLAP
  (`nih_export_clap!`).
- **Targets:** the plugin MUST run in Reaper on Linux (Debian 13) and Windows 11, both x86_64.
  CI MUST build and test on both operating systems.
- **Bundling:** `cargo run --package xtask --release -- bundle omore-punch --release` MUST produce
  `omore-punch.vst3` and `omore-punch.clap` in `target/bundled/`.
- **GUI:** dark theme inspired by Omore Sampler, applied through `apply_omore_dark_theme`, and
  ASCII-only text (Principle II). TODO(GUI_STANDARDS): `GEMIDI.md` section 4 is truncated;
  complete the remaining `egui` standards there, then amend this bullet.

## Development Workflow & Quality Gates

Before any change lands on `main`, and before any release tag (`v*`), the following gates MUST
pass. They mirror `.github/workflows/build.yml`:

1. ASCII check on `src/lib.rs` (Principle II).
2. `cargo test --release` (Principle V).
3. VST3 and CLAP bundling through `xtask`, on Linux x86_64 and Windows x86_64.

Additional rules:

- Changes to `process()` or `StereoPunchDsp` MUST be reviewed against Principles III and IV.
- Every release MUST bump the version in `Cargo.toml` (Principle VI) and record its notable
  changes in `CHANGELOG.md`.
- Spec Kit plans MUST pass the Constitution Check against Principles I to VI; any deviation MUST
  be justified in the plan's Complexity Tracking table, otherwise the plan MUST be revised.

## Governance

This constitution supersedes every other development practice of Omore Punch. `GEMIDI.md`
(loaded through `CLAUDE.md`) and `ARCHITECTURE.md` hold the detailed runtime guidance and MUST
stay consistent with it: an amendment that changes a rule MUST update those files in the same
change.

- **Amendments:** proposed through `/speckit-constitution`, documented by a Sync Impact Report,
  versioned, and approved by the maintainer before they take effect.
- **Versioning policy:** semantic versioning. MAJOR for removing or redefining a principle, MINOR
  for adding a principle or section or materially expanding guidance, PATCH for clarifications
  and wording fixes.
- **Compliance review:** every specification, plan and pull request MUST be checked against this
  constitution. CI enforces the quality gates; the other rules are checked in review.

**Version**: 1.0.0 | **Ratified**: 2026-11-05 | **Last Amended**: 2026-11-05
