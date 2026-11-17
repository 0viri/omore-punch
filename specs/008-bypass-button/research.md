# Research: Bouton Bypass Global (`008-bypass-button`)

## Decision 1: Parametre Bypass officiel DAW via `BoolParam::make_bypass()`

- **Decision**: Ajouter un champ `#[id = "bypass"] pub bypass: BoolParam` dans `OmorePunchParams`, initialise par `BoolParam::new("Bypass", false).make_bypass()`.
- **Rationale**: Dans `nih_plug`, `BoolParam::make_bypass()` insere `ParamFlags::BYPASS`. Les wrappers CLAP et VST3 de `nih_plug` (inspectes dans `nih_plug/src/wrapper/clap/wrapper.rs` et `vst3/wrapper.rs`) traduisent automatiquement ce flag en `CLAP_PARAM_IS_BYPASS` (CLAP) et `ParameterFlags::kIsBypass` (VST3), tout en conservant les flags d'automatisation (`CLAP_PARAM_IS_AUTOMATABLE`, `kCanAutomate`) et la persistence de session via `#[derive(Params)]`. Nos proxies `ClapGainReductionProxy` et `Vst3GainReductionProxy` deleguent integralement l'enumeration, la lecture et l'ecriture des parametres a `nih_plug`, donc aucune modification des proxies ABI n'est requise. Le bouton Bypass de l'UI et le bouton Bypass natif de Reaper restent ainsi synchronises de facon bidirectionnelle sur un unique etat partage.
- **Alternatives considered**:
  - *Parametre booleen standard sans `.make_bypass()`* : rejete, car il ne serait pas lie au bouton Bypass natif de la barre FX de Reaper (et `nih_plug` indique qu'un seul parametre bypass officiel est attendu).
  - *Etat interne UI uniquement (hors `OmorePunchParams`)* : rejete, car il ne serait ni automatisable ni persistant dans les sessions DAW.

## Decision 2: Soft Bypass sans clic par rampe lineaire de 10 ms

- **Decision**: Implementer le soft bypass directement dans `StereoPunchDsp` a l'aide d'un facteur d'enchainement `bypass_wet: f32` dans `[0.0, 1.0]` (`1.0` = traitement actif, `0.0` = bypass complet) pilote par une rampe lineaire de duree fixe `BYPASS_RAMP_MS = 10.0` ms (`bypass_step = 1.0 / (0.010 * sr)` precalcule dans `update_bypass_coeff()` lors des changements de `sample_rate`).
- **Rationale**:
  - Une rampe lineaire de `10 ms` atteint **exactement** `0.0` (en `480` echantillons a `48 kHz`) et `1.0` en un temps strictement inferieur au seuil de `20 ms` (`SC-002`), sans traîne asymptotique.
  - Des que `bypass_wet <= 0.0` (bypass etabli), `process_sample()` court-circuite le mixage et retourne directement `(dry_l, dry_r)` passes par `sanitize_sample()`, assurant une transparence exacte (`0.0` d'ecart dans la plage nominale `[-4.0, 4.0]`, soit `< -120 dBFS`, `SC-001`) tout en conservant la protection anti-NaN/Inf/auto-mute (`FR-004`).
  - Lors d'aller-retours rapides, `bypass_wet` part de sa valeur courante et progresse a pas constant sans discontinuité.
- **Alternatives considered**:
  - *Bascule instantanee sans rampe* : rejete, car elle genere un clic audible des que le traitement modifie l'amplitude ou la phase (compression, EQ, clipper).
  - *Lissage exponentiel (IIR)* : rejete, car une decroissance asymptotique met beaucoup plus longtemps a atteindre exactement `0.0` (risque de residu audessus de `-120 dBFS` pendant plusieurs dizaines de millisecondes).

## Decision 3: Gel des memoires (Gain Match, Max GR, Clip) et remise a zero propre des etats transitoires

- **Decision**:
  - Pendant `cfg.bypass == true` (des le debut de la rampe de bypass) :
    1. Les accumulateurs du Gain Match (`gm_in_ms`, `gm_post_ms`, `gm_presence_ms`, `gm_gain_db`) sont **figes** (ni mis a jour, ni decrements vers `0.0`), ce qui preserve exactement la correction apprise pour la comparaison A/B (`FR-009`).
    2. `SampleOutput::clipped` vaut `false` des que `bypass_wet <= 0.0` (et durant le bypass actif, `process()` n'alimente plus `clip_latched` pour ne pas declencher `[CLIP: ACTIVE]` depuis un clipper contourne — meme pendant la courte rampe de sortie).
    3. `SampleOutput::gr_db` retourne `0.0` lorsque `cfg.bypass` est actif, et `OmorePunch::process()` ecrit `0.0` dans `telemetry.current_gr_db` lorsque `bypass` est actif (mettant automatiquement `host_gain_reduction_db()` a `0.0` pour Reaper VST3/CLAP et la jauge `GR` de l'UI a `0.0 dB`, sans modifier `telemetry.max_gr_db` qui reste fige).
    4. Une fois `bypass_wet == 0.0`, les etats purement transitoires du compresseur et des filtres (`env_gr_db`, `auto_slow_gr_db`, `sc_prev_*`, biquads EQ) sont remis a `0.0` afin d'eviter qu'un residu d'echo ou d'enveloppe obsolete ne resurface au retour en mode actif.
- **Rationale**: Conforme a la fois aux exigences `FR-008`, `FR-009` et au comportement attendu en studio : aucune fausse reduction de gain ou faux clip ne s'affiche en bypass, les pics historiques (`Max GR`, `Peak GR Hold`, `[CLIP: ACTIVE]` preexistant) restent consultables, et la reprise de traitement apres bypass est propre et sans saut de niveau du Gain Match.
- **Alternatives considered**:
  - *Continuer a faire tourner le détecteur/EQ en arriere-plan pendant le bypass* : rejete, car inutilement couteux en CPU quand le plugin est contourne et peut laisser l'enveloppe s'accumuler alors que l'utilisateur s'attend a une comparaison propre, tout en risquant de modifier `gm_gain_db` si non gele.

## Decision 4: Interface graphique (`egui`) — Bouton en-tete et attenuation visuelle sans blocage

- **Decision**:
  1. Dans `draw_header_bar()`, ajouter le bouton `[BYPASS: OFF]` / `[BYPASS: ON]` dans le bloc `right_to_left`, immediatement a gauche du bouton `[CLIP: ...]`. Utiliser des couleurs distinctes : fond sombre neutre (`rgb(42, 46, 58)`) en `OFF` et fond ambre-orange vif (`rgb(215, 130, 20)`) avec texte noir ou blanc tres contraste en `ON` (distinct du rouge `rgb(220, 45, 45)` reserve a `[CLIP: ACTIVE]`). Ajuster finement l'espacement/taille typographique du header si necessaire (ou verifier avec `test_ui_header_fits_window_with_dev_badge`) pour garantir l'absence totale de debordement dans `UI_WINDOW_WIDTH` (`820` px) meme avec le pire cas `[DEV 1d2c5b9-dirty]` + `[BYPASS: OFF]` + `[CLIP: ACTIVE - CLICK TO RESET]`.
  2. Dans `draw_omore_punch_ui()`, lorsque `params.bypass.value()` est `true`, appliquer `ui.set_opacity(0.55)` aux groupes de la colonne gauche (`1. COMPRESSOR & ENVELOPE BALLISTICS`, `2. OUTPUT CLIPPER & PARALLEL MIX`, `3. DRUM BUS EQ`) ainsi qu'au bloc de controle `[Gain Match: ...]` dans la colonne droite, tout en gardant les vu-metres `In L/R`, `Out L/R`, `GR` et l'en-tete a `1.0` d'opacite.
- **Rationale**: `egui::Ui::set_opacity` (verifie dans `egui 0.31.1`) attenue visuellement le rendu des widgets sans desactiver les interactions (`ui.disable()` bloquerait l'edition, ce qui violerait `FR-010`). L'utilisateur voit d'un coup d'oeil que le traitement est contourne tout en pouvant preparer ses reglages.
- **Alternatives considered**:
  - *Utiliser `ui.add_enabled(false, ...)`* : rejete, car cela interdit la modification des parametres en mode bypass (`FR-010`).
  - *Bandeau d'avertissement supplementaire dans le CentralPanel* : rejete, car cela consommerait de la hauteur verticale dans une fenetre fixe `820x650` deja ajustee au pixel pres.
