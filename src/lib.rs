//! Core interface definitions for the zed-pkg universal package manager.
//!
//! This crate is the single source of truth for the on-disk formats
//! (`.zpkg.toml`, `.zpkg.lock`, store layout), the registry REST API DTOs, and
//! the publish-time exclusion rules. It is consumed by `zed-cli`,
//! `zed-api-server`, `zed-web-server`, and (via generated JSON Schemas in
//! `schemas/`) by the non-Rust client libraries in `zed-clients`.

pub mod artifact;
pub mod dependency_graph;
pub mod environment;
pub mod environment_lock;
pub mod environment_v2;
pub mod excludes;
pub mod language;
pub mod lockfile;
pub mod manifest;
pub mod native_dependency;
pub mod native_registry;
pub mod nix;
pub mod nix_plan;
pub mod oci;
pub mod paths;
pub mod registry;
pub mod sync;
pub mod vcs;
pub mod version;

pub use artifact::ArtifactFormat;
pub use dependency_graph::{
    DEPENDENCY_GRAPH_DIGEST_HEADER, DEPENDENCY_GRAPH_JSON_MEDIA_TYPE, DEPENDENCY_GRAPH_SCHEMA_V1,
    DEPENDENCY_GRAPH_TOML_MEDIA_TYPE, DEPENDENCY_GRAPH_YAML_MEDIA_TYPE, DeclaredDependency,
    DependencyGraphCompleteness, DependencyGraphData, DependencyGraphDocument,
    DependencyGraphError, DependencyGraphFormat, DependencyGraphProjection, DependencyKind,
    PackageVersionIdentity, RegistrySnapshot, ResolutionProvenance, ResolvedDependencyEdge,
    ResolvedDependencyNode, declared_dependency_graph_path, resolution_dependency_graph_path,
};
pub use environment::{
    ActivationPolicy, Checksum, ChecksumAlgorithm, EnvironmentManager, EnvironmentPlan,
    EnvironmentPlanError, EnvironmentSource, EnvironmentValidationMode, ImmutableSource,
    SystemPackageRequirement, ToolRequirement, differ_only_in_build_metadata,
    validate_semver_export,
};
pub use environment_lock::{
    ENVIRONMENT_LOCK_SCHEMA_VERSION, EnvironmentLock, EnvironmentLockError,
    EnvironmentLockValidationMode, LockedArtifact, LockedArtifactFormat, LockedExecutable,
    LockedInstall, LockedPlatform, LockedSignature, LockedSource, LockedSourceKind, LockedTool,
};
pub use environment_v2::{
    EnvironmentPlanV2, EnvironmentPlanV2Error, EnvironmentValue, SystemPackageSpec,
    TaskConfirmation, TaskGroup, TaskInvocation, TaskSpec, TaskStep, ToolSpec, ToolVersion,
};
pub use language::{Ecosystem, Language, detect_ecosystems};
pub use lockfile::{LockedPackage, Lockfile, LockfileError};
pub use manifest::{
    InstallHooksSection, Manifest, ManifestError, NATIVE_PACKAGE_MANAGERS, NativeDependencies,
    NixExportRoute,
};
pub use native_dependency::{
    NATIVE_DEPENDENCY_LOCK_SCHEMA_V1, NativeDependencyError, NativeDependencyLock,
    NativeVersionCandidate, NativeVersionRequirement,
};
pub use native_registry::{
    NATIVE_REGISTRY_ADAPTER_SCHEMA_V1, NativeArtifact, NativePackageIdentity, NativePlatform,
    NativePlatformPackage, NativePublication, NativePublicationKind, NativeRegistry,
    NativeRegistryAdapterRecord, NativeRegistryError, ZedNativePackageIdentity,
    native_versions_collide, semver_precedence_identity,
};
pub use nix::{
    NIX_ADAPTER_SCHEMA_V1, NixAdapterRecord, NixBuilderNetwork, NixExportMode, NixExportSection,
    NixInteropArtifact, NixInteropError, NixOutputOrigin, NixPackageIdentity, NixPolicyEvidence,
    NixPolicyProfile, NixRealizedOutput, NixStoreReference, ZedArtifactOrigin,
};
pub use nix_plan::{
    NIX_EXPORT_PLAN_SCHEMA_V1, NixExportPackageClass, NixExportPlan, NixExportPlanError,
    PlannedNixExportDependency, PlannedZedExportArtifact, ResolvedNixExportIntent,
};
pub use oci::{
    CYCLONEDX_JSON_MEDIA_TYPE, IN_TOTO_JSON_MEDIA_TYPE, OCI_ADAPTER_SCHEMA_V1,
    OCI_IMAGE_MANIFEST_MEDIA_TYPE, OciAdapterRecord, OciDescriptor, OciDigest, OciInteropError,
    OciLayer, OciLayerKind, OciPackageIdentity, OciPlatform, OciReference, SPDX_JSON_MEDIA_TYPE,
    ZED_OCI_BINARY_MEDIA_TYPE_V1, ZED_OCI_CONFIG_MEDIA_TYPE_V1, ZED_OCI_LOCK_MEDIA_TYPE_V1,
    ZED_OCI_MANIFEST_MEDIA_TYPE_V1, ZED_OCI_PACKAGE_TAR_GZ_MEDIA_TYPE_V1,
    ZED_OCI_PACKAGE_ZIP_MEDIA_TYPE_V1,
};
pub use vcs::Vcs;
pub use version::{Requirement, VersionScheme};
