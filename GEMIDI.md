# GEMINI.md - Directives Strictes du Projet Omore Punch

Ce fichier définit les règles d'architecture, de DSP temps réel et d'interface graphique du projet **Omore Punch** (`omore-punch`).
**IMPÉRATIF :** Tu dois lire et respecter intégralement ce document avant toute modification du code source.

## 1. Identité & Stack Technique
- **Nom du plugin :** `Omore Punch` (`omore-punch`)
- **Rôle :** Compresseur parallèle et Clipper de Bus Batterie Stéréo (Stereo Drum Bus).
- **Stack :** Rust (Edition 2021), `nih-plug` et `nih_plug_egui`.
- **Formats cibles :** Export simultané **VST3** (`nih_export_vst3!`) et **CLAP** (`nih_export_clap!`).
- **OS & DAW cibles :** 100 % compatible avec **Reaper** sous **Linux (Debian 13)** et **Windows 11**.

## 2. Règles Architecturales Absolues
1. **Architecture Monolithique (`src/lib.rs`) :**
   - Tout le code du plugin (moteur DSP, paramètres, télémétrie atomique, UI `egui`, exports VST3/CLAP et tests unitaires) DOIT rester dans le fichier unique `src/lib.rs`. Ne crée pas de sous-modules éparpillés.
2. **100 % Pure ASCII dans `src/lib.rs` (Zéro Emoji / Zéro Unicode non-ASCII) :**
   - Il est strictement interdit d'utiliser des emojis ou des caractères accentués/Unicode non-ASCII dans `src/lib.rs` (commentaires inclus) afin d'éviter l'affichage de carrés vides (`□`) dans `egui` sous Linux.
   - Vérifie systématiquement avec : `LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs` (qui ne doit rien retourner).
3. **Versioning Dynamique :**
   - Utilise toujours `env!("CARGO_PKG_VERSION")` dans les métadonnées du plugin et dans l'en-tête de l'interface graphique `egui`. Ne code jamais un numéro de version en dur dans l'UI.

## 3. Contraintes Strictes du Thread Audio Temps Réel (`process`)
1. **Zéro Allocation Mémoire :**
   - Interdiction absolue d'utiliser `Vec`, `String`, `Box`, `format!`, `.to_string()`, ou `.clone()` sur des types alloués sur le tas dans `process()` ou dans `StereoPunchDsp`.
   - La feature `assert_process_allocs` de `nih-plug` est activée et fera planter les tests en cas d'allocation.
2. **Zéro Verrou Bloquant :**
   - Interdiction absolue d'utiliser `Mutex`, `RwLock` ou des canaux bloquants dans `process()`.
   - Toute communication entre le thread audio et l'UI doit passer par les paramètres `nih-plug` ou par `MeterTelemetry` (`AtomicU32` via `to_bits`/`from_bits` et `AtomicBool` avec `Ordering::Relaxed`).
3. **Précalcul des Fonctions Coûteuses :**
   - N'appelle jamais `.exp()`, `.sin()`, `.cos()` ou `.tan()` à chaque échantillon si les paramètres (`sample_rate`, `attack_ms`, `release_ms`, fréquences de filtres) n'ont pas changé. Mets en cache la dernière valeur connue et ne recalcule les coefficients que lors d'une variation.
4. **Sanitisation & Safety Clamp Anti Auto-Mute Reaper :**
   - Chaque échantillon entrant et sortant (ainsi que les variables d'état internes d'enveloppe et de filtres) doit être protégé contre `NaN`, `Inf` et les dénormaux.
   - La sortie finale doit toujours passer par `sanitize_sample()` qui remplace toute valeur non-finie par `0.0` et borne le signal dans `[-4.0, 4.0]`.

## 4. Standards de l'Interface Graphique (`egui`)
- **Style :** Thème sombre inspiré d'`Omore Sampler` (`apply_omore_dark_theme`).
- **