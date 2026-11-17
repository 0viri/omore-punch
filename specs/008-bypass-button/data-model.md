# Data Model: Bouton Bypass Global (`008-bypass-button`)

## 1. Parametre DAW (`OmorePunchParams`)

| Champ | Type | ID DAW | Nom affiche | Valeur par defaut | Flags / Metadata |
|---|---|---|---|---|---|
| `bypass` | `BoolParam` | `"bypass"` | `"Bypass"` | `false` (`OFF`) | `.make_bypass()` (`ParamFlags::BYPASS` -> VST3 `kIsBypass` + `kCanAutomate`, CLAP `CLAP_PARAM_IS_BYPASS` + `CLAP_PARAM_IS_AUTOMATABLE`) |

- **Regles de validation**:
  - Un seul parametre avec `ParamFlags::BYPASS` dans `OmorePunchParams` (respecte la contrainte `nih_plug`).
  - Persiste automatiquement dans l'etat DAW (`serialize_fields` / `deserialize_fields`), compatible avec les sessions existantes qui n'ont pas la cle `"bypass"` (repli sur `false`).

## 2. Configuration et Etat DSP (`DspConfig`, `StereoPunchDsp`)

### Extension de `DspConfig`

| Champ | Type | Defaut | Role |
|---|---|---|---|
| `bypass` | `bool` | `false` | Active (`true`) ou desactive (`false`) le contournement soft bypass |

### Extension de `StereoPunchDsp`

| Champ | Type | Plage / Init | Role |
|---|---|---|---|
| `bypass_wet` | `f32` | `[0.0, 1.0]`, init `1.0` | Facteur courant de crossfade (`1.0` = 100 % traite, `0.0` = 100 % dry bypass) |
| `bypass_step` | `f32` | `1.0 / (0.010 * sr)` | Pas lineaire par echantillon pour une rampe de `10.0 ms`, precalcule uniquement au changement de `sample_rate` |

- **Invariants**:
  - `bypass_wet` est toujours borne dans `[0.0, 1.0]` et protege contre les valeurs non finies.
  - `bypass_step` est toujours strictement positif et fini (`sample_rate >= 8000.0`).
  - `reset()` remet `bypass_wet = 1.0` par defaut dans `StereoPunchDsp`, et un helper `sync_bypass_state(bypass: bool)` permet a `OmorePunch::initialize()` et `OmorePunch::reset()` d'aligner immediatement `bypass_wet` sur `params.bypass.value()` (`0.0` si une session sauvegardee demarre deja en bypass).

## 3. Machine d'Etats & Transitions du Soft Bypass

```text
[ACTIVE: cfg.bypass=false, bypass_wet=1.0]
   │
   │ (cfg.bypass passe a true)
   ▼
[RAMP_TO_BYPASS: 0.0 < bypass_wet < 1.0]
   │ - Gain Match gele (gm_in_ms, gm_post_ms, gm_presence_ms, gm_gain_db inchanges)
   │ - Sortie = mix lineaire (1 - bypass_wet)*dry + bypass_wet*processed, sanitisee
   │ - gr_db = 0.0, clipped = false (aucun nouveau clip ni Max GR enregistre)
   │
   │ (bypass_wet atteint 0.0 en <= 10 ms)
   ▼
[BYPASSED: cfg.bypass=true, bypass_wet=0.0]
   │ - Sortie = (sanitize(raw_l), sanitize(raw_r)) (bit-transparent dans [-4.0, 4.0])
   │ - Etats transitoires (env_gr_db, auto_slow_gr_db, sc_prev_*, biquads EQ) a 0.0
   │ - Memoires Gain Match (gm_*) conservees/figees intactes
   │ - gr_db = 0.0, clipped = false
   │
   │ (cfg.bypass passe a false)
   ▼
[RAMP_TO_ACTIVE: 0.0 < bypass_wet < 1.0]
   │ - Chaine de traitement reactivee (avec gm_gain_db pre-existant conserve)
   │ - Sortie = mix lineaire (1 - bypass_wet)*dry + bypass_wet*processed, sanitisee
   │ - clipped et gr_db reprennent progressivement (ou des sortie de rampe)
   │
   │ (bypass_wet atteint 1.0 en <= 10 ms)
   ▼
[ACTIVE: cfg.bypass=false, bypass_wet=1.0]
```

## 4. Telemetrie & Interface (`MeterTelemetry`, `draw_omore_punch_ui`)

- `MeterTelemetry` n'a pas besoin de nouveau champ atomique pour l'etat bypass lui-meme (l'UI lit `params.bypass.value()` directement via `Arc<OmorePunchParams>`).
- Comportement des champs existants dans `OmorePunch::process()`:
  - `in_peak_l` / `in_peak_r` : continuent a refleter le signal d'entree (`clean_in_l/r`).
  - `out_peak_l` / `out_peak_r` : continuent a refleter le signal de sortie reel (`out.out_l/r`), donc identiques aux niveaux d'entree en bypass etabli.
  - `current_gr_db` : mis a `0.0` lorsque `bypass == true` (ce qui entraine `GR: -00.0 dB`, jauge verticale `GR` a `0 dB` et `current_gain_adjustment_db()` a `0.0 dB` cote DAW).
  - `max_gr_db` : non mis a jour lorsque `bypass == true` (conserve la valeur crête precedente jusqu'a `[Reset Peak]`).
  - `clip_latched` : non arme lorsque `bypass == true` (conserve un eventuel etat `true` preexistant jusqu'au clic d'acquittement).
  - `gain_match_db` : conserve la valeur figee `self.dsp.gm_gain_db`.
