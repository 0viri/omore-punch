# Implementation Plan: Section EQ 3 bandes pour Bus Batterie (`3. DRUM BUS EQ`)

**Branch**: `feat/003-drum-bus-eq` | **Date**: 2026-11-14 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/003-drum-bus-eq/spec.md`

## Summary

Ajouter une section dédiée `3. DRUM BUS EQ` dans `Omore Punch` (`src/lib.rs`) comprenant exactement trois fonctions via trois potentiomètres rotatifs style SSL (`Low Cut` de `20.0` à `160.0 Hz` avec bypass à `<= 20.5 Hz`, `500Hz Cut` atténuateur de `0.0` à `-12.0 dB` à `500 Hz` fixe, et `High Shelf` de `-6.0` à `+12.0 dB` à `10 kHz` fixe). Le traitement stéréo apparié est implémenté dans `StereoPunchDsp` par trois filtres biquads Direct Form II Transposed placés post-compresseur et immédiatement pré-clipper (`Compressor -> Drum Bus EQ -> Output Clipper -> Parallel Mix`), avec mise en cache des coefficients, sanitisation anti-dénormaux/`NaN` et bypass strict aux valeurs neutres. Côté interface `egui`, la section `3. DRUM BUS EQ` rejoint la colonne gauche (réutilisant `draw_ssl_knob_cell` avec `SslKnobColor::Filter`), le metering de la colonne droite est renuméroté `4. GAIN REDUCTION & STEREO BUS METERING`, et la hauteur de fenêtre passe de `480` à `620` px (`820x620`) afin d'éviter tout débordement.

## Technical Context

**Language/Version**: Rust (Edition 2021)

**Primary Dependencies**: `nih_plug`, `nih_plug_egui` (`egui 0.31.1`), `atomic_float`

**Storage**: N/A (état des paramètres et `EguiState` persistés par l'hôte DAW VST3/CLAP via `nih-plug`)

**Testing**: `cargo test --release` (module `tests` intégré dans `src/lib.rs`)

**Target Platform**: Linux (Debian 13 x86_64) et Windows 11 (x86_64) dans Reaper (formats VST3 et CLAP)

**Project Type**: Audio Plugin (VST3 / CLAP) à architecture monolithique (`src/lib.rs`)

**Performance Goals**: Thread audio 100 % temps réel (zéro allocation, zéro verrou, coefficients biquads précalculés/mis en cache) et rafraîchissement UI fluide à 60 FPS

**Constraints**: Fichier unique `src/lib.rs`, 100 % ASCII pur dans `src/lib.rs`, fenêtre `820x620` px sans chevauchement ni débordement, compatibilité ascendante totale des sessions DAW (aucun ID existant modifié)

**Scale/Scope**: Extensions ciblées des 4 blocs de `src/lib.rs` (`StereoPunchDsp` / `DspConfig`, `OmorePunchParams` / `process()`, `draw_omore_punch_ui`, et module `tests`), ainsi que la mise à jour de `README.md`, `USER_GUIDE.md` et `CHANGELOG.md`

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. Monolithic Single-File Architecture (`src/lib.rs`)** : **PASS** — Les structures biquads (`BiquadCoeffs`, `StereoBiquad`), les champs de `DspConfig` et `StereoPunchDsp`, les paramètres `OmorePunchParams`, la section UI `3. DRUM BUS EQ` dans `draw_omore_punch_ui` et les nouveaux tests résident intégralement dans `src/lib.rs`, chacun dans son bloc dédié. Aucun sous-module créé.
- **II. Pure ASCII Source** : **PASS** — Toutes les nouvelles chaînes (`"3. DRUM BUS EQ"`, `"4. GAIN REDUCTION & STEREO BUS METERING"`, `"Low Cut"`, `"500Hz Cut"`, `"High Shelf"`, commentaires et tests) sont 100 % ASCII (`LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs`).
- **III. Real-Time Audio Thread Safety** : **PASS** — Aucune allocation mémoire ni aucun verrou dans `StereoPunchDsp` ou `process()`. Les coefficients des 3 biquads utilisent `.sin()`, `.cos()`, `.sqrt()` et `10.0_f32.powf()` uniquement dans `update_eq()` lorsque `sample_rate` ou un paramètre EQ varie (`> 1.0e-4`), grâce aux valeurs mises en cache (`cached_eq_*`).
- **IV. Signal Sanitization & Safety Clamp** : **PASS** — Toutes les variables d'état internes des 3 biquads (`z1_l, z2_l, z1_r, z2_r`) sont protégées contre `NaN`, `Inf` et les dénormaux (`< 1.0e-20 => 0.0`), réinitialisées dans `reset()` et en mode bypass, et la sortie finale continue de passer par `sanitize_sample()` (`[-4.0, 4.0]`).
- **V. Mandatory DSP Unit Tests** : **PASS** — Tous les tests DSP existants restent intacts et de nouveaux tests unitaires numériques valident la transparence au repos, le Low Cut (`12 dB/oct`), le 500 Hz Cut, le High Shelf (`-6` à `+12 dB`), le respect du plafond Clipper post-EQ, la stabilité multi-sample-rate (`44.1 / 48 / 96 kHz`) et la géométrie UI dans `820x620` px (`cargo test --release`).
- **VI. Single-Source Versioning** : **PASS** — `Cargo.toml` reste la source unique de version (`env!("CARGO_PKG_VERSION")`), inchangée jusqu'à une release explicite.
- **Platform, Format & GUI Constraints (Constitution v1.1.0)** : **PASS** — Thème sombre `Omore` conservé, potentiomètres rotatifs SSL via `draw_ssl_knob_cell`, contrôles compacts `-`/`+`/`Reset`, en-tête et colonnes sans chevauchement, export simultané VST3/CLAP.

*Post-Design Re-evaluation*: **PASS** — Aucune violation de la Constitution v1.1.0 ; `GEMINI.md` et `.specify/memory/constitution.md` restent intacts.

## Project Structure

### Documentation (this feature)

```text
specs/003-drum-bus-eq/
├── plan.md                             # Plan d'implémentation (/speckit-plan)
├── research.md                         # Décisions techniques Phase 0 (/speckit-plan)
├── data-model.md                       # Modèle DSP, paramètres et géométrie Phase 1 (/speckit-plan)
├── quickstart.md                       # Guide de validation Phase 1 (/speckit-plan)
├── contracts/
│   └── ui-and-dsp-eq-contract.md       # Contrat DSP, paramètres hôte et UI (/speckit-plan)
└── tasks.md                            # Liste des tâches Phase 2 (généré par /speckit-tasks)
```

### Source Code (repository root)

```text
src/
└── lib.rs               # Code monolithique du plugin (DSP, Params, UI egui, Tests)

xtask/
└── src/main.rs          # Outil de bundling VST3 / CLAP

README.md                # Présentation des fonctionnalités et de l'EQ
USER_GUIDE.md            # Guide d'utilisation de la section 3. DRUM BUS EQ
CHANGELOG.md             # Journal des modifications (section [Unreleased])
```

**Structure Decision**: Respect strict de l'architecture monolithique dans `src/lib.rs` (Principe I de la Constitution), complété lors de l'implémentation par la mise à jour de la documentation utilisateur (`README.md`, `USER_GUIDE.md`, `CHANGELOG.md`).

## Complexity Tracking

Aucune violation de la Constitution — table non applicable.
