# Implementation Plan: Compressor Auto-Release

**Branch**: `feat/004-compressor-auto-release` | **Date**: 2026-11-14 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/004-compressor-auto-release/spec.md`

## Summary

Ajouter un sélecteur de mode de relâchement à 4 états (`ReleaseMode`: `Manual` par défaut, `Auto Tight` 50/600 ms, `Auto Punch` 80/800 ms, `Auto Glue` 100/1200 ms) au compresseur de bus batterie d'`Omore Punch` dans `src/lib.rs`. Dans `StereoPunchDsp`, chaque mode Auto-Release implémente une courbe SSL G-Bus à double étage dépendant du programme (intégrateur lent `auto_slow_gr_db` à charge `180 ms` et décharge `600 / 800 / 1200 ms`, servant de plancher dynamique à un relâchement rapide `50 / 80 / 100 ms`), totalement indépendant du potentiomètre manuel `Release`, avec précalcul des coefficients sur changement de mode ou de fréquence d'échantillonnage et sanitisation anti-dénormaux/`NaN`. En mode `Manual`, le chemin d'enveloppe existant reste bit-à-bit inchangé. Côté interface `egui`, la section `1. COMPRESSOR & ENVELOPE BALLISTICS` accueille une rangée ASCII `Release Mode: [Manual] [Auto Tight] [Auto Punch] [Auto Glue]`, le potentiomètre `Release` est visuellement atténué avec indication `AUTO` tout en restant éditable lorsqu'un mode Auto est actif, le bouton `[Punch 3ms / 80ms]` repasse `release_mode` sur `Manual`, et la hauteur de fenêtre passe de `620` à `650` px (`820x650`, `UI_METER_HEIGHT = 416.0`) afin d'éviter tout débordement.

## Technical Context

**Language/Version**: Rust (Edition 2021)

**Primary Dependencies**: `nih_plug`, `nih_plug_egui` (`egui 0.31.1`), `atomic_float`

**Storage**: N/A (état des paramètres VST3/CLAP et `EguiState` persistés par l'hôte DAW via `nih-plug`)

**Testing**: `cargo test --release` (module `tests` intégré dans `src/lib.rs`)

**Target Platform**: Linux (Debian 13 x86_64) et Windows 11 (x86_64) dans Reaper (formats VST3 et CLAP)

**Project Type**: Audio Plugin (VST3 / CLAP) à architecture monolithique (`src/lib.rs`)

**Performance Goals**: Thread audio 100 % temps réel (zéro allocation, zéro verrou, coefficients `.exp()` précalculés et mis en cache) et rafraîchissement UI fluide à 60 FPS

**Constraints**: Fichier unique `src/lib.rs`, 100 % ASCII pur dans `src/lib.rs`, fenêtre `820x650` px sans chevauchement ni débordement, compatibilité ascendante totale des sessions DAW (paramètre `release_mode` par défaut sur `Manual`, aucun ID existant modifié)

**Scale/Scope**: Extensions ciblées des 4 blocs de `src/lib.rs` (`ReleaseMode`, `DspConfig`, `StereoPunchDsp`, `OmorePunchParams` / `process()`, `draw_omore_punch_ui`, et module `tests`), ainsi que la mise à jour de `README.md`, `USER_GUIDE.md` et `CHANGELOG.md` lors de l'implémentation

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. Monolithic Single-File Architecture (`src/lib.rs`)** : **PASS** — L'énumération `ReleaseMode`, les champs de `DspConfig` et `StereoPunchDsp` (Bloc 1), le paramètre `EnumParam<ReleaseMode>` dans `OmorePunchParams` et le câblage dans `process()` (Bloc 3), les contrôles UI dans `draw_omore_punch_ui` / `draw_ssl_knob_cell` (Bloc 4) et les nouveaux tests unitaires résident intégralement dans `src/lib.rs`. Aucun sous-module créé.
- **II. Pure ASCII Source** : **PASS** — Toutes les chaînes ajoutées (`"Release Mode:"`, `"[Manual]"`, `"[Auto Tight]"`, `"[Auto Punch]"`, `"[Auto Glue]"`, `"AUTO (...)"`, variantes `#[name = "..."]`, commentaires et tests) sont 100 % ASCII (`LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs`).
- **III. Real-Time Audio Thread Safety** : **PASS** — Zéro allocation mémoire et zéro verrou dans `StereoPunchDsp` et `process()`. Les appels `.exp()` des coefficients `auto_fast_release_coeff`, `auto_slow_release_coeff` et `auto_slow_charge_coeff` sont exécutés uniquement dans `update_ballistics()` lorsque `sample_rate` ou `release_mode` change (via `cached_release_mode`).
- **IV. Signal Sanitization & Safety Clamp** : **PASS** — `auto_slow_gr_db` et `env_gr_db` sont protégés contre `NaN`, `Inf` et les dénormaux (`< 1.0e-20 => 0.0`), réinitialisés dans `reset()`, bornés de manière cohérente (`auto_slow_gr_db <= env_gr_db`), et la sortie audio finale continue de passer par `sanitize_sample()` (`[-4.0, 4.0]`).
- **V. Mandatory DSP Unit Tests** : **PASS** — Tous les tests DSP existants restent intacts et de nouveaux tests numériques valident la différence transitoire court (`15 ms`) vs signal soutenu (`400 ms`) sur chaque profil et sur plusieurs fréquences d'échantillonnage (`44.1 / 48 / 96 kHz`), l'ordre strict `AutoTight > AutoPunch > AutoGlue`, la continuité sans clic ni divergence L/R lors d'un changement de mode en compression active, la compatibilité bit-à-bit en mode `Manual`, et la géométrie UI dans `820x650` px (`cargo test --release`).
- **VI. Single-Source Versioning** : **PASS** — `Cargo.toml` reste la source unique de version (`env!("CARGO_PKG_VERSION")`), inchangée jusqu'à une release explicite.
- **Platform, Format & GUI Constraints (Constitution v1.2.0)** : **PASS** — Thème sombre `Omore` conservé, boutons ASCII homogènes avec `Clipper Mode:`, potentiomètre `Release` visuellement atténué avec mention `AUTO` mais éditable, en-tête et colonnes sans chevauchement dans `820x650` px, export simultané VST3/CLAP.

*Post-Design Re-evaluation*: **PASS** — Aucune violation de la Constitution v1.2.0 ; `GEMINI.md` et `.specify/memory/constitution.md` restent intacts.

## Project Structure

### Documentation (this feature)

```text
specs/004-compressor-auto-release/
├── plan.md                                             # Plan d'implémentation (/speckit-plan)
├── research.md                                         # Décisions techniques Phase 0 (/speckit-plan)
├── data-model.md                                       # Modèle DSP, paramètres et géométrie Phase 1 (/speckit-plan)
├── quickstart.md                                       # Guide de validation Phase 1 (/speckit-plan)
├── contracts/
│   └── ui-and-dsp-auto-release-contract.md             # Contrat DSP, paramètres hôte et UI (/speckit-plan)
└── tasks.md                                            # Liste des tâches Phase 2 (généré par /speckit-tasks)
```

### Source Code (repository root)

```text
src/
└── lib.rs               # Code monolithique du plugin (DSP, Params, UI egui, Tests)

xtask/
└── src/main.rs          # Outil de bundling VST3 / CLAP

README.md                # Documentation des modes Auto-Release (lors de l'implémentation)
USER_GUIDE.md            # Guide d'utilisation de Release Mode (lors de l'implémentation)
CHANGELOG.md             # Journal des modifications sous [Unreleased] (lors de l'implémentation)
```

**Structure Decision**: Respect strict de l'architecture monolithique dans `src/lib.rs` (Principe I de la Constitution), complété lors de l'implémentation par la mise à jour de la documentation utilisateur (`README.md`, `USER_GUIDE.md`, `CHANGELOG.md`).

## Complexity Tracking

Aucune violation de la Constitution — table non applicable.
