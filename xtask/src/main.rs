use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const PLUGIN_PACKAGE: &str = "omore-punch";
const PLUGIN_NAME: &str = "Omore Punch";
const PLUGIN_VENDOR: &str = "Omore";
const PLUGIN_URL: &str = "https://omore-audio.local";
const PLUGIN_EMAIL: &str = "contact@omore-audio.local";
const PLUGIN_CID_HEX: &str = "4F6D6F726550756E6368427573563031";
const PLUGIN_SDK_VERSION: &str = "VST 3.6.14";

#[derive(Debug, Deserialize)]
struct CargoManifest {
    package: CargoPackage,
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    name: String,
    version: String,
}

#[derive(Debug, Deserialize)]
struct BundlerPackageConfig {
    name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct ModuleInfo {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Version")]
    version: String,
    #[serde(rename = "Factory Info")]
    factory_info: FactoryInfo,
    #[serde(rename = "Classes")]
    classes: Vec<ModuleClass>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct FactoryInfo {
    #[serde(rename = "Vendor")]
    vendor: String,
    #[serde(rename = "URL")]
    url: String,
    #[serde(rename = "E-Mail")]
    email: String,
    #[serde(rename = "Flags")]
    flags: FactoryFlags,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct FactoryFlags {
    #[serde(rename = "Unicode")]
    unicode: bool,
    #[serde(rename = "Classes Discardable")]
    classes_discardable: bool,
    #[serde(rename = "Component Non Discardable")]
    component_non_discardable: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct ModuleClass {
    #[serde(rename = "CID")]
    cid: String,
    #[serde(rename = "Category")]
    category: String,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Vendor")]
    vendor: String,
    #[serde(rename = "Version")]
    version: String,
    #[serde(rename = "SDKVersion")]
    sdk_version: String,
    #[serde(rename = "Sub Categories")]
    sub_categories: Vec<String>,
    #[serde(rename = "Class Flags")]
    class_flags: u32,
    #[serde(rename = "Cardinality")]
    cardinality: u32,
    #[serde(rename = "Snapshots")]
    snapshots: Vec<String>,
}

fn build_module_info(version: &str) -> ModuleInfo {
    ModuleInfo {
        name: PLUGIN_NAME.to_string(),
        version: version.to_string(),
        factory_info: FactoryInfo {
            vendor: PLUGIN_VENDOR.to_string(),
            url: PLUGIN_URL.to_string(),
            email: PLUGIN_EMAIL.to_string(),
            flags: FactoryFlags {
                unicode: true,
                classes_discardable: false,
                component_non_discardable: false,
            },
        },
        classes: vec![ModuleClass {
            cid: PLUGIN_CID_HEX.to_string(),
            category: "Audio Module Class".to_string(),
            name: PLUGIN_NAME.to_string(),
            vendor: PLUGIN_VENDOR.to_string(),
            version: version.to_string(),
            sdk_version: PLUGIN_SDK_VERSION.to_string(),
            sub_categories: vec![
                "Fx".to_string(),
                "Dynamics".to_string(),
                "Distortion".to_string(),
                "Stereo".to_string(),
            ],
            class_flags: 2,
            cardinality: 0x7FFF_FFFF,
            snapshots: Vec::new(),
        }],
    }
}

fn parse_package_version(cargo_toml: &str) -> nih_plug_xtask::Result<(String, String)> {
    let manifest: CargoManifest = toml::from_str(cargo_toml)?;
    Ok((manifest.package.name, manifest.package.version))
}

fn resolve_bundle_name(
    package_name: &str,
    bundler_toml_path: &Path,
) -> nih_plug_xtask::Result<String> {
    if !bundler_toml_path.exists() {
        return Ok(package_name.to_string());
    }
    let raw = fs::read_to_string(bundler_toml_path)?;
    let cfg: HashMap<String, BundlerPackageConfig> = toml::from_str(&raw)?;
    Ok(cfg
        .get(package_name)
        .and_then(|p| p.name.clone())
        .unwrap_or_else(|| package_name.to_string()))
}

fn target_dir_from_env_or_default(workspace_root: &Path) -> PathBuf {
    match std::env::var_os("CARGO_TARGET_DIR") {
        Some(dir) if !dir.is_empty() => {
            let p = PathBuf::from(dir);
            if p.is_absolute() {
                p
            } else {
                workspace_root.join(p)
            }
        }
        _ => workspace_root.join("target"),
    }
}

fn write_vst3_moduleinfo(workspace_root: &Path) -> nih_plug_xtask::Result<()> {
    let cargo_toml_path = workspace_root.join("Cargo.toml");
    let cargo_raw = fs::read_to_string(&cargo_toml_path)?;
    let (package_name, version) = parse_package_version(&cargo_raw)?;
    if package_name != PLUGIN_PACKAGE {
        return Ok(());
    }

    let bundle_name = resolve_bundle_name(&package_name, &workspace_root.join("bundler.toml"))?;
    let vst3_bundle_dir = target_dir_from_env_or_default(workspace_root)
        .join("bundled")
        .join(format!("{bundle_name}.vst3"));

    if !vst3_bundle_dir.exists() {
        return Ok(());
    }

    let resources_dir = vst3_bundle_dir.join("Contents").join("Resources");
    fs::create_dir_all(&resources_dir)?;

    let moduleinfo_path = resources_dir.join("moduleinfo.json");
    let mut json = serde_json::to_string_pretty(&build_module_info(&version))?;
    json.push('\n');
    fs::write(&moduleinfo_path, json)?;

    eprintln!(
        "Generated VST3 moduleinfo at '{}'",
        moduleinfo_path.display()
    );
    Ok(())
}

fn main() -> nih_plug_xtask::Result<()> {
    let first_arg = std::env::args().nth(1);
    nih_plug_xtask::main()?;

    if matches!(first_arg.as_deref(), Some("bundle" | "bundle-universal")) {
        // nih_plug_xtask::main() has already chdir'd to the workspace root
        let workspace_root = std::env::current_dir()
            .map_err(|e| io::Error::new(e.kind(), format!("Could not resolve workspace root: {e}")))?;
        write_vst3_moduleinfo(&workspace_root)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_moduleinfo_json_matches_contract_for_dev_version() {
        let info = build_module_info("0.3.1-dev");
        let serialized = serde_json::to_string_pretty(&info).expect("serialize moduleinfo");
        let val: serde_json::Value =
            serde_json::from_str(&serialized).expect("parse serialized json");

        assert_eq!(val["Name"], "Omore Punch");
        assert_eq!(val["Version"], "0.3.1-dev");
        assert_eq!(val["Factory Info"]["Vendor"], "Omore");
        assert_eq!(val["Factory Info"]["URL"], "https://omore-audio.local");
        assert_eq!(val["Factory Info"]["E-Mail"], "contact@omore-audio.local");
        assert_eq!(val["Factory Info"]["Flags"]["Unicode"], true);

        let classes = val["Classes"].as_array().expect("Classes array");
        assert_eq!(classes.len(), 1);
        let cls = &classes[0];
        assert_eq!(cls["CID"], "4F6D6F726550756E6368427573563031");
        assert_eq!(cls["Category"], "Audio Module Class");
        assert_eq!(cls["Name"], "Omore Punch");
        assert_eq!(cls["Vendor"], "Omore");
        assert_eq!(cls["Version"], "0.3.1-dev");
        assert_eq!(cls["SDKVersion"], "VST 3.6.14");
        assert_eq!(
            cls["Sub Categories"],
            serde_json::json!(["Fx", "Dynamics", "Distortion", "Stereo"])
        );
        assert_eq!(cls["Class Flags"], 2);
        assert_eq!(cls["Cardinality"], 2147483647);
    }

    #[test]
    fn test_resolve_bundle_name_with_and_without_bundler_toml() {
        let tmp = std::env::temp_dir().join("omore_punch_xtask_test_bundler.toml");
        let _ = fs::remove_file(&tmp);
        assert_eq!(
            resolve_bundle_name("omore-punch", &tmp).unwrap(),
            "omore-punch"
        );

        fs::write(&tmp, "[omore-punch]\nname = \"Omore Punch\"\n").unwrap();
        assert_eq!(
            resolve_bundle_name("omore-punch", &tmp).unwrap(),
            "Omore Punch"
        );
        let _ = fs::remove_file(&tmp);
    }
}
