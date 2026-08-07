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

fn write<T: JsonSchema>(dir: &Path, name: &str) {
    let schema = schema_for!(T);
    let json = serde_json::to_string_pretty(&schema).expect("schema serializes");
    let path = dir.join(format!("{name}.json"));
    fs::write(&path, json + "\n").expect("schema file writes");
    println!("wrote {}", path.display());
}

fn main() {
    // Manifest-relative (this crate is `src/rust/`, the schemas are at the
    // repository root) so the output is the same from any working directory.
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas");
    let dir = dir.as_path();
    fs::create_dir_all(dir).expect("schemas dir");

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
