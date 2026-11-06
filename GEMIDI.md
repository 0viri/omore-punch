# GEMINI.md - Directives Strictes du Projet Omore Punch

Ce fichier définit les règles d'architecture, de DSP temps réel et d'interface graphique du projet **Omore Punch** (`omore-punch`).
**IMPÉRATIF :** Tu dois lire et respecter intégralement ce document avant toute modification du code source.
**Référence :** La constitution du projet (`.specify/memory/constitution.md`) fait foi. Ce document la détaille et doit rester cohérent avec elle : une règle modifiée ici doit l'être aussi dans la constitution (via `/speckit-constitution`), et inversement.

## 1. Identité & Stack Technique
- **Nom du plugin :** `Omore Punch` (`omore-punch`)
- **Rôle :** Compresseur parallèle et Clipper de Bus Batterie Stéréo (Stereo Drum Bus).
- **Stack :** Rust (Edition 2021), `nih-plug` et `nih_plug_egui`.
- **Formats cibles :** Export simultané **VST3** (`nih_export_vst3!`) et **CLAP** (`nih_export_clap!`).
- **OS & DAW cibles :** 100 % compatible avec **Reaper** sous **Linux (Debian 13)** et **Windows 11**.

## 2. Règles Architecturales Absolues
1. **Architecture Monolithique (`src/lib.rs`) :**
   - Tout le code du plugin (moteur DSP, paramètres, télémétrie atomique, UI `egui`, exports VST3/CLAP et tests unitaires) DOIT rester dans le fichier unique `src/lib.rs`. Ne crée pas de sous-modules éparpillés.
   - Les outils de build (`xtask/`, `build.sh`) ne sont pas du code du plugin et ne sont pas concernés par cette règle.
   - `src/lib.rs` est organisé en quatre blocs isolés (voir `ARCHITECTURE.md`) : tout nouveau code DOIT rejoindre le bloc auquel il appartient. Le moteur DSP (`StereoPunchDsp`, `DspConfig`) ne DOIT dépendre ni de l'UI ni du wrapper hôte.
2. **100 % Pure ASCII dans `src/lib.rs` (Zéro Emoji / Zéro Unicode non-ASCII) :**
   - Il est strictement interdit d'utiliser des emojis ou des caractères accentués/Unicode non-ASCII dans `src/lib.rs` (commentaires inclus) afin d'éviter l'affichage de carrés vides (`□`) dans `egui` sous Linux.
   - Vérifie systématiquement avec : `LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs` (qui ne doit rien retourner). La CI exécute ce contrôle à chaque push et pull request vers `main`.
   - Cette règle ne vise que `src/lib.rs` : la documentation Markdown (specs Spec Kit comprises) peut contenir des accents.
3. **Versioning Dynamique :**
   - Le numéro de version est défini uniquement dans `Cargo.toml`.
   - Utilise toujours `env!("CARGO_PKG_VERSION")` dans les métadonnées du plugin et dans l'en-tête de l'interface graphique `egui`. Ne code jamais un numéro de version en dur dans `src/lib.rs`.

## 3. Contraintes Strictes du Thread Audio Temps Réel (`process`)
1. **Zéro Allocation Mémoire :**
   - Interdiction absolue d'utiliser `Vec`, `String`, `Box`, `format!`, `.to_string()`, ou `.clone()` sur des types alloués sur le tas dans `process()` et dans tout code qu'il appelle (dont `StereoPunchDsp`).
   - La feature `assert_process_allocs` de `nih-plug` DOIT rester activée dans `Cargo.toml`. Elle fait planter le plugin en cas d'allocation, mais uniquement en build debug et à l'intérieur du wrapper `process` de `nih-plug` (par exemple un bundle compilé sans `--release` et chargé dans Reaper).
   - Les tests unitaires appellent le DSP directement et la CI tourne en `--release` : `cargo test --release` ne détecte donc aucune allocation (d'où la règle 3.5).
2. **Zéro Verrou Bloquant :**
   - Interdiction absolue d'utiliser `Mutex`, `RwLock` ou des canaux bloquants dans `process()` et dans tout code qu'il appelle (dont `StereoPunchDsp`).
   - Toute communication entre le thread audio et l'UI doit passer par les paramètres `nih-plug` ou par `MeterTelemetry` (`AtomicU32` via `to_bits`/`from_bits` et `AtomicBool` avec `Ordering::Relaxed`).
3. **Précalcul des Fonctions Coûteuses :**
   - N'appelle jamais `.exp()`, `.sin()`, `.cos()` ou `.tan()` à chaque échantillon si les paramètres (`sample_rate`, `attack_ms`, `release_ms`, fréquences de filtres) n'ont pas changé. Mets en cache la dernière valeur connue et ne recalcule les coefficients que lors d'une variation.
4. **Sanitisation & Safety Clamp Anti Auto-Mute Reaper :**
   - Chaque échantillon entrant et sortant (ainsi que les variables d'état internes d'enveloppe et de filtres) doit être protégé contre `NaN`, `Inf` et les dénormaux.
   - La sortie finale doit toujours passer par `sanitize_sample()` qui remplace toute valeur non-finie par `0.0` et borne le signal dans `[-4.0, 4.0]`.
   - Un test unitaire DOIT continuer à couvrir ce comportement (actuellement `test_nan_inf_and_safety_clamp`).
5. **Relecture Obligatoire :**
   - Tout changement dans `process()` ou `StereoPunchDsp` DOIT être relu au regard des règles 3.1 à 3.4.

## 4. Standards de l'Interface Graphique (`egui`)
- **Style :** Thème sombre inspiré d'`Omore Sampler` (`apply_omore_dark_theme`).
- **Texte :** 100 % ASCII, comme tout `src/lib.rs` (règle 2.2).
- **À compléter (mainteneur) :** le texte d'origine de cette section est tronqué. Les standards `egui` restants seront ajoutés ici, puis reportés dans la constitution (`TODO(GUI_STANDARDS)`).

## 5. Tests DSP Obligatoires
- Tout comportement DSP DOIT être vérifié par des tests unitaires dans le module `tests` de `src/lib.rs`. Les tests existants (compression stéréo liée, HPF de sidechain, plafond du clipper, mix dry/wet, sanitisation et safety clamp) DOIVENT continuer à passer.
- Toute fonctionnalité DSP nouvelle ou modifiée DOIT ajouter ou mettre à jour au moins un test qui vérifie son comportement numérique (exemple : `test_sidechain_hpf_kick_immunity`, livré avec le HPF de sidechain en v0.2.0).
- `cargo test --release` DOIT passer sous Linux et Windows avant toute arrivée sur `main` et avant toute release.

## 6. Contrôles Qualité & Livraison
- Avant toute arrivée sur `main` et avant tout tag de release (`v*`), ces contrôles DOIVENT passer (ils reprennent `.github/workflows/build.yml`) :
  1. Contrôle ASCII de `src/lib.rs` (règle 2.2).
  2. `cargo test --release` (section 5).
  3. Génération des bundles VST3 et CLAP sous Linux x86_64 et Windows x86_64 : `cargo run --package xtask --release -- bundle omore-punch --release` DOIT produire `omore-punch.vst3` et `omore-punch.clap` dans `target/bundled/`.
- Chaque release DOIT incrémenter la version dans `Cargo.toml` (règle 2.3) et consigner ses changements notables dans `CHANGELOG.md`.
