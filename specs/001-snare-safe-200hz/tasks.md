---

description: "Task list for feature 001-snare-safe-200hz (Remplacement du bouton rapide Kick Safe 90Hz par Snare Safe 200Hz)"
---

# Tasks: Remplacement du bouton rapide Kick Safe 90Hz par Snare Safe 200Hz

**Input**: Design documents from `/specs/001-snare-safe-200hz/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/ui-quick-action-contract.md, quickstart.md

**Tests**: Inclus conformément au Principe V de la Constitution (`Mandatory DSP Unit Tests`) et au critère `SC-003` de `spec.md`.

**Organization**: Les tâches sont groupées par User Story (`US1`, `US2`) pour permettre une implémentation et une validation incrémentales.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Exécutable en parallèle (fichiers distincts, aucune dépendance sur une tâche incomplète)
- **[Story]**: User Story associée (`[US1]`, `[US2]`)
- Chaque tâche indique le chemin exact du fichier concerné.

## Path Conventions

- **Architecture monolithique du plugin** : `src/lib.rs` (blocs DSP, paramètres, télémétrie, UI `egui`, tests unitaires)
- **Documentation & directives du projet** : `README.md`, `USER_GUIDE.md`, `CHANGELOG.md`, `GEMINI.md` à la racine du dépôt

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Vérification de l'environnement de compilation et de la suite de tests existante avant modification.

- [X] T001 Verify baseline compilation and unit tests with `cargo test --release` on `src/lib.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Vérification des invariants de rétrocompatibilité du paramètre hôte et du moteur DSP définis dans `data-model.md`.

**⚠️ CRITICAL**: Ces invariants ne doivent pas être altérés lors de la modification de l'UI.

- [X] T002 Verify parameter and DSP invariants on `OmorePunchParams::sc_hpf_hz` (`Identifiant stable (#[id]): "sc_hpf_hz"`, `Nom affiché: "SC HPF"`, `Plage: 20.0 Hz à 250.0 Hz`, `Valeur par défaut (Reset): 20.0 Hz`) and `DspConfig::sc_hpf_hz` (`Borné dans [20.0, 250.0] par StereoPunchDsp::update_sidechain_hpf ; bypass complet si <= 20.5 Hz, filtre RC 1er ordre précalculé (sc_hpf_coeff) sinon.`) in `src/lib.rs`

**Checkpoint**: Invariants vérifiés — l'implémentation des User Stories peut commencer.

---

## Phase 3: User Story 1 - Activation rapide du filtre de détection à 200 Hz (Priority: P1) 🎯 MVP

**Goal**: Remplacer le bouton `[Kick Safe 90Hz]` par `[Snare Safe 200Hz]` dans `draw_omore_punch_ui` et régler `sc_hpf_hz` sur `200.0 Hz` en un clic avec notification complète d'automation hôte.

**Independent Test**: Ouvrir l'interface du plugin (ou inspecter `draw_omore_punch_ui` dans `src/lib.rs`), vérifier que le bouton affiche `[Snare Safe 200Hz]` entre `[Punch 3ms / 80ms]` et `[Auto Makeup: ON/OFF]`, et qu'un clic exécute la séquence `begin_set_parameter` -> `set_parameter(&params.sc_hpf_hz, 200.0)` -> `end_set_parameter`.

### Implementation for User Story 1

- [X] T003 [US1] Replace quick-action button label `"[Kick Safe 90Hz]"` with `egui::RichText::new("[Snare Safe 200Hz]").monospace().strong()` (`Contrainte d'encodage: 100 % ASCII strict (0x20..=0x7E)`) and update parameter target to `200.0` (`Valeur cible [Snare Safe 200Hz]: 200.0 Hz` via `setter.begin_set_parameter(&params.sc_hpf_hz)`, `setter.set_parameter(&params.sc_hpf_hz, 200.0)`, `setter.end_set_parameter(&params.sc_hpf_hz)`) in `draw_omore_punch_ui` in `src/lib.rs`

**Checkpoint**: User Story 1 (MVP) est fonctionnelle — un clic sur `[Snare Safe 200Hz]` règle `SC HPF` à `200.0 Hz`.

---

## Phase 4: User Story 2 - Compréhension immédiate du rôle du bouton via l'infobulle et la documentation (Priority: P2)

**Goal**: Mettre à jour l'infobulle en anglais (100 % ASCII), couvrir la coupure à `200.0 Hz` dans les tests unitaires DSP et synchroniser la documentation du projet (`README.md`, `USER_GUIDE.md`, `CHANGELOG.md`, `GEMINI.md`).

**Independent Test**: Survoler `[Snare Safe 200Hz]` pour vérifier l'infobulle, exécuter `cargo test --release` pour valider l'atténuation de réduction de gain à `200.0 Hz`, et vérifier qu'aucune référence obsolète à `[Kick Safe 90Hz]` ne subsiste dans `README.md`, `USER_GUIDE.md` et `GEMINI.md`.

### Tests & Implementation for User Story 2

- [X] T004 [US2] Update `.on_hover_text(...)` on the `[Snare Safe 200Hz]` button to `"Set Sidechain HPF to 200.0 Hz so Kick and Snare body (up to 200 Hz) do not pump the Drum Bus"` (`Contrainte d'encodage: 100 % ASCII strict (0x20..=0x7E)`) in `draw_omore_punch_ui` in `src/lib.rs`
- [X] T005 [US2] Update DSP unit test coverage in the `tests` module of `src/lib.rs` to verify sidechain HPF detector immunity and low-frequency preservation at `sc_hpf_hz: 200.0` Hz (and update `90.0` Hz preset test references where appropriate) in `src/lib.rs`
- [X] T006 [P] [US2] Update sidechain HPF feature description from `[Kick Safe 90Hz]` to `[Snare Safe 200Hz]` in `README.md`
- [X] T007 [P] [US2] Update step 3 of the Metal/Rock Drum Bus quick setup guide from `[Kick Safe 90Hz]` to `[Snare Safe 200Hz]` (`200 Hz`, technique Cristian Kohle protégeant le grave de la grosse caisse et le corps de la caisse claire) in `USER_GUIDE.md`
- [X] T008 [P] [US2] Add an entry under `## [Unreleased]` (`### Modifié`) documenting the replacement of `[Kick Safe 90Hz]` with `[Snare Safe 200Hz]` (`200.0 Hz`) in `CHANGELOG.md`
- [X] T009 [P] [US2] Update bracketed button typography example from `[Kick Safe 90Hz]` to `[Snare Safe 200Hz]` in section 4 of `GEMINI.md`

**Checkpoint**: User Stories 1 et 2 sont complètes, testées et documentées.

---

## Phase 5: Polish & Cross-Cutting Concerns

**Purpose**: Exécution des contrôles qualité obligatoires de la Constitution (section 6 de `GEMINI.md` et `quickstart.md`).

- [X] T010 Run strict ASCII verification `LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs` on `src/lib.rs`
- [X] T011 Run release unit test suite `cargo test --release` on `src/lib.rs`
- [X] T012 Build VST3 and CLAP plugin bundles with `cargo run --package xtask --release -- bundle omore-punch --release` to verify `target/bundled/omore-punch.vst3` and `target/bundled/omore-punch.clap`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)** : Aucune dépendance — démarre immédiatement (`T001`).
- **Foundational (Phase 2)** : Dépend de `T001` — vérifie les invariants dans `src/lib.rs` (`T002`).
- **User Story 1 (Phase 3 - P1 MVP)** : Dépend de `T002` — modifie le bouton rapide et sa valeur cible dans `src/lib.rs` (`T003`).
- **User Story 2 (Phase 4 - P2)** :
  - `T004` et `T005` touchent `src/lib.rs` et s'exécutent séquentiellement après `T003`.
  - `T006` (`README.md`), `T007` (`USER_GUIDE.md`), `T008` (`CHANGELOG.md`) et `T009` (`GEMINI.md`) portent sur des fichiers distincts et peuvent être exécutées en parallèle (`[P]`).
- **Polish (Phase 5)** : Dépend de la complétion des Phases 3 et 4 (`T010` -> `T011` -> `T012`).

### User Story Dependencies

- **User Story 1 (P1)** : Indépendante — constitue le MVP.
- **User Story 2 (P2)** : Complète l'infobulle de `US1` dans `src/lib.rs`, enrichit les tests unitaires DSP pour `200.0 Hz` et met à jour la documentation du projet.

### Parallel Opportunities

- Dans la **Phase 4 (US2)**, les tâches documentaires `T006` (`README.md`), `T007` (`USER_GUIDE.md`), `T008` (`CHANGELOG.md`) et `T009` (`GEMINI.md`) touchent quatre fichiers différents et sont marquées `[P]`.

---

## Parallel Example: User Story 2

```bash
# Les mises à jour documentaires de US2 peuvent être réalisées en parallèle :
Task T006: "Update sidechain HPF feature description from [Kick Safe 90Hz] to [Snare Safe 200Hz] in README.md"
Task T007: "Update step 3 of the Metal/Rock Drum Bus quick setup guide from [Kick Safe 90Hz] to [Snare Safe 200Hz] in USER_GUIDE.md"
Task T008: "Add an entry under ## [Unreleased] (### Modifié) in CHANGELOG.md"
Task T009: "Update bracketed button typography example from [Kick Safe 90Hz] to [Snare Safe 200Hz] in section 4 of GEMINI.md"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Compléter Phase 1 (`T001`) et Phase 2 (`T002`).
2. Compléter Phase 3 (`T003` — remplacement du bouton et de la cible `200.0 Hz` dans `src/lib.rs`).
3. **STOP & VALIDATE** : Vérifier que le bouton `[Snare Safe 200Hz]` compile et applique `200.0 Hz`.

### Incremental Delivery

1. Ajouter Phase 4 (`T004` à `T009` — infobulle ASCII, test unitaire DSP à `200.0 Hz`, documentation `README.md`, `USER_GUIDE.md`, `CHANGELOG.md`, `GEMINI.md`).
2. Exécuter Phase 5 (`T010` à `T012` — contrôle ASCII, `cargo test --release`, bundling VST3/CLAP via `xtask`).
