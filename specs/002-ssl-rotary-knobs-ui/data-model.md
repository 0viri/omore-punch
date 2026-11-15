# Phase 1 Data Model: Interface type SSL avec potentiomètres rotatifs en grille

**Feature**: `002-ssl-rotary-knobs-ui`
**Date**: 2026-11-14

## Entities & UI Structures

Cette fonctionnalité ne modifie aucune structure du moteur DSP (`StereoPunchDsp`, `DspConfig`), ni la télémétrie atomique (`MeterTelemetry`), ni les identifiants de paramètres (`OmorePunchParams`). Elle introduit uniquement des descripteurs visuels sans état sur la pile dans le bloc 4 (`draw_omore_punch_ui`) de `src/lib.rs`.

### 1. Famille de couleur de capuchon : `SslKnobColor` (interne UI)
| Variante | Couleur RGB du capuchon | Paramètres associés |
|----------|-------------------------|---------------------|
| `GainDrive` | `Color32::from_rgb(215, 145, 30)` (Ambre/Or) | `Input Drive`, `Makeup Gain` |
| `Dynamics` | `Color32::from_rgb(150, 158, 170)` (Gris/Argent SSL) | `Threshold`, `Ratio` |
| `Envelope` | `Color32::from_rgb(55, 155, 95)` (Vert SSL) | `Attack`, `Release` |
| `Filter` | `Color32::from_rgb(55, 125, 195)` (Bleu SSL) | `SC HPF` |
| `ClipperMix` | `Color32::from_rgb(195, 65, 60)` (Rouge/Corail SSL) | `Clip Ceiling`, `Mix Dry/Wet` |

### 2. Correspondance des 9 paramètres continus (`OmorePunchParams`)
| Section | Rangée / Colonne | Label UI | Champ `OmorePunchParams` (`#[id]`) | Plage & Loi | Pas (`-`/`+`) | Défaut (`Reset`) | Famille `SslKnobColor` |
|---------|------------------|----------|------------------------------------|-------------|---------------|------------------|------------------------|
| Section 1 | R1, C1 | `"Input Drive"` | `input_db` (`"input_db"`) | `-12.0..=12.0 dB` (Linear) | `0.5` | `0.0 dB` | `GainDrive` |
| Section 1 | R1, C2 | `"Threshold"` | `threshold_db` (`"threshold_db"`) | `-40.0..=0.0 dB` (Linear) | `0.5` | `0.0 dB` | `Dynamics` |
| Section 1 | R1, C3 | `"Ratio"` | `ratio` (`"ratio"`) | `1.0..=20.0 :1` (Skewed `-1.5`) | `0.25` | `4.0 :1` | `Dynamics` |
| Section 1 | R1, C4 | `"Attack"` | `attack_ms` (`"attack_ms"`) | `0.1..=100.0 ms` (Skewed `-1.5`) | `0.5` | `3.0 ms` | `Envelope` |
| Section 1 | R2, C1 | `"Release"` | `release_ms` (`"release_ms"`) | `10.0..=500.0 ms` (Skewed `-1.2`) | `5.0` | `80.0 ms` | `Envelope` |
| Section 1 | R2, C2 | `"SC HPF"` | `sc_hpf_hz` (`"sc_hpf_hz"`) | `20.0..=250.0 Hz` (Skewed `-1.0`) | `5.0` | `20.0 Hz` | `Filter` |
| Section 1 | R2, C3 | `"Makeup Gain"` | `makeup_db` (`"makeup_db"`) | `-12.0..=24.0 dB` (Linear) | `0.5` | `0.0 dB` | `GainDrive` |
| Section 2 | R1, C1 | `"Clip Ceiling"` | `ceiling_db` (`"ceiling_db"`) | `-6.0..=0.0 dB` (Linear) | `0.25` | `0.0 dB` | `ClipperMix` |
| Section 2 | R1, C2 | `"Mix Dry/Wet"` | `mix_pct` (`"mix_pct"`) | `0.0..=100.0 %` (Linear) | `5.0` | `100.0 %` | `ClipperMix` |

### 3. Constantes géométriques de mise en page (`src/lib.rs`)
| Constante | Valeur | Rôle et invariant vérifié par test unitaire |
|-----------|--------|---------------------------------------------|
| `UI_WINDOW_WIDTH` | `820` (`u32`) | Largeur totale de la fenêtre `EguiState` |
| `UI_WINDOW_HEIGHT` | `480` (`u32`) | Hauteur totale de la fenêtre `EguiState` |
| `UI_LEFT_COL_WIDTH` | `480.0` (`f32`) | Largeur allouée à la colonne des contrôles |
| `UI_RIGHT_COL_WIDTH` | `310.0` (`f32`) | Largeur allouée à la colonne de metering |
| `UI_COL_GAP` | `10.0` (`f32`) | Espacement entre les deux colonnes |
| `UI_KNOB_COLS_MAX` | `4` (`usize`) | Nombre maximal de boutons ronds par rangée |
| `UI_KNOB_CELL_WIDTH` | `110.0` (`f32`) | Largeur d'une cellule de potentiomètre (`4 * 110 + 3 * 8 + 12 = 476 <= 480`) |
| `UI_KNOB_CELL_HEIGHT` | `94.0` (`f32`) | Hauteur d'une cellule de potentiomètre |
| `UI_KNOB_DIAMETER` | `44.0` (`f32`) | Diamètre de la zone du potentiomètre rotatif |
| `UI_KNOB_GRID_SPACING_X` | `8.0` (`f32`) | Espacement horizontal entre deux cellules de potentiomètre |
| `UI_KNOB_GRID_SPACING_Y` | `4.0` (`f32`) | Espacement vertical entre deux rangées de potentiomètres |
| `UI_KNOB_STEP_BTN_WIDTH` | `22.0` (`f32`) | Largeur des petits boutons `-` et `+` sous chaque bouton rond |
| `UI_KNOB_RESET_BTN_WIDTH` | `46.0` (`f32`) | Largeur du petit bouton `Reset` sous chaque bouton rond |

## State Transitions (Interactions Potentiomètre)

1. **Glisser vertical souris (`drag_started` -> `dragged` -> `drag_stopped`)** :
   - Début : `setter.begin_set_parameter(param)`
   - En cours : `delta_norm = -resp.drag_delta().y / sensitivity` (avec `sensitivity = 600.0` si `Shift` actif, sinon `150.0`), `next_norm = (param.modulated_normalized_value() + delta_norm).clamp(0.0, 1.0)`, `next_plain = param.preview_plain(next_norm).clamp(min_val, max_val)`, puis `setter.set_parameter(param, next_plain)`
   - Fin : `setter.end_set_parameter(param)`
2. **Double-clic sur le potentiomètre OU clic sur `[Reset]`** :
   - `setter.begin_set_parameter(param)`
   - `setter.set_parameter(param, param.default_plain_value())`
   - `setter.end_set_parameter(param)`
3. **Clic sur `[-]` ou `[+]`** :
   - `next_val = (param.value() ± step).clamp(min_val, max_val)`
   - `setter.begin_set_parameter(param)`
   - `setter.set_parameter(param, next_val)`
   - `setter.end_set_parameter(param)`
