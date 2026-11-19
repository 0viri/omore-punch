# Bug Assessment: Decroissance des vu-metres dependante de la taille de buffer et remise a zero incomplete de MeterTelemetry

- **Slug**: meter-decay-buffer-size-and-reset
- **Created**: 2026-11-18
- **Source**: pasted text (audit de code baseline de `main`)
- **Verdict**: valid
- **Severity**: medium

## Report (verbatim or summarized)

> Rendre la decroissance des vu-metres `in_peak_l/r` et `out_peak_l/r` independante de la taille du buffer audio et de la frequence d'echantillonnage (`src/lib.rs:1590-1598`), et remettre a zero `in_peak_l/r`, `out_peak_l/r` et `gain_match_db` lors des evenements de reset / cycle de vie / chargement d'etat sans casser l'independance du bouton UI `[Reset Peak]` (`reset_peaks()`).

## Symptom

1. Dans `OmorePunch::process()` (`src/lib.rs:1589-1598`), la decroissance des cretes `in_peak_l/r` et `out_peak_l/r` applique un facteur fixe `* 0.88` **une fois par appel a `process()`**, independamment du nombre d'echantillons du bloc et de la frequence d'echantillonnage :
   - A `48 kHz` avec un buffer de `64` echantillons (`750 blocs/s`), le vu-metre chute de `-60 dB` en `~72 ms` (scintillement tres rapide).
   - A `48 kHz` avec un buffer de `2048` echantillons (`23,4 blocs/s`), la meme chute prend `~2,30 s` (**32x plus lent**), et varie encore en `96 kHz` ou `192 kHz` ainsi que sur les sous-blocs decoupes par l'automation sample-accurate.
   - En outre, la multiplication repetee `* 0.88` sans flush anti-denormaux laisse les valeurs atomiques de cretes deriver vers des subnormaux en periode de silence.
2. Lors des evenements de cycle de vie et de chargement d'etat (`OmorePunch::initialize` `L1485`, `OmorePunch::reset` `L1493`, `clap_state_load` `L2895`, `clap_deactivate` `L2933`, `clap_stop_processing` `L2944`, `clap_reset` `L2955`, `vst3::terminate` `L3204`, `vst3::set_active(0)` `L3253`, `vst3::set_state` `L3262`, `vst3::set_processing(0)` `L3383`), seule `clear_gain_reduction()` (`current_gr_db = 0.0`) est appelee :
   - Les barres `In L/R` et `Out L/R` restent figees a leur derniere hauteur quand le DAW arrete le traitement (`stop_processing` / `set_processing(0)`) ou reinitialise le plugin.
   - Alors que `StereoPunchDsp::reset()` remet `self.gm_gain_db = 0.0` (`L399`), `telemetry.gain_match_db` conserve son ancienne valeur (`Trim: +X.X dB`) apres `initialize()`, `reset()`, `deactivate()` ou un chargement de preset a l'arret.

Comportement attendu :
- La constante de temps de redescente des vu-metres `In L/R` et `Out L/R` doit etre physiquement identique (en millisecondes) quelle que soit la taille de buffer (`32` a `2048` ou sous-blocs variables) et quelle que soit la frequence d'echantillonnage (`44.1` a `192 kHz`), sans appel `.exp()` dans `process()`.
- Les evenements de cycle de vie / arret de traitement doivent remettre a `0.0` les jauges instantanees (`in_peak_l/r`, `out_peak_l/r`, `current_gr_db`), et les resets / chargement d'etat / desactivation doivent aussi remettre `gain_match_db` a `0.0`, tandis que `reset_peaks()` (bouton UI `[Reset Peak]`) continue de ne toucher qu'a `max_gr_db` et `clip_latched`.

## Reproduction

1. Initialiser deux instances `OmorePunch` a `48000 Hz`, injecter une impulsion de `1.0` suivie de `4096` echantillons de silence (`85.3 ms`) :
   - Instance A par blocs de `64` echantillons : `in_peak_l` tombe a `0.88^64 ~ 0.00028` (`-71 dB`).
   - Instance B par blocs de `2048` echantillons : `in_peak_l` reste a `0.88^2 = 0.7744` (`-2.2 dB`).
2. Alimenter `OmorePunch` en audio avec `gain_match = true`, puis appeler `plugin.reset()` (ou `clap_state_load` / `vst3::set_state` / `set_processing(0)`) : `telemetry.in_peak_l`, `telemetry.out_peak_l` et `telemetry.gain_match_db` restent non nuls alors que `dsp.gm_gain_db == 0.0`.

## Suspected Code Paths

- `src/lib.rs:971-1009` — `MeterTelemetry` : `clear_gain_reduction()` ne remet a `0.0` que `current_gr_db` ; aucune methode ne nettoie les cretes live (`in_peak_l/r`, `out_peak_l/r`) ni `gain_match_db`.
- `src/lib.rs:1415-1439` — `OmorePunch` : ne stocke aucun coefficient de decroissance de vu-metre precalcule par echantillon.
- `src/lib.rs:1476-1494` — `OmorePunch::initialize()` et `OmorePunch::reset()` : n'initialisent pas de coefficient de decroissance de vu-metre et ne remettent a zero que `current_gr_db` (et `reset_peaks()` dans `initialize`).
- `src/lib.rs:1589-1598` — `OmorePunch::process()` : applique `* 0.88` une fois par bloc au lieu d'une decroissance calibree en temps reel, sans flush anti-denormaux.
- `src/lib.rs:2892-2959` (`mod clap`) et `src/lib.rs:3203-3386` (`mod vst3`) — Appels a `clear_gain_reduction()` sur `clap_state_load`, `clap_deactivate`, `clap_stop_processing`, `clap_reset`, `vst3::terminate`, `vst3::set_active(0)`, `vst3::set_state` et `vst3::set_processing(0)`.

## Root Cause Hypothesis

1. La decroissance des vu-metres `In L/R` / `Out L/R` a ete codee par simplicite avec un facteur scalaire par bloc (`* 0.88`) calibre empiriquement autour de `48 kHz / 512` echantillons (`~85 ms` de constante de temps), sans l'indexer sur `sample_rate` ni sur le nombre d'echantillons traites.
2. Lorsque les extensions de metering GR natif (`007-gain-reduction-metering`), Gain Match (`006-gain-match`) et Presets Reaper (`008-reaper-presets`) ont ete ajoutees, les hooks de cycle de vie CLAP/VST3/Plugin ont ete branches sur `clear_gain_reduction()` uniquement pour remettre a zero le GR hote, en omettant les barres `in_peak_l/r`, `out_peak_l/r` et l'affichage `gain_match_db` (confiance : **high**).

## Proposed Remediation

**Preferred**:
1. **Decroissance de vu-metre par echantillon precalculee (zero `.exp()` dans `process()`, independante du buffer et du `sample_rate`) :**
   - Definir une constante `const METER_PEAK_RELEASE_MS: f32 = 85.0;` (qui correspond a `~0.882` sur un bloc de `512` echantillons a `48 kHz`, conservant le ressenti visuel habituel tout en devenant invariant en temps reel).
   - Ajouter un champ `meter_decay_coeff: f32` sur `OmorePunch`, initialise dans `Default` a `StereoPunchDsp::exp_coeff_from_ms(METER_PEAK_RELEASE_MS, 48000.0, 0.001)` et mis a jour dans `OmorePunch::initialize()` avec la frequence d'echantillonnage sanitisee (`self.dsp.sample_rate`).
   - Dans `OmorePunch::process()`, charger `peak_in_l/r` et `peak_out_l/r` en debut de bloc (via `flush_denormal(load_f32_relaxed(...))`), mettre a jour par echantillon dans la boucle `buffer.iter_samples()` :
     ```rust
     peak_in_l = (peak_in_l * meter_decay).max(clean_in_l.abs());
     peak_in_r = (peak_in_r * meter_decay).max(clean_in_r.abs());
     ...
     peak_out_l = (peak_out_l * meter_decay).max(out.out_l.abs());
     peak_out_r = (peak_out_r * meter_decay).max(out.out_r.abs());
     ```
     puis ecrire une seule fois en fin de bloc via `store_f32_relaxed(..., flush_denormal(peak_*))`.
     Ainsi :
     - Aucun `.exp()` n'est jamais appele dans `process()` (regle 3.3 de `GEMINI.md`).
     - La trajectoire de decroissance est identique quelle que soit la taille du bloc (`32`, `64`, `512`, `2048` ou decoupages variables) et la frequence d'echantillonnage (`44.1`..`192 kHz`).
     - Aucun flottant subnormal n'est stocke dans les atomiques de cretes.
2. **Methodes de remise a zero semantiques sur `MeterTelemetry` :**
   - Conserver `reset_peaks(&self)` strictement inchange (`max_gr_db = 0.0`, `clip_latched = false`) pour le bouton UI `[Reset Peak]`.
   - Conserver `clear_gain_reduction(&self)` (`current_gr_db = 0.0`) pour compatibilite avec les tests existants.
   - Ajouter `clear_live_meters(&self)` : remet a `0.0` `current_gr_db`, `in_peak_l`, `in_peak_r`, `out_peak_l`, `out_peak_r` -> appelee dans `clap_stop_processing` et `vst3::set_processing(0)` (arret de transport sans perte du trim Gain Match en memoire DSP si le DAW ne fait pas de `reset()`).
   - Ajouter `clear_lifecycle_telemetry(&self)` : appelle `clear_live_meters()` et remet aussi `gain_match_db = 0.0` -> appelee dans `OmorePunch::initialize()`, `OmorePunch::reset()`, `clap_deactivate`, `clap_reset`, `clap_state_load`, `vst3::terminate`, `vst3::set_active(0)` et `vst3::set_state`.

**Alternatives**:
- Calculer `(-num_samples / (tau * sr)).exp()` par bloc avec un cache `(sr, block_len)` : plus complexe, appelle quand meme `.exp()` des que la taille de sous-bloc varie (automation sample-accurate), et depend legerement de la position de l'impulsion dans un grand bloc (`2048`). L'approche par echantillon precalculee est plus simple, plus exacte et 100 % sans `.exp()` dans `process()`.

**Files likely to change**:
- `src/lib.rs` (`METER_PEAK_RELEASE_MS`, `MeterTelemetry::clear_live_meters` / `clear_lifecycle_telemetry`, `OmorePunch::meter_decay_coeff`, `initialize`, `reset`, `process`, appels lifecycle dans `mod clap` / `mod vst3`, et tests unitaires dans `mod tests`)
- `CHANGELOG.md` (entree sous `## [Unreleased]` / `### Corrigé`)

**Tests to add or update**:
- Ajouter un test unitaire (ex. `test_meter_decay_buffer_size_invariance_and_lifecycle_reset`) verifiant :
  1. Qu'a `48 kHz` et `96 kHz`, une impulsion suivie de `85 ms` de silence traitee en blocs de `64`, `512` et `2048` echantillons (ainsi que des tailles mixtes) donne le meme niveau `in_peak_l/r` et `out_peak_l/r` (`~1/e = 0.3679` a tolerance `< 1e-3`), sans subnormaux apres un long silence.
  2. Que `OmorePunch::reset()`, `OmorePunch::initialize()`, `clear_live_meters()`, `clear_lifecycle_telemetry()`, `clap_state_load` et `vst3::set_state` nettoient correctement `in_peak_l/r`, `out_peak_l/r`, `current_gr_db` et (pour les resets de cycle de vie/etat) `gain_match_db`, tandis que `reset_peaks()` preserve `current_gr_db`, `in_peak_*`, `out_peak_*` et `gain_match_db`.

## Risks & Considerations

- **Cout CPU temps reel (`GEMINI.md` regles 3.1-3.3) :** 4 multiplications et 4 `f32::max` scalaires sur registres locaux par echantillon, zero allocation, zero verrou, zero `.exp()` dans `process()`.
- **Compatibilite des tests existants (`test_host_gr_telemetry_parity_clamp_and_independence`, `test_clap_*`, `test_vst3_*`) :** `reset_peaks()` et `clear_gain_reduction()` gardent leur signature et leur comportement ; `clear_live_meters()` / `clear_lifecycle_telemetry()` etendent proprement le nettoyage des jauges live.
- **Note sur l'arret de transport :** Dans `clap_stop_processing` / `vst3::set_processing(0)`, garder `gain_match_db` (via `clear_live_meters`) evite d'effacer a tort le `Trim` affiche si Reaper met simplement le flux en pause sans rappeler `reset()` ; des que `reset()`, `deactivate()`, `set_active(0)` ou `set_state()` intervient, `clear_lifecycle_telemetry()` remet `gain_match_db` a `0.0`.
- **Conformite `src/lib.rs` (`GEMINI.md`) :** 100 % ASCII pur (`LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs`), `cargo test --workspace --release` et bundling `xtask`.

## Open Questions

- Aucune ambiguite bloquante.
