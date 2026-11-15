use std::fs;
use std::path::Path;
use std::process::Command;

fn emit_rerun_if_exists(path: &str) {
    if Path::new(path).exists() {
        println!("cargo:rerun-if-changed={}", path);
    }
}

fn capture_git_tag() -> String {
    let Ok(rev) = Command::new("git")
        .args(["rev-parse", "--short=7", "HEAD"])
        .output()
    else {
        return String::new();
    };

    if !rev.status.success() {
        return String::new();
    }

    let hash = String::from_utf8_lossy(&rev.stdout).trim().to_string();
    if hash.is_empty() || !hash.is_ascii() {
        return String::new();
    }

    let is_dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .map(|out| out.status.success() && !out.stdout.is_empty())
        .unwrap_or(false);

    if is_dirty {
        format!("{}-dirty", hash)
    } else {
        hash
    }
}

#[cfg(windows)]
fn embed_windows_version_info() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let version = env!("CARGO_PKG_VERSION");
    let mut res = winresource::WindowsResource::new();
    // VFT_DLL = 0x02 (plugins VST3 and CLAP are dynamic libraries)
    res.set_version_info(winresource::VersionInfo::FILETYPE, 2);
    res.set("CompanyName", "Omore");
    res.set("ProductName", "Omore Punch");
    res.set(
        "FileDescription",
        "Omore Punch - Stereo Drum Bus Compressor & Clipper",
    );
    res.set("FileVersion", version);
    res.set("ProductVersion", version);

    let file_flags = if version.contains('-') {
        winresource::VersionInfo::VS_FF_PRERELEASE
    } else {
        0
    };
    res.set_version_info(winresource::VersionInfo::FILEFLAGS, file_flags);
    res.compile()
        .expect("Failed to compile Windows VS_VERSION_INFO resource");
}

#[cfg(not(windows))]
fn embed_windows_version_info() {}

fn main() {
    for watched in ["src/lib.rs", "Cargo.toml", "build.rs", ".git/HEAD", ".git/index"] {
        emit_rerun_if_exists(watched);
    }

    if let Ok(head) = fs::read_to_string(".git/HEAD") {
        if let Some(rel_ref) = head.trim().strip_prefix("ref: ") {
            let ref_path = format!(".git/{}", rel_ref);
            emit_rerun_if_exists(&ref_path);
        }
    }

    let git_tag = capture_git_tag();
    println!("cargo:rustc-env=OMORE_GIT_TAG={}", git_tag);

    embed_windows_version_info();
}
