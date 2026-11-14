# Changelog — Omore Punch

Toutes les modifications notables de ce projet sont documentées dans ce fichier.

## [Unreleased]
### Modifié
- **Bouton Rapide `[Snare Safe 200Hz]` dans l'interface `egui` (`001-snare-safe-200hz`) :**
  - Remplacement du bouton rapide `[Kick Safe 90Hz]` par `[Snare Safe 200Hz]` réglant `SC HPF` sur `200.0 Hz` en un clic (technique Cristian Kohle) afin que ni le sous-grave de la grosse caisse ni le corps de la caisse claire ne fassent pomper le bus batterie.

### Corrigé
- **Mise en page UI `egui` (`ui-columns-overlap`) :**
  - Suppression du chevauchement entre la colonne de gauche (sections `1.` et `2.`) et la colonne de droite (`3. GAIN REDUCTION & STEREO BUS METERING`) qui tronquait les boutons `Reset`.
  - Remplacement du partage rigide 50/50 (`ui.columns(2, ...)`) par deux colonnes asymétriques (`480 px` à gauche, `310 px` à droite) adaptées au contenu.
  - Synchronisation de `ui.spacing_mut().slider_width` avec la largeur allouée au slider (`140 px`) dans `draw_param_row` pour éliminer les vides parasites entre `-`, le rail du slider et `+`, et ajustement de la hauteur de fenêtre (`820x480`) et des vu-mètres (`255 px`).

## [0.2.0] - 2026-10-16
### Ajouté
- **Filtre Passe-Haut de Sidechain Interne (`SC HPF`, `20.0 Hz` à `250.0 Hz`) :**
  - Filtre passe-haut stéréo du 1er ordre (`6 dB/oct`, sans résonance de phase) inséré exclusivement dans le chemin de détection avant le détecteur de crête stéréo lié.
  - Empêche l'énergie sub-grave de la grosse caisse (`40 Hz - 80 Hz`) d'écraser le compresseur et de faire pomper les cymbales, tout en préservant 100 % des graves sur le chemin audio principal.
  - Bypass intégral automatique lorsque `SC HPF <= 20.5 Hz` et précalcul mis en cache du coefficient `sc_hpf_coeff`.
- **Bouton Rapide `[Kick Safe 90Hz]` dans l'interface `egui` :**
  - Permet de régler instantanément `SC HPF` sur `90.0 Hz` en un clic à côté de `[Punch 3ms / 80ms]`.
- **Test Unitaire DSP (`test_sidechain_hpf_kick_immunity`) :**
  - Vérifie sur une onde sinusoïdale grave à `50 Hz` que le filtre Sidechain réduit fortement la réduction de gain indésirable et préserve l'amplitude sub-grave en sortie.

## [0.1.0] - 2026-10-16
### Ajouté
- Moteur DSP stéréo lié complet (`StereoPunchDsp`) reprenant la signature dynamique d'`Omore Sampler` :
  - `Input Trim` (`-12.0 dB` à `+12.0 dB`).
  - Détecteur de crête stéréo lié (`max(|L|, |R|)`).
  - Compresseur Feed-Forward en dB (`Threshold` de `-40.0 dB` à `0.0 dB`, `Ratio` de `1.0:1` à `20.0:1`).
  - Balistique `Attack` (`0.1 ms` à `100.0 ms`, défaut `3.0 ms`) et `Release` (`10.0 ms` à `500.0 ms`, défaut `80.0 ms`) avec précalcul des coefficients exponentiels et bouton rapide `[Punch 3ms / 80ms]`.
  - Compensation de gain `Auto Makeup` (35 %) débrayable + `Makeup Gain` manuel (`-12.0 dB` à `+24.0 dB`).
  - Étage Clipper final à 3 modes (`Soft Clip` cubique/tanh $C^1$, `Hard Clip`, `Bypass Clipper`) et `Clip Ceiling` (`-6.0 dB` à `0.0 dB`).
  - Mix parallèle interne `Mix Dry/Wet` (`0 %` à `100 %`).
  - Sanitisation complète anti-`NaN`/`Inf` et Safety Clamp `[-4.0, 4.0]`.
- Interface graphique `egui` sombre 100 % ASCII avec :
  - Versioning dynamique via `env!("CARGO_PKG_VERSION")`.
  - Grande jauge verticale de Gain Reduction (barre jaune descendante de `0.0 dB` vers `-24.0 dB` style Reaper) + indicateurs numériques `GR` et `Max GR`.
  - Vu-mètres stéréo `In L/R` et `Out L/R` avec témoin d'écrêtage `[CLIP]`.
- Suite de tests unitaires automatisés (`cargo test`) et pipeline CI GitHub Actions multi-OS (Linux x86_64 & Windows x86_64).