# Data Model: DAW Gain Reduction Metering

**Feature**: `007-gain-reduction-metering`
**Date**: 2026-11-15

## Entités et flux de données

### 1. `MeterTelemetry` (existant, étendu par méthode de lecture hôte)
Structure partagée lock-free (`Arc<MeterTelemetry>`) entre `OmorePunch` (thread audio), `draw_omore_punch_ui` (thread GUI), le wrapper CLAP et le proxy COM VST3.

| Champ / Méthode | Type | Plage / Sémantique | Producteur | Consommateurs |
|-----------------|------|--------------------|------------|---------------|
| `current_gr_db` | `AtomicU32` (bits `f32`, `Relaxed`) | `0.0` au repos, positif jusqu'à `env_gr_db` en compression | Thread audio (`OmorePunch::process`), lifecycle (`reset`, `stop_processing`, `deactivate`, `set_processing(false)`, `set_active(false)`) | GUI `egui` (jauge `0..24 dB`), `host_gain_reduction_db()` |
| `host_gain_reduction_db(&self) -> f64` | Méthode pure lock-free | `0.0` si `!finite \|\| <= 0.0`, sinon `-(val.min(24.0) as f64)` dans `[-24.0, 0.0]` | N/A | CLAP `gain_adjustment_metering::get`, VST3 `IGainReductionInfo::get_gain_reduction_value_in_db`, `OmorePunch::current_gain_adjustment_db` |

### 2. `ClapGainReductionBox` (export CLAP dans `src/lib.rs`)
Enveloppe `#[repr(C)]` par instance autour du `clap_plugin` produit par `nih_plug::wrapper::clap::Wrapper<OmorePunch>`.

- `plugin: clap_plugin` — **premier champ obligatoire** (`#[repr(C)]`) dont le pointeur est retourné au DAW par `create_plugin`. `plugin.plugin_data` reste inchangé (`*const Wrapper<OmorePunch>`).
- `inner_get_extension: Option<unsafe extern "C" fn(*const clap_plugin, *const c_char) -> *const c_void>` — pointeur original de `nih_plug`.
- `inner_stop_processing: Option<unsafe extern "C" fn(*const clap_plugin)>` — pointeur original de `nih_plug`.
- `inner_deactivate: Option<unsafe extern "C" fn(*const clap_plugin)>` — pointeur original de `nih_plug`.
- `inner_reset: Option<unsafe extern "C" fn(*const clap_plugin)>` — pointeur original de `nih_plug`.
- `inner_destroy: Option<unsafe extern "C" fn(*const clap_plugin)>` — pointeur original de `nih_plug`.
- `telemetry: Arc<MeterTelemetry>` — télémétrie propre à cette instance.

### 3. `Vst3GainReductionProxy` (export VST3 dans `src/lib.rs`)
Co-classe COM `#[VST3(implements(IComponent, IEditController, IAudioProcessor, IMidiMapping, INoteExpressionController, IProcessContextRequirements, IUnitInfo, IGainReductionInfo))]` créée par `Factory::create_instance`.

- `inner: *mut nih_plug::wrapper::vst3::Wrapper<OmorePunch>` — instance interne `nih_plug` détenue exclusivement par le proxy (`ref_count` interne maintenu à `1`, libéré par `(*inner).release()` dans `Drop`).
- `telemetry: Arc<MeterTelemetry>` — télémétrie propre à cette instance.

### 4. Suppression dans `OmorePunchParams`
- Suppression de `gr_meter_db: FloatParam` (`#[id = "gr_meter_db"]`). Aucun paramètre de metering n'est exposé dans la table des paramètres du plugin.

## Règles de validation et transitions d'état

1. **Traitement actif (`process`)** :
   - `self.dsp.env_gr_db` (après Attack/Release/Auto-Release, avant make-up/EQ/clipper/Mix/Gain Match) est stocké dans `telemetry.current_gr_db`.
   - `host_gain_reduction_db()` projette cette valeur sur `[-24.0, 0.0] dB` (`0.0` exact si `<= 0.0` ou non-fini).
2. **Silence ou signal < Threshold** :
   - `env_gr_db` décroît vers `0.0` selon le Release actif (seuil anti-dénormal `< 1e-20 => 0.0`), donc `host_gain_reduction_db()` revient à `0.0`.
3. **Bypass / Reset / Désactivation hôte** :
   - `OmorePunch::reset()`, CLAP `stop_processing`/`deactivate`/`reset`, et VST3 `set_processing(false)`/`set_active(false)` écrivent `0.0` dans `telemetry.current_gr_db`.
4. **Mix parallèle (`mix_pct < 100.0`)** :
   - `env_gr_db` n'est pas pondéré par `mix_pct` ; la jauge DAW et le vumètre interne restent alignés au centième de dB près.
