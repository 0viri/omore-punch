# Quickstart Validation Guide (`005-plugin-bundle-metadata`)

**Date**: 2026-11-14
**Feature**: [spec.md](file:///home/tdavio/src/omore-punch/specs/005-plugin-bundle-metadata/spec.md)
**Contracts**: [bundle-metadata-contract.md](file:///home/tdavio/src/omore-punch/specs/005-plugin-bundle-metadata/contracts/bundle-metadata-contract.md)

## Prérequis

- Toolchain Rust stable configurée pour la plateforme cible (Linux x86_64 ou Windows 11 x86_64)
- Sur Windows : outil de compilation de ressources Windows disponible dans la toolchain (MSVC `rc.exe` ou MinGW `windres`)
- Pour la validation end-to-end hôte/gestionnaire : Reaper (Linux/Windows) et Plugoff (Windows)

---

## 1. Contrôles Qualité Automatisés (Linux & Windows)

### Contrôle ASCII strict sur `src/lib.rs`

```bash
LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs
```

**Résultat attendu** : aucune sortie (code de retour `1` de `grep`, signifiant 0 caractère non-ASCII).

### Tests unitaires DSP et métadonnées

```bash
cargo test --release
```

**Résultat attendu** : 100 % des tests DSP existants et des assertions d'identité/métadonnées passent sans échec ni warning bloquant.

### Bundling VST3 & CLAP

```bash
cargo run --package xtask --release -- bundle omore-punch --release
```

**Résultat attendu** :
- `target/bundled/Omore Punch.vst3` et `target/bundled/Omore Punch.clap` sont créés.
- Aucun ancien fichier `omore-punch.vst3` ou `omore-punch.clap` n'est utilisé comme sortie finale.

---

## 2. Inspection des Artefacts de Bundle

### Vérifier l'arborescence et le manifeste VST3 (Linux / Git Bash)

```bash
ls -la target/bundled/
cat "target/bundled/Omore Punch.vst3/Contents/Resources/moduleinfo.json"
```

**Résultat attendu** :
- `Omore Punch.vst3/Contents/Resources/moduleinfo.json` existe et est un JSON valide conforme au [contrat de métadonnées](file:///home/tdavio/src/omore-punch/specs/005-plugin-bundle-metadata/contracts/bundle-metadata-contract.md).
- `Name` = `"Omore Punch"`, `Vendor` = `"Omore"`, `Version` = version courante de `Cargo.toml`, `Sub Categories` = `["Fx", "Dynamics", "Distortion", "Stereo"]`, `CID` = `"4F6D6F726550756E6368427573563031"`.

---

## 3. Validation Locale & Gestionnaire de Plugins (Plugoff / Reaper)

### Installation locale et nettoyage des anciens bundles

- **Sous Linux** :
  ```bash
  ./build.sh
  ```
  Vérifier que `~/.vst3/omore-punch.vst3` et `~/.clap/omore-punch.clap` ont été supprimés et remplacés par `~/.vst3/Omore Punch.vst3` et `~/.clap/Omore Punch.clap`.

- **Sous Windows (PowerShell)** :
  ```powershell
  .\build.ps1
  ```
  Vérifier que les anciens dossiers/fichiers `omore-punch.vst3` / `omore-punch.clap` ont été retirés du répertoire d'installation cible et que `Omore Punch.vst3` et `Omore Punch.clap` y sont présents.

### Vérification visuelle dans Plugoff (Windows)

1. Ouvrir **Plugoff** et cliquer sur **Scan**.
2. Filtrer sur `omore`.
3. **Résultat attendu** :
   - **PLUGIN** : `Omore Punch`
   - **VERSION** : version courante (ex. `0.3.1-dev`)
   - **MANUFACTURER** : `Omore`
   - **CATEGORY** : renseignée (ex. `Dynamics` / `Fx|Dynamics`, plus de `Not Supplied`)
   - Aucune entrée en double `omore-punch`.

### Vérification de rétrocompatibilité dans Reaper (Linux & Windows)

1. Charger un projet Reaper existant créé avant le renommage (utilisant l'ancien `omore-punch`).
2. Vérifier que Reaper charge le plugin sans erreur de plugin manquant et restaure exactement tous les paramètres enregistrés et le rendu audio.
