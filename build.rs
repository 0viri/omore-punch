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
}
