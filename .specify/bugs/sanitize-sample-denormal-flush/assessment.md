# Bug Assessment: Flush des valeurs denormalisees dans sanitize_sample() et sc_prev_in_l/r

- **Slug**: sanitize-sample-denormal-flush
- **Created**: 2026-11-16
- **Source**: pasted text (audit de code baseline de `main`)
- **Verdict**: valid
- **Severity**: medium

## Report (verbatim or summarized)

> Flusher a 0.0 les valeurs denormalisees (`abs() < 1.0e-20`) dans `sanitize_sample()` (`src/lib.rs`) et dans `sc_prev_in_l/r` (`src/lib.rs`).

## Symptom

1. `sanitize_sample()` (`src/lib.rs:45-51`) ne verifie que `x.is_finite()`, laissant passer les flottants denormalises (subnormaux, `0.0 < |x| < f32::MIN_POSITIVE` ~ `1.175e-38`, ainsi que tout signal sous `1.0e-20`) aussi bien a l'entree qu'a la sortie finale du thread audio (y compris en bypass).
2. Dans le filtre passe-haut de sidechain (`src/lib.rs:752-753`), `sc_prev_in_l` et `sc_prev_in_r` ne verifient que `driven_*.is_finite()`, alors que `sc_prev_out_l/r` juste au-dessus (`src/lib.rs:741-750`) flushent a `0.0` sous `1.0e-20`.

Comportement attendu (regle 3.4 de `GEMINI.md`) : chaque echantillon entrant et sortant ainsi que toutes les variables d'etat internes de filtres doivent etre proteges contre `NaN`, `Inf` et les denormaux.

## Reproduction

1. Appeler `sanitize_sample(1.0e-39_f32)` ou `sanitize_sample(f32::from_bits(1))` : la fonction retourne le flottant subnormal inchange au lieu de `0.0`.
2. Executer `StereoPunchDsp::process_sample(1.0e-39, -1.0e-39, &cfg)` avec `input_db = 12.0` et `sc_hpf_hz = 90.0` (cas deja present dans `test_nan_inf_and_safety_clamp` a `src/lib.rs:4009`) : `driven_l = 3.9810717e-39 < f32::MIN_POSITIVE` est stocke tel quel comme subnormal dans `dsp.sc_prev_in_l` et `dsp.sc_prev_in_r`.

## Suspected Code Paths

- `src/lib.rs:45-51` — `sanitize_sample(x: f32)` : verifie uniquement `x.is_finite()` puis `x.clamp(-SAFETY_CLAMP_LIMIT, SAFETY_CLAMP_LIMIT)` sans seuil anti-denormaux.
- `src/lib.rs:146-152` — `sanitize_biquad_state(s: f32)` : utilise deja le seuil `s.is_finite() && s.abs() > 1.0e-20` (reference de coherence).
- `src/lib.rs:684-685`, `src/lib.rs:926-927`, `src/lib.rs:1545-1546` — Points d'appel de `sanitize_sample()` en entree et sortie de `StereoPunchDsp::process_sample` et `OmorePunch::process`.
- `src/lib.rs:741-755` — Mise a jour des etats du filtre HPF sidechain : `clean_det_l/r` (`sc_prev_out_l/r`) filtrent `> 1.0e-20`, mais `sc_prev_in_l/r` ne filtrent que `.is_finite()`.
- `src/lib.rs:3996-4152` — `test_nan_inf_and_safety_clamp` : injecte `(1.0e-39, -1.0e-39)` mais n'asserte ni `sanitize_sample()` sur des subnormaux ni `dsp.sc_prev_in_l/r` / `out.out_l/r` non subnormaux.

## Root Cause Hypothesis

Lors de l'implementation initiale de la protection anti-denormaux (`1.0e-20` dans `sanitize_biquad_state`, `sc_prev_out_*`, `env_gr_db`, `auto_slow_gr_db` et `gm_*`), `sanitize_sample()` et les deux memoires d'entree `sc_prev_in_l/r` du HPF sidechain n'ont conserve que la garde `.is_finite()` sans appliquer le meme plancher `> 1.0e-20` (confiance : **high**).

## Proposed Remediation

**Preferred**:
1. Dans `sanitize_sample(x: f32)` (`src/lib.rs:45-51`), flusher les valeurs sous le seuil anti-denormaux (par exemple via une constante partagee `const DENORMAL_FLUSH_THRESHOLD: f32 = 1.0e-20;`) :
   ```rust
   if x.is_finite() && x.abs() > 1.0e-20 {
       x.clamp(-SAFETY_CLAMP_LIMIT, SAFETY_CLAMP_LIMIT)
   } else {
       0.0
   }
   ```
   Un seuil a `1.0e-20` (~`-400 dBFS`) elimine tous les subnormaux (`< ~1.175e-38`) et normalise `-0.0` en `+0.0`, tout en preservant intacte au bit pres (`to_bits()`) toute amplitude audio normale.
2. Dans `StereoPunchDsp::process_sample()` (`src/lib.rs:752-753`), appliquer la meme garde `> 1.0e-20` a `sc_prev_in_l` et `sc_prev_in_r` :
   ```rust
   self.sc_prev_in_l = if driven_l.is_finite() && driven_l.abs() > 1.0e-20 {
       driven_l
   } else {
       0.0
   };
   self.sc_prev_in_r = if driven_r.is_finite() && driven_r.abs() > 1.0e-20 {
       driven_r
   } else {
       0.0
   };
   ```

**Alternatives**:
- Utiliser `!x.is_normal()` au lieu de `abs() <= 1.0e-20` : eliminerait uniquement les subnormaux IEEE 754 (`< 1.175e-38`), mais resterait moins protecteur que le seuil `1.0e-20` deja standardise dans le reste du DSP (un nombre tres proche de `1e-38` pourrait re-basculer en subnormal apres multiplication par un coefficient `< 1`).

**Files likely to change**:
- `src/lib.rs` (`sanitize_sample`, `sc_prev_in_l/r`, et `test_nan_inf_and_safety_clamp` dans `mod tests`)
- `CHANGELOG.md` (entree `Fixed` / `Corrigé` sous `## [Unreleased]`)

**Tests to add or update**:
- Dans `test_nan_inf_and_safety_clamp` (`src/lib.rs:3996`), verifier explicitement :
  - `sanitize_sample(1.0e-39) == 0.0`, `sanitize_sample(-1.0e-39) == 0.0`, `sanitize_sample(f32::from_bits(1)) == 0.0`, `sanitize_sample(-0.0).to_bits() == 0.0_f32.to_bits()`,
  - `out.out_l == 0.0 || out.out_l.is_normal()` et `out.out_r == 0.0 || out.out_r.is_normal()`,
  - `dsp.sc_prev_in_l == 0.0 || dsp.sc_prev_in_l.is_normal()`, `dsp.sc_prev_in_r == 0.0 || dsp.sc_prev_in_r.is_normal()`, ainsi que `sc_prev_out_l/r`.

## Risks & Considerations

- **Transparence bit-exacte (`test_bypass_transparency_and_safety_clamp` et `test_eq_neutral_bypass_transparency`) :** ces tests utilisent des signaux normaux d'amplitude `0.17..0.65` bien au-dessus de `1.0e-20`, donc l'egalite `to_bits()` reste strictement preservee.
- **Temps reel & ASCII (`GEMINI.md`) :** aucun appel allouant ni non-ASCII ; verifier avec `LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs`, `cargo test --release` et le bundling `xtask`.

## Open Questions

- Aucune ambiguite : seuil `1.0e-20` deja uniforme dans tout `StereoPunchDsp`.
