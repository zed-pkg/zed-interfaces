//! This repository's own `.zpkg.toml` must satisfy the rules this crate
//! defines. zed-interfaces is where the polyglot manifest model lives, so a
//! manifest here that its own parser rejects is the worst possible bug — and
//! the one nobody would notice, because nothing else in CI parses it.

use std::path::Path;

use zed_interfaces::manifest::Manifest;

fn own_manifest() -> Manifest {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.zpkg.toml");
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    Manifest::parse(&raw).expect("the repository manifest parses and validates")
}

#[test]
fn the_repository_manifest_is_valid() {
    let manifest = own_manifest();
    assert_eq!(manifest.package.name, "zed-interfaces");
    assert!(
        manifest.is_polyglot(),
        "the language slices are what make this package polyglot"
    );
}

#[test]
fn every_language_slice_is_its_own_target_with_an_isolated_root() {
    let manifest = own_manifest();
    for (target, dir, adapter) in [
        ("rust", "src/rust", "rust"),
        ("dart", "src/dart", "dart"),
        ("typescript", "src/ts", "node"),
    ] {
        let section = manifest
            .targets
            .get(target)
            .unwrap_or_else(|| panic!("missing `[targets.{target}]`"));
        assert_eq!(section.dir, dir, "target `{target}` moved");
        assert_eq!(section.adapter.as_deref(), Some(adapter));
        assert!(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(dir)
                .is_dir(),
            "target `{target}` points at `{dir}`, which does not exist"
        );
    }
}

#[test]
fn the_rust_slice_is_the_crate_that_publishes_to_crates_io() {
    let manifest = own_manifest();
    let rust = manifest.targets.get("rust").expect("rust target");
    let native = rust
        .native
        .as_ref()
        .expect("the Rust slice keeps the crates.io route");
    assert_eq!(native.package, "zed-interfaces");
    // A target that owned `dir = "."` could not carry this route at all, which
    // is why the crate manifest lives in `src/rust/` rather than at the root.
    assert_ne!(rust.dir, ".");
    assert!(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("Cargo.toml")
            .is_file(),
        "the crate manifest must sit inside the Rust slice"
    );
}

#[test]
fn selecting_a_target_yields_that_slice_alone() {
    let manifest = own_manifest();
    assert_eq!(
        manifest.target_subdir(Some("dart")).unwrap(),
        Some("src/dart")
    );
    assert_eq!(
        manifest.target_subdir(Some("typescript")).unwrap(),
        Some("src/ts")
    );
    // An unpublished language must fail loudly rather than silently install the
    // whole repository.
    assert!(manifest.target_subdir(Some("swift")).is_err());
}
