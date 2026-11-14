# Quickstart & Validation Guide: `[Snare Safe 200Hz]`

**Feature**: `001-snare-safe-200hz`
**Date**: 2026-11-14

## Prérequis
- Toolchain Rust stable (Edition 2021) sous Linux (Debian 13) ou Windows 11.

## 1. Contrôle de conformité ASCII (`src/lib.rs`)
Vérifier qu'aucun caractère non-ASCII n'a été introduit dans `src/lib.rs` :
```bash
LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs
```
**Résultat attendu** : Aucune sortie (code de retour `1`).

## 2. Exécution des tests unitaires DSP en mode Release
```bash
cargo test --release
```
**Résultat attendu** : Tous les tests unitaires passent (`ok`), y compris la vérification de l'immunité du détecteur avec `sc_hpf_hz = 200.0 Hz`.

## 3. Génération des bundles VST3 et CLAP
```bash
cargo run --package xtask --release -- bundle omore-punch --release
```
**Résultat attendu** : Génération sans erreur de `target/bundled/omore-punch.vst3` et `target/bundled/omore-punch.clap`.

## 4. Validation fonctionnelle dans Reaper
1. Charger `Omore Punch` sur un bus batterie stéréo dans Reaper.
2. Vérifier que le bouton `[Snare Safe 200Hz]` est affiché dans la section `1. COMPRESSOR & ENVELOPE BALLISTICS` à la place de `[Kick Safe 90Hz]`.
3. Survoler `[Snare Safe 200Hz]` et vérifier l'affichage de l'infobulle en anglais.
4. Cliquer sur `[Snare Safe 200Hz]` et vérifier que le paramètre `SC HPF` passe immédiatement à `200.0 Hz`.
5. Cliquer sur le bouton `Reset` de la ligne `SC HPF` et vérifier qu'il revient à `20.0 Hz`.
