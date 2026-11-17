# Quickstart & Validation Guide: Bouton Bypass Global (`008-bypass-button`)

## Prerequis

- Se placer dans le worktree dedie : `/home/tdavio/src/omore-punch/.worktrees/008-bypass-button` (branche `feat/008-bypass-button`).
- Toolchain Rust installee (`cargo`, `rustc`).

## 1. Controle ASCII strict de `src/lib.rs`

```bash
LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs
```

**Attendu**: aucune sortie (code de retour `1`, zero caractere non-ASCII).

## 2. Suite de tests unitaires DSP, ABI & UI

```bash
cargo test --release
```

**Attendu**: 100 % des tests passent, incluant :
- Transparence du bypass (`< -120 dBFS` apres transition <= 20 ms) ;
- Transition lissée sans discontinuité a l'activation et a la desactivation (y compris bascules rapides) ;
- Maintien du safety clamp / sanitisation `NaN`/`Inf` en mode bypass ;
- `gr_db == 0.0`, `clipped == false` et gel du Gain Match pendant le bypass ;
- Exposition du parametre `"bypass"` avec `ParamFlags::BYPASS` (CLAP `CLAP_PARAM_IS_BYPASS` & VST3 `kIsBypass`), defaut `false` ;
- Non-depassement de l'en-tête (`test_ui_header_fits_window_with_dev_badge`) et de la grille UI (`test_ui_layout_fits_without_overlap`).

## 3. Generation des bundles VST3 et CLAP

```bash
cargo run --package xtask --release -- bundle omore-punch --release
```

**Attendu**: generation de `target/bundled/omore-punch.vst3` et `target/bundled/omore-punch.clap`.

## 4. Verification manuelle dans Reaper (VST3 et CLAP)

1. Charger `Omore Punch` sur une piste de bus batterie avec compression marquee (ex. Threshold `-20 dB`, Ratio `8:1`, EQ boost, Soft Clip).
2. Verifier qu'au chargement `[BYPASS: OFF]` est affiche et le traitement est actif.
3. Cliquer `[BYPASS: OFF]` -> passe a `[BYPASS: ON]` en surbrillance ; les sections de controles s'attenuent visuellement mais restent modifiables ; aucun clic audio ; `GR` affiche `-00.0 dB` tandis que `Max GR` reste conserve ; `Out L/R` egale `In L/R`.
4. Cliquer la case Bypass native dans la barre FX de Reaper -> le bouton `[BYPASS: ON]`/`[BYPASS: OFF]` de l'interface suit immediatement dans les deux sens.
5. Sauvegarder et recharger le projet Reaper avec le bypass actif -> l'etat est restaure a l'identique.
