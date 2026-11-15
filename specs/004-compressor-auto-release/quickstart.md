# Quickstart & Validation Guide: Compressor Auto-Release (`004-compressor-auto-release`)

**Feature**: `004-compressor-auto-release`
**Date**: 2026-11-14

## Prérequis
- Toolchain Rust stable (Edition 2021) sous Linux (Debian 13) ou Windows 11.

## 1. Contrôle de conformité ASCII (`src/lib.rs`)
Vérifier qu'aucun caractère non-ASCII n'a été introduit dans `src/lib.rs` :
```bash
LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs
```
**Résultat attendu** : Aucune sortie (code de retour `1`).

## 2. Exécution des tests unitaires DSP et Géométrie UI en mode Release
```bash
cargo test --release
```
**Résultat attendu** : Tous les tests unitaires existants et nouveaux passent (`ok`), notamment :
- `test_auto_release_transient_vs_sustained_recovery` (récupération 90 % au moins 2x plus rapide après un transitoire de `15 ms` qu'après un passage soutenu de `400 ms` sur chacun des 3 profils, et cohérence à `44.1 / 48 / 96 kHz`),
- `test_auto_release_profile_ordering` (ordre strict de vitesse `AutoTight > AutoPunch > AutoGlue`),
- `test_auto_release_mode_switch_continuity_and_stereo_link` (aucun saut d'enveloppe ni `NaN`/`Inf` lors d'un changement de mode en compression active, parité stéréo L/R < `0.001 dB`),
- `test_auto_release_manual_bit_exact_compatibility` (sortie identique en mode `Manual` même lorsque `release_ms` varie, et pas d'impact du bouton manuel sur les modes Auto),
- `test_ui_layout_column_widths_fit_window` (zéro débordement horizontal et vertical dans `820x650` px).

## 3. Génération des bundles VST3 et CLAP
```bash
cargo run --package xtask --release -- bundle omore-punch --release
```
**Résultat attendu** : Génération sans erreur de `target/bundled/omore-punch.vst3` et `target/bundled/omore-punch.clap`.

## 4. Validation fonctionnelle dans Reaper
1. Charger `Omore Punch` sur un bus batterie stéréo dans Reaper.
2. Vérifier que la section `1. COMPRESSOR & ENVELOPE BALLISTICS` affiche la rangée `Release Mode: [Manual] [Auto Tight] [Auto Punch] [Auto Glue]` sans aucun chevauchement.
3. Sélectionner `Auto Tight`, `Auto Punch`, puis `Auto Glue` :
   - Vérifier que le potentiomètre `Release` apparaît visuellement atténué avec la mention `AUTO (...)`, tout en restant éditable pour préparer la valeur manuelle.
   - Écouter la différence entre des coups isolés (récupération rapide du punch) et des cymbales/roulements soutenus (relâchement progressif sans pompage).
4. Cliquer sur `[Punch 3ms / 80ms]` : vérifier que `Attack` passe à `3.00 ms`, `Release` à `80.0 ms`, et que `Release Mode` repasse sur `[Manual]` (potentiomètre `Release` actif sans mention `AUTO`).
5. Sauvegarder et recharger un projet Reaper pour vérifier la persistance de `Release Mode` et de la valeur manuelle pré-réglée.
