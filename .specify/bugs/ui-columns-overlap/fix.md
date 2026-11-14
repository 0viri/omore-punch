# Bug Fix: Chevauchement des colonnes UI et troncature des boutons Reset

- **Slug**: ui-columns-overlap
- **Fixed**: 2026-11-14
- **Assessment**: ./assessment.md
- **Status**: applied

## Summary

Remplacement du partage rigide 50/50 (`ui.columns(2, ...)`) par deux colonnes asymétriques (`480 px` à gauche, `310 px` à droite) dans `draw_omore_punch_ui`, et synchronisation de `ui.spacing_mut().slider_width` avec la largeur allouée aux sliders (`140 px`) dans `draw_param_row` afin d'éliminer les espaces vides parasites autour des rails et d'empêcher tout débordement des boutons `Reset` sur le panneau de metering.

## Changes

| File | Change | Notes |
|------|--------|-------|
| `src/lib.rs` | modified | Définition des constantes de géométrie UI (`UI_WINDOW_WIDTH`, `UI_WINDOW_HEIGHT`, `UI_LEFT_COL_WIDTH`, `UI_RIGHT_COL_WIDTH`, `UI_SLIDER_WIDTH`, etc.), correction de `draw_param_row`, mise en page asymétrique dans `draw_omore_punch_ui` et ajout du test unitaire `test_ui_layout_column_widths_fit_window`. |
| `CHANGELOG.md` | modified | Ajout d'une entrée sous `## [Unreleased] -> ### Corrigé` documentant le correctif `ui-columns-overlap`. |

## Diff Highlights (optional)

```rust
const UI_WINDOW_WIDTH: u32 = 820;
const UI_WINDOW_HEIGHT: u32 = 480;
const UI_LEFT_COL_WIDTH: f32 = 480.0;
const UI_RIGHT_COL_WIDTH: f32 = 310.0;
const UI_COL_GAP: f32 = 10.0;
const UI_SLIDER_WIDTH: f32 = 140.0;
const UI_STEP_BTN_WIDTH: f32 = 24.0;
const UI_RESET_BTN_WIDTH: f32 = 48.0;
const UI_ROW_HEIGHT: f32 = 22.0;
const UI_ROW_ITEM_SPACING: f32 = 6.0;
const UI_METER_HEIGHT: f32 = 255.0;
```

```rust
        let mut slider_val = current_val;
        ui.spacing_mut().slider_width = UI_SLIDER_WIDTH;
        let slider_resp = ui.add_sized(
            [UI_SLIDER_WIDTH, UI_ROW_HEIGHT],
            egui::Slider::new(&mut slider_val, min_val..=max_val).show_value(false),
        );
```

## Tests Added or Updated

- `src/lib.rs::tests::test_ui_layout_column_widths_fit_window` — vérifie que la somme des largeurs des deux colonnes, de l'espacement inter-colonnes et des marges du `CentralPanel` (`480 + 10 + 310 + 16 = 816 px`) tient dans `UI_WINDOW_WIDTH` (`820 px`), et que la largeur totale d'une ligne `draw_param_row` (`436 px` marges incluses) tient dans `UI_LEFT_COL_WIDTH` (`480 px`).

## Local Verification

- Commands run:
  - `LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs` → `0` caractère non-ASCII détecté.
  - `cargo test --release` → `6 passed; 0 failed` (incluant `test_ui_layout_column_widths_fit_window`).
  - `cargo run --package xtask --release -- bundle omore-punch --release` → génération des bundles `target/bundled/omore-punch.vst3` et `target/bundled/omore-punch.clap`.
- Manual checks: Possibilité d'installer les bundles localement via `./build.sh` pour vérifier visuellement le rendu dans Reaper.

## Deviations from Assessment

Aucune.

## Follow-ups

- Exécuter `/speckit-bug-test slug=ui-columns-overlap` (et optionnellement `./build.sh` pour tester visuellement dans Reaper).

