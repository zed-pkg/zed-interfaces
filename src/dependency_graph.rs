//! Versioned, deterministic dependency-graph wire contract.
//!
//! The registry exposes two different facts that must not be conflated:
//! declared requirements attached to one immutable package version, and an
//! exact resolution produced for a target/features/registry-snapshot/lock
//! tuple. Both are represented by [`DependencyGraphDocument`], but the
//! internally tagged [`DependencyGraphData`] keeps their invariants distinct.

use std::collections::BTreeSet;
use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::registry::API_V1;

/// Semantic schema identifier carried by every graph document.
pub const DEPENDENCY_GRAPH_SCHEMA_V1: &str = "zpkg/dependency-graph/v1";
/// Response header carrying semantic graph identity across JSON/YAML/TOML.
pub const DEPENDENCY_GRAPH_DIGEST_HEADER: &str = "x-zpkg-graph-digest";
/// Canonical JSON representation media type.
pub const DEPENDENCY_GRAPH_JSON_MEDIA_TYPE: &str = "application/vnd.zpkg.dependency-graph.v1+json";
/// Lossless safe-subset YAML representation media type.
pub const DEPENDENCY_GRAPH_YAML_MEDIA_TYPE: &str = "application/vnd.zpkg.dependency-graph.v1+yaml";
/// Lossless normalized TOML representation media type.
pub const DEPENDENCY_GRAPH_TOML_MEDIA_TYPE: &str = "application/vnd.zpkg.dependency-graph.v1+toml";

fn dependency_graph_schema_v1() -> String {
    DEPENDENCY_GRAPH_SCHEMA_V1.to_string()
}

/// Download/serialization formats exposed by the API and CLI.
///
/// JSON, YAML and TOML are authoritative lossless projections of the typed
/// model. DOT and Mermaid are convenience renderings and must not be used as
/// interchange or digest inputs.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DependencyGraphFormat {
    Json,
    Yaml,
    Toml,
    Dot,
    Mermaid,
}

impl DependencyGraphFormat {
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Yaml => "yaml",
            Self::Toml => "toml",
            Self::Dot => "dot",
            Self::Mermaid => "mmd",
        }
    }

    pub const fn media_type(self) -> &'static str {
        match self {
            Self::Json => DEPENDENCY_GRAPH_JSON_MEDIA_TYPE,
            Self::Yaml => DEPENDENCY_GRAPH_YAML_MEDIA_TYPE,
            Self::Toml => DEPENDENCY_GRAPH_TOML_MEDIA_TYPE,
            Self::Dot => "text/vnd.graphviz; charset=utf-8",
            Self::Mermaid => "text/vnd.mermaid; charset=utf-8",
        }
    }

    pub const fn is_authoritative(self) -> bool {
        matches!(self, Self::Json | Self::Yaml | Self::Toml)
    }
}

/// Exact package identity. Registry identity is intrinsic, so remapping a
/// local registry alias cannot reinterpret a graph node.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub struct PackageVersionIdentity {
    pub registry_id: String,
    pub org: String,
    pub name: String,
    pub version: String,
}

impl fmt::Display for PackageVersionIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}::{}/{}@{}",
            self.registry_id, self.org, self.name, self.version
        )
    }
}

/// Dependency edge classification independent of any one language ecosystem.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DependencyKind {
    Runtime,
    Build,
    Development,
    Peer,
    Tooling,
}

/// One unresolved requirement from an immutable package manifest.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub struct DeclaredDependency {
    pub registry_id: String,
    pub org: String,
    pub name: String,
    pub requirement: String,
    pub kind: DependencyKind,
    #[serde(default)]
    pub optional: bool,
    #[serde(default = "default_true")]
    pub default_features: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

const fn default_true() -> bool {
    true
}

/// Whether a resolved graph is the whole resolution or an explicit filtered
/// projection with its own semantic digest.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DependencyGraphCompleteness {
    Complete,
    Projected,
}

/// A package selected by the resolver.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub struct ResolvedDependencyNode {
    pub id: PackageVersionIdentity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
}

/// A selected edge between exact package versions.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub struct ResolvedDependencyEdge {
    pub from: PackageVersionIdentity,
    pub to: PackageVersionIdentity,
    pub kind: DependencyKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requirement: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default)]
    pub optional: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
}

/// Immutable registry metadata checkpoint consulted by the resolver.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub struct RegistrySnapshot {
    pub registry_id: String,
    pub checkpoint_digest: String,
}

/// Inputs that make an exact resolution reproducible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResolutionProvenance {
    pub resolver_version: String,
    pub target: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enabled_features: Vec<String>,
    pub registry_snapshots: Vec<RegistrySnapshot>,
    pub lock_digest: String,
}

/// Canonical description of an explicit graph projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DependencyGraphProjection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kinds: Vec<DependencyKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_depth: Option<u32>,
}

impl DependencyGraphProjection {
    pub fn is_empty(&self) -> bool {
        self.target.is_none()
            && self.features.is_empty()
            && self.kinds.is_empty()
            && self.max_depth.is_none()
    }
}

/// View-specific graph body. The `view` tag is serialized at the document
/// root through [`DependencyGraphDocument::graph`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "view", rename_all = "snake_case")]
pub enum DependencyGraphData {
    Declared {
        package: PackageVersionIdentity,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        dependencies: Vec<DeclaredDependency>,
    },
    Resolved {
        completeness: DependencyGraphCompleteness,
        roots: Vec<PackageVersionIdentity>,
        nodes: Vec<ResolvedDependencyNode>,
        edges: Vec<ResolvedDependencyEdge>,
        provenance: ResolutionProvenance,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_graph_digest: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        projection: Option<DependencyGraphProjection>,
    },
}

/// Top-level dependency graph document.
///
/// `graph_digest` is `sha256:` plus lowercase hex over canonical JSON of the
/// normalized document with the `graph_digest` member omitted. A strong HTTP
/// ETag remains representation-specific and is therefore not this value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DependencyGraphDocument {
    #[serde(default = "dependency_graph_schema_v1")]
    pub schema: String,
    #[serde(flatten)]
    pub graph: DependencyGraphData,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph_digest: Option<String>,
}

impl DependencyGraphDocument {
    /// Sorts every set-like collection into its normative order and removes
    /// exact duplicates. This operation is idempotent.
    pub fn normalize_in_place(&mut self) {
        match &mut self.graph {
            DependencyGraphData::Declared { dependencies, .. } => {
                for dependency in dependencies.iter_mut() {
                    sort_dedup(&mut dependency.features);
                }
                dependencies.sort();
                dependencies.dedup();
            }
            DependencyGraphData::Resolved {
                roots,
                nodes,
                edges,
                provenance,
                projection,
                ..
            } => {
                roots.sort();
                roots.dedup();

                for node in nodes.iter_mut() {
                    sort_dedup(&mut node.features);
                }
                nodes.sort();

                for edge in edges.iter_mut() {
                    sort_dedup(&mut edge.features);
                }
                edges.sort();
                edges.dedup();

                sort_dedup(&mut provenance.enabled_features);
                provenance.registry_snapshots.sort();
                provenance.registry_snapshots.dedup();

                if let Some(projection) = projection {
                    sort_dedup(&mut projection.features);
                    projection.kinds.sort();
                    projection.kinds.dedup();
                }
            }
        }
    }

    /// Normalizes, validates and stamps a semantic digest.
    pub fn finalize(mut self) -> Result<Self, DependencyGraphError> {
        self.normalize_in_place();
        self.graph_digest = None;
        self.validate_structure()?;
        self.graph_digest = Some(self.compute_graph_digest()?);
        Ok(self)
    }

    /// Validates all structural invariants and verifies a present digest.
    pub fn validate(&self) -> Result<(), DependencyGraphError> {
        self.validate_structure()?;
        if let Some(actual) = &self.graph_digest {
            let expected = self.compute_graph_digest()?;
            if actual != &expected {
                return Err(DependencyGraphError::DigestMismatch {
                    expected,
                    actual: actual.clone(),
                });
            }
        }
        Ok(())
    }

    /// Requires and verifies `graph_digest`.
    pub fn verify_digest(&self) -> Result<(), DependencyGraphError> {
        if self.graph_digest.is_none() {
            return Err(DependencyGraphError::MissingGraphDigest);
        }
        self.validate()
    }

    /// Computes semantic graph identity from normalized canonical JSON.
    pub fn compute_graph_digest(&self) -> Result<String, DependencyGraphError> {
        let bytes = self.canonical_payload_bytes()?;
        let digest = Sha256::digest(bytes);
        Ok(format!("sha256:{}", hex::encode(digest)))
    }

    /// Canonical JSON bytes used as the semantic digest preimage. The
    /// `graph_digest` member is omitted to avoid self-reference.
    pub fn canonical_payload_bytes(&self) -> Result<Vec<u8>, DependencyGraphError> {
        let mut document = self.clone();
        document.normalize_in_place();
        document.graph_digest = None;
        document.validate_structure()?;
        canonical_json_bytes(&serde_json::to_value(document)?)
    }

    /// Canonical JSON bytes for storage/transport, including a present digest.
    pub fn canonical_document_bytes(&self) -> Result<Vec<u8>, DependencyGraphError> {
        let mut document = self.clone();
        document.normalize_in_place();
        document.validate()?;
        canonical_json_bytes(&serde_json::to_value(document)?)
    }

    fn validate_structure(&self) -> Result<(), DependencyGraphError> {
        if self.schema != DEPENDENCY_GRAPH_SCHEMA_V1 {
            return Err(DependencyGraphError::UnsupportedSchema(self.schema.clone()));
        }

        if let Some(digest) = &self.graph_digest {
            validate_sha256_digest("graph_digest", digest)?;
        }

        match &self.graph {
            DependencyGraphData::Declared {
                package,
                dependencies,
            } => {
                validate_identity(package)?;
                for dependency in dependencies {
                    validate_non_empty("declared dependency registry_id", &dependency.registry_id)?;
                    validate_non_empty("declared dependency org", &dependency.org)?;
                    validate_non_empty("declared dependency name", &dependency.name)?;
                    validate_non_empty("declared dependency requirement", &dependency.requirement)?;
                    for feature in &dependency.features {
                        validate_non_empty("declared dependency feature", feature)?;
                    }
                    if let Some(target) = &dependency.target {
                        validate_non_empty("declared dependency target", target)?;
                    }
                }
            }
            DependencyGraphData::Resolved {
                completeness,
                roots,
                nodes,
                edges,
                provenance,
                parent_graph_digest,
                projection,
            } => {
                if roots.is_empty() {
                    return Err(DependencyGraphError::EmptyResolvedGraph("roots"));
                }
                if nodes.is_empty() {
                    return Err(DependencyGraphError::EmptyResolvedGraph("nodes"));
                }

                let mut node_ids = BTreeSet::new();
                for node in nodes {
                    validate_identity(&node.id)?;
                    if let Some(digest) = &node.artifact_digest {
                        validate_sha256_digest("artifact_digest", digest)?;
                    }
                    for feature in &node.features {
                        validate_non_empty("resolved node feature", feature)?;
                    }
                    if !node_ids.insert(node.id.clone()) {
                        return Err(DependencyGraphError::DuplicateNode(node.id.to_string()));
                    }
                }

                for root in roots {
                    validate_identity(root)?;
                    if !node_ids.contains(root) {
                        return Err(DependencyGraphError::MissingRootNode(root.to_string()));
                    }
                }

                for edge in edges {
                    validate_identity(&edge.from)?;
                    validate_identity(&edge.to)?;
                    if !node_ids.contains(&edge.from) {
                        return Err(DependencyGraphError::MissingEdgeEndpoint(
                            edge.from.to_string(),
                        ));
                    }
                    if !node_ids.contains(&edge.to) {
                        return Err(DependencyGraphError::MissingEdgeEndpoint(
                            edge.to.to_string(),
                        ));
                    }
                    if let Some(requirement) = &edge.requirement {
                        validate_non_empty("resolved edge requirement", requirement)?;
                    }
                    if let Some(target) = &edge.target {
                        validate_non_empty("resolved edge target", target)?;
                    }
                    for feature in &edge.features {
                        validate_non_empty("resolved edge feature", feature)?;
                    }
                }

                validate_non_empty("resolver_version", &provenance.resolver_version)?;
                validate_non_empty("resolution target", &provenance.target)?;
                validate_sha256_digest("lock_digest", &provenance.lock_digest)?;
                if provenance.registry_snapshots.is_empty() {
                    return Err(DependencyGraphError::EmptyRegistrySnapshots);
                }
                let mut registry_ids = BTreeSet::new();
                for snapshot in &provenance.registry_snapshots {
                    validate_non_empty("registry snapshot registry_id", &snapshot.registry_id)?;
                    validate_sha256_digest(
                        "registry checkpoint_digest",
                        &snapshot.checkpoint_digest,
                    )?;
                    if !registry_ids.insert(&snapshot.registry_id) {
                        return Err(DependencyGraphError::DuplicateRegistrySnapshot(
                            snapshot.registry_id.clone(),
                        ));
                    }
                }

                match completeness {
                    DependencyGraphCompleteness::Complete => {
                        if parent_graph_digest.is_some() || projection.is_some() {
                            return Err(DependencyGraphError::UnexpectedProjectionMetadata);
                        }
                    }
                    DependencyGraphCompleteness::Projected => {
                        let parent = parent_graph_digest
                            .as_deref()
                            .ok_or(DependencyGraphError::MissingProjectionMetadata)?;
                        validate_sha256_digest("parent_graph_digest", parent)?;
                        let projection = projection
                            .as_ref()
                            .ok_or(DependencyGraphError::MissingProjectionMetadata)?;
                        if projection.is_empty() {
                            return Err(DependencyGraphError::EmptyProjection);
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

/// `GET`/`HEAD` path for immutable declared requirements.
pub fn declared_dependency_graph_path(org: &str, name: &str, version: &str) -> String {
    format!("{API_V1}/packages/{org}/{name}/versions/{version}/dependency-graph?view=declared")
}

/// `GET`/`HEAD` path for an immutable exact resolution artifact.
pub fn resolution_dependency_graph_path(resolution_digest: &str) -> String {
    format!("{API_V1}/resolutions/{resolution_digest}/dependency-graph")
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DependencyGraphError {
    #[error("unsupported dependency graph schema: {0}")]
    UnsupportedSchema(String),
    #[error("dependency graph field must not be empty: {0}")]
    EmptyField(&'static str),
    #[error("resolved dependency graph must contain at least one {0}")]
    EmptyResolvedGraph(&'static str),
    #[error("duplicate resolved node: {0}")]
    DuplicateNode(String),
    #[error("root does not reference a graph node: {0}")]
    MissingRootNode(String),
    #[error("edge endpoint does not reference a graph node: {0}")]
    MissingEdgeEndpoint(String),
    #[error("resolved graph must record at least one immutable registry snapshot")]
    EmptyRegistrySnapshots,
    #[error("duplicate registry snapshot identity: {0}")]
    DuplicateRegistrySnapshot(String),
    #[error("projected graph requires parent_graph_digest and a projection spec")]
    MissingProjectionMetadata,
    #[error("complete graph must not carry projection metadata")]
    UnexpectedProjectionMetadata,
    #[error("projected graph must specify at least one filter")]
    EmptyProjection,
    #[error("{field} must be canonical sha256:<64 lowercase hex>, got {value}")]
    InvalidDigest { field: &'static str, value: String },
    #[error("dependency graph does not contain graph_digest")]
    MissingGraphDigest,
    #[error("dependency graph digest mismatch: expected {expected}, got {actual}")]
    DigestMismatch { expected: String, actual: String },
    #[error("canonical dependency graph JSON forbids non-integer number: {0}")]
    UnsupportedJsonNumber(String),
    #[error("dependency graph JSON serialization failed: {0}")]
    Json(String),
}

impl From<serde_json::Error> for DependencyGraphError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error.to_string())
    }
}

fn sort_dedup<T: Ord>(values: &mut Vec<T>) {
    values.sort();
    values.dedup();
}

fn validate_identity(identity: &PackageVersionIdentity) -> Result<(), DependencyGraphError> {
    validate_non_empty("package registry_id", &identity.registry_id)?;
    validate_non_empty("package org", &identity.org)?;
    validate_non_empty("package name", &identity.name)?;
    validate_non_empty("package version", &identity.version)
}

fn validate_non_empty(field: &'static str, value: &str) -> Result<(), DependencyGraphError> {
    if value.trim().is_empty() {
        return Err(DependencyGraphError::EmptyField(field));
    }
    Ok(())
}

fn validate_sha256_digest(field: &'static str, value: &str) -> Result<(), DependencyGraphError> {
    let valid = value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    });
    if !valid {
        return Err(DependencyGraphError::InvalidDigest {
            field,
            value: value.to_string(),
        });
    }
    Ok(())
}

fn canonical_json_bytes(value: &Value) -> Result<Vec<u8>, DependencyGraphError> {
    let mut bytes = Vec::new();
    write_canonical_json(value, &mut bytes)?;
    Ok(bytes)
}

fn write_canonical_json(value: &Value, bytes: &mut Vec<u8>) -> Result<(), DependencyGraphError> {
    match value {
        Value::Null => bytes.extend_from_slice(b"null"),
        Value::Bool(value) => bytes.extend_from_slice(if *value { b"true" } else { b"false" }),
        Value::Number(number) => {
            if !number.is_i64() && !number.is_u64() {
                return Err(DependencyGraphError::UnsupportedJsonNumber(
                    number.to_string(),
                ));
            }
            bytes.extend_from_slice(number.to_string().as_bytes());
        }
        Value::String(value) => bytes.extend_from_slice(serde_json::to_string(value)?.as_bytes()),
        Value::Array(values) => {
            bytes.push(b'[');
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    bytes.push(b',');
                }
                write_canonical_json(value, bytes)?;
            }
            bytes.push(b']');
        }
        Value::Object(values) => {
            bytes.push(b'{');
            let mut keys: Vec<_> = values.keys().collect();
            keys.sort_unstable();
            for (index, key) in keys.into_iter().enumerate() {
                if index != 0 {
                    bytes.push(b',');
                }
                bytes.extend_from_slice(serde_json::to_string(key)?.as_bytes());
                bytes.push(b':');
                write_canonical_json(&values[key], bytes)?;
            }
            bytes.push(b'}');
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    fn package(name: &str, version: &str) -> PackageVersionIdentity {
        PackageVersionIdentity {
            registry_id: "registry:test".into(),
            org: "acme".into(),
            name: name.into(),
            version: version.into(),
        }
    }

    fn sample_resolved_graph() -> DependencyGraphDocument {
        let root = package("app", "1.0.0");
        let library = package("library", "2.1.0");
        DependencyGraphDocument {
            schema: DEPENDENCY_GRAPH_SCHEMA_V1.into(),
            graph: DependencyGraphData::Resolved {
                completeness: DependencyGraphCompleteness::Complete,
                roots: vec![root.clone()],
                nodes: vec![
                    ResolvedDependencyNode {
                        id: library.clone(),
                        artifact_digest: Some(digest('b')),
                        features: vec!["tls".into(), "serde".into(), "tls".into()],
                    },
                    ResolvedDependencyNode {
                        id: root.clone(),
                        artifact_digest: Some(digest('a')),
                        features: vec!["default".into()],
                    },
                ],
                edges: vec![ResolvedDependencyEdge {
                    from: root,
                    to: library,
                    kind: DependencyKind::Runtime,
                    requirement: Some("^2.0".into()),
                    target: Some("cfg(unix)".into()),
                    optional: false,
                    features: vec!["tls".into(), "serde".into()],
                }],
                provenance: ResolutionProvenance {
                    resolver_version: "zed-resolver/1.0.0".into(),
                    target: "x86_64-unknown-linux-gnu".into(),
                    enabled_features: vec!["server".into(), "server".into()],
                    registry_snapshots: vec![RegistrySnapshot {
                        registry_id: "registry:test".into(),
                        checkpoint_digest: digest('c'),
                    }],
                    lock_digest: digest('d'),
                },
                parent_graph_digest: None,
                projection: None,
            },
            graph_digest: None,
        }
    }

    #[test]
    fn digest_is_deterministic_across_input_order() {
        let first = sample_resolved_graph().finalize().unwrap();
        let mut shuffled = sample_resolved_graph();
        let DependencyGraphData::Resolved {
            roots,
            nodes,
            edges,
            provenance,
            ..
        } = &mut shuffled.graph
        else {
            unreachable!();
        };
        roots.reverse();
        nodes.reverse();
        edges.reverse();
        provenance.enabled_features.reverse();
        provenance.registry_snapshots.reverse();
        let second = shuffled.finalize().unwrap();

        assert_eq!(first.graph_digest, second.graph_digest);
        assert_eq!(
            first.canonical_document_bytes().unwrap(),
            second.canonical_document_bytes().unwrap()
        );
    }

    #[test]
    fn finalized_graph_round_trips_and_verifies() {
        let graph = sample_resolved_graph().finalize().unwrap();
        let encoded = serde_json::to_vec(&graph).unwrap();
        let decoded: DependencyGraphDocument = serde_json::from_slice(&encoded).unwrap();
        decoded.verify_digest().unwrap();
    }

    #[test]
    fn declared_requirements_are_not_forced_into_exact_nodes() {
        let graph = DependencyGraphDocument {
            schema: DEPENDENCY_GRAPH_SCHEMA_V1.into(),
            graph: DependencyGraphData::Declared {
                package: package("app", "1.0.0"),
                dependencies: vec![DeclaredDependency {
                    registry_id: "registry:test".into(),
                    org: "acme".into(),
                    name: "library".into(),
                    requirement: "^2".into(),
                    kind: DependencyKind::Runtime,
                    optional: false,
                    default_features: true,
                    features: vec!["tls".into()],
                    target: Some("cfg(unix)".into()),
                }],
            },
            graph_digest: None,
        }
        .finalize()
        .unwrap();

        graph.verify_digest().unwrap();
        let json = String::from_utf8(graph.canonical_document_bytes().unwrap()).unwrap();
        assert!(json.contains("\"view\":\"declared\""));
        assert!(json.contains("\"requirement\":\"^2\""));
    }

    #[test]
    fn missing_edge_endpoint_is_rejected() {
        let mut graph = sample_resolved_graph();
        let DependencyGraphData::Resolved { edges, .. } = &mut graph.graph else {
            unreachable!();
        };
        edges[0].to = package("missing", "9.9.9");

        assert!(matches!(
            graph.finalize(),
            Err(DependencyGraphError::MissingEdgeEndpoint(_))
        ));
    }

    #[test]
    fn projected_graph_requires_parent_and_non_empty_spec() {
        let mut graph = sample_resolved_graph();
        let DependencyGraphData::Resolved {
            completeness,
            projection,
            ..
        } = &mut graph.graph
        else {
            unreachable!();
        };
        *completeness = DependencyGraphCompleteness::Projected;
        *projection = Some(DependencyGraphProjection {
            target: None,
            features: Vec::new(),
            kinds: Vec::new(),
            max_depth: None,
        });

        assert_eq!(
            graph.finalize().unwrap_err(),
            DependencyGraphError::MissingProjectionMetadata
        );
    }

    #[test]
    fn format_and_route_contract_is_stable() {
        assert_eq!(DependencyGraphFormat::Json.extension(), "json");
        assert_eq!(
            DependencyGraphFormat::Toml.media_type(),
            DEPENDENCY_GRAPH_TOML_MEDIA_TYPE
        );
        assert!(!DependencyGraphFormat::Dot.is_authoritative());
        assert_eq!(
            declared_dependency_graph_path("acme", "app", "1.0.0"),
            "/v1/packages/acme/app/versions/1.0.0/dependency-graph?view=declared"
        );
        assert_eq!(
            resolution_dependency_graph_path("sha256:abc"),
            "/v1/resolutions/sha256:abc/dependency-graph"
        );
    }
}
