# Bug Verification: Decroissance des vu-metres dependante de la taille de buffer et remise a zero incomplete de MeterTelemetry

- **Slug**: meter-decay-buffer-size-and-reset
- **Tested**: 2026-11-18 (revision 2 + nettoyage post-review)
- **Assessment**: ./assessment.md
- **Fix**: ./fix.md
- **Result**: verified

## Summary

La version finale corrige la regression relevee en premiere review (sous-affichage d'une transitoire en debut de bloc) ainsi que les trois notes mineures de la seconde review (suppression du drapeau redondant `meter_reset_pending`, retrait de la garde `sample_rate` dupliquee dans `PeakMeterBallistics::set_sample_rate()`, retrait de `Copy` sur `PeakMeterBallistics`). Tous les scenarios de reproduction de `assessment.md` sont verifies, et la suite workspace (`43 + 2`), le controle ASCII et le bundling VST3/CLAP passent sans warning ni regression.

## Checks Performed

| Check | Command / Action | Result | Notes |
|-------|------------------|--------|-------|
| Reproduction 1 (redescente / pic de bloc) | `test_peak_meter_ballistics_block_peak_and_segmentation_invariance`, `test_meter_decay_buffer_size_invariance_and_lifecycle_reset` | pass | 44.1 a 192 kHz, decoupages 1 a 2048 et mixtes : pic de bloc exact, enveloppe et valeur publiee identiques au bit pres, transitoire en debut et milieu de bloc affichee a pleine valeur |
| Reproduction 2 (cycle de vie / etat) | idem + `test_clap_wrapper_state_roundtrip_and_host_sync`, `test_vst3_proxy_state_roundtrip_and_host_sync` | pass | `reset()`, `initialize()`, CLAP `stop_processing` (+ relance `start_processing`) / `reset` / `deactivate` / `state_load`, VST3 `set_processing(0)` / `set_active(0)` / `terminate` / `set_state` |
| Test de mutation | Reintroduction temporaire de la publication en fin de bloc | fail attendu | Le test echoue (`sr=48000 [fixed_64]`) : la regression du review est detectee. Mutation retiree. |
| Regression suite | `cargo test --workspace --release` | pass | `43 passed; 0 failed` (`omore-punch`) + `2 passed; 0 failed` (`xtask`), sans warning |
| ASCII compliance check | `LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs \|\| echo "ASCII_OK"` | pass | `ASCII_OK` |
| Bundle VST3 & CLAP | `cargo run --package xtask --release -- bundle omore-punch --release` | pass | `Omore Punch.clap`, `Omore Punch.vst3` et `moduleinfo.json` generes |

## Output Excerpts

```text
test result: ok. 43 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.16s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

```text
Created a CLAP bundle at '/home/tdavio/src/omore-punch/target/bundled/Omore Punch.clap'
Created a VST3 bundle at '/home/tdavio/src/omore-punch/target/bundled/Omore Punch.vst3'
Generated VST3 moduleinfo at '/home/tdavio/src/omore-punch/target/bundled/Omore Punch.vst3/Contents/Resources/moduleinfo.json'
```

## Residual Risks

- Pas de controle visuel dans Reaper live (tailles de buffer multiples, arret/relance du transport) ; recommande avant la fusion via `./build.sh` / `build.ps1`.
- La cible Windows x86_64 sera couverte par la CI GitHub Actions sur la Pull Request.

## Recommendation

Close the bug — verified at unit and CLAP/VST3 wrapper level; a quick visual check in Reaper is recommended before merging.
