# Implementation Plan: Bouton Bypass Global

**Branch**: `feat/008-bypass-button` (`008-bypass-button`) | **Date**: 2026-11-15 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/008-bypass-button/spec.md`

## Summary

Ajouter un bouton **Bypass** global dans Omore Punch entierement integre au protocole DAW (VST3 `kIsBypass` & CLAP `CLAP_PARAM_IS_BYPASS` via `BoolParam::make_bypass()`), avec transition soft-bypass anti-clic par rampe lineaire de `10 ms` dans `StereoPunchDsp`, transparence totale en bypass etabli sous protection `sanitize_sample()`, gel des memoires de mesure (`Max GR`, `Peak GR Hold`, `[CLIP: ACTIVE]` preexistant) et du `Gain Match` pendant le bypass, remise a `0.0 dB` de `GR` instantane (y compris vers l'hote DAW), bouton `[BYPASS: OFF]` / `[BYPASS: ON]` dans la barre d'en-tete `egui` (sans tronquer les libelles constitutionnels du titre ni de `[CLIP: ACTIVE - CLICK TO RESET]`, en ajustant l'espacement/taille de police ou `UI_WINDOW_WIDTH` si necessaire), et attenuation visuelle (`opacity = 0.55`) des sections de traitement sans bloquer l'edition des parametres.

## Technical Context

**Language/Version**: Rust (Edition 2021)
**Primary Dependencies**: `nih_plug` (feature `assert_process_allocs`), `nih_plug_egui` (`egui 0.31.1`), `atomic_float` (aucune nouvelle dependance)
**Storage**: Etat de session/preset DAW via `#[derive(Params)]` (`#[id = "bypass"]`)
**Testing**: `cargo test --release` (tests unitaires DSP, ABI parametres VST3/CLAP et tests headless `egui` dans `src/lib.rs`)
**Target Platform**: Linux x86_64 (Debian 13) et Windows 11 x86_64 — Reaper VST3 & CLAP
**Project Type**: Plugin audio monolithique (`cdylib` + `lib`)
**Performance Goals**: Zero allocation et zero verrou sur le thread audio temps reel (`process`), pas de rampe precalcule au changement de `sample_rate`, rafraichissement UI 60 FPS
**Constraints**: Architecture monolithique dans `src/lib.rs`, 100 % pure ASCII dans `src/lib.rs`, sanitisation + safety clamp `[-4.0, 4.0]` maintenus meme en bypass, aucune regression de layout dans `UI_WINDOW_WIDTH x UI_WINDOW_HEIGHT`
**Scale/Scope**: Modification ciblee des 4 blocs de `src/lib.rs` + tests unitaires dans `mod tests` + entree `CHANGELOG.md`

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principe | Verification | Statut |
|---|---|---|
| **I. Monolithic Single-File Architecture** | Toutes les modifications (`DspConfig`, `StereoPunchDsp`, `OmorePunchParams`, `OmorePunch::process`, `draw_header_bar`, `draw_omore_punch_ui`, `mod tests`) restent dans les 4 blocs de `src/lib.rs`. | PASS |
| **II. Pure ASCII Source** | Tous les identifiants, commentaires et libelles UI (`[BYPASS: OFF]`, `[BYPASS: ON]`) sont strictement ASCII. Controle via `LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs`. | PASS |
| **III. Real-Time Audio Thread Safety** | Zero allocation tas et zero verrou dans `process()` et `StereoPunchDsp`. `bypass_step` precalcule uniquement quand `sample_rate` change. Pas d'appel `.exp()`/`.sin()`/`.cos()`/`.tan()` par echantillon. | PASS |
| **IV. Signal Sanitization & Safety Clamp** | Meme lorsque `bypass == true`, les echantillons d'entree et de sortie passent par `sanitize_sample()` (protection `NaN`, `Inf`, denormaux et clamp `[-4.0, 4.0]`). `test_nan_inf_and_safety_clamp` reste couvert et etendu au mode bypass. | PASS |
| **V. Mandatory DSP Unit Tests** | Ajout de tests numeriques verifiant transparence `<-120 dBFS`, transition lisse `< 20 ms` (activation, desactivation, bascules rapides), sanitisation en bypass, gel du Gain Match, `gr_db == 0.0` / `clipped == false`, flags `BYPASS` en CLAP/VST3, et tests headless `egui` (header + layout). | PASS |
| **VI. Single-Source Versioning** | `Cargo.toml` reste sur sa version `-dev` courante (`env!("CARGO_PKG_VERSION")` inchange). | PASS |

*Re-check post-Phase 1 design*: Aucune deviation ni complexite injustifiee apres redaction de [research.md](research.md), [data-model.md](data-model.md), [contracts/bypass-contract.md](contracts/bypass-contract.md) et [quickstart.md](quickstart.md).

## Project Structure

### Documentation (this feature)

```text
specs/008-bypass-button/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── checklists/
│   └── requirements.md
└── contracts/
    └── bypass-contract.md
```

### Source Code (repository root)

```text
src/
└── lib.rs               # Bloc 1: DspConfig + StereoPunchDsp (soft bypass 10ms, gel Gain Match)
                         # Bloc 2/3: OmorePunchParams (bypass BoolParam::make_bypass()) + OmorePunch::process
                         # Bloc 4: draw_header_bar ([BYPASS: OFF/ON]) + draw_omore_punch_ui (opacity 0.55)
                         # Bloc 5: tests unitaires DSP, ABI & egui
CHANGELOG.md             # Entree sous ## [Unreleased] (Added)
```

**Structure Decision**: Aucun nouveau fichier source Rust cree (Principe I). L'implementation est entierement contenue dans `src/lib.rs`, avec documentation de release sous `## [Unreleased]` dans `CHANGELOG.md`.

## Complexity Tracking

Aucune violation de la Constitution — table non applicable.
