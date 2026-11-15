# Contrat de Métadonnées et Structure de Bundle (`005-plugin-bundle-metadata`)

**Date**: 2026-11-14
**Spec**: [spec.md](file:///home/tdavio/src/omore-punch/specs/005-plugin-bundle-metadata/spec.md)

## 1. Arborescence des Artefacts dans `target/bundled/`

### Linux x86_64

```text
target/bundled/
├── Omore Punch.clap
└── Omore Punch.vst3/
    └── Contents/
        ├── Resources/
        │   └── moduleinfo.json
        └── x86_64-linux/
            └── Omore Punch.so
```

### Windows x86_64

```text
target/bundled/
├── Omore Punch.clap                  # PE DLL avec ressource VS_VERSION_INFO
└── Omore Punch.vst3/
    └── Contents/
        ├── Resources/
        │   └── moduleinfo.json
        └── x86_64-win/
            └── Omore Punch.vst3      # PE DLL avec ressource VS_VERSION_INFO
```

Les anciens noms `omore-punch.vst3` et `omore-punch.clap` ne doivent plus être générés dans `target/bundled/`, et les scripts d'installation locale (`build.sh` et `build.ps1`) doivent supprimer ces anciens chemins s'ils existent dans les dossiers utilisateur/système avant d'installer `Omore Punch.vst3` et `Omore Punch.clap`.

---

## 2. Contrat JSON : `Contents/Resources/moduleinfo.json`

Exemple complet généré pour la version courante `0.3.1-dev` :

```json
{
  "Name": "Omore Punch",
  "Version": "0.3.1-dev",
  "Factory Info": {
    "Vendor": "Omore",
    "URL": "https://omore-audio.local",
    "E-Mail": "contact@omore-audio.local",
    "Flags": {
      "Unicode": true,
      "Classes Discardable": false,
      "Component Non Discardable": false
    }
  },
  "Classes": [
    {
      "CID": "4F6D6F726550756E6368427573563031",
      "Category": "Audio Module Class",
      "Name": "Omore Punch",
      "Vendor": "Omore",
      "Version": "0.3.1-dev",
      "SDKVersion": "VST 3.6.14",
      "Sub Categories": [
        "Fx",
        "Dynamics",
        "Distortion",
        "Stereo"
      ],
      "Class Flags": 2,
      "Cardinality": 2147483647,
      "Snapshots": []
    }
  ]
}
```

### Invariants du contrat `moduleinfo.json`

- `Version` et `Classes[0].Version` doivent toujours refléter exactement `[package].version` dans `Cargo.toml`.
- `Factory Info.Vendor` et `Classes[0].Vendor` doivent être `"Omore"`.
- `Name` et `Classes[0].Name` doivent être `"Omore Punch"`.
- `Classes[0].CID` doit être `"4F6D6F726550756E6368427573563031"` (correspondant à `VST3_CLASS_ID = *b"OmorePunchBusV01"`).
- `Classes[0].Sub Categories` doit contenir `["Fx", "Dynamics", "Distortion", "Stereo"]`.

---

## 3. Contrat PE `VS_VERSION_INFO` (Windows uniquement)

Chaque binaire compilé pour Windows (`Omore Punch.clap` et `Omore Punch.vst3/Contents/x86_64-win/Omore Punch.vst3`) doit contenir une ressource `VERSIONINFO` avec :

| Propriété PE | Valeur attendue |
|--------------|-----------------|
| `FileVersion` (numérique) | `<MAJOR>.<MINOR>.<PATCH>.0` |
| `ProductVersion` (numérique) | `<MAJOR>.<MINOR>.<PATCH>.0` |
| `FileVersion` (string) | `CARGO_PKG_VERSION` (ex. `"0.3.1-dev"`) |
| `ProductVersion` (string) | `CARGO_PKG_VERSION` (ex. `"0.3.1-dev"`) |
| `CompanyName` | `"Omore"` |
| `ProductName` | `"Omore Punch"` |
| `FileDescription` | `"Omore Punch - Stereo Drum Bus Compressor & Clipper"` |

---

## 4. Contrat de Rétrocompatibilité DAW (Invariants)

Afin de garantir qu'aucun projet DAW existant ne soit cassé lors du passage de `omore-punch` à `Omore Punch` :

- `OmorePunch::VST3_CLASS_ID` MUST remain `*b"OmorePunchBusV01"`.
- `OmorePunch::CLAP_ID` MUST remain `"local.omore-audio.omore-punch"`.
- Tous les identifiants `#[id = "..."]` des paramètres de `OmorePunchParams` MUST remain unchanged.
