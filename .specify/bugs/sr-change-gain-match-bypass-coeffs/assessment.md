# Bug Assessment: Desynchronisation des coefficients Gain Match / Bypass et garde NaN lors d'un changement de sample_rate

- **Slug**: sr-change-gain-match-bypass-coeffs
- **Created**: 2026-11-18
- **Source**: pasted text (audit de code baseline de `main`)
- **Verdict**: valid
- **Severity**: medium

## Report (verbatim or summarized)

> Recalculer `update_gain_match_coeffs(sr)` et `update_bypass_coeff(sr)` (et reinitialiser `auto_slow_gr_db`) quand un changement de `sample_rate` passe par `update_sidechain_hpf()` ou `update_eq()`, et ajouter une garde `.is_finite()` sur `sc_hpf_hz` (et les parametres de `update_ballistics` / `sample_rate`).

## Symptom

1. Lorsque `StereoPunchDsp::update_sidechain_hpf(new_sr, ...)` (`src/lib.rs:489-511`) ou `StereoPunchDsp::update_eq(new_sr, ...)` (`src/lib.rs:611-661`) est appele avec une nouvelle frequence d'echantillonnage `new_sr`, `self.sample_rate` est mis a jour avec `new_sr` sans recalculer `gm_rms_coeff`, `gm_presence_coeff`, `gm_slew_coeff`, `gm_off_slew_coeff` (`update_gain_match_coeffs`) ni `bypass_step` (`update_bypass_coeff`).
2. Comme `self.sample_rate` vaut deja `new_sr`, tout appel ulterieur a `set_sample_rate(new_sr)` (`src/lib.rs:406`) ou `process_sample()` -> `update_ballistics(self.sample_rate, ...)` (`src/lib.rs:447`) evalue `sr_changed == false` et ne recalcule **jamais** les coefficients de Gain Match ni la rampe de bypass, qui restent figes a l'ancienne frequence (ex. `48000.0` Hz).
3. Dans `update_sidechain_hpf()` (`src/lib.rs:491`), `sc_hpf_hz.clamp(20.0, 250.0)` ne verifie pas `sc_hpf_hz.is_finite()` (contrairement a `update_eq()` a `src/lib.rs:631-645`) : en Rust, `f32::NAN.clamp(20.0, 250.0)` renvoie `NaN`, ce qui corrompt `cached_sc_hpf_hz` et `sc_hpf_coeff` si `sr_changed` est vrai. Un risque identique existe dans `update_ballistics()` si `attack_ms` ou `release_ms` est non fini (`src/lib.rs:462-472`), ou si `sample_rate` est `NaN` / `Inf`.
4. Enfin, `self.auto_slow_gr_db` n'est pas remis a `0.0` lors d'un changement direct de `sample_rate` (`set_sample_rate` ou `update_*`), contrairement a ce que documente `CHANGELOG.md:26` (*« reinitialisation de l'integrateur lors des changements de frequence d'echantillonnage ou `reset()` »*).

Comportement attendu : tout changement de `sample_rate`, quel que soit le point d'entree public appele sur `StereoPunchDsp`, doit garantir la coherence complete de tous les coefficients dependants de `sample_rate` (`attack`/`release`/auto-release, sidechain HPF, EQ 3 bandes, Gain Match et rampe Bypass), reinitialiser `auto_slow_gr_db`, et rejeter proprement toute valeur non finie (`NaN`/`Inf`).

## Reproduction

1. Instancier `let mut dsp = StereoPunchDsp::default();` (`sample_rate = 48000.0`).
2. Appeler `dsp.update_sidechain_hpf(96000.0, 90.0);` (ou `dsp.update_eq(96000.0, 80.0, -3.0, 2.0);`), puis `dsp.set_sample_rate(96000.0);` et `dsp.process_sample(0.5, 0.5, &DspConfig::default());`.
3. Comparer `dsp.gm_rms_coeff`, `dsp.gm_presence_coeff`, `dsp.gm_slew_coeff`, `dsp.gm_off_slew_coeff` et `dsp.bypass_step` avec ceux de `StereoPunchDsp::new(96000.0)` : ils conservent les valeurs calculees pour `48000.0` Hz au lieu de `96000.0` Hz (rampe bypass 2x plus rapide, constantes de temps Gain Match divisees par 2).
4. Appeler `dsp.update_sidechain_hpf(44100.0, f32::NAN);` : `dsp.sc_hpf_coeff.is_nan()` vaut `true`.
5. Charger `dsp.auto_slow_gr_db` via quelques echantillons compresses en mode `AutoPunch`, puis appeler `dsp.set_sample_rate(96000.0);` : `dsp.auto_slow_gr_db` reste non nul.

## Suspected Code Paths

- `src/lib.rs:357-370` — `update_gain_match_coeffs()` et `update_bypass_coeff()` : utilisent `sample_rate.max(8000.0)` sans garde `.is_finite()`.
- `src/lib.rs:404-436` — `set_sample_rate()` : duplique l'invalidation des sentinelles de cache et ne remet pas `auto_slow_gr_db` a `0.0`.
- `src/lib.rs:439-486` — `update_ballistics()` : appelle bien `update_gain_match_coeffs`/`update_bypass_coeff` sur `sr_changed`, mais duplique l'invalidation des caches, ne remet pas `auto_slow_gr_db = 0.0` et ne verifie pas `.is_finite()` sur `attack_ms`/`release_ms`.
- `src/lib.rs:489-511` — `update_sidechain_hpf()` : sur `sr_changed`, ecrase `self.sample_rate = sr` sans appeler `update_gain_match_coeffs(sr)` ni `update_bypass_coeff(sr)`, et applique `sc_hpf_hz.clamp(20.0, 250.0)` sans garde `.is_finite()`.
- `src/lib.rs:611-661` — `update_eq()` : sur `sr_changed`, ecrase `self.sample_rate = sr` sans appeler `update_gain_match_coeffs(sr)` ni `update_bypass_coeff(sr)`.

## Root Cause Hypothesis

La logique d'invalidation lors d'un changement de `sample_rate` a ete copiee-collee dans quatre methodes publiques (`set_sample_rate`, `update_ballistics`, `update_sidechain_hpf`, `update_eq`). Lorsque les etages Gain Match (`006-gain-match`), Bypass (`008-bypass-button`) et Auto-Release (`004-compressor-auto-release`) ont ete ajoutes, leurs mises a jour/reinitialisations sur changement de `sample_rate` n'ont ete ajoutees qu'a `set_sample_rate()` et partiellement a `update_ballistics()`, laissant `update_sidechain_hpf()` et `update_eq()` incomplets. Parallelement, les gardes `.is_finite()` ajoutees dans `update_eq()` n'ont pas ete reportees sur `update_sidechain_hpf()`, `update_ballistics()` et la sanitisation de `sample_rate` (confiance : **high**).

## Proposed Remediation

**Preferred**:
1. Introduire un assistant prive `sanitize_sample_rate(sample_rate: f32) -> f32` (repli sur `48000.0` si non fini, sinon `sample_rate.max(8000.0)`) afin d'empecher qu'un `NaN` ou `+Inf` ne corrompe `self.sample_rate` ou les coefficients.
2. Centraliser l'invalidation de changement de frequence dans un unique assistant prive `apply_sample_rate_change(&mut self, sr: f32)` sur `StereoPunchDsp` :
   - met a jour `self.sample_rate = sr`,
   - invalide toutes les sentinelles de cache (`cached_attack_ms = -1.0`, `cached_release_ms = -1.0`, `auto_coeffs_initialized = false`, `cached_sc_hpf_hz = -1.0`, `cached_eq_low_cut_hz = -1.0`, `cached_eq_mid_cut_db = -999.0`, `cached_eq_high_shelf_db = -999.0`),
   - remet `self.auto_slow_gr_db = 0.0`,
   - appelle `self.update_gain_match_coeffs(sr)` et `self.update_bypass_coeff(sr)`.
3. Appeler `self.apply_sample_rate_change(sr)` depuis `set_sample_rate()`, `update_ballistics()`, `update_sidechain_hpf()` et `update_eq()` des que `sr_changed` est vrai, supprimant ainsi la duplication entre les 4 methodes.
   - Note d'implementation dans `set_sample_rate()` : lire les valeurs en cache (`att`, `rel`, `rel_mode`, `hpf`, `eq_lc`, `eq_mc`, `eq_hs`) *avant* d'appeler `apply_sample_rate_change(sr)`, puis appeler `update_ballistics`, `update_sidechain_hpf` et `update_eq` (qui verront `sr_changed == false` mais les sentinelles invalidees, et recalculeront leurs coefficients une seule fois).
4. Ajouter les gardes `.is_finite()` manquantes :
   - `update_sidechain_hpf()` : `let clamped_hz = if sc_hpf_hz.is_finite() { sc_hpf_hz.clamp(20.0, 250.0) } else { 20.0 };`
   - `update_ballistics()` : `let att = if attack_ms.is_finite() { attack_ms.clamp(0.1, 100.0) } else { 3.0 };` et `let rel = if release_ms.is_finite() { release_ms.clamp(10.0, 500.0) } else { 80.0 };`.

**Alternatives**:
- Dupliquer `update_gain_match_coeffs(sr)`, `update_bypass_coeff(sr)` et `auto_slow_gr_db = 0.0` dans `update_sidechain_hpf()` et `update_eq()` sans factoriser : deconseille car la duplication dans 4 methodes est la cause directe de ce bug.

**Files likely to change**:
- `src/lib.rs` (`StereoPunchDsp::set_sample_rate`, `update_ballistics`, `update_sidechain_hpf`, `update_eq`, assistants prives `sanitize_sample_rate` / `apply_sample_rate_change`, et nouveaux tests unitaires dans `mod tests`)
- `CHANGELOG.md` (entree sous `## [Unreleased]` / `### Corrigé`)

**Tests to add or update**:
- Ajouter un test unitaire dans `mod tests` (ex. `test_sample_rate_change_updates_all_coeffs_and_guards_non_finite`) verifiant :
  1. Qu'un changement de `sample_rate` (`48000.0 -> 96000.0`) declenche via `update_sidechain_hpf()`, `update_eq()`, `update_ballistics()` ou `set_sample_rate()` produit exactement les memes `gm_rms_coeff`, `gm_presence_coeff`, `gm_slew_coeff`, `gm_off_slew_coeff`, `bypass_step` (et apres `process_sample`, les memes `attack_coeff`, `release_coeff`, `auto_*_coeff`, `sc_hpf_coeff` et biquads EQ) qu'une instance neuve `StereoPunchDsp::new(96000.0)`.
  2. Que `auto_slow_gr_db` est remis a `0.0` apres un changement de `sample_rate`.
  3. Que des appels avec `f32::NAN` / `f32::INFINITY` sur `sample_rate`, `sc_hpf_hz`, `attack_ms`, `release_ms` laissent tous les coefficients finis et valides (sans jamais produire `NaN`).

## Risks & Considerations

- **Performance temps reel (`GEMINI.md` regle 3.3) :** Aucun calcul supplementaire en regime etabli (`sr_changed == false` reste une simple comparaison flottante sans appel `.exp()`/`.sin()`/`.cos()`). Zéro allocation, zéro verrou.
- **Compatibilite des tests existants :** Les tests d'auto-release (`test_auto_release_*`), EQ (`test_eq_low_cut_sub_attenuation`), Gain Match (`test_gain_match_*`) et Bypass (`test_bypass_*`) utilisent des parametres finis dans les plages nominales (`attack_ms` in `[0.1, 100.0]`, `release_ms` in `[10.0, 500.0]`, `sc_hpf_hz` in `[20.0, 250.0]`). Verifier qu'aucun test ne passe volontairement un `attack_ms` ou `release_ms` hors plage avant d'appliquer un `.clamp(...)` dans `update_ballistics()` (au besoin, verifier avec `grep` pendant `/speckit-bug-fix` ou utiliser `.max(0.1)` / `.max(10.0)` comme dans `set_sample_rate`).
- **Conformite `src/lib.rs` (`GEMINI.md`) :** 100 % ASCII pur (`LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs`), `cargo test --workspace --release` et bundling `xtask`.

## Open Questions

- Aucune ambiguite bloquante. Point a confirmer pendant `/speckit-bug-fix` : verifier si un test unitaire passe une valeur `attack_ms > 100.0` ou `release_ms > 500.0` a `update_ballistics()` (sinon `.clamp(0.1, 100.0)` / `.clamp(10.0, 500.0)` est strictement aligne avec `OmorePunchParams`).
