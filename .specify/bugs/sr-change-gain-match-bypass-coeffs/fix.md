# Bug Fix: Desynchronisation des coefficients Gain Match / Bypass et garde NaN lors d'un changement de sample_rate

- **Slug**: sr-change-gain-match-bypass-coeffs
- **Fixed**: 2026-11-18
- **Assessment**: ./assessment.md
- **Status**: applied

## Summary

Centralisation de la sanitisation de `sample_rate` (`sanitize_sample_rate`) et de l'invalidation/recalcul des coefficients (`apply_sample_rate_change`) dans `StereoPunchDsp` afin que tout changement de frequence d'echantillonnage (via `set_sample_rate`, `update_ballistics`, `update_sidechain_hpf` ou `update_eq`) recalcule uniformement `gm_*_coeff` et `bypass_step`, reinitialise `auto_slow_gr_db = 0.0`, et protege `sample_rate`, `sc_hpf_hz`, `attack_ms` et `release_ms` contre `NaN`/`Inf`.

## Changes

| File | Change | Notes |
|------|--------|-------|
| `src/lib.rs` | modified | Ajout de `sanitize_sample_rate()` (L357) et `apply_sample_rate_change()` (L384) ; refactorisation de `set_sample_rate()` (L430), `update_ballistics()` (L476), `update_sidechain_hpf()` (L527) et `update_eq()` (L644) avec gardes `.is_finite()` |
| `src/lib.rs` | added test | Ajout de `test_sample_rate_change_updates_all_coeffs_and_guards_non_finite` (L8547) |
| `CHANGELOG.md` | modified | Ajout d'une entree sous `## [Unreleased]` / `### Corrigé` |

## Diff Highlights

```rust
#[inline(always)]
fn sanitize_sample_rate(&self, sample_rate: f32) -> f32 {
    if sample_rate.is_finite() {
        sample_rate.max(8000.0)
    } else if self.sample_rate.is_finite() {
        self.sample_rate.max(8000.0)
    } else {
        48000.0
    }
}

#[inline(always)]
fn apply_sample_rate_change(&mut self, sr: f32) {
    self.sample_rate = sr;
    self.cached_attack_ms = -1.0;
    self.cached_release_ms = -1.0;
    self.auto_coeffs_initialized = false;
    self.cached_sc_hpf_hz = -1.0;
    self.cached_eq_low_cut_hz = -1.0;
    self.cached_eq_mid_cut_db = -999.0;
    self.cached_eq_high_shelf_db = -999.0;
    self.auto_slow_gr_db = 0.0;
    self.update_gain_match_coeffs(sr);
    self.update_bypass_coeff(sr);
}
```

## Tests Added or Updated

- `src/lib.rs::tests::test_sample_rate_change_updates_all_coeffs_and_guards_non_finite` — Verifie que le passage de `48000.0` a `96000.0` Hz via chacun des 4 points d'entree publics (`set_sample_rate`, `update_ballistics`, `update_sidechain_hpf`, `update_eq`) reinitialise `auto_slow_gr_db` a `0.0` et produit les memes coefficients (`gm_*_coeff`, `bypass_step`, balistiques, `sc_hpf_coeff` et biquads EQ) qu'une instance neuve `StereoPunchDsp::new(96000.0)` ; verifie aussi que des valeurs `NaN`/`±Inf` sur `sample_rate`, `sc_hpf_hz`, `attack_ms`, `release_ms` et parametres EQ laissent tous les coefficients finis.

## Local Verification

- Commands run: `LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs || echo "ASCII_OK"` → `ASCII_OK` (exit 0)
- Commands run: `cargo test --release` → `test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`
- Manual checks: verification de l'ensemble des occurrences `attack_ms:` / `release_ms:` dans `src/lib.rs` (toutes dans `[0.1, 100.0]` / `[10.0, 500.0]`) et relecture RT (regles 3.1-3.4 de `GEMINI.md` : zero allocation, zero verrou, zero recalcul en regime etabli).

## Deviations from Assessment

Aucune deviation fonctionnelle. `sanitize_sample_rate(&self, sample_rate: f32)` prefere conserver `self.sample_rate` (si fini) avant de retomber sur `48000.0` lorsqu'un `sample_rate` non fini est passe, afin de ne pas perturber une instance deja configuree a une frequence valide (ex. `96000.0` Hz).

## Follow-ups

- Executer `/speckit-bug-test slug=sr-change-gain-match-bypass-coeffs` pour completer la validation (`--workspace` et bundling VST3/CLAP) et generer `test.md`.
