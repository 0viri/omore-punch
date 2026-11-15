# Feature Specification: Gain Match Automatique (Égalisation de Volume Entrée/Sortie)

**Feature Branch**: `006-gain-match`

**Created**: 2026-11-14

**Status**: Draft

**Input**: User description: "mettre en place une fonction gain match pour que le volum de sortie soit égale au volume d'entrée après traitement"

## Clarifications

### Session 2026-11-14

- Q: À quel endroit de la chaîne audio le Gain Match doit-il appliquer sa correction de niveau par rapport à l'Output Clipper ? → A: Après l'Output Clipper (en toute fin de chaîne), afin de compenser y compris l'écrêtage sans modifier la saturation du clipper.
- Q: Comment le Gain Match doit-il interagir avec `Auto Makeup` et `Makeup Gain` ? → A: Ils restent indépendants et actifs (complémentaires) ; Gain Match compense l'écart net final en sortie, permettant d'utiliser `Makeup Gain` comme drive vers le clipper.
- Q: Comment doit se comporter la correction de gain du Gain Match pendant un silence ou une pause sous le seuil de bruit ? → A: Figer (Hold) la dernière correction valide mesurée afin que la reprise du jeu démarre au bon niveau, sans amplifier le bruit de fond.
- Q: Où placer le bouton `[Gain Match: OFF/ON]` et l'affichage de la compensation en dB dans l'interface ? → A: Dans la section `4. GAIN REDUCTION & STEREO BUS METERING` (colonne droite, à proximité du bloc `GR` / `[Reset Peak]` et des vu-mètres `In L/R` et `Out L/R`).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Égalisation automatique du volume entrée/sortie après traitement (Priority: P1)

En tant qu'ingénieur du son ou producteur utilisant Omore Punch sur un bus batterie, je veux activer une fonction **Gain Match** afin que le volume perçu en sortie du plugin reste automatiquement égal au volume d'entrée, quels que soient les réglages de compression, de saturation/clipper, d'égalisation (Drum Bus EQ) ou de mix parallèle.

**Why this priority**: C'est le cœur de la demande : permettre à l'utilisateur d'évaluer objectivement l'impact tonal, le punch et la densité du traitement sans être trompé par une hausse ou une baisse de volume globale (« louder sounds better »), et éviter les sauts de niveau dans le mix lors des réglages.

**Independent Test**: Peut être testé de façon autonome en envoyant un signal stéréo de batterie (ou un signal de test continu/percussif) à un niveau connu, en activant le traitement (compression, EQ, clipper) avec Gain Match activé, et en vérifiant que le niveau RMS moyen en sortie converge vers le niveau RMS moyen d'entrée (à ±0,5 dB près) sans décalage de balance stéréo.

**Acceptance Scenarios**:

1. **Given** un signal audio stéréo actif et le traitement du plugin entraînant une hausse de niveau (ex. EQ boost, Makeup Gain ou Drive), **When** l'utilisateur active Gain Match, **Then** le niveau de sortie est automatiquement atténué de façon lisse jusqu'à correspondre au niveau moyen d'entrée.
2. **Given** un signal audio stéréo actif et le traitement entraînant une baisse de niveau (ex. forte compression sans Auto Makeup, ou clipper à plafond bas), **When** Gain Match est activé, **Then** le niveau de sortie est automatiquement compensé à la hausse de façon lisse pour rejoindre le niveau moyen d'entrée (dans la limite de la plage de compensation autorisée).
3. **Given** Gain Match est activé sur un bus batterie stéréo, **When** le traitement est appliqué, **Then** la même correction de gain est appliquée aux canaux gauche et droit afin de préserver intégralement l'image stéréo.
4. **Given** Gain Match est activé pendant la lecture, **When** l'utilisateur modifie en direct le Threshold, le Ratio, le Mix, le Ceiling ou le Drum Bus EQ, **Then** la compensation s'adapte progressivement pour maintenir le volume de sortie aligné sur celui d'entrée, sans artefacts audibles ni effet de pompage rapide sur les transitoires.

---

### User Story 2 - Activation/Désactivation fluide et affichage de la compensation appliquée (Priority: P2)

En tant qu'utilisateur, je veux pouvoir activer ou désactiver le Gain Match d'un simple clic (et automatiser ce choix dans mon DAW), tout en visualisant clairement l'état actif et la valeur de compensation (en dB) actuellement appliquée.

**Why this priority**: L'utilisateur doit garder le contrôle sur l'activation de la fonction (notamment pour préserver le comportement existant de ses projets lorsque Gain Match est désactivé) et savoir de combien de dB le plugin corrige le signal en sortie.

**Independent Test**: Peut être testé en basculant le contrôle Gain Match entre ON et OFF pendant la lecture audio, en vérifiant l'absence de clic audio à la transition, la persistance de l'état dans la session DAW, et la mise à jour de l'indicateur de compensation en dB.

**Acceptance Scenarios**:

1. **Given** un projet existant ou une nouvelle instance du plugin, **When** le plugin est chargé par défaut, **Then** Gain Match est désactivé (`OFF`) afin de ne pas altérer les sessions existantes ni le niveau manuel configuré.
2. **Given** du signal audio passe dans le plugin, **When** l'utilisateur bascule Gain Match de `OFF` à `ON` ou de `ON` à `OFF`, **Then** le changement de gain s'effectue par une transition lissée sans clic ni pop audible.
3. **Given** Gain Match est activé, **When** une compensation est en cours, **Then** l'interface affiche l'état actif ainsi que la correction appliquée en dB (ex. `-3.2 dB` ou `+2.5 dB`), et affiche `0.0 dB` (ou un état inactif) lorsque Gain Match est désactivé.

---

### User Story 3 - Préservation des transitoires et comportement sûr sur les silences (Priority: P3)

En tant qu'utilisateur, je veux que le Gain Match mesure le volume sur une fenêtre temporelle adaptée au bus batterie et reste inactif sur les passages silencieux, afin de ne jamais remonter le bruit de fond entre les frappes ni écraser l'attaque des fûts.

**Why this priority**: Un ajustement trop rapide se comporterait comme un second compresseur et détruirait le punch recherché ; à l'inverse, une compensation active pendant le silence amplifierait le souffle.

**Independent Test**: Peut être testé en envoyant une séquence percussive suivie d'un silence ou d'un bruit de fond très faible (< -60 dBFS) avec Gain Match activé, et en vérifiant que la compensation ne dérive pas vers un gain extrême pendant le silence et n'écrase pas les transitoires rapides.

**Acceptance Scenarios**:

1. **Given** Gain Match est activé, a convergé vers une correction valide (ex. `-3.0 dB`), et le signal d'entrée tombe sous le seuil de bruit (silence ou break entre deux mesures), **When** aucun signal utile n'est présent, **Then** la correction de gain reste figée (Hold) à sa dernière valeur valide sans dériver ni chercher à amplifier le bruit de fond.
2. **Given** Gain Match est activé sur une boucle de batterie percussive (kick/snare avec silence relatif entre les coups), **When** les coups frappent, **Then** l'enveloppe de mesure reste suffisamment lente et stable pour égaliser le volume moyen sans pomper à chaque coup.

---

### Edge Cases

- **Silence total ou bruit très faible** : Lorsque le niveau d'entrée est inférieur au seuil de détection (ex. -60 dBFS), le système ne calcule pas de ratio extrême, fige (Hold) la dernière correction valide (ou reste à `0.0 dB` si aucun signal utile n'a encore été mesuré) et évite toute amplification du bruit de fond.
- **Plage de compensation extrême** : Si l'utilisateur règle un plafond de clipper très bas (ex. -18 dBFS) avec un signal d'entrée à 0 dBFS, ou applique un boost extrême, la correction de gain est bornée dans une plage sûre (ex. `[-24 dB, +24 dB]`) et le limiteur de sécurité final reste prioritaire pour éviter tout dépassement dangereux vers le DAW.
- **Traitement neutre** : Si aucun traitement ne modifie le niveau (Threshold à 0 dB, EQ neutre, Clipper sans écrêtage, Mix à 100 % ou 0 %), la correction appliquée reste proche de `0.0 dB`.
- **Variation brutale du signal d'entrée** : En cas d'arrêt brusque de la lecture dans le DAW ou de saut de niveau d'entrée, le lissage évite les discontinuités audio.
- **Interaction avec Auto Makeup et Makeup Gain** : Gain Match agit en bout de chaîne globale (après compression, Auto Makeup/Makeup Gain, EQ, Mix parallèle et Clipper) pour compenser l'écart final net entre entrée et sortie, quel que soit l'état d'Auto Makeup.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Le système DOIT fournir un paramètre activable/désactivable (« Gain Match »), désactivé (`OFF`) par défaut, exposable à l'automatisation du DAW et sauvegardé avec l'état du plugin.
- **FR-002**: Lorsque Gain Match est activé, le système DOIT mesurer en continu le niveau moyen (RMS stéréo lié) du signal d'entrée brut (avant traitement) et le niveau moyen du signal traité en fin de chaîne (après compression, makeup, mix parallèle, égalisation et clipper).
- **FR-003**: Le système DOIT calculer et appliquer un gain de correction unique et identique sur les deux canaux stéréo (gauche et droite) après l'Output Clipper (en toute fin de chaîne de traitement), sans altérer le niveau entrant dans le clipper ; le témoin d'écrêtage global et la protection anti-saturation finale restent évalués sur le signal de sortie final.
- **FR-004**: La mesure de niveau et l'ajustement du gain de correction DOIVENT utiliser un lissage temporel lent et progressif afin de préserver les transitoires et d'éviter tout effet de pompage ou de clic lors des variations de signal ou du basculement ON/OFF.
- **FR-005**: Le calcul de compensation DOIT ignorer les passages sous un seuil de silence (noise gate de mesure) et figer (Hold) la dernière correction valide pendant le silence (en restant à `0.0 dB` tant qu'aucun signal utile n'a été détecté), afin de ne pas amplifier le bruit de fond et de garantir un niveau immédiat lors de la reprise du signal.
- **FR-006**: Le gain de correction DOIT être borné dans une plage de sécurité définie (ex. `[-24 dB, +24 dB]`) et la sortie finale DOIT rester soumise à la protection anti-saturation/valeurs invalides du plugin.
- **FR-007**: Lorsque Gain Match est désactivé (`OFF`), le gain de correction effectif DOIT revenir de manière lisse à `0.0 dB` (gain unitaire `1.0`) afin de ne produire aucun changement de niveau ni clic audio.
- **FR-008**: L'interface utilisateur DOIT proposer un contrôle clair (`[Gain Match: OFF]` / `[Gain Match: ON]`) ainsi qu'un affichage en temps réel de la correction appliquée en dB dans la section `4. GAIN REDUCTION & STEREO BUS METERING` (colonne droite, à proximité des vu-mètres `In L/R` et `Out L/R`).
- **FR-009**: Les réglages existants `Auto Makeup` et `Makeup Gain` DOIVENT rester actifs et modifiables lorsque Gain Match est activé ; Gain Match compense alors l'écart de niveau global résultant de l'ensemble de la chaîne (y compris leur effet).

### Key Entities *(include if feature involves data)*

- **Paramètre Gain Match** : État booléen (`ON` / `OFF`, défaut `OFF`) contrôlé par l'utilisateur et le DAW.
- **Mesure de niveau Entrée / Sortie** : Enveloppes RMS stéréo liées représentant le volume perçu avant traitement et après traitement.
- **Trim de Compensation (Gain Match Trim)** : Gain correctif lissé en dB (borné), appliqué uniformément en sortie et exposé en lecture à l'interface utilisateur.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Sur un signal stéréo stationnaire ou une boucle de batterie active (au-dessus du seuil de silence) soumis à une compression, un boost/cut d'EQ ou un écrêtage modifiant le niveau de ±3 dB à ±12 dB, l'écart entre le niveau RMS de sortie et le niveau RMS d'entrée après stabilisation avec Gain Match activé est inférieur ou égal à **0,5 dB**.
- **SC-002**: L'activation, la désactivation et la modification des paramètres de traitement avec Gain Match activé ne produisent aucune discontinuité brusque (pas de clic audible ni de valeur non-finie) et préservent strictement la balance gauche/droite (écart inter-canal introduit par Gain Match égal à `0.0 dB`).
- **SC-003**: En présence d'un signal d'entrée silencieux ou inférieur au seuil de bruit (ex. -70 dBFS) après un signal actif ou dès l'initialisation, le gain de correction reste figé à la dernière valeur valide (ou à `0.0 dB` à froid), ne dérive pas vers le plafond maximal d'amplification et n'amplifie pas le bruit de fond.
- **SC-004**: L'utilisateur peut activer Gain Match en un clic depuis la section `4. GAIN REDUCTION & STEREO BUS METERING` et lire instantanément la compensation appliquée en dB sans chevauchement visuel dans l'interface.

## Assumptions

- **Position dans la chaîne** : Gain Match agit tout à la fin du traitement audio (post-compression, post-mix parallèle, post-EQ et post-clipper), juste avant l'étage de sanitisation/sécurité finale, conformément à la demande « après traitement ».
- **État par défaut** : Gain Match est `OFF` par défaut pour préserver la rétrocompatibilité des sessions DAW existantes et ne pas surprendre l'utilisateur qui souhaite régler manuellement son niveau de sortie.
- **Cohabitation avec Auto Makeup / Makeup Gain** : Les contrôles existants `Auto Makeup` et `Makeup Gain` restent disponibles ; lorsque Gain Match est activé, il compense automatiquement le résultat net final, quel que soit leur réglage.
- **Type de mesure** : Une mesure RMS stéréo liée (énergie moyenne des deux canaux) avec une constante d'intégration lente (ordre de grandeur ~300–500 ms) et un lissage doux du gain constitue la référence standard pour du gain matching musical sur bus batterie.
