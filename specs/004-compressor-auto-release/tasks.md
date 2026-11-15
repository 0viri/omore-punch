---

description: "Task list for feature 004-compressor-auto-release (Compressor Auto-Release)"
---

# Tasks: Compressor Auto-Release (`004-compressor-auto-release`)

**Input**: Design documents from `/specs/004-compressor-auto-release/`

**Prerequisites**: `plan.md` (required), `spec.md` (required for user stories), `research.md`, `data-model.md`, `contracts/ui-and-dsp-auto-release-contract.md`, `quickstart.md`

**Tests**: Tests unitaires numériques obligatoires dans `mod tests` de `src/lib.rs` (Principe V de la Constitution) couvrant le comportement transitoire court vs signal soutenu sur les 3 profils (`AutoTight`, `AutoPunch`, `AutoGlue`) et plusieurs fréquences d'échantillonnage (`44.1 / 48 / 96 kHz`), l'ordre strict des vitesses de récupération, la compatibilité bit-à-bit en mode `Manual`, la continuité sans clic ni divergence L/R lors des bascules de mode en compression active, la sécurité `NaN`/`Inf`/dénormaux, et la géométrie UI dans `820x650` px.

**Organization**: Les tâches sont groupées par User Story (`US1`, `US2`, `US3`) afin de livrer d'abord un moteur DSP Auto-Release complet et testé (MVP), puis l'intégration visuelle dans l'interface `egui` + documentation, puis les tests d'automation/rappel/stabilité en bascule.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Exécutable en parallèle (fichiers distincts, aucune dépendance sur une tâche incomplète)
- **[Story]**: User Story associée (`[US1]`, `[US2]`, `[US3]`) — uniquement dans les phases User Story
- Chaque tâche mentionne le chemin exact du fichier concerné.

## Path Conventions

- **Code monolithique du plugin** : `src/lib.rs` (Blocs 1 `StereoPunchDsp`/`DspConfig`, 3 `OmorePunchParams`/`OmorePunch`, 4 `draw_omore_punch_ui`, et module `tests`)
- **Documentation du dépôt** : `README.md`, `USER_GUIDE.md`, `CHANGELOG.md` à la racine du dépôt

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Vérifier l'état initial de conformité ASCII et de tests avant toute modification de `src/lib.rs`.

- [X] T001 Verify baseline ASCII check (`LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs` returns nothing) and release unit tests (`cargo test --release`) on `src/lib.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Définir l'énumération `ReleaseMode`, étendre `DspConfig` / `StereoPunchDsp` et exposer le paramètre `EnumParam<ReleaseMode>` dans `OmorePunchParams` / `OmorePunch::process()` afin de débloquer indépendamment `US1`, `US2` et `US3`.

**⚠️ CRITICAL**: Aucune modification des identifiants `#[id = "..."]` ou plages existantes dans `OmorePunchParams`. Tout code des blocs 1 et 3 doit être 100 % temps réel (zéro allocation, zéro verrou) et 100 % ASCII pur.

- [X] T002 Define `ReleaseMode` enum (`#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug)]`) with variants `Manual` (`#[id = "manual"]`, `#[name = "Manual"]`), `AutoTight` (`#[id = "auto_tight"]`, `#[name = "Auto Tight (50/600 ms)"]`), `AutoPunch` (`#[id = "auto_punch"]`, `#[name = "Auto Punch (80/800 ms)"]`), `AutoGlue` (`#[id = "auto_glue"]`, `#[name = "Auto Glue (100/1200 ms)"]`) and `pub fn auto_time_constants_ms(self) -> Option<(f32, f32)>` returning `None`, `Some((50.0, 600.0))`, `Some((80.0, 800.0))`, `Some((100.0, 1200.0))`, add `pub release_mode: ReleaseMode` (default `ReleaseMode::Manual`) to `DspConfig`, and update explicit `DspConfig { ... }` test literals in `src/lib.rs`
- [X] T003 Add `cached_release_mode: ReleaseMode`, `auto_coeffs_initialized: bool`, `auto_fast_release_coeff: f32`, `auto_slow_release_coeff: f32`, `auto_slow_charge_coeff: f32`, and `auto_slow_gr_db: f32` (default `0.0`) to `StereoPunchDsp` in block 1 of `src/lib.rs`, resetting `auto_slow_gr_db = 0.0` in `StereoPunchDsp::reset()` and invalidating `auto_coeffs_initialized = false` in `StereoPunchDsp::set_sample_rate()`
- [X] T004 Add `#[id = "release_mode"] pub release_mode: EnumParam<ReleaseMode>` (`EnumParam::new("Release Mode", ReleaseMode::Manual)`) to `OmorePunchParams` and pass `release_mode: self.params.release_mode.value()` into `DspConfig` inside `OmorePunch::process()` in block 3 of `src/lib.rs`

**Checkpoint**: Infrastructure partagée (`ReleaseMode`, état `StereoPunchDsp`, paramètre DAW `release_mode`) prête — les User Stories 1, 2 et 3 peuvent être implémentées.

---

## Phase 3: User Story 1 - Program-Dependent Auto-Release on the Drum Bus (Priority: P1) 🎯 MVP

**Goal**: Implémenter la balistique Auto-Release à double étage dépendant du programme (`AutoTight` 50/600 ms, `AutoPunch` 80/800 ms, `AutoGlue` 100/1200 ms, charge lente `180 ms`) dans `StereoPunchDsp` tout en conservant un comportement bit-à-bit inchangé en `Manual`, et valider numériquement le comportement DSP.

**Independent Test**: Exécuter `cargo test --release` et vérifier que `test_auto_release_transient_vs_sustained_recovery`, `test_auto_release_profile_ordering` et `test_auto_release_manual_bit_exact_compatibility` (ainsi que tous les tests existants) passent sans erreur.

### Implementation & DSP Tests for User Story 1

- [X] T005 [US1] Extend `StereoPunchDsp::update_ballistics(&mut self, sample_rate: f32, attack_ms: f32, release_ms: f32, release_mode: ReleaseMode)` (and update its calls in `StereoPunchDsp::default()`, `set_sample_rate()`, and `process_sample()`) in block 1 of `src/lib.rs` so `.exp()` coefficients `auto_slow_charge_coeff` (fixed `180.0 ms`), `auto_fast_release_coeff` (`50.0 / 80.0 / 100.0 ms`, fallback `80.0 ms` in `Manual`), and `auto_slow_release_coeff` (`600.0 / 800.0 / 1200.0 ms`, fallback `800.0 ms` in `Manual`) are recomputed only when `sample_rate` changes (`> 1e-3`), `release_mode != cached_release_mode`, or `!auto_coeffs_initialized`
- [X] T006 [US1] Implement dual-stage auto-release envelope tracking in `StereoPunchDsp::process_sample` in block 1 of `src/lib.rs`: update `auto_slow_gr_db` every sample from `target_gr_db` (`auto_slow_charge_coeff` when `target_gr_db > auto_slow_gr_db`, else `auto_slow_release_coeff`), sanitize `auto_slow_gr_db` (`!is_finite() || < 1.0e-20 => 0.0`), keep `Manual` release calculation bit-exact to existing formula (`release_coeff`), use `fast_next.max(self.auto_slow_gr_db)` on release for `AutoTight`/`AutoPunch`/`AutoGlue` (independent of `cfg.release_ms`), and clamp `self.auto_slow_gr_db = self.auto_slow_gr_db.min(self.env_gr_db)`
- [X] T007 [US1] Add unit test `test_auto_release_transient_vs_sustained_recovery` in `mod tests` of `src/lib.rs` verifying across `44100.0`, `48000.0`, and `96000.0` Hz and all three modes (`AutoTight`, `AutoPunch`, `AutoGlue`) that 90% GR recovery after a `15 ms` transient burst is at least `2.0x` faster than after a `400 ms` sustained signal at identical threshold/ratio
- [X] T008 [US1] Add unit test `test_auto_release_profile_ordering` in `mod tests` of `src/lib.rs` verifying strict recovery speed ordering `AutoTight > AutoPunch > AutoGlue` on both short transient bursts and sustained passages
- [X] T009 [US1] Add unit test `test_auto_release_manual_bit_exact_compatibility` in `mod tests` of `src/lib.rs` verifying that `ReleaseMode::Manual` matches manual `release_ms` envelope behavior and that changing `release_ms` (`10.0` vs `500.0 ms`) has zero effect (`0.0` bit difference) while any Auto-Release mode is active

**Checkpoint**: User Story 1 (MVP) est complète — l'Auto-Release à 3 profils fonctionne en temps réel dans `StereoPunchDsp` et est validé numériquement.

---

## Phase 4: User Story 2 - Clear Control Integration & Interaction with Manual Release (Priority: P2)

**Goal**: Intégrer le sélecteur ASCII `Release Mode: [Manual] [Auto Tight] [Auto Punch] [Auto Glue]` dans `1. COMPRESSOR & ENVELOPE BALLISTICS`, griser/annoter `AUTO (...)` le potentiomètre `Release` tout en le laissant éditable, faire repasser `[Punch 3ms / 80ms]` en `Manual`, ajuster la fenêtre à `820x650` px et mettre à jour la documentation.

**Independent Test**: Exécuter `cargo test --release` (vérifiant `test_ui_layout_column_widths_fit_window` dans `820x650` px), ouvrir l'éditeur dans Reaper et vérifier le sélecteur 4 boutons, l'indication `AUTO (...)` sur `Release` tout en restant manipulable, et le retour en `Manual` via `[Punch 3ms / 80ms]`.

### UI Implementation, Geometry Test & Documentation for User Story 2

- [X] T010 [US2] Update UI window height and meter height constants (`UI_WINDOW_HEIGHT: u32 = 650`, `UI_METER_HEIGHT: f32 = 416.0`) in block 4 of `src/lib.rs`
- [X] T011 [US2] Extend `draw_ssl_knob_cell` (or extract shared helper preserving all existing call sites) in block 4 of `src/lib.rs` with `auto_override: bool` so that when `auto_override` is `true` on the `Release` knob, its SSL cap, radial ticks, and numeric label render visually dimmed with ASCII readout `format!("AUTO ({})", formatted_val)` while keeping vertical drag, `Shift` fine-drag, double-click reset, and `-`/`+`/`Reset` buttons interactive
- [X] T012 [US2] Add the ASCII horizontal selector `Release Mode: [Manual] [Auto Tight] [Auto Punch] [Auto Glue]` (with active amber highlight `rgb(195, 125, 20)` and `ParamSetter` `begin`/`set`/`end`) inside section `1. COMPRESSOR & ENVELOPE BALLISTICS`, pass `params.release_mode.value() != ReleaseMode::Manual` to the `Release` knob cell, and update `[Punch 3ms / 80ms]` click handler to also set `&params.release_mode` to `ReleaseMode::Manual` in `draw_omore_punch_ui` in block 4 of `src/lib.rs`
- [X] T013 [US2] Update `test_ui_layout_column_widths_fit_window` in `mod tests` of `src/lib.rs` to account for the `Release Mode:` selector row (`+26.0` px in `section1_height`, `total_left_col_height = 578.0 <= 602.0` px) and verify empirical `egui` rendering bounds inside `820x650` px in both default `Manual` mode and worst-case `AutoTight` with `release_ms = 500.0` (`"AUTO (500.0 ms)"` fitting within `UI_KNOB_CELL_WIDTH = 110.0` without wrapping or overflowing)
- [X] T014 [P] [US2] Document the 4-state `Release Mode` (`Manual`, `Auto Tight 50/600 ms`, `Auto Punch 80/800 ms`, `Auto Glue 100/1200 ms`) in `README.md`
- [X] T015 [P] [US2] Add compressor Auto-Release usage instructions and `[Punch 3ms / 80ms]` interaction details in `USER_GUIDE.md`
- [X] T016 [P] [US2] Add an entry under `## [Unreleased]` (`### Ajouté`) describing the Compressor Auto-Release (`004-compressor-auto-release`) in `CHANGELOG.md`

**Checkpoint**: User Stories 1 et 2 sont complètes — l'interface `820x650` px, l'état `AUTO` éditable du potentiomètre `Release` et la documentation sont alignés.

---

## Phase 5: User Story 3 - Session Recall, Automation & Real-Time Stability (Priority: P3)

**Goal**: Garantir la continuité sans clic lors des bascules de mode en pleine compression, la parité stéréo L/R, la protection `NaN`/`Inf`/dénormaux en mode Auto-Release et le rappel par défaut sur `Manual`.

**Independent Test**: Exécuter `cargo test --release` et vérifier que `test_auto_release_mode_switch_continuity_and_stereo_link`, `test_nan_inf_and_safety_clamp` (avec modes Auto-Release) et le test de valeur par défaut de `OmorePunchParams::default().release_mode` passent sans erreur.

### Stability, Stereo-Link & Recall Tests for User Story 3

- [X] T017 [US3] Add unit test `test_auto_release_mode_switch_continuity_and_stereo_link` in `mod tests` of `src/lib.rs` switching across `Manual -> AutoTight -> AutoPunch -> AutoGlue -> Manual` mid-compression with active GR, asserting zero `NaN`/`Inf`, no instantaneous envelope jump (`<= 1.0 dB` per sample), and stereo L/R output parity (`< 1.0e-5` / `< 0.001 dB`)
- [X] T018 [US3] Extend `test_nan_inf_and_safety_clamp` (and verify `OmorePunchParams::default().release_mode.value() == ReleaseMode::Manual` plus `DspConfig::default().release_mode == ReleaseMode::Manual`) in `mod tests` of `src/lib.rs` to cover `AutoTight`, `AutoPunch`, and `AutoGlue` under `NaN`, `+Inf`, `-Inf`, silence (zero denormals), and extreme amplitudes

**Checkpoint**: Toutes les User Stories (P1, P2, P3) sont implémentées et testées.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Exécuter l'audit temps réel et tous les contrôles qualité obligatoires (ASCII, tests Release, bundles VST3/CLAP).

- [X] T019 Audit `StereoPunchDsp`, `ReleaseMode`, and `OmorePunch::process()` in `src/lib.rs` against Constitution Principles III & IV (zero heap allocation, zero locks, cached `.exp()` coefficients via `cached_release_mode`, `NaN`/`Inf`/denormal protection on `auto_slow_gr_db` and `env_gr_db`, `sanitize_sample()` final clamp)
- [X] T020 Run strict ASCII check `LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs` on `src/lib.rs` (must return no output)
- [X] T021 Run full release unit test suite `cargo test --release` on `src/lib.rs` (must pass with zero compiler warnings)
- [X] T022 Build VST3 and CLAP bundles with `cargo run --package xtask --release -- bundle omore-punch --release` and confirm `target/bundled/omore-punch.vst3` and `target/bundled/omore-punch.clap`
- [X] T023 Verify automated checklist items 1-3 in `specs/004-compressor-auto-release/quickstart.md` and note manual DAW verification (step 4 in Reaper)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: Aucune dépendance — démarre immédiatement (`T001`).
- **Foundational (Phase 2)**: Dépend de `T001` — crée `ReleaseMode`, étend `DspConfig`, `StereoPunchDsp` et `OmorePunchParams` (`T002` -> `T003` -> `T004`).
- **User Story 1 (Phase 3 - P1 🎯 MVP)**: Dépend de `T003` — implémente le calcul des coefficients auto (`T005`), la double enveloppe (`T006`) et les tests unitaires DSP (`T007`, `T008`, `T009`).
- **User Story 2 (Phase 4 - P2)**:
  - `T010`, `T011`, `T012`, `T013` modifient `src/lib.rs` et s'exécutent séquentiellement après `T004` (`T010` -> `T011` -> `T012` -> `T013`).
  - `T014` (`README.md`), `T015` (`USER_GUIDE.md`) et `T016` (`CHANGELOG.md`) touchent des fichiers distincts et peuvent s'exécuter en parallèle (`[P]`) une fois `T012` stabilisé.
- **User Story 3 (Phase 5 - P3)**: Dépend de `T004` et `T006` — ajoute les tests de continuité en bascule, parité stéréo, `NaN`/`Inf` et rappel par défaut (`T017` -> `T018`).
- **Polish (Phase 6)**: Dépend de la complétion des Phases 3, 4 et 5 (`T019` -> `T020` -> `T021` -> `T022` -> `T023`).

### User Story Dependencies

- **User Story 1 (P1)**: Indépendante après Phase 2 — livre le moteur DSP Auto-Release 3 profils complet et vérifié numériquement (MVP).
- **User Story 2 (P2)**: Indépendante de US3 après Phase 2 — s'appuie sur `OmorePunchParams::release_mode` (`T004`) pour afficher le sélecteur UI et l'état `AUTO` du potard `Release`.
- **User Story 3 (P3)**: S'appuie sur `StereoPunchDsp` (`T006`) et `OmorePunchParams` (`T004`) pour valider la stabilité temps réel, la parité stéréo et la rétrocompatibilité.

### Parallel Opportunities

- Toutes les tâches modifiant `src/lib.rs` (`T002` à `T013`, `T017`, `T018`) doivent être exécutées séquentiellement en raison de l'architecture monolithique imposée par le Principe I de la Constitution.
- Dans la **Phase 4 (US2)**, les tâches documentaires `T014` (`README.md`), `T015` (`USER_GUIDE.md`) et `T016` (`CHANGELOG.md`) concernent trois fichiers distincts et sont parallélisables (`[P]`).

---

## Parallel Example: User Story 2

```bash
# Les mises à jour documentaires de US2 peuvent être lancées en parallèle :
Task T014: "Document the 4-state Release Mode in README.md"
Task T015: "Add compressor Auto-Release usage instructions in USER_GUIDE.md"
Task T016: "Add an entry under ## [Unreleased] (### Ajouté) in CHANGELOG.md"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Compléter Phase 1 (`T001`) et Phase 2 (`T002`–`T004`).
2. Compléter Phase 3 (`T005`–`T009`) : moteur DSP Auto-Release à double étage (`AutoTight`, `AutoPunch`, `AutoGlue`) et tests unitaires numériques.
3. **STOP & VALIDATE** : Lancer `cargo test --release` et vérifier que les 3 profils Auto-Release et le mode `Manual` répondent conformément au contrat DSP.

### Incremental Delivery

1. Ajouter Phase 4 (`T010`–`T016`) : fenêtre `820x650` px, sélecteur `Release Mode:` dans `1. COMPRESSOR & ENVELOPE BALLISTICS`, potard `Release` atténué avec `AUTO (...)` mais éditable, reset sur `[Punch 3ms / 80ms]`, test géométrique et documentation.
2. Ajouter Phase 5 (`T017`–`T018`) : tests de continuité lors des bascules de mode, parité stéréo L/R, sécurité `NaN`/`Inf`/dénormaux et défaut `Manual`.
3. Exécuter Phase 6 (`T019`–`T023`) : audit temps réel, contrôle ASCII strict, `cargo test --release`, bundling VST3/CLAP via `xtask` et validation finale dans Reaper.

---

## Notes

- Les tâches marquées `[P]` portent sur des fichiers différents et n'ont pas de dépendance mutuelle.
- Toute chaîne ajoutée dans `src/lib.rs` (commentaires, étiquettes UI, noms de paramètres, assertions de tests) DOIT rester 100 % ASCII (Principe II).
- Aucune allocation ni verrou sur le thread audio (`StereoPunchDsp` / `process()`, Principes III et IV).
