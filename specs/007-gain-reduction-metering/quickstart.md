# Quickstart Validation Guide: DAW Gain Reduction Metering

**Feature**: `007-gain-reduction-metering`
**Date**: 2026-11-15

## 1. Contrôles automatisés (Linux & Windows)

1. **Contrôle ASCII strict (`src/lib.rs`)** :
   ```bash
   LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs
   ```
   Attendu : aucune sortie (code de sortie `1`).

2. **Tests unitaires DSP & contrats ABI en Release** :
   ```bash
   cargo test --release
   ```
   Attendu : tous les tests passent, y compris les nouveaux tests vérifiant :
   - la mesure de réduction de gain hôte (`0.0` au repos/silence/non-fini, négative jusqu'à `-24.0` en compression) ;
   - l'indépendance vis-à-vis du make-up gain, du clipper et du `Mix` parallèle ;
   - le retour à `0.0` lors de `reset()` et sur les taux d'échantillonnage `44100..192000 Hz` ;
   - l'exposition effective des interfaces CLAP (`clap.gain-adjustment-metering/0`) et VST3 (`IGainReductionInfo` via `query_interface`) et l'absence de paramètre de metering dans `OmorePunchParams`.

3. **Compilation des bundles VST3 et CLAP** :
   ```bash
   cargo run --package xtask --release -- bundle omore-punch --release
   ```
   Attendu : génération de `target/bundled/omore-punch.vst3` et `target/bundled/omore-punch.clap`.

## 2. Validation manuelle dans Reaper (VST3 et CLAP)

1. Installer les bundles générés (`./build.sh` sous Linux ou `.\build.ps1` sous Windows) et rescanner les plugins dans Reaper.
2. Charger **Omore Punch** (tester successivement le format **VST3** puis le format **CLAP**) sur une piste ou un bus batterie.
3. **Vérifier SC-001 (Jauge console)** : sur la tranche MCP/TCP de Reaper, la jauge jaune descendante de réduction de gain apparaît automatiquement à droite des vumètres de niveau.
4. **Vérifier SC-004 (Absence de paramètre utilisateur)** : ouvrir la liste des paramètres / enveloppes d'automation (`Env`) du plugin dans Reaper et confirmer qu'aucun paramètre de réduction de gain n'apparaît.
5. **Vérifier SC-003 (Correspondance au dB près & fenêtre fermée)** :
   - Envoyer une boucle de batterie dépassant le Threshold (`Ratio 4:1`, différents réglages Attack / Release / Auto-Release, Make-up gain et Clipper actifs).
   - Comparer la descente de la barre jaune Reaper avec le vumètre interne `GR (0 to -24 dB)` : écart maximal < 1 dB.
   - Baisser `Mix` à `30 %` : les deux jauges continuent d'afficher la même réduction brute du compresseur.
   - Fermer la fenêtre du plugin : la jauge Reaper continue de réagir en temps réel.
6. **Vérifier SC-002 (Silence & Bypass)** :
   - Couper la lecture audio ou passer le plugin en Bypass FX dans Reaper : la barre jaune revient proprement à `0.0 dB` (complètement rétractée en haut).
