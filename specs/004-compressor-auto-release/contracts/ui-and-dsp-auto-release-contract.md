# UI & DSP Contract: Compressor Auto-Release (`004-compressor-auto-release`)

**Feature**: `004-compressor-auto-release`
**Date**: 2026-11-14

## 1. Contrat des Paramètres Hôte VST3 / CLAP (`OmorePunchParams`)
- Un nouveau paramètre `EnumParam<ReleaseMode>` exposé à l'hôte, automatable et persisté dans l'état DAW :
  - `#[id = "release_mode"]` (`"Release Mode"`) : 4 valeurs (`Manual`, `Auto Tight (50/600 ms)`, `Auto Punch (80/800 ms)`, `Auto Glue (100/1200 ms)`), défaut `ReleaseMode::Manual`.
- **Rétrocompatibilité** : Aucun `#[id = "..."]` existant (`release_ms`, `attack_ms`, etc.) n'est renommé ou supprimé. Les projets Reaper sauvegardés avant cette fonctionnalité chargent avec `ReleaseMode::Manual`, garantissant un comportement audio strictement identique aux versions précédentes.

## 2. Contrat DSP (`StereoPunchDsp`)
- **Identité en mode `Manual`** (`SC-003`) : Lorsque `cfg.release_mode == ReleaseMode::Manual`, la trajectoire de `env_gr_db` et les échantillons de sortie `(out_l, out_r)` sont bit-à-bit identiques au compresseur d'origine pour un même signal et une même configuration.
- **Comportement dépendant du programme en mode Auto** (`SC-001`) :
  - Pour chacun des 3 profils (`AutoTight`, `AutoPunch`, `AutoGlue`), sur un transitoire court (ex. `15 ms` au-dessus du seuil suivi de silence), `env_gr_db` récupère `90%` de sa réduction de gain au moins `2x` plus vite que sur un passage soutenu (ex. `400 ms` au-dessus du seuil suivi de silence) aux mêmes réglages de seuil et ratio.
  - La vitesse de récupération suit l'ordre strict `AutoTight > AutoPunch > AutoGlue` (aussi bien sur transitoire court que sur passage soutenu).
  - La constante de temps effective s'adapte proportionnellement à la fréquence d'échantillonnage (`44.1 kHz`, `48 kHz`, `96 kHz`, `192 kHz`).
- **Stéréo liée & continuité lors des changements de mode** (`SC-002`) :
  - Les canaux gauche et droit reçoivent la même réduction de gain (parité L/R `< 0.001 dB` sur signal mono/stéréo symétrique, même en basculant de mode au milieu d'une compression active).
  - Basculer entre `Manual`, `AutoTight`, `AutoPunch` et `AutoGlue` en pleine réduction de gain ne produit ni `NaN`, ni `Inf`, ni discontinuité brusque d'enveloppe.
- **Sécurité Temps Réel & Anti-Dénormaux** (Principes III et IV) :
  - Zéro allocation mémoire, zéro verrou, aucun appel `.exp()` par échantillon tant que `sample_rate` et `release_mode` restent inchangés.
  - `auto_slow_gr_db` et `env_gr_db` sont protégés contre `NaN`, `Inf` et les dénormaux (`< 1.0e-20 => 0.0`), et la sortie passe toujours par `sanitize_sample()` (`[-4.0, 4.0]`).

## 3. Contrat UI (`egui`, 100 % ASCII)
- **Section `1. COMPRESSOR & ENVELOPE BALLISTICS`** :
  - Affiche un sélecteur horizontal 4 états en ASCII pur : `Release Mode: [Manual] [Auto Tight] [Auto Punch] [Auto Glue]`.
  - Le bouton correspondant au mode actif est surligné (`rgb(195, 125, 20)`).
  - Lorsqu'un mode Auto est actif (`release_mode != ReleaseMode::Manual`), le potentiomètre manuel `Release` est visuellement atténué (capuchon et graduations assombris) et affiche une mention ASCII `AUTO` dans sa lecture (`AUTO (<valeur>)`), tout en restant manipulable (`drag`, `Shift`, double-clic, `-`, `+`, `Reset`) pour pré-régler la valeur manuelle.
  - Cliquer sur `[Punch 3ms / 80ms]` positionne `attack_ms = 3.0`, `release_ms = 80.0` ET `release_mode = ReleaseMode::Manual`.
- **Dimensions de fenêtre** : `820 x 650` px (avec `UI_METER_HEIGHT = 416.0` px en colonne droite), sans aucun chevauchement ni débordement (`test_ui_layout_column_widths_fit_window`).
- **Encodage** : 100 % ASCII strict dans `src/lib.rs`.
