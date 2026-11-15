# Problem Definition: Ergonomie et lisibilité visuelle des contrôles façon console/compresseur SSL

- **Slug**: ssl-rotary-knobs-ui
- **Created**: 2026-11-14
- **Inputs used**: intake.md | research.md

## Problem Statement

Lors du réglage d'`Omore Punch` sur un bus batterie dans Reaper, l'empilement vertical de 9 curseurs horizontaux donne à l'interface l'apparence d'un formulaire logiciel générique plutôt que d'un compresseur de studio, occupe une largeur importante par ligne (~442 px, source récente de collision entre colonnes) et n'offre pas le repérage visuel angulaire immédiat auquel les utilisateurs de processeurs matériels type SSL sont habitués.

## Affected Users & Stakeholders

- **Users**: Ingénieur du son / producteur mixant un bus batterie stéréo dans Reaper (`tdavio`, utilisateur principal identifié dans `intake.md` et `research.md`) — doit parcourir 9 barres horizontales empilées au lieu de lire d'un coup d'œil la position angulaire et le groupement spatial des réglages.
- **Stakeholders**: Mainteneur du projet `Omore Punch` (`tdavio`) — arbitre l'identité visuelle du plugin, la maintenabilité du fichier monolithique `src/lib.rs` et la cohérence avec la constitution du projet (`.specify/memory/constitution.md` et `GEMINI.md` section 4).

## Goals

- Offrir une lecture visuelle immédiate (par position angulaire et regroupement spatial) et une esthétique de compresseur de studio type SSL pour les 9 paramètres continus du plugin.
- Réduire l'empreinte horizontale unitaire des contrôles afin que les sections de paramètres et de metering tiennent confortablement et sans tension dans la fenêtre de `820x480` px.
- Conserver intégralement la précision de réglage, la capacité de remise à la valeur par défaut, la lisibilité des valeurs numériques (100 % ASCII) et la compatibilité d'automation hôte (VST3/CLAP).

## Non-Goals

- Modifier le traitement audio numérique (`StereoPunchDsp`), la chaîne DSP ou les performances temps réel du thread audio.
- Ajouter, supprimer ou modifier les identifiants et plages des paramètres exposés au DAW (aucune rupture de compatibilité avec les projets Reaper existants).
- Introduire des ressources bitmap externes, des caractères non-ASCII ou découper `src/lib.rs` en sous-modules.

## Success Metrics

- **Absence de débordement de mise en page** : 0 chevauchement entre les groupes de contrôles et la section metering dans une fenêtre de `820x480` px, vérifié par test unitaire de géométrie UI (baseline : 9 lignes horizontales de ~442 px dans une colonne de `480.0` px après le correctif `ui-columns-overlap`).
- **Maintien des capacités de contrôle précis** : 9/9 paramètres continus permettent un ajustement fin, une remise à zéro déterministe, l'affichage numérique formaté de la valeur courante et la notification complète à l'hôte DAW (baseline : 9/9 paramètres via `draw_param_row`).
- **Adéquation visuelle et ergonomique *(qualitative)*** : validation par l'utilisateur principal que l'interface procure le repérage visuel et le ressenti d'un processeur type SSL en remplacement des faders horizontaux (baseline : insatisfaction exprimée dans `intake.md`).

## Cost of Inaction

Si rien n'est modifié, le plugin conserve exactement ses performances audio, sa stabilité temps réel et sa compatibilité VST3/CLAP actuelles (`research.md`). Le coût de l'inaction est uniquement ergonomique et esthétique : l'interface garde une présentation en liste de curseurs horizontaux moins compacte et moins familière qu'une façade de compresseur de bus type SSL.

## Open Questions

- [NEEDS CLARIFICATION: Le « look type SSL » vise-t-il uniquement le passage à des contrôles rotatifs avec repères angulaires sur le fond sombre actuel d'`Omore Punch`, ou également une refonte de la palette de couleurs (façade gris/anthracite, codes couleurs des capuchons par fonction) ?]
- [NEEDS CLARIFICATION: Comment l'utilisateur souhaite-t-il effectuer les réglages fins au pas-à-pas et la remise à zéro sur les contrôles rotatifs (conservation des boutons `-` / `+` / `Reset`, ou double-clic + modificateur clavier) ?]
- [NEEDS CLARIFICATION: La section de metering (`3. GAIN REDUCTION & STEREO BUS METERING`) doit-elle conserver ses bargraphes verticaux actuels (`In L/R`, `Out L/R`, `GR` descendant style Reaper) ou évoluer visuellement ?]
- [NEEDS CLARIFICATION: Le mainteneur souhaite-t-il amender formellement la constitution (`.specify/memory/constitution.md` et `GEMINI.md` section 4) qui impose actuellement les curseurs horizontaux et la palette `Omore Sampler` ?]
