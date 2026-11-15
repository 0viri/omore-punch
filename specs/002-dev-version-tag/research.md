# Phase 0 Research: Tag de build Git dans le badge de version de développement (DEV)

**Feature**: `002-dev-version-tag`
**Date**: 2026-11-14

## Decisions

### 1. Capture du hash Git et de l'état `-dirty` à la compilation via `build.rs`
- **Decision**: Ajouter un script de build Cargo standard `build.rs` à la racine du dépôt (tooling de build hors code plugin, autorisé par le Principe I de la constitution) qui injecte la variable d'environnement de compilation `OMORE_GIT_TAG` via `println!("cargo:rustc-env=OMORE_GIT_TAG=...")` :
  - Exécute `git rev-parse --short=7 HEAD` pour obtenir le hash court exact sur 7 caractères (ex. `1d2c5b9`).
  - Exécute `git status --porcelain` ; si la sortie non-trimmée est non vide, ajoute le suffixe `-dirty` (ex. `1d2c5b9-dirty`).
  - En cas d'échec ou d'absence de `git`/`.git`, émet une chaîne vide (`""`) afin de ne jamais bloquer la compilation.
  - Émet `cargo:rerun-if-changed` sur `src/lib.rs`, `Cargo.toml`, `build.rs`, ainsi que sur `.git/HEAD`, `.git/index` et le fichier de référence active sous `.git/` (s'ils existent) pour que le tag `-dirty` et le hash se mettent à jour automatiquement lors de toute modification locale, commit ou changement de branche.
- **Rationale**:
  - `git describe --always` a été écarté car dès qu'un tag Git (`v0.2.0`) existe dans l'historique, `git describe` renvoie `v0.2.0-N-g1d2c5b9` au lieu du hash court `1d2c5b9` demandé. `git rev-parse --short=7 HEAD` + `git status --porcelain` garantit exactement le format `1d2c5b9` / `1d2c5b9-dirty` sous Linux et Windows.
  - `build.rs` est du build tooling Cargo au même titre que `xtask/` et `build.sh` et respecte le Principe I.
- **Alternatives considered**:
  - *Injecter la variable uniquement dans `build.sh` / `build.ps1`* : rejeté car un appel direct `cargo test --release` ou `cargo run --package xtask ...` ou un build CI ne passerait plus par ces scripts et perdrait le hash.
  - *Lire `.git` manuellement en Rust pur* : rejeté car `git status --porcelain` reste la référence fiable pour `-dirty` (fichiers modifiés, indexés ou non suivis, packed-refs).

### 2. Formatage du badge `[DEV <hash>]` / `[DEV <hash>-dirty]` dans `src/lib.rs`
- **Decision**: Conserver `parse_display_version(version: &str) -> (&str, bool)` (zéro allocation) et ajouter un helper pur `format_dev_badge_label(git_tag: &str) -> String` (appelé uniquement dans le thread UI) :
  - Si `git_tag.trim()` est non vide (et ASCII) : retourne `format!("[DEV {}]", trimmed)` (ex. `"[DEV 1d2c5b9]"` ou `"[DEV 1d2c5b9-dirty]"`).
  - Sinon (hors dépôt Git / repli) : retourne `"[DEV]".to_string()`.
  - Le badge reste affiché uniquement quand `is_dev == true` (version se terminant par `"-dev"`). En release (`is_dev == false`), ni `[DEV]` ni hash Git ne sont affichés.
- **Rationale**: Combine l'identification immédiate de l'état dev avec la traçabilité exacte du commit et de l'état dirty, tout en respectant le Principe VI (`Cargo.toml` reste l'unique source de version et `OmorePunch::VERSION` reste `env!("CARGO_PKG_VERSION")`).

### 3. Ajustement typographique de l'en-tête pour tenir dans `UI_WINDOW_WIDTH = 820`
- **Decision**: Dans `TopBottomPanel::top("omore_punch_header")`, passer la taille de police du titre et du sous-titre à `13.5` (au lieu de `15.0`), celle du badge `[DEV ...]` à `11.5` (`Margin::symmetric(5, 1)`), celle du bouton `[CLIP: ...]` à `12.5` et l'espacement local `item_spacing.x = 5.0`.
- **Rationale**: Garantit qu'avec le pire cas `[DEV 1d2c5b9-dirty]` à gauche et `[CLIP: ACTIVE - CLICK TO RESET]` à droite, l'en-tête complet tient confortablement dans `820 px` sans débordement ni retour à la ligne sur Linux et Windows, vérifié empiriquement par le test unitaire `egui` dans `src/lib.rs`.
