# Architecture Technique — Omore Punch

## 1. Vue d'Ensemble Monolithique
L'intégralité du code source d'`Omore Punch` réside dans `src/lib.rs` afin de simplifier l'audit temps réel, les tests unitaires et la maintenance assistée par IA.

Le fichier est structuré en quatre blocs isolés :
1. **`StereoPunchDsp` & `DspConfig` :** Moteur DSP pur, sans dépendance à l'interface ni au wrapper hôte.
2. **`MeterTelemetry` :** Structure de communication inter-threads non bloquante basée sur `AtomicU32` (conversions `f32::to_bits` / `f32::from_bits`) et `AtomicBool` avec `Ordering::Relaxed`.
3. **`OmorePunchParams` & `OmorePunch` :** Implémentation des traits `Plugin`, `ClapPlugin` et `Vst3Plugin` de `nih-plug`.
4. **`draw_omore_punch_ui` :** Interface graphique `egui` à 60 FPS avec jauge de réduction de gain descendante (`0.0 dB` vers `-24.0 dB`).

## 2. Garanties Temps Réel (`process`)
- **Zéro allocation mémoire :** La fonctionnalité `assert_process_allocs` de `nih-plug` est activée pour garantir qu'aucune allocation sur le tas (`Vec`, `String`, `Box`, `format!`) n'a lieu dans le thread audio.
- **Zéro verrou bloquant :** Aucune primitive `Mutex` ou `RwLock` n'est utilisée entre le thread audio et l'UI.
- **Précalcul des coefficients :** Les coefficients balistiques (`attack_coeff`, `release_coeff`) et le coefficient du filtre passe-haut de Sidechain (`sc_hpf_coeff`) ne sont recalculés que si `sample_rate`, `attack_ms`, `release_ms` ou `sc_hpf_hz` a varié.
- **Protection Anti Auto-Mute Reaper :** `sanitize_sample()` filtre systématiquement les `NaN` et `Inf` à l'entrée et à la sortie (ainsi que dans l'état du filtre Sidechain) et applique un clamp matériel strict dans `[-4.0, 4.0]` (~`+12 dBFS`).

## 3. Chaîne de Traitement Audio (Ordre des Opérations)
```text
Entrée Stéréo (L/R)
   |
   +---> [Sanitize [-4.0, 4.0]] --------------------------> Signal DRY (L/R)
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
            |     [4. Enveloppe Attack (3ms) / Release (80ms)]     |
            |              |                                       |
            |              v                                       |
            |     [5. Auto-Makeup (35%) + Makeup Manuel]           |
            |              |                                       |
            +<-------------+ (Gain appliqué au signal plein spectre)
            |                                                      |
            v                                                      |
   [6. Clipper Final (Soft Clip / Hard Clip / Bypass @ Ceiling)]   |
            |                                                      |
            v                                                      v
       Signal WET (L/R) ------------------------------> [7. Mix Parallèle Dry/Wet]
                                                                   |
                                                                   v
                                                   [Sanitize & Safety Clamp [-4, 4]]
                                                                   |
                                                                   v
                                                          Sortie Stéréo (L/R)
```