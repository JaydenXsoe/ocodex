use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    // Walk up to find the repo root. Favor a folder literally named
    // "ocodex" or an ancestor that contains a ".codex" directory.
    let mut repo_root: Option<PathBuf> = None;
    for ancestor in manifest_dir.ancestors() {
        let name_matches = ancestor
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s == "ocodex")
            .unwrap_or(false);
        if name_matches || ancestor.join(".codex").is_dir() {
            repo_root = Some(ancestor.to_path_buf());
            break;
        }
    }

    // Fallback: go up two levels (../../) from the crate dir – typically
    // `<repo>/codex-rs/arg0` -> `<repo>`.
    let repo_root = repo_root.unwrap_or_else(|| {
        manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .unwrap_or(&manifest_dir)
            .to_path_buf()
    });

    println!(
        "cargo:rustc-env=OCODEX_DEFAULT_REPO_ROOT={}",
        repo_root.display()
    );
}
