# Omore Punch

**Omore Punch** (`omore-punch`) est un plugin audio stéréo haute performance de compression parallèle et de clipping dédié au **Bus Batterie (Drum Bus)**, écrit en Rust avec `nih-plug` et `nih_plug_egui`.

Il extrait et adapte en bus stéréo lié l'algorithme de dynamique "Punch" et l'étage de saturation/clipping d'**Omore Sampler**, avec une compatibilité 100 % native pour **Reaper** sous **Linux (Debian 13)** et **Windows 11**.

## Fonctionnalités Clés

- **Formats exportés :** `VST3` et `CLAP`.
- **Détecteur de Crête Stéréo Lié (Stereo-Linked Peak Detector) :** Conserve une image stéréo parfaitement stable pour la caisse claire, la grosse caisse et les overheads.
- **Filtre Passe-Haut de Sidechain Interne (`SC HPF` de `20 Hz` à `250 Hz`) :** Filtre 6 dB/oct dédié uniquement au détecteur avec bouton rapide `[Kick Safe 90Hz]` pour éviter que le sub de la grosse caisse ne fasse pomper le bus, sans aucune perte de grave dans le signal audio.
- **Balistique "Omore Punch" :** Pré-réglage signature rapide (`Attack = 3.0 ms`, `Release = 80.0 ms`) accessible en un clic via `[Punch 3ms / 80ms]`.
- **Compensation Auto-Makeup (35 %) :** Redonne instantanément du corps dès que l'on abaisse le `Threshold`, complétée par un `Makeup Gain` manuel (`-12 dB` à `+24 dB`).
- **Étage Clipper Final :** 3 modes (`Soft Clip` cubique/tanh $C^1$ sans perte à faible niveau, `Hard Clip`, ou `Bypass`) avec plafond `Clip Ceiling` ajustable de `-6.0 dB` à `0.0 dB`.
- **Mix Parallèle (Dry/Wet) :** Dosage direct de `0 %` (100 % Dry au bit près) à `100 %` (100 % Wet) en phase.
- **Interface 100 % ASCII :** Zéro emoji et zéro caractère Unicode exotique, garantissant un rendu sans carré vide sous Debian Linux comme sous Windows 11.

## Compilation Locale (Debian / Windows)

```bash
# 1. Lancer la suite de tests unitaires DSP
cargo test --release

# 2. Générer les bundles .vst3 et .clap
cargo run --package xtask --release -- bundle omore-punch --release