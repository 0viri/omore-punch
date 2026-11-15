# Quickstart & Validation Guide: Tag Git dans le badge `[DEV]`

**Feature**: `002-dev-version-tag`
**Date**: 2026-11-14

## Prérequis
- Toolchain Rust stable (Edition 2021) et `git` dans le `PATH` sous Linux (Debian 13) ou Windows 11.
- `Cargo.toml` configuré en version de développement (ex. `0.2.1-dev`).

## 1. Contrôle de conformité ASCII (`src/lib.rs`)
```bash
LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs
```
**Résultat attendu** : Aucune sortie (code de retour `1`).

## 2. Exécution des tests unitaires en mode Release
```bash
cargo test --release
```
**Résultat attendu** : Tous les tests unitaires DSP, parsing (`parse_display_version`, `format_dev_badge_label`), métadonnées hôte et encombrement `egui` (avec `[DEV 1d2c5b9-dirty]` + `[CLIP: ACTIVE - CLICK TO RESET]` dans `820 px`) passent (`ok`).

## 3. Génération des bundles VST3 et CLAP
```bash
cargo run --package xtask --release -- bundle omore-punch --release
```
**Résultat attendu** : Génération sans erreur de `target/bundled/omore-punch.vst3` et `target/bundled/omore-punch.clap`.

## 4. Validation visuelle dans Reaper
1. Compiler localement avec des modifications non commitées (ou sur un commit propre) et charger le bundle dans Reaper.
2. Vérifier dans l'en-tête supérieur :
   - Sur arbre local modifié : `OMORE PUNCH v0.2.1` `[DEV <hash>-dirty]` `|  STEREO DRUM BUS COMPRESSOR & CLIPPER`.
   - Sur commit propre : `OMORE PUNCH v0.2.1` `[DEV <hash>]` `|  STEREO DRUM BUS COMPRESSOR & CLIPPER`.
   - Aucun chevauchement avec `[CLIP: OK]` ou `[CLIP: ACTIVE - CLICK TO RESET]` à droite.
3. Vérifier dans les infos FX de Reaper que la version exposée à l'hôte reste `0.2.1-dev`.
