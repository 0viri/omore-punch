# Phase 1 Data Model: Compressor Auto-Release (`004-compressor-auto-release`)

**Feature**: `004-compressor-auto-release`
**Date**: 2026-11-14

## Entities & Structures (`src/lib.rs`)

### 1. Énumération `ReleaseMode` (Bloc 1 — Moteur DSP & Paramètres)
`#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug)]`

| Variante Rust | `#[id = "..."]` | `#[name = "..."]` | Étage rapide (`fast_ms`) | Étage lent (`slow_ms`) | Charge lente (`charge_ms`) |
|---------------|-----------------|-------------------|--------------------------|------------------------|----------------------------|
| `Manual` | `"manual"` | `"Manual"` | N/A (utilise `release_ms`) | N/A | `180.0 ms` (suivi passif) |
| `AutoTight` | `"auto_tight"` | `"Auto Tight (50/600 ms)"` | `50.0 ms` | `600.0 ms` | `180.0 ms` |
| `AutoPunch` | `"auto_punch"` | `"Auto Punch (80/800 ms)"` | `80.0 ms` | `800.0 ms` | `180.0 ms` |
| `AutoGlue` | `"auto_glue"` | `"Auto Glue (100/1200 ms)"` | `100.0 ms` | `1200.0 ms` | `180.0 ms` |

Méthode d'assistance pure DSP :
- `pub fn auto_time_constants_ms(self) -> Option<(f32, f32)>` : retourne `None` pour `Manual`, `Some((50.0, 600.0))` pour `AutoTight`, `Some((80.0, 800.0))` pour `AutoPunch`, `Some((100.0, 1200.0))` pour `AutoGlue`.

### 2. Additions à `DspConfig` (Bloc 1 — Configuration DSP par échantillon)
| Champ | Type | Défaut | Plage valide | Rôle |
|-------|------|--------|--------------|------|
| `release_mode` | `ReleaseMode` | `ReleaseMode::Manual` | 4 variantes `ReleaseMode` | Sélectionne le release manuel (`release_ms`) ou l'un des 3 profils Auto-Release |

### 3. Additions à `StereoPunchDsp` (Bloc 1 — État & Cache temps réel)
- Cache de mode :
  - `cached_release_mode: ReleaseMode` (initialisé dans `Default` et `set_sample_rate()` pour forcer le calcul initial)
  - `auto_coeffs_initialized: bool` (ou sentinelle permettant de recalculer les coefficients auto dès le premier appel et à chaque changement de `sample_rate` ou `release_mode`)
- Coefficients précalculés (Principe III — recalculés uniquement dans `update_ballistics` lors d'un changement de `sample_rate` ou `release_mode`) :
  - `auto_fast_release_coeff: f32` : `(-1.0 / (fast_sec * sr)).exp()`
  - `auto_slow_release_coeff: f32` : `(-1.0 / (slow_sec * sr)).exp()`
  - `auto_slow_charge_coeff: f32` : `(-1.0 / (0.180 * sr)).exp()`
- État interne d'enveloppe :
  - `auto_slow_gr_db: f32` (initialisé à `0.0`, remis à `0.0` dans `reset()`)
  - Invariant de sanitisation (Principe IV) : après chaque échantillon, `if !self.auto_slow_gr_db.is_finite() || self.auto_slow_gr_db < 1.0e-20 { self.auto_slow_gr_db = 0.0; }`, et `self.auto_slow_gr_db = self.auto_slow_gr_db.min(self.env_gr_db);`.

### 4. Additions à `OmorePunchParams` (Bloc 3 — Paramètres `nih-plug`)
| Champ Rust | `#[id = "..."]` | Type `nih-plug` | Nom DAW | Défaut | Exposition hôte |
|------------|-----------------|-----------------|---------|--------|-----------------|
| `release_mode` | `"release_mode"` | `EnumParam<ReleaseMode>` | `"Release Mode"` | `ReleaseMode::Manual` | Automatable et persisté en session VST3/CLAP |

Note de rétrocompatibilité : aucun paramètre existant (`release_ms`, `attack_ms`, etc.) n'est renommé ou supprimé. Les projets DAW existants chargent avec `release_mode = ReleaseMode::Manual`.

### 5. Modèle UI (`draw_omore_punch_ui`, Bloc 4)
- **Sélecteur `Release Mode:`** dans `1. COMPRESSOR & ENVELOPE BALLISTICS` :
  - 4 boutons ASCII : `[Manual]`, `[Auto Tight]`, `[Auto Punch]`, `[Auto Glue]`.
  - Bouton actif mis en évidence (`rgb(195, 125, 20)`), inactifs en `rgb(45, 48, 58)`.
- **Potentiomètre `Release`** (`&params.release_ms`) :
  - `dimmed_auto_override = params.release_mode.value() != ReleaseMode::Manual`.
  - Lorsque `dimmed_auto_override == true` :
    - Capuchon SSL, graduations et texte de lecture atténués visuellement ;
    - Lecture numérique formatée en `"AUTO ({formatted_val})"` (100 % ASCII) ;
    - Contrôles (`drag`, `Shift` fin, double-clic, `-`, `+`, `Reset`) conservés actifs pour pré-régler `release_ms`.
- **Bouton `[Punch 3ms / 80ms]`** :
  - Met `attack_ms = 3.0`, `release_ms = 80.0`, et `release_mode = ReleaseMode::Manual`.

### 6. Constantes géométriques mises à jour (`src/lib.rs`)
| Constante | Ancienne valeur | Nouvelle valeur | Invariant vérifié |
|-----------|-----------------|-----------------|-------------------|
| `UI_WINDOW_WIDTH` | `820` | `820` (inchangé) | `480 + 10 + 310 + 16 = 816 <= 820` |
| `UI_WINDOW_HEIGHT` | `620` | `650` | `280 + 6 + 156 + 6 + 130 = 578 <= 602` (`650 - 48`) |
| `UI_METER_HEIGHT` | `390.0` | `416.0` | Hauteur des vu-mètres adaptée à la fenêtre `650` px sans débordement |

## State Transitions (Signal & Paramètres)

1. **Mise à jour par échantillon dans `StereoPunchDsp::process_sample`** :
   - Appeler `self.update_ballistics(self.sample_rate, cfg.attack_ms, cfg.release_ms, cfg.release_mode);` (recalcul `.exp()` uniquement si `sample_rate`, `attack_ms`, `release_ms` ou `release_mode` a changé).
   - Calculer `target_gr_db` à partir du détecteur crête stéréo lié.
   - Mettre à jour `auto_slow_gr_db` à partir de `target_gr_db` (`auto_slow_charge_coeff` si `target_gr_db > auto_slow_gr_db`, sinon `auto_slow_release_coeff`), avec garde `NaN`/`Inf`/dénormal.
   - Mettre à jour `env_gr_db` :
     - Si `target_gr_db > self.env_gr_db` : attaque avec `attack_coeff`.
     - Sinon si `cfg.release_mode == ReleaseMode::Manual` : relâchement manuel avec `release_coeff`.
     - Sinon (`AutoTight` / `AutoPunch` / `AutoGlue`) : `fast_next = auto_fast_release_coeff * env_gr_db + (1.0 - auto_fast_release_coeff) * target_gr_db; env_gr_db = fast_next.max(auto_slow_gr_db);`.
   - Borner `auto_slow_gr_db = auto_slow_gr_db.min(env_gr_db)`.
2. **Transition entre modes (`Manual <-> AutoTight <-> AutoPunch <-> AutoGlue`)** :
   - `env_gr_db` n'est jamais réinitialisé lors d'un changement de mode ; seule la loi d'évolution des échantillons suivants change, assurant une transition continue sans clic audio.
