# Phase 0 Research: Compressor Auto-Release (`004-compressor-auto-release`)

**Feature**: `004-compressor-auto-release`
**Date**: 2026-11-14

## Decisions & Findings

### 1. Topologie DSP de l'Auto-Release à double étage (`ReleaseMode` dans `StereoPunchDsp`, Bloc 1)
- **Decision**: Ajouter une énumération `ReleaseMode` dans `src/lib.rs` (Bloc 1, avec `#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug)]` pour partage direct entre `DspConfig` et `OmorePunchParams`, exactement comme `ClipperMode`) comportant 4 variantes :
  1. `Manual` (`#[id = "manual"]`, `#[name = "Manual"]`) — comportement par défaut, utilisant exclusivement `release_ms` (`10.0..=500.0 ms`).
  2. `AutoTight` (`#[id = "auto_tight"]`, `#[name = "Auto Tight (50/600 ms)"]`) — étage rapide `50.0 ms`, étage lent `600.0 ms`.
  3. `AutoPunch` (`#[id = "auto_punch"]`, `#[name = "Auto Punch (80/800 ms)"]`) — étage rapide `80.0 ms`, étage lent `800.0 ms`.
  4. `AutoGlue` (`#[id = "auto_glue"]`, `#[name = "Auto Glue (100/1200 ms)"]`) — étage rapide `100.0 ms`, étage lent `1200.0 ms`.

  En mode Auto (`AutoTight`, `AutoPunch`, `AutoGlue`), modéliser le comportement SSL G-Bus à double réseau RC directement sur l'enveloppe stéréo liée en dB (`env_gr_db >= 0.0`) :
  - Un réservoir lent `auto_slow_gr_db: f32` est mis à jour à chaque échantillon à partir de `target_gr_db` :
    - Charge (lorsque `target_gr_db > self.auto_slow_gr_db`) : lissage exponentiel avec constante d'intégration fixe $\tau_{\text{charge}} = 180.0\text{ ms}$ (`auto_slow_charge_coeff`).
    - Décharge (lorsque `target_gr_db <= self.auto_slow_gr_db`) : lissage exponentiel avec la constante lente du profil (`600.0 ms`, `800.0 ms` ou `1200.0 ms`, `auto_slow_release_coeff`).
    - Dans tous les cas, `auto_slow_gr_db` est borné supérieurement par `self.env_gr_db` après mise à jour de l'enveloppe principale (`self.auto_slow_gr_db = self.auto_slow_gr_db.min(self.env_gr_db)`).
  - Mise à jour de l'enveloppe principale `env_gr_db` :
    - Phase d'attaque (`target_gr_db > self.env_gr_db`) : inchangée (`attack_coeff`).
    - Phase de relâchement (`target_gr_db <= self.env_gr_db`) :
      - En `ReleaseMode::Manual` : formule existante `release_coeff * env_gr_db + (1 - release_coeff) * target_gr_db`.
      - En `AutoTight` / `AutoPunch` / `AutoGlue` : l'étage rapide (`auto_fast_release_coeff`) relâche `env_gr_db` vers `target_gr_db`, avec comme plancher dynamique l'intégrateur lent : `let fast_next = self.auto_fast_release_coeff * self.env_gr_db + (1.0 - self.auto_fast_release_coeff) * target_gr_db; self.env_gr_db = fast_next.max(self.auto_slow_gr_db);`.
- **Rationale**:
  - Sur un transitoire batterie court (`15 ms` au-dessus du seuil), `auto_slow_gr_db` ne se charge qu'à $1 - e^{-15/180} \approx 8\%$ de la réduction de gain crête : dès la fin du coup, `env_gr_db` relâche plus de 90 % de la réduction de gain à la vitesse de l'étage rapide (`50 / 80 / 100 ms`).
  - Sur un passage soutenu (`400 ms` au-dessus du seuil), `auto_slow_gr_db` atteint $1 - e^{-400/180} \approx 89\%$ de la réduction de gain crête et dicte une décroissance lente et musicale à la vitesse de l'étage lent (`600 / 800 / 1200 ms`), produisant un écart de récupération > 8x entre transitoire court et signal soutenu (largement supérieur au seuil `SC-001 >= 2x`) et garantissant l'ordre strict `AutoTight > AutoPunch > AutoGlue`.
  - Le chemin `Manual` ne touche pas au calcul de `env_gr_db`, garantissant une identité bit-à-bit avec la version précédente (`SC-003`).
- **Alternatives considered**:
  - *Calculer un temps de release variable à chaque échantillon et appeler `.exp()` par échantillon* : Rejeté car cela violerait le Principe III de la Constitution (interdiction d'appeler `.exp()` à chaque échantillon quand les paramètres sont constants).
  - *Moduler l'Auto-Release autour du potentiomètre manuel `Release`* : Rejeté suite à la clarification Q1 ; la courbe fixe à double étage est indépendante du potentiomètre manuel pour un comportement SSL prévisible.

### 2. Continuité sans clic lors des changements de mode (`SC-002`)
- **Decision**: Conserver `env_gr_db` comme unique enveloppe de réduction de gain appliquée aux canaux stéréo (`comp_gain`), et continuer à mettre à jour `auto_slow_gr_db` en arrière-plan même en mode `Manual` (sans jamais modifier `env_gr_db` en mode `Manual`), tout en maintenant l'invariant `auto_slow_gr_db <= env_gr_db`.
- **Rationale**:
  - Lors d'un changement de mode (`Manual <-> AutoTight <-> AutoPunch <-> AutoGlue`) en pleine compression (ex. `-12 dB` de réduction de gain), `env_gr_db` reste continu ($C^0$) sans aucun saut instantané de gain, et `auto_slow_gr_db` reflète déjà l'historique récent de compression.
  - La détection stéréo liée (`max(|det_l|, |det_r|)`) reste l'unique source de `target_gr_db`, garantissant une parité L/R parfaite (< `0.001 dB`).
- **Alternatives considered**:
  - *Remettre `auto_slow_gr_db` à `0.0` lors de l'activation d'un mode Auto* : Rejeté car si le compresseur est déjà en compression soutenue, cela ferait soudainement accélérer le release jusqu'à recharge complète de l'étage lent.

### 3. Sécurité Temps Réel (Principe III) et Sanitisation Anti-Dénormaux / NaN (Principe IV)
- **Decision**:
  - Zéro allocation sur le tas et zéro verrou dans `StereoPunchDsp` et `process()`.
  - Ajouter `cached_release_mode: ReleaseMode` (et optionnellement un indicateur de validité de cache pour `sample_rate`) dans `StereoPunchDsp` : `update_ballistics()` ne recalcule `auto_fast_release_coeff`, `auto_slow_release_coeff` et `auto_slow_charge_coeff` via `.exp()` que si `sample_rate` ou `release_mode` change (avec mise à jour explicite dans `set_sample_rate()` et `Default`).
  - Sanitiser `auto_slow_gr_db` à chaque échantillon (`if !self.auto_slow_gr_db.is_finite() || self.auto_slow_gr_db < 1.0e-20 { self.auto_slow_gr_db = 0.0; }`) et le réinitialiser à `0.0` dans `StereoPunchDsp::reset()`.
- **Rationale**: Respecte strictement les Principes III et IV ; élimine tout risque de dénormaux CPU lors des passages silencieux et garantit l'immunité contre `NaN`/`Inf` et l'auto-mute Reaper.
- **Alternatives considered**:
  - *Recalculer les coefficients auto à chaque échantillon* : Rejeté (violation du Principe III).

### 4. Intégration UI dans `1. COMPRESSOR & ENVELOPE BALLISTICS` (`FR-007`, `FR-008`, `FR-009`)
- **Decision**:
  - Ajouter une rangée horizontale dans la section `1. COMPRESSOR & ENVELOPE BALLISTICS`, juste au-dessus ou en dessous de la rangée de presets rapides `[Punch 3ms / 80ms]` / `[Snare Safe 200Hz]` / `[Auto Makeup: ...]`, reprenant exactement le patron de `Clipper Mode:` :
    - Label `Release Mode:` suivi des 4 boutons ASCII `[Manual]`, `[Auto Tight]`, `[Auto Punch]`, `[Auto Glue]`, avec mise en évidence ambre `rgb(195, 125, 20)` sur le mode actif et infobulles détaillant les temps rapide/lent (`50/600 ms`, `80/800 ms`, `100/1200 ms`).
  - Lorsqu'un mode Auto est actif (`release_mode != ReleaseMode::Manual`), afficher le potentiomètre `Release` via un état visuellement atténué (`dimmed_auto_override = true`) dans `draw_ssl_knob_cell` (ou helper associé) :
    - Capuchon SSL, graduations et lecture numérique assombris/désaturés ;
    - Libellé de valeur formaté en ASCII indiquant clairement l'override : `format!("AUTO ({})", formatted_val)` (ou badge `AUTO` sous/dans la lecture) ;
    - Potentiomètre et boutons `-` / `+` / `Reset` toujours interactifs afin de pré-régler la valeur manuelle avant de repasser en `Manual`.
  - Dans le gestionnaire de clic du bouton `[Punch 3ms / 80ms]`, ajouter `setter.begin_set_parameter(&params.release_mode); setter.set_parameter(&params.release_mode, ReleaseMode::Manual); setter.end_set_parameter(&params.release_mode);`.
- **Rationale**: Conforme aux clarifications Q2, Q3, Q4 et aux contraintes GUI de la Constitution (100 % ASCII, monospace, `ParamSetter` complet, aucun chevauchement).
- **Alternatives considered**:
  - *Verrouiller (`ui.add_enabled(false, ...)`) le potentiomètre `Release` en mode Auto* : Rejeté suite à la clarification Q3 (l'utilisateur veut pouvoir pré-régler la valeur manuelle tout en voyant clairement l'état `AUTO`).

### 5. Budget géométrique de la fenêtre d'éditeur (`820x650` px)
- **Decision**:
  - Passer `UI_WINDOW_HEIGHT` de `620` à `650` px (tout en conservant `UI_WINDOW_WIDTH = 820` px) et `UI_METER_HEIGHT` de `390.0` à `416.0` px.
- **Rationale**:
  - L'ajout de la rangée `Release Mode:` (`24.0 px` de hauteur d'action + `2.0 px` d'espacement = `+26.0 px`) porte la hauteur de la Section 1 de `254.0 px` à `280.0 px`, et la hauteur totale de la colonne gauche de `552.0 px` à `578.0 px`.
  - Avec `UI_WINDOW_HEIGHT = 650`, la hauteur disponible dans `CentralPanel` passe à `650 - 48 = 602.0 px` (`578.0 <= 602.0`, soit `24.0 px` de marge).
  - La rangée horizontale `Release Mode: [Manual] [Auto Tight] [Auto Punch] [Auto Glue]` (~416 px) tient largement dans `UI_LEFT_COL_WIDTH = 480.0 px`.
- **Alternatives considered**:
  - *Tasser la rangée dans `620 px` sans agrandir la fenêtre* : Rejeté car cela réduirait la marge verticale sous `CentralPanel` à `-6.0 px` (risque de débordement sur Linux/Windows et échec de `test_ui_layout_column_widths_fit_window`).
