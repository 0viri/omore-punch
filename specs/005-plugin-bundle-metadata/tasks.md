---

description: "Task list for feature implementation: Métadonnées et Nommage du Bundle Plugin"
---

# Tasks: Métadonnées et Nommage du Bundle Plugin

**Input**: Design documents from `/specs/005-plugin-bundle-metadata/`
**Prerequisites**: `plan.md` (required), `spec.md` (required), `research.md`, `data-model.md`, `contracts/bundle-metadata-contract.md`, `quickstart.md`

**Tests**: Des tests unitaires ciblés sont inclus conformément à la Constitution (Principe V) et au plan pour vérifier l'identité du plugin, l'invariance des identifiants DAW (`VST3_CLASS_ID`, `CLAP_ID`) et la validité du `moduleinfo.json` généré.

**Organization**: Les tâches sont regroupées par User Story afin de permettre une implémentation et une validation indépendantes (US1 = MVP métadonnées statiques ; US2 = renommage des bundles `Omore Punch.vst3` / `Omore Punch.clap` et nettoyage d'installation).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Peut s'exécuter en parallèle (fichiers distincts, aucune dépendance sur une tâche incomplète)
- **[Story]**: User Story associée (`[US1]` ou `[US2]`)
- Chaque description inclut le chemin exact du ou des fichiers concernés

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Préparer les manifestes Cargo et dépendances de build/bundling nécessaires aux deux User Stories

- [X] T001 Mettre à jour `authors = ["Omore"]` et ajouter `[target.'cfg(windows)'.build-dependencies]` avec `winres = "0.1"` dans `Cargo.toml`
- [X] T002 [P] Ajouter les dépendances de lecture TOML et sérialisation JSON (`serde`, `serde_json`, `toml`) dans `xtask/Cargo.toml` pour générer `moduleinfo.json` et lire le nom de bundle depuis `bundler.toml`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Mettre à jour l'identité fabricant dans le plugin et verrouiller les invariants de compatibilité DAW avant toute génération d'artefact

**⚠️ CRITICAL**: Doit être complété avant de valider US1 et US2 afin que `Omore` et les invariants `VST3_CLASS_ID`/`CLAP_ID` soient garantis par les tests

- [X] T003 Mettre à jour `OmorePunch::VENDOR` de `"Omore Audio"` vers `"Omore"` dans `src/lib.rs` (bloc 3) tout en conservant strictement inchangés `OmorePunch::NAME = "Omore Punch"`, `OmorePunch::VERSION = env!("CARGO_PKG_VERSION")`, `OmorePunch::VST3_CLASS_ID = *b"OmorePunchBusV01"`, `OmorePunch::CLAP_ID = "local.omore-audio.omore-punch"`, et tous les `#[id = "..."]` de `OmorePunchParams` (100 % ASCII)
- [X] T004 Ajouter un test unitaire dans le module `tests` de `src/lib.rs` vérifiant `OmorePunch::NAME == "Omore Punch"`, `OmorePunch::VENDOR == "Omore"`, `OmorePunch::VST3_CLASS_ID == *b"OmorePunchBusV01"`, et `OmorePunch::CLAP_ID == "local.omore-audio.omore-punch"` (100 % ASCII)

**Checkpoint**: `cargo test --release` passe et l'identité fabricant + invariants DAW sont verrouillés.

---

## Phase 3: User Story 1 - Affichage complet des métadonnées dans les gestionnaires de plugins (Priority: P1) 🎯 MVP

**Goal**: Exposer `Version` (issue de `Cargo.toml`), `Manufacturer = Omore` et `Category` (`["Fx", "Dynamics", "Distortion", "Stereo"]`) dans les métadonnées statiques du bundle VST3 (`Contents/Resources/moduleinfo.json`) et des binaires PE Windows (`VS_VERSION_INFO`).

**Independent Test**: Exécuter `cargo test --release`, `cargo test -p xtask --release` et `cargo run --package xtask --release -- bundle omore-punch --release`, puis vérifier que `<bundle>.vst3/Contents/Resources/moduleinfo.json` contient un JSON valide conforme au contrat (`Name = "Omore Punch"`, `Vendor = "Omore"`, `Version = <Cargo.toml>`, `CID = "4F6D6F726550756E6368427573563031"`, `Sub Categories = ["Fx", "Dynamics", "Distortion", "Stereo"]`). Sur Windows, vérifier dans Plugoff que Version, Manufacturer (`Omore`) et Category (`Dynamics`) ne sont plus vides / `Not Supplied`.

### Implementation for User Story 1

- [X] T005 [P] [US1] Créer `build.rs` à la racine du dépôt (actif uniquement lorsque `CARGO_CFG_TARGET_OS == "windows"`) pour embarquer la ressource PE `VS_VERSION_INFO` via `winres` avec les 4 composantes numériques `(CARGO_PKG_VERSION_MAJOR, CARGO_PKG_VERSION_MINOR, CARGO_PKG_VERSION_PATCH, 0)`, les chaînes `FileVersion` et `ProductVersion` égales à `CARGO_PKG_VERSION`, `CompanyName = "Omore"`, `ProductName = "Omore Punch"`, et `FileDescription = "Omore Punch - Stereo Drum Bus Compressor & Clipper"`
- [X] T006 [US1] Étendre `xtask/src/main.rs` pour lire dynamiquement `[package].version` dans `Cargo.toml` et le nom de bundle éventuel dans `bundler.toml` (repli sur le nom du package Cargo `omore-punch`), puis écrire `<bundle>.vst3/Contents/Resources/moduleinfo.json` après `nih_plug_xtask::main()` avec `Name = "Omore Punch"`, `Version`, `Factory Info` (`Vendor = "Omore"`, `URL = "https://omore-audio.local"`, `E-Mail = "contact@omore-audio.local"`, `Flags: { Unicode: true, Classes Discardable: false, Component Non Discardable: false }`) et `Classes[0]` (`CID = "4F6D6F726550756E6368427573563031"`, `Category = "Audio Module Class"`, `Name = "Omore Punch"`, `Vendor = "Omore"`, `Version`, `SDKVersion = "VST 3.6.14"`, `Sub Categories = ["Fx", "Dynamics", "Distortion", "Stereo"]`, `Class Flags = 2`, `Cardinality = 2147483647`, `Snapshots = []`)
- [X] T007 [US1] Ajouter des tests unitaires dans `xtask/src/main.rs` vérifiant la construction du `moduleinfo.json` conforme au contrat (champs obligatoires, `CID`, `Vendor = "Omore"`, `Sub Categories`, et prise en charge d'une version avec suffixe `-dev` comme `"0.3.1-dev"`)

**Checkpoint**: User Story 1 est fonctionnelle et testable indépendamment (MVP prêt : toutes les métadonnées statiques sont exposées dans le bundle VST3 et dans les ressources Windows).

---

## Phase 4: User Story 2 - Renommage affiché vers « Omore Punch » sans rupture de compatibilité (Priority: P2)

**Goal**: Générer les bundles distribués sous les noms `Omore Punch.vst3` et `Omore Punch.clap` sur Linux et Windows, mettre à jour les scripts d'installation locale et la CI en retirant proprement les anciens bundles `omore-punch.*`, tout en préservant à 100 % l'ouverture des projets DAW existants.

**Independent Test**: Exécuter `cargo run --package xtask --release -- bundle omore-punch --release` et vérifier que `target/bundled/Omore Punch.vst3` (avec `Contents/Resources/moduleinfo.json` et le binaire interne `Omore Punch.so` / `Omore Punch.vst3`) et `target/bundled/Omore Punch.clap` sont créés ; vérifier que `build.sh` et `build.ps1` suppriment d'éventuels anciens `omore-punch.vst3` / `omore-punch.clap` ; vérifier sous Reaper qu'un projet sauvegardé avec `omore-punch` s'ouvre sans erreur avec `Omore Punch`.

### Implementation for User Story 2

- [X] T008 [P] [US2] Créer `bundler.toml` à la racine du dépôt avec la section `[omore-punch]` et `name = "Omore Punch"` pour que `nih_plug_xtask` produise `Omore Punch.vst3` et `Omore Punch.clap` dans `target/bundled/` sur Linux et Windows
- [X] T009 [P] [US2] Mettre à jour `build.sh` pour supprimer les anciens chemins (`"$HOME/.clap/omore-punch.clap"`, `"$HOME/.vst3/omore-punch.vst3"`) ainsi que les éventuels précédents (`"$HOME/.clap/Omore Punch.clap"`, `"$HOME/.vst3/Omore Punch.vst3"`) avant de copier `"target/bundled/Omore Punch.clap"` et `"target/bundled/Omore Punch.vst3"`
- [X] T010 [P] [US2] Mettre à jour `build.ps1` pour supprimer les anciens chemins (`omore-punch.vst3`, `omore-punch.clap`) ainsi que les éventuels précédents (`Omore Punch.vst3`, `Omore Punch.clap`) dans les répertoires cibles avant de copier `"target\bundled\Omore Punch.vst3"` et `"target\bundled\Omore Punch.clap"` (avec guillemets gérant l'espace)
- [X] T011 [P] [US2] Mettre à jour `.github/workflows/build.yml` dans l'étape `Upload Bundled Plugins (VST3 + CLAP)` pour cibler `target/bundled/Omore Punch.vst3` et `target/bundled/Omore Punch.clap`

**Checkpoint**: User Story 1 et User Story 2 fonctionnent ensemble : les bundles `Omore Punch.vst3` et `Omore Punch.clap` sont produits, installés proprement sans doublons `omore-punch.*`, et exposent toutes leurs métadonnées.

---

## Phase 5: Polish & Cross-Cutting Concerns

**Purpose**: Journalisation des changements et vérification bout-en-bout sur les portes qualité du projet

- [X] T012 [P] Ajouter une entrée sous `## [Unreleased]` dans `CHANGELOG.md` décrivant l'exposition des métadonnées statiques (`Version`, `Manufacturer = Omore`, `Category`) et le renommage des bundles distribués en `Omore Punch.vst3` et `Omore Punch.clap` (sans modifier la version dans `Cargo.toml`)
- [X] T013 Exécuter les contrôles de `specs/005-plugin-bundle-metadata/quickstart.md` et des portes qualité CI : `LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs`, `cargo test --release`, `cargo test -p xtask --release`, `cargo run --package xtask --release -- bundle omore-punch --release`, et inspecter `target/bundled/Omore Punch.vst3/Contents/Resources/moduleinfo.json` (plus exécution locale `./build.sh` sur Linux si souhaité)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: Aucune dépendance — démarre immédiatement (`T001`, `T002` en parallèle).
- **Foundational (Phase 2)**: Dépend de `T001` pour la compilation du crate racine (`T003` puis `T004`). Bloque la validation complète de US1 et US2.
- **User Story 1 (Phase 3 - P1 🎯 MVP)**: Dépend de Phase 1 (`T001`, `T002`) et Phase 2 (`T003`, `T004`). `T005` (`build.rs`) peut tourner en parallèle de `T006` (`xtask/src/main.rs`). `T007` complète `xtask/src/main.rs`.
- **User Story 2 (Phase 4 - P2)**: Dépend de Phase 2 (`T003` pour l'invariance des IDs) et de `T006` pour que `xtask/src/main.rs` lise déjà `bundler.toml` lors de l'écriture de `moduleinfo.json`. Les tâches `T008`, `T009`, `T010` et `T011` touchent des fichiers distincts et peuvent toutes tourner en parallèle.
- **Polish (Phase 5)**: Dépend de la fin des Phases 3 et 4 (`T012` en parallèle avant `T013` qui valide l'ensemble).

### User Story Dependencies

- **User Story 1 (P1)**: Indépendante de US2 — fonctionne dès que `xtask/src/main.rs` et `build.rs` sont en place (cible `<bundle>.vst3` dynamiquement).
- **User Story 2 (P2)**: S'appuie sur `T006` (lecture de `bundler.toml` dans `xtask`) et reste indépendamment vérifiable sur le renommage de bundle, le nettoyage des anciens fichiers et la rétrocompatibilité DAW.

### Parallel Opportunities

- **Phase 1**: `T001` (`Cargo.toml`) || `T002` (`xtask/Cargo.toml`)
- **Phase 3 (US1)**: `T005` (`build.rs`) || `T006` (`xtask/src/main.rs`)
- **Phase 4 (US2)**: `T008` (`bundler.toml`) || `T009` (`build.sh`) || `T010` (`build.ps1`) || `T011` (`.github/workflows/build.yml`)
- **Phase 5**: `T012` (`CHANGELOG.md`) avant `T013` (validation globale)

---

## Parallel Example: User Story 2

```bash
# Lancer en parallèle les 4 fichiers distincts de User Story 2 :
Task: "T008 [P] [US2] Créer bundler.toml à la racine du dépôt"
Task: "T009 [P] [US2] Mettre à jour build.sh (nettoyage omore-punch.* + copie Omore Punch.*)"
Task: "T010 [P] [US2] Mettre à jour build.ps1 (nettoyage omore-punch.* + copie Omore Punch.*)"
Task: "T011 [P] [US2] Mettre à jour .github/workflows/build.yml (upload Omore Punch.vst3 & Omore Punch.clap)"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Compléter Phase 1 (`T001`, `T002`) et Phase 2 (`T003`, `T004`).
2. Compléter Phase 3 (`T005`, `T006`, `T007`).
3. **STOP and VALIDATE**: Tester `cargo test --release`, `cargo test -p xtask --release` et vérifier le `moduleinfo.json` généré. Toutes les métadonnées (`Version`, `Manufacturer = Omore`, `Category`) sont déjà fonctionnelles.

### Incremental Delivery

1. MVP (US1) validé → métadonnées complètes dans le bundle VST3 et les ressources Windows.
2. Ajouter Phase 4 (US2 : `T008`–`T011`) → bundles renommés en `Omore Punch.vst3` et `Omore Punch.clap`, scripts `build.sh`/`build.ps1` et CI alignés, anciens `omore-punch.*` nettoyés.
3. Finaliser Phase 5 (`T012`, `T013`) → `CHANGELOG.md` à jour et validation complète Linux (+ Windows via CI/poste Windows).
