//! Regenerates the JSON Schemas under `schemas/`, which the non-Rust
//! client libraries in `zed-clients` use for codegen and validation.
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
    let dir = Path::new("schemas");
    fs::create_dir_all(dir).expect("schemas dir");

    write_dependency_graph_schema(dir);
    write_dependency_graph_fixtures();
    write::<zed_interfaces::Manifest>(dir, "manifest");
    write::<zed_interfaces::Lockfile>(dir, "lockfile");
    write::<zed_interfaces::EnvironmentPlan>(dir, "environment-plan-v1");
    write::<zed_interfaces::EnvironmentPlanV2>(dir, "environment-plan");
    write::<zed_interfaces::EnvironmentLock>(dir, "environment-lock-v1");
    write::<zed_interfaces::NixExportSection>(dir, "nix-export-section");
    write::<zed_interfaces::NixExportPlan>(dir, "nix-export-plan");
    write::<zed_interfaces::NixAdapterRecord>(dir, "nix-adapter-record");
    write::<zed_interfaces::NativeRegistryAdapterRecord>(dir, "native-registry-adapter-record");
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
