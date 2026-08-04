#!/usr/bin/env python3
"""Compose and harden EnvironmentLock v1 on current zed-interfaces main."""

from __future__ import annotations

import re
from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if old in text:
        return text.replace(old, new, 1)
    if new in text:
        return text
    raise SystemExit(f"missing exact source anchor for {label}")


def sub_once(text: str, pattern: str, replacement: str, label: str) -> str:
    updated, count = re.subn(
        pattern,
        lambda _match: replacement,
        text,
        count=1,
        flags=re.MULTILINE | re.DOTALL,
    )
    if count == 1:
        return updated
    if replacement in text:
        return text
    raise SystemExit(f"missing regex source anchor for {label}")


def compose_manifest() -> None:
    path = Path("Cargo.toml")
    text = path.read_text()
    if '\nhex = "0.4"\n' not in text:
        text = text.replace("[dependencies]\n", "[dependencies]\nhex = \"0.4\"\n", 1)
    if '\nsha2 = "0.10"\n' not in text:
        text = text.replace(
            'serde_json = "1.0.151"\n',
            'serde_json = "1.0.151"\nsha2 = "0.10"\n',
            1,
        )
    path.write_text(text)


def compose_public_interface() -> None:
    path = Path("src/lib.rs")
    text = path.read_text()
    if "pub mod environment_lock;" not in text:
        text = text.replace(
            "pub mod environment;\n",
            "pub mod environment;\npub mod environment_lock;\n",
            1,
        )
    exports = """pub use environment_lock::{
    ENVIRONMENT_LOCK_SCHEMA_VERSION, EnvironmentLock, EnvironmentLockError,
    EnvironmentLockValidationMode, LockedArtifact, LockedArtifactFormat, LockedExecutable,
    LockedInstall, LockedPlatform, LockedSignature, LockedSource, LockedSourceKind, LockedTool,
};
"""
    if "pub use environment_lock::{" not in text:
        anchor = "pub use environment_v2::{\n"
        if anchor not in text:
            raise SystemExit("environment_v2 public export anchor missing")
        text = text.replace(anchor, exports + anchor, 1)
    path.write_text(text)


def compose_schema_generator() -> None:
    path = Path("examples/generate_schemas.rs")
    text = path.read_text()
    line = '    write::<zed_interfaces::EnvironmentLock>(dir, "environment-lock-v1");\n'
    if line not in text:
        anchor = '    write::<zed_interfaces::EnvironmentPlanV2>(dir, "environment-plan");\n'
        if anchor not in text:
            raise SystemExit("environment-plan-v2 schema anchor missing")
        text = text.replace(anchor, anchor + line, 1)
    path.write_text(text)


def harden_lock_contract() -> None:
    path = Path("src/environment_lock.rs")
    text = path.read_text()

    digest_error = """    #[error("{field} must be a 64-character hexadecimal SHA-256 digest")]
    InvalidSha256 { field: String },
"""
    hardened_errors = digest_error + """
    #[error("{field} must not contain credentials, query parameters, or fragments: `{value}`")]
    UnsafeLocator { field: String, value: String },

    #[error("tool `{tool}` source {kind:?} is incompatible with artifact format {format:?}")]
    SourceArtifactMismatch {
        tool: String,
        kind: LockedSourceKind,
        format: LockedArtifactFormat,
    },

    #[error("extension value `{path}` cannot be null")]
    NullExtension { path: String },
"""
    text = replace_once(text, digest_error, hardened_errors, "lock errors")

    locator_replacement = """        let locator_field = format!("tool `{tool}` source locator");
        validate_text(&locator_field, &self.locator)?;
        validate_source_locator(&locator_field, &self.locator, self.kind)?;
"""
    text = sub_once(
        text,
        r'''        validate_text\(\s*&format!\("tool `\{tool\}` source locator"\),\s*&self\.locator\s*,?\s*\)\?;\n''',
        locator_replacement,
        "source locator validation",
    )

    consistency = """        let path_source = self.kind == LockedSourceKind::Path;
        let directory_artifact = artifact.format == LockedArtifactFormat::Directory;
        if path_source != directory_artifact || (!path_source && self.tree_sha256.is_some()) {
            return Err(EnvironmentLockError::SourceArtifactMismatch {
                tool: tool.to_string(),
                kind: self.kind,
                format: artifact.format,
            });
        }

"""
    portability_anchor = """        if self.kind == LockedSourceKind::Path
            && mode == EnvironmentLockValidationMode::Portable
"""
    if consistency not in text:
        if portability_anchor not in text:
            raise SystemExit("path-source portability anchor missing")
        text = text.replace(portability_anchor, consistency + portability_anchor, 1)

    mirror_replacement = """        for (index, mirror) in self.mirrors.iter().enumerate() {
            let field = format!("tool `{tool}` mirror {index}");
            validate_text(&field, mirror)?;
            validate_network_locator(&field, mirror, false)?;
        }
"""
    text = sub_once(
        text,
        r'''        for \(index, mirror\) in self\.mirrors\.iter\(\)\.enumerate\(\) \{\s*validate_text\(\s*&format!\("tool `\{tool\}` mirror \{index\}"\),\s*mirror\s*,?\s*\)\?;\s*\}\n''',
        mirror_replacement,
        "artifact mirror validation",
    )

    text = replace_once(
        text,
        "if !names.insert(executable.name.clone()) {",
        "if !names.insert(portable_executable_key(&executable.name)) {",
        "primary executable collision key",
    )
    text = replace_once(
        text,
        "if !names.insert(alias.clone()) {",
        "if !names.insert(portable_executable_key(alias)) {",
        "executable alias collision key",
    )

    helper_anchor = "fn validate_text(field: &str, value: &str) -> Result<(), EnvironmentLockError> {\n"
    helpers = """fn portable_executable_key(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    for suffix in [".exe", ".cmd", ".bat", ".com"] {
        if let Some(stem) = lower.strip_suffix(suffix)
            && !stem.is_empty()
        {
            return stem.to_string();
        }
    }
    lower
}

fn validate_source_locator(
    field: &str,
    value: &str,
    kind: LockedSourceKind,
) -> Result<(), EnvironmentLockError> {
    if kind == LockedSourceKind::Path || kind == LockedSourceKind::Registry {
        return Ok(());
    }
    validate_network_locator(field, value, kind == LockedSourceKind::Vcs)
}

fn validate_network_locator(
    field: &str,
    value: &str,
    allow_git_user: bool,
) -> Result<(), EnvironmentLockError> {
    if value.contains('?') || value.contains('#') {
        return Err(EnvironmentLockError::UnsafeLocator {
            field: field.to_string(),
            value: value.to_string(),
        });
    }

    if let Some((_, remainder)) = value.split_once("://") {
        let authority = remainder.split('/').next().unwrap_or(remainder);
        if let Some((userinfo, _)) = authority.rsplit_once('@') {
            let allowed = allow_git_user && userinfo == "git";
            if !allowed {
                return Err(EnvironmentLockError::UnsafeLocator {
                    field: field.to_string(),
                    value: value.to_string(),
                });
            }
        }
    }
    Ok(())
}

"""
    if "fn portable_executable_key" not in text:
        if helper_anchor not in text:
            raise SystemExit("validation helper anchor missing")
        text = text.replace(helper_anchor, helpers + helper_anchor, 1)

    extension_replacement = """fn validate_extensions(
    field: &str,
    extensions: &BTreeMap<String, serde_json::Value>,
) -> Result<(), EnvironmentLockError> {
    for (key, value) in extensions {
        validate_text(&format!("{field} key"), key)?;
        validate_extension_value(&format!("{field}.{key}"), value)?;
    }
    serde_json::to_vec(extensions).map_err(|error| EnvironmentLockError::JsonSerialize {
        message: format!("{field}: {error}"),
    })?;
    Ok(())
}

fn validate_extension_value(
    path: &str,
    value: &serde_json::Value,
) -> Result<(), EnvironmentLockError> {
    match value {
        serde_json::Value::Null => Err(EnvironmentLockError::NullExtension {
            path: path.to_string(),
        }),
        serde_json::Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                validate_extension_value(&format!("{path}[{index}]"), value)?;
            }
            Ok(())
        }
        serde_json::Value::Object(values) => {
            for (key, value) in values {
                validate_text(&format!("{path} key"), key)?;
                validate_extension_value(&format!("{path}.{key}"), value)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn looks_floating"""
    text = sub_once(
        text,
        r'''fn validate_extensions\(\n    field: &str,\n    extensions: &BTreeMap<String, serde_json::Value>,\n\) -> Result<\(\), EnvironmentLockError> \{.*?\n\}\n\nfn looks_floating''',
        extension_replacement,
        "recursive extension validation",
    )

    regression_tests = """    #[test]
    fn credential_bearing_and_signed_urls_are_rejected() {
        let mut credential = registry_tool("1.2.3", "x86_64-unknown-linux-gnu");
        credential.source = LockedSource {
            kind: LockedSourceKind::Http,
            locator: "https://user:placeholder@example.invalid/tool.tar.gz".to_string(),
            revision: None,
            tree_sha256: None,
            immutable: false,
            portable: false,
            extensions: BTreeMap::new(),
        };
        assert!(matches!(
            lock_with(credential).validate(EnvironmentLockValidationMode::Portable),
            Err(EnvironmentLockError::UnsafeLocator { .. })
        ));

        let mut signed = registry_tool("1.2.3", "x86_64-unknown-linux-gnu");
        signed.artifact.mirrors = vec![
            "https://example.invalid/tool.tar.gz?X-Signature=placeholder".to_string(),
        ];
        assert!(matches!(
            lock_with(signed).validate(EnvironmentLockValidationMode::Portable),
            Err(EnvironmentLockError::UnsafeLocator { .. })
        ));
    }

    #[test]
    fn source_and_artifact_format_must_agree() {
        let mut local = registry_tool("1.2.3", "x86_64-unknown-linux-gnu");
        local.source = LockedSource {
            kind: LockedSourceKind::Path,
            locator: "vendor/tool".to_string(),
            revision: None,
            tree_sha256: Some(A.to_string()),
            immutable: false,
            portable: true,
            extensions: BTreeMap::new(),
        };
        assert!(matches!(
            lock_with(local).validate(EnvironmentLockValidationMode::Portable),
            Err(EnvironmentLockError::SourceArtifactMismatch { .. })
        ));

        let mut remote_directory = registry_tool("1.2.3", "x86_64-unknown-linux-gnu");
        remote_directory.artifact.format = LockedArtifactFormat::Directory;
        assert!(matches!(
            lock_with(remote_directory).validate(EnvironmentLockValidationMode::Portable),
            Err(EnvironmentLockError::SourceArtifactMismatch { .. })
        ));
    }

    #[test]
    fn executable_collisions_follow_windows_command_semantics() {
        let mut tool = registry_tool("22.4.0", "x86_64-pc-windows-msvc");
        tool.install.executables.push(LockedExecutable {
            name: "Node.EXE".to_string(),
            path: "bin/Node.EXE".to_string(),
            aliases: Vec::new(),
        });
        assert!(matches!(
            lock_with(tool).validate(EnvironmentLockValidationMode::Portable),
            Err(EnvironmentLockError::ExecutableCollision { .. })
        ));
    }

    #[test]
    fn null_extension_values_are_rejected_recursively() {
        let mut lock = lock_with(registry_tool("22.4.0", "x86_64-unknown-linux-gnu"));
        lock.extensions.insert(
            "future".to_string(),
            serde_json::json!({"nested": [1, null]}),
        );
        assert!(matches!(
            lock.validate(EnvironmentLockValidationMode::Portable),
            Err(EnvironmentLockError::NullExtension { .. })
        ));
    }

"""
    test_anchor = "    #[test]\n    fn malformed_digest_is_rejected() {\n"
    if "fn credential_bearing_and_signed_urls_are_rejected" not in text:
        if test_anchor not in text:
            raise SystemExit("environment-lock regression test anchor missing")
        text = text.replace(test_anchor, regression_tests + test_anchor, 1)

    path.write_text(text)


def main() -> None:
    compose_manifest()
    compose_public_interface()
    compose_schema_generator()
    harden_lock_contract()


if __name__ == "__main__":
    main()
