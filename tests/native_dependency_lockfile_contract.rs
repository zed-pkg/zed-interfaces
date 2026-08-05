use zed_interfaces::{
    ArtifactFormat, Lockfile, LockfileError, NativeArtifact, NativeDependencyError,
    NativeDependencyLock, NativeRegistry, NativeVersionCandidate,
};

fn artifact(digit: char) -> NativeArtifact {
    NativeArtifact {
        sha256: std::iter::repeat_n(digit, 64).collect(),
        size: 512,
        format: ArtifactFormat::TarGz,
    }
}

fn exact_lock(
    registry: NativeRegistry,
    package: &str,
    declared: &str,
    version: &str,
    digit: char,
) -> NativeDependencyLock {
    NativeDependencyLock::resolve(
        registry,
        package,
        declared,
        &[NativeVersionCandidate {
            version: version.to_string(),
            artifact: artifact(digit),
        }],
    )
    .expect("fixture must resolve")
}

#[test]
fn legacy_lockfile_v1_remains_readable_without_native_entries() {
    let legacy = r#"
version = 1

[[package]]
org = "zedtest"
name = "core"
version = "1.0.0"
sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
size = 128
format = "tar.gz"
vcs_tag = "v1.0.0"
vcs_commit = "0123456789abcdef0123456789abcdef01234567"
source = "file:///tmp/registry"
"#;

    let parsed = Lockfile::parse(legacy).expect("legacy v1 lockfile must parse");
    assert!(parsed.native_dependencies.is_empty());
    assert!(parsed.nix_adapters.is_empty());
    assert_eq!(parsed.find("zedtest", "core").unwrap().version, "1.0.0");

    let emitted = parsed.to_toml_string().unwrap();
    assert!(!emitted.contains("native-dependency"));
}

#[test]
fn native_locks_round_trip_and_serialize_independent_of_insertion_order() {
    let npm = exact_lock(NativeRegistry::Npm, "@fiducia/core", "^1.2.3", "1.9.0", 'a');
    let cargo = exact_lock(NativeRegistry::Cargo, "fiducia_core", "1.2.3", "1.9.0", 'b');

    let mut forward = Lockfile::default();
    forward.upsert_native_dependency(cargo.clone()).unwrap();
    forward.upsert_native_dependency(npm.clone()).unwrap();

    let mut reverse = Lockfile::default();
    reverse.upsert_native_dependency(npm.clone()).unwrap();
    reverse.upsert_native_dependency(cargo.clone()).unwrap();

    let forward_toml = forward.to_toml_string().unwrap();
    let reverse_toml = reverse.to_toml_string().unwrap();
    assert_eq!(forward_toml, reverse_toml);
    assert_eq!(forward_toml.matches("[[native-dependency]]").count(), 2);
    assert!(forward_toml.contains("declared = \"^1.2.3\""));
    assert!(forward_toml.contains("canonical = \"^1.2.3\""));

    let parsed = Lockfile::parse(&forward_toml).unwrap();
    assert_eq!(parsed, forward);
    assert_eq!(
        parsed
            .find_native_dependency(NativeRegistry::Npm, "@fiducia/core")
            .unwrap()
            .package
            .version,
        "1.9.0"
    );
    assert_eq!(
        parsed
            .find_native_dependency(NativeRegistry::Cargo, "fiducia_core")
            .unwrap()
            .artifact
            .sha256,
        "b".repeat(64)
    );
}

#[test]
fn upsert_replaces_only_the_same_registry_and_package_identity() {
    let npm_123 = exact_lock(NativeRegistry::Npm, "core", "1.2.3", "1.2.3", 'a');
    let npm_124 = exact_lock(NativeRegistry::Npm, "core", "1.2.4", "1.2.4", 'b');
    let cargo = exact_lock(NativeRegistry::Cargo, "core", "1.2.3", "1.9.0", 'c');

    let mut lockfile = Lockfile::default();
    lockfile.upsert_native_dependency(npm_123).unwrap();
    lockfile.upsert_native_dependency(cargo).unwrap();
    lockfile.upsert_native_dependency(npm_124).unwrap();

    assert_eq!(lockfile.native_dependencies.len(), 2);
    assert_eq!(
        lockfile
            .find_native_dependency(NativeRegistry::Npm, "core")
            .unwrap()
            .package
            .version,
        "1.2.4"
    );
    assert_eq!(
        lockfile
            .find_native_dependency(NativeRegistry::Cargo, "core")
            .unwrap()
            .package
            .version,
        "1.9.0"
    );
}

#[test]
fn duplicate_native_keys_fail_during_write_and_parse() {
    let first = exact_lock(NativeRegistry::Npm, "@fiducia/core", "1.2.3", "1.2.3", 'a');
    let second = exact_lock(NativeRegistry::Npm, "@fiducia/core", "1.2.4", "1.2.4", 'b');
    let duplicate = Lockfile {
        version: Lockfile::CURRENT_VERSION,
        packages: Vec::new(),
        native_dependencies: vec![first, second],
        nix_adapters: Vec::new(),
    };

    assert!(matches!(
        duplicate.to_toml_string(),
        Err(LockfileError::DuplicateNativeDependency(_))
    ));

    let raw = toml::to_string_pretty(&duplicate).unwrap();
    assert!(matches!(
        Lockfile::parse(&raw),
        Err(LockfileError::DuplicateNativeDependency(_))
    ));
}

#[test]
fn invalid_embedded_provenance_fails_during_upsert_write_and_parse() {
    let mut drift = exact_lock(NativeRegistry::Npm, "@fiducia/core", "^1.2.3", "1.9.0", 'a');
    drift.requirement.canonical = "^1.3.0".to_string();

    let mut lockfile = Lockfile::default();
    assert!(matches!(
        lockfile.upsert_native_dependency(drift.clone()),
        Err(LockfileError::InvalidNativeDependency(_))
    ));

    lockfile.native_dependencies.push(drift);
    assert!(matches!(
        lockfile.to_toml_string(),
        Err(LockfileError::InvalidNativeDependency(_))
    ));

    let raw = toml::to_string_pretty(&lockfile).unwrap();
    assert!(matches!(
        Lockfile::parse(&raw),
        Err(LockfileError::InvalidNativeDependency(_))
    ));
}

#[test]
fn embedded_lock_preserves_native_validation_errors() {
    let mut invalid = exact_lock(NativeRegistry::Cargo, "fiducia_core", "1.2.3", "1.9.0", 'a');
    invalid.schema = "zed.native-dependency-lock/v2".to_string();
    assert!(matches!(
        invalid.validate(),
        Err(NativeDependencyError::UnsupportedSchema { .. })
    ));

    let lockfile = Lockfile {
        version: Lockfile::CURRENT_VERSION,
        packages: Vec::new(),
        native_dependencies: vec![invalid],
        nix_adapters: Vec::new(),
    };
    assert!(matches!(
        lockfile.to_toml_string(),
        Err(LockfileError::InvalidNativeDependency(_))
    ));
}
