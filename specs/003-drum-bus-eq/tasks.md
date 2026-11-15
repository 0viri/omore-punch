---

description: "Task list for feature 003-drum-bus-eq (Section EQ 3 bandes pour Bus Batterie)"
---

# Tasks: Drum Bus 3-Band EQ Section (`3. DRUM BUS EQ`)

**Input**: Design documents from `/specs/003-drum-bus-eq/`

**Prerequisites**: `plan.md` (required), `spec.md` (required for user stories), `research.md`, `data-model.md`, `contracts/ui-and-dsp-eq-contract.md`, `quickstart.md`

**Tests**: Tests unitaires numériques obligatoires dans `mod tests` de `src/lib.rs` (Principe V de la Constitution) couvrant la transparence au repos, le Low Cut (`12 dB/oct`), le 500 Hz Cut, le High Shelf (`-6` à `+12 dB`), le respect du plafond Clipper post-EQ, la stabilité multi-sample-rate (`44.1 / 48 / 96 kHz`) et la géométrie UI dans `820x620` px.

**Organization**: Les tâches sont groupées par User Story (`US1`, `US2`) afin de livrer d'abord un moteur EQ DSP complet et testé (MVP), puis l'intégration visuelle en potards ronds SSL et la documentation.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Exécutable en parallèle (fichiers distincts, aucune dépendance sur une tâche incomplète)
- **[Story]**: User Story associée (`[US1]`, `[US2]`)
- Chaque tâche mentionne le chemin exact du fichier concerné.

## Path Conventions

- **Code monolithique du plugin** : `src/lib.rs` (Blocs 1 `StereoPunchDsp`/`DspConfig`, 3 `OmorePunchParams`/`OmorePunch`, 4 `draw_omore_punch_ui`, et module `tests`)
- **Documentation du dépôt** : `README.md`, `USER_GUIDE.md`, `CHANGELOG.md` à la racine du dépôt

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Vérifier l'état initial de compilation, de conformité ASCII et de tests avant toute modification de `src/lib.rs`.

- [ ] T001 Verify baseline ASCII check (`LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs`) and release unit tests (`cargo test --release`) on `src/lib.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Mettre en place les structures de données biquad sans allocation et étendre `DspConfig` / `StereoPunchDsp` pour accueillir les 3 bandes EQ avant l'implémentation des User Stories.

**⚠️ CRITICAL**: Aucune modification des identifiants `#[id = "..."]` ou plages existantes dans `OmorePunchParams`. Tout code du bloc 1 doit être 100 % temps réel (zéro allocation, zéro verrou) et 100 % ASCII.

- [ ] T002 Define `BiquadCoeffs` (`b0: f32`, `b1: f32`, `b2: f32`, `a1: f32`, `a2: f32`, `bypass: bool`) and `StereoBiquad` (`coeffs: BiquadCoeffs`, `z1_l: f32`, `z2_l: f32`, `z1_r: f32`, `z2_r: f32`, plus `reset()` zeroing all 4 states) in block 1 of `src/lib.rs`
- [ ] T003 Add `eq_low_cut_hz: f32` (default `20.0`), `eq_mid_cut_db: f32` (default `0.0`), and `eq_high_shelf_db: f32` (default `0.0`) to `DspConfig`, and add `eq_low_cut: StereoBiquad`, `eq_mid_cut: StereoBiquad`, `eq_high_shelf: StereoBiquad` plus cached fields (`cached_eq_low_cut_hz: -1.0`, `cached_eq_mid_cut_db: -999.0`, `cached_eq_high_shelf_db: -999.0`) with reset and sample-rate cache invalidation in `StereoPunchDsp::default()`, `reset()`, and `set_sample_rate()` in block 1 of `src/lib.rs`

**Checkpoint**: Structures DSP de base prêtes — l'implémentation des User Stories peut commencer.

---

## Phase 3: User Story 1 - Shape Drum Bus Tone with 3 Focused EQ Controls (Priority: P1) 🎯 MVP

**Goal**: Implémenter le moteur EQ 3 bandes (`Low Cut` 12 dB/oct, `500Hz Cut` peaking, `High Shelf` 10 kHz) dans la chaîne `Compressor -> Drum Bus EQ -> Output Clipper -> Parallel Mix`, exposer les 3 paramètres DAW dans `OmorePunchParams` / `process()` et valider numériquement chaque bande par des tests unitaires.

**Independent Test**: Exécuter `cargo test --release` et vérifier que les nouveaux tests EQ (`test_eq_neutral_bypass_transparency`, `test_eq_low_cut_sub_attenuation`, `test_eq_500hz_cut_attenuation`, `test_eq_high_shelf_boost_and_cut`) ainsi que tous les tests existants passent sans erreur.

### Implementation & DSP Tests for User Story 1

- [x] T004 [US1] Implement `StereoPunchDsp::update_eq(&mut self, sample_rate: f32, low_cut_hz: f32, mid_cut_db: f32, high_shelf_db: f32)` in block 1 of `src/lib.rs` using Audio EQ Cookbook `f32` formulas (2nd-order Butterworth HPF `Q = 1/sqrt(2)` clamped `20.0..=160.0 Hz` with `bypass = true` when `<= 20.5 Hz`; Peaking EQ at `500.0 Hz` fixed, `Q = 1.0`, clamped `-12.0..=0.0 dB` with `bypass = true` when `abs < 1.0e-3`; 2nd-order High-Shelf at `10000.0 Hz` clamped to `0.45 * sample_rate`, `S = 1.0`, gain `-6.0..=12.0 dB` with `bypass = true` when `abs < 1.0e-3`), recomputing coefficients only when `sample_rate` changes (`> 1e-3`) or an EQ parameter changes (`> 1e-4`)
- [x] T005 [US1] Implement `StereoBiquad::process_sample` (Direct Form II Transposed with per-sample `NaN`/`Inf` and `< 1.0e-20 => 0.0` denormal protection on `z1_l, z2_l, z1_r, z2_r`, and strict bypass zeroing states when `coeffs.bypass`) and insert stage `5b. Drum Bus EQ` (`eq_low_cut` -> `eq_mid_cut` -> `eq_high_shelf`) in `StereoPunchDsp::process_sample` immediately after `comp_l`/`comp_r` and before stage `6. Final Clipper Stage` in block 1 of `src/lib.rs`
- [x] T006 [US1] Add `eq_low_cut_hz` (`#[id = "eq_low_cut_hz"]`, `"EQ Low Cut"`, default `20.0`, `FloatRange::Skewed { min: 20.0, max: 160.0, factor: FloatRange::skew_factor(-1.0) }`, `SmoothingStyle::Linear(5.0)`, unit `" Hz"`, `v2s_f32_rounded(1)`), `eq_mid_cut_db` (`#[id = "eq_mid_cut_db"]`, `"EQ 500Hz Cut"`, default `0.0`, `FloatRange::Linear { min: -12.0, max: 0.0 }`, `SmoothingStyle::Linear(5.0)`, unit `" dB"`, `v2s_f32_rounded(2)`), and `eq_high_shelf_db` (`#[id = "eq_high_shelf_db"]`, `"EQ High Shelf"`, default `0.0`, `FloatRange::Linear { min: -6.0, max: 12.0 }`, `SmoothingStyle::Linear(5.0)`, unit `" dB"`, `v2s_f32_rounded(2)`) to `OmorePunchParams` and populate `DspConfig` from `self.params.*.smoothed.next()` in `OmorePunch::process()` in block 3 of `src/lib.rs`
- [x] T007 [US1] Add EQ unit tests (`test_eq_neutral_bypass_transparency`, `test_eq_low_cut_sub_attenuation` across `44100.0`/`48000.0`/`96000.0` Hz, `test_eq_500hz_cut_attenuation`, `test_eq_high_shelf_boost_and_cut` including post-EQ clipper ceiling enforcement) and update existing `DspConfig` struct literals and `test_nan_inf_and_safety_clamp` in `mod tests` of `src/lib.rs`

**Checkpoint**: User Story 1 (MVP) est complète — l'EQ 3 bandes DSP fonctionne en temps réel, est automatable par le DAW et est validé par les tests unitaires.

---

## Phase 4: User Story 2 - Consistent Rotary Knob Interaction & Section Layout (Priority: P2)

**Goal**: Ajouter la section `3. DRUM BUS EQ` avec ses 3 potards rotatifs bleus SSL dans la colonne de gauche, renuméroter le metering à droite en `4. GAIN REDUCTION & STEREO BUS METERING`, ajuster la hauteur de fenêtre à `820x620` px sans débordement et mettre à jour la documentation.

**Independent Test**: Lancer `cargo test --release` (vérifiant `test_ui_layout_column_widths_fit_window` dans `820x620` px), charger le plugin dans Reaper et confirmer l'affichage des 3 sections à gauche, de la section `4.` à droite et de toutes les interactions des potards (`drag`, `Shift`, double-clic, `-`, `+`, `Reset`).

### UI Implementation, Geometry Test & Documentation for User Story 2

- [ ] T008 [US2] Update UI window height and meter height constants (`UI_WINDOW_HEIGHT: u32 = 620`, `UI_METER_HEIGHT: f32 = 390.0`) in block 4 of `src/lib.rs`
- [ ] T009 [US2] Add section `3. DRUM BUS EQ` below `2. OUTPUT CLIPPER & PARALLEL MIX` in the left column of `draw_omore_punch_ui` with one 3-knob row using `draw_ssl_knob_cell` (`&params.eq_low_cut_hz` `"Low Cut"` step `5.0`, `20.0..=160.0`; `&params.eq_mid_cut_db` `"500Hz Cut"` step `0.5`, `-12.0..=0.0`; `&params.eq_high_shelf_db` `"High Shelf"` step `0.5`, `-6.0..=12.0`; all with `SslKnobColor::Filter`) and renumber the right-column metering header to `"4. GAIN REDUCTION & STEREO BUS METERING"` in block 4 of `src/lib.rs`
- [ ] T010 [US2] Update `test_ui_layout_column_widths_fit_window` in `mod tests` of `src/lib.rs` to include `section3_height` (`group_header_and_margins_px + UI_KNOB_CELL_HEIGHT = 130.0` px) + `inter_section_gap_px` (`6.0` px) in `total_left_col_height` (`552.0 <= 572.0` px) and verify empirical `egui` rendering bounds inside `820x620` px
- [ ] T011 [P] [US2] Document the `3. DRUM BUS EQ` section (`Low Cut`, `500Hz Cut`, `High Shelf`) and updated `4.` metering section in `README.md`
- [ ] T012 [P] [US2] Add Drum Bus EQ usage instructions (`Low Cut`, `500Hz Cut`, `High Shelf`) in `USER_GUIDE.md`
- [ ] T013 [P] [US2] Add an entry under `## [Unreleased]` (`### Ajouté`) describing the 3-band Drum Bus EQ section (`003-drum-bus-eq`) in `CHANGELOG.md`

**Checkpoint**: User Stories 1 et 2 sont complètes — le DSP, l'UI en potards ronds SSL `820x620` px et la documentation sont alignés.

---

## Phase 5: Polish & Cross-Cutting Concerns

**Purpose**: Exécuter l'audit temps réel et tous les contrôles qualité obligatoires (ASCII, tests Release, bundles VST3/CLAP).

- [ ] T014 Audit `StereoPunchDsp` and `OmorePunch::process()` in `src/lib.rs` against Constitution Principles III & IV (zero heap allocation, zero locks, cached coefficients, `NaN`/`Inf`/denormal protection, `sanitize_sample()` final clamp)
- [ ] T015 Run strict ASCII check `LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs` on `src/lib.rs` (must return no output)
- [ ] T016 Run full release unit test suite `cargo test --release` on `src/lib.rs` (must pass with zero compiler warnings)
- [ ] T017 Build VST3 and CLAP bundles with `cargo run --package xtask --release -- bundle omore-punch --release` and confirm `target/bundled/omore-punch.vst3` and `target/bundled/omore-punch.clap`
- [ ] T018 Verify automated checklist items 1-3 in `specs/003-drum-bus-eq/quickstart.md` and note manual DAW verification (step 4 in Reaper)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: Aucune dépendance — démarre immédiatement (`T001`).
- **Foundational (Phase 2)**: Dépend de `T001` — crée `BiquadCoeffs`, `StereoBiquad` et les champs EQ de `DspConfig` / `StereoPunchDsp` (`T002` -> `T003`).
- **User Story 1 (Phase 3 - P1 🎯 MVP)**: Dépend de `T003` — implémente le calcul des coefficients EQ, le filtrage stéréo pré-clipper, les paramètres DAW `OmorePunchParams` et les tests unitaires DSP (`T004` -> `T005` -> `T006` -> `T007`).
- **User Story 2 (Phase 4 - P2)**:
  - `T008`, `T009` et `T010` modifient `src/lib.rs` et s'exécutent séquentiellement après `T006`/`T007` (`T008` -> `T009` -> `T010`).
  - `T011` (`README.md`), `T012` (`USER_GUIDE.md`) et `T013` (`CHANGELOG.md`) touchent des fichiers distincts et peuvent s'exécuter en parallèle (`[P]`) une fois `T009` stabilisé.
- **Polish (Phase 5)**: Dépend de la complétion des Phases 3 et 4 (`T014` -> `T015` -> `T016` -> `T017` -> `T018`).

### User Story Dependencies

- **User Story 1 (P1)**: Indépendante après Phase 2 — livre le moteur DSP EQ complet, automatable par le DAW et vérifié numériquement (MVP).
- **User Story 2 (P2)**: S'appuie sur les 3 `FloatParam` créés dans `T006` (`US1`) pour afficher `3. DRUM BUS EQ` dans l'UI `egui` et renuméroter la section `4.`.

### Parallel Opportunities

- Toutes les tâches modifiant `src/lib.rs` (`T002` à `T010`) doivent être exécutées séquentiellement en raison de l'architecture monolithique imposée par le Principe I de la Constitution.
- Dans la **Phase 4 (US2)**, les tâches documentaires `T011` (`README.md`), `T012` (`USER_GUIDE.md`) et `T013` (`CHANGELOG.md`) concernent trois fichiers distincts et sont parallélisables (`[P]`).

---

## Parallel Example: User Story 2

```bash
# Les mises à jour documentaires de US2 peuvent être lancées en parallèle :
Task T011: "Document the 3. DRUM BUS EQ section and updated 4. metering section in README.md"
Task T012: "Add Drum Bus EQ usage instructions in USER_GUIDE.md"
Task T013: "Add an entry under ## [Unreleased] (### Ajouté) in CHANGELOG.md"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Compléter Phase 1 (`T001`) et Phase 2 (`T002`–`T003`).
2. Compléter Phase 3 (`T004`–`T007`) : moteur DSP biquad 3 bandes pré-clipper, paramètres `OmorePunchParams` et tests unitaires numériques.
3. **STOP & VALIDATE** : Lancer `cargo test --release` et vérifier que les 3 bandes EQ répondent conformément au contrat DSP.

### Incremental Delivery

1. Ajouter Phase 4 (`T008`–`T013`) : fenêtre `820x620` px, section `3. DRUM BUS EQ` avec 3 potards ronds `SslKnobColor::Filter`, renumérotation `4.` du metering, test de géométrie `egui` et mise à jour de `README.md`, `USER_GUIDE.md`, `CHANGELOG.md`.
2. Exécuter Phase 5 (`T014`–`T018`) : audit temps réel, contrôle ASCII strict, `cargo test --release`, bundling VST3/CLAP via `xtask` et validation finale dans Reaper.

---

## Notes

- Les tâches marquées `[P]` portent sur des fichiers différents et n'ont pas de dépendance mutuelle.
- Toute chaîne ajoutée dans `src/lib.rs` (commentaires, étiquettes UI, noms de paramètres, assertions de tests) DOIT rester 100 % ASCII (Principe II).
- Aucune allocation ni verrou sur le thread audio (`StereoPunchDsp` / `process()`, Principes III et IV).
