# Bug Assessment: Chevauchement des colonnes UI et troncature des boutons Reset

- **Slug**: ui-columns-overlap
- **Created**: 2026-11-14
- **Source**: pasted text + capture d'écran jointe (`problème d'affichage`)
- **Verdict**: valid
- **Severity**: medium

## Report (verbatim or summarized)

Texte fourni par l'utilisateur :
> `problème d'affichage`

Capture d'écran jointe (interface `OMORE PUNCH v0.2.0` ouverte dans l'hôte) :
- Dans la colonne de gauche (`1. COMPRESSOR & ENVELOPE BALLISTICS` et `2. OUTPUT CLIPPER & PARALLEL MIX`), les boutons `Reset` situés à l'extrémité droite de chaque ligne de paramètre (`Input Drive`, `Threshold`, `Ratio`, `Attack`, `Release`, `SC HPF`, `Makeup Gain`, `Clip Ceiling`, `Mix Dry/Wet`) débordent hors de leur colonne et passent sous la bordure gauche de la colonne de droite (`3. GAIN REDUCTION & STEREO BUS METERING`) ainsi que sous l'encadré `GR: -00.0 dB / Max GR: -00.0 dB`.
- Un espace vide important (~55 px de chaque côté) apparaît autour du rail de chaque slider entre les boutons `-` et `+`.
- Un espace vide vertical important subsiste également en bas de la fenêtre sous les deux colonnes.

## Symptom

Dans l'interface graphique `egui` (`820x560`), chaque ligne de paramètre de la colonne de gauche dépasse la largeur allouée à cette colonne (~398 px pour un partage 50/50 via `ui.columns(2, ...)`), ce qui provoque le chevauchement des boutons `Reset` et de la bordure des groupes 1 et 2 avec le panneau de droite (`3. GAIN REDUCTION & STEREO BUS METERING`). Il était attendu que les deux colonnes soient côte à côte sans collision visuelle ni troncature des boutons `Reset`, avec des sliders occupant proprement l'espace entre `-` et `+`.

## Reproduction

1. Compiler et charger le plugin `Omore Punch` (VST3 ou CLAP) dans Reaper.
2. Ouvrir la fenêtre de l'éditeur graphique (`820x560`).
3. Observer l'intersection entre la colonne de gauche (sections `1.` et `2.`) et la colonne de droite (section `3.`) : les boutons `Reset` sont coupés par la bordure gauche de la section `3.` et recouverts par l'afficheur `GR: -00.0 dB`.

## Suspected Code Paths

- `src/lib.rs:454` — `editor_state: EguiState::from_size(820, 560)` définit la largeur totale de la fenêtre à `820` px et la hauteur à `560` px.
- `src/lib.rs:772-847` (`draw_param_row`) — chaque ligne empile horizontalement : un label monospace de 14 caractères (~105-112 px), un bouton `-` (`28.0` px), un slider dans `ui.add_sized([210.0, 22.0], egui::Slider::new(...))` (`210.0` px réservés alors que `ui.spacing().slider_width` vaut ~100 px par défaut, créant ~55 px de vide de chaque côté du rail), un bouton `+` (`28.0` px), une valeur formatée sur 10 caractères (~80 px) et un bouton `Reset` (`52.0` px), soit **~550-560 px** au total avec les espacements et marges de groupe.
- `src/lib.rs:1062-1063` (`draw_omore_punch_ui`) — `ui.columns(2, |cols| ...)` divise le `CentralPanel` en deux colonnes de largeur strictement égale (50 % / 50 %, soit ~398 px chacune sur une fenêtre de 820 px), ce qui est insuffisant pour les ~550 px requis par `draw_param_row` (et juste pour la rangée de 3 boutons `[Punch 3ms / 80ms] [Kick Safe 90Hz] [Auto Makeup: ON]` aux lignes `1116-1180`), tandis que la colonne de droite (`cols[1]`, lignes `1261-1319`) n'utilise qu'environ 280-310 px.

## Root Cause Hypothesis

Le problème combine deux causes dans `src/lib.rs` (confiance : **high**) :
1. **Largeur réservée excessive dans `draw_param_row` (`src/lib.rs:805-808`) :** L'appel `ui.add_sized([210.0, 22.0], egui::Slider::new(...))` alloue une boîte de `210.0` px de large sans modifier `ui.spacing_mut().slider_width` (qui reste à sa valeur par défaut de `100.0` px dans `egui`). En conséquence, `egui` centre un rail de 100 px au milieu de 210 px (créant deux vides inutiles de ~55 px autour du slider) tout en faisant avancer le curseur de `210.0` px, ce qui porte la largeur d'une ligne `draw_param_row` à ~555 px.
2. **Partage 50/50 rigide via `ui.columns(2, ...)` (`src/lib.rs:1063`) :** `ui.columns(2, ...)` attribue 50 % de la largeur (~398 px) à `cols[0]` et 50 % (~398 px) à `cols[1]`. Comme `egui::Ui::group` et `ui.columns` ne rognent pas (`clip`) les widgets enfants qui dépassent la largeur minimale mais positionnent `cols[1]` à `x ≈ 414 px`, le contenu de `cols[0]` (~555 px) déborde de ~155 px dans `cols[1]`.

## Proposed Remediation

**Preferred**:
1. Dans `draw_param_row` (`src/lib.rs:772-847`), fixer explicitement `ui.spacing_mut().slider_width` (par ex. `130.0` à `150.0` px) et utiliser `ui.add(egui::Slider::new(...))` (ou un `add_sized` cohérent avec `slider_width`) ainsi qu'un formatage compact (`{:<12}` pour le label, `{:>9}` pour la valeur) afin que le rail du slider occupe tout l'espace entre `-` et `+` sans vide parasite, ramenant la largeur totale d'une ligne de contrôle autour de ~450-470 px.
2. Dans `draw_omore_punch_ui` (`src/lib.rs:1062-1321`), remplacer le partage 50/50 de `ui.columns(2, ...)` par une disposition asymétrique adaptée au contenu (par exemple `egui::SidePanel::right` ou un `ui.horizontal_top` avec deux `ui.vertical` de largeurs explicites, ex. ~490 px pour la colonne de gauche et ~300 px pour la colonne de droite, ou élargir légèrement `EguiState::from_size` à `860x470` / ajuster la hauteur pour supprimer la bande vide en bas de fenêtre).

**Alternatives** (optional):
- Conserver `ui.columns(2, ...)` mais augmenter la largeur de la fenêtre dans `EguiState::from_size` à ~`980x480` tout en corrigeant `slider_width` : plus simple (2 lignes modifiées), mais laisse un grand espace vide à droite des vu-mètres dans la colonne 2 qui n'a besoin que de ~300 px.
- Utiliser `egui::SidePanel::right("metering_panel").resizable(false).exact_width(310.0)` pour la section `3. GAIN REDUCTION & STEREO BUS METERING` et laisser le `CentralPanel` (~490-510 px) accueillir les sections `1.` et `2.` : garantit par construction dans `egui` que les deux panneaux ne peuvent jamais se chevaucher.

**Files likely to change**:
- `src/lib.rs`
- `CHANGELOG.md`

**Tests to add or update**:
- Extraire ou exposer les constantes/calculs de mise en page UI (ou vérifier par test unitaire dans `mod tests` de `src/lib.rs`) que la somme des largeurs des colonnes et marges reste inférieure ou égale à la taille de fenêtre `EguiState::from_size`, en complément des contrôles obligatoires (`LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs`, `cargo test --release`, et validation visuelle dans Reaper).

## Risks & Considerations

- **Persistance de `EguiState` dans les projets DAW existants :** `EguiState::from_size(width, height)` est sérialisé par `nih_plug_egui` avec l'état du plugin. Si la correction repose uniquement sur l'agrandissement de `from_size(...)`, une instance déjà sauvegardée dans un projet Reaper pourrait conserver l'ancienne taille `820x560`. Il est donc indispensable que la mise en page des deux colonnes rentre proprement dans `820` px de large sans chevauchement.
- **Conformité ASCII & Règles UI (`GEMINI.md` sections 2.2 et 4) :** Tous les textes (`Reset`, `[Punch 3ms / 80ms]`, `[Kick Safe 90Hz]`, `[Auto Makeup: ON]`, etc.) et la structure en 2 colonnes avec `draw_param_row` doivent rester 100 % ASCII et conserver tous leurs contrôles.

## Open Questions

- Aucune ambiguïté bloquante : la capture d'écran et la lecture de `src/lib.rs` identifient directement la cause exacte du chevauchement.

