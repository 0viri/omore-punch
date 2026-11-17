---
name: pr-review
description: >-
  Review local uncommitted changes, a feature/fix branch, or a Pull Request for
  Omore Punch before commit or merge into main. Use when the user asks for a
  code review, PR review, or pre-commit diff check.
---

# Omore Punch PR & Pre-Commit Review

Perform a strict **read-only** review of the changes against `GEMINI.md` and `ARCHITECTURE.md`. Never edit source files or commit during the review; report findings with exact file paths, line numbers, and observed code evidence.

## 1. Determine Scope & Gather Diff

1. Check repository state:
   - `git status`
   - `git branch --show-current`
2. Select the diff target:
   - **Pre-commit (Option A)**: inspect `git diff` and `git diff --staged`, plus any relevant untracked files (for example `.specify/bugs/<slug>/` or `specs/<feature>/`). Flag any unrelated untracked files that must not be staged with this PR.
   - **Branch / PR (Option B)**: inspect `git diff main...HEAD` (or `gh pr diff <number>` if a PR number is provided).
3. Respect the token-economy rule on `src/lib.rs`: NEVER read `src/lib.rs` in full. Use `grep -n` to locate modified symbols and `read_file` with targeted 100-250 line windows around changed hunks and their callers/callees.

## 2. Five-Axis Audit Checklist

### Axis 1: Real-Time Audio Safety (`process()`, `StereoPunchDsp`, `DspConfig`)
- Zero heap allocations or deallocations (`Vec`, `String`, `Box`, `format!`, `.to_string()`, `.clone()` on heap types, implicit `drop`) on the audio thread.
- Zero blocking locks (`Mutex`, `RwLock`, blocking channels).
- No per-sample transcendental calls (`.exp()`, `.sin()`, `.cos()`, `.tan()`) unless cached against `sample_rate` and parameter changes.
- `NaN`, `Inf`, and denormal protection on internal states (envelope, sidechain HPF, biquads) and final output routed through `sanitize_sample()` (`[-4.0, 4.0]` safety clamp).
- Correct `reset()` and `sample_rate` reinitialization behavior.

### Axis 2: DSP Logic & DAW Compatibility (`OmorePunchParams`, `MeterTelemetry`, `Plugin`/`ClapPlugin`/`Vst3Plugin`)
- Signal flow matches `ARCHITECTURE.md` and stereo linking remains consistent.
- No audio clicks from unsmoothed parameter changes where smoothing is expected.
- `MeterTelemetry` uses lock-free atomics (`AtomicU32` via `to_bits`/`from_bits`, `AtomicBool`, `Ordering::Relaxed`).
- DAW state backward compatibility: parameter `#[id = "..."]`, ranges, defaults, `VST3_CLASS_ID`, and `CLAP_ID` are preserved (or any breaking `0.x` change is intentional and documented).
- `assert_process_allocs` remains enabled in `Cargo.toml`.

### Axis 3: `egui` Interface (`draw_omore_punch_ui`)
- Every parameter interaction is wrapped in `begin_set_parameter` / `set_parameter` / `end_set_parameter`.
- `request_repaint()` is preserved; no blocking or heavy per-frame work in UI code.
- Version string uses `env!("CARGO_PKG_VERSION")` (never hardcoded).

### Axis 4: Architecture, Spec-Kit & Project Rules (`GEMINI.md`)
- Four-block isolation in `src/lib.rs` preserved (`StereoPunchDsp` independent of UI and host wrapper).
- Strictly pure ASCII in `src/lib.rs` (zero accents, Unicode symbols, or emojis).
- `CHANGELOG.md` includes an entry under `## [Unreleased]` (and `Cargo.toml` version is untouched outside a release PR).
- Bug fixes on `main` have corresponding Spec-Kit artifacts under `.specify/bugs/<slug>/` (`assessment.md`, `fix.md`, and `test.md` as applicable).
- Branch hygiene: one branch = one intention = one PR (`fix/<slug>`, `feat/<folder>`, `docs/...`, `chore/...`).

### Axis 5: Mandatory Local Checks
Run and report exact outcomes:
1. Pure ASCII check on `src/lib.rs`:
   ```bash
   LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs
   ```
   (Expected: no matches / exit code 1.)
2. Release unit tests:
   ```bash
   cargo test --release
   ```
   Verify all existing DSP tests (including `test_nan_inf_and_safety_clamp`) pass, and check that any new or modified DSP behavior has dedicated unit test coverage.
3. Bundle build (run when requested or for non-trivial code changes; otherwise state clearly if not run):
   ```bash
   cargo run --package xtask --release -- bundle omore-punch --release
   ```

## 3. Report Format

Present the review concisely in the user's conversation language (French with *tu* per `GEMINI.md`):
- **Verdict**: `APPROVE`, `APPROVE WITH MINOR NOTES`, or `CHANGES REQUESTED`.
- **Checks**: exact status of ASCII check, `cargo test --release`, and bundle build (if run).
- **Findings** ordered by severity (`Critical / RT & Audio Blocker`, `Major / DAW & Functional`, `Minor / UI, Tests & GEMINI.md`, `Nit / Optional`), each citing `file:line`, observed evidence, and a concrete recommendation.
- **Git Staging Note** (for Option A pre-commit reviews): list the exact files belonging to this PR vs. any unrelated local files to leave unstaged.
