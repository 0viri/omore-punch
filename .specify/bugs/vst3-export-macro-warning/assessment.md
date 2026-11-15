# Bug Assessment: Warning de compilation `semicolon_in_expressions_from_non_local_macros` sur `nih_export_vst3!`

- **Slug**: vst3-export-macro-warning
- **Created**: 2026-11-14
- **Source**: pasted text (sortie de `./build.sh`)
- **Verdict**: valid
- **Severity**: low

## Report (verbatim or summarized)

Sortie de `./build.sh` (`cargo test --release` et `cargo run --package xtask --release -- bundle omore-punch --release`) sous `rustc 1.99.0` :

```text
warning: trailing semicolon in macro used in expression position
    --> src/lib.rs:1409:1
     |
1409 | nih_export_vst3!(OmorePunch);
     | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
     |
     = warning: this was previously accepted by the compiler but is being phased out; it will become a hard error in a future release!
     = note: for more information, see issue #79813 <https://github.com/rust-lang/rust/issues/79813>
     = note: `#[warn(semicolon_in_expressions_from_non_local_macros)]` (part of `#[warn(future_incompatible)]`) on by default
     = note: this warning originates in the macro `vst3_com::vtable` which comes from the expansion of the macro `nih_export_vst3` (in Nightly builds, run with -Z macro-backtrace for more info)

warning: `omore-punch` (lib) generated 3 warnings (1 duplicate)
warning: `omore-punch` (lib test) generated 3 warnings (3 duplicates)
```

Aucune URL externe n'a été fournie dans le rapport (pas de récupération réseau requise par la politique URL Trust Policy).

## Symptom

Lors de l'exécution de `./build.sh`, `cargo test --release` et `cargo run --package xtask --release -- bundle omore-punch --release`, le compilateur Rust (`rustc 1.99.0`) émet 3 warnings `trailing semicolon in macro used in expression position` (`#[warn(semicolon_in_expressions_from_non_local_macros)]`, faisant partie de `#[warn(future_incompatible)]`) pointant sur `nih_export_vst3!(OmorePunch);` à la ligne `src/lib.rs:1409`. Il est attendu que la compilation et le bundling s'exécutent sans aucun warning (`0 warnings`).

## Reproduction

1. À la racine du dépôt `/home/tdavio/src/omore-punch`, exécuter `./build.sh` (ou `cargo test --release` / `cargo check --tests`).
2. Observer les 3 warnings `semicolon_in_expressions_from_non_local_macros` émis sur `src/lib.rs:1409:1` (`nih_export_vst3!(OmorePunch);`).

## Suspected Code Paths

- `src/lib.rs:1409` — Appel de la macro `nih_export_vst3!(OmorePunch);` qui génère le module caché `mod vst3` et le point d'entrée VST3 `GetPluginFactory`.
- `Cargo.toml:16` & `Cargo.lock:1057-1059, 2052-2062` — La dépendance `nih_plug` (`de421011f41a6d10fc8c7a6084e4f4dee0143683`) dépend de `vst3-sys` / `vst3-com` (`b3ff4d775940f5b476b9d1cca02a90e07e1922a2`). Dans `nih_plug::wrapper::vst3::nih_export_vst3!`, la structure `Factory` est annotée avec `#[VST3(implements(IPluginFactory, IPluginFactory2, IPluginFactory3))]`, qui invoque 3 fois la macro externe `vst3_com::vtable!` en position d'expression avec un point-virgule final.

## Root Cause Hypothesis

La cause est identifiée avec certitude (confiance : **high**) :
La macro `nih_export_vst3!(OmorePunch);` (`src/lib.rs:1409`) s'expanse en un module caché `#[doc(hidden)] mod vst3 { ... }` contenant `#[VST3(implements(IPluginFactory, IPluginFactory2, IPluginFactory3))] pub struct Factory`. La proc-macro `#[VST3(...)]` (issue de `vst3-com-macros` dans `vst3-sys`) génère dans `Factory::allocate` trois appels à la macro déclarative `vst3_com::vtable!` en position d'expression, dont la définition amont se termine par un point-virgule. Les versions récentes de `rustc` activent par défaut le lint `semicolon_in_expressions_from_non_local_macros` (`future_incompatible`, Rust issue #79813) et rapportent l'avertissement au point d'invocation `src/lib.rs:1409`.

## Proposed Remediation

**Preferred**:
Ajouter l'attribut `#[allow(semicolon_in_expressions_from_non_local_macros)]` directement au-dessus de `nih_export_vst3!(OmorePunch);` dans `src/lib.rs` (avec un commentaire 100 % ASCII expliquant que le warning provient de `vst3_com::vtable` dans l'expansion de `nih_export_vst3!`). Comme `mod vst3` est le premier item généré par `nih_export_vst3!`, l'attribut `#[allow(semicolon_in_expressions_from_non_local_macros)]` s'applique uniquement à ce module généré sans masquer le lint sur le reste de `src/lib.rs`. (Si une version future de `nih_export_vst3!` plaçait `Factory` hors du premier item, un `#![allow(semicolon_in_expressions_from_non_local_macros)]` en tête de `src/lib.rs` ou un module englobant servirait de repli immédiat.)

**Alternatives** (optional):
- Ajouter `[lints.rust]` avec `semicolon_in_expressions_from_non_local_macros = "allow"` dans `Cargo.toml` : fonctionne aussi, mais désactive le lint à l'échelle de tout le crate au lieu de cibler uniquement l'expansion de `nih_export_vst3!` dans `src/lib.rs`.
- Surcharger `vst3-sys` via `[patch]` dans `Cargo.toml` : disproportionné pour un lint de macro amont et introduit une dépendance Git supplémentaire à maintenir.

**Files likely to change**:
- `src/lib.rs`
- `CHANGELOG.md`

**Tests to add or update**:
- Vérifier que `cargo test --release` et `cargo run --package xtask --release -- bundle omore-punch --release` compilent sans aucun warning (`0 warnings`) et que les 7 tests unitaires existants ainsi que le contrôle ASCII (`LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs`) passent.

## Risks & Considerations

- **Conformité ASCII (`GEMINI.md` règle 2.2) :** Tout commentaire accompagnant l'attribut `#[allow(semicolon_in_expressions_from_non_local_macros)]` dans `src/lib.rs` doit être rédigé en anglais et 100 % ASCII.
- **Export des symboles VST3 :** L'attribut `#[allow(...)]` ne modifie en rien la visibilité ni l'export C (`#[no_mangle] pub extern "system" fn GetPluginFactory`) généré par `nih_export_vst3!`.
- **Isolation Git (`GEMINI.md` section 9) :** Le correctif devra être appliqué sur une branche dédiée `fix/vst3-export-macro-warning` créée depuis `main` (indépendamment de la branche de fonctionnalité `feat/001-snare-safe-200hz`).

## Open Questions

- Aucune question ouverte : la cause et le correctif sont entièrement déterminés.
