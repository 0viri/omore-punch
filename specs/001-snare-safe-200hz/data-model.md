# Phase 1 Data Model: Remplacement du bouton rapide Kick Safe 90Hz par Snare Safe 200Hz

**Feature**: `001-snare-safe-200hz`
**Date**: 2026-11-14

## Entities & State Representation

Cette fonctionnalité ne crée aucune nouvelle entité persistante ni nouvelle structure de données ; elle agit sur le paramètre existant de filtre passe-haut de détection (`sc_hpf_hz`).

### 1. Paramètre Hôte : `OmorePunchParams::sc_hpf_hz`
| Champ / Propriété | Valeur | Notes |
|-------------------|--------|-------|
| Identifiant stable (`#[id]`) | `"sc_hpf_hz"` | Inchangé (rétrocompatibilité sessions DAW) |
| Nom affiché | `"SC HPF"` | Inchangé |
| Type | `FloatParam` | Paramètre flottant lissé (`SmoothingStyle::Linear(5.0)`) |
| Plage | `20.0 Hz` à `250.0 Hz` | `FloatRange::Skewed` (`factor: skew_factor(-1.0)`) |
| Valeur par défaut (`Reset`) | `20.0 Hz` | Bypass du filtre de détection (`<= 20.5 Hz`) |
| Valeur cible `[Snare Safe 200Hz]` | `200.0 Hz` | Appliquée en un clic via `ParamSetter` |

### 2. Configuration Temps Réel : `DspConfig::sc_hpf_hz`
| Champ | Type | Contrainte / Invariant |
|-------|------|------------------------|
| `sc_hpf_hz` | `f32` | Borné dans `[20.0, 250.0]` par `StereoPunchDsp::update_sidechain_hpf` ; bypass complet si `<= 20.5 Hz`, filtre RC 1er ordre précalculé (`sc_hpf_coeff`) sinon. |

## State Transitions (UI Quick Action)

1. **État initial** : `sc_hpf_hz` vaut $f \in [20.0, 250.0]\text{ Hz}$.
2. **Action utilisateur** : Clic sur le bouton `[Snare Safe 200Hz]`.
3. **Transition** :
   - `setter.begin_set_parameter(&params.sc_hpf_hz)`
   - `setter.set_parameter(&params.sc_hpf_hz, 200.0)`
   - `setter.end_set_parameter(&params.sc_hpf_hz)`
4. **État final** : `sc_hpf_hz` converge vers `200.0 Hz` (via le lisseur linéaire de `5.0 ms` sur le thread audio), `StereoPunchDsp` met à jour `sc_hpf_coeff` uniquement pendant la transition et stabilise le cache `cached_sc_hpf_hz = 200.0`.
