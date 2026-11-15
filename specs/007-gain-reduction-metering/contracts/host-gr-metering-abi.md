# Interface Contracts: Host Gain Reduction Metering (CLAP & VST3)

**Feature**: `007-gain-reduction-metering`
**Date**: 2026-11-15

## 1. Contrat CLAP : `clap.gain-adjustment-metering/0`

- **Identifiant d'extension** : `"clap.gain-adjustment-metering/0"` (chaîne C terminée par `\0`).
- **Point d'entrée** : `clap_plugin::get_extension(plugin, id)` retourne un pointeur statique vers `ClapPluginGainAdjustmentMetering` lorsque `id == "clap.gain-adjustment-metering/0"`, sinon délègue à l'implémentation `nih_plug`.
- **Layout C ABI** :
  ```c
  typedef struct clap_plugin_gain_adjustment_metering {
      // [audio-thread] Retourne l'ajustement de gain courant en dB (<= 0.0 en réduction)
      double (CLAP_ABI *get)(const clap_plugin_t *plugin);
  } clap_plugin_gain_adjustment_metering_t;
  ```
- **Invariant temps réel** : `get(plugin)` est annoté `[audio-thread]` ; il ne doit exécuter aucune allocation, aucun verrou, aucun appel système bloquant (lecture atomique `Relaxed` uniquement).
- **Plage de retour** : `f64` dans `[-24.0, 0.0]` (`0.0` exact lorsqu'aucune réduction n'est active ou après `stop_processing`/`deactivate`/`reset`).

## 2. Contrat VST3 : `PreSonus::IGainReductionInfo`

- **Identifiant COM (IID)** : `{8E3C292C-9592-4F9D-B259-0B1E100E4198}`
  - Encodage Linux/macOS : `[0x8E, 0x3C, 0x29, 0x2C, 0x95, 0x92, 0x4F, 0x9D, 0xB2, 0x59, 0x0B, 0x1E, 0x10, 0x0E, 0x41, 0x98]`
  - Encodage Windows COM : généré automatiquement par `#[vst3_sys::com_interface("8E3C292C-9592-4F9D-B259-0B1E100E4198")]`.
- **Héritage COM** : hérite de `IUnknown` (`queryInterface`, `addRef`, `release`).
- **Méthode** :
  ```cpp
  virtual double PLUGIN_API getGainReductionValueInDb() = 0;
  ```
- **Accessibilité** : `queryInterface(IID_IGAIN_REDUCTION_INFO, &ptr)` doit réussir depuis toute interface COM exposée par le plugin (`IComponent`, `IEditController`, `IAudioProcessor`, etc.) avec incrément de compteur de référence COM standard.
- **Plage de retour** : `f64` dans `[-24.0, 0.0]` (`0.0` exact au repos ou après `set_processing(false)`/`set_active(false)`/`reset`).

## 3. Contrat de liste de paramètres hôte

- **Aucun paramètre de metering exposé** : `OmorePunchParams` ne contient plus `gr_meter_db`.
- Le nombre de paramètres exposés via `IEditController::getParameterCount` (VST3) et `clap_plugin_params::count` (CLAP) correspond strictement aux contrôles utilisateur du compresseur, du clipper, du mix parallèle, du gain match et de l'EQ.
- L'ancien état DAW contenant éventuellement `"gr_meter_db"` dans les données JSON sérialisées de `nih_plug` reste toléré au chargement sans erreur.
