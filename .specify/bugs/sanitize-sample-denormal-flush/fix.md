# Bug Fix: Flush des valeurs denormalisees dans sanitize_sample() et sc_prev_in_l/r

- **Slug**: sanitize-sample-denormal-flush
- **Fixed**: 2026-11-16
- **Assessment**: ./assessment.md
- **Status**: applied

## Summary

Extraction de `DENORMAL_FLUSH_THRESHOLD` (`1.0e-20`) et de l'assistant `flush_denormal()` dans `src/lib.rs` pour flusher a `+0.0` les flottants denormalises dans `sanitize_sample()` et sur les memoires d'entree/sortie du filtre HPF sidechain (`sc_prev_in_l/r`, `sc_prev_out_l/r`), tout en preservant la transparence bit-exacte des signaux normaux.

## Changes

| File | Change | Notes |
|------|--------|-------|
| `src/lib.rs` | modified | Ajout de `DENORMAL_FLUSH_THRESHOLD` et `flush_denormal()` (L17, L45-L57) ; mise a jour de `sanitize_sample()` et des etats `sc_prev_in_l/r` / `clean_det_l/r` (L747-L751) |
| `src/lib.rs` | updated test | Ajout d'assertions subnormales dans `test_nan_inf_and_safety_clamp` (L4001-L4024, L4089-L4110, L4198-L4215) |
| `CHANGELOG.md` | modified | Ajout d'une entree sous `## [Unreleased]` / `### Corrigé` |

## Diff Highlights

```rust
const DENORMAL_FLUSH_THRESHOLD: f32 = 1.0e-20;

#[inline(always)]
fn flush_denormal(x: f32) -> f32 {
    if x.is_finite() && x.abs() > DENORMAL_FLUSH_THRESHOLD {
        x
    } else {
        0.0
    }
}

#[inline(always)]
pub fn sanitize_sample(x: f32) -> f32 {
    flush_denormal(x).clamp(-SAFETY_CLAMP_LIMIT, SAFETY_CLAMP_LIMIT)
}
```

## Tests Added or Updated

- `src/lib.rs::tests::test_nan_inf_and_safety_clamp` — Verifie le flush exact vers `+0.0` dans `sanitize_sample()` (subnormaux `1.0e-39`, `f32::from_bits(1)`, seuil `1.0e-20`, `-0.0`, `NaN`, `±Inf`), l'absence de sortie ou d'etat `sc_prev_in_*/sc_prev_out_*` subnormal dans la boucle pathologique, et le flush complet sur entree purement subnormale en modes actif et bypass.

## Local Verification

- Commands run: `LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs || echo "ASCII_OK"` → `ASCII_OK` (exit 0)
- Commands run: `cargo test --release` → `test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`
- Manual checks: `git diff src/lib.rs CHANGELOG.md` relu (zero allocation/verrou, transparence bit-exacte preservee pour les signaux normaux)

## Deviations from Assessment

Aucune deviation fonctionnelle. Par souci de non-duplication dans `src/lib.rs`, la constante `DENORMAL_FLUSH_THRESHOLD` (`1.0e-20`) et le helper `flush_denormal()` ont ete partages entre `sanitize_sample()`, `clean_det_l/r` et `sc_prev_in_l/r`.

## Follow-ups

- Executer `/speckit-bug-test slug=sanitize-sample-denormal-flush` pour completer la verification (dont le bundling VST3/CLAP) et generer `test.md`.
