# Quickstart & Validation Guide: Section EQ 3 bandes (`3. DRUM BUS EQ`)

**Feature**: `003-drum-bus-eq`
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
- `test_eq_neutral_bypass_transparency` (transparence aux valeurs par défaut),
- `test_eq_low_cut_sub_attenuation` (atténuation 12 dB/oct sous la coupure à 44.1 / 48 / 96 kHz),
- `test_eq_500hz_cut_attenuation` (atténuation ciblée à 500 Hz sans creuser les graves ni les aigus),
- `test_eq_high_shelf_boost_and_cut` (boost et cut à 10 kHz + respect du plafond Clipper),
- `test_ui_layout_column_widths_fit_window` (zéro débordement horizontal et vertical dans `820x620` px).

## 3. Génération des bundles VST3 et CLAP
```bash
cargo run --package xtask --release -- bundle omore-punch --release
```
**Résultat attendu** : Génération sans erreur de `target/bundled/omore-punch.vst3` et `target/bundled/omore-punch.clap`.

## 4. Validation fonctionnelle dans Reaper
1. Charger `Omore Punch` sur un bus batterie stéréo dans Reaper.
2. Vérifier que la section `3. DRUM BUS EQ` apparaît sous la section `2.` avec exactement 3 potentiomètres rotatifs bleus (`Low Cut`, `500Hz Cut`, `High Shelf`), et que la colonne de droite affiche `4. GAIN REDUCTION & STEREO BUS METERING` sans aucun chevauchement.
3. Tester les 3 fonctions :
   - Monter `Low Cut` (ex. `60 Hz` à `120 Hz`) : nettoyage net du sous-grave sans pompage.
   - Baisser `500Hz Cut` (ex. `-3 dB` à `-6 dB`) : réduction immédiate du côté carton/boîte.
   - Pousser `High Shelf` (ex. `+3 dB` à `+6 dB`) : gain d'air sur les cymbales et l'attaque, avec les crêtes maîtrisées par le Clipper (`Clip Ceiling`).
4. Double-cliquer ou cliquer `Reset` sur chaque potard EQ et vérifier le retour à la transparence (`20.0 Hz`, `0.00 dB`, `0.00 dB`), puis vérifier la sauvegarde/rechargement d'un projet Reaper existant.
