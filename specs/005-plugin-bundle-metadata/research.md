# Research: Métadonnées et Nommage du Bundle Plugin (`005-plugin-bundle-metadata`)

**Date**: 2026-11-14
**Feature**: [spec.md](file:///home/tdavio/src/omore-punch/specs/005-plugin-bundle-metadata/spec.md)

## Décisions Techniques

### 1. Exposition statique des métadonnées VST3 via `Contents/Resources/moduleinfo.json`

- **Decision**: Générer automatiquement `<Bundle>.vst3/Contents/Resources/moduleinfo.json` (spécification Steinberg VST 3.7.5+) lors de l'étape de bundling dans `xtask/src/main.rs`, en renseignant :
  - `Name`: `"Omore Punch"`
  - `Version`: lue dynamiquement depuis `Cargo.toml` (`0.3.1-dev` en dev, `X.Y.Z` en release)
  - `Factory Info`: `Vendor = "Omore"`, `URL = "https://omore-audio.local"`, `E-Mail = "contact@omore-audio.local"`, `Flags` (`Unicode = true`, `Classes Discardable = false`, `Component Non Discardable = false`)
  - `Classes`: entrée unique avec `CID = "4F6D6F726550756E6368427573563031"`, `Category = "Audio Module Class"`, `Name = "Omore Punch"`, `Vendor = "Omore"`, `Version = <Cargo.toml>`, `SDKVersion = "VST 3.6.14"`, `Sub Categories = ["Fx", "Dynamics", "Distortion", "Stereo"]`, `Class Flags = 2`, `Cardinality = 2147483647`.
- **Rationale**: Les gestionnaires de plugins modernes (dont Plugoff sous Windows) et les DAWs lisent `Contents/Resources/moduleinfo.json` pour afficher la version, le fabricant et la catégorie sans avoir à charger la bibliothèque dynamique du plugin en mémoire. `nih_plug_xtask` ne génère pas ce fichier nativement (il ne génère que `Info.plist`/`PkgInfo` sur macOS).
- **Alternatives considered**:
  - Compter uniquement sur `IPluginFactory2` / `IPluginFactory3` exposés par `nih-plug` : rejeté car déjà en place dans `src/lib.rs`, mais ignoré par les scanners statiques qui ne chargent pas le binaire `.vst3`.
  - Maintenir un fichier `moduleinfo.json` statique versionné dans Git : rejeté car cela dupliquerait le numéro de version et violerait le Principe VI de la constitution (source unique de version dans `Cargo.toml`).

---

### 2. Intégration des ressources PE `VERSIONINFO` sous Windows via `build.rs` (`winres`)

- **Decision**: Ajouter un script `build.rs` (actif uniquement lorsque `CARGO_CFG_TARGET_OS == "windows"`) utilisant la crate `winres` dans `[target.'cfg(windows)'.build-dependencies]` de `Cargo.toml` pour embarquer une ressource Win32 `VS_VERSION_INFO` dans la DLL (`.vst3` et `.clap`) avec :
  - `CompanyName = "Omore"`
  - `ProductName = "Omore Punch"`
  - `FileDescription = "Omore Punch - Stereo Drum Bus Compressor & Clipper"`
  - `ProductVersion` / `FileVersion` = `CARGO_PKG_VERSION`
  - Version numérique à 4 composantes basée sur `CARGO_PKG_VERSION_MAJOR`, `CARGO_PKG_VERSION_MINOR`, `CARGO_PKG_VERSION_PATCH` et `0`.
- **Rationale**: Sous Windows, les explorateurs de fichiers et gestionnaires de plugins (comme Plugoff) lisent aussi les propriétés PE `VERSIONINFO` embarquées dans le binaire `.vst3` pour afficher la version et l'éditeur. Construire la version binaire à 4 entiers à partir de `CARGO_PKG_VERSION_MAJOR/MINOR/PATCH` garantit que les versions de développement contenant un suffixe (ex. `0.3.1-dev`) compilent sans erreur avec le compilateur de ressources Windows tout en conservant la chaîne complète dans les champs texte `ProductVersion` et `FileVersion`.
- **Alternatives considered**:
  - Fournir uniquement `moduleinfo.json` sans ressource `VERSIONINFO` : rejeté car les propriétés de fichier Windows resteraient vides et l'affichage pourrait rester incomplet selon l'ordre de priorité du scanner Windows.

---

### 3. Renommage des bundles en `Omore Punch.vst3` et `Omore Punch.clap` via `bundler.toml`

- **Decision**: Créer le fichier `bundler.toml` à la racine du workspace avec :
  ```toml
  [omore-punch]
  name = "Omore Punch"
  ```
- **Rationale**: `nih_plug_xtask` supporte nativement `bundler.toml` (`load_bundler_config()`) pour définir le nom des bundles générés dans `target/bundled/` (`Omore Punch.vst3` et `Omore Punch.clap` sous Linux et Windows) tout en conservant le nom de package Cargo `omore-punch` (qui alimente `cargo run --package xtask --release -- bundle omore-punch --release`). De plus, `nih_plug_xtask` ajuste automatiquement le nom de la bibliothèque interne à l'intérieur du bundle VST3 (`Omore Punch.vst3/Contents/<arch>/Omore Punch.so` ou `Omore Punch.vst3`), comme exigé par la spécification VST3.
- **Alternatives considered**:
  - Renommer le package dans `Cargo.toml` : rejeté car un nom de crate Cargo ne peut pas contenir d'espace.
  - Renommer manuellement les fichiers dans `build.sh` / `build.ps1` après l'appel à `xtask` : rejeté car le binaire interne dans `Contents/<arch>/` ne correspondrait plus au nom du répertoire `.vst3`, ce qui rompt la conformité du bundle VST3 sous Linux et Windows.

---

### 4. Alignement du fabricant sur `Omore` et préservation de la rétrocompatibilité DAW

- **Decision**: Mettre à jour `OmorePunch::VENDOR` dans `src/lib.rs` (et `authors` dans `Cargo.toml`) de `"Omore Audio"` vers `"Omore"`, tout en conservant strictement inchangés :
  - `VST3_CLASS_ID = *b"OmorePunchBusV01"`
  - `CLAP_ID = "local.omore-audio.omore-punch"`
  - tous les attributs `#[id = "..."]` de `OmorePunchParams`
- **Rationale**: Aligne le champ Manufacturer avec *Omore Sampler* dans Plugoff (`Omore`), conformément à la décision issue de `/speckit-clarify`. Les hôtes VST3 et CLAP (dont Reaper) identifient les plugins dans les projets sauvegardés par leur `VST3_CLASS_ID` (16 octets) et leur `CLAP_ID`, et non par la chaîne `VENDOR` ni par le nom de fichier du bundle.
- **Alternatives considered**:
  - Modifier également `CLAP_ID` (`local.omore-audio.omore-punch`) ou `VST3_CLASS_ID` : rejeté car cela casserait le chargement des projets DAW existants, en violation directe de FR-005 et SC-002.

---

### 5. Nettoyage des anciens bundles `omore-punch.*` dans les scripts d'installation locale et CI

- **Decision**:
  - Mettre à jour `build.sh` (Linux) et `build.ps1` (Windows) pour supprimer à la fois les anciens bundles (`omore-punch.vst3`, `omore-punch.clap`) et les nouveaux (`Omore Punch.vst3`, `Omore Punch.clap`) dans les dossiers cibles avant de copier les nouveaux artefacts générés dans `target/bundled/`.
  - Mettre à jour `.github/workflows/build.yml` pour uploader `target/bundled/Omore Punch.vst3` et `target/bundled/Omore Punch.clap`.
- **Rationale**: Empêche la coexistence de l'ancien et du nouveau bundle dans le même répertoire système VST3/CLAP avec les mêmes identifiants internes (`VST3_CLASS_ID` / `CLAP_ID`), ce qui provoquerait des entrées en double ou des conflits de scan dans Reaper et Plugoff.
- **Alternatives considered**:
  - Laisser l'utilisateur supprimer manuellement les anciens fichiers `omore-punch.*` : rejeté car source d'erreurs lors des mises à jour locales et contraire à FR-006.
