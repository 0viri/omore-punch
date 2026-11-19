# Omore Punch Constitution

## Core Principles

### I. Monolithic Single-File Architecture

All plugin code (DSP engine, parameters, state sanitization, atomic telemetry, `egui` UI, VST3/CLAP
export proxies and unit tests) MUST live in the single file `src/lib.rs`. Plugin submodules and
additional plugin source files are forbidden. Build tooling (`xtask/`, `build.rs`, `build.sh`,
`build.ps1`) is not plugin code and is outside this rule.

`src/lib.rs` is organized in isolated logical blocks, and new code MUST join the block it belongs
to:

1. `StereoPunchDsp` and `DspConfig`: the pure DSP engine. This block MUST NOT depend on the UI or
   on the host wrapper.
2. `MeterTelemetry`: non-blocking communication from the audio thread to the UI and host metering.
3. `OmorePunchParams`, `sanitize_plugin_state`, `PeakMeterBallistics`, and `OmorePunch`: the
   nih-plug `Plugin` (including `Plugin::filter_state`), `ClapPlugin` and `Vst3Plugin`
   implementations.
4. `draw_omore_punch_ui`: the `egui` interface.
5. `mod clap` and `mod vst3`: local C/COM export proxies wrapping `nih-plug` to expose native host
   gain reduction metering and state-load parameter resynchronization.

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

All code reachable from `process()`, including the whole of `StereoPunchDsp` and
`PeakMeterBallistics`, MUST be real-time safe:

- **No heap allocation:** `Vec`, `String`, `Box`, `format!`, `.to_string()` and `.clone()` of
  heap-allocated types are forbidden. The nih-plug feature `assert_process_allocs` MUST stay
  enabled in `Cargo.toml`.
- **No blocking:** `Mutex`, `RwLock` and blocking channels are forbidden. Communication between
  the audio thread and the UI/host metering MUST go through nih-plug parameters or
  `MeterTelemetry` (`AtomicU32` through `f32::to_bits`/`f32::from_bits`, and `AtomicBool`, with
  `Ordering::Relaxed`).
- **Precomputed coefficients:** `.exp()`, `.sin()`, `.cos()` and `.tan()` MUST NOT be evaluated
  per sample while their inputs (`sample_rate`, `attack_ms`, `release_ms`, `release_mode`, filter
  frequencies and gains such as `sc_hpf_hz` and EQ controls) are unchanged. Coefficients
  (`attack_coeff`, `release_coeff`, auto-release, `sc_hpf_coeff`, EQ biquads, Gain Match,
  soft-bypass ramp, and `PeakMeterBallistics` decay) MUST be cached or precomputed outside
  `process()` and recomputed only when one of their inputs changes.

`assert_process_allocs` only fires in debug builds, inside nih-plug's process wrapper, so
`cargo test --workspace --release` does not detect allocations. Every change to `process()` or
`StereoPunchDsp` MUST therefore be reviewed against the forbidden constructs above.

Rationale: an allocation, a lock or an expensive per-sample call on the audio thread can cause
audio dropouts in Reaper.

### IV. Signal Sanitization & Safety Clamp (NON-NEGOTIABLE)

Every input sample, every output sample and every internal state variable (compressor/auto-release
envelopes, sidechain HPF and EQ biquad states, Gain Match accumulators, and `PeakMeterBallistics`
envelopes) MUST be protected against `NaN`, `Inf` and denormals (flushed to `0.0` at or below
`1.0e-20` via `flush_denormal()`). The final output MUST pass through `sanitize_sample()`, which
flushes non-finite and denormal values to `0.0` and clamps the signal to `[-4.0, 4.0]` (about
+12 dBFS). A unit test MUST keep covering this behaviour (currently
`test_nan_inf_and_safety_clamp`).

Rationale: a single non-finite, denormal or runaway sample can cause CPU spikes or trigger
Reaper's automatic track muting, and it propagates to every processor downstream of the drum bus.

### V. Mandatory DSP & Workspace Unit Tests

DSP behaviour MUST be verified by unit tests in the `tests` module of `src/lib.rs`, and bundling
tooling MUST be covered by unit tests in `xtask/src/main.rs`. The existing tests (stereo-linked
compression, dual-stage auto-release, sidechain HPF, 3-band EQ, clipper ceiling, dry/wet mix,
post-processing Gain Match, soft-bypass, peak meter ballistics, state sanitization, host GR ABI
proxies, sanitization and safety clamp) MUST keep passing. Any new or modified DSP feature MUST
add or update at least one test that checks its numerical behaviour, as
`test_sidechain_hpf_kick_immunity` did for the sidechain HPF in v0.2.0.
`cargo test --workspace --release` MUST pass on Linux and Windows before any change lands on
`main` and before any release.

Rationale: DSP and metadata regressions are hard to spot in isolation; numerical and ABI tests on
synthetic signals catch them before a bundle reaches Reaper.

### VI. Single-Source Versioning

The version number MUST be defined only in `Cargo.toml`. Plugin host metadata and the `egui`
header MUST read it through `env!("CARGO_PKG_VERSION")`; a hard-coded version string anywhere in
`src/lib.rs` is forbidden.

For development builds carrying a `-dev` suffix (e.g. `0.3.1-dev`), host metadata reports the raw
`CARGO_PKG_VERSION`, while the `egui` header splits `-dev` via `parse_display_version()` to render
`OMORE PUNCH v<BASE>` alongside a distinct `[DEV <hash>]` / `[DEV <hash>-dirty]` badge (captured
at compile time by `build.rs` into `OMORE_GIT_TAG`, falling back to `[DEV]`). Release builds
render `OMORE PUNCH v<VERSION>` without any dev badge.

Rationale: a single source of truth keeps the version reported to the host and the version shown
in the UI aligned while making test builds immediately identifiable from published releases.

## Platform, Format & GUI Constraints

- **Stack:** Rust (edition 2021) with `nih_plug` and `nih_plug_egui`.
- **Formats:** every build MUST export both VST3 and CLAP through local proxy modules in
  `src/lib.rs`:
  - `mod vst3`: wraps `nih_plug`'s VST3 wrapper to expose `PreSonus::IGainReductionInfo` for live
    Reaper mixer GR metering and invokes `IComponentHandler::restart_component(kParamValuesChanged)`
    after `IComponent::set_state`.
  - `mod clap`: wraps `nih_plug`'s CLAP wrapper to expose `clap.gain-adjustment-metering/0` and
    invokes `clap_host_params::rescan(CLAP_PARAM_RESCAN_VALUES)` after `clap.state::load`.
- **State Compatibility:** `Plugin::filter_state` (`sanitize_plugin_state`) MUST normalize legacy,
  partial or malformed Reaper states/presets by filling missing defaults, clamping floats,
  validating enums and stripping unknown keys (`editor-state` included).
- **Targets:** the plugin MUST run in Reaper on Linux (Debian 13) and Windows 11, both x86_64.
  CI MUST build and test on both operating systems.
- **Bundling:** `cargo run --package xtask --release -- bundle omore-punch --release` MUST produce
  `Omore Punch.vst3` (including `Contents/Resources/moduleinfo.json`) and `Omore Punch.clap` in
  `target/bundled/`, using `bundler.toml` for display naming and `build.rs` (`winresource`) to
  embed Windows PE `VS_VERSION_INFO`.
- **GUI:**
  - Dark theme inspired by Omore Sampler applied through `apply_omore_dark_theme` (`panel_fill`
    `rgb(18, 19, 24)`, `window_fill` `rgb(24, 26, 32)`, `extreme_bg_color` `rgb(10, 11, 14)`,
    amber/gold accents `rgb(225, 145, 25)` / `rgb(245, 185, 35)`), combined with SSL-inspired
    color-coded rotary knob caps and graduated scales.
  - Strictly ASCII-only text (Principle II) using `monospace()` typography for parameter labels,
    formatted readouts and bracketed action buttons (`[BYPASS: OFF/ON]`, `[Manual]`, `[Auto Tight]`,
    `[Auto Punch]`, `[Auto Glue]`, `[Punch 3ms / 80ms]`, `[Snare Safe 200Hz]`,
    `[Auto Makeup: ON/OFF]`, `[Soft Clip]`, `[Hard Clip]`, `[Bypass]`, `[Gain Match: OFF/ON]`,
    `[Reset Peak]`, `[CLIP: OK]`).
  - Top header bar showing `OMORE PUNCH v<VERSION>  |  STEREO DRUM BUS COMPRESSOR & CLIPPER`
    (with the `[DEV <hash>]` badge on `-dev` builds) on the left, and `[BYPASS: OFF]` /
    `[BYPASS: ON]` alongside `[CLIP: OK]` / `[CLIP: ACTIVE - CLICK TO RESET]` on the right.
  - Two-column central layout fitting within the editor window (`900x650` px, with vertical
    `ScrollArea` fallback) without overlap: left column for controls
    (`1. COMPRESSOR & ENVELOPE BALLISTICS` including `Release Mode:` profiles,
    `2. OUTPUT CLIPPER & PARALLEL MIX`, and `3. DRUM BUS EQ`) using SSL-style rotary knobs arranged
    in a compact grid (graduated ring, color-coded cap, angular pointer, vertical mouse drag,
    modifier fine-drag, double-click reset, formatted ASCII readout, and compact `-`, `+` and
    `Reset` buttons wrapped in `begin_set_parameter` / `set_parameter` / `end_set_parameter`);
    right column for metering (`4. GAIN REDUCTION & STEREO BUS METERING`) with `GR` / `Max GR`
    readouts, `[Reset Peak]`, `[Gain Match: OFF/ON]` + `Trim: +/-X.X dB`, stereo `In L/R` and
    `Out L/R` bars driven by buffer-size-invariant `PeakMeterBallistics` (`85 ms`), and a
    Reaper-style top-down `GR (0 to -24 dB)` meter with peak hold.
  - Continuous 60 FPS meter refresh via `egui_ctx.request_repaint()` while the editor is open.

## Development Workflow & Quality Gates

Before any change lands on `main`, and before any release tag (`v*`), the following gates MUST
pass. They mirror `.github/workflows/build.yml`:

1. ASCII check on `src/lib.rs` (Principle II).
2. `cargo test --workspace --release` (Principle V).
3. VST3 (`Omore Punch.vst3` + `moduleinfo.json`) and CLAP (`Omore Punch.clap`) bundling through
   `xtask`, on Linux x86_64 and Windows x86_64.

Additional rules:

- Changes to `process()`, `StereoPunchDsp` or `PeakMeterBallistics` MUST be reviewed against
  Principles III and IV.
- Every release MUST bump the version in `Cargo.toml` (Principle VI) and record its notable
  changes in `CHANGELOG.md`.
- Spec Kit plans MUST pass the Constitution Check against Principles I to VI; any deviation MUST
  be justified in the plan's Complexity Tracking table, otherwise the plan MUST be revised.

## Governance

This constitution supersedes every other development practice of Omore Punch. `GEMINI.md`
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

**Version**: 1.3.0 | **Ratified**: 2026-11-05 | **Last Amended**: 2026-11-19
