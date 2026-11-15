# Bug Assessment: Troncature verticale du bas de l'interface graphique (section EQ et bas des vu-mètres)

- **Slug**: ui-bottom-clipping
- **Created**: 2026-11-14
- **Source**: pasted text + capture d'écran jointe (`petit bug graphique`, Reaper sous Linux Debian 13)
- **Verdict**: valid
- **Severity**: medium

## Report (verbatim or summarized)

Texte fourni par l'utilisateur :
> `petit bug graphique`

Environnement précisé par l'utilisateur :
- **Hôte / OS** : Reaper sous Linux (Debian 13), à la taille de fenêtre ouverte par le plugin.

Capture d'écran jointe (`OMORE PUNCH v0.2.1 [DEV a6fa972-dirty]`, dimensions de l'image `827x571` px) :
- Dans la colonne de gauche, la section `3. DRUM BUS EQ` (`Low Cut`, `500Hz Cut`, `High Shelf`) est coupée verticalement juste sous le cercle des trois potentiomètres bleus : les valeurs numériques formatées, les boutons compacts `-` / `+` / `Reset` et la bordure inférieure du groupe `3.` sont tronqués et inaccessibles.
- Dans la colonne de droite (`4. GAIN REDUCTION & STEREO BUS METERING`), les labels `-inf dB` situés sous les vu-mètres `In L/R` et `Out L/R` sont coupés horizontalement en deux au ras du bord inférieur de la fenêtre, et la bordure basse du vu-mètre `GR (0 to -24 dB)` touche le bord inférieur.

## Symptom

Lors de l'ouverture de l'éditeur graphique d'`Omore Punch v0.2.1-dev` dans Reaper sous Linux (Debian 13), la fenêtre s'ouvre avec une zone cliente d'environ `820x560` px (`827x571` px avec les bordures de fenêtre sur la capture) au lieu des `820x620` px attendus (`UI_WINDOW_WIDTH x UI_WINDOW_HEIGHT`). Ce déficit de ~60 px en hauteur tronque le bas de la section `3. DRUM BUS EQ` (valeurs et boutons `-` / `+` / `Reset`) ainsi que le bas des labels `-inf dB` des vu-mètres `In L/R` et `Out L/R`.

## Reproduction

1. Avoir une session Reaper, un FX chain, un template ou un preset sauvegardé avec une version antérieure d'`Omore Punch` (`v0.2.0` en `820x560` px, ou `v0.2.1-dev` pré-EQ en `820x480` px), ou laisser Reaper restaurer l'état persisté `editor-state` du plugin.
2. Charger le build `v0.2.1-dev` incluant la section `3. DRUM BUS EQ` (`src/lib.rs`, `UI_WINDOW_HEIGHT = 620`) dans Reaper sous Linux (Debian 13).
3. Ouvrir la fenêtre de l'éditeur graphique du plugin : la fenêtre reste contrainte à `~820x560` px et la moitié basse de la section `3. DRUM BUS EQ` ainsi que le bas des labels `-inf dB` sont coupés sans possibilité de redimensionner la fenêtre manuellement.

## Suspected Code Paths

- `src/lib.rs:697-698` — `OmorePunchParams::editor_state` est annoté `#[persist = "editor-state"] pub editor_state: Arc<EguiState>`.
- `src/lib.rs:751` — `Default::default()` initialise `editor_state: EguiState::from_size(UI_WINDOW_WIDTH, UI_WINDOW_HEIGHT)` avec `UI_WINDOW_WIDTH = 820` et `UI_WINDOW_HEIGHT = 620` (`src/lib.rs:1095-1096`).
- `nih_plug_egui/src/lib.rs:80-91` (`impl PersistentField<'a, EguiState> for Arc<EguiState>`) — `set()` exécute `self.size.store(new_value.size.load())`, ce qui écrase silencieusement `(820, 620)` par la taille sauvegardée dans l'état DAW (ex. `(820, 560)` en `v0.2.0`) lors du rappel de session/preset (`set_state`).
- `src/lib.rs:992-1004` (`OmorePunch::editor`) — Utilise `create_egui_editor(...)` sans `ResizableWindow` et sans garde-fou sur la taille minimale ni zone de défilement : une fois `editor_state.size` écrasé à `(820, 560)`, la fenêtre s'ouvre à `820x560` et l'utilisateur ne peut pas l'agrandir.
- `src/lib.rs:1607-1978` (`draw_omore_punch_ui`) — Le `CentralPanel` empile les sections `1.`, `2.` et `3.` à gauche (~552 px dans le panneau central, ~600 px au total avec le bandeau supérieur) et `UI_METER_HEIGHT = 390.0` à droite sans repli adaptatif (ni `ScrollArea` de secours) si le viewport réel est inférieur à `620` px de haut.
- `src/lib.rs:2723-2879` (`test_ui_layout_column_widths_fit_window`) — Ne teste le rendu `egui` qu'à la taille nominale `820x620` (qui passe), sans vérifier l'absence de régression liée à un état persisté `editor-state` plus petit (ex. `820x560` ou `820x480`).

## Root Cause Hypothesis

La cause principale réside dans la persistance de `editor_state` combinée à une fenêtre à taille fixe dont la hauteur minimale requise a augmenté entre versions (confiance : **high**) :

1. **Écrasement de `UI_WINDOW_HEIGHT` par `#[persist = "editor-state"]` (`src/lib.rs:697-698`) :** L'interface d'`Omore Punch` n'est pas redimensionnable par l'utilisateur, mais `OmorePunchParams` persiste `editor_state: Arc<EguiState>` dans l'état du plugin. Lorsqu'une session Reaper (ou un preset/FX chain/template) créée avant `003-drum-bus-eq` (par ex. `v0.2.0` en `820x560`, ce qui correspond exactement à la capture `827x571` avec les bordures de fenêtre Reaper Linux) est rechargée, `nih-plug` désérialise `"editor-state"` et `PersistentField::set` écrase `(820, 620)` par `(820, 560)` sans vérifier la taille minimale requise pour afficher la section `3. DRUM BUS EQ` et les vu-mètres `UI_METER_HEIGHT = 390.0`.
2. **Absence de filet de sécurité dans `draw_omore_punch_ui` (`src/lib.rs:1607`) :** Si l'hôte (via un état persisté ou un cache de géométrie de fenêtre Reaper) impose malgré tout une hauteur inférieure à `620` px, `CentralPanel` coupe simplement le bas des deux colonnes sans barre de défilement ni redimensionnement.

## Proposed Remediation

**Preferred**:
1. **Ne plus persister une taille obsolète pour une UI non redimensionnable (ou imposer un plancher minimal) :**
   - Option A (la plus simple et propre pour une UI à taille fixe) : retirer `#[persist = "editor-state"]` de `OmorePunchParams` et déplacer `editor_state: Arc<EguiState>` directement sur la struct `OmorePunch` (`src/lib.rs:925-929`), initialisée à `EguiState::from_size(UI_WINDOW_WIDTH, UI_WINDOW_HEIGHT)`. Comme `nih-plug` ignore les champs persistés inconnus lors du chargement d'anciennes sessions, toute session `v0.2.0` / `v0.2.1-dev` s'ouvrira immédiatement en `820x620` sans régression.
   - Option B (si l'on souhaite conserver le champ `"editor-state"` dans `OmorePunchParams` pour une future UI redimensionnable) : encapsuler `Arc<EguiState>` dans un wrapper `PersistentField` qui borne la taille restaurée à `(w.max(UI_WINDOW_WIDTH), h.max(UI_WINDOW_HEIGHT))` dans `PersistentField::set(...)` via `EguiState::from_size(...)` (note : le champ `size` de `EguiState` étant privé, le wrapper devrait stocker une `Arc<EguiState>` remplaçable ou recréer l'état avant `editor()`).
2. **Filet de sécurité dans `draw_omore_punch_ui` (optionnel mais recommandé) :**
   - Envelopper le contenu du `CentralPanel` dans `egui::ScrollArea::vertical().auto_shrink([false, false])` : à la taille nominale `820x620`, aucune barre de défilement n'apparaît (vérifié par test), mais si un hôte contraint la fenêtre en hauteur (ex. petit écran ou cache hôte Reaper), tous les contrôles de `3. DRUM BUS EQ` restent accessibles.

**Alternatives**:
- Réduire les espacements/hauteurs de cellules (`UI_KNOB_CELL_HEIGHT`, `UI_METER_HEIGHT`) pour tout faire tenir dans `820x560` : déconseillé car cela compresserait inutilement les potards SSL et leurs boutons `-` / `+` / `Reset`, alors que `820x620` convient déjà très bien et que `v0.2.1-dev` pré-EQ était même à `820x480` (qui resterait cassé si `"editor-state"` à `480` px était restauré).

**Files likely to change**:
- `src/lib.rs` (déplacement de `editor_state` hors de `OmorePunchParams` ou garde-fou sur la taille minimale, plus éventuel `ScrollArea` de secours dans `draw_omore_punch_ui`, et tests unitaires)
- `CHANGELOG.md` (entrée `Fixed` sous `## [Unreleased]`, conformément à `GEMINI.md` section 2.3)

**Tests to add or update**:
- Ajouter un test unitaire dans `mod tests` de `src/lib.rs` vérifiant qu'après un cycle `get_state()` / `set_state()` (ou chargement d'un état `PluginState` contenant un ancien `"editor-state"` `820x560` ou `820x480`), `plugin.editor(...).size()` reste bien `(UI_WINDOW_WIDTH, UI_WINDOW_HEIGHT)` (`(820, 620)`).
- Conserver `test_ui_layout_column_widths_fit_window` pour garantir qu'à `820x620`, l'ensemble de l'UI tient sans débordement ni barre de défilement.

## Risks & Considerations

- **Compatibilité des sessions DAW (`GEMINI.md` / `specs/003-drum-bus-eq`) :** Déplacer `editor_state` de `OmorePunchParams` vers `OmorePunch` ne modifie aucun `#[id = "..."]` de paramètre DSP/automation ni les IDs VST3/CLAP ; `nih-plug` ignore proprement l'ancien champ `"editor-state"` présent dans les sauvegardes antérieures.
- **Cache géométrique côté hôte (Reaper) :** Même après correction du plugin, Reaper peut parfois mémoriser les dimensions d'une fenêtre FX flottante déjà ouverte dans un `.rpp` existant (selon les réglages Reaper VST3/CLAP). Le `ScrollArea::vertical()` de secours garantit que même dans ce cas extrême, les contrôles `3. DRUM BUS EQ` restent utilisables, et fermer/rouvrir ou réinsérer le FX réapplique `820x620`.
- **Conformité `src/lib.rs` (`GEMINI.md`) :** Tout ajout/modification dans `src/lib.rs` doit rester 100 % ASCII pur (`LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs`) et passer `cargo test --release` et le bundle VST3/CLAP.

## Open Questions

- Aucune ambiguïté bloquante : la capture d'écran (`827x571` px, soit `820x560` utiles) combinée à `#[persist = "editor-state"]` dans `src/lib.rs:697-698` et `PersistentField::set` dans `nih_plug_egui` explique directement la troncature observée. (En vérification post-fix, il faudra confirmer dans Reaper sous Linux si Reaper réapplique de lui-même `820x620` sur la session affectée ou si une `ScrollArea` de secours est également utile.)
