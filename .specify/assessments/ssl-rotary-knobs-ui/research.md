# Idea Research: Interface type SSL avec potentiomètres rotatifs à la place des faders horizontaux

- **Slug**: ssl-rotary-knobs-ui
- **Created**: 2026-11-14
- **Evidence confidence (overall)**: medium

## Users & Demand

- La demande provient directement du mainteneur et utilisateur principal du projet (`tdavio`) lors de la capture d'idée initiale : remplacer les curseurs horizontaux actuels par des boutons ronds (potentiomètres rotatifs) avec un look type SSL — [source: `.specify/assessments/ssl-rotary-knobs-ui/intake.md:8-14`, `cited`] (confidence: high)
- Aucune donnée d'usage externe, ticket utilisateur tiers ou étude comparative n'est disponible dans le dépôt pour quantifier la demande au-delà du souhait exprimé par l'auteur — [source: `ASSUMPTION`] (confidence: low)
- L'empilement actuel de 9 lignes de curseurs horizontaux (`draw_param_row`) a déjà généré de la friction ergonomique et de mise en page, documentée dans un bug de chevauchement entre la colonne des contrôles et celle du metering — [source: `.specify/bugs/ui-columns-overlap/assessment.md:14-40`, `cited`] (confidence: high)

## Prior Art

- **Implémentation actuelle dans `Omore Punch` (`draw_param_row`)** : les 9 paramètres continus (`Input Drive`, `Threshold`, `Ratio`, `Attack`, `Release`, `SC HPF`, `Makeup Gain`, `Clip Ceiling`, `Mix Dry/Wet`) sont affichés sous forme de lignes horizontales comprenant un label monospace de 12 caractères, un bouton `-`, un `egui::Slider` de `140.0` px, un bouton `+`, une lecture numérique sur 9 caractères et un bouton `Reset` — [source: `src/lib.rs:784-862`, `src/lib.rs:1100-1283`, `cited`] (confidence: high)
- **Correctif récent de largeur de colonne (`ui-columns-overlap`)** : chaque ligne horizontale `draw_param_row` occupe ~442 px de large (`src/lib.rs:1766-1786`), ce qui a imposé une colonne gauche de `480.0` px sur une fenêtre de `820x480` px et l'ajout du test unitaire `test_ui_layout_column_widths_fit_window` — [source: `.specify/bugs/ui-columns-overlap/assessment.md:32-45`, `src/lib.rs:758-768`, `src/lib.rs:1754-1787`, `cited`] (confidence: high)
- **Dessin vectoriel personnalisé existant dans `Omore Punch`** : le projet utilise déjà `ui.allocate_exact_size` et `egui::Painter` (`rect_filled`, `rect_stroke`, `line_segment`, `text`) dans `src/lib.rs` pour dessiner les vu-mètres `draw_vertical_level_meter` et `draw_reaper_style_gr_meter` sans dépendance externe ni image — [source: `src/lib.rs:864-1012`, `cited`] (confidence: high)
- **Capacités de `egui 0.31.1` et `nih_plug_egui`** : la bibliothèque `egui 0.31.1` utilisée par le projet (`Cargo.lock:531-534`) fournit `Slider` et `DragValue` mais pas de widget de potentiomètre rotatif natif ; un bouton rotatif requiert donc un tracé vectoriel (`Painter::circle_filled`, `circle_stroke`, `line_segment`) et une gestion d'interaction `Sense::click_and_drag()` — [source: `Cargo.lock:531-534`, `Cargo.lock:1098-1103`, `src/lib.rs:820-833`, `cited`] (confidence: high)
- **Codes visuels des compresseurs de bus type SSL (ex. SSL G-Master Bus Compressor / tranches SSL 4000)** : dans l'industrie audio, ces processeurs disposent leurs réglages (`Threshold`, `Make-Up`, `Attack`, `Release`, `Ratio`) sous forme de potentiomètres rotatifs à capuchons colorés sur une façade gris/anthracite avec graduations angulaires autour de chaque bouton — [source: `ASSUMPTION`] (confidence: medium)

## Market & Context

- **Solution actuelle utilisée** : l'utilisateur règle les paramètres via les sliders horizontaux `egui`, les boutons d'incrément `-`/`+`, le bouton `Reset` et les boutons de préréglages rapides (`[Punch 3ms / 80ms]`, `[Snare Safe 200Hz]`, `[Auto Makeup: ON]`) — [source: `src/lib.rs:784-862`, `src/lib.rs:1140-1204`, `cited`] (confidence: high)
- **Habitudes des utilisateurs de DAW (Reaper)** : les ingénieurs du son travaillant sur un bus batterie ont l'habitude de manipuler des potentiomètres rotatifs disposés en grille sur les émulations de compresseurs matériels, ce qui permet de regrouper visuellement les paramètres en 2D plutôt qu'en longue liste verticale de 9 barres horizontales — [source: `ASSUMPTION`] (confidence: medium)
- **Coût du statu quo (ne rien faire)** : nul sur le plan du traitement audio (DSP), de la stabilité temps réel et de la compatibilité VST3/CLAP ; le coût se limite à une ergonomie et une identité visuelle moins proches d'un compresseur matériel de studio — [source: `src/lib.rs:251-380`, `ARCHITECTURE.md:12-54`, `cited`] (confidence: high)

## Data & Constraints

- **Architecture monolithique obligatoire (Principe I)** : tout le code du widget de potentiomètre rotatif et de la mise en page doit résider dans le bloc 4 (`draw_omore_punch_ui`) du fichier unique `src/lib.rs` (actuellement 1 788 lignes), sans création de sous-module (`gui.rs`, `widgets.rs`) — [source: `.specify/memory/constitution.md:5-23`, `GEMINI.md:14-18`, `cited`] (confidence: high)
- **Source 100 % ASCII (Principe II)** : aucun caractère Unicode non-ASCII (ni symbole de degré `°`, ni flèche, ni caractère accentué) ne peut être utilisé dans `src/lib.rs` pour les graduations ou étiquettes des boutons ronds, sous peine d'échec du contrôle CI `LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs` — [source: `.specify/memory/constitution.md:24-33`, `GEMINI.md:19-22`, `cited`] (confidence: high)
- **Conformité à la Constitution et à `GEMINI.md` (Section 4)** :
  - La constitution stipule actuellement : `GUI: dark theme inspired by Omore Sampler, applied through apply_omore_dark_theme, and ASCII-only text (Principle II)` avec un `TODO(GUI_STANDARDS)` — [source: `.specify/memory/constitution.md:99-101`, `cited`] (confidence: high)
  - `GEMINI.md` section 4 impose explicitement la palette sombre `Omore Sampler` (`rgb(18, 19, 24)`, ambre/or `rgb(225, 145, 25)`) et l'utilisation de `draw_param_row` (`label aligné, bouton -, slider, bouton +, lecture numérique formatée, bouton Reset`) — [source: `GEMINI.md:62-74`, `cited`] (confidence: high)
  - Tout remplacement du slider par un potentiomètre rotatif ou changement de palette vers un thème SSL nécessitera une mise à jour coordonnée de `GEMINI.md` et de `.specify/memory/constitution.md` via `/speckit-constitution` — [source: `.specify/memory/constitution.md:122-128`, `GEMINI.md:5`, `cited`] (confidence: high)
- **Persistance de la taille de fenêtre `EguiState` (`820x480`)** : `EguiState::from_size(820, 480)` est sérialisé dans les sessions DAW ; la nouvelle disposition à boutons ronds doit tenir dans `820x480` px sans chevauchement pour rester compatible avec les projets Reaper existants — [source: `src/lib.rs:454`, `src/lib.rs:758-759`, `.specify/bugs/ui-columns-overlap/assessment.md:60`, `cited`] (confidence: high)
- **Neutralité vis-à-vis du thread audio temps réel (Principe III)** : la modification concerne exclusivement le rendu `egui` et les appels `ParamSetter` (`begin_set_parameter` / `set_parameter` / `end_set_parameter`) sur le thread UI ; elle n'impacte ni `process()` ni `StereoPunchDsp` — [source: `.specify/memory/constitution.md:34-56`, `src/lib.rs:813-833`, `cited`] (confidence: high)

## Evidence Against the Idea

- **Conflit direct avec les règles UI codifiées du projet** : `GEMINI.md` (section 4) et `.specify/memory/constitution.md` (lignes 99-101) prescrivent explicitement le thème sombre `Omore Sampler` et le composant `draw_param_row` à slider horizontal. Changer ce standard demande un amendement formel de la constitution avant ou pendant la spécification — [source: `GEMINI.md:62-74`, `.specify/memory/constitution.md:99-101`, `cited`] (confidence: high)
- **Risque de perte de précision et d'accessibilité des réglages pas-à-pas** : l'actuel `draw_param_row` offre des boutons `-` et `+` avec des pas déterministes (`0.5 dB`, `0.25`, `0.5 ms`, `5.0 ms`, `5.0 Hz`, `5.0 %`) ainsi qu'un bouton `Reset` explicite (`src/lib.rs:807-860`). Si les boutons ronds suppriment ces contrôles auxiliaires sans alternative claire (ex. double-clic pour reset, maintien de `Shift` pour réglage fin, ou conservation des boutons `-`/`+`/`Reset`), le réglage précis au dixième de dB ou de milliseconde deviendra plus difficile à la souris — [source: `src/lib.rs:807-860`, `src/lib.rs:1100-1283`, `cited`] (confidence: high)
- **Complexité de code accrue dans le fichier monolithique `src/lib.rs`** : contrairement à `egui::Slider` qui tient en quelques lignes (`src/lib.rs:818-833`), un potentiomètre rotatif personnalisé style SSL (calcul d'angle polaire, arc ou graduations, capuchon coloré, repère angulaire, gestion du glisser vertical et de la remise à zéro) ajoute du code géométrique et d'état UI à maintenir dans `src/lib.rs` — [source: `src/lib.rs:784-862`, `Cargo.lock:531-534`, `cited`] (confidence: high)
- **Aucun gain sur les performances audio ou les fonctionnalités DSP** : l'effort de refonte porte uniquement sur l'apparence et la manipulation des paramètres existants — [source: `ARCHITECTURE.md:18-54`, `cited`] (confidence: high)

## Gaps & Open Questions

- [NEEDS CLARIFICATION: Le « look type SSL » doit-il adopter une palette de façade grise/anthracite avec capuchons de potentiomètres colorés par fonction (style console/G-Comp SSL), ou conserver le fond très sombre `Omore` (`rgb(18, 19, 24)`) en remplaçant uniquement les faders horizontaux par des boutons rotatifs de style matériel ?]
- [NEEDS CLARIFICATION: Faut-il conserver les boutons `-`, `+` et `Reset` sous/autour de chaque potentiomètre rotatif, ou remplacer `Reset` par un double-clic (et `-`/`+` par un glisser fin avec touche modificatrice `Shift`/`Ctrl`) ?]
- [NEEDS CLARIFICATION: Les 9 paramètres doivent-ils être organisés en grille 2D (ex. 2 rangées de 3 boutons pour la section Compresseur et 1 rangée de 2 boutons pour la section Clipper/Mix) à la place de l'empilement vertical actuel ?]
- [NEEDS CLARIFICATION: La section de droite (`3. GAIN REDUCTION & STEREO BUS METERING`) conserve-t-elle ses bargraphes verticaux actuels (`In L/R`, `Out L/R`, `GR` descendant style Reaper) ou fait-elle aussi partie du restylage SSL ?]

## Sources

- Aucune URL externe consultée (toutes les preuves proviennent de l'inspection locale du dépôt `/home/tdavio/src/omore-punch` : `.specify/assessments/ssl-rotary-knobs-ui/intake.md`, `.specify/bugs/ui-columns-overlap/assessment.md`, `.specify/memory/constitution.md`, `GEMINI.md`, `ARCHITECTURE.md`, `Cargo.toml`, `Cargo.lock`, `src/lib.rs`).
