use std::fs;
use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("Rust slice must live at <repo>/src/rust")
        .to_path_buf()
}

fn read(relative: &str) -> String {
    let path = repository_root().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

#[test]
fn rust_toolchain_is_patch_exact() {
    let toolchain: toml::Value =
        toml::from_str(&read("rust-toolchain.toml")).expect("parse rust-toolchain.toml");
    let channel = toolchain["toolchain"]["channel"]
        .as_str()
        .expect("toolchain.channel must be a string");
    let parts = channel.split('.').collect::<Vec<_>>();
    assert_eq!(parts.len(), 3, "toolchain must be major.minor.patch exact");
    assert!(
        parts.iter().all(|part| part.parse::<u64>().is_ok()),
        "toolchain must contain only numeric major.minor.patch components: {channel}"
    );
    assert!(!matches!(channel, "stable" | "beta" | "nightly"));
}

#[test]
fn environment_format_admission_is_read_only_and_exact_head() {
    let workflow = read(".github/workflows/autofix-environment-source.yml");
    assert!(workflow.contains("contents: read"));
    assert!(!workflow.contains("contents: write"));
    assert!(workflow.contains("github.event.pull_request.head.sha"));
    assert!(workflow.contains("persist-credentials: false"));
    assert!(workflow.contains("cargo fmt --all -- --check"));
    for forbidden in ["git commit", "git push", "rustup toolchain install stable"] {
        assert!(
            !workflow.contains(forbidden),
            "read-only formatting admission contains forbidden `{forbidden}`"
        );
    }
}

#[test]
fn native_lock_materialization_uses_committed_toolchain_and_exact_head() {
    let workflow = read(".github/workflows/materialize-native-lock.yml");
    assert!(workflow.contains("github.event.pull_request.head.sha || github.sha"));
    assert!(workflow.contains("rust-toolchain.toml"));
    assert!(workflow.contains("persist-credentials: false"));
    for forbidden in [
        "rustup toolchain install stable",
        "rustup default stable",
        "toolchain: stable",
    ] {
        assert!(
            !workflow.contains(forbidden),
            "native-lock workflow contains moving toolchain `{forbidden}`"
        );
    }
}
