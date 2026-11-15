# Implementation Plan: Métadonnées et Nommage du Bundle Plugin

**Branch**: `feat/005-plugin-bundle-metadata` | **Date**: 2026-11-14 | **Spec**: [spec.md](file:///home/tdavio/src/omore-punch/specs/005-plugin-bundle-metadata/spec.md)

**Input**: Feature specification from `/specs/005-plugin-bundle-metadata/spec.md`

## Summary

Exposer de manière fiable les métadonnées statiques d'Omore Punch (`Name = Omore Punch`, `Version` issue de `Cargo.toml`, `Manufacturer = Omore`, `Category = Fx|Dynamics|Distortion|Stereo`) pour les gestionnaires de plugins (dont Plugoff sous Windows) et les DAWs, tout en renommant les bundles distribués en `Omore Punch.vst3` et `Omore Punch.clap` sans casser les projets existants.

L'approche technique combine :
1. La configuration `bundler.toml` exploitée nativement par `nih_plug_xtask` pour générer `Omore Punch.vst3` et `Omore Punch.clap`.
2. L'extension de `xtask/src/main.rs` pour générer automatiquement `Contents/Resources/moduleinfo.json` dans le bundle VST3 à partir de la version de `Cargo.toml` et des métadonnées du plugin.
3. Un script `build.rs` (Windows uniquement, via `winres`) pour embarquer les ressources PE `VS_VERSION_INFO` dans les binaires Windows `.vst3` et `.clap`.
4. La mise à jour de `OmorePunch::VENDOR` vers `"Omore"` tout en conservant strictement invariants `VST3_CLASS_ID`, `CLAP_ID` et les IDs de paramètres, ainsi que le nettoyage des anciens bundles `omore-punch.*` dans `build.sh` et `build.ps1`.

## Technical Context

**Language/Version**: Rust (Edition 2021, toolchain stable)

**Primary Dependencies**: `nih_plug`, `nih_plug_egui`, `nih_plug_xtask`, `winres` (`[target.'cfg(windows)'.build-dependencies]` uniquement)

**Storage**: Fichiers de configuration et manifestes de bundle (`bundler.toml`, `Contents/Resources/moduleinfo.json`)

**Testing**: `cargo test --release` + inspection des artefacts générés dans `target/bundled/`

**Target Platform**: Linux (Debian 13 x86_64) et Windows 11 (x86_64) — formats VST3 et CLAP

**Project Type**: Plugin audio temps réel (VST3 / CLAP) avec outil de bundling workspace (`xtask`)

**Performance Goals**: Zéro impact sur le thread audio `process()` ; scan statique instantané par les gestionnaires de plugins sans charger la bibliothèque dynamique en mémoire

**Constraints**: Architecture monolithique (`src/lib.rs` pour tout le code plugin), 100 % ASCII dans `src/lib.rs`, version issue uniquement de `Cargo.toml`, identifiants `VST3_CLASS_ID` et `CLAP_ID` invariants

**Scale/Scope**: `src/lib.rs` (`VENDOR` + tests unitaires), `Cargo.toml`, `bundler.toml`, `build.rs`, `xtask/src/main.rs`, `xtask/Cargo.toml` (si dépendance JSON/metadata requise), `build.sh`, `build.ps1`, `.github/workflows/build.yml`

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

### Évaluation pré-design & post-design

| Principe / Contrainte | Statut | Justification |
|-----------------------|--------|---------------|
| **I. Monolithic Single-File Architecture** | PASS | Tout le code du plugin reste intégralement dans `src/lib.rs`. `bundler.toml`, `build.rs` et `xtask/src/main.rs` appartiennent à l'outillage de build/bundling (explicitement hors du périmètre de cette règle). |
| **II. Pure ASCII Source** | PASS | La chaîne `VENDOR = "Omore"` et les tests unitaires ajoutés dans `src/lib.rs` sont 100 % ASCII. |
| **III. Real-Time Audio Thread Safety** | PASS | Aucune modification du moteur DSP (`StereoPunchDsp`) ni de `process()`. |
| **IV. Signal Sanitization & Safety Clamp** | PASS | Inchangé ; `sanitize_sample()` et `test_nan_inf_and_safety_clamp` restent intacts. |
| **V. Mandatory DSP Unit Tests** | PASS | Tous les tests DSP existants restent inchangés et passants ; des assertions unitaires vérifient en plus `OmorePunch::VENDOR`, `OmorePunch::NAME`, `OmorePunch::VST3_CLASS_ID` et `OmorePunch::CLAP_ID`. |
| **VI. Single-Source Versioning** | PASS | `Cargo.toml` demeure l'unique source de vérité pour la version, lue dynamiquement par `src/lib.rs` (`env!("CARGO_PKG_VERSION")`), `build.rs` (`CARGO_PKG_VERSION*`) et `xtask/src/main.rs`. |
| **Platform, Format & GUI Constraints -> Bundling** | DEVIATION JUSTIFIÉE | La constitution mentionne la production de `omore-punch.vst3` et `omore-punch.clap` dans `target/bundled/`. Suite à la demande explicite de l'utilisateur et aux exigences clarifiées (`FR-004`, `FR-006`), les artefacts produits deviennent `Omore Punch.vst3` et `Omore Punch.clap`. Conformément à `GEMINI.md`, `.specify/memory/constitution.md` n'est pas modifié hors `/speckit-constitution` ; l'écart est justifié dans `Complexity Tracking` ci-dessous. |

## Project Structure

### Documentation (this feature)

```text
specs/005-plugin-bundle-metadata/
├── spec.md                                   # Feature specification (/speckit-specify + /speckit-clarify)
├── checklists/
│   └── requirements.md                       # Spec quality validation checklist
├── plan.md                                   # This file (/speckit-plan output)
├── research.md                               # Phase 0 decisions & alternatives
├── data-model.md                             # Phase 1 metadata entities & invariants
├── quickstart.md                             # Phase 1 validation guide
└── contracts/
    └── bundle-metadata-contract.md           # Phase 1 bundle tree, moduleinfo.json & VERSIONINFO contracts
```

### Source Code (repository root)

```text
.
├── Cargo.toml                                # Source unique de version, authors = ["Omore"], build-dep Windows (winres)
├── bundler.toml                              # [omore-punch] name = "Omore Punch" pour xtask
├── build.rs                                  # Ressources PE VS_VERSION_INFO sous Windows uniquement
├── src/
│   └── lib.rs                                # VENDOR = "Omore", invariants VST3_CLASS_ID/CLAP_ID, tests unitaires
├── xtask/
│   ├── Cargo.toml                            # Dépendances xtask (génération moduleinfo.json)
│   └── src/
│       └── main.rs                           # Appel nih_plug_xtask + génération Contents/Resources/moduleinfo.json
├── build.sh                                  # Nettoyage omore-punch.* + installation Omore Punch.* (Linux)
├── build.ps1                                 # Nettoyage omore-punch.* + installation Omore Punch.* (Windows)
└── .github/
    └── workflows/
        └── build.yml                         # Upload des artefacts Omore Punch.vst3 & Omore Punch.clap
```

**Structure Decision**: Conserver strictement l'architecture existante du dépôt (`src/lib.rs` monolithique + workspace `xtask/`), en ajoutant uniquement `bundler.toml` et `build.rs` à la racine du package pour la configuration de bundling et les ressources Win32, et en étendant `xtask/src/main.rs` pour générer `moduleinfo.json` dans le bundle VST3.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| Écart sur `Platform, Format & GUI Constraints -> Bundling` (`Omore Punch.vst3` / `Omore Punch.clap` dans `target/bundled/` au lieu de `omore-punch.vst3` / `omore-punch.clap`) | L'utilisateur souhaite que le plugin s'affiche sous le nom propre `Omore Punch` dans les gestionnaires de plugins (comme Plugoff) et les explorateurs de fichiers, à l'image de *Omore Sampler* (`FR-004`, `FR-006`). La constitution devra être synchronisée ultérieurement via `/speckit-constitution`. | Conserver les noms de fichiers `omore-punch.vst3` / `omore-punch.clap` laisserait l'ancien identifiant technique visible dans les gestionnaires et dossiers système lorsqu'ils lisent le nom du bundle. |
