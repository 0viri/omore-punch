# Bug Verification: Desynchronisation des coefficients Gain Match / Bypass et garde NaN lors d'un changement de sample_rate

- **Slug**: sr-change-gain-match-bypass-coeffs
- **Tested**: 2026-11-18
- **Assessment**: ./assessment.md
- **Fix**: ./fix.md
- **Result**: verified

## Summary

Tous les scenarios de reproduction identifies dans `assessment.md` (changement de `sample_rate` via `update_sidechain_hpf`, `update_eq`, `update_ballistics` ou `set_sample_rate`, reinitialisation de `auto_slow_gr_db`, et gardes `.is_finite()` sur `sample_rate`, `sc_hpf_hz`, `attack_ms`, `release_ms`) ne reproduisent plus le bug. La suite complete workspace (`41 + 2` tests), le controle 100 % ASCII de `src/lib.rs` et la generation des bundles VST3/CLAP en mode Release passent sans regression.

## Checks Performed

| Check | Command / Action | Result | Notes |
|-------|------------------|--------|-------|
| Reproduction (post-fix) | `cargo test --release -- test_sample_rate_change_updates_all_coeffs_and_guards_non_finite` | pass | Exercice direct des 4 points d'entree de changement de `sample_rate`, de la remise a `0.0` de `auto_slow_gr_db` et des entrees `NaN`/`±Inf` |
| New / updated tests | `cargo test --release -- test_sample_rate_change_updates_all_coeffs_and_guards_non_finite` | pass | `1 passed; 0 failed` |
| Regression suite | `cargo test --workspace --release` | pass | `41 passed; 0 failed` (`omore-punch`) + `2 passed; 0 failed` (`xtask`) |
| ASCII compliance check | `LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs \|\| echo "ASCII_OK"` | pass | `ASCII_OK` (aucun caractere non-ASCII dans `src/lib.rs`) |
| Bundle VST3 & CLAP | `cargo run --package xtask --release -- bundle omore-punch --release` | pass | Generation reussie de `Omore Punch.clap`, `Omore Punch.vst3` et `moduleinfo.json` |

## Output Excerpts

```text
running 1 test
test tests::test_sample_rate_change_updates_all_coeffs_and_guards_non_finite ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 40 filtered out; finished in 0.00s
```

```text
running 41 tests ... test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.14s
running 2 tests ... test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

```text
Created a CLAP bundle at '/home/tdavio/src/omore-punch/target/bundled/Omore Punch.clap'
Created a VST3 bundle at '/home/tdavio/src/omore-punch/target/bundled/Omore Punch.vst3'
Generated VST3 moduleinfo at '/home/tdavio/src/omore-punch/target/bundled/Omore Punch.vst3/Contents/Resources/moduleinfo.json'
```

## Residual Risks

- La compilation Windows x86_64 (`VS_VERSION_INFO` + bundles Windows) et un changement dynamique de frequence d'echantillonnage dans Reaper live ne sont pas executes dans cet environnement Linux ; la cible Windows sera couverte par la CI GitHub Actions sur la Pull Request.

## Recommendation

Close the bug — verified end-to-end.
