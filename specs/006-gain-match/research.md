# Research & Technical Decisions: Gain Match Automatique

**Feature**: `006-gain-match` | **Date**: 2026-11-14

## 1. Algorithme de mesure et de calcul de compensation (Feed-Forward RMS Stereo-Linked)

### Decision
Utiliser une architecture **feed-forward stéréo liée par intégration quadratique moyenne (RMS 1-pôle)** :
- Mesurer la puissance instantanée moyenne d'entrée brute (avant `input_db` drive) : `in_sq = 0.5 * (dry_l * dry_l + dry_r * dry_r)`.
- Mesurer la puissance instantanée moyenne post-traitement (après compression, `Auto Makeup`/`Makeup Gain`, `Drum Bus EQ`, `Output Clipper` et `Mix Dry/Wet`, mais **avant** l'application du gain de Gain Match) : `post_sq = 0.5 * (mixed_l * mixed_l + mixed_r * mixed_r)`.
- Lisser les deux énergies avec la même constante d'intégration RMS lente (`GAIN_MATCH_RMS_MS = 400.0` ms, coefficient exponentiel `rms_coeff` précalculé selon `sample_rate`).
- Calculer la cible en dB : `target_db = 10.0 * (in_ms.max(MIN_POWER) / post_ms.max(MIN_POWER)).log10()`, bornée à `[-24.0, 24.0]` dB (`GAIN_MATCH_MIN_DB` .. `GAIN_MATCH_MAX_DB`).
- Lisser le gain appliqué `gain_match_db` vers `target_db` avec un slew lent (`GAIN_MATCH_SLEW_MS = 150.0` ms) lorsque `gain_match == true`, et vers `0.0` dB avec un slew doux anti-clic (`GAIN_MATCH_TOGGLE_SLEW_MS = 35.0` ms) lorsque `gain_match == false`.

### Rationale
- **Stabilité inconditionnelle** : En feed-forward (mesure de `mixed` avant application du trim), le système n'a aucune boucle de rétroaction susceptible d'osciller ou de sur-compenser.
- **Annulation de la modulation dynamique** : Comme `in_ms` et `post_ms` partagent exactement la même constante d'intégration (`400 ms`), les variations macro-dynamiques de la boucle de batterie s'annulent dans le rapport `in_ms / post_ms`, préservant le punch et les transitoires sans effet de pompage.
- **Image stéréo préservée** : Le facteur linéaire `db_to_lin(gain_match_db)` est unique et appliqué à l'identique aux canaux gauche et droit (`matched_l = mixed_l * gain`, `matched_r = mixed_r * gain`).
- **Conformité temps réel (Principe III)** : Tous les coefficients exponentiels (`rms_coeff`, `presence_coeff`, `slew_coeff`, `off_slew_coeff`) sont précalculés et mis en cache uniquement lorsque `sample_rate` change. Zéro allocation, zéro verrou.

### Alternatives Considered
- **Boucle de rétroaction (mesure après Gain Match)** : Rejetée car elle exige un contrôleur PI/intégrateur délicat à stabiliser face aux transitoires de batterie et aux variations de seuil/ratio.
- **Pondération K-weighting (LUFS complet)** : Non retenue pour v1 car elle ajoute deux filtres biquad supplémentaires sur les chemins de mesure et favoriserait une dérive si l'utilisateur coupe les graves au `Low Cut` du Drum Bus EQ (le volume RMS perçu sur la bande utile du bus devient alors le meilleur compromis). Une évolution ultérieure reste possible sans casser le contrat.

---

## 2. Détection de silence et comportement Hold (Anti-pompage du bruit de fond)

### Decision
Utiliser un détecteur de présence court dédié (`presence_ms`, constante `GAIN_MATCH_PRESENCE_MS = 50.0` ms sur `in_sq`) comparé à un seuil de bruit fixé à `-60.0 dBFS` (`GAIN_MATCH_NOISE_FLOOR_POWER = 1.0e-6`, soit `10^(-60/10)`):
- Quand `presence_ms >= 1.0e-6` (signal utile présent) : mettre à jour `in_ms`, `post_ms` et, si `cfg.gain_match` est actif, faire converger `gain_match_db` vers `target_db`.
- Quand `presence_ms < 1.0e-6` (silence ou break sous `-60 dBFS`) : **figer (`Hold`)** `in_ms`, `post_ms` et `gain_match_db` à leur dernière valeur valide (qui vaut `0.0 dB` à froid tant qu'aucun signal utile n'a été détecté).
- Lorsque `cfg.gain_match == false` : continuer à suivre `in_ms` et `post_ms` sur signal utile (coût de quelques multiplications/additions scalaires sans `log10`/`powf`), tout en ramenant doucement `gain_match_db` vers `0.0 dB` (`off_slew_coeff`), puis forcer exactement `0.0` sous `1.0e-5` dB.

### Rationale
- Sur une onde sinusoïdale basse fréquence ou entre deux frappes rapprochées de batterie, l'enveloppe `presence_ms` de 50 ms ne tombe pas à zéro aux passages par zéro, évitant un clignotement du Hold.
- Pendant un break ou à l'arrêt de lecture DAW (`presence_ms < 1.0e-6`), le gel simultané de `in_ms`, `post_ms` et `gain_match_db` évite toute dérive due à la queue des filtres EQ ou au bruit résiduel, et garantit un niveau exact dès le premier coup de grosse caisse à la reprise.
- Le pré-suivi léger de `in_ms` / `post_ms` même quand `Gain Match` est `OFF` assure qu'au moment où l'utilisateur clique `[Gain Match: OFF]` → `[Gain Match: ON]` en cours de lecture, les accumulateurs RMS connaissent déjà le rapport de niveau réel sans transitoire initial erroné.

### Alternatives Considered
- **Comparaison directe sur `in_sq` instantané** : Rejetée car un signal sinusoïdal passe par zéro deux fois par période, ce qui gèlerait intermittemment le détecteur.
- **Retour progressif à `0.0 dB` sur silence** : Rejeté lors de la clarification (Q3 -> Option A) pour garantir un niveau immédiat au retour d'un break.

---

## 3. Ordre de traitement, témoin `[CLIP]` et sécurité finale

### Decision
- `Gain Match` est appliqué **après** le `Parallel Mix` (étape 7, donc après compression, `Auto Makeup`/`Makeup Gain`, `Drum Bus EQ` et `Output Clipper`) et **immédiatement avant** `sanitize_sample()`.
- Le signal de sortie final `out_l, out_r = sanitize_sample(matched_l), sanitize_sample(matched_r)` reste borné dans `[-4.0, 4.0]`.
- Le drapeau `clipped` de `SampleOutput` est vrai si le clipper interne a écrêté (`clipper_clipped`) **ou** si, `Gain Match` étant actif, le signal compensé dépasse `0 dBFS` (`matched_l.abs() > 1.0 || matched_r.abs() > 1.0`).

### Rationale
- Conforme aux clarifications Q1 (Option A) et Q2 (Option A) : n'altère pas la saturation du clipper, permet d'utiliser `Makeup Gain` comme drive, compense les pertes dues au clipper, et informe l'utilisateur via `[CLIP: ACTIVE]` si le rattrapage de volume RMS pousse des crêtes au-delà de `0 dBFS`.
- Préserve à 100 % tous les tests existants lorsque `gain_match == false`.

---

## 4. Intégration UI dans la section 4 (`GAIN REDUCTION & STEREO BUS METERING`)

### Decision
- Ajouter un paramètre `#[id = "gain_match"] pub gain_match: BoolParam` (nom `"Gain Match"`, défaut `false`) dans `OmorePunchParams`.
- Ajouter `pub gain_match_db: AtomicU32` dans `MeterTelemetry` (initialisé à `0.0`, mis à jour dans `process()` via `store_f32_relaxed`).
- Dans `draw_omore_punch_ui`, section `4. GAIN REDUCTION & STEREO BUS METERING` (colonne droite) :
  - Placer une rangée compacte dédiée sous le bloc `GR / Max GR` + `[Reset Peak]`, juste au-dessus des vu-mètres verticaux :
    - Bouton bascule `[Gain Match: OFF]` / `[Gain Match: ON]` (même style ambre/gris que `[Auto Makeup: ON/OFF]`), encadré par `begin_set_parameter` / `set_parameter` / `end_set_parameter`.
    - Affichage numérique monospace aligné à droite : `Trim: +0.0 dB` / `Trim: -3.2 dB` (ou `Trim:  0.0 dB` quand OFF/neutre).
  - Réduire légèrement `UI_METER_HEIGHT` (de `416.0` à `384.0` px) si nécessaire afin que la colonne droite reste confortablement à l'intérieur de `UI_WINDOW_HEIGHT = 650` avec la marge de 6 px vérifiée par les tests unitaires `egui`.

### Rationale
- Conforme à la clarification Q4 (Option B) et aux standards visuels de la Constitution (texte 100 % ASCII, monospace, boutons entre crochets, aucun chevauchement).
