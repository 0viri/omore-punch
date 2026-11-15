# Data Model: Métadonnées et Nommage du Bundle Plugin (`005-plugin-bundle-metadata`)

**Date**: 2026-11-14
**Feature**: [spec.md](file:///home/tdavio/src/omore-punch/specs/005-plugin-bundle-metadata/spec.md)

## Entités et Modèles de Métadonnées

### 1. `BundleConfiguration` (`bundler.toml`)

Configuration du nom de distribution consommée par `nih_plug_xtask` à la racine du workspace.

| Champ | Type | Valeur / Règle | Validation |
|-------|------|----------------|------------|
| `[omore-punch].name` | String | `"Omore Punch"` | Doit produire `Omore Punch.vst3` et `Omore Punch.clap` dans `target/bundled/` sur Linux et Windows |

---

### 2. `Vst3ModuleInfo` (`Contents/Resources/moduleinfo.json`)

Manifeste JSON standard Steinberg VST 3.7.5+ généré automatiquement dans `target/bundled/Omore Punch.vst3/Contents/Resources/moduleinfo.json`.

#### Structure racine

| Champ JSON | Type | Valeur / Source | Règle |
|------------|------|-----------------|-------|
| `Name` | String | `"Omore Punch"` | Identique au nom du bundle et à `OmorePunch::NAME` |
| `Version` | String | `Cargo.toml` (`package.version`) | Lecture dynamique (Principe VI, ex. `"0.3.1-dev"` ou `"0.3.1"`) |
| `Factory Info` | Object | Voir sous-structure ci-dessous | Obligatoire |
| `Classes` | Array[Object] | Tableau à 1 élément (`Audio Module Class`) | Obligatoire |

#### Sous-structure `Factory Info`

| Champ JSON | Type | Valeur |
|------------|------|--------|
| `Vendor` | String | `"Omore"` |
| `URL` | String | `"https://omore-audio.local"` |
| `E-Mail` | String | `"contact@omore-audio.local"` |
| `Flags.Unicode` | Boolean | `true` |
| `Flags.Classes Discardable` | Boolean | `false` |
| `Flags.Component Non Discardable` | Boolean | `false` |

#### Élément de `Classes`

| Champ JSON | Type | Valeur |
|------------|------|--------|
| `CID` | String (32 hex) | `"4F6D6F726550756E6368427573563031"` (encodage hex de `*b"OmorePunchBusV01"`) |
| `Category` | String | `"Audio Module Class"` |
| `Name` | String | `"Omore Punch"` |
| `Vendor` | String | `"Omore"` |
| `Version` | String | `Cargo.toml` (`package.version`) |
| `SDKVersion` | String | `"VST 3.6.14"` |
| `Sub Categories` | Array[String] | `["Fx", "Dynamics", "Distortion", "Stereo"]` |
| `Class Flags` | Integer | `2` (`kSimpleModeSupported`) |
| `Cardinality` | Integer | `2147483647` (`kManyInstances`) |
| `Snapshots` | Array | `[]` |

---

### 3. `WindowsVersionResource` (`VS_VERSION_INFO` PE, Windows uniquement)

Métadonnées Win32 compilées dans le binaire Windows (`.vst3` et `.clap`) via `build.rs` et `winres`.

| Champ | Source | Format / Contrainte |
|-------|--------|---------------------|
| `FileVersion` (numérique) | `CARGO_PKG_VERSION_MAJOR/MINOR/PATCH` | `(MAJOR, MINOR, PATCH, 0)` — 4 entiers `u16` valides même si `CARGO_PKG_VERSION` contient `-dev` |
| `ProductVersion` (numérique) | `CARGO_PKG_VERSION_MAJOR/MINOR/PATCH` | `(MAJOR, MINOR, PATCH, 0)` |
| `FileVersion` (chaîne) | `CARGO_PKG_VERSION` | Chaîne complète (ex. `"0.3.1-dev"`) |
| `ProductVersion` (chaîne) | `CARGO_PKG_VERSION` | Chaîne complète (ex. `"0.3.1-dev"`) |
| `CompanyName` | Constante | `"Omore"` |
| `ProductName` | Constante | `"Omore Punch"` |
| `FileDescription` | Constante | `"Omore Punch - Stereo Drum Bus Compressor & Clipper"` |

---

### 4. `PluginInternalIdentity` (`src/lib.rs`)

Identité exposée à l'hôte via `nih-plug` (`Plugin`, `ClapPlugin`, `Vst3Plugin`) et invariants de compatibilité DAW.

| Symbole | Valeur | Statut |
|---------|--------|--------|
| `OmorePunch::NAME` | `"Omore Punch"` | Inchangé |
| `OmorePunch::VENDOR` | `"Omore"` | Modifié (aligné avec *Omore Sampler*, était `"Omore Audio"`) |
| `OmorePunch::VERSION` | `env!("CARGO_PKG_VERSION")` | Inchangé (Principe VI) |
| `OmorePunch::VST3_CLASS_ID` | `*b"OmorePunchBusV01"` | **Invariant strict** (FR-005) |
| `OmorePunch::CLAP_ID` | `"local.omore-audio.omore-punch"` | **Invariant strict** (FR-005) |
| `OmorePunch::VST3_SUBCATEGORIES` | `&[Fx, Dynamics, Distortion, Stereo]` | Inchangé |
| Paramètres `#[id = "..."]` | Identifiants existants | **Invariants stricts** (FR-005) |

## Relations et Cohérence

```text
Cargo.toml ([package].version)
   ├──► src/lib.rs (OmorePunch::VERSION)
   ├──► build.rs (Windows VS_VERSION_INFO)
   └──► xtask/src/main.rs (Contents/Resources/moduleinfo.json)

bundler.toml ([omore-punch].name = "Omore Punch")
   └──► xtask (target/bundled/Omore Punch.vst3 & target/bundled/Omore Punch.clap)
           └──► build.sh / build.ps1 (nettoyage omore-punch.* + installation Omore Punch.*)
```
