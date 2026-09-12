use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("Rust slice must live at <repo>/src/rust")
        .to_path_buf()
}

#[test]
fn retired_native_lock_trigger_marker_stays_absent() {
    let marker = repository_root().join(".native-lock-materialize-trigger");
    assert!(
        !marker.exists(),
        "retired one-shot native-lock trigger marker must not return: {}",
        marker.display()
    );
}
