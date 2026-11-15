---

description: "Task list for feature 002-dev-version-tag implementation (with Git hash + dirty tag)"
---

# Tasks: Tag de build Git dans le badge de version de développement (DEV)

**Input**: Design documents from `/specs/002-dev-version-tag/`

**Prerequisites**: `plan.md`, `spec.md`, `research.md`, `data-model.md`, `contracts/ui-version-tag-contract.md`, `quickstart.md`

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (`[US1]`, `[US2]`)
- Exact file paths are included in every task description.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Create the Cargo build script to capture Git commit hash and dirty state at compile time.

- [X] T001 Create `build.rs` at repository root to emit `cargo:rustc-env=OMORE_GIT_TAG=<short7>[-dirty]` (using `git rev-parse --short=7 HEAD` and `git status --porcelain`, falling back cleanly to empty string when `git` is unavailable) and `cargo:rerun-if-changed` directives for `src/lib.rs`, `Cargo.toml`, `build.rs`, `.git/HEAD`, `.git/index`, and the active branch ref file if present

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Implement pure helper functions for `-dev` detection and badge label formatting in Block 4 of `src/lib.rs`.

- [X] T002 Ensure `parse_display_version(version: &str) -> (&str, bool)` and implement `format_dev_badge_label(git_tag: &str) -> String` in Block 4 (UI helpers) of `src/lib.rs`, returning `"[DEV <tag>]"` when `git_tag.trim()` is non-empty and ASCII, and `"[DEV]"` as fallback

**Checkpoint**: Foundation ready — user story implementation can proceed.

---

## Phase 3: User Story 1 - Identification précise du commit et de l'état local sur les builds de développement (Priority: P1) 🎯 MVP

**Goal**: Display `[DEV <hash>]` or `[DEV <hash>-dirty]` (or `[DEV]` fallback) in the header on `-dev` builds, omit badge and hash on release builds, and fit cleanly on one line within `UI_WINDOW_WIDTH = 820` even with `[CLIP: ACTIVE - CLICK TO RESET]`.

**Independent Test**: Run `cargo test --release` to verify `parse_display_version`, `format_dev_badge_label`, and empirical `egui` header width fit inside `820 px`, then visually verify the badge in Reaper.

### Implementation & Tests for User Story 1

- [X] T003 [US1] Update `TopBottomPanel::top("omore_punch_header")` in `draw_omore_punch_ui` in `src/lib.rs` to read `option_env!("OMORE_GIT_TAG").unwrap_or("")`, format the badge via `format_dev_badge_label`, render `"OMORE PUNCH v{base_version}"` and `"|  STEREO DRUM BUS COMPRESSOR & CLIPPER"` at size `13.5`, the steel-blue `[DEV ...]` badge at size `11.5`, `[CLIP: ...]` button at size `12.5`, and local `item_spacing.x = 5.0` (while keeping release builds without badge or hash)
- [X] T004 [US1] Update/add unit tests in `mod tests` of `src/lib.rs` covering `format_dev_badge_label` (`"1d2c5b9"`, `"1d2c5b9-dirty"`, `""`, whitespace, non-ASCII fallback) and empirical `egui` header layout width fit within `UI_WINDOW_WIDTH` (`820.0`) with worst-case `[DEV 1d2c5b9-dirty]` + `[CLIP: ACTIVE - CLICK TO RESET]`

---

## Phase 4: User Story 2 - Cohérence de la version exposée à l'hôte DAW (Priority: P2)

**Goal**: Guarantee that `OmorePunch::VERSION` exposed to VST3/CLAP hosts remains strictly equal to `env!("CARGO_PKG_VERSION")` (unchanged by `OMORE_GIT_TAG`).

**Independent Test**: Run `cargo test --release` to verify `OmorePunch::VERSION == env!("CARGO_PKG_VERSION")`.

### Implementation & Tests for User Story 2

- [X] T005 [US2] Verify `OmorePunch::VERSION` remains `env!("CARGO_PKG_VERSION")` verbatim and `test_host_version_matches_cargo_pkg_version` passes in `src/lib.rs`

---

## Phase 5: Polish & Cross-Cutting Concerns

**Purpose**: Changelog synchronization and mandatory CI/Constitution quality gates.

- [X] T006 [P] Update the `[Unreleased]` entry in `CHANGELOG.md` to document the `[DEV <hash>]` / `[DEV <hash>-dirty]` build badge captured at compile time via `build.rs`
- [X] T007 Run all validation gates from `specs/002-dev-version-tag/quickstart.md`: ASCII check (`LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs`), release tests (`cargo test --release`), and VST3/CLAP bundling (`cargo run --package xtask --release -- bundle omore-punch --release`)
