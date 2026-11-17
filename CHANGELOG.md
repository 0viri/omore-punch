# Changelog — Omore Punch

Toutes les modifications notables de ce projet sont documentées dans ce fichier.

## [Unreleased]
### Ajouté
- **Support complet des presets utilisateur Reaper (`008-reaper-presets`) :**
  - Implémentation de `Plugin::filter_state` (`sanitize_plugin_state`) pour garantir la rétrocompatibilité à 100 % des projets et presets antérieurs (remplissage des paramètres absents par leurs valeurs par défaut, bornage strict des flottants hors limites ou non finis, normalisation des enums et nettoyage des clés obsolètes/`editor-state`).
  - Resynchronisation explicite de Reaper après chaque chargement de preset ou d'état en **CLAP** (`clap_host_params::rescan(CLAP_PARAM_RESCAN_VALUES)`) et en **VST3** (`IComponentHandler::restart_component(RestartFlags::kParamValuesChanged)`), accompagnée de la remise à zéro automatique de `Max GR` et du témoin `CLIP` même lorsque le plugin est inactif.
- **Metering natif de Gain Reduction DAW (`007-gain-reduction-metering`) :**
  - Exposition de la télémétrie native **VST3** (`PreSonus::IGainReductionInfo`) et **CLAP** (`clap.gain-adjustment-metering/0`) via des proxys locaux autour des wrappers `nih-plug` dans `src/lib.rs`, pour l'affichage en temps réel de la jauge jaune de réduction de gain dans les tranches de mixage Reaper (MCP/TCP) sous Linux et Windows.
  - Lecture lock-free de l'enveloppe brute du compresseur (`[-24.0, 0.0]` dB), isolée par instance et indépendante du make-up gain, de l'EQ, du clipper, du mix parallèle Dry/Wet, du Gain Match et de l'état d'ouverture de l'éditeur UI, avec remise à `0.0 dB` lors des événements de cycle de vie (`reset`, `initialize`, `set_processing(false)`, `set_active(false)`, `stop_processing`, `deactivate`).
- **Fonction Gain Match Automatique (`006-gain-match`) :**
  - Ajout du paramètre DAW rétrocompatible `gain_match` (`OFF` par défaut) et d'un étage de compensation stéréo lié en toute fin de chaîne (post-Compression, post-Drum Bus EQ, post-Output Clipper et post-Mix parallèle, juste avant le safety clamp).
  - Mesure feed-forward par double enveloppe RMS appariée (`400 ms`) entre l'entrée brute (avant `Input Trim`) et le signal post-traitement, avec lissage doux (`150 ms` actif, `35 ms` au retour sur `OFF`), bornage `[-24.0, +24.0]` dB et détecteur de présence (`50 ms` à `-60 dBFS`) figeant (`Hold`) la compensation sur les silences pour préserver les transitoires et éviter l'amplification du bruit de fond.
  - Ajout du bouton `[Gain Match: OFF]` / `[Gain Match: ON]` et de l'affichage en temps réel `Trim: +0.0 dB` dans la section `4. GAIN REDUCTION & STEREO BUS METERING` (avec ajustement de `UI_METER_HEIGHT` à `384.0` px) ainsi que des tests unitaires DSP et `egui` dédiés.
- **Métadonnées statiques des bundles VST3 & CLAP (`005-plugin-bundle-metadata`) :**
  - Génération automatique de `Contents/Resources/moduleinfo.json` dans le bundle VST3 lors du bundling `xtask` (exposant `Name = Omore Punch`, `Version` issue de `Cargo.toml`, `Vendor = Omore`, `CID`, et `Sub Categories = ["Fx", "Dynamics", "Distortion", "Stereo"]` pour les gestionnaires de plugins comme Plugoff).
  - Embarquement des ressources PE `VS_VERSION_INFO` dans les binaires Windows (`.vst3` et `.clap`) via `build.rs` et `winresource`, compatible avec les versions de développement suffixées `-dev`.
- **Profils d'Auto-Release Dual-Stage `Release Mode` (`004-compressor-auto-release`) :**
  - Ajout d'un sélecteur `ReleaseMode` à 4 états (`Manual` par défaut, `Auto Tight` `50/600 ms`, `Auto Punch` `80/800 ms`, `Auto Glue` `100/1200 ms`) exposé au DAW via le paramètre rétrocompatible `release_mode`.
  - Implémentation dans `StereoPunchDsp` d'une enveloppe de relâchement à deux étages style console SSL (intégrateur lent `auto_slow_gr_db` chargé en `180 ms` servant de plancher progressif à l'étage rapide), temps réel safe sans allocation ni appel `.exp()` par échantillon, avec protection anti-dénormaux/NaN et réinitialisation de l'intégrateur lors des changements de fréquence d'échantillonnage ou `reset()`.
  - Ajout de la rangée `Release Mode:` (`[Manual]`, `[Auto Tight]`, `[Auto Punch]`, `[Auto Glue]`) dans la section `1.`, atténuation visuelle et lecture `AUTO (<valeur>)` sur le potentiomètre `Release` lorsqu'un profil Auto est actif (tout en conservant l'édition manuelle pour pré-régler un retour en `Manual`), retour automatique en `Manual` lors du clic sur `[Punch 3ms / 80ms]`, ajustement de la fenêtre à `820x650` px (`UI_METER_HEIGHT = 416.0`) et ajout des tests unitaires DSP/UI dédiés.

### Modifié
- **Renommage des bundles en `Omore Punch` et alignement fabricant (`005-plugin-bundle-metadata`) :**
  - Passage de `OmorePunch::VENDOR` et `Cargo.toml` (`authors`) à `Omore` (aligné avec *Omore Sampler*), tout en conservant strictement inchangés `VST3_CLASS_ID`, `CLAP_ID` et tous les identifiants de paramètres pour une rétrocompatibilité totale des projets DAW existants.
  - Renommage des bundles générés en `Omore Punch.vst3` et `Omore Punch.clap` via `bundler.toml`, mise à jour des artefacts CI (`.github/workflows/build.yml`) et nettoyage automatique des anciens bundles `omore-punch.*` dans `build.sh` et `build.ps1`.

### Supprimé
- **Retrait du paramètre DAW `gr_meter_db` (`007-gain-reduction-metering`) :**
  - Suppression complète du paramètre exposé `gr_meter_db` (`Gain Reduction (dB)`) de `OmorePunchParams` et de sa synchronisation depuis l'interface `egui`, évitant toute entrée superflue dans les listes de paramètres et pistes d'automation DAW maintenant que les extensions de metering GR natives VST3/CLAP sont en place.

## [0.3.0] - 2026-11-14
### Ajouté
- **Section EQ 3 bandes pour Bus Batterie `3. DRUM BUS EQ` (`003-drum-bus-eq`) :**
  - Ajout d'un égaliseur stéréo apparié en série (biquads Direct Form II Transposed précalculés et mis en cache, placés post-compresseur et immédiatement pré-clipper) limité à trois contrôles : **`Low Cut`** (passe-haut Butterworth 12 dB/oct de `20.0 Hz` à `160.0 Hz`, bypass strict `<= 20.5 Hz`), **`500Hz Cut`** (peaking atténuateur à `500 Hz`, $Q = 1.0$, `0.0 dB` à `-12.0 dB`) et **`High Shelf`** (plateau haut à `10 kHz`, $S = 1.0$, `-6.0 dB` à `+12.0 dB`).
  - Exposition des trois paramètres DAW automatables et rétrocompatibles (`eq_low_cut_hz`, `eq_mid_cut_db`, `eq_high_shelf_db`) initialisés à leurs valeurs neutres transparentes.
  - Intégration dans la colonne de gauche de l'interface `egui` avec 3 potentiomètres rotatifs bleus SSL (`SslKnobColor::Filter`), renumérotation du metering à droite en `4. GAIN REDUCTION & STEREO BUS METERING`, ajustement de la fenêtre à `820x620` px (`UI_METER_HEIGHT = 390.0`) et ajout des tests unitaires DSP + géométrie UI.

### Modifié
- **Badge `[DEV <hash>]` / `[DEV <hash>-dirty]` dans l'en-tête pour les versions de développement (`002-dev-version-tag`) :**
  - Capture à la compilation via `build.rs` du hash court (7 caractères) du commit Git et de l'état `-dirty` éventuel (`OMORE_GIT_TAG`).
  - Affichage d'un badge bleu-acier contrasté `[DEV <hash>]` ou `[DEV <hash>-dirty]` (repli sur `[DEV]` hors dépôt Git) à côté du numéro de version de base dans l'en-tête `egui`, tout en conservant un affichage épuré sans badge ni hash en release et la version complète exposée au DAW.
- **Interface `egui` Style SSL avec Potentiomètres Rotatifs en Grille (`002-ssl-rotary-knobs-ui`) :**
  - Remplacement des 9 curseurs horizontaux (`draw_param_row`) par des potentiomètres rotatifs vectoriels (`draw_ssl_knob_cell`) sur couronne graduée à 270° (11 graduations) et capuchons colorés style console SSL intégrés au thème sombre `Omore`.
  - Disposition en grille compacte dans la fenêtre `820x480` px (`4` + `3` boutons en section `1.`, `2` boutons en section `2.`) sans chevauchement avec le metering à droite ni modification des identifiants DAW ou du moteur DSP.
  - Contrôle à la souris par glisser vertical (réglage fin 4x sous `Shift`, double-clic reset) et conservation des petits boutons compacts `-`, `+` et `Reset` sous chaque potentiomètre.
- **Bouton Rapide `[Snare Safe 200Hz]` dans l'interface `egui` (`001-snare-safe-200hz`) :**
  - Remplacement du bouton rapide `[Kick Safe 90Hz]` par `[Snare Safe 200Hz]` réglant `SC HPF` sur `200.0 Hz` en un clic (technique Cristian Kohle) afin que ni le sous-grave de la grosse caisse ni le corps de la caisse claire ne fassent pomper le bus batterie.

### Corrigé
- **Troncature verticale du bas de l'UI (`ui-bottom-clipping`) :**
  - Suppression de la persistance de taille fixe (`#[persist = "editor-state"]`) sur `OmorePunchParams` (état d'éditeur déplacé sur `OmorePunch`) afin qu'une session ou un preset sauvegardé avant `003-drum-bus-eq` (`820x560` ou `820x480`) ne force plus une hauteur obsolète coupant la section `3. DRUM BUS EQ` et le bas des vu-mètres `In L/R` / `Out L/R`.
  - Ajout d'un `ScrollArea::vertical()` de secours dans le `CentralPanel` si l'hôte impose une hauteur inférieure à `620` px, sans barre de défilement à la taille nominale `820x620`.
- **Warning de compilation VST3 (`vst3-export-macro-warning`) :**
  - Suppression du warning `semicolon_in_expressions_from_non_local_macros` émis par `rustc` lors de l'expansion de `nih_export_vst3!(OmorePunch)` (`vst3_com::vtable!`).
- **Mise en page UI `egui` (`ui-columns-overlap`) :**
  - Suppression du chevauchement entre la colonne de gauche (sections `1.` et `2.`) et la colonne de droite (`3. GAIN REDUCTION & STEREO BUS METERING`) qui tronquait les boutons `Reset`.
  - Remplacement du partage rigide 50/50 (`ui.columns(2, ...)`) par deux colonnes asymétriques (`480 px` à gauche, `310 px` à droite) adaptées au contenu.
  - Synchronisation de `ui.spacing_mut().slider_width` avec la largeur allouée au slider (`140 px`) dans `draw_param_row` pour éliminer les vides parasites entre `-`, le rail du slider et `+`, et ajustement de la hauteur de fenêtre (`820x480`) et des vu-mètres (`255 px`).

## [0.2.0] - 2026-10-16
### Ajouté
- **Filtre Passe-Haut de Sidechain Interne (`SC HPF`, `20.0 Hz` à `250.0 Hz`) :**
  - Filtre passe-haut stéréo du 1er ordre (`6 dB/oct`, sans résonance de phase) inséré exclusivement dans le chemin de détection avant le détecteur de crête stéréo lié.
  - Empêche l'énergie sub-grave de la grosse caisse (`40 Hz - 80 Hz`) d'écraser le compresseur et de faire pomper les cymbales, tout en préservant 100 % des graves sur le chemin audio principal.
  - Bypass intégral automatique lorsque `SC HPF <= 20.5 Hz` et précalcul mis en cache du coefficient `sc_hpf_coeff`.
- **Bouton Rapide `[Kick Safe 90Hz]` dans l'interface `egui` :**
  - Permet de régler instantanément `SC HPF` sur `90.0 Hz` en un clic à côté de `[Punch 3ms / 80ms]`.
- **Test Unitaire DSP (`test_sidechain_hpf_kick_immunity`) :**
  - Vérifie sur une onde sinusoïdale grave à `50 Hz` que le filtre Sidechain réduit fortement la réduction de gain indésirable et préserve l'amplitude sub-grave en sortie.

## [0.1.0] - 2026-10-16
### Ajouté
- Moteur DSP stéréo lié complet (`StereoPunchDsp`) reprenant la signature dynamique d'`Omore Sampler` :
  - `Input Trim` (`-12.0 dB` à `+12.0 dB`).
  - Détecteur de crête stéréo lié (`max(|L|, |R|)`).
  - Compresseur Feed-Forward en dB (`Threshold` de `-40.0 dB` à `0.0 dB`, `Ratio` de `1.0:1` à `20.0:1`).
  - Balistique `Attack` (`0.1 ms` à `100.0 ms`, défaut `3.0 ms`) et `Release` (`10.0 ms` à `500.0 ms`, défaut `80.0 ms`) avec précalcul des coefficients exponentiels et bouton rapide `[Punch 3ms / 80ms]`.
  - Compensation de gain `Auto Makeup` (35 %) débrayable + `Makeup Gain` manuel (`-12.0 dB` à `+24.0 dB`).
  - Étage Clipper final à 3 modes (`Soft Clip` cubique/tanh $C^1$, `Hard Clip`, `Bypass Clipper`) et `Clip Ceiling` (`-6.0 dB` à `0.0 dB`).
  - Mix parallèle interne `Mix Dry/Wet` (`0 %` à `100 %`).
  - Sanitisation complète anti-`NaN`/`Inf` et Safety Clamp `[-4.0, 4.0]`.
- Interface graphique `egui` sombre 100 % ASCII avec :
  - Versioning dynamique via `env!("CARGO_PKG_VERSION")`.
  - Grande jauge verticale de Gain Reduction (barre jaune descendante de `0.0 dB` vers `-24.0 dB` style Reaper) + indicateurs numériques `GR` et `Max GR`.
  - Vu-mètres stéréo `In L/R` et `Out L/R` avec témoin d'écrêtage `[CLIP]`.
- Suite de tests unitaires automatisés (`cargo test`) et pipeline CI GitHub Actions multi-OS (Linux x86_64 & Windows x86_64).