# Architecture Technique — Omore Punch

## 1. Vue d'Ensemble Monolithique
L'intégralité du code source d'`Omore Punch` réside dans `src/lib.rs` afin de simplifier l'audit temps réel, les tests unitaires et la maintenance assistée par IA.

Le fichier est structuré en blocs logiques isolés :
1. **`StereoPunchDsp` & `DspConfig` :** Moteur DSP pur, sans dépendance à l'interface ni au wrapper hôte.
2. **`MeterTelemetry` :** Structure de communication inter-threads non bloquante (thread audio -> UI & metering hôte DAW) basée sur `AtomicU32` (conversions `f32::to_bits` / `f32::from_bits`) et `AtomicBool` avec `Ordering::Relaxed`.
3. **`OmorePunchParams`, `sanitize_plugin_state`, `PeakMeterBallistics` & `OmorePunch` :** Paramètres DAW, rétrocompatibilité/sanitisation d'état (`Plugin::filter_state`), balistique de vu-mètres précalculée (`85 ms`) et implémentation des traits `Plugin`, `ClapPlugin` et `Vst3Plugin` de `nih-plug`.
4. **`draw_omore_punch_ui` :** Interface graphique `egui` (`900x650` px avec `ScrollArea` de secours) à 60 FPS, avec badge `[DEV <hash>]` sur les builds `-dev`, bouton `[BYPASS: OFF/ON]`, profils `Release Mode:`, `[Gain Match: OFF/ON]` + `Trim`, et jauge de réduction de gain descendante (`0.0 dB` vers `-24.0 dB`).
5. **`mod clap` & `mod vst3` :** Proxys locaux C/COM autour des wrappers `nih-plug` exposant le metering de réduction de gain natif DAW (`clap.gain-adjustment-metering/0` et `PreSonus::IGainReductionInfo`) ainsi que la resynchronisation explicite de l'hôte après chargement d'état (`clap_host_params::rescan` / `IComponentHandler::restart_component`).
6. **`mod tests` :** Suite de tests unitaires DSP, UI, sanitisation d'état et ABI VST3/CLAP (complétée par les tests de `xtask/src/main.rs` via `cargo test --workspace --release`).

## 2. Garanties Temps Réel (`process`)
- **Zéro allocation mémoire :** La fonctionnalité `assert_process_allocs` de `nih-plug` est activée pour garantir qu'aucune allocation sur le tas (`Vec`, `String`, `Box`, `format!`) n'a lieu dans le thread audio.
- **Zéro verrou bloquant :** Aucune primitive `Mutex` ou `RwLock` n'est utilisée dans le thread audio (`process()`, `StereoPunchDsp`, `PeakMeterBallistics`) ni dans les callbacks de lecture de télémétrie hôte.
- **Précalcul des coefficients :** Les coefficients balistiques (`attack_coeff`, `release_coeff`, auto-release dual-stage), le coefficient du filtre passe-haut de Sidechain (`sc_hpf_coeff`), les coefficients biquads de l'EQ 3 bandes (`eq_low_cut`, `eq_mid_cut`, `eq_high_shelf`), les constantes `Gain Match`, la rampe `Bypass` (`10 ms`) et la décroissance `PeakMeterBallistics` (`85 ms`) sont précalculés hors `process()` ou mis en cache et invalidés uniformément via `apply_sample_rate_change()`.
- **Protection Anti Auto-Mute Reaper :** `flush_denormal()` (`1.0e-20`) et `sanitize_sample()` filtrent systématiquement les `NaN`, `Inf` et dénormaux à l'entrée, à la sortie et sur tous les états internes (biquads, HPF sidechain, enveloppes, Gain Match, vu-mètres), avec un clamp matériel strict dans `[-4.0, 4.0]` (~`+12 dBFS`).

## 3. Chaîne de Traitement Audio (Ordre des Opérations)
```text
Entrée Stéréo (L/R)
   |
   +---> [Sanitize & Denormal Flush [-4.0, 4.0]] ----------> Signal DRY (L/R)
            |                                                      |
            v                                                      |
   [1. Input Trim (-12..+12 dB)]                                   |
            |                                                      |
            +---> [1b. Sidechain HPF 1er Ordre (20..250 Hz)]       |
            |              |   (Chemin détection uniquement)       |
            |              v                                       |
            |     [2. Détecteur Crête Stéréo max(|L|, |R|)]        |
            |              |                                       |
            |              v                                       |
            |     [3. Calcul Cible GR (Threshold / Ratio)]         |
            |              |                                       |
            |              v                                       |
            |     [4. Enveloppe Attack / Release]                  |
            |              |   (Manual ou Auto-Release Dual-Stage :|
            |              |    Auto Tight / Auto Punch / Auto Glue)|
            |              v                                       |
            |     [5. Auto-Makeup (35%) + Makeup Manuel]           |
            |              |                                       |
            +<-------------+ (Gain appliqué au signal plein spectre)
            |                                                      |
            v                                                      |
   [5b. Drum Bus EQ (Low Cut / 500Hz Cut / High Shelf)]            |
            |                                                      |
            v                                                      |
   [6. Clipper Final (Soft Clip / Hard Clip / Bypass @ Ceiling)]   |
            |                                                      |
            v                                                      v
       Signal WET (L/R) ------------------------------> [7. Mix Parallèle Dry/Wet]
                                                                   |
                                                                   v
                                              [8. Gain Match RMS 400ms (Hold -60 dBFS)]
                                                                   |
                                                                   v
                                              [9. Soft-Bypass Crossfade (10 ms)]
                                                                   |
                                                                   v
                                              [Sanitize & Safety Clamp [-4.0, 4.0]]
                                                                   |
                                                                   v
                                                          Sortie Stéréo (L/R)
```
