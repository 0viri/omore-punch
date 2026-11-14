# Bug Verification: Chevauchement des colonnes UI et troncature des boutons Reset

- **Slug**: ui-columns-overlap
- **Tested**: 2026-11-14
- **Assessment**: ./assessment.md
- **Fix**: ./fix.md
- **Result**: verified

## Summary

Tous les contrôles automatisés (test unitaire géométrique `test_ui_layout_column_widths_fit_window`, suite de non-régression DSP en mode Release, contrôle 100 % ASCII et génération des bundles VST3/CLAP) ainsi que la vérification visuelle dans Reaper confirment que le chevauchement des colonnes et la troncature des boutons `Reset` ont disparu.

## Checks Performed

| Check | Command / Action | Result | Notes |
|-------|------------------|--------|-------|
| Reproduction (post-fix) | Ouverture visuelle de l'éditeur dans Reaper (`820x480`) | pass | Confirmé visuellement par l'utilisateur : plus aucun chevauchement entre les colonnes ni troncature des boutons `Reset`. |
| New / updated tests | `cargo test --release test_ui_layout_column_widths_fit_window` | pass | Vérifie que la largeur cumulée des deux colonnes (`816 px`) rentre dans `820 px` et qu'une ligne `draw_param_row` (`436 px`) rentre dans la colonne de gauche (`480 px`). |
| Regression suite | `cargo test --release` | pass | `6 passed; 0 failed` (les 5 tests DSP + le test de géométrie UI). |
| Lint / type-check | `LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs` & `cargo run --package xtask --release -- bundle omore-punch --release` | pass | `0` caractère non-ASCII dans `src/lib.rs` ; bundles `omore-punch.vst3` et `omore-punch.clap` compilés avec succès. |

## Output Excerpts

```text
running 1 test
test tests::test_ui_layout_column_widths_fit_window ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out; finished in 0.00s
```

```text
running 6 tests
test tests::test_clipper_strict_ceiling ... ok
test tests::test_nan_inf_and_safety_clamp ... ok
test tests::test_parallel_mix_dry_wet ... ok
test tests::test_compressor_stereo_linked_reduction ... ok
test tests::test_ui_layout_column_widths_fit_window ... ok
test tests::test_sidechain_hpf_kick_immunity ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
ASCII_CLEAN
```

## Residual Risks

- Validation visuelle finale dans Reaper sous Linux (Debian 13) et Windows 11 : bien que les largeurs et espacements `egui` soient désormais bornés (`UI_LEFT_COL_WIDTH = 480.0`, `UI_RIGHT_COL_WIDTH = 310.0`, `UI_SLIDER_WIDTH = 140.0`), un contrôle visuel dans Reaper après `./build.sh` permet de confirmer le rendu final avec la police monospace de l'hôte.

## Recommendation

Close the bug — verified end-to-end (contrôles automatisés + validation visuelle dans Reaper).

