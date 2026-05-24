//! Surface the codex binary's version to the API documentation generator.
//!
//! The OpenAPI spec embeds a `version` string at compile time via the
//! `utoipa::OpenApi` derive. Inside the `codex-api` crate, `env!("CARGO_PKG_VERSION")`
//! resolves to this crate's own `0.0.0` workspace-internal placeholder, which
//! is not the user-visible version. Read the root `Cargo.toml` once here and
//! re-emit it as a build-time env var the derive can pick up.

use std::path::PathBuf;

fn main() {
    // Root manifest is two levels up from this crate's manifest dir.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root_manifest = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("Cargo.toml"))
        .expect("codex-api should live under <workspace>/crates/codex-api");

    println!("cargo:rerun-if-changed={}", root_manifest.display());

    let contents = std::fs::read_to_string(&root_manifest)
        .unwrap_or_else(|e| panic!("read {}: {e}", root_manifest.display()));

    let version = contents
        .lines()
        .skip_while(|l| l.trim() != "[package]")
        .find_map(|l| l.trim().strip_prefix("version = "))
        .and_then(|v| v.trim().strip_prefix('"'))
        .and_then(|v| v.strip_suffix('"'))
        .expect("root Cargo.toml must have a `version = \"...\"` line in [package]");

    println!("cargo:rustc-env=CODEX_BIN_VERSION={version}");
}
