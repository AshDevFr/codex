//! Surface workspace-root paths/values to the API crate at compile time.
//!
//! Two things get hoisted out of this build script:
//!
//! - `CODEX_BIN_VERSION`: the OpenAPI spec embeds a `version` string at compile
//!   time via the `utoipa::OpenApi` derive. Inside the `codex-api` crate,
//!   `env!("CARGO_PKG_VERSION")` resolves to this crate's own placeholder,
//!   which is not the user-visible version. Read the root `Cargo.toml` once
//!   here and re-emit it as a build-time env var the derive can pick up.
//! - `CODEX_WEB_DIST`: rust-embed's `#[folder = ...]` resolves relative to the
//!   consuming crate's `CARGO_MANIFEST_DIR`. The frontend's `web/dist` lives at
//!   the workspace root, not under `crates/codex-api/`, so emit the absolute
//!   path here for `src/web.rs` to consume.

use std::path::PathBuf;

fn main() {
    // Workspace root is two levels up from this crate's manifest dir.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("codex-api should live under <workspace>/crates/codex-api")
        .to_path_buf();

    let root_manifest = workspace_root.join("Cargo.toml");
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
    println!(
        "cargo:rustc-env=CODEX_WEB_DIST={}",
        workspace_root.join("web").join("dist").display()
    );
}
