# Feature Specification: DAW Gain Reduction Metering

**Feature Branch**: `007-gain-reduction-metering`

**Created**: 2026-11-15

**Status**: Draft

**Input**: User description: "Ajouter l'exposition de la réduction de gain (Gain Reduction Metering) vers le DAW hôte (Reaper) pour le plugin Omore Punch (VST3 et CLAP), afin d'afficher la barre de réduction de gain directement sur la tranche de console (MCP/TCP) en temps réel."

## Clarifications

### Session 2026-11-15

- Q: Comment la jauge de réduction de gain du DAW et le vumètre interne du plugin doivent-ils réagir lorsque le réglage de mélange parallèle (`Mix`) est inférieur à 100 % ? → A: Rapporter en permanence la réduction brute du détecteur/compresseur (avant le mélange Dry/Wet), à l'identique du vumètre interne existant du plugin, quel que soit le pourcentage de Mix.
- Q: Quelle plage maximale de réduction de gain doit être retenue entre la jauge exposée au DAW et le vumètre interne du plugin ? → A: Aligner la mesure DAW sur l'échelle existante du vumètre interne : de 0.0 dB à -24.0 dB.
- Q: La mesure de réduction de gain doit-elle être totalement absente de la liste des paramètres du plugin dans le DAW, ou peut-elle y figurer en lecture seule pour d'éventuels usages avancés ? → A: Aucun paramètre visible dans la liste des contrôles ou enveloppes d'automation du DAW ; seule la jauge de réduction de gain de la tranche console est alimentée.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Affichage temps réel de la réduction de gain sur la tranche de console du DAW (Priority: P1)

En tant qu'ingénieur du son utilisant Omore Punch (au format VST3 ou CLAP) sur un bus batterie dans Reaper, je veux voir la jauge jaune descendante de réduction de gain directement sur la tranche de console (MCP/TCP) à droite des vumètres de niveau, afin de contrôler visuellement la compression d'un coup d'oeil sans avoir à garder la fenêtre du plugin ouverte.

**Why this priority**: C'est l'objectif principal de la fonctionnalité : offrir un retour visuel immédiat et natif de la compression dans le mixeur du DAW, synchronisé avec le comportement réel du compresseur.

**Independent Test**: Insérer le plugin sur un bus batterie avec un signal dynamique dépassant le Threshold, puis comparer visuellement la descente de la jauge jaune sur la tranche Reaper avec le vumètre interne de réduction de gain du plugin.

**Acceptance Scenarios**:

1. **Given** Omore Punch (VST3 ou CLAP) est chargé sur une piste ou un bus dans Reaper, **When** la tranche de console (MCP/TCP) est affichée, **Then** la jauge jaune descendante de réduction de gain apparaît automatiquement à droite des vumètres de niveau de la tranche.
2. **Given** un signal batterie dépasse le Threshold du compresseur, **When** l'étage de compression atténue les transitoires selon les réglages d'Attack, de Release ou d'Auto-Release, **Then** la jauge jaune du DAW descend de manière fluide et synchrone avec l'enveloppe, et sa valeur correspond au dB près à celle affichée sur le vumètre interne du plugin.
3. **Given** le make-up gain ou l'étage de clipping final modifie le niveau de sortie, **When** le signal est compressé, **Then** la jauge de réduction de gain du DAW continue de refléter uniquement l'atténuation du compresseur sans être faussée par ces étages aval.

---

### User Story 2 - Isolation totale vis-à-vis de la liste des paramètres et de l'automation (Priority: P2)

En tant que mixeur, je veux que l'information de réduction de gain envoyée au DAW passe uniquement par la télémétrie dédiée de la tranche console sans exposer aucun paramètre dans la liste des contrôles ou des enveloppes d'automation, afin que l'ergonomie du DAW et des surfaces de contrôle reste épurée.

**Why this priority**: Un paramètre de télémétrie exposé dans la liste des contrôles perturberait l'ergonomie du DAW, les surfaces de contrôle et les menus d'automation.

**Independent Test**: Ouvrir la liste des paramètres / enveloppes d'automation du plugin dans le DAW et vérifier qu'aucun paramètre de réduction de gain n'y figure.

**Acceptance Scenarios**:

1. **Given** le plugin est chargé dans le DAW, **When** l'utilisateur consulte la liste des contrôles, la vue générique des paramètres ou les pistes d'automation du plugin, **Then** aucun paramètre de réduction de gain n'est exposé ; seule la jauge console dédiée reçoit la mesure.

---

### User Story 3 - Retour à zéro au repos, en Bypass et cohérence en mix parallèle (Priority: P2)

En tant qu'ingénieur du son, je veux que la jauge du DAW revienne proprement à 0.0 dB en l'absence de compression ou en Bypass, et reste cohérente avec le vumètre interne lorsque le mix parallèle (Dry/Wet) est inférieur à 100 %, afin de ne jamais afficher une fausse atténuation sur la console.

**Why this priority**: Une jauge qui reste bloquée en négatif en Bypass ou en silence induit l'utilisateur en erreur sur l'état actif du traitement.

**Independent Test**: Activer puis désactiver le Bypass, couper le signal d'entrée, et réduire le paramètre Mix sous 100 % tout en comparant la jauge du DAW à l'affichage interne du plugin.

**Acceptance Scenarios**:

1. **Given** le compresseur est en train d'atténuer le signal, **When** l'utilisateur active le Bypass ou que le signal d'entrée repasse sous le Threshold (ou en silence), **Then** la jauge du DAW revient proprement à 0.0 dB (barre totalement rétractée en haut).
2. **Given** le réglage de mélange parallèle (Mix / Dry-Wet) est inférieur à 100 %, **When** le compresseur agit sur le signal traité, **Then** la jauge du DAW et le vumètre interne rapportent tous deux la réduction brute du détecteur/compresseur (avant le mélange Dry/Wet), de manière identique au dB près quel que soit le pourcentage de Mix.

---

### Edge Cases

- **Silence et Bypass** : Lorsqu'aucun audio n'entre ou que le plugin est contourné (Bypass), la valeur rapportée doit être exactement 0.0 dB et ne jamais rester figée sur la dernière atténuation connue.
- **Taux d'échantillonnage et tailles de buffer variables** : Entre 44,1 kHz et 192 kHz et avec des tailles de buffer audio petites ou grandes, la jauge doit rester fluide sans crénelage visuel ni dérive temporelle.
- **Forte compression / pics extrêmes** : Lorsque l'atténuation atteint ou dépasse la plage d'affichage (`-24.0 dB`), la valeur rapportée reste bornée dans `[-24.0, 0.0] dB`, finie et alignée sur l'échelle du vumètre interne.
- **Mix parallèle (Dry/Wet < 100 %)** : La mesure exposée au DAW et le vumètre interne du plugin rapportent en permanence la réduction brute du détecteur/compresseur (avant mélange Dry/Wet), sans pondération par le Mix, quel que soit son pourcentage.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Le plugin (aux formats VST3 et CLAP) MUST transmettre au DAW hôte la valeur instantanée de réduction de gain appliquée par l'étage de compression, exprimée en décibels (dB), bornée entre `0.0 dB` (aucune compression) et `-24.0 dB` (atténuation maximale affichée, alignée sur le vumètre interne).
- **FR-002**: La valeur transmise au DAW MUST refléter uniquement la réduction de gain de l'étage de compression, mesurée avant le make-up gain et avant l'étage de clipping final.
- **FR-003**: La valeur transmise MUST évoluer de manière fluide et synchrone avec l'enveloppe d'attaque et de relâchement (Attack, Release, Auto-Release) du compresseur, sans crénelage visuel ni latence perceptible, sur toute la plage de taux d'échantillonnage (44,1 kHz à 192 kHz) et pour toute taille de buffer audio, y compris lorsque la fenêtre graphique du plugin est fermée.
- **FR-004**: L'exposition de la réduction de gain vers le DAW MUST utiliser exclusivement le canal de télémétrie en lecture seule dédié à la jauge console : aucun paramètre de réduction de gain ne doit être visible dans la liste des contrôles ni dans les enveloppes d'automation du plugin.
- **FR-005**: Lorsque le plugin est en Bypass, en l'absence de signal ou lorsque le signal détecté reste sous le Threshold, la valeur rapportée au DAW MUST revenir proprement à 0.0 dB.
- **FR-006**: Lorsque le mélange parallèle (Mix / Dry-Wet) est inférieur à 100 %, la réduction de gain rapportée au DAW MUST refléter en permanence la réduction brute de l'étage de compression (avant le mélange Dry/Wet), à l'identique du vumètre interne de réduction de gain du plugin, quel que soit le pourcentage de Mix.
- **FR-007**: La mise à jour de cette mesure sur le chemin de traitement audio MUST être thread-safe et ne provoquer aucune allocation mémoire ni aucun verrouillage bloquant.

### Key Entities

- **Mesure de réduction de gain DAW (Host Gain Reduction Meter)** : Valeur scalaire en dB (`0.0 dB` au repos, négative jusqu'à `-24.0 dB` en compression), en lecture seule, exposée aux formats VST3 et CLAP pour l'affichage natif sur les tranches de console du DAW hôte.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Chargé sur une piste ou un bus dans Reaper (VST3 ou CLAP), le plugin fait apparaître automatiquement la jauge jaune descendante de réduction de gain à droite des vumètres de niveau de la tranche de console (MCP/TCP), sans action manuelle de configuration.
- **SC-002**: En l'absence de signal ou en état de Bypass, la jauge de réduction de gain affichée dans Reaper reste à 0.0 dB (barre totalement rétractée en haut).
- **SC-003**: Lors d'un pic sur un bus batterie, la descente de la barre jaune dans Reaper correspond au dB près (écart maximal de 1 dB) à l'atténuation affichée sur le vumètre interne de l'interface du plugin.
- **SC-004**: Dans le DAW hôte, aucun paramètre de réduction de gain n'apparaît dans la liste des paramètres ou des pistes d'automation du plugin, seule la jauge console dédiée est alimentée en lecture seule.
- **SC-005**: La fluidité et la précision de la jauge sont maintenues sur tous les taux d'échantillonnage de 44,1 kHz à 192 kHz et avec différentes tailles de buffer audio, sans coupure ni artefact audio.

## Assumptions

- Le DAW cible principal est Reaper (sous Linux Debian 13 et Windows 11), qui prend en charge l'affichage natif de la réduction de gain sur la tranche MCP/TCP via les interfaces standard VST3 et CLAP.
- La cohérence « au dB près » entre le vumètre interne du plugin et la jauge Reaper s'appuie sur la même grandeur de réduction de gain brute du détecteur/compresseur (avant make-up gain, clipper et mélange parallèle Dry/Wet).
- La plage de réduction de gain transmise (`0.0 dB` jusqu'à `-24.0 dB`, alignée sur la graduation du vumètre interne) couvre l'ensemble des cas d'usage sur bus batterie.
