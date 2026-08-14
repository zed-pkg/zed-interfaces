//! Regenerates the JSON Schemas under `schemas/`. They are the source of
//! truth for every non-Rust consumer: `codegen/generate.mjs` turns the
//! front-end-facing subset (see `schemas/index.json`) into `src/dart/` and
//! `src/ts/`, and the client libraries in `zed-clients` codegen/validate
//! against the same files.
//!
//! Run with: `cargo run --example generate_schemas`

use std::fs;
use std::path::Path;

use schemars::{JsonSchema, schema_for};
use serde_json::Value;

fn write<T: JsonSchema>(dir: &Path, name: &str) {
    let schema = schema_for!(T);
    let json = serde_json::to_string_pretty(&schema).expect("schema serializes");
    let path = dir.join(format!("{name}.json"));
    fs::write(&path, json + "\n").expect("schema file writes");
    println!("wrote {}", path.display());
}

/// The Rust validator enforces the binary descriptor's exact schema marker,
/// sibling manifest location, digest alphabet, platform tokens, and safe path
/// grammar. Schemars cannot infer those cross-format refinements from ordinary
/// `String` fields, so pin them in the generated wire schema as well.
fn write_binary_artifact_schema(dir: &Path) {
    let schema = schema_for!(zed_interfaces::BinaryArtifactManifestV1);
    let mut value = serde_json::to_value(schema).expect("schema serializes");
    reject_explicit_nulls(&mut value);
    pin_binary_artifact_contract(&mut value);
    let json = serde_json::to_string_pretty(&value).expect("schema serializes");
    let path = dir.join("binary-artifact-v1.json");
    fs::write(&path, json + "\n").expect("schema file writes");
    println!("wrote {}", path.display());
}

/// Generate a binary API/lock schema with the same canonical optional-member
/// rule as the in-archive descriptor. Rust `Option<T>` accepts JSON null for
/// ergonomic deserialization, but signed/canonical binary records have one
/// spelling for absence: the member is omitted.
fn write_versioned_binary_schema<T: JsonSchema>(dir: &Path, name: &str, schema_id: &str) {
    let schema = schema_for!(T);
    let mut value = serde_json::to_value(schema).expect("schema serializes");
    reject_explicit_nulls(&mut value);
    pin_versioned_binary_schema(&mut value, schema_id);
    let json = serde_json::to_string_pretty(&value).expect("schema serializes");
    let path = dir.join(format!("{name}.json"));
    fs::write(&path, json + "\n").expect("schema file writes");
    println!("wrote {}", path.display());
}

fn pin_versioned_binary_schema(value: &mut Value, schema_id: &str) {
    let root = value
        .as_object_mut()
        .expect("versioned binary schema is an object");
    let properties = root
        .get_mut("properties")
        .and_then(Value::as_object_mut)
        .expect("versioned binary schema properties exist");
    properties
        .get_mut("schema")
        .and_then(Value::as_object_mut)
        .expect("versioned binary schema marker exists")
        .insert("const".to_owned(), Value::String(schema_id.to_owned()));
    pin_flat_binary_release_identity(properties);

    if let Some(definitions) = root.get_mut("$defs").and_then(Value::as_object_mut) {
        if let Some(metadata) = definitions
            .get_mut("BinaryArtifactMetadataV1")
            .and_then(Value::as_object_mut)
            .and_then(|definition| definition.get_mut("properties"))
            .and_then(Value::as_object_mut)
        {
            metadata
                .get_mut("schema")
                .and_then(Value::as_object_mut)
                .expect("embedded metadata schema marker exists")
                .insert(
                    "const".to_owned(),
                    Value::String(zed_interfaces::BINARY_ARTIFACT_METADATA_SCHEMA_V1.to_owned()),
                );
            pin_flat_binary_release_identity(metadata);
        }
        pin_binary_common_definitions(definitions);
    }
    pin_binary_digest_members(value);
}

fn pin_flat_binary_release_identity(properties: &mut serde_json::Map<String, Value>) {
    if !["org", "name", "version"]
        .into_iter()
        .all(|field| properties.contains_key(field))
    {
        return;
    }
    for field in ["org", "name"] {
        let property = properties
            .get_mut(field)
            .and_then(Value::as_object_mut)
            .expect("flat release coordinate exists");
        property.insert("minLength".to_owned(), Value::from(1));
        property.insert(
            "pattern".to_owned(),
            Value::String(r"^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$".to_owned()),
        );
    }
    let version = properties
        .get_mut("version")
        .and_then(Value::as_object_mut)
        .expect("flat release version exists");
    version.insert("minLength".to_owned(), Value::from(1));
    version.insert(
        "pattern".to_owned(),
        Value::String(r"^[^\u0000-\u0020\u007f]+$".to_owned()),
    );
}

fn pin_binary_common_definitions(definitions: &mut serde_json::Map<String, Value>) {
    const PLATFORM_PATTERN: &str = "^[a-z0-9](?:[a-z0-9._-]*[a-z0-9])?$";
    if let Some(platform) = definitions
        .get_mut("BinaryPlatformV1")
        .and_then(|definition| definition.get_mut("properties"))
        .and_then(Value::as_object_mut)
    {
        for field in ["target", "os", "arch", "libc", "abi"] {
            let property = platform
                .get_mut(field)
                .and_then(Value::as_object_mut)
                .expect("binary platform field exists");
            property.insert("maxLength".to_owned(), Value::from(128));
            property.insert("minLength".to_owned(), Value::from(1));
            property.insert(
                "pattern".to_owned(),
                Value::String(PLATFORM_PATTERN.to_owned()),
            );
        }
    }

    if let Some(package) = definitions
        .get_mut("BinaryPackageIdentityV1")
        .and_then(|definition| definition.get_mut("properties"))
        .and_then(Value::as_object_mut)
    {
        pin_flat_binary_release_identity(package);
    }

    if let Some(source) = definitions
        .get_mut("BinarySourceProvenanceV1")
        .and_then(|definition| definition.get_mut("properties"))
        .and_then(Value::as_object_mut)
    {
        for field in ["repository", "vcs_tag"] {
            source
                .get_mut(field)
                .and_then(Value::as_object_mut)
                .expect("binary source field exists")
                .insert("minLength".to_owned(), Value::from(1));
        }
        let commit = source
            .get_mut("vcs_commit")
            .and_then(Value::as_object_mut)
            .expect("binary source commit exists");
        commit.insert("maxLength".to_owned(), Value::from(128));
        commit.insert("minLength".to_owned(), Value::from(7));
        commit.insert(
            "pattern".to_owned(),
            Value::String(r"^[A-Za-z0-9._+:/-]+$".to_owned()),
        );
    }
}

fn pin_binary_digest_members(value: &mut Value) {
    const DIGEST_PATTERN: &str = "^[0-9a-f]{64}$";
    match value {
        Value::Array(values) => {
            for value in values {
                pin_binary_digest_members(value);
            }
        }
        Value::Object(object) => {
            if let Some(Value::Object(properties)) = object.get_mut("properties") {
                let immutable_artifact = ["sha256", "size", "descriptor_sha256"]
                    .into_iter()
                    .all(|field| properties.contains_key(field));
                let attachment = ["sha256", "size", "subject_sha256"]
                    .into_iter()
                    .all(|field| properties.contains_key(field));
                if (immutable_artifact || attachment)
                    && let Some(Value::Object(size)) = properties.get_mut("size")
                {
                    size.insert("minimum".to_owned(), Value::from(1));
                }
                for (name, property) in properties {
                    if matches!(
                        name.as_str(),
                        "sha256" | "descriptor_sha256" | "subject_sha256"
                    ) && let Value::Object(property) = property
                    {
                        property.insert("maxLength".to_owned(), Value::from(64));
                        property.insert("minLength".to_owned(), Value::from(64));
                        property.insert(
                            "pattern".to_owned(),
                            Value::String(DIGEST_PATTERN.to_owned()),
                        );
                    }
                }
            }
            for value in object.values_mut() {
                pin_binary_digest_members(value);
            }
        }
        _ => {}
    }
}

fn pin_binary_artifact_contract(value: &mut Value) {
    const DIGEST_PATTERN: &str = "^[0-9a-f]{64}$";
    const PLATFORM_PATTERN: &str = "^[a-z0-9](?:[a-z0-9._-]*[a-z0-9])?$";
    // JSON Schema patterns count Unicode code points rather than UTF-8 bytes,
    // so this mirrors the component grammar and catches the 255-character
    // ASCII case. The Rust validator remains authoritative for the stricter
    // 255-byte ceiling on non-ASCII paths and for cross-entry lowercase-key
    // collision detection.
    const PATH_PATTERN: &str = r#"^(?!\.{1,2}(?:/|$))(?!.*\/\.{1,2}(?:/|$))(?!.*[. ](?:/|$))(?!.*(?:^|/)(?:[Cc][Oo][Nn]|[Pp][Rr][Nn]|[Aa][Uu][Xx]|[Nn][Uu][Ll]|[Cc][Ll][Oo][Cc][Kk]\$|[Cc][Oo][Mm][1-9]|[Ll][Pp][Tt][1-9])(?:\.|/|$))[^/\\:<>"|?*\u0000-\u001f\u007f]{1,255}(?:/[^/\\:<>"|?*\u0000-\u001f\u007f]{1,255})*$"#;
    const COMMAND_PATTERN: &str = r"^(?!\.)[A-Za-z0-9._+\-]{1,128}$";
    const VERSION_PATTERN: &str = r"^[^\u0000-\u0020\u007f]+$";

    let root = value.as_object_mut().expect("binary schema is an object");
    let properties = root
        .get_mut("properties")
        .and_then(Value::as_object_mut)
        .expect("binary schema has properties");
    properties
        .get_mut("schema")
        .and_then(Value::as_object_mut)
        .expect("binary schema marker exists")
        .insert(
            "const".to_owned(),
            Value::String(zed_interfaces::BINARY_ARTIFACT_SCHEMA_V1.to_owned()),
        );
    properties
        .get_mut("package_manifest")
        .and_then(Value::as_object_mut)
        .expect("binary package_manifest exists")
        .insert(
            "const".to_owned(),
            Value::String(zed_interfaces::BINARY_PACKAGE_MANIFEST_PATH.to_owned()),
        );
    properties
        .get_mut("files")
        .and_then(Value::as_object_mut)
        .expect("binary files exists")
        .insert("minItems".to_owned(), Value::from(2));
    let entrypoints = properties
        .get_mut("entrypoints")
        .and_then(Value::as_object_mut)
        .expect("binary entrypoints exists");
    entrypoints.insert("minProperties".to_owned(), Value::from(1));
    entrypoints.insert(
        "propertyNames".to_owned(),
        serde_json::json!({ "pattern": COMMAND_PATTERN }),
    );
    let entrypoint_path = entrypoints
        .get_mut("additionalProperties")
        .and_then(Value::as_object_mut)
        .expect("entrypoint values have a schema");
    entrypoint_path.insert("maxLength".to_owned(), Value::from(4096));
    entrypoint_path.insert("minLength".to_owned(), Value::from(1));
    entrypoint_path.insert("pattern".to_owned(), Value::String(PATH_PATTERN.to_owned()));

    let definitions = root
        .get_mut("$defs")
        .and_then(Value::as_object_mut)
        .expect("binary schema has definitions");
    let file_properties = definitions
        .get_mut("BinaryFileV1")
        .and_then(|value| value.get_mut("properties"))
        .and_then(Value::as_object_mut)
        .expect("BinaryFileV1 properties exist");
    let file_path = file_properties
        .get_mut("path")
        .and_then(Value::as_object_mut)
        .expect("BinaryFileV1.path exists");
    file_path.insert("maxLength".to_owned(), Value::from(4096));
    file_path.insert("minLength".to_owned(), Value::from(1));
    file_path.insert("pattern".to_owned(), Value::String(PATH_PATTERN.to_owned()));
    let file_digest = file_properties
        .get_mut("sha256")
        .and_then(Value::as_object_mut)
        .expect("BinaryFileV1.sha256 exists");
    file_digest.insert("maxLength".to_owned(), Value::from(64));
    file_digest.insert("minLength".to_owned(), Value::from(64));
    file_digest.insert(
        "pattern".to_owned(),
        Value::String(DIGEST_PATTERN.to_owned()),
    );

    let package_properties = definitions
        .get_mut("BinaryPackageIdentityV1")
        .and_then(|value| value.get_mut("properties"))
        .and_then(Value::as_object_mut)
        .expect("BinaryPackageIdentityV1 properties exist");
    for field in ["org", "name"] {
        let property = package_properties
            .get_mut(field)
            .and_then(Value::as_object_mut)
            .expect("package coordinate exists");
        property.insert("minLength".to_owned(), Value::from(1));
        property.insert(
            "pattern".to_owned(),
            Value::String(r"^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$".to_owned()),
        );
    }
    let version = package_properties
        .get_mut("version")
        .and_then(Value::as_object_mut)
        .expect("package version exists");
    version.insert("minLength".to_owned(), Value::from(1));
    version.insert(
        "pattern".to_owned(),
        Value::String(VERSION_PATTERN.to_owned()),
    );

    let platform_properties = definitions
        .get_mut("BinaryPlatformV1")
        .and_then(|value| value.get_mut("properties"))
        .and_then(Value::as_object_mut)
        .expect("BinaryPlatformV1 properties exist");
    for field in ["target", "os", "arch", "libc", "abi"] {
        let property = platform_properties
            .get_mut(field)
            .and_then(Value::as_object_mut)
            .expect("platform property exists");
        property.insert("maxLength".to_owned(), Value::from(128));
        property.insert("minLength".to_owned(), Value::from(1));
        property.insert(
            "pattern".to_owned(),
            Value::String(PLATFORM_PATTERN.to_owned()),
        );
    }

    let source_properties = definitions
        .get_mut("BinarySourceProvenanceV1")
        .and_then(|value| value.get_mut("properties"))
        .and_then(Value::as_object_mut)
        .expect("BinarySourceProvenanceV1 properties exist");
    for field in ["repository", "vcs_tag"] {
        source_properties
            .get_mut(field)
            .and_then(Value::as_object_mut)
            .expect("source property exists")
            .insert("minLength".to_owned(), Value::from(1));
    }
    let vcs_commit = source_properties
        .get_mut("vcs_commit")
        .and_then(Value::as_object_mut)
        .expect("source vcs_commit exists");
    vcs_commit.insert("maxLength".to_owned(), Value::from(128));
    vcs_commit.insert("minLength".to_owned(), Value::from(7));
    vcs_commit.insert(
        "pattern".to_owned(),
        Value::String(r"^[A-Za-z0-9._+:/-]+$".to_owned()),
    );
}

/// The dependency-graph wire contract omits absent optional members. Explicit
/// JSON `null` is not a second spelling for absence, because accepting it would
/// make canonical JSON, YAML, and TOML projections disagree. Schemars models a
/// Rust `Option<T>` as `T | null`, so normalize this one schema to the actual
/// v1 serialization contract while leaving the field itself non-required.
fn write_dependency_graph_schema(dir: &Path) {
    let schema = schema_for!(zed_interfaces::DependencyGraphDocument);
    let mut value = serde_json::to_value(schema).expect("schema serializes");
    reject_explicit_nulls(&mut value);
    pin_dependency_graph_identity(&mut value);
    let json = serde_json::to_string_pretty(&value).expect("schema serializes");
    let path = dir.join("dependency-graph-v1.json");
    fs::write(&path, json + "\n").expect("schema file writes");
    println!("wrote {}", path.display());
}

/// Keep the shared interface schema byte-semantically aligned with the
/// canonical offline `zed inspect` v1 contract. Schemars cannot infer constant
/// safety markers, and it treats every `Option<T>` as both nullable and
/// optional even when the wire contract requires an explicit `null` value.
fn write_inspection_schema(dir: &Path) {
    let schema = schema_for!(zed_interfaces::inspection::InspectionReport);
    let mut value = serde_json::to_value(schema).expect("schema serializes");
    let root = value
        .as_object_mut()
        .expect("inspection schema is an object");
    root.insert(
        "$id".to_owned(),
        Value::String("https://zpkg.net/schemas/inspect-v1.schema.json".to_owned()),
    );

    let definitions = root
        .get_mut("$defs")
        .and_then(Value::as_object_mut)
        .expect("inspection schema has definitions");

    let cli_properties = definitions
        .get_mut("InspectionCliIdentity")
        .and_then(|definition| definition.get_mut("properties"))
        .and_then(Value::as_object_mut)
        .expect("inspection CLI identity properties exist");
    for (field, property_schema) in [
        (
            "implementation",
            serde_json::json!({"type": "string", "const": "zed-pkg"}),
        ),
        (
            "command",
            serde_json::json!({"type": "string", "const": "inspect"}),
        ),
        (
            "offline",
            serde_json::json!({"type": "boolean", "const": true}),
        ),
        (
            "mutates_project",
            serde_json::json!({"type": "boolean", "const": false}),
        ),
        (
            "loads_credentials",
            serde_json::json!({"type": "boolean", "const": false}),
        ),
    ] {
        cli_properties.insert(field.to_owned(), property_schema);
    }

    for (definition, property) in [
        ("InspectedPackageState", "identity"),
        ("InspectedLockedPackage", "store_present"),
        ("InteropStatus", "source"),
    ] {
        let required = definitions
            .get_mut(definition)
            .and_then(|value| value.get_mut("required"))
            .and_then(Value::as_array_mut)
            .expect("inspection object has required members");
        if !required
            .iter()
            .any(|member| member.as_str() == Some(property))
        {
            required.push(Value::String(property.to_owned()));
        }
    }

    for (definition, property) in [
        ("InspectionDiagnostic", "detail"),
        ("InspectionLocation", "line"),
        ("InspectionLocation", "column"),
    ] {
        let property_schema = definitions
            .get_mut(definition)
            .and_then(|value| value.get_mut("properties"))
            .and_then(|value| value.get_mut(property))
            .expect("optional inspection property exists");
        reject_explicit_nulls(property_schema);
    }

    let json = serde_json::to_string_pretty(&value).expect("schema serializes");
    let path = dir.join("inspection-report.json");
    fs::write(&path, json + "\n").expect("schema file writes");
    println!("wrote {}", path.display());
}

/// Schemars leaves `schema` a free string with only a default and digest
/// members as bare strings, so a `v99` document or a malformed digest would
/// still validate. Wire documents pin both: the schema id is the exact v1
/// string and every digest member uses the canonical lowercase `sha256:` form.
fn pin_dependency_graph_identity(value: &mut Value) {
    const DIGEST_PATTERN: &str = "^sha256:[0-9a-f]{64}$";
    const DIGEST_MEMBERS: [&str; 5] = [
        "graph_digest",
        "parent_graph_digest",
        "lock_digest",
        "checkpoint_digest",
        "artifact_digest",
    ];

    match value {
        Value::Array(values) => {
            for value in values {
                pin_dependency_graph_identity(value);
            }
        }
        Value::Object(object) => {
            if let Some(Value::Object(properties)) = object.get_mut("properties") {
                for (name, property) in properties.iter_mut() {
                    let Value::Object(property) = property else {
                        continue;
                    };
                    if property.get("type").and_then(Value::as_str) != Some("string") {
                        continue;
                    }
                    if name == "schema" {
                        property.insert(
                            "const".to_owned(),
                            Value::String(zed_interfaces::DEPENDENCY_GRAPH_SCHEMA_V1.to_owned()),
                        );
                    } else if DIGEST_MEMBERS.contains(&name.as_str()) {
                        property.insert("pattern".to_owned(), Value::String(DIGEST_PATTERN.into()));
                    }
                }
            }
            for value in object.values_mut() {
                pin_dependency_graph_identity(value);
            }
        }
        _ => {}
    }
}

/// Golden dependency-graph conformance vectors. Written as exact canonical
/// bytes with no trailing newline, because verifiers compare byte-for-byte.
fn write_dependency_graph_fixtures() {
    let dir = Path::new("fixtures/dependency-graph-v1/golden");
    fs::create_dir_all(dir).expect("golden fixtures dir");
    for (name, document) in zed_interfaces::golden_fixture_documents() {
        let bytes = document
            .canonical_document_bytes()
            .expect("golden fixture canonicalizes");
        let path = dir.join(format!("{name}.json"));
        fs::write(&path, bytes).expect("fixture file writes");
        println!("wrote {}", path.display());
    }
}

fn reject_explicit_nulls(value: &mut Value) {
    match value {
        Value::Array(values) => {
            for value in values {
                reject_explicit_nulls(value);
            }
        }
        Value::Object(object) => {
            for value in object.values_mut() {
                reject_explicit_nulls(value);
            }

            let type_replacement = match object.get_mut("type") {
                Some(Value::Array(types)) => {
                    types.retain(|value| value.as_str() != Some("null"));
                    (types.len() == 1).then(|| types[0].clone())
                }
                _ => None,
            };
            if let Some(replacement) = type_replacement {
                object.insert("type".to_owned(), replacement);
            }

            let any_of_replacement = match object.get_mut("anyOf") {
                Some(Value::Array(branches)) => {
                    branches.retain(|branch| {
                        branch.get("type").and_then(Value::as_str) != Some("null")
                    });
                    (branches.len() == 1).then(|| branches.remove(0))
                }
                _ => None,
            };
            if let Some(Value::Object(branch)) = any_of_replacement {
                object.remove("anyOf");
                for (key, value) in branch {
                    object.entry(key).or_insert(value);
                }
            }
        }
        _ => {}
    }
}

fn main() {
    // Manifest-relative (this crate is `src/rust/`, the schemas are at the
    // repository root) so the output is the same from any working directory.
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas");
    let dir = dir.as_path();
    fs::create_dir_all(dir).expect("schemas dir");

    write_dependency_graph_schema(dir);
    write_dependency_graph_fixtures();
    write::<zed_interfaces::Manifest>(dir, "manifest");
    write::<zed_interfaces::Lockfile>(dir, "lockfile");
    write_inspection_schema(dir);
    write_binary_artifact_schema(dir);
    write_versioned_binary_schema::<zed_interfaces::BinaryArtifactMetadataV1>(
        dir,
        "binary-artifact-metadata-v1",
        zed_interfaces::BINARY_ARTIFACT_METADATA_SCHEMA_V1,
    );
    write_versioned_binary_schema::<zed_interfaces::BinaryArtifactListResponseV1>(
        dir,
        "binary-artifact-list-v1",
        zed_interfaces::BINARY_ARTIFACT_LIST_SCHEMA_V1,
    );
    write_versioned_binary_schema::<zed_interfaces::BinaryArtifactPublishMetaV1>(
        dir,
        "binary-artifact-publish-meta-v1",
        zed_interfaces::BINARY_ARTIFACT_PUBLISH_META_SCHEMA_V1,
    );
    write_versioned_binary_schema::<zed_interfaces::BinaryArtifactLockV1>(
        dir,
        "binary-artifact-lock-v1",
        zed_interfaces::BINARY_ARTIFACT_LOCK_SCHEMA_V1,
    );
    write::<zed_interfaces::EnvironmentPlan>(dir, "environment-plan-v1");
    write::<zed_interfaces::EnvironmentPlanV2>(dir, "environment-plan");
    write::<zed_interfaces::EnvironmentLock>(dir, "environment-lock-v1");
    write::<zed_interfaces::NixExportSection>(dir, "nix-export-section");
    write::<zed_interfaces::NixExportPlan>(dir, "nix-export-plan");
    write::<zed_interfaces::NixAdapterRecord>(dir, "nix-adapter-record");
    write::<zed_interfaces::NativeRegistryAdapterRecord>(dir, "native-registry-adapter-record");
    write::<zed_interfaces::RegistryNamespacePlan>(dir, "registry-namespace-plan");
    write::<zed_interfaces::RegistryNamespaceClaimReceipt>(dir, "registry-namespace-claim-receipt");
    // The resolved (host, channel, version, endpoint) destination a release
    // publishes to. Emitted so non-Rust release tooling reads the same shape
    // `zed release plan --json` prints.
    write::<zed_interfaces::ChannelRoute>(dir, "channel-route");
    write::<zed_interfaces::NativeDependencyLock>(dir, "native-dependency-lock");
    write::<zed_interfaces::OciAdapterRecord>(dir, "oci-adapter-record");
    write::<zed_interfaces::registry::PackageMetadata>(dir, "package-metadata");
    write::<zed_interfaces::registry::VersionMetadata>(dir, "version-metadata");
    write::<zed_interfaces::registry::PublishMeta>(dir, "publish-meta");
    write::<zed_interfaces::registry::PublishResponse>(dir, "publish-response");
    write::<zed_interfaces::registry::SearchResponse>(dir, "search-response");
    write::<zed_interfaces::registry::ClaimOrgRequest>(dir, "claim-org-request");
    write::<zed_interfaces::registry::ClaimOrgResponse>(dir, "claim-org-response");
    write::<zed_interfaces::registry::YankRequest>(dir, "yank-request");
    write::<zed_interfaces::registry::YankResponse>(dir, "yank-response");
    // Governance/audit reads, so non-Rust clients can consume the trail and
    // verify the chain against the same shape the server serves.
    write::<zed_interfaces::registry::AuditLogResponse>(dir, "audit-log-response");
    write::<zed_interfaces::registry::AuditIntegrityResponse>(dir, "audit-integrity-response");
    write::<zed_interfaces::registry::ApiError>(dir, "api-error");

    // Registry protocol v1 trust, static-read, archive, and publish contracts.
    write::<zed_interfaces::RegistryDiscoveryV1>(dir, "registry-discovery-v1");
    write::<zed_interfaces::RegistryIndexRecordV1>(dir, "registry-index-record-v1");
    write::<zed_interfaces::RegistryIndexSnapshotV1>(dir, "registry-index-snapshot-v1");
    write::<zed_interfaces::RegistryCheckpointV1>(dir, "registry-checkpoint-v1");
    write::<zed_interfaces::RegistryArchiveManifestV1>(dir, "registry-archive-manifest-v1");
    write::<zed_interfaces::RegistryPublishRequestV1>(dir, "registry-publish-request-v1");
    write::<zed_interfaces::RegistryProtocolErrorV1>(dir, "registry-protocol-error-v1");

    // Sync contract types shared with zed-sync + zed-clients.
    write::<zed_interfaces::sync::SyncChangeEvent>(dir, "sync-change-event");
    write::<zed_interfaces::sync::SyncWriteMode>(dir, "sync-write-mode");
    write::<zed_interfaces::sync::SyncErrorPolicy>(dir, "sync-error-policy");
    write::<zed_interfaces::sync::SyncConflictResolution>(dir, "sync-conflict-resolution");

    // Registry list + RAG/embedding search DTOs.
    write::<zed_interfaces::registry::PackageListResponse>(dir, "package-list-response");
    write::<zed_interfaces::registry::SemanticSearchRequest>(dir, "semantic-search-request");
    write::<zed_interfaces::registry::SemanticSearchResponse>(dir, "semantic-search-response");
    write::<zed_interfaces::registry::EmbeddingUpsertRequest>(dir, "embedding-upsert-request");
}
