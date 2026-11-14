# Feature Specification: Remplacement du bouton rapide Kick Safe 90Hz par Snare Safe 200Hz

**Feature Branch**: `feat/001-snare-safe-200hz`

**Created**: 2026-11-14

**Status**: Draft

**Input**: User description: "Changer le bouton "Kick safe 90Hz" en "Snare Safe 200Hz" (technique Cristian Kohle)"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Activation rapide du filtre de détection à 200 Hz (Priority: P1)

Un ingénieur du son ou producteur mixant un bus batterie stéréo souhaite appliquer en un clic la technique de compression parallèle popularisée par Cristian Kohle : filtrer le signal de détection du compresseur jusqu'à 200 Hz afin que ni le sous-grave de la grosse caisse (Kick) ni le corps fondamental de la caisse claire (Snare, situé autour de 150-200 Hz) ne déclenchent une réduction de gain excessive. Le compresseur réagit alors principalement aux transitoires d'attaque et aux hautes fréquences, laissant passer tout le poids et l'impact de la grosse caisse et de la caisse claire sans pompage.

**Why this priority**: C'est l'objectif principal de la demande et le cœur de l'amélioration ergonomique sur le bus batterie.

**Independent Test**: Peut être testé de manière autonome en ouvrant l'interface du plugin, en vérifiant la présence du bouton `[Snare Safe 200Hz]` à la place de `[Kick Safe 90Hz]`, et en cliquant dessus pour constater que la fréquence du filtre passe-haut de détection (`SC HPF`) passe immédiatement à `200.0 Hz`.

**Acceptance Scenarios**:

1. **Given** le filtre passe-haut de détection (`SC HPF`) réglé à sa valeur par défaut (`20.0 Hz`) ou à toute autre valeur, **When** l'utilisateur clique sur le bouton d'action rapide `[Snare Safe 200Hz]`, **Then** le paramètre `SC HPF` est immédiatement réglé sur `200.0 Hz` et l'affichage numérique ainsi que le curseur reflètent `200.0 Hz`.
2. **Given** un hôte audionumérique enregistrant les mouvements de paramètres (automation ou état de session), **When** l'utilisateur clique sur `[Snare Safe 200Hz]`, **Then** le changement de valeur vers `200.0 Hz` est correctement notifié à l'hôte et conservé lors de la sauvegarde de la session.

---

### User Story 2 - Compréhension immédiate du rôle du bouton via l'infobulle et la documentation (Priority: P2)

Un utilisateur survolant le bouton d'action rapide ou consultant la documentation du plugin comprend immédiatement que le préréglage `200.0 Hz` protège à la fois la grosse caisse et le corps de la caisse claire contre la sur-compression du bus batterie.

**Why this priority**: Garantit la cohérence entre le nouveau libellé du bouton, son texte d'aide au survol et la documentation utilisateur du projet.

**Independent Test**: Peut être testé en survolant le bouton `[Snare Safe 200Hz]` avec la souris pour lire l'infobulle et en vérifiant que la documentation utilisateur mentionne `[Snare Safe 200Hz]` au lieu de `[Kick Safe 90Hz]`.

**Acceptance Scenarios**:

1. **Given** l'interface du plugin ouverte, **When** l'utilisateur survole le bouton `[Snare Safe 200Hz]`, **Then** une infobulle en anglais (100 % ASCII) explique que le filtre passe-haut de sidechain est réglé à `200.0 Hz` afin que le grave de la grosse caisse et le corps de la caisse claire ne fassent pas pomper le bus batterie.
2. **Given** un signal basse fréquence ou bas-médium sous `200 Hz` représentatif du corps de la caisse claire ou de la grosse caisse envoyé dans le compresseur avec le préréglage `200.0 Hz` actif, **When** on compare la réduction de gain avec le filtre désactivé (`20.0 Hz`), **Then** la réduction de gain déclenchée par ces fréquences est significativement atténuée tandis que le signal audio de sortie conserve l'intégralité de ses basses fréquences.

### Edge Cases

- Que se passe-t-il lorsque `SC HPF` est déjà réglé sur `200.0 Hz` et que l'utilisateur clique à nouveau sur `[Snare Safe 200Hz]` ? Le paramètre reste stable à `200.0 Hz` sans saut audio ni réinitialisation d'état indésirable.
- Que se passe-t-il si l'utilisateur ajuste manuellement le curseur `SC HPF`, les boutons `-` / `+` ou le bouton `Reset` après avoir cliqué sur `[Snare Safe 200Hz]` ? Le paramètre répond normalement sur toute sa plage (`20.0 Hz` à `250.0 Hz`) et `Reset` le ramène à sa valeur par défaut (`20.0 Hz`).
- Que se passe-t-il lors de l'ouverture d'un projet existant sauvegardé avec une ancienne valeur de `SC HPF` (par exemple `90.0 Hz`) ? La valeur sauvegardée dans la session de l'hôte est intégralement préservée car l'identifiant et la plage du paramètre `SC HPF` restent inchangés.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: L'interface du plugin DOIT remplacer le bouton d'action rapide `[Kick Safe 90Hz]` par `[Snare Safe 200Hz]` dans la section des contrôles du compresseur, à côté du bouton `[Punch 3ms / 80ms]` et du bouton `[Auto Makeup: ON/OFF]`.
- **FR-002**: Un clic sur le bouton `[Snare Safe 200Hz]` DOIT régler la fréquence de coupure du filtre passe-haut de détection (`SC HPF`) exactement à `200.0 Hz` et notifier l'hôte du début, de la valeur et de la fin de la modification du paramètre.
- **FR-003**: Le survol du bouton `[Snare Safe 200Hz]` DOIT afficher une infobulle explicative en anglais (100 % caractères ASCII imprimables) précisant que le filtre passe-haut de sidechain passe à `200.0 Hz` pour empêcher le grave de la grosse caisse et le corps de la caisse claire de faire pomper le bus batterie.
- **FR-004**: La plage du paramètre `SC HPF` (`20.0 Hz` à `250.0 Hz`), sa valeur par défaut (`20.0 Hz`) et son identifiant d'automation hôte DOIVENT rester strictement identiques afin de garantir 100 % de rétrocompatibilité avec les sessions existantes.
- **FR-005**: La documentation du projet (`README.md`, `USER_GUIDE.md`, `CHANGELOG.md`, `GEMINI.md`) DOIT être mise à jour lors de l'implémentation pour refléter le remplacement de `[Kick Safe 90Hz]` par `[Snare Safe 200Hz]`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: L'utilisateur peut régler le filtre passe-haut de détection à `200.0 Hz` en 1 seul clic via le bouton `[Snare Safe 200Hz]`.
- **SC-002**: 100 % des textes d'interface ajoutés ou modifiés (bouton et infobulle) utilisent exclusivement des caractères ASCII standards afin de garantir un affichage sans caractère manquant sous Linux et Windows.
- **SC-003**: À `200.0 Hz`, un signal de test représentatif du grave/bas-médium (grosse caisse et fondamentale de caisse claire sous `200 Hz`) subit une réduction de gain nettement inférieure à celle mesurée lorsque le filtre est à `20.0 Hz`, tout en préservant l'intégralité du spectre grave dans le signal audio traité.
- **SC-004**: 100 % des sessions existantes utilisant le paramètre `SC HPF` se rechargent sans perte d'automation ni altération de la valeur enregistrée.

## Assumptions

- Le remplacement de `[Kick Safe 90Hz]` par `[Snare Safe 200Hz]` est une substitution directe d'un bouton unique (pas d'ajout d'un second bouton côte à côte), conformément à la demande explicite « Changer le bouton "Kick safe 90Hz" en "Snare Safe 200Hz" ».
- La plage actuelle du filtre passe-haut de détection (`20.0 Hz` à `250.0 Hz`) couvre déjà nativement la fréquence cible de `200.0 Hz` sans nécessiter de modification de l'algorithme du filtre.
- L'exemple typographique `[Kick Safe 90Hz]` dans `GEMINI.md` (section 4) est mis à jour en `[Snare Safe 200Hz]`, tandis que `.specify/memory/constitution.md` (qui ne mentionne pas ce bouton) n'est jamais modifié en dehors de `/speckit-constitution`.
