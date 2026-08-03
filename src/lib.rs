//! Core interface definitions for the zed-pkg universal package manager.
//!
//! This crate is the single source of truth for the on-disk formats
//! (`.zpkg.toml`, `.zpkg.lock`, store layout), the registry REST API DTOs, and
//! the publish-time exclusion rules. It is consumed by `zed-cli`,
//! `zed-api-server`, `zed-web-server`, and (via generated JSON Schemas in
//! `schemas/`) by the non-Rust client libraries in `zed-clients`.

pub mod artifact;
pub mod environment;
pub mod excludes;
pub mod language;
pub mod lockfile;
pub mod manifest;
pub mod nix;
pub mod oci;
pub mod paths;
pub mod registry;
pub mod sync;
pub mod vcs;
pub mod version;

pub use artifact::ArtifactFormat;
pub use environment::{
    ActivationPolicy, Checksum, ChecksumAlgorithm, EnvironmentManager, EnvironmentPlan,
    EnvironmentPlanError, EnvironmentSource, EnvironmentValidationMode, ImmutableSource,
    SystemPackageRequirement, ToolRequirement, differ_only_in_build_metadata,
    validate_semver_export,
};
pub use language::{Ecosystem, Language, detect_ecosystems};
pub use lockfile::{LockedPackage, Lockfile, LockfileError};
pub use manifest::{Manifest, ManifestError, NixExportRoute};
pub use nix::{
    NIX_ADAPTER_SCHEMA_V1, NixAdapterRecord, NixBuilderNetwork, NixExportMode, NixExportSection,
    NixInteropArtifact, NixInteropError, NixOutputOrigin, NixPackageIdentity, NixPolicyEvidence,
    NixPolicyProfile, NixRealizedOutput, NixStoreReference, ZedArtifactOrigin,
};
pub use oci::{
    CYCLONEDX_JSON_MEDIA_TYPE, IN_TOTO_JSON_MEDIA_TYPE, OCI_ADAPTER_SCHEMA_V1,
    OCI_IMAGE_MANIFEST_MEDIA_TYPE, OciAdapterRecord, OciDescriptor, OciDigest, OciInteropError,
    OciLayer, OciLayerKind, OciPackageIdentity, OciPlatform, OciReference, SPDX_JSON_MEDIA_TYPE,
    ZED_OCI_BINARY_MEDIA_TYPE_V1, ZED_OCI_CONFIG_MEDIA_TYPE_V1,
    ZED_OCI_LOCK_MEDIA_TYPE_V1, ZED_OCI_MANIFEST_MEDIA_TYPE_V1,
    ZED_OCI_PACKAGE_TAR_GZ_MEDIA_TYPE_V1, ZED_OCI_PACKAGE_ZIP_MEDIA_TYPE_V1,
};
pub use vcs::Vcs;
pub use version::{Requirement, VersionScheme};
