use zed_interfaces::{Lockfile, Manifest};

const TEMPLATES: &str = r#"
[package]
org = "oresoftware"
name = "ores-formal-methods-templates"
version = "0.1.0"
description = "Polyglot formal-methods templates"
license = "MIT"

[package.repository]
vcs = "git"
url = "https://github.com/ORESoftware/ores-formal-methods-templates"

[install]
adapter = "none"
dir = ".vendor/.zed"

[tool-dependencies]
"oresoftware/typespec-json-schema-validator" = "^0.1.1"
"#;

const TEMPLATES_LOCK: &str = r#"
version = 1

[[tool]]
org = "oresoftware"
name = "typespec-json-schema-validator"
version = "0.1.1"
sha256 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
size = 42
format = "tar.gz"
vcs_tag = "v0.1.1"
vcs_commit = "66aff520ab946f7d9b116e8c4f39423fa70e9558"
source = "https://registry.zpkg.net"
"#;

const FORMAL_METHODS_RS: &str = r#"
[package]
org = "oresoftware"
name = "formal-methods-rs"
version = "0.1.0"
description = "Formal-methods runner and polyglot SDKs"
license = "Apache-2.0"

[package.repository]
vcs = "git"
url = "https://github.com/ORESoftware/formal-methods.rs"

[install]
adapter = "none"
dir = ".vendor/.zed"
"#;

const ORES_WIT: &str = r#"
[package]
org = "oresoftware"
name = "ores-wit"
version = "0.1.0"
description = "WIT contract validation and binding orchestration"
license = "MIT"
language = "rust"

[package.repository]
vcs = "git"
url = "https://github.com/ORESoftware/ores-wit"

[build]
command = "cargo build --release --locked --bin ores-wit"
outputs = ["target/release/ores-wit"]
outputs_windows = ["target/release/ores-wit.exe"]

[bin]
ores-wit = "target/release/ores-wit"

[install]
adapter = "none"
dir = ".vendor/.zed"
"#;

#[test]
fn templates_use_a_tool_pin_without_creating_a_runtime_dependency() {
    let manifest = Manifest::parse(TEMPLATES).expect("templates manifest must be valid");
    assert!(manifest.dependencies.is_empty());
    assert!(manifest.build_dependencies.is_empty());
    assert!(manifest.targets.is_empty());
    assert_eq!(
        manifest
            .tool_dependencies
            .get("oresoftware/typespec-json-schema-validator")
            .map(String::as_str),
        Some("^0.1.1")
    );

    let lock = Lockfile::parse(TEMPLATES_LOCK).expect("templates tool lock must be valid");
    assert!(lock.packages.is_empty());
    let tool = lock
        .find_tool("oresoftware", "typespec-json-schema-validator")
        .expect("TJSV must be pinned as a tool");
    assert_eq!(tool.version, "0.1.1");
    assert_eq!(
        tool.vcs_commit.as_deref(),
        Some("66aff520ab946f7d9b116e8c4f39423fa70e9558")
    );
}

#[test]
fn tandem_repositories_stay_independent_and_acyclic() {
    for input in [FORMAL_METHODS_RS, ORES_WIT] {
        let manifest =
            Manifest::parse(input).expect("formal-method package manifest must be valid");
        assert!(manifest.dependencies.is_empty());
        assert!(manifest.build_dependencies.is_empty());
        assert!(manifest.tool_dependencies.is_empty());
        assert!(manifest.targets.is_empty());
    }

    let ores_wit = Manifest::parse(ORES_WIT).expect("ores-wit manifest must be valid");
    let build = ores_wit
        .build
        .expect("ores-wit must declare a locked build");
    assert!(build.command.contains("cargo build --release --locked"));
}
