# Interface & Behavior Contract: Bouton Bypass Global (`008-bypass-button`)

## 1. Contrat Parametre DAW (VST3 / CLAP / State)

1. **Identifiant et type**:
   - ID parametre : `"bypass"`
   - Nom affiche : `"Bypass"`
   - Type : `BoolParam`, defaut `false` (`OFF`)
   - Flag : `ParamFlags::BYPASS` via `.make_bypass()`
2. **Exposition ABI**:
   - **VST3**: `ParameterInfo.flags` inclut `kIsBypass` et `kCanAutomate`.
   - **CLAP**: `clap_param_info.flags` inclut `CLAP_PARAM_IS_BYPASS` et `CLAP_PARAM_IS_AUTOMATABLE`.
   - **Unicite**: exactement 1 parametre porte `ParamFlags::BYPASS` dans `OmorePunchParams`.
3. **Compatibilite ascendante**:
   - Le chargement d'un preset/session DAW sans la cle `"bypass"` initialise `bypass` a `false`.

## 2. Contrat DSP (`StereoPunchDsp::process_sample`)

1. **Transparence en bypass etabli** (`cfg.bypass == true` apres stabilisation <= 20 ms):
   - Pour tout signal stéréo dans `[-4.0, 4.0]`, `out_l == raw_l` et `out_r == raw_r` (ecart < `-120 dBFS`).
   - Pour tout signal non-fini (`NaN`, `Inf`) ou hors plage, `sanitize_sample()` borne la sortie dans `[-4.0, 4.0]` et remplace les non-finis par `0.0`.
2. **Transitions sans clic**:
   - Activation (`false -> true`) et desactivation (`true -> false`) lissees sur `10.0 ms` (strictement `< 20.0 ms`), sans saut d'amplitude discontinu meme en cas de bascules repetees.
3. **Gel des memoires et coherence de sortie**:
   - `gr_db == 0.0` et `clipped == false` lorsque `cfg.bypass == true`.
   - `gm_gain_db` reste strictement inchange pendant toute la duree du bypass (`gm_in_ms`, `gm_post_ms`, `gm_presence_ms` geles).
4. **Surete temps reel**:
   - Zéro allocation tas, zéro verrou, zéro appel `.exp()`/`.sin()`/`.cos()` par echantillon (`bypass_step` mis en cache avec `sample_rate`).

## 3. Contrat Interface Graphique (`draw_header_bar` / `draw_omore_punch_ui`)

1. **Texte 100 % ASCII**:
   - Bouton inactif : `[BYPASS: OFF]`
   - Bouton actif : `[BYPASS: ON]`
2. **Position et mise en page**:
   - Place dans le panneau superieur (`omore_punch_header`), dans le groupe aligne a droite, a gauche du bouton `[CLIP: OK]` / `[CLIP: ACTIVE - CLICK TO RESET]`.
   - Aucun depassement ni chevauchement dans `UI_WINDOW_WIDTH x UI_WINDOW_HEIGHT`, y compris dans le pire cas (`[DEV 1d2c5b9-dirty]` + `[BYPASS: OFF]` + `[CLIP: ACTIVE - CLICK TO RESET]`).
3. **Interaction & Feedback**:
   - Un clic bascule `params.bypass` via `setter.begin_set_parameter` / `set_parameter` / `end_set_parameter`.
   - Lorsque `bypass == true` :
     - `[BYPASS: ON]` est visuellement mis en evidence (couleur d'etat actif distincte du rouge `[CLIP: ACTIVE]`).
     - Les sections de controles de traitement (`1. COMPRESSOR & ENVELOPE BALLISTICS`, `2. OUTPUT CLIPPER & PARALLEL MIX`, `3. DRUM BUS EQ`, et le controle `[Gain Match: ...]`) sont rendues avec une opacite reduite (`0.55`) tout en restant pleinement interactives et editables.
     - Les afficheurs de metering (`GR`, `Max GR`, `In L/R`, `Out L/R`, jauge `GR`) restent a pleine opacite (`1.0`).
