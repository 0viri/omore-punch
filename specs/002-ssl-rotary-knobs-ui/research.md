# Phase 0 Research: Interface type SSL avec potentiomètres rotatifs en grille

**Feature**: `002-ssl-rotary-knobs-ui`
**Date**: 2026-11-14

## Decisions & Findings

### 1. Rendu vectoriel des potentiomètres rotatifs via `egui::Painter` dans `src/lib.rs`
- **Decision**: Remplacer `draw_param_row` par une fonction de rendu de cellule de potentiomètre rotatif (`draw_ssl_knob_cell`) dans le bloc 4 (`draw_omore_punch_ui`) de `src/lib.rs`, utilisant exclusivement les primitives vectorielles 2D d'`egui 0.31.1` (`Painter::circle_filled`, `circle_stroke`, `line_segment`) sans image externe ni nouvelle dépendance Cargo.
- **Rationale**: Respecte le Principe I (fichier monolithique `src/lib.rs`) et le Principe II (100 % ASCII). `src/lib.rs` utilise déjà cette approche avec succès pour `draw_vertical_level_meter` et `draw_reaper_style_gr_meter`.
- **Alternatives considered**:
  - *Ajouter une crate tierce de widgets rotatifs (`egui_knob`, etc.)* : Rejeté pour éviter d'alourdir `Cargo.toml`, garantir la compatibilité exacte avec la version `egui` embarquée par `nih_plug_egui` et maîtriser le style visuel SSL.
  - *Utiliser des textures/sprites PNG* : Rejeté car hors périmètre (Non-Goals) et incompatible avec la simplicité du binaire unique sans assets externes.

### 2. Course angulaire (270°) et correspondance avec `FloatRange` normalisé
- **Decision**:
  - Utiliser une course angulaire de `270°` (`-135°` à `+135°` par rapport à la verticale haute, soit de 7h à 5h), entourée d'une couronne de `11` graduations radiales (`0 %` à `100 %`).
  - Piloter l'angle d'affichage et le glisser vertical de la souris dans l'espace normalisé `[0.0, 1.0]` du paramètre via `param.modulated_normalized_value()` et `param.preview_plain(next_norm)`.
- **Rationale**: Quatre paramètres clés (`ratio`, `attack_ms`, `release_ms`, `sc_hpf_hz`) utilisent `FloatRange::Skewed`. Travailler en valeur normalisée `[0.0, 1.0]` permet aux valeurs musicales nominales (`Attack = 3.0 ms`, `Release = 80.0 ms`, `Ratio = 4.0:1`) de se positionner naturellement vers le milieu de la course du bouton (10h–12h) comme sur un compresseur matériel SSL, et assure une sensibilité de glisser souris uniforme sur toute la course.
- **Alternatives considered**:
  - *Interpolation linéaire directe entre `min_val` et `max_val` pour l'angle* : Rejeté car sur `Attack` (`0.1..100.0 ms`), `3.0 ms` n'occuperait que 3 % de la course angulaire, rendant la position illisible et le réglage des courtes attaques imprécis.

### 3. Codage couleur des capuchons par famille fonctionnelle (Style SSL sur fond sombre)
- **Decision**: Conserver le thème sombre `apply_omore_dark_theme` (`panel_fill = rgb(18, 19, 24)`) et attribuer à chaque famille fonctionnelle une couleur de capuchon (`SslKnobColor`) inspirée des tranches et compresseurs de bus SSL :
  - **Gain / Drive** (`Input Drive`, `Makeup Gain`) : Ambre / Or chaud `rgb(215, 145, 30)`
  - **Dynamique** (`Threshold`, `Ratio`) : Gris clair / Argent SSL `rgb(150, 158, 170)`
  - **Enveloppe** (`Attack`, `Release`) : Vert SSL `rgb(55, 155, 95)`
  - **Filtre Sidechain** (`SC HPF`) : Bleu SSL `rgb(55, 125, 195)`
  - **Clipper & Mix** (`Clip Ceiling`, `Mix Dry/Wet`) : Rouge / Corail SSL `rgb(195, 65, 60)`
- **Rationale**: Offre un repérage visuel immédiat des groupes fonctionnels sur le fond sombre sans compromettre le contraste du repère angulaire blanc/crème ni la lisibilité des textes monospace ASCII.
- **Alternatives considered**:
  - *Façade gris clair/anthracite intégrale* : Écartée lors de la clarification utilisateur au profit du fond sombre `Omore` combiné aux capuchons colorés SSL.

### 4. Interactions : Glisser vertical, `Shift` fin, Double-clic Reset et boutons compacts `[-]` `[+]` `[Reset]`
- **Decision**:
  - Glisser vertical (`Sense::click_and_drag()`) : monter augmente la valeur normalisée (`-drag_delta.y / sensitivity`), descendre la diminue. Sensibilité nominale : `150.0` px pour la course complète (`600.0` px lorsque `Shift` est maintenu, soit 4x plus précis).
  - Double-clic sur le bouton rond (`resp.double_clicked()`) : réinitialise le paramètre à `param.default_plain_value()`.
  - Sous la lecture numérique de chaque cellule, conserver trois petits boutons compacts `[-]`, `[+]` (appliquant le `step` en unités réelles borné à `[min_val, max_val]`) et `[Reset]` (appliquant `default_plain_value()`).
  - Encadrer chaque geste par `setter.begin_set_parameter(param)`, `setter.set_parameter(param, ...)` et `setter.end_set_parameter(param)`.
- **Rationale**: Combine l'ergonomie rapide d'un bouton rotatif de DAW et la précision déterministe au pas-à-pas déjà présente dans `Omore Punch`.

### 5. Budget géométrique dans la fenêtre `820x480` px
- **Decision**:
  - Fixer les dimensions d'une cellule de potentiomètre à `UI_KNOB_CELL_WIDTH = 110.0` px et `UI_KNOB_CELL_HEIGHT = 94.0` px (diamètre du bouton `UI_KNOB_DIAMETER = 44.0` px, espacement horizontal inter-cellules `UI_KNOB_GRID_SPACING_X = 8.0` px, espacement vertical `UI_KNOB_GRID_SPACING_Y = 4.0` px).
  - Section 1 (`1. COMPRESSOR & ENVELOPE BALLISTICS`) : Grille de 7 boutons sur 2 rangées (Rangée 1 : `Input Drive`, `Threshold`, `Ratio`, `Attack` ; Rangée 2 : `Release`, `SC HPF`, `Makeup Gain`), suivie de la rangée des 3 boutons d'action rapide (`[Punch 3ms / 80ms]`, `[Snare Safe 200Hz]`, `[Auto Makeup: ON/OFF]`).
  - Section 2 (`2. OUTPUT CLIPPER & PARALLEL MIX`) : Rangée du sélecteur `Clipper Mode`, suivie d'une rangée de 2 boutons (`Clip Ceiling`, `Mix Dry/Wet`).
- **Rationale**:
  - **Largeur** : 4 cellules de `110.0` px + 3 espacements de `8.0` px + marges de groupe (`12.0` px) = `476.0` px `<= UI_LEFT_COL_WIDTH` (`480.0` px).
  - **Hauteur** : Section 1 (~`254.0` px) + espacement (`6.0` px) + Section 2 (~`156.0` px) = `416.0` px `<= 432.0` px disponibles dans `CentralPanel` (sur `UI_WINDOW_HEIGHT = 480` px).
  - Garantit zéro débordement horizontal et vertical dans les sessions Reaper existantes.
