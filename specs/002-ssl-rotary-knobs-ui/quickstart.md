# Quickstart & Validation Guide: Interface type SSL avec potentiomètres rotatifs en grille

**Feature**: `002-ssl-rotary-knobs-ui`
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
**Résultat attendu** : Tous les tests unitaires passent (`ok`), y compris les tests DSP et le test de géométrie de la grille de potentiomètres (`test_ui_layout_column_widths_fit_window`).

## 3. Génération des bundles VST3 et CLAP
```bash
cargo run --package xtask --release -- bundle omore-punch --release
```
**Résultat attendu** : Génération sans erreur de `target/bundled/omore-punch.vst3` et `target/bundled/omore-punch.clap`.

## 4. Validation fonctionnelle dans Reaper
1. Installer et charger `Omore Punch` sur un bus batterie stéréo dans Reaper.
2. Vérifier que les 9 paramètres continus sont affichés sous forme de boutons ronds gradués à capuchons colorés style SSL (4+3 boutons dans la section `1.`, 2 boutons dans la section `2.`) sans aucun chevauchement avec la colonne de droite (`3. GAIN REDUCTION & STEREO BUS METERING`).
3. Cliquer et glisser verticalement sur un bouton rond : vérifier la rotation fluide du repère angulaire et la mise à jour de la lecture numérique.
4. Maintenir `Shift` tout en glissant verticalement : vérifier que le réglage est 4 fois plus fin.
5. Double-cliquer sur un bouton rond (ou cliquer sur son bouton `Reset`) : vérifier le retour immédiat à la valeur par défaut.
6. Cliquer sur les petits boutons `-` et `+` sous un bouton rond : vérifier l'incrément/décrément exact au pas défini.
