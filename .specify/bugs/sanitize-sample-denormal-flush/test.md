# Bug Verification: Flush des valeurs denormalisees dans sanitize_sample() et sc_prev_in_l/r

- **Slug**: sanitize-sample-denormal-flush
- **Tested**: 2026-11-16
- **Assessment**: ./assessment.md
- **Fix**: ./fix.md
- **Result**: verified

## Summary

Les deux scenarios de reproduction identifies dans `assessment.md` (`sanitize_sample()` sur des subnormaux et `sc_prev_in_l/r` dans `StereoPunchDsp::process_sample()`) ne reproduisent plus le defaut. L'ensemble de la suite de tests workspace (`40 + 2` tests), le controle 100 % ASCII de `src/lib.rs` et la generation des bundles VST3/CLAP en mode Release passent sans regression.

## Checks Performed

| Check | Command / Action | Result | Notes |
|-------|------------------|--------|-------|
| Reproduction (post-fix) | `cargo test --release -- test_nan_inf_and_safety_clamp` | pass | Exercice direct de `sanitize_sample(1.0e-39 / f32::from_bits(1) / -0.0)` et `process_sample(1.0e-39, -1.0e-39)` (etats `sc_prev_in_*/sc_prev_out_*` + sorties actif & bypass) |
| New / updated tests | `cargo test --release -- test_nan_inf_and_safety_clamp` | pass | `1 passed; 0 failed` |
| Regression suite | `cargo test --workspace --release` | pass | `40 passed; 0 failed` (`omore-punch`) + `2 passed; 0 failed` (`xtask`) |
| ASCII compliance check | `LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs \|\| echo "ASCII_OK"` | pass | `ASCII_OK` (aucun caractere non-ASCII dans `src/lib.rs`) |
| Bundle VST3 & CLAP | `cargo run --package xtask --release -- bundle omore-punch --release` | pass | Generation reussie de `Omore Punch.clap`, `Omore Punch.vst3` et `moduleinfo.json` |

## Output Excerpts

```text
running 1 test
test tests::test_nan_inf_and_safety_clamp ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 39 filtered out; finished in 2.06s
```

```text
running 40 tests ... test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.31s
running 2 tests ... test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

```text
Created a CLAP bundle at '/home/tdavio/src/omore-punch/target/bundled/Omore Punch.clap'
Created a VST3 bundle at '/home/tdavio/src/omore-punch/target/bundled/Omore Punch.vst3'
Generated VST3 moduleinfo at '/home/tdavio/src/omore-punch/target/bundled/Omore Punch.vst3/Contents/Resources/moduleinfo.json'
```

## Residual Risks

- La compilation croisee Windows x86_64 (incluant la ressource `VS_VERSION_INFO`) et le test en session Reaper live n'ont pas ete executes dans cet environnement Linux ; la cible Windows sera couverte par le workflow CI GitHub Actions sur la Pull Request.

## Recommendation

Close the bug — verified end-to-end.
