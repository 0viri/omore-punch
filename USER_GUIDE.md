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
2. Clique sur le bouton **`[Punch 3ms / 80ms]`** pour verrouiller l'attaque de `3.0 ms` (laisse passer le "crack" de la caisse claire et le clic de la grosse caisse), le relâchement de `80.0 ms` (fait respirer le bus entre chaque coup) et repasser **`Release Mode`** sur **`[Manual]`** ; ou sélectionne un des 3 profils adaptatifs **`Release Mode`** (double étage style SSL) selon le morceau :
   - **`[Auto Tight]`** (`50 / 600 ms`) : récupération nerveuse sur les coups isolés et tenue maîtrisée lors des passages rapides (double pédale / blast beats).
   - **`[Auto Punch]`** (`80 / 800 ms`) : équilibre polyvalent pour batterie rock/metal dynamique avec sustain de cymbales et overheads naturels.
   - **`[Auto Glue]`** (`100 / 1200 ms`) : relâchement lent progressif pour coller le bus batterie sur les grooves plus ouverts.
   *(En mode Auto, le potentiomètre **`Release`** s'atténue visuellement avec la mention **`AUTO (...)`** pour indiquer qu'il est outrepassé, tout en restant éditable afin de pré-régler un retour vers **`[Manual]`**.)*
3. Clique sur le bouton **`[Snare Safe 200Hz]`** (ou règle **`SC HPF`** jusqu'à `200 Hz`, selon la technique de Cristian Kohle) pour filtrer le sous-grave de la grosse caisse et le corps fondamental de la caisse claire uniquement dans le détecteur du compresseur : le bus ne pompera plus lors des coups de double pédale ou des impacts lourds de caisse claire, tout en conservant 100 % des graves et du corps dans le signal audio.
4. Vérifie que **`[Auto Makeup: ON]`** est activé.
5. Abaisse progressivement le **`Threshold`** jusqu'à lire une réduction de gain (`GR`) comprise entre **`-4.0 dB` et `-8.0 dB`** sur les impacts de caisse claire dans la grande jauge jaune descendante de la section **`4. GAIN REDUCTION & STEREO BUS METERING`**.
6. Façonne le bus dans la section **`3. DRUM BUS EQ`** (placée après le compresseur et juste avant le clipper) :
   - **`Low Cut`** (`20.0 Hz` à `160.0 Hz`, bypass en butée gauche `<= 20.5 Hz`) : nettoie l'infra-grave inutile sous la grosse caisse sans perturber le détecteur du compresseur.
   - **`500Hz Cut`** (`0.0 dB` à `-12.0 dB`) : creuse le bas-médium "carton / boîte" autour de `500 Hz` (ex. `-2.0 dB` à `-4.0 dB`) pour ouvrir le kit.
   - **`High Shelf`** (`-6.0 dB` à `+12.0 dB` à `10 kHz`) : ajoute de l'air sur les cymbales et de l'attaque de baguette (ou calme un haut brillant en négatif) ; les crêtes générées par un boost sont rattrapées proprement par le Clipper en aval.
7. Sélectionne **`[Soft Clip]`** et règle **`Clip Ceiling`** sur **`-0.5 dB`** ou **`-1.0 dB`** pour raboter musicalement les transitoires résiduelles sans faire saturer le convertisseur ou le bus Master.
8. Ajuste **`Mix Dry/Wet`** entre **`40 %` et `70 %`** pour doser la compression parallèle directement dans le plugin sans créer de piste auxiliaire supplémentaire.

## 3. Contrôles de Précision
Chaque paramètre continu (compresseur, clipper, mix et les 3 bandes de **`3. DRUM BUS EQ`**) est présenté sous forme de **potentiomètre rotatif style SSL** disposant :
- d'un réglage fluide par **glisser vertical** à la souris sur le bouton rond (maintenir **`Shift`** pour un réglage 4x plus fin),
- d'un **double-clic** sur le bouton rond ou d'un clic sur **`Reset`** pour rétablir immédiatement sa valeur par défaut,
- de petits boutons compacts **`-`** et **`+`** sous chaque potentiomètre pour incrémenter ou décrémenter par pas exacts sans saut de souris,
- d'un bouton global **`[Reset Peak]`** remettant à zéro le compteur **`Max GR`** ainsi que le témoin **`[CLIP]`**.