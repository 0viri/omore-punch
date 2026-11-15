# Implementation Plan: Interface type SSL avec potentiomètres rotatifs en grille

**Branch**: `feat/002-ssl-rotary-knobs-ui` | **Date**: 2026-11-14 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/002-ssl-rotary-knobs-ui/spec.md`

## Summary

Remplacer les 9 curseurs horizontaux (`draw_param_row`) de la colonne gauche d'`Omore Punch` (`src/lib.rs`) par une grille compacte de potentiomètres rotatifs vectoriels (`egui::Painter`) au look type SSL (couronne de 11 graduations sur 270°, capuchons colorés par famille fonctionnelle sur le fond sombre `Omore`, repère angulaire, glisser vertical souris avec mode fin sous `Shift`, double-clic de remise à zéro, lecture numérique ASCII et petits boutons compacts `-`, `+`, `Reset`). Mettre à jour le test unitaire de géométrie UI pour valider l'absence de débordement horizontal et vertical dans `820x480` px et synchroniser la documentation utilisateur (`README.md`, `USER_GUIDE.md`, `CHANGELOG.md`).

## Technical Context

**Language/Version**: Rust (Edition 2021)

**Primary Dependencies**: `nih_plug`, `nih_plug_egui` (`egui 0.31.1`), `atomic_float`

**Storage**: N/A (état des paramètres et `EguiState` persistés par l'hôte DAW VST3/CLAP via `nih-plug`)

**Testing**: `cargo test --release` (module `tests` intégré dans `src/lib.rs`)

**Target Platform**: Linux (Debian 13 x86_64) et Windows 11 (x86_64) dans Reaper (formats VST3 et CLAP)

**Project Type**: Audio Plugin (VST3 / CLAP) à architecture monolithique (`src/lib.rs`)

**Performance Goals**: Thread audio 100 % temps réel (zéro allocation, zéro verrou, coefficients précalculés) et rafraîchissement UI fluide à 60 FPS

**Constraints**: Fichier unique `src/lib.rs`, 100 % ASCII pur dans `src/lib.rs`, fenêtre `820x480` px sans chevauchement de colonnes, compatibilité ascendante totale des sessions DAW (IDs et plages de paramètres inchangés)

**Scale/Scope**: Refonte du composant de contrôle de paramètre et de la disposition des sections `1.` et `2.` dans le bloc 4 (`draw_omore_punch_ui`) de `src/lib.rs`, mise à jour du test de géométrie dans `mod tests` et mise à jour de `README.md`, `USER_GUIDE.md` et `CHANGELOG.md`

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. Monolithic Single-File Architecture (`src/lib.rs`)** : **PASS** — Le type `SslKnobColor`, la fonction `draw_ssl_knob_cell`, la mise en page en grille dans `draw_omore_punch_ui` et les tests associés résident intégralement dans `src/lib.rs`. Aucun fichier source ou sous-module supplémentaire.
- **II. Pure ASCII Source** : **PASS** — Le tracé des graduations et du repère angulaire est purement géométrique (`Painter::line_segment`, `circle_filled`, `circle_stroke`) et tous les textes/infobulles restent 100 % ASCII (`LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs`).
- **III. Real-Time Audio Thread Safety** : **PASS** — Aucune modification dans `process()` ni dans `StereoPunchDsp`. Les interactions des potentiomètres s'exécutent sur le thread UI et communiquent avec le thread audio via `ParamSetter`.
- **IV. Signal Sanitization & Safety Clamp** : **PASS** — `sanitize_sample()` et la chaîne DSP sont inchangés.
- **V. Mandatory DSP Unit Tests** : **PASS** — Tous les tests DSP existants restent intacts et `test_ui_layout_column_widths_fit_window` est enrichi pour vérifier les dimensions horizontales et verticales de la grille de potentiomètres dans `820x480` px (`cargo test --release`).
- **VI. Single-Source Versioning** : **PASS** — `Cargo.toml` reste la source unique de version (`env!("CARGO_PKG_VERSION")`).
- **Platform, Format & GUI Constraints (Constitution v1.1.0)** : **PASS** — Conforme aux standards GUI v1.1.0 (thème sombre `Omore` + boutons rotatifs en grille style SSL à capuchons colorés, glisser vertical, `Shift` fin, double-clic reset et boutons compacts `-`, `+`, `Reset`).

*Post-Design Re-evaluation*: **PASS** — Aucun écart par rapport à la Constitution v1.1.0.

## Project Structure

### Documentation (this feature)

```text
specs/002-ssl-rotary-knobs-ui/
├── plan.md              # Plan d'implémentation (/speckit-plan)
├── research.md          # Décisions techniques Phase 0 (/speckit-plan)
├── data-model.md        # Modèle UI, couleurs SSL et géométrie Phase 1 (/speckit-plan)
├── quickstart.md        # Guide de validation Phase 1 (/speckit-plan)
├── contracts/
│   └── ui-ssl-rotary-knob-contract.md # Contrat visuel et automation hôte (/speckit-plan)
└── tasks.md             # Liste des tâches Phase 2 (généré par /speckit-tasks)
```

### Source Code (repository root)

```text
src/
└── lib.rs               # Code monolithique du plugin (DSP, Params, UI egui, Tests)

xtask/
└── src/main.rs          # Outil de bundling VST3 / CLAP

README.md                # Présentation des fonctionnalités et de l'interface
USER_GUIDE.md            # Guide d'utilisation des contrôles rotatifs et raccourcis
CHANGELOG.md             # Journal des modifications (section [Unreleased])
```

**Structure Decision**: Maintien strict de l'architecture monolithique dans `src/lib.rs` (bloc 4 `draw_omore_punch_ui` et bloc `tests`) conformément au Principe I de la Constitution, complété par la mise à jour de la documentation utilisateur.

## Complexity Tracking

Aucune violation de la Constitution — table non applicable.
