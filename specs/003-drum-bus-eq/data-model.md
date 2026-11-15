# Phase 1 Data Model: Section EQ 3 bandes pour Bus Batterie (`3. DRUM BUS EQ`)

**Feature**: `003-drum-bus-eq`
**Date**: 2026-11-14

## Entities & Structures (`src/lib.rs`)

### 1. Additions à `DspConfig` (Bloc 1 — Moteur DSP pur)
| Champ | Type | Défaut | Plage valide | Rôle |
|-------|------|--------|--------------|------|
| `eq_low_cut_hz` | `f32` | `20.0` | `20.0..=160.0` | Fréquence de coupure du passe-haut 12 dB/oct (`<= 20.5 Hz` = bypass strict) |
| `eq_mid_cut_db` | `f32` | `0.0` | `-12.0..=0.0` | Atténuation peaking à `500.0 Hz` ($Q = 1.0$, `abs < 1e-3` = bypass strict) |
| `eq_high_shelf_db` | `f32` | `0.0` | `-6.0..=12.0` | Gain High Shelf 2e ordre à `10000.0 Hz` ($S = 1.0$, `abs < 1e-3` = bypass strict) |

### 2. Structures internes Biquad (`BiquadCoeffs` et `StereoBiquad`, Bloc 1)
Représentation Direct Form II Transposed sans allocation sur le tas :
- **`BiquadCoeffs`** (`#[derive(Clone, Copy, Debug)]`) :
  - `b0: f32`, `b1: f32`, `b2: f32` (numérateur normalisé par `a0`)
  - `a1: f32`, `a2: f32` (dénominateur normalisé par `a0`)
  - `bypass: bool` (`true` lorsque le filtre est neutre)
- **`StereoBiquad`** (`#[derive(Clone, Debug, Default)]`) :
  - `coeffs: BiquadCoeffs`
  - `z1_l: f32`, `z2_l: f32` (états canal gauche)
  - `z1_r: f32`, `z2_r: f32` (états canal droit)
  - Invariant de sanitisation : après chaque échantillon, chaque état `z` vérifie `if z.is_finite() && z.abs() > 1.0e-20 { z } else { 0.0 }` ; lorsque `coeffs.bypass == true`, les 4 états sont remis à `0.0` et le signal traverse sans modification.

### 3. Additions à `StereoPunchDsp` (Bloc 1)
- Filtres :
  - `eq_low_cut: StereoBiquad`
  - `eq_mid_cut: StereoBiquad`
  - `eq_high_shelf: StereoBiquad`
- Cache de coefficients (Principe III — recalcul uniquement sur changement) :
  - `cached_eq_low_cut_hz: f32` (initialisé à `-1.0` pour forcer l'init)
  - `cached_eq_mid_cut_db: f32` (initialisé à `f32::NAN` ou sentinelle `-999.0`)
  - `cached_eq_high_shelf_db: f32` (initialisé à `-999.0`)
- Méthodes :
  - `update_eq(&mut self, sample_rate: f32, low_cut_hz: f32, mid_cut_db: f32, high_shelf_db: f32)`
  - Intégration dans `set_sample_rate()` (invalidation des caches EQ) et `reset()` (remise à zéro des états `eq_low_cut`, `eq_mid_cut`, `eq_high_shelf`).

### 4. Additions à `OmorePunchParams` (Bloc 3 — Paramètres `nih-plug`)
| Champ Rust | `#[id = "..."]` | Nom DAW | Défaut | `FloatRange` | Lissage | Unité & Formatage |
|------------|-----------------|---------|--------|--------------|---------|-------------------|
| `eq_low_cut_hz` | `"eq_low_cut_hz"` | `"EQ Low Cut"` | `20.0` | `Skewed { min: 20.0, max: 160.0, factor: skew_factor(-1.0) }` | `Linear(5.0)` | `" Hz"`, `v2s_f32_rounded(1)` |
| `eq_mid_cut_db` | `"eq_mid_cut_db"` | `"EQ 500Hz Cut"` | `0.0` | `Linear { min: -12.0, max: 0.0 }` | `Linear(5.0)` | `" dB"`, `v2s_f32_rounded(2)` |
| `eq_high_shelf_db` | `"eq_high_shelf_db"` | `"EQ High Shelf"` | `0.0` | `Linear { min: -6.0, max: 12.0 }` | `Linear(5.0)` | `" dB"`, `v2s_f32_rounded(2)` |

### 5. Modèle UI de la section `3. DRUM BUS EQ` (Bloc 4 — `draw_omore_punch_ui`)
| Section UI | Rangée / Colonne | Label UI | Champ `OmorePunchParams` | Pas (`-`/`+`) | Plage `[min, max]` | Famille `SslKnobColor` |
|------------|------------------|----------|--------------------------|---------------|--------------------|------------------------|
| `3. DRUM BUS EQ` | R1, C1 | `"Low Cut"` | `eq_low_cut_hz` | `5.0` | `20.0..=160.0` | `SslKnobColor::Filter` |
| `3. DRUM BUS EQ` | R1, C2 | `"500Hz Cut"` | `eq_mid_cut_db` | `0.5` | `-12.0..=0.0` | `SslKnobColor::Filter` |
| `3. DRUM BUS EQ` | R1, C3 | `"High Shelf"` | `eq_high_shelf_db` | `0.5` | `-6.0..=12.0` | `SslKnobColor::Filter` |

En-tête colonne droite renuméroté :
- `"4. GAIN REDUCTION & STEREO BUS METERING"`

### 6. Constantes géométriques mises à jour (`src/lib.rs`)
| Constante | Ancienne valeur | Nouvelle valeur | Invariant vérifié |
|-----------|-----------------|-----------------|-------------------|
| `UI_WINDOW_WIDTH` | `820` | `820` (inchangé) | `480 + 10 + 310 + 16 = 816 <= 820` |
| `UI_WINDOW_HEIGHT` | `480` | `620` | `254 + 6 + 156 + 6 + 130 = 552 <= 572` (`620 - 48`) |
| `UI_METER_HEIGHT` | `255.0` | `390.0` | Hauteur des vu-mètres adaptée à la fenêtre `620` px sans débordement |

## State Transitions (Signal & Paramètres)

1. **Flux audio `StereoPunchDsp::process_sample`** :
   - `0.` Sanitisation des entrées (`dry_l`, `dry_r`)
   - `1.` `Input Drive` -> `1b.` `SC HPF` -> `2..5.` Détection stéréo liée, balistique et `comp_l / comp_r`
   - `5b.` **Drum Bus EQ** : `comp -> eq_low_cut -> eq_mid_cut -> eq_high_shelf -> eq_out`
   - `6.` `Final Clipper Stage` sur `(eq_out_l, eq_out_r)` -> `(wet_l, wet_r, clipped)`
   - `7.` `Parallel Mix` entre `(dry_l, dry_r)` et `(wet_l, wet_r)` -> `sanitize_sample()`
2. **Transition Bypass -> Actif sur chaque bande EQ** :
   - Dès qu'un paramètre quitte sa zone neutre (`eq_low_cut_hz > 20.5`, `eq_mid_cut_db <= -1e-3`, `|eq_high_shelf_db| >= 1e-3`), `update_eq()` recalcule les coefficients biquads correspondants et désactive le flag `bypass`.
   - Lorsqu'un paramètre revient dans sa zone neutre (ou via double-clic / `Reset`), `bypass` repasse à `true` et les états `z1_l, z2_l, z1_r, z2_r` du filtre concerné sont remis à `0.0`.
