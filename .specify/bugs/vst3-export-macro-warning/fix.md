# Bug Fix: Warning de compilation `semicolon_in_expressions_from_non_local_macros` sur `nih_export_vst3!`

- **Slug**: vst3-export-macro-warning
- **Fixed**: 2026-11-14
- **Assessment**: ./assessment.md
- **Status**: applied

## Summary

Ajout de l'attribut interne `#![allow(semicolon_in_expressions_from_non_local_macros)]` (accompagné d'un commentaire 100 % ASCII en anglais) en tête de `src/lib.rs` afin de supprimer les warnings émis par `rustc 1.99.0` lors de l'expansion de `nih_export_vst3!(OmorePunch)` via la macro transitive `vst3_com::vtable!`, sans modifier ni le code DSP ni l'interface UI.

## Changes

| File | Change | Notes |
|------|--------|-------|
| `src/lib.rs` | modified | Ajout de `#![allow(semicolon_in_expressions_from_non_local_macros)]` et d'un commentaire ASCII en tête de crate (lignes 7-9). |
| `CHANGELOG.md` | modified | Ajout d'une entrée sous `## [Unreleased] -> ### Corrigé` documentant le correctif `vst3-export-macro-warning`. |

## Diff Highlights (optional)

```rust
// Suppress future-incompatibility lint triggered inside nih_plug::nih_export_vst3!
// by transitive macro vst3_com::vtable! (Rust issue #79813).
#![allow(semicolon_in_expressions_from_non_local_macros)]

use nih_plug::prelude::*;
```

## Tests Added or Updated

- Aucun nouveau test unitaire DSP ajouté (le défaut porte uniquement sur un lint de compilation généré par une macro amont `nih_plug` / `vst3_com`) ; vérification de non-régression sur les 7 tests unitaires existants et compilation complète à `0` warning.

## Local Verification

- Commands run:
  - `LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs` → code de sortie `1`, aucune sortie (`0` caractère non-ASCII détecté).
  - `cargo test --release` → compilation sans warning (`0` warning), `7 passed; 0 failed; 0 ignored`.
  - `cargo run --package xtask --release -- bundle omore-punch --release` → compilation sans warning, génération de `target/bundled/omore-punch.clap` et `target/bundled/omore-punch.vst3`.
- Manual checks: Possibilité de relancer `./build.sh` pour vérifier l'absence de warning sur l'ensemble du script d'installation locale.

## Deviations from Assessment

- L'attribut externe `#[allow(semicolon_in_expressions_from_non_local_macros)]` placé directement au-dessus de `nih_export_vst3!(OmorePunch);` a été rejeté par `rustc 1.99.0` avec `warning: unused attribute allow` (`the built-in attribute allow will be ignored, since it's applied to the macro invocation nih_export_vst3`) et laissait subsister les warnings d'origine. Le repli prévu dans `./assessment.md` (`#![allow(semicolon_in_expressions_from_non_local_macros)]` en tête de `src/lib.rs`) a donc été appliqué.

## Follow-ups

- Exécuter `/speckit-bug-test slug=vst3-export-macro-warning`.
