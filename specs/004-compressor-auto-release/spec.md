# Feature Specification: Compressor Auto-Release

**Feature Branch**: `feat/004-compressor-auto-release`
**Created**: 2026-11-14
**Status**: Draft
**Input**: User description: "implémenter l'auto-release au niveau du compresseur"

## Clarifications

### Session 2026-11-14

- Q: When Auto-Release is enabled, does it completely override the manual Release knob with a fixed dual-stage curve, or does the manual Release knob scale the Auto-Release timing? -> A: Fixed dual-stage curve; Auto-Release ignores the manual Release setting and applies its own fast/slow constants.
- Q: Which dual-stage time-constant profile should Auto-Release target, and how should it be structured with Manual mode? -> A: A single 4-mode release selector: `Manual` (default), `Auto Tight` (~50 ms fast / ~600 ms slow), `Auto Punch` (~80 ms fast / ~800 ms slow), and `Auto Glue` (~100 ms fast / ~1200 ms slow).
- Q: How should the manual Release knob behave in the UI when an Auto-Release mode is active? -> A: Visually dimmed with an `AUTO` override indication, while remaining editable so the user can pre-adjust the manual Release value for when `Manual` mode is restored.
- Q: When an Auto-Release mode is active and the user clicks `[Punch 3ms / 80ms]`, should it switch Release Mode back to `Manual`? -> A: Yes; `[Punch 3ms / 80ms]` sets Attack to 3.0 ms, Release to 80.0 ms, and switches Release Mode to `Manual` so the fixed 80 ms release takes effect.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Program-Dependent Auto-Release on the Drum Bus (Priority: P1)

As a mixing engineer using Omore Punch on a stereo drum bus, I want to choose between three program-dependent Auto-Release profiles (`Auto Tight`, `Auto Punch`, `Auto Glue`) on the compressor so that short, punchy transients (kick, snare) recover quickly while sustained or dense passages (cymbal wash, room mics, rapid drum fills) release smoothly at a musical pace suited to the track.

**Why this priority**: This is the core value of the feature. A fixed manual release forces a compromise between transient punch and smooth sustain; offering three tailored program-dependent auto-release curves (`50/600 ms`, `80/800 ms`, `100/1200 ms`) solves both simultaneously across fast, punchy, and glued drum mixes.

**Independent Test**: Can be fully tested by selecting each Auto-Release mode (`Auto Tight`, `Auto Punch`, `Auto Glue`) and feeding two contrasting test signals (a short transient burst vs. a sustained loud signal) into the compressor, verifying that gain reduction recovers significantly faster after the short transient than after the sustained signal, that `Auto Tight` recovers faster than `Auto Punch` which recovers faster than `Auto Glue`, and that both channels remain stereo-linked.

**Acceptance Scenarios**:

1. **Given** one of the Auto-Release modes (`Auto Tight`, `Auto Punch`, or `Auto Glue`) is active and the compressor is reducing gain, **When** a short isolated drum transient (e.g., a 10-20 ms hit) ends, **Then** the compressor recovers rapidly (guided by the mode's fast stage: ~50 ms, ~80 ms, or ~100 ms) so the following hit retains full transient punch.
2. **Given** one of the Auto-Release modes is active and the compressor is reducing gain, **When** a sustained high-level passage (e.g., 300+ ms above threshold) ends, **Then** the compressor releases progressively and smoothly (guided by the mode's slow stage: ~600 ms, ~800 ms, or ~1200 ms), avoiding abrupt gain-recovery pumping.
3. **Given** Release Mode is set to `Manual` (default state), **When** audio is processed, **Then** the compressor uses the manual Release setting exclusively and behaves identically to the existing release behavior.

---

### User Story 2 - Clear Control Integration & Interaction with Manual Release (Priority: P2)

As a user interacting with the plugin interface, I want a clear 4-mode Release selector in the compressor section and unambiguous feedback on how it interacts with the manual Release control and existing quick-preset buttons.

**Why this priority**: Without clear visual feedback, users may turn the manual Release knob while an Auto-Release mode is active and wonder why the sound does not change, or trigger a fixed-time preset without knowing its effect on Release Mode.

**Independent Test**: Can be tested by opening the plugin UI, switching across `Manual`, `Auto Tight`, `Auto Punch`, and `Auto Glue`, verifying the active mode indication on the manual Release control, and clicking `[Punch 3ms / 80ms]` to confirm deterministic state transitions.

**Acceptance Scenarios**:

1. **Given** the plugin editor is open, **When** the user views the Compressor & Envelope Ballistics section, **Then** a 4-mode Release selector (ASCII-only labels for `Manual`, `Auto Tight`, `Auto Punch`, `Auto Glue`) is visible and accessible without overlapping existing controls.
2. **Given** an Auto-Release mode (`Auto Tight`, `Auto Punch`, or `Auto Glue`) is active, **When** the user views or adjusts the manual Release control, **Then** the control appears visually dimmed with an `AUTO` override indicator while still allowing its stored value to be pre-adjusted for `Manual` mode without affecting the active Auto-Release behavior.
3. **Given** an Auto-Release mode is active, **When** the user clicks the `[Punch 3ms / 80ms]` preset button, **Then** Attack is set to 3.0 ms, Release is set to 80.0 ms, and Release Mode is set to `Manual` so the fixed 80 ms signature ballistics are actually applied.

---

### User Story 3 - Session Recall, Automation & Real-Time Stability (Priority: P3)

As a producer working in a DAW (such as Reaper on Linux or Windows), I want the Release Mode setting to be automatable, saved and recalled with my project, and safe to switch during playback without clicks, stereo image shifts, or audio dropouts.

**Why this priority**: Reliable DAW integration and glitch-free real-time operation are essential for production use, though they build on top of the core DSP and UI functionality.

**Independent Test**: Can be tested by switching between `Manual` and the three Auto-Release modes mid-playback during active gain reduction, verifying no audible clicks, non-finite values, or L/R imbalance, and saving/reloading plugin state.

**Acceptance Scenarios**:

1. **Given** audio is playing through the compressor with active gain reduction, **When** Release Mode is switched between `Manual`, `Auto Tight`, `Auto Punch`, and `Auto Glue` (via UI or DAW automation), **Then** the transition is smooth and glitch-free with zero non-finite output values or stereo L/R divergence.
2. **Given** a DAW project saved before Auto-Release existed, **When** the project is loaded in the updated plugin, **Then** Release Mode defaults to `Manual`, preserving the exact sound of existing mixes.
3. **Given** the DAW operates at different sample rates (e.g., 44.1 kHz, 48 kHz, 96 kHz), **When** any Auto-Release mode is active, **Then** the effective release timing remains consistent across sample rates.

---

### Edge Cases

- What happens when the input signal is complete silence or below threshold? Gain reduction stays at 0 dB and both fast and slow release stages remain idle with zero denormal buildup.
- What happens when Release Mode is switched while the compressor is in deep gain reduction (e.g., -12 dB)? The envelope transitions smoothly from the current gain reduction value without sudden jumps or clicks.
- What happens when non-finite samples (`NaN`, `Inf`) or extreme amplitudes enter the compressor with an Auto-Release mode active? Internal envelope states remain finite and output stays clamped within the safety bounds.
- What happens when the user adjusts the manual Release knob while an Auto-Release mode is active? The manual Release parameter value can still be stored/edited for when `Manual` mode is restored, without affecting the active Auto-Release DSP behavior.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST provide a 4-state Release Mode parameter for the compressor (`Manual`, `Auto Tight`, `Auto Punch`, `Auto Glue`), defaulting to `Manual` so existing sessions and presets retain identical behavior.
- **FR-002**: When Release Mode is `Manual`, the compressor MUST use the manual Release parameter exclusively, with no change to existing release behavior.
- **FR-003**: When Release Mode is an Auto-Release mode, the compressor MUST use a fixed program-dependent (dual-stage fast/slow) release curve independent of the manual Release parameter, using the corresponding profile constants: `Auto Tight` (~50 ms fast / ~600 ms slow), `Auto Punch` (~80 ms fast / ~800 ms slow), and `Auto Glue` (~100 ms fast / ~1200 ms slow).
- **FR-004**: Auto-Release MUST operate on the stereo-linked gain reduction signal so that left and right channels always receive identical gain reduction.
- **FR-005**: Auto-Release timing MUST scale with the host sample rate so perceived release behavior is consistent across standard audio sample rates (44.1 kHz through 192 kHz).
- **FR-006**: All envelope state variables involved in Auto-Release MUST remain protected against `NaN`, `Inf`, and denormals, and all audio processing MUST remain real-time safe (no allocations, no locks, no per-sample transcendental math calls when parameters/sample rate are unchanged).
- **FR-007**: The plugin UI MUST display an ASCII-only 4-mode Release Mode selector in the Compressor & Envelope Ballistics section, integrated cleanly into the existing two-column layout without control overlap.
- **FR-008**: When an Auto-Release mode is active, the UI MUST visually dim the manual Release control and display an ASCII `AUTO` override indicator, while keeping the control editable so users can pre-stage the manual Release value.
- **FR-009**: Clicking the `[Punch 3ms / 80ms]` quick-preset button MUST also set Release Mode to `Manual` alongside setting Attack to 3.0 ms and Release to 80.0 ms.
- **FR-010**: The Release Mode parameter MUST be exposed to the DAW host for automation and session state persistence.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: In each Auto-Release mode (`Auto Tight`, `Auto Punch`, `Auto Glue`), on a synthetic short transient burst (e.g., 15 ms above threshold followed by silence), the compressor recovers 90% of applied gain reduction at least 2x faster than on a sustained signal (e.g., 400 ms above threshold followed by silence) at the same threshold and ratio settings, and recovery speed follows the strict ordering `Auto Tight` > `Auto Punch` > `Auto Glue`.
- **SC-002**: Switching Release Mode mid-signal during active compression produces zero non-finite (`NaN`/`Inf`) samples and maintains stereo L/R gain reduction parity within 0.001 dB.
- **SC-003**: With Release Mode set to `Manual`, output on identical test signals matches the pre-feature compressor output with zero regression in existing DSP tests.
- **SC-004**: All quality gates pass on Linux and Windows: 100% pure ASCII source check, release unit test suite including new Auto-Release numerical tests across the three profiles, and VST3/CLAP bundle generation.

## Assumptions

- **Dual-stage program-dependent model**: Each Auto-Release profile implements an SSL-style dual-stage release curve with distinct fast/slow time constants (`Auto Tight`: ~50 ms / ~600 ms, `Auto Punch`: ~80 ms / ~800 ms, `Auto Glue`: ~100 ms / ~1200 ms), without extra user-facing sub-knobs.
- **Backward compatibility**: Existing presets and DAW sessions saved prior to this feature will load with Release Mode set to `Manual`.
- **Preset interaction**: `[Punch 3ms / 80ms]` explicitly targets a fixed 80 ms release, so switching Release Mode to `Manual` when that button is clicked matches user intent.
