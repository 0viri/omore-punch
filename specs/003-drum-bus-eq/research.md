# Phase 0 Research: Section EQ 3 bandes pour Bus Batterie (`3. DRUM BUS EQ`)

**Feature**: `003-drum-bus-eq`
**Date**: 2026-11-14

## Decisions & Findings

### 1. Topologie DSP des 3 filtres EQ (Biquads Audio EQ Cookbook en `f32`, Direct Form II Transposed)
- **Decision**: Implémenter la section EQ dans `StereoPunchDsp` (bloc 1 de `src/lib.rs`) sous forme de trois filtres biquads stéréo en série utilisant les formules de Robert Bristow-Johnson (Audio EQ Cookbook) en simple précision `f32` et structure Direct Form II Transposed (`StereoBiquad`) :
  1. **Low Cut (`eq_low_cut_hz`)** : Passe-haut biquad Butterworth 2e ordre (pente `12 dB/oct`, $Q = 1/\sqrt{2} \approx 0.70710678$), fréquence de coupure de `20.0 Hz` à `160.0 Hz` (défaut `20.0 Hz`). Bypass strict (sortie = entrée et états `z1_l, z2_l, z1_r, z2_r` remis à `0.0`) lorsque `eq_low_cut_hz <= 20.5 Hz` (même convention que `sc_hpf_hz`).
  2. **500 Hz Cut (`eq_mid_cut_db`)** : Filtre Peaking EQ biquad centré à fréquence fixe `500.0 Hz` avec largeur musicale $Q = 1.0$, gain exclusivement atténuateur de `-12.0 dB` à `0.0 dB` (défaut `0.0 dB`). Bypass strict et états remis à `0.0` lorsque `eq_mid_cut_db.abs() < 1.0e-3`.
  3. **High Shelf (`eq_high_shelf_db`)** : Filtre High-Shelf biquad 2e ordre à fréquence charnière fixe `10000.0 Hz` (bornée à `0.45 * sample_rate` par sécurité aux basses fréquences d'échantillonnage) et pente $S = 1.0$ ($Q = 1/\sqrt{2}$), gain de `-6.0 dB` à `+12.0 dB` (défaut `0.0 dB`). Bypass strict et états remis à `0.0` lorsque `eq_high_shelf_db.abs() < 1.0e-3`.
- **Rationale**:
  - La topologie Direct Form II Transposed offre une excellente tenue numérique en `f32` sur les filtres graves (comme un Low Cut à `20–160 Hz` sous `48/96 kHz`), sans distorsion de quantification excessive des pôles.
  - Les trois filtres en série couvrent exactement les 3 actions demandées sans ajouter de contrôles superflus (pas de Q ni de fréquences secondaires pour 500 Hz / High Shelf).
  - Le bypass strict aux positions neutres garantit une transparence bit-à-bit (`SC-001`) et zéro consommation CPU inutile lorsque l'EQ n'est pas activé.
- **Alternatives considered**:
  - *Filtres IIR 1er ordre (`6 dB/oct`) pour le Low Cut* : Rejeté car une pente de `6 dB/oct` est trop douce pour nettoyer efficacement le sous-grave sur un bus batterie comparé à un Butterworth `12 dB/oct`.
  - *Crates DSP externes (`biquad`, `fundsp`, etc.)* : Rejeté pour respecter l'architecture monolithique sans dépendance supplémentaire dans `Cargo.toml` et garder le contrôle total sur la sanitisation temps réel anti-dénormaux.

### 2. Placement dans la chaîne audio (`StereoPunchDsp::process_sample`)
- **Decision**: Insérer la section EQ à l'étape `5b` dans `StereoPunchDsp::process_sample()`, juste après la compensation de gain du compresseur (`comp_l`, `comp_r`) et immédiatement avant l'étape `6. Final Clipper Stage` (`Compressor -> Drum Bus EQ -> Output Clipper -> Parallel Mix`).
- **Rationale**:
  - L'EQ façonne le signal du bus batterie compressé sans perturber la détection du compresseur (qui dispose déjà de son propre `SC HPF`).
  - Tout pic de crête ajouté par le boost du `High Shelf` (jusqu'à `+12 dB`) ou par les déphasages du `Low Cut` passe ensuite dans le Clipper, qui garantit le respect strict du plafond (`Ceiling`, testé par `test_clipper_strict_ceiling`).
  - Le chemin `Dry` du mix parallèle (`0% Mix`) reste bit-à-bit identique à l'entrée (`test_parallel_mix_dry_wet` préservé sans modification).
- **Alternatives considered**:
  - *Placer l'EQ après le Clipper et le Mix parallèle* : Rejeté car un boost `+12 dB` sur le `High Shelf` ferait dépasser le plafond strict du clipper et briserait la garantie de mix parallèle à `0%`.
  - *Placer l'EQ en amont du compresseur* : Rejeté car cela modifierait le déclenchement du compresseur en doublon avec le `SC HPF`.

### 3. Sécurité Temps Réel (Principe III) et Sanitisation Anti-Dénormaux / NaN (Principe IV)
- **Decision**:
  - Aucune allocation sur le tas ni aucun verrou dans `process_sample()` et dans les structures `StereoBiquad`.
  - Mettre en cache les dernières valeurs calculées dans `StereoPunchDsp` (`cached_eq_low_cut_hz`, `cached_eq_mid_cut_db`, `cached_eq_high_shelf_db`) afin que `update_eq()` ne réévalue les fonctions coûteuses (`.sin()`, `.cos()`, `.sqrt()`, `10.0_f32.powf()`) que lorsque `sample_rate` change ou qu'un paramètre EQ varie d'au moins `1.0e-4`.
  - Sanitiser chaque variable d'état interne (`z1_l`, `z2_l`, `z1_r`, `z2_r`) à chaque échantillon (`!s.is_finite() || s.abs() < 1.0e-20 => 0.0`) et réinitialiser les états dans `StereoPunchDsp::reset()`.
- **Rationale**: Conforme aux Principes III et IV de la Constitution ; évite les pics CPU liés aux dénormaux sur x86_64 lors des passages silencieux et garantit qu'aucun `NaN`/`Inf` ne se propage dans les boucles de rétroaction IIR.
- **Alternatives considered**:
  - *Recalculer les coefficients biquads à chaque échantillon lissé* : Rejeté car `nih-plug` lisse les paramètres à chaque échantillon ; sans mise en cache ou seuil, `.sin()`/`.cos()` seraient appelés à chaque échantillon (violation du Principe III lorsque les paramètres sont stables). Note : le seuil `> 1.0e-4` assure l'absence d'évaluation quand le lissage est terminé, et une transition douce pendant les rampes de `5 ms`.

### 4. Intégration UI (`3. DRUM BUS EQ`) et Potentiomètres Rotatifs SSL
- **Decision**:
  - Ajouter une nouvelle section `3. DRUM BUS EQ` dans la colonne de gauche de `draw_omore_punch_ui`, juste sous `2. OUTPUT CLIPPER & PARALLEL MIX`, en réutilisant `draw_ssl_knob_cell` pour une rangée unique de 3 potentiomètres rotatifs (`"Low Cut"`, `"500Hz Cut"`, `"High Shelf"`).
  - Utiliser la famille de couleur `SslKnobColor::Filter` (bleu SSL `rgb(55, 125, 195)`) pour les 3 potentiomètres de la section EQ afin d'unifier visuellement le pôle filtrage/égalisation.
  - Renommer l'en-tête de la colonne de droite en `"4. GAIN REDUCTION & STEREO BUS METERING"`.
- **Rationale**: Respecte intégralement les standards GUI de la Constitution v1.1.0 (potentiomètres ronds SSL en grille, 100 % ASCII, police monospace, interactions glisser vertical / `Shift` fin / double-clic reset / `-` `+` `Reset`).
- **Alternatives considered**:
  - *Attribuer 3 couleurs SSL différentes aux 3 potards de l'EQ* : Rejeté pour garder la cohérence sémantique des familles `SslKnobColor` (où `Filter` identifie les filtres/EQ, `GainDrive` les gains/drives et `ClipperMix` la sortie).

### 5. Budget géométrique de la fenêtre d'éditeur (`820x620` px)
- **Decision**:
  - Augmenter `UI_WINDOW_HEIGHT` de `480` à `620` px tout en conservant `UI_WINDOW_WIDTH = 820` px et toutes les dimensions de cellules (`UI_KNOB_CELL_WIDTH = 110.0`, `UI_KNOB_CELL_HEIGHT = 94.0`, `UI_KNOB_DIAMETER = 44.0`).
  - Ajuster `UI_METER_HEIGHT` de `255.0` à `390.0` px dans la colonne de droite pour équilibrer la hauteur des vu-mètres stéréo et du barregraphe `GR` avec les 3 sections de la colonne de gauche.
- **Rationale**:
  - La colonne de gauche actuelle occupe `416.0` px (`Section 1 = 254.0 px` + `gap = 6.0 px` + `Section 2 = 156.0 px`). L'ajout de `Section 3` (`36.0 px` d'en-tête/marges + `94.0 px` de rangée de potards = `130.0 px` + `6.0 px` d'espacement = `+136.0 px`) porte la hauteur requise à `552.0` px.
  - Avec `UI_WINDOW_HEIGHT = 620`, la hauteur disponible dans `CentralPanel` passe à `620.0 - 48.0 = 572.0` px (`552.0 <= 572.0`, soit `20.0` px de marge).
  - La fenêtre `820x620` reste compacte dans Reaper tout en garantissant l'absence totale de défilement et de chevauchement, vérifiable par `test_ui_layout_column_widths_fit_window`.
- **Alternatives considered**:
  - *Réduire la taille des potentiomètres pour tout faire tenir dans `820x480` px* : Rejeté car cela casserait la lisibilité des graduations, labels et boutons compacts `-`/`+`/`Reset` définis dans la feature `002-ssl-rotary-knobs-ui`.
