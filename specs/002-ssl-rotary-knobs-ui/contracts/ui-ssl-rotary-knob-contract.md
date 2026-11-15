# UI & Host Parameter Contract: Potentiomètres rotatifs type SSL en grille

**Feature**: `002-ssl-rotary-knobs-ui`
**Date**: 2026-11-14

## 1. Contrat Visuel (`egui`)
- **Structure d'une cellule (`draw_ssl_knob_cell`)** :
  1. **Étiquette supérieure** : Nom du paramètre en `monospace().strong()`, centré dans la cellule (`110.0` px de large).
  2. **Potentiomètre rotatif (`44.0 x 44.0` px)** :
     - Couronne de `11` graduations radiales réparties sur `270°` (de `-135°` à `+135°` par rapport à midi) ; les graduations situées jusqu'à l'angle courant sont illuminées en ambre/clair, les suivantes en gris discret.
     - Jupe sombre (`circle_filled` / `circle_stroke`) et capuchon central coloré selon la famille `SslKnobColor` (avec léger éclaircissement au survol ou pendant le glisser).
     - Repère angulaire (`line_segment`) net du centre vers le bord du capuchon indiquant l'angle correspondant à `param.modulated_normalized_value()`.
  3. **Lecture numérique** : `param.to_string()` en police `monospace()` couleur ambre/or `rgb(245, 185, 45)`, centrée sous le bouton.
  4. **Rangée d'actions compactes** : Bouton `-` (`22x16` px), bouton `+` (`22x16` px) et bouton `Reset` (`46x16` px).
- **Disposition en grille** :
  - **Section 1 (`1. COMPRESSOR & ENVELOPE BALLISTICS`)** :
    - Rangée 1 (4 colonnes) : `Input Drive`, `Threshold`, `Ratio`, `Attack`
    - Rangée 2 (3 colonnes) : `Release`, `SC HPF`, `Makeup Gain`
    - Rangée d'actions rapides : `[Punch 3ms / 80ms]`, `[Snare Safe 200Hz]`, `[Auto Makeup: ON/OFF]`
  - **Section 2 (`2. OUTPUT CLIPPER & PARALLEL MIX`)** :
    - Rangée mode clipper : `Clipper Mode: [Soft Clip] [Hard Clip] [Bypass]`
    - Rangée 1 (2 colonnes) : `Clip Ceiling`, `Mix Dry/Wet`
- **Contrainte d'encodage** : 100 % ASCII strict (`0x20..=0x7E`).

## 2. Contrat d'Interaction Hôte VST3 / CLAP (`ParamSetter`)
- Toute action modifiant un paramètre (`drag`, `double_click`, clic `-`, clic `+`, clic `Reset`) DOIT obligatoirement appeler dans l'ordre :
  1. `setter.begin_set_parameter(param)`
  2. `setter.set_parameter(param, new_plain_value)`
  3. `setter.end_set_parameter(param)`
- Pour un glisser continu (`drag_started` / `dragged` / `drag_stopped`), `begin_set_parameter` est appelé à `drag_started()`, `set_parameter` à chaque frame où `drag_delta().y != 0.0`, et `end_set_parameter` à `drag_stopped()`.

## 3. Contrat de Rétrocompatibilité
- Aucun identifiant `#[id = "..."]`, aucune plage `FloatRange` et aucune valeur par défaut de `OmorePunchParams` ne sont modifiés.
- Les sessions Reaper existantes (VST3 et CLAP) chargent et restituent exactement les mêmes valeurs et automations.
