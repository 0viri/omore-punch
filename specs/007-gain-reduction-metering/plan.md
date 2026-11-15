# Implementation Plan: DAW Gain Reduction Metering

**Branch**: `007-gain-reduction-metering` | **Date**: 2026-11-15 | **Spec**: [spec.md](file:///home/tdavio/src/omore-punch/specs/007-gain-reduction-metering/spec.md)

**Input**: Feature specification from `/specs/007-gain-reduction-metering/spec.md`

## Summary

Exposer en temps réel la réduction de gain brute du compresseur (`0.0 dB` au repos jusqu'à `-24.0 dB` en compression) vers le DAW hôte (Reaper sur Linux et Windows) aux formats **VST3** et **CLAP**, sans exposer aucun paramètre dans la liste des contrôles ni dans les enveloppes d'automation.
L'approche technique consiste à conserver le calcul temps réel existant (`MeterTelemetry::current_gr_db` mis à jour par `self.dsp.env_gr_db` dans `process()`), à supprimer l'ancien `FloatParam` `gr_meter_db` qui polluait la liste des paramètres et dépendait de l'UI ouverte, et à remplacer les macros `nih_export_clap!` / `nih_export_vst3!` par des wrappers d'export locaux dans `src/lib.rs` implémentant nativement l'extension CLAP `clap.gain-adjustment-metering/0` et l'interface COM VST3 `PreSonus::IGainReductionInfo` (`{8E3C292C-9592-4F9D-B259-0B1E100E4198}`).

## Technical Context

**Language/Version**: Rust (Edition 2021)
**Primary Dependencies**: `nih_plug`, `nih_plug_egui`, `atomic_float` (et `vst3_sys` / `clap_sys` réexportés via `nih_plug::wrapper`)
**Storage**: N/A
**Testing**: `cargo test --release` (tests unitaires DSP et contrats ABI dans `src/lib.rs`) + validation manuelle dans Reaper (VST3 & CLAP)
**Target Platform**: Linux x86_64 (Debian 13) et Windows x86_64 (Windows 11) — DAW Reaper
**Project Type**: Plugin audio VST3 & CLAP monolithique
**Performance Goals**: Rafraîchissement fluide synchrone avec l'enveloppe Attack/Release/Auto-Release (44,1 kHz à 192 kHz, buffers variables), latence nulle sur le chemin de lecture atomique
**Constraints**: Zéro allocation et zéro verrou sur le thread audio (`process` et `clap_plugin_gain_adjustment_metering::get`), 100 % ASCII dans `src/lib.rs`, architecture monolithique dans `src/lib.rs`, aucun paramètre de GR visible dans la liste des contrôles DAW
**Scale/Scope**: Un seul fichier source plugin (`src/lib.rs`) ; prise en charge multi-instances sans état global partagé

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Principe I — Monolithic Single-File Architecture** : **PASS**. Toute l'implémentation (helper `MeterTelemetry`, suppression `gr_meter_db`, wrappers d'export CLAP et VST3, tests unitaires) réside exclusivement dans `src/lib.rs` dans ses blocs dédiés. Aucun nouveau fichier source Rust ni modification de `Cargo.toml`.
- **Principe II — Pure ASCII Source** : **PASS**. Tout le code, les commentaires et les chaînes ajoutés dans `src/lib.rs` sont en anglais 100 % ASCII, vérifié par `LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs`.
- **Principe III — Real-Time Audio Thread Safety (NON-NEGOTIABLE)** : **PASS**. `process()` conserve uniquement `store_f32_relaxed` sur `telemetry.current_gr_db`. `clap_plugin_gain_adjustment_metering::get()` et `IGainReductionInfo::get_gain_reduction_value_in_db()` n'effectuent qu'une lecture atomique `Relaxed` (`load_f32_relaxed`), sans allocation ni verrou. La capture `thread_local!` de `Arc<MeterTelemetry>` n'a lieu qu'à l'instanciation sur le thread principal.
- **Principe IV — Signal Sanitization & Safety Clamp (NON-NEGOTIABLE)** : **PASS**. `StereoPunchDsp` et `sanitize_sample()` restent intacts. La fonction `MeterTelemetry::host_gain_reduction_db()` protège contre `NaN`, `Inf` et `-0.0`, et borne la valeur dans `[-24.0, 0.0]`.
- **Principe V — Mandatory DSP Unit Tests** : **PASS**. Tous les tests existants restent inchangés (hors retrait éventuel d'une référence à `gr_meter_db` s'il y en avait), et de nouveaux tests vérifient numériquement la mesure hôte (repos, compression, bornage `-24 dB`, indépendance Make-up/Clipper/Mix, `reset()`, multi-taux d'échantillonnage `44.1..192 kHz`) ainsi que les contrats d'export CLAP/VST3 et l'absence de paramètre `gr_meter_db`.
- **Principe VI — Single-Source Versioning** : **PASS**. `Cargo.toml` reste sur `0.3.1-dev` (aucune modification de version hors release), et `env!("CARGO_PKG_VERSION")` reste utilisé partout.

*Re-évaluation post-Phase 1* : Toutes les portes restent vertes sans aucune déviation ni complexité injustifiée.

## Project Structure

### Documentation (this feature)

```text
specs/007-gain-reduction-metering/
├── plan.md              # Ce document (/speckit-plan)
├── research.md          # Phase 0 : analyse nih_plug, CLAP & VST3 ABI, décisions
├── data-model.md        # Phase 1 : entités télémétrie, wrappers et transitions
├── quickstart.md        # Phase 1 : commandes de vérification et validation Reaper
├── contracts/
│   └── host-gr-metering-abi.md  # Contrats CLAP, VST3 COM et liste de paramètres
└── tasks.md             # Phase 2 (généré par /speckit-tasks)
```

### Source Code (repository root)

```text
src/
└── lib.rs               # Fichier monolithique unique :
                         # - Bloc 2 (MeterTelemetry) : host_gain_reduction_db() & reset
                         # - Bloc 3 (OmorePunchParams, OmorePunch, exports CLAP/VST3) :
                         #   retrait gr_meter_db, capture instance, wrappers CLAP/VST3
                         # - Bloc 4 (draw_omore_punch_ui) : retrait synchro gr_meter_db
                         # - Bloc 5 (tests) : tests unitaires DSP & contrats ABI
```

**Structure Decision**: Aucune création de fichier Rust additionnel conformément au Principe I de la Constitution. Les changements sont strictement confinés à `src/lib.rs` (et `CHANGELOG.md` lors de l'implémentation sous `## [Unreleased]`).

## Complexity Tracking

Aucune violation de la Constitution détectée ; tableau vide.
