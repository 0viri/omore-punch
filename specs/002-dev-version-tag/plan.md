# Implementation Plan: Tag de build Git dans le badge de version de développement (DEV)

**Branch**: `feat/002-dev-version-tag` | **Date**: 2026-11-14 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/002-dev-version-tag/spec.md`

## Summary

Capturer à la compilation via un script Cargo `build.rs` le hash court (7 caractères) du commit Git courant et l'état `-dirty` éventuel de l'arbre de travail (`OMORE_GIT_TAG`, ex. `1d2c5b9` ou `1d2c5b9-dirty`, avec repli vide hors Git). Sur les builds de développement (`-dev` dans `Cargo.toml`), afficher dans l'en-tête `egui` d'Omore Punch (`src/lib.rs`) le badge bleu-acier contrasté `[DEV 1d2c5b9]` ou `[DEV 1d2c5b9-dirty]` (repli `[DEV]`) à côté de `OMORE PUNCH v0.2.1`, en ajustant les tailles typographiques d'en-tête (`13.5` / `11.5` / `12.5`) pour tenir sur une ligne dans `820 px` même avec `[CLIP: ACTIVE - CLICK TO RESET]`. Sur les releases (sans `-dev`), aucun badge ni hash n'est affiché, et `OmorePunch::VERSION` reste strictement `env!("CARGO_PKG_VERSION")`.

## Technical Context

**Language/Version**: Rust (Edition 2021)

**Primary Dependencies**: `nih_plug`, `nih_plug_egui` (`egui` 0.31.1), CLI `git` optionnel au build (`build.rs`)

**Storage**: N/A (variables de compilation `CARGO_PKG_VERSION` et `OMORE_GIT_TAG`)

**Testing**: `cargo test --release` (module `tests` intégré dans `src/lib.rs`)

**Target Platform**: Linux (Debian 13 x86_64) et Windows 11 (x86_64) dans Reaper (formats VST3 et CLAP)

**Project Type**: Audio Plugin (VST3 / CLAP) à architecture monolithique (`src/lib.rs`) + build tooling Cargo (`build.rs`)

**Performance Goals**: Thread audio 100 % temps réel (non touché) et rendu UI fluide à 60 FPS

**Constraints**: Code du plugin uniquement dans `src/lib.rs`, 100 % ASCII pur dans `src/lib.rs` et `build.rs`, source unique de versionning (`Cargo.toml`), largeur d'en-tête compatible `UI_WINDOW_WIDTH = 820`

**Scale/Scope**: Ajout de `build.rs` à la racine, mise à jour du bloc 4 (`parse_display_version`, `format_dev_badge_label`, header dans `draw_omore_punch_ui`) et du module `tests` dans `src/lib.rs`, plus mise à jour de `CHANGELOG.md`

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. Monolithic Single-File Architecture (`src/lib.rs`)** : **PASS** — Tout le code du plugin (parsing, formatage UI, rendu `egui`, tests) reste dans `src/lib.rs`. `build.rs` est du build tooling Cargo au même titre que `xtask/` et `build.sh` (explicitement hors règle par le Principe I).
- **II. Pure ASCII Source** : **PASS** — `src/lib.rs` (et `build.rs`) restent 100 % ASCII (`LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs` vide).
- **III. Real-Time Audio Thread Safety** : **PASS** — Aucune modification de `process()`, `StereoPunchDsp` ni `MeterTelemetry`. `parse_display_version` et `format_dev_badge_label` ne s'exécutent que côté UI.
- **IV. Signal Sanitization & Safety Clamp** : **PASS** — Aucun impact sur la chaîne DSP ni sur `sanitize_sample()`.
- **V. Mandatory DSP Unit Tests** : **PASS** — Tous les tests DSP existants restent inchangés et passent ; des tests unitaires vérifient `parse_display_version`, `format_dev_badge_label`, `OmorePunch::VERSION` et l'encombrement `egui` dans `820 px` (`cargo test --release`).
- **VI. Single-Source Versioning** : **PASS** — `Cargo.toml` reste l'unique source du numéro de version (`0.2.1-dev`), `OmorePunch::VERSION` reste `env!("CARGO_PKG_VERSION")`, et `OMORE_GIT_TAG` fournit uniquement un tag de build complémentaire dans l'UI pour les versions `-dev`.

*Post-Design Re-evaluation*: **PASS** — Aucune déviation des principes I à VI.

## Project Structure

### Documentation (this feature)

```text
specs/002-dev-version-tag/
├── plan.md                                  # Plan d'implémentation (/speckit-plan)
├── research.md                              # Décisions techniques Phase 0 (/speckit-plan)
├── data-model.md                            # Modèle de données dérivées Phase 1 (/speckit-plan)
├── quickstart.md                            # Guide de validation Phase 1 (/speckit-plan)
├── contracts/
│   └── ui-version-tag-contract.md           # Contrat build.rs, UI & métadonnées hôte Phase 1
└── tasks.md                                 # Liste des tâches Phase 2
```

### Source Code (repository root)

```text
build.rs                 # Script de build Cargo capturant OMORE_GIT_TAG (<short7>[-dirty])
src/
└── lib.rs               # Bloc 4 (parse_display_version, format_dev_badge_label, header) & mod tests
CHANGELOG.md             # Entrée mise à jour sous ## [Unreleased]
```

**Structure Decision**: Placement du script d'extraction Git dans `build.rs` (tooling de build Cargo standard) et de toute la logique d'affichage et de tests dans `src/lib.rs` conformément au Principe I.

## Complexity Tracking

> Aucune violation de la constitution détectée.
