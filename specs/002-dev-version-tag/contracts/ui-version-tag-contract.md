# UI & Build Contract: Version & `[DEV <git-tag>]` Header Badge

**Feature**: `002-dev-version-tag`
**Components**: `build.rs` (build-time env), `parse_display_version`, `format_dev_badge_label`, and `draw_omore_punch_ui` in `src/lib.rs`

## Interface Contract

### 1. Build Script Contract (`build.rs`)
- Emits `cargo:rustc-env=OMORE_GIT_TAG=<tag>` where `<tag>` is:
  - `<short7>-dirty` when `git rev-parse --short=7 HEAD` succeeds and `git status --porcelain` is non-empty.
  - `<short7>` when `git rev-parse --short=7 HEAD` succeeds and `git status --porcelain` is empty.
  - `""` (empty) when `git` is unavailable or fails, without failing the build.
- Emits `cargo:rerun-if-changed` for `src/lib.rs`, `Cargo.toml`, `build.rs`, `.git/HEAD`, `.git/index`, and the active branch ref file if present.

### 2. Helper Contracts (`src/lib.rs`, Block 4)
- `fn parse_display_version(version: &str) -> (&str, bool)`:
  - Returns `(base, true)` if `version.strip_suffix("-dev")` yields `Some(base)` with `!base.is_empty()`.
  - Returns `(version, false)` otherwise. Zero heap allocation.
- `fn format_dev_badge_label(git_tag: &str) -> String`:
  - Trims `git_tag`. If non-empty and `is_ascii()`, returns `format!("[DEV {}]", trimmed)`.
  - Otherwise returns `"[DEV]".to_string()`.

### 3. Visual Header Contract (`TopBottomPanel::top("omore_punch_header")`)
- **When `is_dev == true`**:
  - Left title: `"OMORE PUNCH v{base}"` in amber/gold `rgb(245, 185, 35)`, monospace strong `13.5`.
  - Framed badge: `egui::Frame::new()` (`fill: rgb(28, 78, 120)`, `stroke: 1.0 px rgb(70, 155, 220)`, `corner_radius: 3.0`, `inner_margin: Margin::symmetric(5, 1)`) displaying `format_dev_badge_label(option_env!("OMORE_GIT_TAG").unwrap_or(""))` in `rgb(235, 248, 255)`, monospace strong `11.5`.
  - Subtitle: `"|  STEREO DRUM BUS COMPRESSOR & CLIPPER"` in `rgb(245, 185, 35)`, monospace strong `13.5`.
- **When `is_dev == false`**:
  - No badge and no Git hash displayed.
  - Renders `"OMORE PUNCH v{base}  |  STEREO DRUM BUS COMPRESSOR & CLIPPER"` in `rgb(245, 185, 35)`, monospace strong `13.5`.
- **Right Zone**:
  - `[CLIP: OK]` / `[CLIP: ACTIVE - CLICK TO RESET]` in monospace strong `12.5`, non-overlapping and strictly inside `UI_WINDOW_WIDTH = 820`.

### 4. Host Metadata Contract (`OmorePunch::VERSION`)
- `Plugin::VERSION` MUST remain `env!("CARGO_PKG_VERSION")` verbatim.

### 5. Compliance
- `src/lib.rs` remains 100% pure ASCII and no audio-thread (`process()`, `StereoPunchDsp`) code is touched.
