# Bug Verification: Validation complète automatisée et visuelle dans Reaper (Linux Debian 13)

- **Slug**: ui-bottom-clipping
- **Tested**: 2026-11-14
- **Assessment**: ./assessment.md
- **Fix**: ./fix.md
- **Result**: verified

## Summary

Sur la branche `fix/ui-bottom-clipping`, tous les contrôles automatisés passent sans régression (`16 passed; 0 failed`, contrôle ASCII pur et génération des bundles VST3/CLAP) et la validation visuelle en conditions réelles dans Reaper sous Linux (Debian 13) confirme l'affichage intégral de la fenêtre (`OMORE PUNCH v0.2.1 [DEV f5767fb-dirty]`), incluant les valeurs et boutons `-` / `+` / `Reset` de `3. DRUM BUS EQ` ainsi que les labels `-inf dB` de `4. GAIN REDUCTION & STEREO BUS METERING`, sans barre de défilement ni rognage.

## Checks Performed

| Check | Command / Action | Result | Notes |
|-------|------------------|--------|-------|
| Reproduction (post-fix, live DAW) | Ouverture de l'éditeur `OMORE PUNCH v0.2.1 [DEV f5767fb-dirty]` dans Reaper sous Debian 13 (capture utilisateur) | pass | Section `3. DRUM BUS EQ` complète (valeurs, `-` / `+` / `Reset`, bordure basse) et labels `-inf dB` de `In L/R` / `Out L/R` entièrement visibles sans scrollbar |
| New / updated tests | `cargo test --release -- test_editor_size_ignores_legacy_persisted_editor_state` (inclus dans `cargo test --release`) | pass | Vérifie l'absence de `"editor-state"` dans `serialize_fields()` et le maintien de `(820, 620)` après `deserialize_fields` avec `820x560` et `820x480` |
| Regression suite | `cargo test --release` | pass | `16 passed; 0 failed` (DSP + header + `test_ui_layout_column_widths_fit_window` avec `ScrollArea::vertical()`) |
| Lint / ASCII check | `LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs` | pass | Exit code `1` (zéro caractère non-ASCII dans `src/lib.rs`) |
| Bundle build | `cargo run --package xtask --release -- bundle omore-punch --release` | pass | Bundles générés : `target/bundled/omore-punch.clap` et `target/bundled/omore-punch.vst3` |

## Output Excerpts

```text
$ git diff --stat
 CHANGELOG.md |  3 +++
 src/lib.rs   | 64 +++++++++++++++++++++++++++++++++++++++++++++---------------
 2 files changed, 51 insertions(+), 16 deletions(-)

$ LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs; echo "ascii_exit=$?"
ascii_exit=1

$ cargo test --release
running 16 tests
test tests::test_editor_size_ignores_legacy_persisted_editor_state ... ok
test tests::test_ui_layout_column_widths_fit_window ... ok
...
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s

$ cargo run --package xtask --release -- bundle omore-punch --release
Created a CLAP bundle at '/home/tdavio/src/omore-punch/target/bundled/omore-punch.clap'
Created a VST3 bundle at '/home/tdavio/src/omore-punch/target/bundled/omore-punch.vst3'
```

## Residual Risks

- Aucun risque résiduel bloquant identifié : le comportement sur sessions antérieures est verrouillé par `test_editor_size_ignores_legacy_persisted_editor_state`, la `ScrollArea::vertical()` reste en filet de sécurité si un hôte contraint la hauteur, et l'affichage nominal `820x620` est validé visuellement dans Reaper sous Linux.

## Recommendation

Close the bug — verified end-to-end (`fix/ui-bottom-clipping` prêt pour commit et PR vers `main`).
