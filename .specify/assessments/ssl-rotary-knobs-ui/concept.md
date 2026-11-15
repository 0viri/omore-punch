# Concept: Potentiomètres rotatifs en grille et esthétique type SSL

- **Slug**: ssl-rotary-knobs-ui
- **Created**: 2026-11-14
- **Recommended option**: Option A — Potentiomètres rotatifs en grille type SSL dans la structure 2 colonnes actuelle

## Options

### Option A — Potentiomètres rotatifs en grille type SSL dans la structure 2 colonnes actuelle
- **Sketch**: Remplacer les 9 curseurs horizontaux empilés de la colonne de gauche par des potentiomètres rotatifs (« boutons ronds ») disposés en grille compacte au sein des deux sections existantes (`1. COMPRESSOR & ENVELOPE BALLISTICS` et `2. OUTPUT CLIPPER & PARALLEL MIX`). Chaque bouton rond adopte les codes visuels type SSL (couronne graduée, capuchon coloré par famille fonctionnelle, repère angulaire net) accompagné de son nom, de sa valeur numérique ASCII et d'une interaction directe de réglage fin et de remise à zéro, tandis que le bandeau supérieur, les boutons d'action rapide entre crochets et la colonne droite de metering (`In L/R`, `Out L/R`, `GR` descendant style Reaper) sont préservés dans la fenêtre de `820x480` px.
- **Appetite**: small
- **Trade-offs**:
  - *Avantages* : répond directement au besoin d'ergonomie rotative et d'identité visuelle type SSL sur les contrôles ; libère de l'espace horizontal dans la colonne gauche (éliminant les risques de collision avec la colonne de droite) ; conserve l'efficacité des bargraphes verticaux actuels pour surveiller le clipper et la réduction de gain sur un bus batterie ; limite l'amendement constitutionnel à la présentation des contrôles.
  - *Inconvénients & risques* : rendu hybride assumé (boutons rotatifs style SSL à gauche, vu-mètres verticaux style Reaper à droite) ; demande un soin particulier sur le ressenti du glisser souris et l'accès au réglage fin pas-à-pas.
- **Rabbit holes**: Chercher à imiter du photoréalisme 3D complexe (ombres portées multi-couches, textures métalliques) ou multiplier les modes d'interaction souris (glisser angulaire polaire vs glisser vertical standard).

### Option B — Refonte visuelle intégrale façon rack matériel SSL (Façade, boutons ronds et VU-mètre à aiguille)
- **Sketch**: Transformer l'intégralité de la fenêtre du plugin en une tranche/rack matériel inspiré du SSL G-Master Bus Compressor : façade gris clair/anthracite, potentiomètres rotatifs à capuchons colorés, remplacement des boutons texte entre crochets par des boutons-poussoirs lumineux rectangulaires, et remplacement (ou doublage) de la jauge verticale `GR` par un cadran analogique à aiguille.
- **Appetite**: medium
- **Trade-offs**:
  - *Avantages* : fidélité esthétique maximale au matériel SSL sur toute la surface du plugin.
  - *Inconvénients & risques* : remet en question toute la charte graphique actuelle (`GEMINI.md` section 4 et `.specify/memory/constitution.md`) ; un cadran à aiguille est moins lisible pour repérer simultanément les crêtes rapides `In L/R`, `Out L/R` et le plafond strict du clipper ; augmente fortement la taille et la complexité du bloc UI dans le fichier unique `src/lib.rs`.
- **Rabbit holes**: Conception d'un galvanomètre à aiguille avec balistique visuelle propre, refonte complète des vu-mètres d'entrée/sortie et dérive vers du skeuomorphisme coûteux à maintenir en pur dessin vectoriel.

### Option C — Statu quo : conserver les curseurs horizontaux actuels
- **Sketch**: Maintenir l'interface actuelle à 9 lignes de curseurs horizontaux (`draw_param_row`) avec boutons `-`, `+` et `Reset`, telle que stabilisée dans la fenêtre `820x480` px après la correction du chevauchement des colonnes.
- **Appetite**: small
- **Trade-offs**:
  - *Avantages* : effort nul, zéro modification de la constitution ou des tests de mise en page, conservation telle quelle des boutons `-`/`+`/`Reset` par ligne.
  - *Inconvénients & risques* : ne résout pas le problème soulevé par l'utilisateur (absence de repérage angulaire et aspect « formulaire de faders horizontaux » éloigné d'un compresseur de studio).
- **Rabbit holes**: Aucun.

## Recommendation

**Option A — Potentiomètres rotatifs en grille type SSL dans la structure 2 colonnes actuelle.**

Cette option satisfait pleinement les trois objectifs définis dans `problem.md` :
1. Elle apporte immédiatement le repérage angulaire et le look type SSL demandés en remplaçant les 9 faders horizontaux par des boutons ronds regroupés spatialement.
2. En passant d'un empilement de 9 larges lignes (~442 px chacune) à une grille de boutons ronds compacts, elle supprime toute tension horizontale dans la fenêtre `820x480` px.
3. Elle préserve la lisibilité 100 % ASCII, les boutons de préréglages rapides (`[Punch 3ms / 80ms]`, `[Snare Safe 200Hz]`, `[Auto Makeup: ON]`) et l'efficacité du metering vertical (`In L/R`, `Out L/R`, `GR` descendant style Reaper), tout en restant dans un budget `small` adapté à l'architecture monolithique de `src/lib.rs`.

## Out of Scope (for the recommended option)

- Toute modification du moteur DSP (`StereoPunchDsp`), des plages de paramètres ou des identifiants d'automation VST3/CLAP.
- Le remplacement des bargraphes verticaux de la colonne de droite (`In L/R`, `Out L/R`, jauge `GR` descendante style Reaper) par un VU-mètre analogique à aiguille.
- L'utilisation d'images bitmap externes, de polices personnalisées ou de caractères Unicode non-ASCII.
- Le découpage de `src/lib.rs` en plusieurs fichiers.

## Assumptions to Validate

- L'utilisateur préfère un glisser vertical standard à la souris (monter/descendre pour tourner le bouton, avec possibilité de réglage fin et de remise à zéro rapide) plutôt qu'un mouvement circulaire autour de l'axe du potentiomètre.
- Le look type SSL souhaité porte principalement sur les potentiomètres ronds (capuchons colorés distinctifs par fonction, repère angulaire et graduations) et leur disposition en grille, sans exiger le remplacement des vu-mètres verticaux de droite.
- Le mainteneur est prêt à amender la section GUI de `.specify/memory/constitution.md` et la section 4 de `GEMINI.md` (via `/speckit-constitution`) pour remplacer la règle du slider horizontal par le standard de potentiomètre rotatif.
