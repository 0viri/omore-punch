# Feature Specification: Métadonnées et Nommage du Bundle Plugin

**Feature Branch**: `005-plugin-bundle-metadata`

**Created**: 2026-11-14

**Status**: Draft

**Input**: User description: "Faire en sorte d'afficher la version, le manufacturer et la category. Est-ce possible de renommer omore-punch en Omore Punch sans 'casser' le plugin ?"

## Clarifications

### Session 2026-11-14

- Q: Quel nom de fabricant (Manufacturer) veux-tu voir affiché pour Omore Punch dans les gestionnaires de plugins et les DAWs ? → A: `Omore` (aligner exactement avec *Omore Sampler* dans Plugoff)
- Q: Souhaites-tu renommer les fichiers/dossiers bundles distribués eux-mêmes en `Omore Punch` (et pour quels formats), ou garder les noms de fichiers actuels ? → A: Renommer les deux bundles (`Omore Punch.vst3` et `Omore Punch.clap`) sur Windows et Linux

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Affichage complet des métadonnées dans les gestionnaires de plugins (Priority: P1)

En tant que producteur ou ingénieur du son utilisant un gestionnaire de plugins (comme Plugoff sous Windows) ou un navigateur de plugins dans un DAW, je veux voir immédiatement le numéro de version, le fabricant (`Omore`) et la catégorie audio du plugin (au lieu de champs vides, `-` ou `Not Supplied`) afin d'identifier, trier et gérer le plugin facilement parmi ma collection.

**Why this priority**: C'est le besoin principal exprimé : sans ces métadonnées exposées au niveau du bundle distribué, les outils d'inventaire et de gestion de plugins affichent des informations incomplètes ou absentes.

**Independent Test**: Peut être testé de façon autonome en installant le bundle généré puis en lançant un scan dans un gestionnaire de plugins (ex. Plugoff) ou un DAW, et en vérifiant que la version, le fabricant et la catégorie apparaissent correctement dans la liste.

**Acceptance Scenarios**:

1. **Given** un bundle VST3 fraîchement compilé et installé sur le poste de l'utilisateur, **When** un gestionnaire de plugins scanne le dossier système des plugins, **Then** la colonne Version affiche la version courante du plugin, la colonne Manufacturer affiche `Omore`, et la colonne Category affiche la catégorie audio appropriée (ex. `Dynamics` / `Fx|Dynamics`).
2. **Given** une nouvelle version du plugin publiée ultérieurement, **When** le bundle est recompilé avec le nouveau numéro de version, **Then** les métadonnées affichées par le gestionnaire de plugins reflètent automatiquement cette nouvelle version sans saisie manuelle supplémentaire.

---

### User Story 2 - Renommage affiché vers « Omore Punch » sans rupture de compatibilité (Priority: P2)

En tant qu'utilisateur ayant déjà des projets DAW utilisant l'ancien nom de bundle (`omore-punch`), je veux que les bundles distribués portent le nom `Omore Punch.vst3` et `Omore Punch.clap` dans les gestionnaires de plugins et les dossiers système (sur Windows et Linux), tout en conservant l'ouverture intacte de mes projets existants et de leurs réglages.

**Why this priority**: Améliore la cohérence visuelle (comme `Omore Sampler` dans le gestionnaire de plugins) tout en garantissant qu'aucun projet musical existant ne soit cassé.

**Independent Test**: Peut être testé en créant un projet DAW avec l'ancienne version (`omore-punch`), en installant la nouvelle version nommée `Omore Punch.vst3` / `Omore Punch.clap`, puis en rouvrant le projet pour vérifier que le plugin et tous ses paramètres enregistrés sont restaurés à l'identique.

**Acceptance Scenarios**:

1. **Given** les bundles installés sous les noms `Omore Punch.vst3` et `Omore Punch.clap`, **When** un gestionnaire de plugins scanne les plugins installés, **Then** le nom affiché dans la liste est `Omore Punch` (et non `omore-punch`).
2. **Given** un projet DAW existant sauvegardé avec la version précédente du plugin (`omore-punch`), **When** l'utilisateur remplace l'ancien bundle par le nouveau bundle `Omore Punch` et rouvre le projet, **Then** le DAW reconnaît le plugin, le charge sans erreur et restaure exactement l'état et les valeurs des paramètres sauvegardés.
3. **Given** une installation précédente contenant `omore-punch.vst3` ou `omore-punch.clap`, **When** l'utilisateur exécute l'installation locale du nouveau bundle, **Then** les anciens bundles sont retirés ou remplacés proprement afin d'éviter la présence de deux entrées en double dans le DAW ou le gestionnaire de plugins.

---

### Edge Cases

- Que se passe-t-il si la version courante est une version de développement contenant un suffixe (ex. `0.3.1-dev`) ? Les métadonnées de version destinées aux formats qui n'acceptent que des numéros numériques (ex. ressources de version Windows à 4 composantes) doivent rester valides tout en conservant la version lisible complète là où les chaînes de caractères sont supportées.
- Que se passe-t-il si l'ancien bundle (`omore-punch.vst3` / `omore-punch.clap`) et le nouveau (`Omore Punch.vst3` / `Omore Punch.clap`) coexistent dans le dossier système des plugins ? L'installation locale doit nettoyer l'ancien nom pour éviter les conflits d'identifiants dans les DAWs.
- Que se passe-t-il pour les deux formats distribués (VST3 et CLAP) sous Windows et sous Linux ? Le nommage (`Omore Punch.vst3` et `Omore Punch.clap`) et les métadonnées doivent rester cohérents sur les deux systèmes d'exploitation et les deux formats.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Le bundle distribué du plugin DOIT exposer le numéro de version courant issu de la source unique de version du projet, de sorte qu'il soit lisible par les gestionnaires de plugins et les DAWs sans charger le traitement audio.
- **FR-002**: Le bundle distribué du plugin DOIT exposer le nom du fabricant `Omore` dans ses métadonnées de plugin, de bundle et d'exécutable.
- **FR-003**: Le bundle distribué du plugin DOIT exposer sa catégorie fonctionnelle (effets de dynamique / compresseur-clipper stéréo) dans les métadonnées standard lues par les gestionnaires de plugins.
- **FR-004**: Les bundles distribués et affichés dans les explorateurs de fichiers et gestionnaires de plugins DOIVENT être nommés `Omore Punch.vst3` et `Omore Punch.clap`, sur Windows et sur Linux.
- **FR-005**: Les identifiants internes uniques du plugin (identifiant de classe VST3, identifiant CLAP et identifiants de paramètres) DOIVENT rester strictement inchangés afin de préserver la rétrocompatibilité totale avec les projets DAW existants.
- **FR-006**: Les scripts d'installation locale et les artefacts de distribution DOIVENT utiliser les noms `Omore Punch.vst3` et `Omore Punch.clap` et retirer toute ancienne installation nommée `omore-punch.vst3` ou `omore-punch.clap` lors de la mise à jour locale.
- **FR-007**: Le traitement audio, l'interface utilisateur et la compatibilité Reaper sous Linux et Windows DOIVENT rester inchangés.

### Key Entities *(include if feature involves data)*

- **Bundle Plugin** : L'artefact distribuable (`Omore Punch.vst3` ou `Omore Punch.clap`) contenant le binaire du plugin et ses métadonnées descriptives (nom, version, fabricant, catégorie).
- **Identité Interne du Plugin** : L'ensemble des identifiants stables (identifiant de classe VST3, identifiant CLAP, identifiants de paramètres) utilisés par les DAWs pour associer un projet sauvegardé au plugin.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Lors d'un scan dans un gestionnaire de plugins sous Windows (tel que Plugoff), 100 % des quatre champs cibles — Nom (`Omore Punch`), Version, Manufacturer (`Omore`) et Category — s'affichent avec des valeurs renseignées et non vides (`-` ou `Not Supplied` éliminés).
- **SC-002**: 100 % des projets DAW existants créés avant le renommage s'ouvrent sans message d'erreur de plugin manquant et conservent exactement leurs réglages de paramètres.
- **SC-003**: Les builds et suites de tests existants passent à 100 % sous Linux et Windows avec les bundles générés `Omore Punch.vst3` et `Omore Punch.clap`.

## Assumptions

- Le fabricant affiché doit être `Omore`, afin de rester cohérent avec *Omore Sampler* dans les gestionnaires de plugins (la compatibilité des projets DAW existants repose sur les identifiants internes uniques de classe VST3 et CLAP qui restent inchangés).
- La catégorie affichée correspond à la nature du plugin (effet de dynamique / compresseur stéréo, cohérent avec les sous-catégories déjà déclarées dans le plugin).
- Renommer les fichiers/dossiers de bundle en `Omore Punch.vst3` et `Omore Punch.clap` sur Windows et Linux est souhaité, ce qui impliquera d'adapter la règle de nommage des bundles lors de la planification si nécessaire.
- Les outils tiers de gestion de plugins sous Windows lisent soit le manifeste standard VST3 embarqué dans le bundle, soit les informations de version intégrées au binaire Windows, soit les deux ; les deux sources doivent donc être renseignées de manière cohérente.
