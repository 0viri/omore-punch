# Implementation Plan: Remplacement du bouton rapide Kick Safe 90Hz par Snare Safe 200Hz

**Branch**: `feat/001-snare-safe-200hz` | **Date**: 2026-11-14 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/001-snare-safe-200hz/spec.md`

## Summary

Remplacer le bouton d'action rapide `[Kick Safe 90Hz]` par `[Snare Safe 200Hz]` dans l'interface graphique `egui` d'Omore Punch (`src/lib.rs`) afin d'appliquer en un clic un filtre passe-haut de détection (`SC HPF`) à `200.0 Hz` selon la technique de bus batterie de Cristian Kohle (empêchant à la fois le sous-grave de la grosse caisse et le corps fondamental de la caisse claire de faire pomper le compresseur). Mettre à jour l'infobulle associée (100 % ASCII), couvrir le réglage `200.0 Hz` dans les tests unitaires DSP et synchroniser la documentation du projet (`README.md`, `USER_GUIDE.md`, `CHANGELOG.md`, `GEMINI.md`).

## Technical Context

**Language/Version**: Rust (Edition 2021)

**Primary Dependencies**: `nih_plug`, `nih_plug_egui` (`egui`)

**Storage**: N/A (état des paramètres géré par l'hôte DAW VST3/CLAP via `nih-plug`)

**Testing**: `cargo test --release` (module `tests` intégré dans `src/lib.rs`)

**Target Platform**: Linux (Debian 13 x86_64) et Windows 11 (x86_64) dans Reaper (formats VST3 et CLAP)

**Project Type**: Audio Plugin (VST3 / CLAP) à architecture monolithique (`src/lib.rs`)

**Performance Goals**: Thread audio 100 % temps réel (zéro allocation, zéro verrou, coefficients précalculés) et rafraîchissement UI fluide à 60 FPS

**Constraints**: Fichier unique `src/lib.rs`, 100 % ASCII pur dans `src/lib.rs`, compatibilité ascendante totale des sessions DAW (`#[id = "sc_hpf_hz"]` inchangé)

**Scale/Scope**: Modification ciblée du bloc 4 (`draw_omore_punch_ui`) et du module `tests` dans `src/lib.rs`, plus mise à jour de `README.md`, `USER_GUIDE.md`, `CHANGELOG.md` et `GEMINI.md`

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. Monolithic Single-File Architecture (`src/lib.rs`)** : **PASS** — Toutes les modifications de code et de test restent dans `src/lib.rs` (bloc 4 `draw_omore_punch_ui` et module `tests`). Aucun sous-module créé.
- **II. Pure ASCII Source** : **PASS** — Le nouveau libellé `"[Snare Safe 200Hz]"` et la nouvelle infobulle `"Set Sidechain HPF to 200.0 Hz so Kick and Snare body (up to 200 Hz) do not pump the Drum Bus"` sont 100 % ASCII. Vérifié par `LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs`.
- **III. Real-Time Audio Thread Safety** : **PASS** — Aucun changement dans `process()` ni dans l'algorithme de `StereoPunchDsp`. Le bouton rapide agit uniquement côté UI via `ParamSetter` et le coefficient du filtre `sc_hpf_coeff` reste mis en cache et recalculé uniquement lors d'une variation de `sc_hpf_hz`.
- **IV. Signal Sanitization & Safety Clamp** : **PASS** — `sanitize_sample()` et la protection des états internes du filtre de sidechain restent intacts.
- **V. Mandatory DSP Unit Tests** : **PASS** — Le test unitaire du filtre passe-haut de sidechain dans `src/lib.rs` vérifiera le comportement numérique avec la coupure à `200.0 Hz` (`cargo test --release`).
- **VI. Single-Source Versioning** : **PASS** — Aucun numéro de version codé en dur ; `Cargo.toml` reste la source unique (non modifié hors release).

*Post-Design Re-evaluation*: **PASS** — Aucun écart par rapport aux principes I à VI.

## Project Structure

### Documentation (this feature)

```text
specs/001-snare-safe-200hz/
├── plan.md              # Plan d'implémentation (/speckit-plan)
├── research.md          # Décisions techniques Phase 0 (/speckit-plan)
├── data-model.md        # Modèle de paramètre et transitions Phase 1 (/speckit-plan)
├── quickstart.md        # Guide de validation Phase 1 (/speckit-plan)
├── contracts/
│   └── ui-quick-action-contract.md # Contrat UI et automation hôte (/speckit-plan)
└── tasks.md             # Liste des tâches Phase 2 (généré par /speckit-tasks)
```

### Source Code (repository root)

```text
src/
└── lib.rs               # Code monolithique du plugin (DSP, Params, UI egui, Tests)

xtask/
└── src/main.rs          # Outil de bundling VST3 / CLAP

README.md                # Présentation des fonctionnalités clés
USER_GUIDE.md            # Guide de réglage rapide du bus batterie
CHANGELOG.md             # Journal des modifications (section [Unreleased])
GEMINI.md                # Directives du projet (exemple de bouton section 4)
```

**Structure Decision**: Respect strict de l'architecture monolithique d'Omore Punch dans `src/lib.rs` (Principe I de la Constitution) accompagnée de la mise à jour des fichiers de documentation à la racine du dépôt.

## Complexity Tracking

Aucune violation de la Constitution — table non applicable.
