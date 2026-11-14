# UI & Host Parameter Contract: Bouton rapide `[Snare Safe 200Hz]`

**Feature**: `001-snare-safe-200hz`
**Date**: 2026-11-14

## 1. Contrat Visuel (`egui`)
- **Emplacement** : Colonne gauche (`1. COMPRESSOR & ENVELOPE BALLISTICS`), barre horizontale des boutons d'action rapide située sous la ligne du paramètre `SC HPF`.
- **Ordre des boutons sur la ligne** :
  1. `[Punch 3ms / 80ms]`
  2. `[Snare Safe 200Hz]`
  3. `[Auto Makeup: ON]` / `[Auto Makeup: OFF]`
- **Style typographique** : `egui::RichText::new("[Snare Safe 200Hz]").monospace().strong()`
- **Infobulle au survol (`on_hover_text`)** :
  `"Set Sidechain HPF to 200.0 Hz so Kick and Snare body (up to 200 Hz) do not pump the Drum Bus"`
- **Contrainte d'encodage** : 100 % ASCII strict (`0x20..=0x7E`).

## 2. Contrat d'Interaction Hôte VST3 / CLAP (`ParamSetter`)
Lorsqu'un clic (`clicked()`) est détecté sur `[Snare Safe 200Hz]` :
1. Appeler `setter.begin_set_parameter(&params.sc_hpf_hz)`
2. Appeler `setter.set_parameter(&params.sc_hpf_hz, 200.0)`
3. Appeler `setter.end_set_parameter(&params.sc_hpf_hz)`

## 3. Contrat de Rétrocompatibilité
- L'identifiant de paramètre exposé à l'hôte reste `"sc_hpf_hz"`.
- La plage de normalisation `[20.0, 250.0]` et le facteur d'asymétrie (`skew_factor(-1.0)`) restent inchangés afin que toute courbe d'automation ou valeur sauvegardée dans une session Reaper existante conserve exactement la même fréquence en Hz.
