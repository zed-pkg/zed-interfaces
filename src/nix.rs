use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::artifact::ArtifactFormat;
use crate::manifest::{is_safe_relative_path, is_sha256_hex, is_slug};

pub const NIX_ADAPTER_SCHEMA: &str = "zed.nix-adapter/v1";
pub const NIX_ADAPTER_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "direction", rename_all = "kebab-case")]
pub enum NixAdapterRecord {
    ZedToNix {
        schema: String,
        schema_version: u32,
        package: NixAdapterPackage,
        zed: ZedArtifactOrigin,
        nix: NixExport,
        policy: ZedToNixPolicy,
        generated_files: BTreeMap<String, String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        licenses: Vec<String>,
    },
    NixToZed {
        schema: String,
        schema_version: u32,
        package: NixAdapterPackage,
        nix: NixRealizedOrigin,
        source: NixSourceMetadata,
        policy: NixToZedPolicy,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        sealed_paths: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        licenses: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        artifact: Option<ZedSealedArtifact>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NixAdapterPackage {
    pub org: String,
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ZedArtifactOrigin {
    pub repository: String,
    pub vcs_tag: String,
    pub vcs_commit: String,
    pub artifact_url: String,
    pub artifact_sha256: String,
    pub artifact_hash_sri: String,
    pub format: ArtifactFormat,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NixExport {
    pub nixpkgs_url: String,
    pub nixpkgs_revision: String,
    pub nixpkgs_nar_hash: String,
    pub systems: Vec<String>,
    pub attribute: String,
    pub install_layout: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ZedToNixPolicy {
    pub profile: String,
    pub resolution_authority: String,
    pub artifact_export: bool,
    pub dependency_graph: String,
    pub arbitrary_build_command: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NixRealizedOrigin {
    pub locked_ref: String,
    pub flake_lock_sha256: String,
    pub attribute: String,
    pub system: String,
    pub output: String,
    pub derivation_json_sha256: String,
    pub store_path: String,
    pub nar_hash: String,
    pub nar_size: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signatures: Vec<String>,
    pub nix_version: String,
    pub store_info_json_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NixSourceMetadata {
    pub repository: String,
    pub revision: String,
    pub available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NixToZedPolicy {
    pub profile: String,
    pub resolution_authority: String,
    pub pure_evaluation: bool,
    pub import_from_derivation: bool,
    pub sandbox_required: bool,
    pub builder_network: String,
    pub dirty_source: bool,
    pub portable_reference_count: u64,
    pub nix_required_at_zed_runtime: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ZedSealedArtifact {
    pub format: ArtifactFormat,
    pub file: String,
    pub sha256: String,
    pub size: u64,
    pub embedded_adapter_sha256: String,
    pub manifest_sha256: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum NixAdapterError {
    #[error("unsupported Nix adapter schema `{schema}` version {version}")]
    UnsupportedSchema { schema: String, version: u32 },
    #[error("invalid Zed package identity `{org}/{name}@{version}`")]
    InvalidPackage {
        org: String,
        name: String,
        version: String,
    },
    #[error("invalid SHA-256 hex digest for `{0}`")]
    InvalidHexDigest(String),
    #[error("invalid SHA-256 SRI hash for `{0}`")]
    InvalidSriHash(String),
    #[error("invalid or mutable Nix flake reference `{0}`")]
    MutableNixReference(String),
    #[error("invalid immutable revision `{0}`")]
    InvalidRevision(String),
    #[error("invalid Nix system `{0}`")]
    InvalidSystem(String),
    #[error("invalid adapter path `{0}`")]
    InvalidPath(String),
    #[error("strict Zed-to-Nix policy invariants are not satisfied")]
    InvalidExportPolicy,
    #[error("strict Nix-to-Zed policy invariants are not satisfied")]
    InvalidImportPolicy,
    #[error("portable Nix output retains external references: {0:?}")]
    NonPortableReferences(Vec<String>),
    #[error("source metadata is incomplete or not publishable")]
    InvalidSourceMetadata,
    #[error("adapter generated-file evidence is empty")]
    MissingGeneratedFiles,
}

impl NixAdapterRecord {
    pub fn validate(&self) -> Result<(), NixAdapterError> {
        match self {
            Self::ZedToNix {
                schema,
                schema_version,
                package,
                zed,
                nix,
                policy,
                generated_files,
                ..
            } => {
                validate_schema(schema, *schema_version)?;
                validate_package(package)?;
                validate_hex(&zed.artifact_sha256, "Zed artifact")?;
                validate_sri(&zed.artifact_hash_sri, "Zed artifact")?;
                validate_revision(&zed.vcs_commit)?;
                validate_revision(&nix.nixpkgs_revision)?;
                validate_sri(&nix.nixpkgs_nar_hash, "Nixpkgs input")?;
                validate_systems(&nix.systems)?;
                validate_path(&nix.install_layout)?;
                validate_generated_files(generated_files)?;
                if policy.profile != "strict-v1"
                    || policy.resolution_authority != "zed"
                    || !policy.artifact_export
                    || policy.dependency_graph != "empty"
                    || policy.arbitrary_build_command
                {
                    return Err(NixAdapterError::InvalidExportPolicy);
                }
            }
            Self::NixToZed {
                schema,
                schema_version,
                package,
                nix,
                source,
                policy,
                sealed_paths,
                artifact,
                ..
            } => {
                validate_schema(schema, *schema_version)?;
                validate_package(package)?;
                validate_immutable_reference(&nix.locked_ref)?;
                validate_hex(&nix.flake_lock_sha256, "flake.lock")?;
                validate_hex(&nix.derivation_json_sha256, "derivation JSON")?;
                validate_system(&nix.system)?;
                validate_sri(&nix.nar_hash, "realized Nix output")?;
                let external = nix
                    .references
                    .iter()
                    .filter(|reference| reference.as_str() != nix.store_path)
                    .cloned()
                    .collect::<Vec<_>>();
                if !external.is_empty() {
                    return Err(NixAdapterError::NonPortableReferences(external));
                }
                validate_revision(&source.revision)?;
                if !source.available || !source.repository.starts_with("https://") {
                    return Err(NixAdapterError::InvalidSourceMetadata);
                }
                if policy.profile != "strict-v1"
                    || policy.resolution_authority != "nix"
                    || !policy.pure_evaluation
                    || policy.import_from_derivation
                    || !policy.sandbox_required
                    || policy.builder_network != "disabled"
                    || policy.dirty_source
                    || policy.portable_reference_count != 0
                    || policy.nix_required_at_zed_runtime
                {
                    return Err(NixAdapterError::InvalidImportPolicy);
                }
                for path in sealed_paths {
                    validate_path(path)?;
                }
                if let Some(artifact) = artifact {
                    validate_path(&artifact.file)?;
                    validate_hex(&artifact.sha256, "sealed Zed artifact")?;
                    validate_hex(&artifact.embedded_adapter_sha256, "embedded adapter")?;
                    validate_hex(&artifact.manifest_sha256, "sealed manifest")?;
                }
            }
        }
        Ok(())
    }
}

fn validate_schema(schema: &str, version: u32) -> Result<(), NixAdapterError> {
    if schema == NIX_ADAPTER_SCHEMA && version == NIX_ADAPTER_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(NixAdapterError::UnsupportedSchema {
            schema: schema.to_string(),
            version,
        })
    }
}

fn validate_package(package: &NixAdapterPackage) -> Result<(), NixAdapterError> {
    if is_slug(&package.org)
        && is_slug(&package.name)
        && !package.version.is_empty()
        && !package.version.chars().any(char::is_whitespace)
        && package
            .target
            .as_ref()
            .map_or(true, |target| is_slug(target))
    {
        Ok(())
    } else {
        Err(NixAdapterError::InvalidPackage {
            org: package.org.clone(),
            name: package.name.clone(),
            version: package.version.clone(),
        })
    }
}

fn validate_hex(value: &str, label: &str) -> Result<(), NixAdapterError> {
    if is_sha256_hex(value) {
        Ok(())
    } else {
        Err(NixAdapterError::InvalidHexDigest(label.to_string()))
    }
}

fn validate_sri(value: &str, label: &str) -> Result<(), NixAdapterError> {
    let valid = value.strip_prefix("sha256-").is_some_and(|payload| {
        payload.len() == 44
            && payload.ends_with('=')
            && payload
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'='))
    });
    if valid {
        Ok(())
    } else {
        Err(NixAdapterError::InvalidSriHash(label.to_string()))
    }
}

fn validate_revision(value: &str) -> Result<(), NixAdapterError> {
    if (40..=64).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(NixAdapterError::InvalidRevision(value.to_string()))
    }
}

fn validate_immutable_reference(value: &str) -> Result<(), NixAdapterError> {
    let immutable_github = value
        .strip_prefix("github:")
        .and_then(|body| body.split('?').next())
        .is_some_and(|body| {
            let parts = body.split('/').collect::<Vec<_>>();
            parts.len() == 3 && validate_revision(parts[2]).is_ok()
        });
    let immutable_git_https = value.starts_with("git+https://")
        && value
            .split("rev=")
            .nth(1)
            .and_then(|revision| revision.split('&').next())
            .is_some_and(|revision| validate_revision(revision).is_ok());
    if immutable_github || immutable_git_https {
        Ok(())
    } else {
        Err(NixAdapterError::MutableNixReference(value.to_string()))
    }
}

fn validate_system(value: &str) -> Result<(), NixAdapterError> {
    if value.contains('-')
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_+.-".contains(&byte)
        })
    {
        Ok(())
    } else {
        Err(NixAdapterError::InvalidSystem(value.to_string()))
    }
}

fn validate_systems(values: &[String]) -> Result<(), NixAdapterError> {
    if values.is_empty() {
        return Err(NixAdapterError::InvalidSystem("<empty>".to_string()));
    }
    for value in values {
        validate_system(value)?;
    }
    Ok(())
}

fn validate_path(value: &str) -> Result<(), NixAdapterError> {
    if is_safe_relative_path(value) {
        Ok(())
    } else {
        Err(NixAdapterError::InvalidPath(value.to_string()))
    }
}

fn validate_generated_files(values: &BTreeMap<String, String>) -> Result<(), NixAdapterError> {
    if values.is_empty() {
        return Err(NixAdapterError::MissingGeneratedFiles);
    }
    for (path, hash) in values {
        validate_path(path)?;
        validate_hex(hash, path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package() -> NixAdapterPackage {
        NixAdapterPackage {
            org: "acme".to_string(),
            name: "portable".to_string(),
            version: "1.0.0".to_string(),
            target: None,
        }
    }

    #[test]
    fn strict_zed_export_validates() {
        let record = NixAdapterRecord::ZedToNix {
            schema: NIX_ADAPTER_SCHEMA.to_string(),
            schema_version: NIX_ADAPTER_SCHEMA_VERSION,
            package: package(),
            zed: ZedArtifactOrigin {
                repository: "https://github.com/acme/portable".to_string(),
                vcs_tag: "v1.0.0".to_string(),
                vcs_commit: "0".repeat(40),
                artifact_url: "https://registry.example/artifact".to_string(),
                artifact_sha256: "a".repeat(64),
                artifact_hash_sri: "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
                    .to_string(),
                format: ArtifactFormat::TarGz,
            },
            nix: NixExport {
                nixpkgs_url: "github:NixOS/nixpkgs/0000000000000000000000000000000000000000"
                    .to_string(),
                nixpkgs_revision: "0".repeat(40),
                nixpkgs_nar_hash: "sha256-BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB=".to_string(),
                systems: vec!["x86_64-linux".to_string()],
                attribute: "packages.${system}.portable".to_string(),
                install_layout: "share/zed-pkg/acme/portable".to_string(),
            },
            policy: ZedToNixPolicy {
                profile: "strict-v1".to_string(),
                resolution_authority: "zed".to_string(),
                artifact_export: true,
                dependency_graph: "empty".to_string(),
                arbitrary_build_command: false,
            },
            generated_files: BTreeMap::from([("flake.nix".to_string(), "c".repeat(64))]),
            licenses: vec!["MIT".to_string()],
        };
        assert_eq!(record.validate(), Ok(()));
    }

    #[test]
    fn closure_free_nix_import_validates_and_runtime_wrapper_fails() {
        let revision = "0".repeat(40);
        let mut record = NixAdapterRecord::NixToZed {
            schema: NIX_ADAPTER_SCHEMA.to_string(),
            schema_version: NIX_ADAPTER_SCHEMA_VERSION,
            package: package(),
            nix: NixRealizedOrigin {
                locked_ref: format!("github:acme/portable/{revision}"),
                flake_lock_sha256: "a".repeat(64),
                attribute: "packages.x86_64-linux.portable".to_string(),
                system: "x86_64-linux".to_string(),
                output: "out".to_string(),
                derivation_json_sha256: "b".repeat(64),
                store_path: "/nix/store/00000000000000000000000000000000-portable".to_string(),
                nar_hash: "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_string(),
                nar_size: 10,
                references: vec![],
                signatures: vec![],
                nix_version: "nix 2.31".to_string(),
                store_info_json_version: 1,
            },
            source: NixSourceMetadata {
                repository: "https://github.com/acme/portable".to_string(),
                revision,
                available: true,
            },
            policy: NixToZedPolicy {
                profile: "strict-v1".to_string(),
                resolution_authority: "nix".to_string(),
                pure_evaluation: true,
                import_from_derivation: false,
                sandbox_required: true,
                builder_network: "disabled".to_string(),
                dirty_source: false,
                portable_reference_count: 0,
                nix_required_at_zed_runtime: false,
            },
            sealed_paths: vec!["bin/portable".to_string()],
            licenses: vec!["MIT".to_string()],
            artifact: Some(ZedSealedArtifact {
                format: ArtifactFormat::TarGz,
                file: "portable-1.0.0.tar.gz".to_string(),
                sha256: "c".repeat(64),
                size: 20,
                embedded_adapter_sha256: "d".repeat(64),
                manifest_sha256: "e".repeat(64),
            }),
        };
        assert_eq!(record.validate(), Ok(()));
        if let NixAdapterRecord::NixToZed { policy, .. } = &mut record {
            policy.nix_required_at_zed_runtime = true;
        }
        assert_eq!(record.validate(), Err(NixAdapterError::InvalidImportPolicy));
    }

    #[test]
    fn external_reference_and_mutable_ref_fail_closed() {
        let external = vec!["/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-glibc".to_string()];
        assert_eq!(
            NixAdapterError::NonPortableReferences(external.clone()),
            NixAdapterError::NonPortableReferences(external)
        );
        assert!(matches!(
            validate_immutable_reference("github:NixOS/nixpkgs/nixos-unstable"),
            Err(NixAdapterError::MutableNixReference(_))
        ));
    }
}
