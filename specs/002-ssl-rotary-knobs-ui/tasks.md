---

description: "Task list for feature 002-ssl-rotary-knobs-ui (Interface type SSL avec potentiometres rotatifs en grille)"
---

# Tasks: Interface type SSL avec potentiomètres rotatifs en grille

**Input**: Design documents from `/specs/002-ssl-rotary-knobs-ui/`

**Prerequisites**: `plan.md` (required), `spec.md` (required for user stories), `research.md`, `data-model.md`, `contracts/ui-ssl-rotary-knob-contract.md`, `quickstart.md`

**Tests**: Mise à jour du test unitaire de géométrie UI dans `mod tests` de `src/lib.rs` (critère `SC-002`) et exécution complète de la suite `cargo test --release` (Principe V de la Constitution).

**Organization**: Les tâches sont groupées par User Story (`US1`, `US2`, `US3`) afin de permettre une implémentation et une vérification incrémentales dans le fichier monolithique `src/lib.rs`.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Exécutable en parallèle (fichiers distincts, aucune dépendance sur une tâche incomplète)
- **[Story]**: User Story associée (`[US1]`, `[US2]`, `[US3]`)
- Chaque tâche mentionne le chemin exact du fichier concerné.

## Path Conventions

- **Code monolithique du plugin** : `src/lib.rs` (bloc 4 `draw_omore_punch_ui` et bloc `tests`)
- **Documentation du dépôt** : `README.md`, `USER_GUIDE.md`, `CHANGELOG.md` à la racine du dépôt

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Vérifier l'état initial de compilation, de conformité ASCII et de tests avant toute modification de l'UI.

- [X] T001 Verify baseline ASCII check (`LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs`) and release unit tests (`cargo test --release`) on `src/lib.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Verrouiller les invariants de rétrocompatibilité DAW et déclarer les constantes géométriques de la grille de potentiomètres dans `src/lib.rs`.

**⚠️ CRITICAL**: Aucune implémentation de User Story ne doit modifier les identifiants `#[id]`, plages `FloatRange`, valeurs par défaut ou le moteur DSP `StereoPunchDsp`.

- [X] T002 Verify parameter and window backward-compatibility invariants (`#[id]` strings `"input_db"`, `"threshold_db"`, `"ratio"`, `"attack_ms"`, `"release_ms"`, `"sc_hpf_hz"`, `"auto_makeup"`, `"makeup_db"`, `"clipper_mode"`, `"ceiling_db"`, `"mix_pct"`, `"gr_meter_db"`, all `FloatRange` min/max/skew factors, default plain values, and `EguiState::from_size(820, 480)`) in `src/lib.rs`
- [X] T003 Define SSL rotary knob grid geometry constants (`UI_KNOB_COLS_MAX: usize = 4`, `UI_KNOB_CELL_WIDTH: f32 = 110.0`, `UI_KNOB_CELL_HEIGHT: f32 = 94.0`, `UI_KNOB_DIAMETER: f32 = 44.0`, `UI_KNOB_GRID_SPACING_X: f32 = 8.0`, `UI_KNOB_GRID_SPACING_Y: f32 = 4.0`, `UI_KNOB_STEP_BTN_WIDTH: f32 = 22.0`, `UI_KNOB_RESET_BTN_WIDTH: f32 = 46.0`) in block 4 (`draw_omore_punch_ui`) of `src/lib.rs`

**Checkpoint**: Invariants vérifiés et budget géométrique défini — l'implémentation des User Stories peut commencer.

---

## Phase 3: User Story 1 - Réglage visuel et ergonomique par potentiomètres rotatifs en grille (Priority: P1) 🎯 MVP

**Goal**: Remplacer les 9 curseurs horizontaux (`draw_param_row`) par des potentiomètres rotatifs vectoriels (`draw_ssl_knob_cell`) disposés en grille dans les sections `1.` et `2.`, pilotés par glisser vertical dans l'espace normalisé `[0.0, 1.0]` et validés sans débordement dans `820x480` px.

**Independent Test**: Compiler et exécuter `cargo test --release` (vérifiant `test_ui_layout_column_widths_fit_window`), ouvrir l'éditeur dans Reaper et vérifier que les 9 paramètres apparaissent en grille (4+3 en section 1, 2 en section 2) avec couronne graduée sur 270 deg, capuchon, repère angulaire et glisser vertical actif.

### Implementation & Geometry Test for User Story 1

- [X] T004 [US1] Implement `draw_ssl_knob_cell` vector rendering in block 4 of `src/lib.rs` using `egui::Painter` (`UI_KNOB_DIAMETER = 44.0` px knob rect, 11 radial ticks over 270 deg from -135 deg to +135 deg, dark outer skirt `circle_filled`/`circle_stroke`, circular cap, angular pointer `line_segment` driven by `param.modulated_normalized_value()`, centered 100% ASCII `monospace()` label above, and centered `param.to_string()` readout in `rgb(245, 185, 45)` below inside `UI_KNOB_CELL_WIDTH = 110.0` x `UI_KNOB_CELL_HEIGHT = 94.0`)
- [X] T005 [US1] Implement vertical mouse drag interaction in `draw_ssl_knob_cell` in `src/lib.rs` (`Sense::click_and_drag()`, upward drag maps `-resp.drag_delta().y / 150.0` onto normalized `[0.0, 1.0]` via `param.modulated_normalized_value()`, converts via `param.preview_plain(next_norm).clamp(min_val, max_val)`, and emits `setter.begin_set_parameter(param)` on `drag_started()`, `setter.set_parameter(param, next_plain)` on drag delta, and `setter.end_set_parameter(param)` on `drag_stopped()`)
- [X] T006 [US1] Replace all 9 `draw_param_row` calls and remove `draw_param_row` (and obsolete `UI_SLIDER_WIDTH`/`UI_STEP_BTN_WIDTH`/`UI_RESET_BTN_WIDTH`/`UI_ROW_HEIGHT`/`UI_ROW_ITEM_SPACING` constants if unused) by laying out section `1. COMPRESSOR & ENVELOPE BALLISTICS` as two knob rows (`Input Drive`, `Threshold`, `Ratio`, `Attack` on row 1; `Release`, `SC HPF`, `Makeup Gain` on row 2) above the quick-action row, and section `2. OUTPUT CLIPPER & PARALLEL MIX` as one knob row (`Clip Ceiling`, `Mix Dry/Wet`) below the clipper mode buttons inside `draw_omore_punch_ui` in `src/lib.rs`
- [X] T007 [US1] Update `test_ui_layout_column_widths_fit_window` in `mod tests` of `src/lib.rs` to assert that total column widths fit `UI_WINDOW_WIDTH` (`820`), the 4-knob row width (`4.0 * UI_KNOB_CELL_WIDTH + 3.0 * UI_KNOB_GRID_SPACING_X + group_margins_px`) fits `UI_LEFT_COL_WIDTH` (`480.0`), and the combined left-column section heights (including group titles, separators, button rows, and inter-section gap) fit within `UI_WINDOW_HEIGHT as f32 - header_and_panel_margins_px` (where `header_and_panel_margins_px = 48.0`, leaving `432.0` px) without vertical overflow

**Checkpoint**: User Story 1 (MVP) est complète — les 9 curseurs horizontaux sont remplacés par la grille de boutons ronds vectoriels pilotables à la souris dans `820x480` px.

---

## Phase 4: User Story 2 - Réglage fin pas-à-pas et remise à la valeur par défaut (Priority: P2)

**Goal**: Garantir un contrôle de précision déterministe et une réinitialisation immédiate sur chaque potentiomètre via `Shift`+glisser fin, double-clic reset et les trois petits boutons compacts `[-]`, `[+]` et `[Reset]`.

**Independent Test**: Sur chaque potentiomètre, vérifier que `Shift`+glisser ralentit la course d'un facteur 4 (`600.0` px), qu'un clic sur `[-]` / `[+]` applique le pas exact (`0.5`, `0.25`, `5.0`) borné à `[min_val, max_val]`, et qu'un double-clic sur le bouton ou un clic sur `[Reset]` rétablit `param.default_plain_value()`.

### Implementation for User Story 2

- [X] T008 [US2] Add `Shift`-modified fine vertical drag (`sensitivity = 600.0` when `ui.input(|i| i.modifiers.shift)` else `150.0`) and double-click reset (`if resp.double_clicked()` calling `setter.begin_set_parameter(param)`, `setter.set_parameter(param, param.default_plain_value())`, `setter.end_set_parameter(param)`) to `draw_ssl_knob_cell` in `src/lib.rs`
- [X] T009 [US2] Add the centered compact button row (`UI_KNOB_STEP_BTN_WIDTH = 22.0` x `16.0` px `"-"` and `"+"` buttons applying `step` clamped to `[min_val, max_val]`, plus `UI_KNOB_RESET_BTN_WIDTH = 46.0` x `16.0` px `"Reset"` button applying `param.default_plain_value()`, all wrapped in `begin_set_parameter`/`set_parameter`/`end_set_parameter` with 100% ASCII hover tooltips) below the numeric readout in `draw_ssl_knob_cell` in `src/lib.rs`

**Checkpoint**: User Stories 1 et 2 sont fonctionnelles — glisser vertical, réglage fin sous `Shift`, pas-à-pas `-`/`+` et remise à zéro par double-clic ou `[Reset]` sont tous opérationnels.

---

## Phase 5: User Story 3 - Identité visuelle type SSL et préservation des raccourcis et du metering (Priority: P3)

**Goal**: Appliquer le codage couleur SSL par famille fonctionnelle sur les capuchons, conserver le contraste du thème sombre `Omore`, vérifier la cohérence avec les préréglages rapides et le metering de droite, et synchroniser la documentation utilisateur.

**Independent Test**: Vérifier visuellement les 5 couleurs de capuchon SSL, cliquer sur `[Punch 3ms / 80ms]` et `[Snare Safe 200Hz]` pour constater la rotation immédiate des boutons `Attack`, `Release` et `SC HPF`, vérifier les vu-mètres `In L/R`, `Out L/R` et `GR` à droite, et relire `README.md`, `USER_GUIDE.md` et `CHANGELOG.md`.

### Implementation & Documentation for User Story 3

- [X] T010 [US3] Add `enum SslKnobColor` (`GainDrive` -> `rgb(215, 145, 30)`, `Dynamics` -> `rgb(150, 158, 170)`, `Envelope` -> `rgb(55, 155, 95)`, `Filter` -> `rgb(55, 125, 195)`, `ClipperMix` -> `rgb(195, 65, 60)`) with hover/active cap highlight and active graduated-tick illumination, and pass the designated `SslKnobColor` variant for all 9 `draw_ssl_knob_cell` calls in `draw_omore_punch_ui` in `src/lib.rs`
- [X] T011 [US3] Audit and tune vertical spacing/alignment across the header (`OMORE PUNCH v<VERSION>` + `[CLIP: OK]`), quick-action row (`[Punch 3ms / 80ms]`, `[Snare Safe 200Hz]`, `[Auto Makeup: ON/OFF]`), clipper mode selector (`[Soft Clip]`, `[Hard Clip]`, `[Bypass]`), and right-column metering (`In L/R`, `Out L/R`, `GR` meter) inside `820x480` px in `draw_omore_punch_ui` in `src/lib.rs`
- [X] T012 [P] [US3] Update plugin UI description to mention the SSL-style color-coded rotary knob grid layout in `README.md`
- [X] T013 [P] [US3] Update parameter control documentation (replacing the horizontal `Slider` bullet with SSL-style rotary knobs supporting vertical drag, `Shift` fine adjust, double-click reset, and compact `-`, `+`, `Reset` buttons) in `USER_GUIDE.md`
- [X] T014 [P] [US3] Add an entry under `## [Unreleased]` (`### Modifié`) describing the SSL-style rotary knob grid UI (`002-ssl-rotary-knobs-ui`) in `CHANGELOG.md`

**Checkpoint**: Toutes les User Stories (`US1`, `US2`, `US3`) sont implémentées et la documentation est synchronisée.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Exécuter tous les contrôles qualité obligatoires (ASCII, tests Release, bundles VST3/CLAP, validation finale).

- [X] T015 Run strict ASCII check `LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs` on `src/lib.rs` (must return no output)
- [X] T016 Run full release test suite `cargo test --release` on `src/lib.rs` (verifying all DSP tests and `test_ui_layout_column_widths_fit_window`)
- [X] T017 Build VST3 and CLAP bundles with `cargo run --package xtask --release -- bundle omore-punch --release` and confirm `target/bundled/omore-punch.vst3` and `target/bundled/omore-punch.clap`
- [X] T018 Verify automated checklist items 1-3 in `specs/002-ssl-rotary-knobs-ui/quickstart.md` with zero compiler warnings in `src/lib.rs`, and hand off step 4 (visual/mouse verification inside Reaper) as manual DAW verification

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: Aucune dépendance — démarre immédiatement (`T001`).
- **Foundational (Phase 2)**: Dépend de `T001` — vérifie les invariants DAW/DSP et pose les constantes géométriques (`T002` -> `T003`).
- **User Story 1 (Phase 3 - P1 🎯 MVP)**: Dépend de `T003` — implémente `draw_ssl_knob_cell`, le glisser vertical, la disposition en grille des 9 boutons et le test géométrique (`T004` -> `T005` -> `T006` -> `T007`).
- **User Story 2 (Phase 4 - P2)**: Dépend de `T006` — ajoute le réglage fin `Shift`, le double-clic reset et les boutons compacts `-`/`+`/`Reset` dans `src/lib.rs` (`T008` -> `T009`).
- **User Story 3 (Phase 5 - P3)**:
  - `T010` et `T011` touchent `src/lib.rs` et s'exécutent séquentiellement après `T009`.
  - `T012` (`README.md`), `T013` (`USER_GUIDE.md`) et `T014` (`CHANGELOG.md`) touchent des fichiers distincts et peuvent s'exécuter en parallèle (`[P]`) une fois l'interface stabilisée.
- **Polish (Phase 6)**: Dépend de la complétion des Phases 3, 4 et 5 (`T015` -> `T016` -> `T017` -> `T018`).

### User Story Dependencies

- **User Story 1 (P1)**: Indépendante après Phase 2 — constitue le MVP livrable (grille de 9 potentiomètres rotatifs pilotables à la souris sans débordement).
- **User Story 2 (P2)**: Enrichit `draw_ssl_knob_cell` (`US1`) avec les interactions de précision (`Shift`, `-`, `+`, `Reset`, double-clic).
- **User Story 3 (P3)**: Enrichit `draw_ssl_knob_cell` et `draw_omore_punch_ui` (`US1`) avec le codage couleur SSL (`SslKnobColor`), vérifie la cohabitation avec les raccourcis et le metering, et met à jour la documentation.

### Parallel Opportunities

- Toutes les tâches modifiant `src/lib.rs` (`T002` à `T011`) doivent être exécutées séquentiellement en raison de l'architecture monolithique imposée par le Principe I de la Constitution.
- Dans la **Phase 5 (US3)**, les tâches documentaires `T012` (`README.md`), `T013` (`USER_GUIDE.md`) et `T014` (`CHANGELOG.md`) concernent trois fichiers distincts et sont parallélisables (`[P]`).

---

## Parallel Example: User Story 3

```bash
# Les mises à jour documentaires de US3 peuvent être lancées en parallèle :
Task T012: "Update plugin UI description to mention the SSL-style color-coded rotary knob grid layout in README.md"
Task T013: "Update parameter control documentation in USER_GUIDE.md"
Task T014: "Add an entry under ## [Unreleased] (### Modifié) in CHANGELOG.md"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Compléter Phase 1 (`T001`) et Phase 2 (`T002`–`T003`).
2. Compléter Phase 3 (`T004`–`T007`) : widget vectoriel `draw_ssl_knob_cell`, glisser vertical, grille 4+3 / 2 dans `draw_omore_punch_ui`, et test unitaire de géométrie `820x480` px.
3. **STOP & VALIDATE** : Lancer `cargo test --release` et vérifier que la grille compile et rentre dans la fenêtre sans débordement.

### Incremental Delivery

1. Ajouter Phase 4 (`T008`–`T009`) : `Shift`+glisser fin, double-clic reset et boutons compacts `-`, `+`, `Reset`.
2. Ajouter Phase 5 (`T010`–`T014`) : codage couleur SSL `SslKnobColor`, audit visuel global et documentation (`README.md`, `USER_GUIDE.md`, `CHANGELOG.md`).
3. Exécuter Phase 6 (`T015`–`T018`) : contrôle ASCII strict, `cargo test --release`, bundling VST3/CLAP via `xtask` et validation finale dans Reaper.

---

## Notes

- Les tâches marquées `[P]` portent sur des fichiers différents et n'ont pas de dépendance mutuelle.
- Toute chaîne ajoutée dans `src/lib.rs` (y compris étiquettes et infobulles) DOIT rester 100 % ASCII (Principe II).
- Aucune modification n'est apportée au thread audio `process()` ni à `StereoPunchDsp` (Principes III et IV).
