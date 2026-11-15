# Feature Specification: Tag de build Git dans le badge de version de développement (DEV)

**Feature Branch**: `feat/002-dev-version-tag`

**Created**: 2026-11-14

**Status**: Draft

**Input**: User description: "avoir un tag différent affiché avec le numéro de version (dev)" — précisé : "C'est un tag type 1d2c5b9 ou 1d2c5b9-dirty quand c'est compilé en local que je voulais, genre v0.2.1-dev+1d2c5b9-dirty (ou équivalent)"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Identification précise du commit et de l'état local sur les builds de développement (Priority: P1)

Un développeur ou testeur ouvrant Omore Punch dans son DAW souhaite savoir immédiatement non seulement qu'il exécute un build de développement (`-dev`), mais aussi **exactement de quel commit Git** provient ce binaire et s'il a été compilé avec des modifications locales non commitées (`-dirty`). L'en-tête affiche alors le numéro de version de base (`OMORE PUNCH v0.2.1`) suivi d'un badge visuellement distinct contenant `DEV` ainsi que l'identifiant court du commit et l'indicateur `-dirty` éventuel (par exemple `[DEV 1d2c5b9]` pour un build sur un arbre Git propre, ou `[DEV 1d2c5b9-dirty]` pour une compilation locale avec modifications non commitées). Sur une version release publiée (sans `-dev`), aucun badge ni hash Git n'est affiché (`OMORE PUNCH v0.2.1`).

**Why this priority**: Évite toute ambiguïté entre plusieurs builds locaux successifs, un build CI de `main` et une release finale installée dans le système.

**Independent Test**: Peut être testé de manière autonome en compilant le plugin en version `-dev` (propre et avec un fichier modifié localement) et en vérifiant que l'en-tête affiche `[DEV <hash>]` ou `[DEV <hash>-dirty]` dans un badge contrasté sans chevaucher les autres éléments de la barre supérieure.

**Acceptance Scenarios**:

1. **Given** une version de développement (`0.2.1-dev`) compilée sur un arbre Git propre au commit `1d2c5b9`, **When** l'utilisateur ouvre l'interface du plugin, **Then** l'en-tête affiche `OMORE PUNCH v0.2.1` suivi du badge distinct `[DEV 1d2c5b9]`.
2. **Given** une version de développement (`0.2.1-dev`) compilée localement au commit `1d2c5b9` avec des modifications locales non commitées, **When** l'utilisateur ouvre l'interface du plugin, **Then** l'en-tête affiche `OMORE PUNCH v0.2.1` suivi du badge distinct `[DEV 1d2c5b9-dirty]`.
3. **Given** une version release (sans `-dev`, par exemple `0.2.1`), **When** l'utilisateur ouvre l'interface du plugin, **Then** l'en-tête affiche uniquement le numéro de version (`OMORE PUNCH v0.2.1`) sans badge `[DEV ...]` ni hash Git.

---

### User Story 2 - Cohérence de la version exposée à l'hôte DAW (Priority: P2)

Un utilisateur inspectant les informations du plugin dans son DAW s'attend à ce que la version rapportée à l'hôte reste fidèle à la source unique de versionning (`0.2.1-dev` en dev, `0.2.1` en release), sans altération du contrat d'identification DAW par l'ajout visuel du hash Git dans l'interface.

**Why this priority**: Garantit la compatibilité et la traçabilité avec les gestionnaires de plugins DAW tout en conservant la source unique de versionning.

**Independent Test**: Vérifier que la version exposée à l'hôte correspond toujours exactement à la version configurée dans le projet, que le build soit `-dev` (propre ou `-dirty`) ou release.

**Acceptance Scenarios**:

1. **Given** un build de développement (`0.2.1-dev`) avec ou sans état `-dirty`, **When** l'hôte DAW interroge les métadonnées de version du plugin, **Then** la version rapportée reste `0.2.1-dev`.

---

### Edge Cases

- Que se passe-t-il si le dépôt est compilé depuis une archive source sans historique Git ou sans outil `git` disponible ? La compilation réussit sans erreur et le badge de développement se replie proprement sur `[DEV]` seul.
- Que se passe-t-il si la version contient un autre suffixe de pré-release (ex. `-rc.1` ou `-alpha`) sans `-dev` ? Seul le marqueur `-dev` déclenche le badge `[DEV ...]` ; tout autre suffixe reste affiché tel quel avec le numéro de version.
- Que se passe-t-il si la fenêtre est affichée avec le témoin d'alerte maximal `[CLIP: ACTIVE - CLICK TO RESET]` à droite ? La taille de police et l'espacement de l'en-tête garantissent qu'un badge long comme `[DEV 1d2c5b9-dirty]` tient sur une seule ligne dans `820 px` sans débordement ni chevauchement sous Linux et Windows.
- Tous les caractères affichés restent 100 % ASCII pur (aucun emoji ni caractère spécial Unicode).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Le processus de build MUST capturer à la compilation l'identifiant court (7 caractères) du commit Git courant et détecter si l'arbre de travail contient des modifications non commitées (suffixe `-dirty`).
- **FR-002**: Si les informations Git ne sont pas disponibles à la compilation, le processus de build MUST réussir sans erreur et fournir une valeur vide permettant un repli propre.
- **FR-003**: L'interface graphique MUST détecter à partir de la source unique de versionning si la version courante porte le suffixe `-dev`.
- **FR-004**: Lorsque la version courante est une version `-dev`, l'en-tête MUST afficher le numéro de version de base (`vX.Y.Z`) suivi d'un badge visuellement distinct affichant `[DEV <hash>]`, `[DEV <hash>-dirty]`, ou `[DEV]` en repli si aucun tag Git n'est disponible.
- **FR-005**: Lorsque la version courante est une version release (absence de `-dev`), l'en-tête MUST afficher uniquement le numéro de version tel quel, sans badge `[DEV ...]` ni hash Git.
- **FR-006**: L'ensemble de l'en-tête (titre, badge `[DEV <hash>-dirty]` et sous-titre à gauche, témoin `[CLIP: ACTIVE - CLICK TO RESET]` à droite) MUST tenir sur une seule ligne dans la largeur de fenêtre (`820 px`) sans chevauchement ni troncature.
- **FR-007**: Le numéro de version MUST dériver exclusivement de la source unique de versionning du projet, sans chaîne de version codée en dur, et le texte de l'en-tête MUST rester 100 % en ASCII pur.
- **FR-008**: Les métadonnées de version exposées à l'hôte DAW MUST rester inchangées et refléter intégralement la version définie dans la source unique de versionning.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Sur un build local `-dev` avec modifications non commitées, le badge `[DEV <hash>-dirty]` est immédiatement lisible dans l'en-tête en moins d'une seconde grâce à un contraste visuel net.
- **SC-002**: Sur un build `-dev` issu d'un commit propre (local propre ou artefact CI), le badge affiche `[DEV <hash>]` sans suffixe `-dirty`.
- **SC-003**: 100 % des builds release (sans `-dev`) n'affichent ni badge `[DEV]` ni hash Git.
- **SC-004**: L'en-tête reste aligné sur une seule ligne sans chevauchement dans `820 px` même avec `[DEV 1d2c5b9-dirty]` à gauche et `[CLIP: ACTIVE - CLICK TO RESET]` à droite sur Linux et Windows.
- **SC-005**: La compilation aboutit dans 100 % des cas, y compris hors dépôt Git (repli sur `[DEV]`), et la version rapportée au DAW reste strictement identique à la source unique de versionning.

## Assumptions

- La longueur standard retenue pour le hash court est de 7 caractères hexadécimaux (ex. `1d2c5b9`), suivie de `-dirty` dès que l'arbre de travail comporte des modifications suivies ou non suivies par rapport à `HEAD`.
- La détection du suffixe `-dev` suit la convention existante du projet (`X.Y.Z-dev`).
- Ce tag de build est purement informatif côté compilation/UI et n'a aucun impact sur le thread audio temps réel.
