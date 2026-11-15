# Research: DAW Gain Reduction Metering

**Feature**: `007-gain-reduction-metering`
**Date**: 2026-11-15

## Contexte technique observé dans `src/lib.rs` et `nih_plug`

L'inspection du dépôt (`Cargo.lock`, `src/lib.rs`, et le checkout `nih_plug` commit `de421011`) a révélé les éléments suivants :

1. **État existant dans `src/lib.rs`** :
   - `MeterTelemetry::current_gr_db` (`AtomicU32`) est déjà mis à jour en fin de `process()` avec `self.dsp.env_gr_db` (valeur positive en dB, issue du détecteur/enveloppe du compresseur avant make-up gain, EQ, clipper, mix parallèle et Gain Match).
   - Une ancienne tentative inachevée existait :
     - `OmorePunchParams::gr_meter_db` (`#[id = "gr_meter_db"]`, `FloatParam` `.non_automatable()`), visible dans la liste des paramètres du DAW et synchronisé uniquement lorsque la fenêtre `egui` était ouverte (`draw_omore_punch_ui`).
     - `OmorePunch::current_gain_adjustment_db(&self) -> f64` (jamais appelée par les wrappers `nih_plug`).
   - `OmorePunch::reset(&mut self)` réinitialisait `self.dsp.reset()` mais pas `self.telemetry.current_gr_db`.

2. **Capacités de `nih_plug` (commit `de421011`)** :
   - Le wrapper CLAP de `nih_plug` (`wrapper::clap::Wrapper<P>`) n'implémente pas l'extension `clap.gain-adjustment-metering/0` dans `get_extension`.
   - Le wrapper VST3 de `nih_plug` (`wrapper::vst3::Wrapper<P>`) n'implémente ni `PreSonus::IGainReductionInfo` ni `Steinberg::Vst::IParameterFunctionName`.
   - En revanche, `nih_plug::wrapper::clap` et `nih_plug::wrapper::vst3` exposent publiquement `Wrapper`, `PluginDescriptor`, `PluginInfo`, `clap_plugin`, `clap_plugin_entry`, `clap_plugin_factory`, et `vst3_sys`. De plus, les macros `nih_export_clap!` et `nih_export_vst3!` sont de simples adaptateurs autour de ces types publics.

## Décisions d'architecture

### Décision 1 : Extension CLAP `clap.gain-adjustment-metering/0` dans `src/lib.rs`
- **Decision** : Remplacer `nih_export_clap!(OmorePunch);` par un module d'export CLAP local dans `src/lib.rs` qui réutilise `nih_plug::wrapper::clap::{Wrapper, PluginDescriptor, ...}` et enveloppe le `clap_plugin` créé dans `create_plugin` par une struct `#[repr(C)]` (`ClapGainReductionBox`) contenant le `clap_plugin` modifié, les pointeurs de fonctions originaux (`get_extension`, `stop_processing`, `deactivate`, `reset`, `destroy`) et un `Arc<MeterTelemetry>` propre à l'instance.
- **Rationale** :
  - `clap_plugin::get_extension` retourne `&CLAP_GAIN_ADJUSTMENT_METERING` lorsque l'ID vaut `"clap.gain-adjustment-metering/0"`, et délègue toutes les autres extensions au `get_extension` de `nih_plug`.
  - La struct C ABI `clap_plugin_gain_adjustment_metering_t` ne contient qu'un pointeur de fonction `get: Option<unsafe extern "C" fn(*const clap_plugin) -> f64>`. Grâce à `#[repr(C)]` avec `plugin: clap_plugin` en premier champ, `get(plugin)` retrouve l'instance `ClapGainReductionBox` en $O(1)$ sans modifier `plugin.plugin_data` (qui reste `*const Wrapper<OmorePunch>` pour toutes les autres méthodes `nih_plug`).
  - `stop_processing`, `deactivate` et `reset` remettent immédiatement `telemetry.current_gr_db` à `0.0` avant de déléguer à `nih_plug`, garantissant le retour instantané à `0.0 dB` au Bypass ou à l'arrêt du traitement.
  - Aucun fork de `nih_plug` ni modification de `Cargo.toml` requis ; conformité totale avec le Principe I (monolithique `src/lib.rs`).
- **Alternatives considered** :
  - *Forker `nih-plug` sur GitHub* : rejeté car cela introduit une dépendance externe supplémentaire à maintenir pour Linux/Windows/CI et sort la logique du dépôt `omore-punch`.

### Décision 2 : Interface VST3 `PreSonus::IGainReductionInfo` via proxy COM dans `src/lib.rs`
- **Decision** : Remplacer `nih_export_vst3!(OmorePunch);` par un module d'export VST3 local dans `src/lib.rs` qui définit l'interface COM `IGainReductionInfo` (`#[com_interface("8E3C292C-9592-4F9D-B259-0B1E100E4198")]`) et un proxy COM `#[VST3(implements(IComponent, IEditController, IAudioProcessor, IMidiMapping, INoteExpressionController, IProcessContextRequirements, IUnitInfo, IGainReductionInfo))]` délégant les 7 interfaces standard à `Wrapper<OmorePunch>` et implémentant `IGainReductionInfo::get_gain_reduction_value_in_db(&self) -> f64`.
- **Rationale** :
  - `PreSonus::IGainReductionInfo` est le standard supporté nativement par Reaper (Linux et Windows) pour afficher la jauge jaune descendante de réduction de gain sur la tranche MCP/TCP **sans exposer aucun paramètre** dans la liste des paramètres ou des enveloppes d'automation (répondant strictement à FR-004 et Q3).
  - La macro `#[com_interface("8E3C292C-9592-4F9D-B259-0B1E100E4198")]` de `vst3_sys` génère automatiquement les IID compatibles Linux (big-endian) et Windows (COM little-endian) pour la compilation croisée et native.
  - Le proxy COM gère un `ref_count` unifié pour toutes les interfaces retournées à l'hôte (donc `queryInterface(IID_IGainReductionInfo)` fonctionne depuis n'importe quel pointeur `IComponent`, `IEditController` ou `IAudioProcessor`) et libère `Wrapper<OmorePunch>` via `(*inner).release()` dans `Drop`.
  - Sur `set_processing(false)` et `set_active(false)`, le proxy remet `telemetry.current_gr_db` à `0.0` avant de déléguer.
- **Alternatives considered** :
  - *`Steinberg::Vst::IParameterFunctionName` (`kCompGainReduction`) + paramètre caché* : nécessite tout de même de déclarer un `Param` dans `OmorePunchParams` et d'écrire dans `output_parameter_changes` dans `IAudioProcessor::process`, ce qui risque d'exposer le paramètre dans les hôtes/menus ne filtrant pas `kIsHidden` et contrevient à l'exigence d'absence totale de paramètre de GR (Q3). `IGainReductionInfo` est plus direct et zéro-paramètre.
  - *Patching manuel des vtables de `Wrapper<OmorePunch>`* : fragile face aux 7 vptrs générés par `vst3_com` et risqué en sûreté mémoire ; le proxy `#[VST3(implements(...))]` est 100 % type-safe.

### Décision 3 : Liaison par instance sans état global via `thread_local!` à l'instanciation
- **Decision** : Capturer l'`Arc<MeterTelemetry>` créé dans `OmorePunch::default()` dans un `thread_local!` consommé immédiatement par `create_plugin` (CLAP) ou `create_instance` (VST3) juste après `Wrapper::<OmorePunch>::new(...)`.
- **Rationale** :
  - Garantit l'isolation stricte entre multiples instances du plugin dans Reaper (par ex. Drum Bus + Snare Bus).
  - N'intervient qu'à la création de l'instance sur le thread principal ; aucun coût ni synchronisation sur le thread audio.
- **Alternatives considered** :
  - *Variable globale `static`* : interdit car plusieurs instances écraseraient mutuellement leur mesure.

### Décision 4 : Suppression de `gr_meter_db` et sanitisation de la lecture hôte
- **Decision** :
  - Supprimer `OmorePunchParams::gr_meter_db` et le bloc `setter.set_parameter(&params.gr_meter_db, ...)` dans `draw_omore_punch_ui`.
  - Centraliser la conversion `MeterTelemetry::host_gain_reduction_db(&self) -> f64` : lire `current_gr_db` en `Ordering::Relaxed`, retourner `0.0` si non-fini ou `<= 0.0` (évitant `-0.0`), sinon `-(val.min(MAX_GR_METER_DB) as f64)` dans `[-24.0, 0.0]`.
  - Réinitialiser `telemetry.current_gr_db` à `0.0` dans `OmorePunch::reset(&mut self)` et `MeterTelemetry::reset_peaks(&self)` (ou `reset_all`).
- **Rationale** :
  - Supprime définitivement le paramètre superflu de la liste des contrôles du DAW (FR-004).
  - Garantit que la jauge Reaper et le vumètre interne `egui` affichent exactement la même valeur bornée dans `[0, 24] dB` (en négatif `[-24.0, 0.0] dB` côté DAW), indépendamment du `Mix` parallèle, du make-up gain et du clipper, et que la fenêtre `egui` soit ouverte ou fermée.
  - Assure zéro allocation et zéro verrou sur `process()` et sur `clap_plugin_gain_adjustment_metering::get()`.
