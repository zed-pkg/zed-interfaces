use zed_interfaces::ArtifactFormat;
use zed_interfaces::excludes::{ALWAYS_INCLUDE, DEFAULT_EXCLUDES, effective_excludes};
use zed_interfaces::language::{Ecosystem, Language};
use zed_interfaces::lockfile::{LockedPackage, Lockfile};
use zed_interfaces::manifest::{Manifest, ManifestError};
use zed_interfaces::paths::store_entry_rel;
use zed_interfaces::vcs::Vcs;

const SAMPLE: &str = r#"
[package]
org = "acme"
name = "http-kit"
version = "1.2.0"
description = "Tiny HTTP helpers"
license = "MIT"

[package.repository]
vcs = "git"
url = "https://github.com/acme/http-kit"

[dependencies]
"acme/logkit" = "^0.3"

[publish]
exclude = ["benches/**"]
smoke_test = "sh scripts/smoke.sh"

[scripts]
test = "make test"
"#;

#[test]
fn manifest_roundtrip() {
    let m = Manifest::parse(SAMPLE).unwrap();
    assert_eq!(m.full_name(), "acme/http-kit");
    assert_eq!(m.package.repository.vcs, Vcs::Git);
    assert_eq!(m.vcs_tag(), "v1.2.0");
    assert_eq!(m.publish.exclude, vec!["benches/**".to_string()]);
    assert!(!m.publish.include_readme);
    assert_eq!(m.scripts.test.as_deref(), Some("make test"));

    let serialized = m.to_toml_string().unwrap();
    let reparsed = Manifest::parse(&serialized).unwrap();
    assert_eq!(m, reparsed);
}

#[test]
fn install_dir_defaults_and_overrides() {
    // No [install] section -> the default dep dir.
    assert_eq!(
        Manifest::parse(SAMPLE).unwrap().modules_dir(),
        "zed_modules"
    );

    // A configured dir relocates the tree and round-trips.
    let with_dir = format!("{SAMPLE}\n[install]\ndir = \".vendor/.zed\"\n");
    let m = Manifest::parse(&with_dir).unwrap();
    assert_eq!(m.modules_dir(), ".vendor/.zed");
    assert_eq!(Manifest::parse(&m.to_toml_string().unwrap()).unwrap(), m);

    // Unsafe dirs are rejected.
    for bad in ["/abs/path", "../escape", "a/../../b"] {
        let src = format!("{SAMPLE}\n[install]\ndir = \"{bad}\"\n");
        assert!(
            matches!(
                Manifest::parse(&src),
                Err(ManifestError::InvalidInstallDir(_, _))
            ),
            "expected {bad} rejected"
        );
    }
}

#[test]
fn manifest_rejects_bad_input() {
    let bad_org = SAMPLE.replace("org = \"acme\"", "org = \"Acme!\"");
    assert!(matches!(
        Manifest::parse(&bad_org),
        Err(ManifestError::InvalidOrg(_))
    ));

    let bad_dep = SAMPLE.replace("\"acme/logkit\"", "\"logkit\"");
    assert!(matches!(
        Manifest::parse(&bad_dep),
        Err(ManifestError::InvalidDependencyKey(_))
    ));

    let bad_version = SAMPLE.replace("version = \"1.2.0\"", "version = \"not-semver\"");
    assert!(matches!(
        Manifest::parse(&bad_version),
        Err(ManifestError::InvalidVersion(_, _))
    ));
}

#[test]
fn lockfile_roundtrip() {
    let mut lock = Lockfile::default();
    lock.upsert(LockedPackage {
        org: "acme".into(),
        name: "http-kit".into(),
        version: "1.2.0".into(),
        sha256: "ab".repeat(32),
        size: 4096,
        format: ArtifactFormat::TarGz,
        vcs_tag: "v1.2.0".into(),
        vcs_commit: Some("deadbeef".into()),
        source: "https://registry.zed-pkg.dev".into(),
    });

    let text = lock.to_toml_string().unwrap();
    assert!(text.contains("[[package]]"));
    let reparsed = Lockfile::parse(&text).unwrap();
    assert_eq!(lock, reparsed);
    assert!(reparsed.find("acme", "http-kit").is_some());
}

#[test]
fn excludes_respect_include_readme() {
    let with_readme_stripped = effective_excludes(&[], false);
    assert!(with_readme_stripped.iter().any(|p| p == "README*"));
    assert!(with_readme_stripped.iter().any(|p| p == "tests/**"));

    let readme_kept = effective_excludes(&["extra/**".to_string()], true);
    assert!(!readme_kept.iter().any(|p| p == "README*"));
    assert!(readme_kept.iter().any(|p| p == "extra/**"));

    assert!(DEFAULT_EXCLUDES.contains(&".github/**"));
    assert!(DEFAULT_EXCLUDES.contains(&".zed-pack/**"));
    assert!(DEFAULT_EXCLUDES.contains(&"**/node_modules/**"));
    assert!(DEFAULT_EXCLUDES.contains(&"**/.dart_tool/**"));
    assert!(DEFAULT_EXCLUDES.contains(&"**/build/**"));
    assert!(ALWAYS_INCLUDE.contains(&"LICENSE*"));
    assert!(ALWAYS_INCLUDE.contains(&".zpkg.toml"));
}

#[test]
fn store_paths_are_sharded() {
    let sha = "abcdef0123".to_string() + &"0".repeat(54);
    assert_eq!(store_entry_rel(&sha), format!("store/v1/ab/{sha}"));
}

/// A polyglot package declares one subtree per ecosystem; consumers pick one.
const POLYGLOT: &str = r#"
[package]
org = "zedtest"
name = "polyglot-lib"
version = "1.0.0"

[package.repository]
vcs = "git"
url = "https://github.com/zed-pkg-test/polyglot-lib"

[targets.node]
dir = "node"

[targets.python]
dir = "python"

[targets.go]
dir = "go"
"#;

#[test]
fn polyglot_targets_roundtrip_and_resolve() {
    let m = Manifest::parse(POLYGLOT).unwrap();
    assert!(m.is_polyglot());
    assert_eq!(m.targets.len(), 3);
    assert_eq!(m.target_subdir(Some("python")).unwrap(), Some("python"));
    assert_eq!(m.target_subdir(Some("node")).unwrap(), Some("node"));
    // Round-trips through TOML unchanged.
    assert_eq!(Manifest::parse(&m.to_toml_string().unwrap()).unwrap(), m);
}

#[test]
fn a_single_language_package_ignores_target_selection() {
    // No [targets] => always the whole tree, even if a consumer asks for one.
    // Existing packages must keep installing exactly as before.
    let m = Manifest::parse(SAMPLE).unwrap();
    assert!(!m.is_polyglot());
    assert_eq!(m.target_subdir(None).unwrap(), None);
    assert_eq!(m.target_subdir(Some("python")).unwrap(), None);
}

#[test]
fn a_polyglot_package_without_a_request_yields_the_whole_tree() {
    // A consumer that has not opted into a target still installs fine.
    let m = Manifest::parse(POLYGLOT).unwrap();
    assert_eq!(m.target_subdir(None).unwrap(), None);
}

#[test]
fn requesting_an_unpublished_target_is_an_error_listing_what_exists() {
    let m = Manifest::parse(POLYGLOT).unwrap();
    let err = m
        .target_subdir(Some("ruby"))
        .expect_err("a target the package does not publish must not silently fall back");
    let msg = err.to_string();
    assert!(msg.contains("ruby"), "{msg}");
    assert!(msg.contains("zedtest/polyglot-lib"), "{msg}");
    // The message enumerates the real targets so the fix is obvious.
    for target in ["go", "node", "python"] {
        assert!(msg.contains(target), "expected `{target}` listed in: {msg}");
    }
}

#[test]
fn target_dirs_and_names_are_validated() {
    // `..` in a target dir would escape the package on install.
    let escaping = POLYGLOT.replace(r#"dir = "python""#, r#"dir = "../../etc""#);
    assert!(matches!(
        Manifest::parse(&escaping),
        Err(ManifestError::InvalidTarget(_, _))
    ));

    // Absolute dirs likewise.
    let absolute = POLYGLOT.replace(r#"dir = "python""#, r#"dir = "/etc/passwd""#);
    assert!(matches!(
        Manifest::parse(&absolute),
        Err(ManifestError::InvalidTarget(_, _))
    ));

    // Target names are slugs.
    let bad_name = POLYGLOT.replace("[targets.node]", "[targets.\"Node JS\"]");
    assert!(matches!(
        Manifest::parse(&bad_name),
        Err(ManifestError::InvalidTarget(_, _))
    ));

    // And so is a consumer's requested target.
    let bad_request = format!("{SAMPLE}\n[install]\ntarget = \"Python 3\"\n");
    assert!(matches!(
        Manifest::parse(&bad_request),
        Err(ManifestError::InvalidTarget(_, _))
    ));

    let bad_published_name = POLYGLOT.replace(
        "[targets.node]\ndir = \"node\"",
        "[targets.node]\ndir = \"node\"\nname = \"Node SDK\"",
    );
    assert!(matches!(
        Manifest::parse(&bad_published_name),
        Err(ManifestError::InvalidTarget(_, _))
    ));

    let duplicate_dir = POLYGLOT.replace(r#"dir = "python""#, r#"dir = "node""#);
    assert!(matches!(
        Manifest::parse(&duplicate_dir),
        Err(ManifestError::InvalidTarget(_, _))
    ));

    let duplicate_name = POLYGLOT.replace(
        "[targets.python]\ndir = \"python\"",
        "[targets.python]\ndir = \"python\"\nname = \"polyglot-lib-node\"",
    );
    assert!(matches!(
        Manifest::parse(&duplicate_name),
        Err(ManifestError::InvalidTarget(_, _))
    ));

    let bad_adapter = POLYGLOT.replace(
        "[targets.node]\ndir = \"node\"",
        "[targets.node]\ndir = \"node\"\nadapter = \"npm\"",
    );
    assert!(matches!(
        Manifest::parse(&bad_adapter),
        Err(ManifestError::InvalidTarget(_, _))
    ));
}

#[test]
fn nested_target_dirs_are_allowed() {
    // Real repos often nest, e.g. clients/go.
    let nested = POLYGLOT.replace(r#"dir = "go""#, r#"dir = "clients/go""#);
    let m = Manifest::parse(&nested).unwrap();
    assert_eq!(m.target_subdir(Some("go")).unwrap(), Some("clients/go"));
}

#[test]
fn consumer_requested_target_is_read_from_the_install_section() {
    let consumer = format!("{SAMPLE}\n[install]\ndir = \".vendor/.zed\"\ntarget = \"python\"\n");
    let m = Manifest::parse(&consumer).unwrap();
    assert_eq!(m.requested_target(), Some("python"));
    assert_eq!(m.modules_dir(), ".vendor/.zed");
    assert_eq!(Manifest::parse(&m.to_toml_string().unwrap()).unwrap(), m);

    // Absent or blank = no request.
    assert_eq!(Manifest::parse(SAMPLE).unwrap().requested_target(), None);
    let blank = format!("{SAMPLE}\n[install]\ntarget = \"  \"\n");
    assert_eq!(Manifest::parse(&blank).unwrap().requested_target(), None);
}

/// The real shape: a client repo publishing one package per language.
const CLIENTS: &str = r#"
[package]
org = "fiducia"
name = "fiducia-clients"
version = "1.1.2"
description = "Fiducia API clients"

[package.repository]
vcs = "git"
url = "https://github.com/fiducia-cloud/fiducia-clients"

[targets.nodejs]
dir = "clients/ts"
adapter = "node"

[targets.java]
dir = "clients/java"
adapter = "java"

[targets.golang]
dir = "clients/go"
"#;

#[test]
fn each_target_publishes_under_its_own_package_name() {
    let m = Manifest::parse(CLIENTS).unwrap();
    assert_eq!(
        m.target_package_name("nodejs").as_deref(),
        Some("fiducia-clients-nodejs")
    );
    assert_eq!(
        m.target_package_name("java").as_deref(),
        Some("fiducia-clients-java")
    );
    assert_eq!(
        m.target_package_name("golang").as_deref(),
        Some("fiducia-clients-golang")
    );
    assert_eq!(m.target_package_name("ruby"), None);

    // Deterministic, sorted fan-out list.
    assert_eq!(
        m.target_package_names(),
        vec![
            ("golang".to_string(), "fiducia-clients-golang".to_string()),
            ("java".to_string(), "fiducia-clients-java".to_string()),
            ("nodejs".to_string(), "fiducia-clients-nodejs".to_string()),
        ]
    );
}

#[test]
fn an_explicit_target_name_overrides_the_suffix_convention() {
    let custom = CLIENTS.replace(
        "[targets.nodejs]\ndir = \"clients/ts\"",
        "[targets.nodejs]\ndir = \"clients/ts\"\nname = \"fiducia-js-sdk\"",
    );
    let m = Manifest::parse(&custom).unwrap();
    assert_eq!(
        m.target_package_name("nodejs").as_deref(),
        Some("fiducia-js-sdk")
    );
    // Other targets keep the convention.
    assert_eq!(
        m.target_package_name("java").as_deref(),
        Some("fiducia-clients-java")
    );
}

#[test]
fn the_per_target_manifest_is_a_standalone_single_language_package() {
    let base = Manifest::parse(CLIENTS).unwrap();
    let java = base
        .manifest_for_target("java")
        .expect("java target exists");

    // It is its own package, sharing org/version/repo with the parent.
    assert_eq!(java.package.name, "fiducia-clients-java");
    assert_eq!(java.package.org, "fiducia");
    assert_eq!(java.package.version, "1.1.2");
    assert_eq!(java.full_name(), "fiducia/fiducia-clients-java");
    assert_eq!(
        java.package.repository.url, base.package.repository.url,
        "the artifact still points back at the source repo"
    );

    // It is NOT itself polyglot — the slice is single-language by construction,
    // so a consumer can never recurse into another fan-out.
    assert!(!java.is_polyglot());
    assert!(java.targets.is_empty());

    // It carries the ecosystem wiring its consumers need.
    assert_eq!(java.install.adapter.as_deref(), Some("java"));
    assert_eq!(
        base.manifest_for_target("nodejs")
            .unwrap()
            .install
            .adapter
            .as_deref(),
        Some("node")
    );
    // A target with no adapter inherits the base (here: none set).
    assert_eq!(
        base.manifest_for_target("golang").unwrap().install.adapter,
        None
    );

    // The derived manifest is valid on its own and round-trips.
    java.validate().expect("derived manifest must be valid");
    assert_eq!(
        Manifest::parse(&java.to_toml_string().unwrap()).unwrap(),
        java
    );

    // The description makes the language obvious in registry listings.
    assert_eq!(
        java.package.description.as_deref(),
        Some("Fiducia API clients (java)")
    );

    assert!(base.manifest_for_target("ruby").is_none());
}

#[test]
fn derived_target_names_must_still_be_valid_package_names() {
    // The suffix convention has to produce a legal slug, or publish would
    // emit an unusable package name.
    let m = Manifest::parse(CLIENTS).unwrap();
    for (_, name) in m.target_package_names() {
        assert!(
            zed_interfaces::manifest::is_slug(&name),
            "derived name `{name}` is not a valid package name"
        );
    }
}

// --- language / ecosystem tagging -----------------------------------------

/// A repo naming its targets the way the published packages read — the
/// colloquial `nodejs` / `golang` a human recalls — rather than the short
/// tokens project inference produces.
const COLLOQUIAL: &str = r#"
[package]
org = "fiducia"
name = "fiducia-clients"
version = "1.1.2"

[package.repository]
vcs = "git"
url = "https://github.com/fiducia-cloud/fiducia-clients"

[targets.nodejs]
dir = "clients/ts"

[targets.golang]
dir = "clients/go"

[targets.java]
dir = "clients/java"

[targets.kotlin]
dir = "clients/kotlin"

[targets.rust-wasm]
dir = "clients/rust-wasm"
ecosystem = "npm"
"#;

#[test]
fn a_project_inferred_as_node_resolves_a_nodejs_target() {
    // The decisive case for synonym resolution. Project inference yields
    // `node`/`go` from package.json/go.mod, but these packages publish as
    // `-nodejs`/`-golang` because that is what the names should read. Without
    // synonym matching every such consumer would hit "publishes no such
    // target" while the package ships exactly what they need.
    let m = Manifest::parse(COLLOQUIAL).unwrap();
    assert_eq!(m.target_subdir(Some("node")).unwrap(), Some("clients/ts"));
    assert_eq!(m.target_subdir(Some("go")).unwrap(), Some("clients/go"));
    // …and the spelling the author used keeps working too.
    assert_eq!(m.target_subdir(Some("nodejs")).unwrap(), Some("clients/ts"));
    assert_eq!(m.target_subdir(Some("golang")).unwrap(), Some("clients/go"));
    // As do the ecosystem's own near-synonyms.
    assert_eq!(
        m.target_subdir(Some("typescript")).unwrap(),
        Some("clients/ts")
    );
    assert_eq!(m.target_subdir(Some("ts")).unwrap(), Some("clients/ts"));
}

#[test]
fn synonym_resolution_does_not_collapse_distinct_languages() {
    // Java and Kotlin share an ecosystem but are separate packages; asking for
    // one must never hand back the other.
    let m = Manifest::parse(COLLOQUIAL).unwrap();
    assert_eq!(m.target_subdir(Some("java")).unwrap(), Some("clients/java"));
    assert_eq!(
        m.target_subdir(Some("kotlin")).unwrap(),
        Some("clients/kotlin")
    );
    // A JVM language the repo does not publish is still an error, not a
    // silent substitution of a sibling JVM target.
    assert!(m.target_subdir(Some("scala")).is_err());
}

#[test]
fn an_exact_target_key_wins_over_a_synonym() {
    // With both `node` and `nodejs` declared, `node` must mean the `node` one.
    let both = COLLOQUIAL.replace(
        "[targets.golang]\ndir = \"clients/go\"",
        "[targets.node]\ndir = \"clients/js-legacy\"",
    );
    let m = Manifest::parse(&both).unwrap();
    assert_eq!(
        m.target_subdir(Some("node")).unwrap(),
        Some("clients/js-legacy")
    );
    assert_eq!(m.target_subdir(Some("nodejs")).unwrap(), Some("clients/ts"));
}

#[test]
fn an_unknown_target_is_still_an_error_after_synonym_expansion() {
    // Synonyms must widen what resolves, never turn a real mistake into a
    // silent whole-tree install.
    let m = Manifest::parse(COLLOQUIAL).unwrap();
    let err = m
        .target_subdir(Some("cobol"))
        .expect_err("unknown language");
    assert!(err.to_string().contains("cobol"), "{err}");
}

#[test]
fn each_published_target_declares_its_own_language_and_ecosystem() {
    // This is what the consumer-side guard reads: the artifact for `-java` must
    // say `jvm` so an npm-only project can be told it is the wrong one.
    let m = Manifest::parse(COLLOQUIAL).unwrap();

    let java = m.manifest_for_target("java").unwrap();
    assert_eq!(java.package.name, "fiducia-clients-java");
    assert_eq!(java.package.language, Language::Java);
    assert_eq!(java.package.ecosystem(), Ecosystem::Jvm);

    let node = m.manifest_for_target("nodejs").unwrap();
    assert_eq!(node.package.name, "fiducia-clients-nodejs");
    assert_eq!(node.package.language, Language::Nodejs);
    assert_eq!(node.package.ecosystem(), Ecosystem::Npm);

    // Kotlin shares Java's ecosystem while keeping its own name — the reason
    // language and ecosystem are separate axes.
    let kotlin = m.manifest_for_target("kotlin").unwrap();
    assert_eq!(kotlin.package.name, "fiducia-clients-kotlin");
    assert_eq!(kotlin.package.language, Language::Kotlin);
    assert_eq!(kotlin.package.ecosystem(), Ecosystem::Jvm);

    // Each slice round-trips as a standalone single-language manifest.
    for target in ["java", "nodejs", "kotlin"] {
        let derived = m.manifest_for_target(target).unwrap();
        let reparsed = Manifest::parse(&derived.to_toml_string().unwrap()).unwrap();
        assert_eq!(reparsed, derived, "{target} manifest must round-trip");
        assert!(!reparsed.is_polyglot());
    }
}

#[test]
fn an_explicit_target_ecosystem_overrides_the_language_default() {
    // rust-wasm is Rust source consumed by a JS bundler, so the package must be
    // gated as npm even though the language is a Rust dialect.
    let m = Manifest::parse(COLLOQUIAL).unwrap();
    let wasm = m.manifest_for_target("rust-wasm").unwrap();
    assert_eq!(wasm.package.name, "fiducia-clients-rust-wasm");
    assert_eq!(wasm.package.language, Language::RustWasm);
    assert_eq!(wasm.package.ecosystem(), Ecosystem::Npm);
}

#[test]
fn a_target_key_that_is_not_a_known_language_stays_ungated() {
    // Arbitrary slugs remain legal target names (pre-existing behavior). Such a
    // package publishes and installs, it just carries no ecosystem claim, so
    // the guard cannot and must not reject it anywhere.
    let custom = POLYGLOT.replace(
        "[targets.go]\ndir = \"go\"",
        "[targets.wasm3]\ndir = \"w3\"",
    );
    let m = Manifest::parse(&custom).unwrap();
    let derived = m.manifest_for_target("wasm3").unwrap();
    assert_eq!(derived.package.name, "polyglot-lib-wasm3");
    assert!(derived.package.language.is_default());
    assert_eq!(derived.package.ecosystem(), Ecosystem::Universal);
}

#[test]
fn untagged_manifests_keep_their_meaning() {
    // Every manifest written before language tagging existed must parse and
    // stay ungated — this is the backwards-compatibility contract.
    let m = Manifest::parse(SAMPLE).unwrap();
    assert!(m.package.language.is_default());
    assert_eq!(m.package.ecosystem(), Ecosystem::Universal);
    // …and must not gain the fields when serialized back out.
    let toml = m.to_toml_string().unwrap();
    assert!(!toml.contains("language"), "{toml}");
    assert!(!toml.contains("ecosystem"), "{toml}");
}

#[test]
fn include_readme_also_keeps_the_changelog_registries_ask_for() {
    // `dart pub publish` fails a package outright for a missing CHANGELOG, and
    // `publish.exclude` can only add patterns — so if the default excludes strip
    // it, no repo can ship one. A package that opted into its README wants its
    // changelog too.
    let stripped = effective_excludes(&[], false);
    assert!(stripped.iter().any(|p| p.starts_with("README")));
    assert!(stripped.iter().any(|p| p.starts_with("CHANGELOG")));

    let kept = effective_excludes(&[], true);
    assert!(
        !kept.iter().any(|p| p.starts_with("README")),
        "include_readme must un-exclude READMEs"
    );
    assert!(
        !kept.iter().any(|p| p.starts_with("CHANGELOG")),
        "include_readme must un-exclude CHANGELOGs: {kept:?}"
    );
    // Everything else still goes.
    assert!(kept.iter().any(|p| p.contains("test")));
}

#[test]
fn a_synonym_request_prefers_the_target_named_after_the_language() {
    // shared-auth-clients ships separate TypeScript and JavaScript clients, so
    // two targets are both `nodejs`. A consumer inferred as `node` must land on
    // the canonical one, not on whichever key happens to sort first.
    let two_node = r#"
[package]
org = "shared-auth"
name = "shared-auth-clients"
version = "0.1.0"

[package.repository]
url = "https://github.com/shared-auth/shared-auth-clients"

[targets.javascript]
dir = "clients/js"

[targets.nodejs]
dir = "clients/ts"
"#;
    let m = Manifest::parse(two_node).unwrap();
    // `javascript` sorts before `nodejs`, so a naive first-match would pick it.
    assert_eq!(m.resolve_target_key("node"), Some("nodejs"));
    assert_eq!(m.target_subdir(Some("node")).unwrap(), Some("clients/ts"));
    // An exact key still wins over the canonical preference.
    assert_eq!(m.resolve_target_key("javascript"), Some("javascript"));
    assert_eq!(
        m.target_subdir(Some("javascript")).unwrap(),
        Some("clients/js")
    );
    // And a single-match repo is unaffected.
    let one = two_node.replace("[targets.javascript]\ndir = \"clients/js\"\n", "");
    let m1 = Manifest::parse(&one).unwrap();
    assert_eq!(m1.resolve_target_key("js"), Some("nodejs"));
}

// --- Audit chain (tamper-evident log) -----------------------------------

/// New chain fields must not break deserialization of a body produced by a
/// server that predates them: `seq` defaults to 0 and the hashes to empty.
#[test]
fn audit_entry_from_a_pre_chain_server_still_parses() {
    let legacy = r#"{
        "at": "2026-07-25T00:00:00Z",
        "action": "publish",
        "subject": "acme/http-kit@1.0.0",
        "actor_token_name": "ci",
        "actor_role": "publisher"
    }"#;
    let entry: zed_interfaces::registry::AuditEntry = serde_json::from_str(legacy).unwrap();
    assert_eq!(entry.seq, 0);
    assert_eq!(entry.entry_hash, "");
    assert_eq!(entry.prev_hash, None);
    assert_eq!(entry.subject, "acme/http-kit@1.0.0");
}

/// A full entry round-trips, and the empty chain fields stay off the wire so
/// the serialized shape is unchanged for servers that do not set them.
#[test]
fn audit_entry_roundtrips_and_omits_empty_chain_fields() {
    use zed_interfaces::registry::{AuditAction, AuditEntry};
    let entry = AuditEntry {
        at: "2026-07-25T00:00:00Z".to_string(),
        action: "yank".to_string(),
        action_kind: Some(AuditAction::Yank),
        subject: "acme/http-kit@1.0.0".to_string(),
        actor_token_name: "ci".to_string(),
        actor_role: "owner".to_string(),
        detail: None,
        seq: 7,
        entry_hash: "ab".repeat(32),
        prev_hash: Some("cd".repeat(32)),
    };
    let json = serde_json::to_string(&entry).unwrap();
    assert_eq!(serde_json::from_str::<AuditEntry>(&json).unwrap(), entry);

    let bare = AuditEntry {
        entry_hash: String::new(),
        prev_hash: None,
        ..entry
    };
    let value: serde_json::Value = serde_json::to_value(&bare).unwrap();
    assert!(
        value.get("entry_hash").is_none(),
        "empty hash must be omitted"
    );
    assert!(
        value.get("prev_hash").is_none(),
        "absent prev must be omitted"
    );
}

/// The preimage must be injective. A separator-joined encoding would let a
/// crafted field (a token literally named `x|publish`) shift boundaries and
/// collide with a different entry; length prefixes must prevent that.
#[test]
fn audit_preimage_resists_field_injection() {
    use zed_interfaces::registry::{AuditChainInput, audit_chain_preimage};

    let base = AuditChainInput {
        org_id: "org",
        seq: 1,
        at: "t",
        action: "publish",
        subject: "s",
        actor_token_id: Some("tok"),
        actor_token_name: "ci",
        actor_role: "owner",
        detail: Some("d"),
        prev_hash: "prev",
    };
    let digest = |i: &AuditChainInput| audit_chain_preimage(i);

    // Same characters, different field boundaries, must not collide.
    let honest = digest(&AuditChainInput {
        actor_role: "owner",
        detail: None,
        ..base
    });
    let shifted = digest(&AuditChainInput {
        actor_role: "own",
        detail: Some("er"),
        ..base
    });
    assert_ne!(honest, shifted, "field boundaries must be unambiguous");

    // A value that mimics the length-prefix syntax cannot fake a layout.
    let sneaky = digest(&AuditChainInput {
        actor_token_name: "5:owner",
        actor_role: "x",
        detail: None,
        ..base
    });
    assert_ne!(honest, sneaky);

    // Every distinguishing field must actually participate in the digest.
    let reference = digest(&base);
    let variants = [
        (
            "org_id",
            AuditChainInput {
                org_id: "OTHER",
                ..base
            },
        ),
        ("seq", AuditChainInput { seq: 2, ..base }),
        ("at", AuditChainInput { at: "T", ..base }),
        (
            "action",
            AuditChainInput {
                action: "yank",
                ..base
            },
        ),
        (
            "subject",
            AuditChainInput {
                subject: "S",
                ..base
            },
        ),
        (
            "actor_token_id",
            AuditChainInput {
                actor_token_id: Some("TOK"),
                ..base
            },
        ),
        (
            "actor_token_name",
            AuditChainInput {
                actor_token_name: "CI",
                ..base
            },
        ),
        (
            "actor_role",
            AuditChainInput {
                actor_role: "reader",
                ..base
            },
        ),
        (
            "detail",
            AuditChainInput {
                detail: Some("D"),
                ..base
            },
        ),
        (
            "prev_hash",
            AuditChainInput {
                prev_hash: "PREV",
                ..base
            },
        ),
    ];
    for (field, variant) in variants {
        assert_ne!(
            reference,
            digest(&variant),
            "changing {field} must change the preimage"
        );
    }

    // None and the empty string are deliberately the same absence.
    assert_eq!(
        digest(&AuditChainInput {
            actor_token_id: None,
            detail: None,
            ..base
        }),
        digest(&AuditChainInput {
            actor_token_id: Some(""),
            detail: Some(""),
            ..base
        }),
    );
}
