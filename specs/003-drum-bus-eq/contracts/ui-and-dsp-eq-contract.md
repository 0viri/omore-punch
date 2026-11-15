# UI & DSP Contract: Section EQ 3 bandes (`3. DRUM BUS EQ`)

**Feature**: `003-drum-bus-eq`
**Date**: 2026-11-14

## 1. Contrat des Paramètres Hôte VST3 / CLAP (`OmorePunchParams`)
- Trois nouveaux paramètres `FloatParam` exposés à l'hôte, tous automatable et persistés dans l'état DAW :
  1. `#[id = "eq_low_cut_hz"]` (`"EQ Low Cut"`) : `20.0..=160.0 Hz`, skewed (`-1.0`), défaut `20.0 Hz`, lissage `Linear(5.0)`.
  2. `#[id = "eq_mid_cut_db"]` (`"EQ 500Hz Cut"`) : `-12.0..=0.0 dB`, linéaire, défaut `0.0 dB`, lissage `Linear(5.0)`.
  3. `#[id = "eq_high_shelf_db"]` (`"EQ High Shelf"`) : `-6.0..=12.0 dB`, linéaire, défaut `0.0 dB`, lissage `Linear(5.0)`.
- **Rétrocompatibilité** : Aucun `#[id = "..."]` existant n'est renommé ou supprimé. Les sessions Reaper créées avant l'ajout de l'EQ chargent avec les valeurs par défaut (`20.0 Hz`, `0.0 dB`, `0.0 dB`), garantissant un son strictement identique aux versions précédentes jusqu'à ce que l'utilisateur touche l'EQ.

## 2. Contrat DSP (`StereoPunchDsp`)
- **Placement dans la chaîne** : Post-compresseur (après `comp_gain`) et immédiatement pré-clipper (`Compressor -> Drum Bus EQ -> Output Clipper -> Parallel Mix`).
- **Transparence au repos** : Avec `eq_low_cut_hz <= 20.5`, `eq_mid_cut_db == 0.0` et `eq_high_shelf_db == 0.0`, les trois biquads sont en bypass strict (sortie identique à l'entrée avant clipper).
- **Garantie spectrale par bande** (identique sur L et R) :
  - **Low Cut** (`12 dB/oct` Butterworth) : À `120 Hz` (ou `160 Hz`), atténuation forte des sous-graves (`40 Hz`) tout en préservant les hautes fréquences (`>= 1 kHz`).
  - **500 Hz Cut** (`Peaking`, `fc = 500 Hz`, `Q = 1.0`) : À `-9 dB` / `-12 dB`, atténuation ciblée à `500 Hz` avec impact minimal à `60 Hz` et `8 kHz`.
  - **High Shelf** (`fc = 10 kHz`, `S = 1.0`) : À `+6 dB` / `+12 dB`, amplification nette à `12 kHz` et inversement atténuation à `-6 dB`, avec impact minimal sur les graves (`100 Hz`).
- **Protection Clipper & Safety Clamp** : Même avec `High Shelf = +12.0 dB`, la sortie en mode `SoftClip` ou `HardClip` reste strictement bornée par `Ceiling`, et la sortie finale reste toujours finie et bornée dans `[-4.0, 4.0]`.
- **Sécurité Temps Réel** : Zéro allocation, zéro verrou, coefficients biquads mis en cache et recalculés uniquement sur changement de paramètre ou de `sample_rate`, états internes protégés contre `NaN`/`Inf` et dénormaux (`< 1e-20`).

## 3. Contrat UI (`egui`, 100 % ASCII)
- **Colonne gauche** :
  - `1. COMPRESSOR & ENVELOPE BALLISTICS` (inchangé)
  - `2. OUTPUT CLIPPER & PARALLEL MIX` (inchangé)
  - `3. DRUM BUS EQ` : groupe contenant uniquement les 3 potentiomètres rotatifs SSL (`"Low Cut"`, `"500Hz Cut"`, `"High Shelf"`) en couleur `SslKnobColor::Filter`, via `draw_ssl_knob_cell` (couronne 11 graduations, repère angulaire, glisser vertical, `Shift` fin, double-clic reset, lecture monospace ASCII, boutons `-` `+` `Reset`, et `ParamSetter` complet `begin/set/end`).
- **Colonne droite** :
  - Renumérotée `4. GAIN REDUCTION & STEREO BUS METERING`, avec `UI_METER_HEIGHT = 390.0` px.
- **Dimensions de fenêtre** : `820 x 620` px sans aucun chevauchement ni débordement de widget (`test_ui_layout_column_widths_fit_window`).
- **Encodage** : 100 % ASCII strict dans `src/lib.rs`.
