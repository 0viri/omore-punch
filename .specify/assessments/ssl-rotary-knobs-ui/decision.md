# Decision: Passage aux potentiomètres rotatifs en grille et look type SSL

- **Slug**: ssl-rotary-knobs-ui
- **Decided**: 2026-11-14
- **Verdict**: go
- **Artifacts reviewed**: intake.md | research.md | problem.md | concept.md

## Scorecard

| Criterion | Rating | Justification |
|-----------|--------|---------------|
| Problem validity | adequate | Les 9 curseurs horizontaux (~442 px de large chacun) donnent un aspect formulaire peu ergonomique pour un compresseur de bus et ont déjà causé le bug `ui-columns-overlap` (`problem.md`, `research.md`). |
| Evidence strength | adequate | Demande directe du mainteneur/utilisateur principal (`tdavio`) étayée par l'analyse factuelle du code UI (`src/lib.rs`) et de l'historique des bugs (`research.md`, confiance globale `medium`). |
| Value vs. inaction | adequate | Bien que le statu quo n'affecte pas le DSP, l'Option A (`small`) apporte un gain net de lisibilité angulaire et de compacité spatiale pour un effort maîtrisé (`problem.md`, `concept.md`). |
| Feasibility / appetite | strong | `src/lib.rs` utilise déjà `egui::Painter` pour des jauges vectorielles sur mesure ; dessiner des boutons rotatifs 100 % ASCII sans dépendance externe tient dans un budget `small` (`research.md`, `concept.md`). |
| Strategic fit | adequate | Respecte les Principes fondamentaux I à VI (monolithe `src/lib.rs`, 100 % ASCII, zéro impact temps réel), sous réserve de mettre à jour la clause GUI de `.specify/memory/constitution.md` (`TODO(GUI_STANDARDS)`) et `GEMINI.md` section 4 via `/speckit-constitution`. |
| Risk posture | strong | Les risques majeurs (perte de précision pas-à-pas, débordement de la fenêtre `820x480` px, complexité graphique) sont identifiés et contenus par le cadrage de l'Option A (`concept.md`). |

## Verdict & Rationale

**Verdict : `go`** (pour l'**Option A — Potentiomètres rotatifs en grille type SSL dans la structure 2 colonnes actuelle**).

Tous les critères de passage sont remplis : la validité du problème (`adequate`) et la solidité des preuves (`adequate`) sont établies, la faisabilité technique dans `egui 0.31.1` est élevée (`strong`) pour un appétit `small`, et la maîtrise des risques est solide (`strong`). En limitant la refonte à la grille de potentiomètres rotatifs type SSL dans la colonne de gauche tout en conservant le bandeau supérieur, les préréglages rapides et le metering vertical de droite, l'Option A répond directement au besoin utilisateur sans alourdir démesurément `src/lib.rs`.

## If go — Handoff to `/speckit-specify`

- **Problem**: L'empilement vertical de 9 larges curseurs horizontaux (~442 px) dans `Omore Punch` donne l'aspect d'un formulaire générique, consomme toute la largeur de la colonne gauche et prive l'utilisateur du repérage angulaire immédiat propre aux compresseurs de studio type SSL.
- **Chosen approach**: **Option A — Potentiomètres rotatifs en grille type SSL dans la structure 2 colonnes actuelle** (remplacement des 9 sliders horizontaux par des boutons ronds vectoriels `egui::Painter` disposés en grille dans les sections `1.` et `2.`, avec couronne graduée, capuchon coloré style SSL, repère angulaire, étiquette, valeur numérique ASCII et contrôle fin/reset).
- **In scope / out of scope**:
  - *In scope* : Widget de potentiomètre rotatif vectoriel 100 % ASCII dans `src/lib.rs` ; disposition en grille des 9 paramètres continus (`Input Drive`, `Threshold`, `Ratio`, `Attack`, `Release`, `SC HPF`, `Makeup Gain`, `Clip Ceiling`, `Mix Dry/Wet`) ; codes couleurs de capuchons / esthétique type SSL ; mécanisme de réglage fin et de remise à zéro ; mise à jour du test unitaire de géométrie UI pour `820x480` px.
  - *Out of scope* : Modification du moteur DSP (`StereoPunchDsp`) ou des identifiants/plages de paramètres VST3/CLAP ; remplacement des vu-mètres verticaux de la colonne droite (`In L/R`, `Out L/R`, `GR` style Reaper) par un VU-mètre à aiguille ; images externes, polices non-ASCII ou découpage de `src/lib.rs`.
- **Success metrics**:
  - 0 débordement ou chevauchement entre les colonnes dans la fenêtre `820x480` px (vérifié par test unitaire).
  - 9/9 paramètres continus dotés d'un bouton rotatif avec lecture angulaire, valeur numérique ASCII formatée, réglage fin, remise à la valeur par défaut et notification d'automation DAW complète (`begin_set_parameter` / `set_parameter` / `end_set_parameter`).
  - Validation visuelle et ergonomique par l'utilisateur principal dans Reaper.
- **Carried-forward open questions**:
  - Palette exacte du « look type SSL » : conserver le fond sombre actuel avec capuchons colorés SSL par fonction, ou adopter aussi un fond de panneau gris anthracite type console SSL ?
  - Ergonomie de précision et reset sur chaque bouton rond : conserver de petits boutons `-` / `+` / `Reset` ou utiliser double-clic pour `Reset` + modificateur clavier (`Shift`) / molette pour le pas-à-pas ?
  - Organisation exacte de la grille 2D dans les sections `1.` (7 paramètres continus + 3 boutons rapides) et `2.` (3 boutons de mode clipper + 2 paramètres continus).
  - Amendement préalable ou coordonné de `.specify/memory/constitution.md` et `GEMINI.md` (section 4) via `/speckit-constitution`.
