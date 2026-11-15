# Bug Verification: Warning de compilation `semicolon_in_expressions_from_non_local_macros` sur `nih_export_vst3!`

- **Slug**: vst3-export-macro-warning
- **Tested**: 2026-11-14
- **Assessment**: ./assessment.md
- **Fix**: ./fix.md
- **Result**: verified

## Summary

Une recompilation propre du crate `omore-punch` (`cargo clean --release -p omore-punch` puis `cargo test --release`) et la génération des bundles VST3/CLAP via `xtask` s'achèvent avec `0` warning sous `rustc 1.99.0`, tout en validant les 7 tests unitaires existants et le contrôle ASCII strict de `src/lib.rs`.

## Checks Performed

| Check | Command / Action | Result | Notes |
|-------|------------------|--------|-------|
| Reproduction (post-fix) | `cargo clean --release -p omore-punch` puis `cargo test --release` | pass | Recompilation complète (`17.54s`) sans aucun warning `semicolon_in_expressions_from_non_local_macros` ni `unused_attributes`. |
| New / updated tests | Aucun nouveau test unitaire ajouté | skipped | Défaut lié uniquement à un lint de compilation amont ; validé par compilation propre à `0` warning et exécution complète des tests existants. |
| Regression suite | `cargo test --release` | pass | `7 passed; 0 failed; 0 ignored` (tous les tests DSP et UI passent). |
| Lint / type-check | `LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs` & `cargo run --package xtask --release -- bundle omore-punch --release` | pass | Code de sortie `1` / `0` caractère non-ASCII dans `src/lib.rs` ; bundles `target/bundled/omore-punch.clap` et `target/bundled/omore-punch.vst3` générés sans warning. |

## Output Excerpts

```text
   Compiling omore-punch v0.2.1-dev (/home/tdavio/src/omore-punch)
    Finished `release` profile [optimized] target(s) in 17.54s
     Running unittests src/lib.rs (target/release/deps/omore_punch-cf1d58ea9529a417)

running 7 tests
test tests::test_clipper_strict_ceiling ... ok
test tests::test_parallel_mix_dry_wet ... ok
test tests::test_nan_inf_and_safety_clamp ... ok
test tests::test_compressor_stereo_linked_reduction ... ok
test tests::test_ui_layout_column_widths_fit_window ... ok
test tests::test_sidechain_hpf_kick_immunity ... ok
test tests::test_sidechain_hpf_snare_safe_200hz ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

```text
    Finished `release` profile [optimized] target(s) in 0.10s
     Running `target/release/xtask bundle omore-punch --release`
    Finished `release` profile [optimized] target(s) in 0.17s

Created a CLAP bundle at '/home/tdavio/src/omore-punch/target/bundled/omore-punch.clap'
Created a VST3 bundle at '/home/tdavio/src/omore-punch/target/bundled/omore-punch.vst3'
```

## Residual Risks

- Si une future mise à jour de `nih-plug` / `vst3-sys` corrige `vst3_com::vtable!` en amont, l'attribut `#![allow(semicolon_in_expressions_from_non_local_macros)]` en tête de `src/lib.rs` pourra être retiré sans impact fonctionnel.
- L'attribut interne `#![allow(semicolon_in_expressions_from_non_local_macros)]` s'applique au niveau du crate `src/lib.rs` (nécessaire car `rustc 1.99.0` ignore `#[allow(...)]` sur l'invocation de macro `nih_export_vst3!(OmorePunch)`).

## Recommendation

Close the bug — verified end-to-end.
