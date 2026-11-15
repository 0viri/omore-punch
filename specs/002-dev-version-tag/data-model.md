# Phase 1 Data Model: Tag de build Git dans le badge `[DEV]`

**Feature**: `002-dev-version-tag`
**Date**: 2026-11-14

## Entities & State Representation

### 1. Entité dérivée : `DisplayVersion` (`parse_display_version`)

| Champ / Propriété | Type | Description / Règle |
|-------------------|------|---------------------|
| `raw_version` | `&'static str` | Valeur brute issue de `env!("CARGO_PKG_VERSION")` (ex. `"0.2.1-dev"` ou `"0.2.1"`). |
| `base_version` | `&str` | Chaîne affichée après `OMORE PUNCH v` : `raw_version` sans `"-dev"` si présent (et non vide), sinon `raw_version`. |
| `is_dev` | `bool` | `true` uniquement si `raw_version` se termine par `"-dev"` avec un préfixe non vide ; `false` sinon. |

### 2. Entité de build : `OMORE_GIT_TAG` (émise par `build.rs`) & `format_dev_badge_label`

| Champ / Propriété | Type | Description / Règle |
|-------------------|------|---------------------|
| `git_tag` | `&'static str` | Variable de compilation `option_env!("OMORE_GIT_TAG").unwrap_or("")` : `""` si hors Git, `<7-hex>` sur arbre propre, `<7-hex>-dirty` si arbre modifié. |
| `badge_label` | `String` | `"[DEV <git_tag>]"` si `git_tag` non vide et ASCII, sinon `"[DEV]"`. Rendu uniquement quand `is_dev == true`. |

### 3. Matrice des cas

| `Cargo.toml` (`raw_version`) | `OMORE_GIT_TAG` | Badge affiché dans l'en-tête | Version exposée au DAW (`OmorePunch::VERSION`) |
|------------------------------|-----------------|------------------------------|------------------------------------------------|
| `"0.2.1-dev"` | `"1d2c5b9-dirty"` | `[DEV 1d2c5b9-dirty]` | `"0.2.1-dev"` |
| `"0.2.1-dev"` | `"1d2c5b9"` | `[DEV 1d2c5b9]` | `"0.2.1-dev"` |
| `"0.2.1-dev"` | `""` (pas de Git) | `[DEV]` | `"0.2.1-dev"` |
| `"0.2.1"` (release) | `"1d2c5b9"` | *(aucun badge)* | `"0.2.1"` |
| `"0.3.0-rc.1"` | `"1d2c5b9"` | *(aucun badge)* | `"0.3.0-rc.1"` |
