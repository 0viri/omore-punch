# Feature Specification: Interface type SSL avec potentiomètres rotatifs en grille

**Feature Branch**: `feat/002-ssl-rotary-knobs-ui`

**Created**: 2026-11-14

**Status**: Draft

**Input**: User description: "avoir un plugin au look type SSL, avec les boutons ronds à la place des faders (horizontaux ... actuels)"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Réglage visuel et ergonomique par potentiomètres rotatifs en grille (Priority: P1)

Un ingénieur du son ou producteur mixant un bus batterie stéréo ouvre l'interface du plugin et visualise les 9 paramètres continus du compresseur et du clipper sous forme de potentiomètres rotatifs (« boutons ronds ») disposés en grille par section, au lieu d'une longue pile de 9 curseurs horizontaux. Chaque bouton rond présente une couronne graduée, un capuchon coloré selon sa famille fonctionnelle (dans l'esprit des consoles et compresseurs de bus SSL sur le fond sombre du plugin) et un repère angulaire indiquant immédiatement la position du réglage. L'utilisateur tourne un bouton en glissant verticalement à la souris et lit la valeur numérique formatée juste sous le bouton.

**Why this priority**: C'est le cœur de la demande utilisateur : remplacer les faders horizontaux par des boutons ronds au look type SSL offrant un repérage angulaire immédiat et une disposition compacte en grille.

**Independent Test**: Peut être testé de manière autonome en ouvrant l'éditeur du plugin dans l'hôte audionumérique, en vérifiant que les 9 paramètres continus (`Input Drive`, `Threshold`, `Ratio`, `Attack`, `Release`, `SC HPF`, `Makeup Gain`, `Clip Ceiling`, `Mix Dry/Wet`) sont affichés sous forme de boutons ronds gradués en grille dans leurs sections respectives, et qu'un glisser vertical de la souris fait varier l'angle du repère ainsi que la valeur du paramètre en temps réel.

**Acceptance Scenarios**:

1. **Given** l'interface du plugin ouverte sur un bus batterie, **When** l'utilisateur observe la colonne de gauche (`1. COMPRESSOR & ENVELOPE BALLISTICS` et `2. OUTPUT CLIPPER & PARALLEL MIX`), **Then** aucun curseur horizontal n'est affiché et les 9 paramètres continus sont représentés par des potentiomètres rotatifs disposés en grille avec étiquette, couronne graduée, capuchon coloré par famille et repère angulaire.
2. **Given** un potentiomètre rotatif à sa valeur courante, **When** l'utilisateur clique et glisse verticalement vers le haut (ou vers le bas) sur le bouton rond, **Then** la valeur du paramètre augmente (ou diminue) proportionnellement sur sa plage autorisée, le repère angulaire tourne entre sa butée minimale et sa butée maximale, la lecture numérique affiche la nouvelle valeur formatée, et l'hôte audionumérique reçoit les notifications de début, de modification et de fin d'automation.
3. **Given** l'interface affichée dans sa taille standard (`820x480` pixels), **When** toutes les sections sont affichées simultanément, **Then** la grille de boutons ronds de la colonne de gauche et la section de metering de la colonne de droite s'affichent côte à côte sans aucun chevauchement ni troncature.

---

### User Story 2 - Réglage fin pas-à-pas et remise à la valeur par défaut (Priority: P2)

Lors d'un mixage précis sur le bus batterie (par exemple ajuster le plafond du clipper au quart de décibel ou revenir instantanément au réglage nominal d'un paramètre), l'utilisateur peut soit maintenir une touche de précision pendant le glisser souris, soit double-cliquer sur le bouton rond pour le réinitialiser, soit utiliser les petits boutons compacts `[-]`, `[+]` et `[Reset]` placés sous chaque potentiomètre rotatif.

**Why this priority**: Garantit que le passage aux boutons ronds ne fait perdre aucune précision ni rapidité de réinitialisation par rapport aux anciens curseurs horizontaux.

**Independent Test**: Peut être testé indépendamment sur n'importe lequel des 9 potentiomètres en cliquant sur `[-]` et `[+]` pour vérifier l'incrément exact, puis en double-cliquant sur le bouton rond ou en cliquant sur `[Reset]` pour vérifier le retour immédiat à la valeur par défaut.

**Acceptance Scenarios**:

1. **Given** un potentiomètre rotatif réglé sur une valeur quelconque, **When** l'utilisateur clique sur le petit bouton `[-]` ou `[+]` associé à ce potentiomètre, **Then** le paramètre diminue ou augmente exactement d'un pas prédéfini (`0.5 dB` pour Input Drive, Threshold et Makeup Gain, `0.25` pour Ratio et Clip Ceiling, `0.5 ms` pour Attack, `5.0 ms` pour Release, `5.0 Hz` pour SC HPF, `5.0 %` pour Mix Dry/Wet) sans dépasser ses bornes.
2. **Given** un potentiomètre rotatif réglé sur une valeur différente de sa valeur par défaut, **When** l'utilisateur double-clique sur le potentiomètre rotatif ou clique sur son bouton compact `[Reset]`, **Then** le paramètre revient immédiatement à sa valeur par défaut et l'angle du bouton ainsi que l'affichage numérique se mettent à jour.
3. **Given** un potentiomètre rotatif en cours de manipulation à la souris, **When** l'utilisateur maintient la touche `Shift` en glissant verticalement, **Then** la variation du paramètre est ralentie pour permettre un ajustement fin.

---

### User Story 3 - Identité visuelle type SSL et préservation des raccourcis et du metering (Priority: P3)

L'utilisateur distingue d'un coup d'œil le rôle de chaque groupe de potentiomètres grâce au codage couleur des capuchons inspiré des consoles SSL sur le fond sombre du plugin, tout en conservant à l'identique les boutons d'action rapide (`[Punch 3ms / 80ms]`, `[Snare Safe 200Hz]`, `[Auto Makeup: ON/OFF]`, modes du clipper) ainsi que les vu-mètres verticaux (`In L/R`, `Out L/R`, `GR` descendant) dans la colonne de droite.

**Why this priority**: Assure la cohérence esthétique et ergonomique globale du plugin sans régression sur le suivi visuel des niveaux et les préréglages rapides de batterie.

**Independent Test**: Peut être testé en vérifiant les couleurs distinctives des capuchons par sous-famille de paramètres, puis en cliquant sur `[Punch 3ms / 80ms]` et `[Snare Safe 200Hz]` pour observer les boutons ronds `Attack`, `Release` et `SC HPF` tourner immédiatement vers leurs positions cibles.

**Acceptance Scenarios**:

1. **Given** les 9 potentiomètres affichés dans les sections `1.` et `2.`, **When** l'utilisateur inspecte leurs capuchons, **Then** chaque famille fonctionnelle arbore une teinte de capuchon distincte de style console SSL (par exemple : niveau/gain d'entrée et makeup, seuil/ratio de compression, constantes de temps Attack/Release, filtre SC HPF, et section Clipper/Mix) qui contraste lisiblement avec le fond sombre et le repère angulaire clair.
2. **Given** l'utilisateur cliquant sur `[Punch 3ms / 80ms]` ou `[Snare Safe 200Hz]`, **When** le préréglage rapide s'applique, **Then** les potentiomètres rotatifs correspondants (`Attack`, `Release`, `SC HPF`) adoptent immédiatement l'angle et l'affichage numérique correspondant aux valeurs cibles (`3.0 ms`, `80.0 ms`, `200.0 Hz`).

---

### Edge Cases

- **Valeurs aux bornes minimale et maximale** : Lorsqu'un utilisateur glisse le curseur au-delà de la limite basse ou haute d'un paramètre (ou clique sur `[-]` / `[+]` en butée), la valeur reste strictement bornée à la plage du paramètre et le repère angulaire s'arrête exactement sur la graduation minimale (~7 heures) ou maximale (~5 heures) sans faire un tour complet.
- **Sessions existantes et redimensionnement** : Lorsqu'un projet existant est rechargé dans l'hôte audionumérique avec la taille de fenêtre enregistrée (`820x480` pixels), toutes les cellules de la grille de potentiomètres et leurs contrôles compacts (`[-]`, `[+]`, `[Reset]`) tiennent intégralement dans la colonne de gauche sans déborder sur la colonne de droite.
- **Compatibilité multi-plateforme des caractères** : Aucune étiquette, graduation ou infobulle autour des boutons ronds n'utilise de caractère non-ASCII (pas de symbole degré `°` ni flèche Unicode), garantissant l'absence de carrés vides sous Linux comme sous Windows.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: L'interface MUST remplacer les 9 curseurs horizontaux par 9 potentiomètres rotatifs (« boutons ronds ») pour les paramètres continus : `Input Drive`, `Threshold`, `Ratio`, `Attack`, `Release`, `SC HPF`, `Makeup Gain`, `Clip Ceiling` et `Mix Dry/Wet`.
- **FR-002**: Chaque potentiomètre rotatif MUST afficher une couronne extérieure avec repères/graduations angulaires (couvrant une course de 270 degrés de `-135` à `+135` degrés par rapport à la verticale haute, soit de 7h à 5h), un corps/capuchon circulaire coloré de style SSL et une ligne indicatrice (repère angulaire) reliant le centre au bord du capuchon.
- **FR-003**: Les capuchons des potentiomètres MUST utiliser un codage couleur par famille fonctionnelle inspiré des consoles SSL tout en s'intégrant au thème sombre du plugin :
  - Gain / Drive (`Input Drive`, `Makeup Gain`) : capuchon ambre/or chaud ;
  - Dynamique (`Threshold`, `Ratio`) : capuchon gris clair / argent SSL ;
  - Enveloppe temporelle (`Attack`, `Release`) : capuchon vert SSL ;
  - Filtre de détection (`SC HPF`) : capuchon bleu SSL ;
  - Sortie Clipper & Mix (`Clip Ceiling`, `Mix Dry/Wet`) : capuchon rouge/corail SSL.
- **FR-004**: Dans la section `1. COMPRESSOR & ENVELOPE BALLISTICS`, les 7 potentiomètres rotatifs MUST être organisés en grille compacte de 2 rangées (4 boutons sur la 1re rangée, 3 boutons sur la 2e rangée) accompagnée de la rangée des 3 boutons d'action rapide (`[Punch 3ms / 80ms]`, `[Snare Safe 200Hz]`, `[Auto Makeup: ON/OFF]`).
- **FR-005**: Dans la section `2. OUTPUT CLIPPER & PARALLEL MIX`, les 2 potentiomètres rotatifs (`Clip Ceiling` et `Mix Dry/Wet`) MUST être disposés côte à côte en grille sous le sélecteur de mode du clipper (`[Soft Clip]`, `[Hard Clip]`, `[Bypass]`).
- **FR-006**: Chaque cellule de potentiomètre MUST présenter de manière alignée et lisible en police monospace 100 % ASCII : le nom du paramètre au-dessus du bouton, le potentiomètre rotatif au centre, la valeur numérique formatée en dessous, et une rangée compacte de trois petits boutons `[-]`, `[+]` et `[Reset]`.
- **FR-007**: L'utilisateur MUST pouvoir ajuster chaque potentiomètre rotatif en cliquant et glissant verticalement la souris (glisser vers le haut augmente la valeur, glisser vers le bas la diminue), avec une sensibilité réduite (réglage fin) lorsque la touche `Shift` est maintenue.
- **FR-008**: Un double-clic sur le potentiomètre rotatif OU un clic sur son bouton compact `[Reset]` MUST réinitialiser immédiatement le paramètre à sa valeur par défaut.
- **FR-009**: Toute modification de paramètre via glisser souris, double-clic, bouton `[-]`, bouton `[+]` ou bouton `[Reset]` MUST notifier correctement l'hôte audionumérique du début, du changement et de la fin du geste d'édition pour assurer l'enregistrement fidèle des automations.
- **FR-010**: Le bandeau supérieur, les boutons d'action rapide entre crochets et l'intégralité de la colonne de droite (`3. GAIN REDUCTION & STEREO BUS METERING`) MUST conserver leur comportement et leur lisibilité actuels, sans aucun chevauchement entre colonnes dans la fenêtre de `820x480` pixels.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Les 9 paramètres continus du plugin (`100 %`) sont contrôlables par potentiomètre rotatif (glisser vertical, `Shift`+glisser fin, double-clic reset) ainsi que par leurs boutons compacts `[-]`, `[+]` et `[Reset]`, avec `0` curseur horizontal restant dans l'interface.
- **SC-002**: La largeur totale de chaque rangée de potentiomètres dans la colonne de gauche et la hauteur totale des sections `1.` et `2.` restent inférieures ou égales aux dimensions allouées dans la fenêtre `820x480` pixels (`0` pixel de débordement horizontal ou vertical), vérifié par au moins un test automatisé de géométrie de mise en page.
- **SC-003**: `100 %` des chaînes de caractères et étiquettes de l'interface respectent la contrainte pure ASCII (`0` caractère non-ASCII détecté).
- **SC-004**: L'intégralité des tests de traitement audio existants continue de passer à `100 %` sans aucune régression ni modification du comportement sonore.

## Assumptions

- Le glisser vertical linéaire à la souris (monter/descendre) est retenu comme mode d'interaction principal sur les boutons ronds car il constitue le standard ergonomique dans les stations de travail audio (Reaper) et évite l'imprécision du suivi angulaire circulaire autour du centre du bouton.
- Une course angulaire de 270 degrés (de -135 degrés à +135 degrés par rapport à la verticale haute, soit de 7h à 5h) offre le meilleur compromis visuel pour lire la position d'un potentiomètre de type SSL.
