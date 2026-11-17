# Feature Specification: Bouton Bypass Global

**Feature Branch**: `feat/008-bypass-button`

**Created**: 2026-11-15

**Status**: Draft

**Input**: User description: "Ajouter un bouton bypass"

## Clarifications

### Session 2026-11-15

- Q: Comment le bouton Bypass du plugin doit-il interagir avec le bouton Bypass natif du DAW ? → A: Paramètre Bypass lié au DAW (synchronisé de manière bidirectionnelle avec le bypass natif du DAW hôte, un seul état partagé avec transition lissée sans clic).
- Q: Que doivent faire les indicateurs mémorisés (`Max GR`, `Peak GR Hold`, témoin `[CLIP: ACTIVE]`) et l'état d'adaptation du Gain Match lorsque le Bypass est actif ? → A: Figer et conserver les valeurs mémorisées (`Max GR`, `Peak GR Hold`, `[CLIP]` déjà allumé et correction en cours du Gain Match) pendant le Bypass tout en mettant `GR` instantané à `0.0 dB`.
- Q: Quel retour visuel et quel comportement d'interaction doit avoir le reste de l'interface graphique lorsque le Bypass est actif (`ON`) ? → A: Bouton `[BYPASS: ON]` mis en évidence dans l'en-tête + contrôles de traitement toujours éditables avec une légère atténuation visuelle des sections de traitement.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Comparaison A/B instantanée et transparente (Priority: P1)

En tant qu'ingénieur du son ou producteur utilisant Omore Punch sur un bus batterie, je veux pouvoir activer et désactiver un bouton **Bypass** global afin de comparer instantanément le signal brut (non traité) et le signal traité par le plugin (compression, égalisation, clipper, gain match).

**Why this priority**: La comparaison A/B rapide est un geste essentiel en mixage pour évaluer si le traitement améliore réellement le bus batterie sans biais. C'est la valeur principale d'un bouton bypass.

**Independent Test**: Peut être testé de façon autonome en envoyant un signal audio stéréo actif à travers le plugin avec des réglages de traitement audibles (compression, EQ, clipper), en activant le bypass et en vérifiant que la sortie devient identique à l'entrée (à la latence près, nulle ici), puis en le désactivant pour retrouver le signal traité.

**Acceptance Scenarios**:

1. **Given** le plugin traite un signal stéréo avec compression, EQ et/ou clipper actifs, **When** l'utilisateur active le bouton Bypass, **Then** le signal audio d'entrée est transmis directement en sortie sans aucune compression, égalisation, saturation, écrêtage ni correction de gain.
2. **Given** le plugin est en état Bypass actif, **When** l'utilisateur désactive le bouton Bypass, **Then** le traitement complet du plugin est immédiatement rétabli selon les réglages courants.
3. **Given** un signal audio actif passe dans le plugin, **When** l'utilisateur bascule le Bypass (activation ou désactivation), **Then** la transition entre signal traité et signal brut s'effectue sans clic, pop ni artefact audible.

---

### User Story 2 - Accès permanent dans l'interface, état visuel et intégration DAW (Priority: P2)

En tant qu'utilisateur, je veux trouver le bouton Bypass immédiatement accessible dans l'en-tête de l'interface, voir clairement s'il est actif ou inactif, et pouvoir l'automatiser ou le sauvegarder dans ma session DAW.

**Why this priority**: Un bouton bypass doit être visible en permanence sans avoir à chercher dans les sections de paramètres, et son état doit être fidèlement mémorisé par le DAW comme tout autre contrôle du plugin.

**Independent Test**: Peut être testé en ouvrant l'interface du plugin, en vérifiant la présence et la lisibilité du bouton dans la barre d'en-tête, en enregistrant et rechargeant un projet DAW avec le bypass activé, et en pilotant le bypass via une piste d'automation DAW.

**Acceptance Scenarios**:

1. **Given** une nouvelle instance du plugin ou un projet existant ouvert pour la première fois après mise à jour, **When** le plugin est chargé par défaut, **Then** le Bypass est désactivé (`OFF`, traitement actif) afin de préserver le comportement attendu des sessions existantes.
2. **Given** l'interface du plugin est ouverte, **When** le Bypass est activé (`ON`), **Then** le bouton dans l'en-tête s'affiche de manière distincte et mise en évidence (`[BYPASS: ON]` en texte ASCII), les sections de contrôles de traitement présentent une légère atténuation visuelle indiquant que le traitement est contourné, et tous les contrôles restent néanmoins modifiables par l'utilisateur.
3. **Given** le Bypass est activé par l'utilisateur depuis l'interface, depuis le bouton Bypass natif du DAW ou via une automation, **When** l'état change ou que le projet est sauvegardé puis rechargé, **Then** l'état du Bypass est synchronisé de manière bidirectionnelle entre l'interface et le DAW, utilise la même transition lissée sans clic et est restauré à l'identique au rechargement.

---

### User Story 3 - Cohérence du metering pendant et après le Bypass (Priority: P3)

En tant qu'utilisateur, je veux que les indicateurs visuels (vu-mètres et réduction de gain) restent cohérents lorsque le Bypass est actif afin de ne pas être induit en erreur par des mesures résiduelles du traitement contourné.

**Why this priority**: Voir une jauge de réduction de gain bouger alors que le plugin est bypassé crée de la confusion lors d'une comparaison A/B.

**Independent Test**: Peut être testé en activant le Bypass pendant la lecture d'un signal fort qui déclenchait auparavant de la compression et/ou du clipping, et en observant les afficheurs de réduction de gain et les vu-mètres de sortie.

**Acceptance Scenarios**:

1. **Given** le plugin applique une réduction de gain visible avant activation du Bypass, **When** le Bypass est activé, **Then** l'affichage de la réduction de gain instantanée indique `0.0 dB` (aucune réduction appliquée), les vu-mètres de sortie reflètent le niveau du signal brut transmis, et les valeurs crêtes déjà mémorisées (`Max GR` et `Peak GR Hold`) restent figées et consultables.
2. **Given** le Bypass est actif sur un signal dépassant le plafond configuré du clipper, **When** le signal traverse le plugin, **Then** l'indicateur de clipping interne ne se déclenche pas (tout en conservant l'état mémorisé s'il avait déjà été déclenché avant le Bypass, jusqu'à acquittement manuel).
3. **Given** le Gain Match est actif et a convergé vers une correction donnée avant activation du Bypass, **When** le Bypass est activé puis désactivé rapidement pour une comparaison A/B, **Then** la correction mémorisée du Gain Match reste figée pendant le Bypass et reprend sans saut de niveau ni réadaptation brutale au retour.

---

### Edge Cases

- Que se passe-t-il si le signal d'entrée contient des valeurs anormales (non finies ou hors plage extrême de sécurité) alors que le Bypass est actif ? La protection de sécurité en sortie reste active afin de ne jamais provoquer de mise en sourdine automatique du DAW hôte.
- Que se passe-t-il si l'utilisateur bascule rapidement le Bypass plusieurs fois de suite pendant un passage percussif ? Chaque transition reste lissée et stable, sans accumulation d'état erronée ni saturation.
- Que se passe-t-il lorsque les réglages (Threshold, Mix, EQ, Ceiling, Gain Match) sont modifiés pendant que le Bypass est actif ? Les réglages sont bien mémorisés et pris en compte dès la désactivation du Bypass, sans affecter le signal tant que le Bypass reste actif.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Le plugin DOIT exposer un contrôle de **Bypass** global activable et désactivable par l'utilisateur.
- **FR-002**: L'état par défaut du contrôle Bypass DOIT être désactivé (`OFF`, traitement audio actif).
- **FR-003**: Lorsque le Bypass est actif, le signal audio d'entrée stéréo DOIT être acheminé vers la sortie sans modification par la chaîne de traitement (ni égalisation, ni compression, ni gain de compensation, ni mix parallèle, ni clipper, ni Gain Match).
- **FR-004**: Lorsque le Bypass est actif, la protection de sécurité contre les valeurs non finies et les niveaux extrêmes hors plage DOIT rester active sur le signal de sortie.
- **FR-005**: Les transitions entre l'état actif et l'état bypassé (dans les deux sens) DOIVENT être lissées sur une courte durée afin d'éviter tout clic ou pop audible.
- **FR-006**: Le contrôle Bypass DOIT être exposé comme le paramètre Bypass officiel du plugin auprès du DAW hôte (automatisable, persistant dans la session et synchronisé de manière bidirectionnelle avec le contrôle Bypass natif du DAW).
- **FR-007**: L'interface graphique DOIT afficher un bouton Bypass dédié dans la barre d'en-tête supérieure, avec les libellés en ASCII pur `[BYPASS: OFF]` et `[BYPASS: ON]`.
- **FR-008**: Lorsque le Bypass est actif, l'indicateur de réduction de gain instantanée DOIT indiquer `0.0 dB`, les vu-mètres de sortie DOIVENT refléter le niveau réel du signal bypassé émis en sortie, et aucun nouvel événement de clipping interne ne DOIT être signalé.
- **FR-009**: Lorsque le Bypass est actif, les indicateurs mémorisés (`Max GR`, `Peak GR Hold`, témoin `[CLIP: ACTIVE]` existant) ainsi que la correction courante du Gain Match DOIVENT être conservés/figés en mémoire afin de permettre une comparaison A/B sans perte de repères ni saut de niveau au retour.
- **FR-010**: Lorsque le Bypass est actif, le bouton `[BYPASS: ON]` DOIT être visuellement mis en évidence dans l'en-tête, et les sections de contrôles de traitement DOIVENT afficher une légère atténuation visuelle tout en restant interactives et modifiables.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: En mode Bypass actif, la différence entre le signal d'entrée stéréo (dans la plage nominale de fonctionnement) et le signal de sortie est nulle (écart maximal inférieur à `-120 dBFS`, soit une transparence totale après stabilisation de la transition).
- **SC-002**: Le basculement du Bypass (activation ou désactivation) sur un signal continu ou percussif s'effectue sans discontinuité brusque (transition progressive achevée en moins de 20 ms, sans clic audible).
- **SC-003**: Un utilisateur peut activer ou désactiver le Bypass en un seul clic depuis n'importe quel moment de l'utilisation de l'interface, avec un retour visuel immédiat dans la barre d'en-tête.
- **SC-004**: 100 % des paramètres existants (compression, EQ, clipper, mix, Gain Match) conservent leurs valeurs et reprennent leur comportement nominal dès la désactivation du Bypass.

## Assumptions

- Le bouton Bypass est placé dans la barre d'en-tête supérieure (à proximité des indicateurs globaux existants) afin de rester visible et accessible en permanence sans modifier la grille des contrôles de traitement ni la section de metering.
- Il s'agit d'un « soft bypass » (transition lissée sans clic, avec maintien du flux audio dans le plugin pour une bascule instantanée) plutôt qu'une coupure sèche, afin d'assurer une comparaison A/B propre en contexte de mixage.
- La latence du plugin reste inchangée (0 échantillon) en mode actif comme en mode bypass.
