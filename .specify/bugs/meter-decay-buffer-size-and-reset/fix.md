# Bug Fix: Decroissance des vu-metres dependante de la taille de buffer et remise a zero incomplete de MeterTelemetry

- **Slug**: meter-decay-buffer-size-and-reset
- **Fixed**: 2026-11-18 (revision 2 + nettoyage post-review)
- **Assessment**: ./assessment.md
- **Status**: applied

## Summary

Remplacement du facteur fixe `* 0.88` par bloc dans `OmorePunch::process()` par une balistique de vu-metre par echantillon (`PeakMeterBallistics`, `METER_PEAK_RELEASE_MS = 85.0`, coefficient precalcule hors de `process()`, flush anti-denormaux). Chaque bloc publie le **maximum de l'enveloppe atteint pendant le bloc** : la redescente est invariante en taille de buffer et en `sample_rate`, et une transitoire n'est jamais sous-affichee. Ajout de `MeterTelemetry::clear_live_meters()` / `clear_lifecycle_telemetry()` branchees sur tous les evenements de cycle de vie `Plugin`, CLAP et VST3, sans alterer `reset_peaks()` ni `clear_gain_reduction()`.

## Revision 2 & Nettoyage Post-Review

| Constat des reviews | Correction |
|---|---|
| **Major** : seule la valeur de fin de bloc de l'integrateur etait publiee ; une transitoire en debut de bloc etait affichee trop bas (-4,4 dB a 2048 frames / 48 kHz, -1,1 dB a 512). | `process()` publie desormais `meter_block_max` (maximum de l'enveloppe sur le bloc, alimente par `PeakMeterBallistics::tick`). Un bloc vide republie l'enveloppe courante. |
| Etat de l'integrateur stocke dans les atomiques de telemetrie. | Etat deplace dans `OmorePunch::meters` (`PeakMeterBallistics`, `#[derive(Clone, Debug)]`, sans `Copy`). Les atomiques ne servent plus qu'a l'affichage ; `nih-plug` appelle deja `Plugin::reset()` au redemarrage du traitement (`start_processing` / `set_processing(true)`) et apres chargement d'etat actif, ce qui remet `self.meters` a zero sans drapeau atomique supplementaire. |
| `clap_state_load` et `vst3::set_state` non testes. | `test_clap_wrapper_state_roundtrip_and_host_sync` et `test_vst3_proxy_state_roundtrip_and_host_sync` amorcent maintenant `in_peak_*`, `out_peak_*` et `gain_match_db` et verifient leur remise a `0.0` apres chargement d'etat. |
| Test de silence fragile (dependance implicite a la constante de 85 ms). | Borne de flush derivee des constantes : `tau * ln(SAFETY_CLAMP_LIMIT / DENORMAL_FLUSH_THRESHOLD) * 1.01`, avec verification d'absence de subnormal a chaque bloc. |
| Calcul du coefficient duplique dans `Default` et `initialize()` + garde `sample_rate` redondante. | Centralise dans `PeakMeterBallistics::new()` / `set_sample_rate()`, recevant directement `self.dsp.sample_rate` deja sanitise par `StereoPunchDsp::sanitize_sample_rate()`. |

## Changes

| File | Change | Notes |
|------|--------|-------|
| `src/lib.rs` | modified | `METER_PEAK_RELEASE_MS` (L30) ; `MeterTelemetry::clear_live_meters()` (L1014), `clear_lifecycle_telemetry()` (L1025) ; `PeakMeterBallistics` (L1443, `tick` L1472) ; `OmorePunch::meters`, `Default`, `initialize()`, `reset()` ; `process()` (publication du pic de bloc L1662) ; hooks `mod clap` / `mod vst3` |
| `src/lib.rs` | added tests | `test_peak_meter_ballistics_block_peak_and_segmentation_invariance` (L8796), `test_meter_decay_buffer_size_invariance_and_lifecycle_reset` (L8877) |
| `src/lib.rs` | updated tests | `test_clap_wrapper_state_roundtrip_and_host_sync` et `test_vst3_proxy_state_roundtrip_and_host_sync` (L8195, L8360) |
| `CHANGELOG.md` | modified | Entree sous `## [Unreleased]` / `### Corrigé` |

## Diff Highlights

```rust
#[derive(Clone, Debug)]
struct PeakMeterBallistics {
    decay_coeff: f32,
    env: [f32; 4],
}

#[inline(always)]
fn tick(&mut self, values: [f32; 4], block_max: &mut [f32; 4]) {
    for idx in 0..4 {
        let next = flush_denormal((self.env[idx] * self.decay_coeff).max(values[idx].abs()));
        self.env[idx] = next;
        block_max[idx] = block_max[idx].max(next);
    }
}
```

```rust
// OmorePunch::process()
self.meters.tick([clean_in_l, clean_in_r, out.out_l, out.out_r], &mut meter_block_max);
...
let meter_display = if meter_frames_seen { meter_block_max } else { self.meters.env };
```

## Tests Added or Updated

- `test_peak_meter_ballistics_block_peak_and_segmentation_invariance` — a 44.1 / 48 / 96 / 192 kHz, pour des decoupages de 1, 32, 64, 512, 2048 et mixtes : valeur publiee = pic exact de l'enveloppe de reference sur chaque bloc et jamais inferieure au pic brut du bloc (transitoires en debut, milieu et fin de bloc) ; enveloppe interne identique au bit pres quel que soit le decoupage ; calibrage `amp / e` apres une constante de temps ; flush vers `+0.0` sans subnormal dans une borne derivee des constantes.
- `test_meter_decay_buffer_size_invariance_and_lifecycle_reset` (bout en bout via le wrapper CLAP) — transitoire en debut de premier bloc affichee a pleine valeur (64 / 512 / 2048 / mixte, 48 et 96 kHz) ; valeur publiee identique au bit pres entre decoupages et egale a la reference ; transitoire au milieu d'un bloc de 2048 affichee a pleine valeur ; flush apres silence ; semantique `reset_peaks()` / `clear_live_meters()` / `clear_lifecycle_telemetry()`, `OmorePunch::reset()` / `initialize()`, CLAP (`stop_processing` + relance `start_processing`, `reset`, `deactivate`) et VST3 (`set_processing(0)`, `set_active(0)`, `terminate`).
- `test_clap_wrapper_state_roundtrip_and_host_sync` / `test_vst3_proxy_state_roundtrip_and_host_sync` — remise a `0.0` de `in_peak_*`, `out_peak_*`, `gain_match_db` apres `clap_state_load` / `vst3::set_state`.

## Local Verification

- `LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs || echo "ASCII_OK"` → `ASCII_OK`
- `cargo test --workspace --release` → `43 passed; 0 failed` (`omore-punch`) + `2 passed; 0 failed` (`xtask`), sans warning
- `cargo run --package xtask --release -- bundle omore-punch --release` → `Omore Punch.clap`, `Omore Punch.vst3` et `moduleinfo.json` generes
- Test de mutation (realise en revision 2) : en reintroduisant temporairement la publication de l'enveloppe de fin de bloc, `test_meter_decay_buffer_size_invariance_and_lifecycle_reset` echoue (`sr=48000 [fixed_64]`, premier bloc sous-affiche).
- Relecture temps reel (regles 3.1 a 3.4 de `GEMINI.md`) : zero allocation, zero verrou, zero `.exp()` dans `process()`, flush anti-denormaux par echantillon sur l'etat de balistique.

## Deviations from Assessment

- L'assessment proposait de publier l'integrateur une seule fois en fin de bloc ; le review a montre que cela sous-affiche les transitoires. La publication porte desormais sur le maximum de l'enveloppe atteint pendant le bloc (meme constante de temps, meme invariance de la redescente).
- L'etat de balistique est conserve dans `OmorePunch::meters` (`PeakMeterBallistics`) plutot que relu depuis les atomiques de telemetrie, ces derniers ne servant qu'a la publication vers l'UI ; `OmorePunch::initialize()` et `OmorePunch::reset()` (appele par `nih-plug` au redemarrage du traitement et apres chargement d'etat actif) reinitialisent `self.meters`.

## Follow-ups

- Controle visuel dans Reaper avec plusieurs tailles de buffer (`./build.sh` / `build.ps1`) avant ou pendant la PR.
