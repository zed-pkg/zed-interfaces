//! Immutable publication contracts for npm, Cargo, and future native registries.
//!
//! Zed may resolve versions using its broader polyglot version model, but a
//! publication crossing a strict native-registry boundary must use one exact
//! Semantic Versioning 2.0.0 identity. Platform identity is represented by
//! explicit operating-system, architecture, and libc selectors. It is never
//! encoded in SemVer build metadata, because build metadata does not
//! participate in version precedence and some registries treat versions that
//! differ only there as the same release.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use semver::{BuildMetadata, Version};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ArtifactFormat;

/// Current immutable native-registry adapter-record schema.
pub const NATIVE_REGISTRY_ADAPTER_SCHEMA_V1: &str = "zed.native-registry-adapter/v1";

/// Native registries with a first-class publication identity contract.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum NativeRegistry {
    Npm,
    Cargo,
}

/// Role of one package in a native-registry publication family.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum NativePublicationKind {
    /// One package whose payload is portable across all supported platforms.
    Portable,
    /// A generic wrapper package that selects platform-specific packages.
    Meta,
    /// One package containing bytes for exactly one explicit platform.
    Platform,
}

/// Platform identity kept outside the package version.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
pub struct NativePlatform {
    pub os: String,
    pub arch: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub libc: Option<String>,
}

impl NativePlatform {
    /// Stable human-readable selector used in diagnostics and deterministic
    /// ordering. It is not a package name and does not affect version
    /// precedence.
    pub fn selector(&self) -> String {
        match &self.libc {
            Some(libc) => format!("{}-{}-{libc}", self.os, self.arch),
            None => format!("{}-{}", self.os, self.arch),
        }
    }

    fn validate(&self, field: &str) -> Result<(), NativeRegistryError> {
        validate_lower_token(&format!("{field}.os"), &self.os)?;
        validate_lower_token(&format!("{field}.arch"), &self.arch)?;
        if let Some(libc) = &self.libc {
            validate_lower_token(&format!("{field}.libc"), libc)?;
        }
        Ok(())
    }
}

/// Strict SemVer source identity in the Zed registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ZedNativePackageIdentity {
    pub org: String,
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

impl ZedNativePackageIdentity {
    fn validate(&self) -> Result<(), NativeRegistryError> {
        validate_identity_component("source.org", &self.org)?;
        validate_identity_component("source.name", &self.name)?;
        if let Some(target) = &self.target {
            validate_identity_component("source.target", target)?;
        }
        validate_native_version("source.version", &self.version).map(|_| ())
    }
}

/// Public identity selected in npm or a Cargo-compatible registry.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub struct NativePackageIdentity {
    pub name: String,
    pub version: String,
}

impl NativePackageIdentity {
    fn validate(&self, registry: NativeRegistry, field: &str) -> Result<(), NativeRegistryError> {
        validate_native_package_name(registry, &self.name)?;
        validate_native_version(&format!("{field}.version"), &self.version).map(|_| ())
    }
}

/// Exact immutable bytes published under one native package identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NativeArtifact {
    /// Lowercase hexadecimal SHA-256 of the exact uploaded archive bytes.
    pub sha256: String,
    pub size: u64,
    #[serde(default)]
    pub format: ArtifactFormat,
}

impl NativeArtifact {
    fn validate(&self, field: &str) -> Result<(), NativeRegistryError> {
        if self.sha256.len() != 64
            || !self
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(NativeRegistryError::InvalidSha256 {
                field: format!("{field}.sha256"),
                value: self.sha256.clone(),
            });
        }
        if self.size == 0 {
            return Err(NativeRegistryError::EmptyArtifact {
                field: field.to_string(),
            });
        }
        Ok(())
    }
}

/// One platform-to-package edge emitted by a generic wrapper package.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub struct NativePlatformPackage {
    pub platform: NativePlatform,
    pub package: String,
}

impl NativePlatformPackage {
    fn validate(&self, registry: NativeRegistry, field: &str) -> Result<(), NativeRegistryError> {
        self.platform.validate(&format!("{field}.platform"))?;
        validate_native_package_name(registry, &self.package)
    }
}

/// One uploaded package within a publication family.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NativePublication {
    pub package: NativePackageIdentity,
    pub kind: NativePublicationKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<NativePlatform>,
    /// Only a `meta` publication may contain selector edges. Each edge must
    /// reference a `platform` publication in the same adapter record.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub platform_packages: Vec<NativePlatformPackage>,
    pub artifact: NativeArtifact,
}

impl NativePublication {
    fn normalized(&self) -> Self {
        let mut publication = self.clone();
        publication.platform_packages.sort();
        publication
    }

    fn validate(
        &self,
        registry: NativeRegistry,
        source_version: &str,
        index: usize,
    ) -> Result<(), NativeRegistryError> {
        let field = format!("publications[{index}]");
        self.package
            .validate(registry, &format!("{field}.package"))?;
        if self.package.version != source_version {
            return Err(NativeRegistryError::VersionDrift {
                package: self.package.name.clone(),
                source_version: source_version.to_string(),
                publication_version: self.package.version.clone(),
            });
        }

        match self.kind {
            NativePublicationKind::Platform => {
                let platform = self.platform.as_ref().ok_or_else(|| {
                    NativeRegistryError::PlatformRequired {
                        package: self.package.name.clone(),
                    }
                })?;
                platform.validate(&format!("{field}.platform"))?;
                if !self.platform_packages.is_empty() {
                    return Err(NativeRegistryError::PlatformPackagesNotAllowed {
                        package: self.package.name.clone(),
                        kind: self.kind,
                    });
                }
            }
            NativePublicationKind::Portable | NativePublicationKind::Meta => {
                if self.platform.is_some() {
                    return Err(NativeRegistryError::UnexpectedPlatform {
                        package: self.package.name.clone(),
                        kind: self.kind,
                    });
                }
                if self.kind == NativePublicationKind::Portable
                    && !self.platform_packages.is_empty()
                {
                    return Err(NativeRegistryError::PlatformPackagesNotAllowed {
                        package: self.package.name.clone(),
                        kind: self.kind,
                    });
                }
            }
        }

        let mut selected_platforms = BTreeSet::new();
        for (selection_index, selection) in self.platform_packages.iter().enumerate() {
            selection.validate(
                registry,
                &format!("{field}.platform-packages[{selection_index}]"),
            )?;
            if !selected_platforms.insert(selection.platform.clone()) {
                return Err(NativeRegistryError::DuplicateMetaPlatform {
                    package: self.package.name.clone(),
                    platform: selection.platform.selector(),
                });
            }
        }
        self.artifact.validate(&format!("{field}.artifact"))
    }
}

/// Final immutable mapping from one Zed source target to a family of native
/// registry packages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NativeRegistryAdapterRecord {
    pub schema: String,
    pub registry: NativeRegistry,
    pub source: ZedNativePackageIdentity,
    pub publications: Vec<NativePublication>,
}

impl NativeRegistryAdapterRecord {
    pub const SCHEMA_V1: &'static str = NATIVE_REGISTRY_ADAPTER_SCHEMA_V1;

    /// Presentation-independent ordering for hashing, signing, lock
    /// provenance, and generated client fixtures.
    pub fn normalized(&self) -> Self {
        let mut record = self.clone();
        record.publications = record
            .publications
            .iter()
            .map(NativePublication::normalized)
            .collect();
        record.publications.sort_by(|left, right| {
            (
                &left.package,
                left.kind,
                &left.platform,
                &left.platform_packages,
                &left.artifact.sha256,
            )
                .cmp(&(
                    &right.package,
                    right.kind,
                    &right.platform,
                    &right.platform_packages,
                    &right.artifact.sha256,
                ))
        });
        record
    }

    /// Canonical compact JSON bytes. Validation runs first so invalid identity
    /// cannot be made acceptable merely by normalization.
    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>, NativeRegistryError> {
        self.validate()?;
        serde_json::to_vec(&self.normalized())
            .map_err(|error| NativeRegistryError::Serialization(error.to_string()))
    }

    pub fn validate(&self) -> Result<(), NativeRegistryError> {
        if self.schema != Self::SCHEMA_V1 {
            return Err(NativeRegistryError::UnsupportedSchema {
                found: self.schema.clone(),
                supported: Self::SCHEMA_V1.to_string(),
            });
        }
        self.source.validate()?;
        if self.publications.is_empty() {
            return Err(NativeRegistryError::NoPublications);
        }

        let mut package_identities = BTreeSet::new();
        let mut platform_publications = BTreeMap::new();
        let mut meta_count = 0usize;
        let mut portable_count = 0usize;

        for (index, publication) in self.publications.iter().enumerate() {
            publication.validate(self.registry, &self.source.version, index)?;
            let identity = (
                publication.package.name.clone(),
                semver_precedence_identity(&publication.package.version)?,
            );
            if !package_identities.insert(identity.clone()) {
                return Err(NativeRegistryError::DuplicatePackageVersion {
                    package: identity.0,
                    version: identity.1,
                });
            }

            match publication.kind {
                NativePublicationKind::Meta => {
                    meta_count += 1;
                    if meta_count > 1 {
                        return Err(NativeRegistryError::MultipleMetaPackages);
                    }
                }
                NativePublicationKind::Platform => {
                    let platform = publication
                        .platform
                        .as_ref()
                        .expect("platform publication validated above")
                        .clone();
                    if let Some(existing) = platform_publications
                        .insert(platform.clone(), publication.package.name.clone())
                    {
                        return Err(NativeRegistryError::DuplicatePlatformPublication {
                            platform: platform.selector(),
                            first: existing,
                            second: publication.package.name.clone(),
                        });
                    }
                }
                NativePublicationKind::Portable => {
                    portable_count += 1;
                    if portable_count > 1 {
                        return Err(NativeRegistryError::MultiplePortablePackages);
                    }
                }
            }
        }

        for publication in self
            .publications
            .iter()
            .filter(|publication| publication.kind == NativePublicationKind::Meta)
        {
            if publication.platform_packages.is_empty() {
                return Err(NativeRegistryError::MetaPackageRequiresPlatformSelections {
                    package: publication.package.name.clone(),
                });
            }

            let mut selected_platforms = BTreeSet::new();
            for selection in &publication.platform_packages {
                selected_platforms.insert(selection.platform.clone());
                match platform_publications.get(&selection.platform) {
                    Some(package) if package == &selection.package => {}
                    Some(package) => {
                        return Err(NativeRegistryError::PlatformPackageMismatch {
                            meta_package: publication.package.name.clone(),
                            platform: selection.platform.selector(),
                            expected: package.clone(),
                            selected: selection.package.clone(),
                        });
                    }
                    None => {
                        return Err(NativeRegistryError::MissingPlatformPublication {
                            meta_package: publication.package.name.clone(),
                            platform: selection.platform.selector(),
                            selected: selection.package.clone(),
                        });
                    }
                }
            }

            for (platform, package) in &platform_publications {
                if !selected_platforms.contains(platform) {
                    return Err(NativeRegistryError::UnselectedPlatformPublication {
                        meta_package: publication.package.name.clone(),
                        platform: platform.selector(),
                        package: package.clone(),
                    });
                }
            }
        }

        Ok(())
    }
}

/// SemVer precedence identity with build metadata removed.
///
/// Native adapters can use this before planning a publication set to detect
/// collisions such as `1.0.0+linux` and `1.0.0+darwin`.
pub fn semver_precedence_identity(version: &str) -> Result<String, NativeRegistryError> {
    let mut version =
        Version::parse(version).map_err(|error| NativeRegistryError::InvalidSemver {
            field: "version".to_string(),
            version: version.to_string(),
            detail: error.to_string(),
        })?;
    version.build = BuildMetadata::EMPTY;
    Ok(version.to_string())
}

/// Whether two valid SemVer strings identify the same native-registry version
/// after build metadata is ignored.
pub fn native_versions_collide(left: &str, right: &str) -> Result<bool, NativeRegistryError> {
    Ok(semver_precedence_identity(left)? == semver_precedence_identity(right)?)
}

fn validate_native_version(field: &str, version: &str) -> Result<Version, NativeRegistryError> {
    let parsed = Version::parse(version).map_err(|error| NativeRegistryError::InvalidSemver {
        field: field.to_string(),
        version: version.to_string(),
        detail: error.to_string(),
    })?;
    if parsed.build != BuildMetadata::EMPTY {
        return Err(NativeRegistryError::BuildMetadataNotAllowed {
            field: field.to_string(),
            version: version.to_string(),
        });
    }
    Ok(parsed)
}

fn validate_native_package_name(
    registry: NativeRegistry,
    name: &str,
) -> Result<(), NativeRegistryError> {
    match registry {
        NativeRegistry::Cargo => validate_cargo_name(name),
        NativeRegistry::Npm => validate_npm_name(name),
    }
}

fn validate_cargo_name(name: &str) -> Result<(), NativeRegistryError> {
    if name.is_empty()
        || name.len() > 64
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(NativeRegistryError::InvalidPackageName {
            registry: NativeRegistry::Cargo,
            name: name.to_string(),
            detail: "expected 1-64 ASCII alphanumeric, `-`, or `_` characters".to_string(),
        });
    }
    Ok(())
}

fn validate_npm_name(name: &str) -> Result<(), NativeRegistryError> {
    if name.is_empty() || name.len() > 214 || name.bytes().any(|byte| byte.is_ascii_uppercase()) {
        return Err(NativeRegistryError::InvalidPackageName {
            registry: NativeRegistry::Npm,
            name: name.to_string(),
            detail: "expected a lowercase npm name no longer than 214 bytes".to_string(),
        });
    }

    let components: Vec<&str> = if let Some(scoped) = name.strip_prefix('@') {
        let mut parts = scoped.split('/');
        let scope = parts.next().unwrap_or_default();
        let package = parts.next().unwrap_or_default();
        if scope.is_empty() || package.is_empty() || parts.next().is_some() {
            return Err(NativeRegistryError::InvalidPackageName {
                registry: NativeRegistry::Npm,
                name: name.to_string(),
                detail: "scoped names must use exactly `@scope/package`".to_string(),
            });
        }
        vec![scope, package]
    } else {
        if name.contains('/') {
            return Err(NativeRegistryError::InvalidPackageName {
                registry: NativeRegistry::Npm,
                name: name.to_string(),
                detail: "unscoped names may not contain `/`".to_string(),
            });
        }
        vec![name]
    };

    for component in components {
        let mut bytes = component.bytes();
        let first = bytes
            .next()
            .ok_or_else(|| NativeRegistryError::InvalidPackageName {
                registry: NativeRegistry::Npm,
                name: name.to_string(),
                detail: "name components must not be empty".to_string(),
            })?;
        if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
            return Err(NativeRegistryError::InvalidPackageName {
                registry: NativeRegistry::Npm,
                name: name.to_string(),
                detail: "name components must start with a lowercase letter or digit".to_string(),
            });
        }
        if !component.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
        }) {
            return Err(NativeRegistryError::InvalidPackageName {
                registry: NativeRegistry::Npm,
                name: name.to_string(),
                detail: "name components may contain lowercase letters, digits, `-`, `_`, or `.`"
                    .to_string(),
            });
        }
    }
    Ok(())
}

fn validate_identity_component(field: &str, value: &str) -> Result<(), NativeRegistryError> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(NativeRegistryError::InvalidIdentityComponent {
            field: field.to_string(),
            value: value.to_string(),
        });
    }
    Ok(())
}

fn validate_lower_token(field: &str, value: &str) -> Result<(), NativeRegistryError> {
    if value.is_empty()
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
        })
    {
        return Err(NativeRegistryError::InvalidPlatformToken {
            field: field.to_string(),
            value: value.to_string(),
        });
    }
    Ok(())
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NativeRegistryError {
    #[error("unsupported native-registry adapter schema `{found}`; expected `{supported}`")]
    UnsupportedSchema { found: String, supported: String },
    #[error("native-registry adapter record must contain at least one publication")]
    NoPublications,
    #[error("invalid native package name `{name}` for {registry:?}: {detail}")]
    InvalidPackageName {
        registry: NativeRegistry,
        name: String,
        detail: String,
    },
    #[error("invalid strict SemVer `{version}` at `{field}`: {detail}")]
    InvalidSemver {
        field: String,
        version: String,
        detail: String,
    },
    #[error(
        "SemVer build metadata is not allowed at `{field}` (`{version}`); encode platform identity outside the version"
    )]
    BuildMetadataNotAllowed { field: String, version: String },
    #[error(
        "native publication `{package}` uses version `{publication_version}`, but the Zed source uses `{source_version}`"
    )]
    VersionDrift {
        package: String,
        source_version: String,
        publication_version: String,
    },
    #[error("invalid Zed package identity component `{value}` at `{field}`")]
    InvalidIdentityComponent { field: String, value: String },
    #[error("invalid lowercase platform token `{value}` at `{field}`")]
    InvalidPlatformToken { field: String, value: String },
    #[error("invalid lowercase SHA-256 `{value}` at `{field}`")]
    InvalidSha256 { field: String, value: String },
    #[error("artifact at `{field}` must contain at least one byte")]
    EmptyArtifact { field: String },
    #[error("platform publication `{package}` requires an explicit platform selector")]
    PlatformRequired { package: String },
    #[error("{kind:?} publication `{package}` may not contain a platform selector")]
    UnexpectedPlatform {
        package: String,
        kind: NativePublicationKind,
    },
    #[error("{kind:?} publication `{package}` may not select platform packages")]
    PlatformPackagesNotAllowed {
        package: String,
        kind: NativePublicationKind,
    },
    #[error("meta publication `{package}` selects platform `{platform}` more than once")]
    DuplicateMetaPlatform { package: String, platform: String },
    #[error("duplicate native package identity `{package}@{version}`")]
    DuplicatePackageVersion { package: String, version: String },
    #[error("one adapter record may contain at most one portable package")]
    MultiplePortablePackages,
    #[error("one adapter record may contain at most one meta package")]
    MultipleMetaPackages,
    #[error("meta publication `{package}` must select at least one platform package")]
    MetaPackageRequiresPlatformSelections { package: String },
    #[error("platform `{platform}` is published twice by `{first}` and `{second}`")]
    DuplicatePlatformPublication {
        platform: String,
        first: String,
        second: String,
    },
    #[error(
        "meta package `{meta_package}` selects `{selected}` for `{platform}`, but the record publishes `{expected}`"
    )]
    PlatformPackageMismatch {
        meta_package: String,
        platform: String,
        expected: String,
        selected: String,
    },
    #[error(
        "meta package `{meta_package}` selects missing platform package `{selected}` for `{platform}`"
    )]
    MissingPlatformPublication {
        meta_package: String,
        platform: String,
        selected: String,
    },
    #[error(
        "meta package `{meta_package}` does not select published platform package `{package}` for `{platform}`"
    )]
    UnselectedPlatformPublication {
        meta_package: String,
        platform: String,
        package: String,
    },
    #[error("failed to serialize native-registry adapter record: {0}")]
    Serialization(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn platform(os: &str, arch: &str, libc: Option<&str>) -> NativePlatform {
        NativePlatform {
            os: os.to_string(),
            arch: arch.to_string(),
            libc: libc.map(str::to_string),
        }
    }

    fn artifact(byte: char) -> NativeArtifact {
        NativeArtifact {
            sha256: std::iter::repeat_n(byte, 64).collect(),
            size: 128,
            format: ArtifactFormat::TarGz,
        }
    }

    fn publication(
        name: &str,
        kind: NativePublicationKind,
        platform: Option<NativePlatform>,
        byte: char,
    ) -> NativePublication {
        NativePublication {
            package: NativePackageIdentity {
                name: name.to_string(),
                version: "1.2.3".to_string(),
            },
            kind,
            platform,
            platform_packages: Vec::new(),
            artifact: artifact(byte),
        }
    }

    fn record() -> NativeRegistryAdapterRecord {
        let linux = platform("linux", "arm64", Some("musl"));
        let darwin = platform("darwin", "arm64", None);
        let mut meta = publication("@fiducia/core", NativePublicationKind::Meta, None, 'a');
        meta.platform_packages = vec![
            NativePlatformPackage {
                platform: linux.clone(),
                package: "@fiducia/core-linux-arm64-musl".to_string(),
            },
            NativePlatformPackage {
                platform: darwin.clone(),
                package: "@fiducia/core-darwin-arm64".to_string(),
            },
        ];
        NativeRegistryAdapterRecord {
            schema: NATIVE_REGISTRY_ADAPTER_SCHEMA_V1.to_string(),
            registry: NativeRegistry::Npm,
            source: ZedNativePackageIdentity {
                org: "fiducia".to_string(),
                name: "core".to_string(),
                version: "1.2.3".to_string(),
                target: Some("node".to_string()),
            },
            publications: vec![
                publication(
                    "@fiducia/core-linux-arm64-musl",
                    NativePublicationKind::Platform,
                    Some(linux),
                    'b',
                ),
                meta,
                publication(
                    "@fiducia/core-darwin-arm64",
                    NativePublicationKind::Platform,
                    Some(darwin),
                    'c',
                ),
            ],
        }
    }

    #[test]
    fn valid_multi_platform_record_is_canonical() {
        let mut reversed = record();
        reversed.publications.reverse();
        for publication in &mut reversed.publications {
            publication.platform_packages.reverse();
        }

        assert!(record().validate().is_ok());
        assert_eq!(
            record().canonical_json_bytes().unwrap(),
            reversed.canonical_json_bytes().unwrap()
        );
    }

    #[test]
    fn build_metadata_collides_and_is_rejected_for_publication() {
        assert!(native_versions_collide("1.2.3+linux", "1.2.3+darwin").unwrap());
        assert!(!native_versions_collide("1.2.3-rc.1", "1.2.3").unwrap());

        let mut record = record();
        record.source.version = "1.2.3+linux".to_string();
        for publication in &mut record.publications {
            publication.package.version = "1.2.3+linux".to_string();
        }
        assert!(matches!(
            record.validate(),
            Err(NativeRegistryError::BuildMetadataNotAllowed { .. })
        ));
    }

    #[test]
    fn platform_identity_cannot_be_smuggled_into_a_portable_publication() {
        let mut record = record();
        let meta = record
            .publications
            .iter_mut()
            .find(|publication| publication.kind == NativePublicationKind::Meta)
            .unwrap();
        meta.platform = Some(platform("linux", "x64", Some("gnu")));
        assert!(matches!(
            record.validate(),
            Err(NativeRegistryError::UnexpectedPlatform { .. })
        ));
    }

    #[test]
    fn meta_edges_must_reference_the_exact_platform_publication() {
        let mut record = record();
        let meta = record
            .publications
            .iter_mut()
            .find(|publication| publication.kind == NativePublicationKind::Meta)
            .unwrap();
        meta.platform_packages[0].package = "@fiducia/not-published".to_string();
        assert!(matches!(
            record.validate(),
            Err(NativeRegistryError::PlatformPackageMismatch { .. })
        ));
    }

    #[test]
    fn duplicate_platforms_and_precedence_identities_fail_closed() {
        let mut duplicate_platform = record();
        let first_platform = duplicate_platform
            .publications
            .iter()
            .find(|publication| publication.kind == NativePublicationKind::Platform)
            .unwrap()
            .platform
            .clone();
        let mut extra = publication(
            "@fiducia/core-another",
            NativePublicationKind::Platform,
            first_platform,
            'd',
        );
        extra.package.version = "1.2.3".to_string();
        duplicate_platform.publications.push(extra);
        assert!(matches!(
            duplicate_platform.validate(),
            Err(NativeRegistryError::DuplicatePlatformPublication { .. })
        ));

        let mut duplicate_package = record();
        let duplicate = duplicate_package.publications[0].clone();
        duplicate_package.publications.push(duplicate);
        assert!(matches!(
            duplicate_package.validate(),
            Err(NativeRegistryError::DuplicatePackageVersion { .. })
        ));
    }

    #[test]
    fn publication_family_cardinality_and_meta_coverage_fail_closed() {
        let mut multiple_portable = record();
        multiple_portable.publications.extend([
            publication(
                "@fiducia/core-portable",
                NativePublicationKind::Portable,
                None,
                'd',
            ),
            publication(
                "@fiducia/core-portable-extra",
                NativePublicationKind::Portable,
                None,
                'e',
            ),
        ]);
        assert!(matches!(
            multiple_portable.validate(),
            Err(NativeRegistryError::MultiplePortablePackages)
        ));

        let mut empty_meta = record();
        empty_meta
            .publications
            .iter_mut()
            .find(|publication| publication.kind == NativePublicationKind::Meta)
            .unwrap()
            .platform_packages
            .clear();
        assert!(matches!(
            empty_meta.validate(),
            Err(NativeRegistryError::MetaPackageRequiresPlatformSelections { .. })
        ));

        let mut incomplete_meta = record();
        incomplete_meta
            .publications
            .iter_mut()
            .find(|publication| publication.kind == NativePublicationKind::Meta)
            .unwrap()
            .platform_packages
            .pop();
        assert!(matches!(
            incomplete_meta.validate(),
            Err(NativeRegistryError::UnselectedPlatformPublication { .. })
        ));

        let mut platform_only = record();
        platform_only
            .publications
            .retain(|publication| publication.kind == NativePublicationKind::Platform);
        assert!(platform_only.validate().is_ok());
    }

    #[test]
    fn cargo_and_npm_names_use_conservative_portable_subsets() {
        assert!(validate_native_package_name(NativeRegistry::Cargo, "fiducia_core-sys").is_ok());
        assert!(validate_native_package_name(NativeRegistry::Cargo, "bad/name").is_err());
        assert!(
            validate_native_package_name(NativeRegistry::Npm, "@fiducia/core-linux-x64").is_ok()
        );
        assert!(validate_native_package_name(NativeRegistry::Npm, "@Fiducia/core").is_err());
        assert!(validate_native_package_name(NativeRegistry::Npm, "../core").is_err());
    }

    #[test]
    fn artifact_and_version_drift_are_rejected() {
        let mut invalid_artifact = record();
        invalid_artifact.publications[0].artifact.sha256 = "A".repeat(64);
        assert!(matches!(
            invalid_artifact.validate(),
            Err(NativeRegistryError::InvalidSha256 { .. })
        ));

        let mut version_drift = record();
        version_drift.publications[0].package.version = "1.2.4".to_string();
        assert!(matches!(
            version_drift.validate(),
            Err(NativeRegistryError::VersionDrift { .. })
        ));
    }
}
