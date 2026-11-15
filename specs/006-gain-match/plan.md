# Implementation Plan: Gain Match Automatique

**Branch**: `feat/006-gain-match` (`006-gain-match`) | **Date**: 2026-11-14 | **Spec**: [spec.md](file:///home/tdavio/src/omore-punch/.worktrees/006-gain-match/specs/006-gain-match/spec.md)

**Input**: Feature specification from `specs/006-gain-match/spec.md`

## Summary

Implémenter une fonction **Gain Match** post-traitement dans `src/lib.rs` qui égalise automatiquement le volume RMS stéréo de sortie du plugin sur le volume RMS d'entrée brut (avant `input_db`), appliquée en toute fin de chaîne (après compression, `Auto Makeup`/`Makeup Gain`, `Drum Bus EQ`, `Output Clipper` et `Parallel Mix`, juste avant `sanitize_sample()`). La mesure utilise deux enveloppes RMS stéréo liées appariées (`400 ms`), un détecteur de présence à `-60 dBFS` (`50 ms`) figeant (`Hold`) la correction pendant les silences, un lissage progressif (`150 ms` actif / `35 ms` retour à `0 dB` sur `OFF`) borné à `[-24.0, +24.0]` dB, un paramètre DAW `gain_match` (`OFF` par défaut), une télémétrie atomique sans verrou, et un bouton `[Gain Match: OFF/ON]` avec lecture `Trim: +0.0 dB` dans la section `4. GAIN REDUCTION & STEREO BUS METERING` de l'UI `egui`.

## Technical Context

**Language/Version**: Rust (Edition 2021)
**Primary Dependencies**: `nih_plug` (avec feature `assert_process_allocs`), `nih_plug_egui`, `atomic_float` (aucune nouvelle dépendance requise)
**Storage**: État de session/preset DAW via `#[derive(Params)]` (`#[id = "gain_match"]`)
**Testing**: `cargo test --release` (tests unitaires DSP et tests de mise en page headless `egui` dans `src/lib.rs`)
**Target Platform**: Linux x86_64 (Debian 13) et Windows 11 x86_64 — Reaper VST3 & CLAP
**Project Type**: Plugin audio monolithique (`cdylib` + `lib`)
**Performance Goals**: Zéro allocation / zéro verrou sur le thread audio temps réel (`process`), coût scalaire négligeable par échantillon, rafraîchissement UI fluide à 60 FPS
**Constraints**: Architecture monolithique dans `src/lib.rs`, 100 % pure ASCII dans `src/lib.rs`, coefficients exponentiels précalculés et mis en cache, sanitisation + safety clamp `[-4.0, 4.0]` sur la sortie
**Scale/Scope**: Modification ciblée des 4 blocs de `src/lib.rs` + tests unitaires + entrée `CHANGELOG.md`

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principe | Vérification | Statut |
|---|---|---|
| **I. Monolithic Single-File Architecture** | Tout le code DSP (`DspConfig`, `SampleOutput`, `StereoPunchDsp`), télémétrie (`MeterTelemetry`), paramètres (`OmorePunchParams`), `process()`, UI (`draw_omore_punch_ui`) et tests unitaires reste dans `src/lib.rs` dans leurs 4 blocs respectifs. | PASS |
| **II. Pure ASCII Source** | Tous les identifiants, commentaires et libellés UI ajoutés dans `src/lib.rs` (`[Gain Match: OFF]`, `[Gain Match: ON]`, `Trim: +0.0 dB`) sont strictement ASCII. Vérifié par `LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs`. | PASS |
| **III. Real-Time Audio Thread Safety** | Zéro allocation tas, zéro `Mutex`/`RwLock` dans `process()` et `StereoPunchDsp`. Communication audio → UI via `MeterTelemetry::gain_match_db: AtomicU32` (`Ordering::Relaxed`). Coefficients `gm_rms_coeff`, `gm_presence_coeff`, `gm_slew_coeff`, `gm_off_slew_coeff` précalculés uniquement lors d'un changement de `sample_rate`. | PASS |
| **IV. Signal Sanitization & Safety Clamp** | Entrées déjà sanitisées ; toutes les variables d'état (`gm_in_ms`, `gm_post_ms`, `gm_presence_ms`, `gm_gain_db`) protégées contre `NaN`/`Inf`/dénormaux ; trim borné à `[-24.0, 24.0]` dB ; sortie finale passée par `sanitize_sample()` (`[-4.0, 4.0]`). | PASS |
| **V. Mandatory DSP Unit Tests** | Ajout de tests numériques dans `src/lib.rs` couvrant convergence `<= 0.5 dB` (compression, EQ, clipper), symétrie L/R, comportement `Hold` sur silence/bruit, bornage `[-24, +24]` dB, transition lisse ON/OFF, coexistence `Auto Makeup`/`Makeup Gain`, et test headless `egui` dans `820x650`. | PASS |
| **VI. Single-Source Versioning** | `Cargo.toml` et `env!("CARGO_PKG_VERSION")` inchangés (feature sur version `-dev` courante). | PASS |

*Re-check post-Phase 1 design*: Aucune déviation identifiée après rédaction de `research.md`, `data-model.md` et `contracts/plugin-contract.md`.

## Project Structure

### Documentation (this feature)

```text
specs/006-gain-match/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── checklists/
│   └── requirements.md
└── contracts/
    └── plugin-contract.md
```

### Source Code (repository root)

```text
src/
└── lib.rs               # Blocs 1-4 (DSP, Telemetry, Params/Plugin, UI) + mod tests
CHANGELOG.md             # Ajout de l'entrée sous ## [Unreleased]
```

**Structure Decision**: Aucun nouveau fichier source Rust créé, conformément au Principe I de la Constitution. Toutes les modifications fonctionnelles portent sur `src/lib.rs` (les 4 blocs isolés + `mod tests`), avec une entrée dans `CHANGELOG.md` sous `## [Unreleased]`.

## Complexity Tracking

Aucune violation de la Constitution — table non applicable.
