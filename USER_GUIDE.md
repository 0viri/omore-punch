# Guide Utilisateur — Omore Punch (Reaper Linux & Windows)

## 1. Installation des Plugins
Copie les bundles générés depuis `target/bundled/` vers ton dossier de plugins Reaper :
- **Sous Debian 13 (Linux) :**
  - CLAP : `~/.clap/omore-punch.clap`
  - VST3 : `~/.vst3/omore-punch.vst3`
- **Sous Windows 11 :**
  - CLAP : `C:\Program Files\Common Files\CLAP\omore-punch.clap`
  - VST3 : `C:\Program Files\Common Files\VST3\omore-punch.vst3`

## 2. Réglage Rapide pour un Bus Batterie Metal / Rock
1. Insère **Omore Punch** sur ta piste dossier/bus `DRUM BUS` dans Reaper.
2. Clique sur le bouton **`[Punch 3ms / 80ms]`** pour verrouiller l'attaque de `3.0 ms` (laisse passer le "crack" de la caisse claire et le clic de la grosse caisse) et le relâchement de `80.0 ms` (fait respirer le bus entre chaque coup).
3. Clique sur le bouton **`[Kick Safe 90Hz]`** (ou règle **`SC HPF`** entre `80 Hz` et `120 Hz`) pour filtrer l'énergie sub-grave de la grosse caisse uniquement dans le détecteur du compresseur : le bus ne pompera plus sur les cymbales lors des coups de double pédale, tout en conservant 100 % des sub-basses dans le signal audio.
4. Vérifie que **`[Auto Makeup: ON]`** est activé.
5. Abaisse progressivement le **`Threshold`** jusqu'à lire une réduction de gain (`GR`) comprise entre **`-4.0 dB` et `-8.0 dB`** sur les impacts de caisse claire dans la grande jauge jaune descendante.
6. Sélectionne **`[Soft Clip]`** et règle **`Clip Ceiling`** sur **`-0.5 dB`** ou **`-1.0 dB`** pour raboter musicalement les transitoires résiduelles sans faire saturer le convertisseur ou le bus Master.
7. Ajuste **`Mix Dry/Wet`** entre **`40 %` et `70 %`** pour doser la compression parallèle directement dans le plugin sans créer de piste auxiliaire supplémentaire.

## 3. Contrôles de Précision
Chaque paramètre dispose :
- de boutons **`-`** et **`+`** pour incrémenter par pas fins sans saut de souris,
- d'un **`Slider`** glissant,
- d'un bouton **`Reset`** individuel ramenant immédiatement le paramètre à sa valeur nominale,
- d'un bouton **`[Reset Peak]`** remettant à zéro le compteur **`Max GR`** ainsi que le témoin **`[CLIP]`**.