# Tasks: Bouton Bypass Global

**Input**: Design documents from `specs/008-bypass-button/`
**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/bypass-contract.md](contracts/bypass-contract.md), [quickstart.md](quickstart.md)

**Tests**: Conformement au Principe V de la Constitution d'Omore Punch, tout comportement DSP nouveau ou modifie DOIT etre couvert par des tests unitaires dans le module `tests` de `src/lib.rs`.

**Organization**: Toutes les modifications de code plugin resident dans le fichier unique `src/lib.rs` (Principe I), organisees par User Story pour permettre une verification incrementale.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Peut etre execute en parallele (fichiers differents, sans dependance incomplete)
- **[Story]**: User Story concernee (`[US1]`, `[US2]`, `[US3]`)
- Chemins de fichiers exacts inclus dans chaque tache

---

## Phase 1: Setup (Verification Baseline)

**Purpose**: Verifier que le worktree et la suite de tests existante sont sains avant toute modification

- [X] T001 Verifier l'etat propre de la branche `feat/008-bypass-button` et executer `cargo test --release` ainsi que `LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs` dans `.worktrees/008-bypass-button/src/lib.rs`

---

## Phase 2: Foundational (Prerequis DSP partages)

**Purpose**: Etendre `DspConfig` et preparer les champs/coefficients de rampe du soft bypass dans `StereoPunchDsp`

**CRITICAL**: Ces prerequis doivent etre en place avant d'implementer les User Stories car tous les tests et le moteur DSP utilisent `DspConfig` et `StereoPunchDsp`.

- [X] T002 Ajouter `pub bypass: bool` (defaut `false`) dans `DspConfig` et `DspConfig::default()`, et mettre a jour les litteraux `DspConfig { ... }` existants dans `src/lib.rs`
- [X] T003 Ajouter `BYPASS_RAMP_MS: f32 = 10.0`, les champs `bypass_wet: f32` (init `1.0`, borne `[0.0, 1.0]`) et `bypass_step: f32` (`1.0 / (0.010 * sr)`) dans `StereoPunchDsp`, precalculer `bypass_step` via `update_bypass_coeff(sample_rate)` dans `Default`, `set_sample_rate()` et `update_ballistics()` (sur changement de `sample_rate`), ajouter le helper `sync_bypass_state(&mut self, bypass: bool)` (`bypass_wet = if bypass { 0.0 } else { 1.0 }`), et reinitialiser `bypass_wet = 1.0` dans `StereoPunchDsp::reset()` dans `src/lib.rs`

**Checkpoint**: `cargo test --release` compile et passe toujours avec `bypass: false` par defaut.

---

## Phase 3: User Story 1 - Comparaison A/B instantanee et transparente (Priority: P1) MVP

**Goal**: Permettre le contournement complet de la chaine de traitement (compression, EQ, clipper, mix, Gain Match) avec transition lissée sans clic (< 20 ms) et maintien de la protection `sanitize_sample()`.

**Independent Test**: Executer les tests DSP synthétiques dans `mod tests` de `src/lib.rs` verifiant l'ecart `< -120 dBFS` en bypass etabli, la rampe lissée sans discontinuité et le safety clamp `[-4.0, 4.0]` sur `NaN`/`Inf`.

### Implementation & Tests for User Story 1

- [X] T004 [US1] Implementer le soft bypass dans `StereoPunchDsp::process_sample()` dans `src/lib.rs` : faire progresser `bypass_wet` par pas `bypass_step` vers `0.0` (`cfg.bypass == true`) ou `1.0` (`cfg.bypass == false`), court-circuiter directement vers `(dry_l, dry_r)` passes par `sanitize_sample()` lorsque `bypass_wet <= 0.0`, remettre a zero les etats transitoires (`env_gr_db`, `auto_slow_gr_db`, `sc_prev_*`, `eq_low_cut`, `eq_mid_cut`, `eq_high_shelf`) en bypass etabli, et mixer lineairement `(1.0 - bypass_wet) * dry + bypass_wet * processed` pendant les rampes
- [X] T005 [US1] Ajouter les tests unitaires DSP `test_bypass_transparency_and_safety_clamp` (ecart `< -120 dBFS` apres stabilisation sur signal actif avec compression/EQ/clipper, et sanitisation `NaN`/`Inf`/`[-4.0, 4.0]` en bypass actif) et `test_bypass_smooth_transition_no_clicks` (transition `< 20 ms` a l'activation, desactivation et bascules rapides) dans `mod tests` de `src/lib.rs`

**Checkpoint**: User Story 1 est fonctionnelle et testable de maniere autonome via `cargo test --release`.

---

## Phase 4: User Story 2 - Acces permanent dans l'interface, etat visuel et integration DAW (Priority: P2)

**Goal**: Exposer le parametre officiel DAW `bypass` synchronise bidirectionnellement avec l'hote VST3/CLAP, afficher le bouton `[BYPASS: OFF]` / `[BYPASS: ON]` dans la barre d'en-tête et attenuer visuellement les sections de traitement sans bloquer leur edition.

**Independent Test**: Executer les tests ABI parametres VST3/CLAP, persistance de session et rendu headless `egui` dans `UI_WINDOW_WIDTH x UI_WINDOW_HEIGHT` dans `mod tests` de `src/lib.rs`.

### Implementation & Tests for User Story 2

- [X] T006 [US2] Ajouter `#[id = "bypass"] pub bypass: BoolParam` initialise via `BoolParam::new("Bypass", false).make_bypass()` dans `OmorePunchParams`, appeler `self.dsp.sync_bypass_state(self.params.bypass.value())` dans `OmorePunch::initialize()` et `OmorePunch::reset()`, et transmettre `self.params.bypass.value()` vers `DspConfig.bypass` dans `OmorePunch::process()` dans `src/lib.rs`
- [X] T007 [US2] Mettre a jour `draw_header_bar()` et `draw_omore_punch_ui()` dans `src/lib.rs` pour afficher le bouton `[BYPASS: OFF]` / `[BYPASS: ON]` en ASCII pur a gauche de `[CLIP: ...]` (couleur active distincte de `[CLIP: ACTIVE]`, pilotage via `begin_set_parameter`/`set_parameter`/`end_set_parameter`), ajuster espacement/taille de police (ou `UI_WINDOW_WIDTH` si necessaire) dans l'en-tête sans jamais tronquer les libelles constitutionnels (`OMORE PUNCH v<VERSION>  |  STEREO DRUM BUS COMPRESSOR & CLIPPER` et `[CLIP: ACTIVE - CLICK TO RESET]`), et appliquer `set_opacity(0.55)` aux sections `1.`, `2.`, `3.` et au bloc `Gain Match` lorsque `bypass == true` tout en conservant leur interactivite
- [X] T008 [US2] Ajouter les tests unitaires d'integration DAW et UI headless dans `mod tests` de `src/lib.rs` : verification du parametre `"bypass"` (defaut `false`, flag `ParamFlags::BYPASS`, exposition `CLAP_PARAM_IS_BYPASS` et `kIsBypass`, compatibilite `deserialize_fields` sans cle `"bypass"`, transparence immediate au premier echantillon si initialise avec `bypass = true`), mise a jour de `test_ui_header_fits_window_with_dev_badge` avec le bouton Bypass, et ajout du cas `bypass = true` dans `test_ui_layout_fits_without_overlap` verifiant aussi qu'une interaction sur un controle de traitement reste active malgre l'opacite reduite (`0.55`)

**Checkpoint**: User Stories 1 et 2 fonctionnent ensemble et passent tous les tests DSP, ABI et UI.

---

## Phase 5: User Story 3 - Coherence du metering pendant et apres le Bypass (Priority: P3)

**Goal**: Garantir que `GR` instantane affiche `0.0 dB` (UI et DAW), qu'aucun nouveau clip n'est signale, et que `Max GR`, `Peak GR Hold`, `[CLIP: ACTIVE]` preexistant et `Gain Match` restent figes en memoire pendant le bypass.

**Independent Test**: Executer les tests de gel Gain Match et de telemetrie `MeterTelemetry` pendant et apres bypass dans `mod tests` de `src/lib.rs`.

### Implementation & Tests for User Story 3

- [X] T009 [US3] Geler la mise a jour des accumulateurs et de la correction Gain Match (`gm_in_ms`, `gm_post_ms`, `gm_presence_ms`, `gm_gain_db`) et forcer `gr_db: 0.0` et `clipped: false` dans `StereoPunchDsp::process_sample()` lorsque `cfg.bypass == true`, et dans `OmorePunch::process()` forcer `telemetry.current_gr_db` a `0.0` tout en ne modifiant ni `max_gr_db` ni `clip_latched` lorsque `bypass == true` dans `src/lib.rs`
- [X] T010 [US3] Ajouter le test unitaire `test_bypass_freezes_gain_match_and_clears_gr_and_clip` dans `mod tests` de `src/lib.rs` verifiant `gr_db == 0.0`, `clipped == false`, `gm_gain_db` strictement fige pendant le bypass et reprise sans saut au retour, ainsi que la conservation de `max_gr_db` et `clip_latched` dans `MeterTelemetry`

**Checkpoint**: Toutes les User Stories (P1, P2, P3) sont completes et testees independamment.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, audit temps reel et validation complete des quality gates avant livraison

- [X] T011 [P] Ajouter l'entree de la fonctionnalite sous `## [Unreleased]` (`Added`) dans `CHANGELOG.md`
- [X] T012 Relire toutes les modifications de `OmorePunch::process()` et `StereoPunchDsp` dans `src/lib.rs` au regard des Principes III (zero allocation, zero verrou, coefficients precalcules) et IV (sanitisation & safety clamp `[-4.0, 4.0]`) de la Constitution
- [ ] T013 Executer les quality gates complets decrits dans `specs/008-bypass-button/quickstart.md` : `LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs`, `cargo test --release`, et `cargo run --package xtask --release -- bundle omore-punch --release` (ASCII + `cargo test --workspace --release` + `cargo build --release` valides dans le worktree ; `xtask bundle` a cible le repo racine `main` a cause de `nih_plug_xtask::chdir_workspace_root()` dans un worktree imbrique sous `.worktrees/`)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: Aucune dependance — demarre immediatement.
- **Foundational (Phase 2)**: Depend de Phase 1 — BLOQUE toutes les User Stories (car `DspConfig` et `StereoPunchDsp` sont partages).
- **User Story 1 (Phase 3)**: Depend de Phase 2 — delivre le MVP (soft bypass DSP transparent et sans clic).
- **User Story 2 (Phase 4)**: Depend de Phase 2 et s'appuie sur `DspConfig.bypass` cable dans `process()` (peut suivre Phase 3 sequentiellement dans `src/lib.rs`).
- **User Story 3 (Phase 5)**: Depend de Phase 3 (`process_sample`) et Phase 4 (`OmorePunch::process`).
- **Polish (Phase 6)**: Depend de la completion des Phases 3, 4 et 5.

### Parallel Opportunities

- Comme tout le code du plugin reside dans un fichier unique (`src/lib.rs`, Principe I), les taches d'implementation sur `src/lib.rs` s'executent **sequentiellement** pour eviter tout conflit d'edition.
- Seule la tache `T011` (`CHANGELOG.md`) peut etre executee en parallele (`[P]`) une fois les specs/implementation stabilisees.

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Completer Phase 1 (Setup) + Phase 2 (Foundational).
2. Completer Phase 3 (User Story 1 : soft bypass DSP + tests de transparence et anti-clic).
3. Valider avec `cargo test --release`.

### Incremental Delivery

1. Phase 1 + Phase 2 -> Base compilee.
2. Phase 3 (US1) -> Soft bypass DSP valide.
3. Phase 4 (US2) -> Parametre DAW + bouton UI en-tête + attenuation visuelle valides.
4. Phase 5 (US3) -> Gel Gain Match et coherence metering valides.
5. Phase 6 (Polish) -> `CHANGELOG.md`, audit temps reel et generation des bundles `.vst3` / `.clap`.
