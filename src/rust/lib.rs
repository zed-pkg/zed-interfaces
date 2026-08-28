//! Core interface definitions for the zed-pkg universal package manager.
//!
//! This crate is the single source of truth for the on-disk formats
//! (`.zpkg.toml`, `.zpkg.lock`, store layout), the registry REST API DTOs, and
//! the publish-time exclusion rules. It is consumed by `zed-cli`,
//! `zed-api-server`, `zed-web-server`, and (via generated JSON Schemas in
//! `schemas/`) by the non-Rust client libraries in `zed-clients`.

pub mod artifact;
pub mod binary_artifact;
pub mod dependency_graph;
pub mod dependency_graph_export;
pub mod environment;
pub mod environment_lock;
pub mod environment_v2;
pub mod excludes;
pub mod inspection;
pub mod language;
pub mod lockfile;
pub mod manifest;
pub mod mirror;
pub mod namespace_claim;
pub mod native_dependency;
pub mod native_host;
pub mod native_registry;
pub mod nix;
pub mod nix_plan;
pub mod oci;
pub mod paths;
pub mod registry;
pub mod registry_protocol_v1;
pub mod signing;
pub mod sync;
pub mod vcs;
pub mod version;

pub use artifact::ArtifactFormat;
pub use binary_artifact::{
    BINARY_ARCHIVE_ROOT, BINARY_ARTIFACT_LIST_SCHEMA_V1, BINARY_ARTIFACT_LOCK_SCHEMA_V1,
    BINARY_ARTIFACT_METADATA_SCHEMA_V1, BINARY_ARTIFACT_PUBLISH_META_SCHEMA_V1,
    BINARY_ARTIFACT_SCHEMA_V1, BINARY_DESCRIPTOR_ARCHIVE_PATH, BINARY_DESCRIPTOR_PATH,
    BINARY_PACKAGE_MANIFEST_ARCHIVE_PATH, BINARY_PACKAGE_MANIFEST_PATH, BinaryArchiveFormatV1,
    BinaryArtifactAttachmentKindV1, BinaryArtifactAttachmentV1, BinaryArtifactError,
    BinaryArtifactListResponseV1, BinaryArtifactLockV1, BinaryArtifactManifestV1,
    BinaryArtifactMetadataV1, BinaryArtifactPublishMetaV1, BinaryFileV1, BinaryPackageIdentityV1,
    BinaryPlatformV1, BinarySourceProvenanceV1,
};
pub use dependency_graph::{
    DEPENDENCY_GRAPH_DECLARED_ROUTE_TEMPLATE, DEPENDENCY_GRAPH_DEFAULT_MAX_EDGES,
    DEPENDENCY_GRAPH_DEFAULT_MAX_ENCODED_BYTES, DEPENDENCY_GRAPH_DEFAULT_MAX_NODES,
    DEPENDENCY_GRAPH_DEFAULT_MAX_PROJECTION_DEPTH, DEPENDENCY_GRAPH_DIGEST_HEADER,
    DEPENDENCY_GRAPH_JSON_MEDIA_TYPE, DEPENDENCY_GRAPH_RESOLUTION_ROUTE_TEMPLATE,
    DEPENDENCY_GRAPH_SCHEMA_V1, DEPENDENCY_GRAPH_TOML_MEDIA_TYPE, DEPENDENCY_GRAPH_YAML_MEDIA_TYPE,
    DeclaredDependency, DependencyGraphCompleteness, DependencyGraphData, DependencyGraphDocument,
    DependencyGraphError, DependencyGraphFormat, DependencyGraphProjection, DependencyKind,
    PackageVersionIdentity, RegistrySnapshot, ResolutionProvenance, ResolvedDependencyEdge,
    ResolvedDependencyNode, declared_dependency_graph_path, golden_fixture_documents,
    resolution_dependency_graph_path,
};
pub use dependency_graph_export::{
    DEPENDENCY_GRAPH_AUTHORITATIVE_HEADER, DEPENDENCY_GRAPH_CSV_MEDIA_TYPE,
    DEPENDENCY_GRAPH_EXPORT_ROUTE_TEMPLATE, DEPENDENCY_GRAPH_JSON5_MEDIA_TYPE,
    DEPENDENCY_GRAPH_MESSAGEPACK_MEDIA_TYPE, DEPENDENCY_GRAPH_PROTOBUF_MEDIA_TYPE,
    DEPENDENCY_GRAPH_XML_MEDIA_TYPE, DependencyGraphExportFormat,
    declared_dependency_graph_export_path,
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
    GitInteropSection, InstallHooksSection, InteropSection, Manifest, ManifestError,
    NATIVE_PACKAGE_MANAGERS, NativeDependencies, NixExportRoute, SigningSection,
};
pub use mirror::{
    DEFAULT_ARTIFACT_TEMPLATE, DEFAULT_ASSET_PREFIX, DEFAULT_CDN_ALTERNATE_URL, DEFAULT_CDN_URL,
    DEFAULT_INDEX_TAG, DEFAULT_INDEX_TEMPLATE, DEFAULT_RAW_BRANCH, DEFAULT_RAW_PREFIX,
    DEFAULT_VERSION_TEMPLATE, MAX_MIRRORS, MIRROR_BOOTSTRAP_PATH, MIRROR_BOOTSTRAP_SCHEMA_V1,
    MIRROR_DESCRIPTOR_SCHEMA_V1, MirrorBootstrapV1, MirrorCoordinateV1, MirrorDescriptorV1,
    MirrorError, MirrorKindV1, MirrorServesV1, RepoRefV1, default_public_mirrors,
    normalize_mirrors, parse_repo_ref,
};
pub use namespace_claim::{
    REGISTRY_NAMESPACE_PLAN_SCHEMA_V1, REGISTRY_NAMESPACE_RECEIPT_SCHEMA_V1,
    RegistryNamespaceAction, RegistryNamespaceAutomation, RegistryNamespaceClaimOutcome,
    RegistryNamespaceClaimReceipt, RegistryNamespaceDisposition, RegistryNamespaceEntry,
    RegistryNamespaceError, RegistryNamespaceEvidence, RegistryNamespaceModel,
    RegistryNamespacePlan, RegistryNamespaceProof, RegistryNamespaceProvider,
    RegistryNamespaceRequest, RegistryNamespaceStep,
};
pub use native_dependency::{
    NATIVE_DEPENDENCY_LOCK_SCHEMA_V1, NativeDependencyError, NativeDependencyLock,
    NativeVersionCandidate, NativeVersionRequirement,
};
pub use native_host::{
    ApiKeyHeader, ChannelRoute, HostEndpoints, NativeHost, NativeHostError, PrereleaseSyntax,
    RegistryAuth, RegistryProtocol, ReleaseChannel, UniversalHost,
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
pub use registry_protocol_v1::{
    REGISTRY_ARCHIVE_MANIFEST_SCHEMA_V1, REGISTRY_CHECKPOINT_SCHEMA_V1,
    REGISTRY_DISCOVERY_SCHEMA_V1, REGISTRY_INDEX_RECORD_SCHEMA_V1,
    REGISTRY_INDEX_SNAPSHOT_SCHEMA_V1, REGISTRY_PROTOCOL_ERROR_SCHEMA_V1, REGISTRY_PROTOCOL_V1,
    REGISTRY_PUBLISH_REQUEST_SCHEMA_V1, RegistryArchiveEntryKindV1, RegistryArchiveEntryV1,
    RegistryArchiveFormatV1, RegistryArchiveManifestV1, RegistryArchiveReferenceV1,
    RegistryAuthDescriptorV1, RegistryAuthModeV1, RegistryCapabilitiesV1, RegistryCheckpointV1,
    RegistryDependencyV1, RegistryDiscoveryV1, RegistryEndpointsV1, RegistryIndexRecordV1,
    RegistryIndexSnapshotEntryV1, RegistryIndexSnapshotV1, RegistryLifecycleStateV1,
    RegistryLimitsV1, RegistryProtocolErrorCodeV1, RegistryProtocolErrorV1,
    RegistryProtocolV1Error, RegistryPublishRequestV1, RegistryRootSignatureV1,
    RegistrySigningKeyStateV1, RegistrySigningKeyV1, RegistryVisibilityV1,
};
pub use signing::{
    DetachedSignatureV1, ED25519_PUBLIC_KEY_BYTES, ED25519_SIGNATURE_BYTES, IndexAttestationV1,
    IndexEntryV1, MAX_KEYS_PER_ORG, MAX_SIGNATURES, PUBLISHER_KEYS_SCHEMA_V1, PublisherKeySetV1,
    PublisherKeyStateV1, PublisherKeyV1, SIGNED_INDEX_SCHEMA_V1, SIGNED_VERSION_SCHEMA_V1,
    SIGNING_ALGORITHM, SignedIndexV1, SignedVersionV1, SigningError, VersionAttestationV1,
    decode_base64url, decode_multibase_base58btc, encode_base64url, encode_multibase_base58btc,
    index_attestation_preimage, version_attestation_preimage,
};
pub use vcs::Vcs;
pub use version::{Requirement, VersionScheme};
