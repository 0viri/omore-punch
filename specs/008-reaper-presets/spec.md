# Feature Specification: Reaper Preset Support

**Feature Branch**: `feat/008-reaper-presets`

**Created**: 2026-11-15

**Status**: Draft

**Input**: User description: "faire fonctionner les preset de Reaper"

## Clarifications

### Session 2026-11-15

- Q: Should Omore Punch also provide built-in factory presets visible in Reaper's preset dropdown, or should the feature focus solely on making Reaper's user preset save/load/switch/reset/import-export work reliably? -> A: Focus solely on Reaper's user preset workflow (saving, recalling, switching, resetting to default, and importing/exporting user presets and libraries); built-in factory presets are out of scope for this feature.
- Q: Must exported Reaper presets be portable between Linux and Windows within the same plugin format only (VST3 <-> VST3 and CLAP <-> CLAP), or also across formats (VST3 <-> CLAP)? -> A: Within the same plugin format only across Linux and Windows (VST3 <-> VST3 and CLAP <-> CLAP); cross-format preset loading (VST3 <-> CLAP) is out of scope.
- Q: What should happen to transient metering latches (`Max GR` peak hold and `CLIP: ACTIVE` indicator) when a preset is loaded or switched in Reaper? -> A: Automatically reset `Max GR` peak hold and the `CLIP` indicator latch whenever a preset is loaded or switched so meters reflect only the newly active preset.
- Q: Must existing Reaper projects and presets saved with previous versions of Omore Punch remain 100% backward-compatible and restore their exact settings? -> A: Yes, full backward compatibility is required for all previously saved Reaper projects and presets.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Save and Recall User Presets in Reaper (Priority: P1)

A mixing engineer adjusts Omore Punch's controls (compressor, envelope ballistics, sidechain high-pass filter, clipper, parallel mix, and drum bus EQ) on a drum bus track in Reaper, then uses Reaper's FX preset menu (`+` -> `Save preset...`) to store the configuration under a custom name. Later, on another track, in another project, or after closing and reopening the project, the engineer selects that saved preset from Reaper's preset dropdown and expects every control, its displayed readout, and the audio processing behavior to match the saved configuration exactly.

**Why this priority**: Saving and recalling custom presets in Reaper is the core workflow that allows engineers to reuse drum bus settings across tracks and sessions.

**Independent Test**: Load Omore Punch in Reaper, set non-default values across all plugin controls, save a preset via Reaper's FX preset menu, change all controls to different values, recall the saved preset, and verify that all controls, UI readouts, and audio output match the originally saved state.

**Acceptance Scenarios**:

1. **Given** Omore Punch is loaded on a Reaper track with custom values across all controls (continuous knobs and toggle/mode controls), **When** the user saves a preset using Reaper's preset menu and later recalls it after altering controls, **Then** every control returns to its exact saved value, the UI updates to reflect those values, and the audio processing matches the saved state.
2. **Given** a Reaper project containing Omore Punch with a recalled or modified preset, **When** the project is saved, closed, and reopened, **Then** Omore Punch restores the exact parameter state and active preset selection.

---

### User Story 2 - Reset to Default and Switch Between Presets During Playback (Priority: P2)

During an active mixing session with audio playing through the drum bus, the engineer switches between multiple saved presets in Reaper's preset dropdown (or triggers Reaper's reset-to-default action) to compare different compression, clipping, and EQ settings in real time.

**Why this priority**: Rapid A/B comparison between presets during playback is a primary reason engineers use Reaper's preset selector; switching presets must be glitch-free and immediately audible and visible.

**Independent Test**: Play a continuous drum loop through Omore Punch in Reaper while switching between at least two distinct saved presets and resetting to default; verify no audio dropouts, clicks/runaway levels, or stale UI values occur.

**Acceptance Scenarios**:

1. **Given** audio is playing through Omore Punch and two presets with distinct settings exist in Reaper, **When** the user switches from Preset A to Preset B in Reaper's preset dropdown, **Then** audio processing transitions cleanly without dropouts or non-finite output and all UI controls immediately reflect Preset B.
2. **Given** Omore Punch has modified controls, **When** the user selects Reaper's reset-to-factory-default action, **Then** all plugin controls return to their documented default values and audio processing updates accordingly.

---

### User Story 3 - Import, Export, and Cross-Platform Preset Compatibility (Priority: P3)

An engineer working on both Linux and Windows workstations exports a preset library (or individual preset file) from Reaper on one machine and imports it into Reaper on the other machine, or exchanges presets between plugin instances using Reaper's preset import/export actions.

**Why this priority**: Omore Punch targets both Linux and Windows Reaper environments; portable preset files ensure studio setups remain consistent across machines.

**Independent Test**: Export a preset library in Reaper, reset or remove local presets, import the exported library into a fresh Omore Punch instance of the same plugin format (including across Linux and Windows), and verify all presets recall identically.

**Acceptance Scenarios**:

1. **Given** one or more saved Omore Punch presets in Reaper, **When** the user exports the preset library and imports it into another Omore Punch instance of the same plugin format (on the same OS or across Linux/Windows), **Then** all exported presets appear in the list and recall their exact parameter values.

---

### Edge Cases

- What happens when a preset saved in an earlier or slightly different plugin version is loaded (e.g., if a parameter was added or a value is out of bounds)? Out-of-range values MUST be clamped to valid parameter ranges, and missing parameters MUST fall back to safe defaults without crashing or corrupting audio.
- What happens when a user recalls a preset while the plugin's editor window is closed, and then opens the editor window? The UI MUST display the recalled preset's values immediately upon opening.
- What happens when a user recalls a preset while the plugin's editor window is already open? All knobs, mode toggles, and readouts MUST update immediately without requiring manual interaction.
- What happens to transient metering state (such as peak gain reduction hold or clip indicator latch) when a preset is loaded? Both `Max GR` peak hold and the `CLIP` indicator latch MUST reset automatically upon preset load/switch, while parameter state remains independent of transient meters.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Omore Punch MUST allow Reaper to read and restore the complete set of user-configurable plugin parameters when saving, loading, and switching presets through Reaper's FX preset menu.
- **FR-002**: Preset saving, loading, and library export/import MUST work identically in both supported plugin formats (VST3 and CLAP) and across both target operating systems (Linux and Windows) within the same format (VST3 <-> VST3 and CLAP <-> CLAP); cross-format preset translation (VST3 <-> CLAP) is out of scope.
- **FR-003**: Recalling a preset MUST update both the audio processing state and the plugin's visual interface (knobs, toggles, and numeric readouts), whether the editor window is open or closed at the time of recall.
- **FR-004**: Triggering Reaper's reset-to-default action MUST restore all plugin parameters to their default values.
- **FR-005**: Switching presets during active audio playback MUST be real-time safe (no audio dropouts, clicks from uninitialized/invalid state, or runaway output).
- **FR-006**: Loading valid state from earlier Omore Punch versions MUST remain 100% backward-compatible and restore all previously saved parameters accurately; loading malformed, partial, or cross-version preset data MUST NOT crash the host or produce non-finite audio output, falling back safely to defaults for missing/unknown fields and clamping out-of-range values.
- **FR-007**: Built-in factory presets are out of scope for this feature; Reaper's preset menu MUST support user-saved presets, preset library import/export, and resetting to the plugin's default state.
- **FR-008**: Loading or switching a preset MUST automatically reset transient metering latches (`Max GR` peak hold and `CLIP` indicator) without altering saved preset parameters.

### Key Entities *(include if feature involves data)*

- **Preset State**: The complete snapshot of all user-configurable plugin parameters (compressor settings, sidechain filter, clipper settings, mix, and drum bus EQ controls) plus a version/format identifier for safe forward/backward compatibility.
- **Transient Telemetry State**: Real-time metering indicators (current gain reduction, peak hold, clip latch) that reflect live audio and are not treated as persistent preset parameters.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of Omore Punch's user-configurable parameters (all knobs and mode toggles) are preserved with exact fidelity across a save-preset, modify-controls, and recall-preset cycle in Reaper.
- **SC-002**: Recalling any saved preset updates all visible UI controls and audio behavior within one UI refresh cycle / audio buffer without requiring the user to reopen the plugin window.
- **SC-003**: Preset save, recall, reset-to-default, and same-format library export/import succeed in Reaper on both Linux and Windows (including Linux <-> Windows round-trips within VST3 and within CLAP).
- **SC-004**: Switching presets 10 times consecutively during active drum bus playback produces zero host crashes, zero non-finite output samples, and zero automatic track mutes.

## Assumptions

- Transient metering values (live input/output levels, peak gain reduction hold, and clip indicator latch) are runtime-only and are not saved as part of a preset.
- Preset management UI (preset list, save/delete/rename dialogs, `.rpl` / `.vstpreset` file handling) is provided by Reaper itself; Omore Punch does not need a custom in-plugin preset browser unless specified separately.
- Existing Reaper project state persistence and all sessions/presets saved with previous Omore Punch versions remain 100% backward-compatible alongside preset recall.
