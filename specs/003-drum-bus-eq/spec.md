# Feature Specification: Drum Bus 3-Band EQ Section

**Feature Branch**: `feat/003-drum-bus-eq`

**Created**: 2026-11-14

**Status**: Draft

**Input**: User description: "Ajouter une section EQ (3. ?), avec des potards ronds toujours, qui fait uniquement 3 choses : réduire la fréquence des 500Hz, faire un high shelf et un low cut"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Shape Drum Bus Tone with 3 Focused EQ Controls (Priority: P1)

As a mix engineer using Omore Punch on a stereo drum bus, I want a dedicated 3-function EQ section with rotary knobs so that I can clean up sub-bass rumble (low cut), tame boxiness around 500 Hz (500 Hz cut), and add top-end air/presence (high shelf) directly inside the plugin without inserting a separate EQ plugin.

**Why this priority**: This is the core value of the feature: giving the user the exact three drum-bus tonal adjustments requested in a fast, uncluttered workflow.

**Independent Test**: Can be fully tested by feeding a full-range drum bus test signal through the plugin, adjusting each of the three EQ knobs independently from its neutral position, and verifying the expected frequency attenuation or boost on the stereo output.

**Acceptance Scenarios**:

1. **Given** all three EQ controls are at their neutral/default positions, **When** audio passes through the plugin, **Then** the EQ introduces no audible coloration or level change (transparent bypass behavior).
2. **Given** the Low Cut control is raised above its 20.0 Hz minimum/bypass setting, **When** low-frequency drum energy (e.g., sub-bass rumble below the cutoff) passes through, **Then** frequencies below the cutoff are attenuated smoothly on both left and right channels.
3. **Given** the 500 Hz Cut control is turned down below 0 dB, **When** midrange energy around 500 Hz passes through, **Then** the 500 Hz region is attenuated proportionally to the selected dB reduction while leaving unrelated frequency bands intact.
4. **Given** the High Shelf control is adjusted away from 0 dB, **When** high-frequency transients (cymbals, snare wires, stick attack) pass through, **Then** frequencies above the shelf corner are boosted (or cut, if bipolar) according to the control value.

---

### User Story 2 - Consistent Rotary Knob Interaction & Section Layout (Priority: P2)

As a user already familiar with Omore Punch's console-style interface, I want the new EQ section to appear as a clearly numbered section (`3. DRUM BUS EQ`) using the same rotary knobs, numeric readouts, fine adjustment, and reset gestures as the compressor and clipper sections.

**Why this priority**: Visual and ergonomic consistency ensures the new section feels native to the plugin, fits cleanly in the editor window without overlap, and supports DAW automation and parameter recall.

**Independent Test**: Can be tested by opening the plugin editor, verifying that the EQ section and its three rotary knobs are visible without overlapping existing controls or meters, and confirming mouse drag, fine drag, double-click reset, `-`/`+`/`Reset` buttons, and DAW preset save/recall.

**Acceptance Scenarios**:

1. **Given** the plugin editor is open, **When** the user views the control panels, **Then** the EQ section appears as section 3 with three color-coded rotary knobs, numeric readouts, and compact adjustment/reset controls, and the metering panel remains clearly visible without overlapping.
2. **Given** the user adjusts any EQ rotary knob, **When** the user saves and reloads a DAW project or switches presets, **Then** all three EQ parameter values are restored accurately and remain automatable.

---

### Edge Cases

- What happens when the user sends extreme or non-finite audio (silence, full-scale transients, NaN/Inf) into the EQ filters? The filters must remain stable, produce no runaway resonance or denormal CPU spikes, and output finite bounded audio.
- What happens when the host sample rate changes (e.g., 44.1 kHz, 48 kHz, 96 kHz)? The EQ filter frequencies and slopes must adapt accurately to the active sample rate without clicks or tuning drift.
- What happens when the High Shelf boosts high frequencies into the output clipper? Because the EQ sits post-compressor and immediately pre-clipper (`Compressor -> Drum Bus EQ -> Output Clipper -> Parallel Mix`), any added high-frequency peaks are caught cleanly by the output clipper and remain bounded by the clipper ceiling, while the `0%` parallel mix dry path remains bit-exact.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST provide a dedicated EQ section limited strictly to three tonal functions: a Low Cut (high-pass) filter, a 500 Hz cut, and a High Shelf.
- **FR-002**: System MUST position the EQ in the stereo audio signal chain post-compressor and immediately pre-clipper (`Compressor -> Drum Bus EQ -> Output Clipper -> Parallel Mix`), so it shapes the compressed drum bus without affecting the compressor detector, lets the clipper catch any peaks added by the High Shelf, and preserves the bit-exact dry path at `0%` parallel mix.
- **FR-003**: System MUST provide exactly three rotary knob controls in the EQ section:
  - **Low Cut**: High-pass filter cutoff frequency from 20.0 Hz (minimum, bypassed at <= 20.5 Hz) up to 160.0 Hz (default: 20.0 Hz, bypassed).
  - **500 Hz Cut**: Cut-only peaking attenuation centered at 500 Hz from 0.0 dB down to -12.0 dB (default: 0.0 dB).
  - **High Shelf**: High-shelf gain centered at a fixed musical 10 kHz corner frequency from -6.0 dB to +12.0 dB (default: 0.0 dB).
- **FR-004**: Each EQ control MUST default to a transparent neutral state (Low Cut at 20.0 Hz bypassed, 500 Hz Cut at 0.0 dB, High Shelf at 0.0 dB) so existing mix balance is unchanged until adjusted.
- **FR-005**: System MUST process both stereo channels identically using matched stereo filter instances so the stereo image and phase relationship of the drum bus remain centered.
- **FR-006**: System MUST expose all EQ parameters to the host DAW for automation and state persistence (save/recall).
- **FR-007**: The user interface MUST present the EQ as a numbered section (`3. DRUM BUS EQ`) in the left control column using the same rotary knob style (graduated ring, color-coded cap, angular pointer, vertical drag, fine-drag modifier, double-click reset, formatted readout, and compact `-`/`+`/`Reset` buttons) and renumber the right-column metering section to `4. GAIN REDUCTION & STEREO BUS METERING`.
- **FR-008**: All UI labels, values, and section headers MUST use strictly ASCII characters and monospace typography matching the existing interface.
- **FR-009**: Filter state and output MUST remain finite, click-free on parameter changes, and safe against non-finite inputs or sample-rate changes.

### Key Entities *(include if feature involves data)*

- **EQ Parameters**: The user-facing, automatable settings for Low Cut, 500 Hz Cut, and High Shelf, including their default neutral states and formatted display units (Hz, dB).
- **Stereo EQ Processor**: The stereo-matched filter bank (high-pass, 500 Hz peaking cut, high-shelf) operating identically on Left and Right channels.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: With all three EQ controls at their default neutral positions, the processed audio matches the pre-EQ audio within transparent tolerance (no unintended gain or tonal shift).
- **SC-002**: Adjusting each of the three EQ controls produces the expected measurable attenuation or boost in its target frequency band (sub-bass for Low Cut, 500 Hz for the mid cut, upper frequencies for the High Shelf) across standard sample rates (44.1 kHz, 48 kHz, 96 kHz).
- **SC-003**: Users can identify and adjust all three EQ rotary knobs in the plugin window without any overlapping UI elements or clipped text.
- **SC-004**: All existing plugin tests and new EQ numerical verification tests pass, and VST3/CLAP bundles build cleanly on both Linux and Windows.

## Assumptions

- The 500 Hz cut uses a musical drum-bus bandwidth (Q around 1.0 to 1.4) suitable for removing boxy snare/tom/room buildup without sounding overly surgical or hollow.
- The Low Cut uses a clean 12 dB/oct (2nd-order) high-pass slope, which is standard for drum bus sub-rumble control without excessive phase shift.
- Adding the `3. DRUM BUS EQ` section in the left control column requires renumbering the right-column metering section from `3.` to `4.` so section numbers remain sequential.
- The editor window height may be adjusted if needed so all three left-column sections fit comfortably without scrolling or overlap.
