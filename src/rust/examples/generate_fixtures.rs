//! Serializes representative values of the front-end-facing types with serde,
//! producing `fixtures/*.json` — the bytes a real server actually sends.
//!
//! The schemas prove the *shape* agrees. These prove the *encoding* does:
//! whether an absent `Option` arrives as `null` or as a missing key, how a
//! `#[serde(default)]` field looks when the server omits it, which spelling an
//! enum variant serializes to. A generated Dart or TypeScript class can compile
//! cleanly against a correct schema and still fail to decode this, which is the
//! failure mode that reaches a user.
//!
//! Run with: `cargo run --locked --example generate_fixtures`

use std::fs;
use std::path::Path;

use serde::Serialize;
use zed_interfaces::registry::{
    ApiError, AuditAction, AuditEntry, AuditIntegrityResponse, AuditLogResponse, ClaimOrgRequest,
    ClaimOrgResponse, PackageListResponse, PackageMetadata, PackageSummary, PublishResponse,
    SearchResponse, SemanticHit, SemanticSearchRequest, SemanticSearchResponse, VersionMetadata,
    YankRequest, YankResponse,
};
use zed_interfaces::sync::{
    SyncChangeEvent, SyncConflictResolution, SyncErrorPolicy, SyncOp, SyncWriteMode,
};
use zed_interfaces::version::VersionScheme;
use zed_interfaces::{ArtifactFormat, Vcs};

fn write<T: Serialize>(dir: &Path, name: &str, cases: &[(&str, T)]) {
    let mut map = serde_json::Map::new();
    for (case, value) in cases {
        map.insert(
            (*case).to_string(),
            serde_json::to_value(value).expect("value serializes"),
        );
    }
    let json = serde_json::to_string_pretty(&serde_json::Value::Object(map)).expect("pretty");
    let path = dir.join(format!("{name}.json"));
    fs::write(&path, json + "\n").expect("fixture writes");
    println!("wrote fixtures/{name}.json");
}

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let dir = dir.as_path();
    fs::create_dir_all(dir).expect("fixtures dir");

    write(
        dir,
        "package-metadata",
        &[
            (
                "minimal",
                PackageMetadata {
                    org: "acme".into(),
                    name: "http-kit".into(),
                    vcs: Vcs::Git,
                    repo_url: "https://github.com/acme/http-kit".into(),
                    description: None,
                    latest: None,
                    versions: vec!["1.0.0".into()],
                    version_scheme: VersionScheme::default(),
                    tags: Vec::new(),
                },
            ),
            (
                "full",
                PackageMetadata {
                    org: "acme".into(),
                    name: "http-kit".into(),
                    vcs: Vcs::Jj,
                    repo_url: "https://github.com/acme/http-kit".into(),
                    description: Some("An HTTP kit".into()),
                    latest: Some("2.0.0".into()),
                    versions: vec!["2.0.0".into(), "1.0.0".into()],
                    version_scheme: VersionScheme::Calver,
                    tags: vec!["http".into(), "client".into()],
                },
            ),
        ],
    );

    write(
        dir,
        "version-metadata",
        &[(
            "published",
            VersionMetadata {
                org: "acme".into(),
                name: "http-kit".into(),
                version: "1.0.0".into(),
                sha256: "e".repeat(64),
                size: 4096,
                vcs_tag: "v1.0.0".into(),
                vcs_commit: Some("c".repeat(40)),
                download_url: "/v1/artifacts/abc".into(),
                published_at: "2026-08-06T00:00:00Z".into(),
                format: ArtifactFormat::TarGz,
                yanked: false,
            },
        )],
    );

    write(
        dir,
        "api-error",
        &[(
            "not_found",
            ApiError {
                code: "not_found".into(),
                message: "no such package".into(),
            },
        )],
    );

    let summary = PackageSummary {
        org: "acme".into(),
        name: "http-kit".into(),
        description: Some("An HTTP kit".into()),
        latest: Some("1.0.0".into()),
        tags: vec!["http".into()],
    };
    write(
        dir,
        "package-list-response",
        &[(
            "page",
            PackageListResponse {
                items: vec![summary.clone()],
                total: 1,
            },
        )],
    );
    write(
        dir,
        "search-response",
        &[(
            "hit",
            SearchResponse {
                query: "http".into(),
                items: vec![summary.clone()],
            },
        )],
    );

    write(
        dir,
        "audit-log-response",
        &[(
            "chain",
            AuditLogResponse {
                org: "acme".into(),
                entries: vec![AuditEntry {
                    seq: 1,
                    at: "2026-08-06T00:00:00Z".into(),
                    actor_token_name: "ci".into(),
                    actor_role: "publisher".into(),
                    action: "publish".into(),
                    action_kind: Some(AuditAction::Publish),
                    subject: "acme/http-kit@1.0.0".into(),
                    detail: None,
                    prev_hash: None,
                    entry_hash: "a".repeat(64),
                }],
            },
        )],
    );

    write(
        dir,
        "sync-change-event",
        &[(
            "upsert",
            SyncChangeEvent {
                table: "packages".into(),
                op: SyncOp::Upsert,
                id: "acme/http-kit".into(),
                version: zed_interfaces::sync::Hlc {
                    wall_ms: 1_754_400_000_000,
                    counter: 3,
                    actor: "server-1".into(),
                },
                at_ms: 1_754_400_000_001,
                row: Some(serde_json::json!({ "org": "acme" })),
                sync_sequence: Some(42),
                write_key: None,
            },
        )],
    );

    // Bare enums: their wire spelling is the whole contract.
    write(
        dir,
        "enums",
        &[
            (
                "write_mode",
                serde_json::to_value(SyncWriteMode::default()).unwrap(),
            ),
            (
                "error_policy",
                serde_json::to_value(SyncErrorPolicy::default()).unwrap(),
            ),
            (
                "conflict_resolution",
                serde_json::to_value(SyncConflictResolution::default()).unwrap(),
            ),
            (
                "artifact_format",
                serde_json::to_value(ArtifactFormat::Zip).unwrap(),
            ),
            ("vcs", serde_json::to_value(Vcs::Sapling).unwrap()),
            (
                "version_scheme",
                serde_json::to_value(VersionScheme::Opaque).unwrap(),
            ),
            (
                "audit_action",
                serde_json::to_value(AuditAction::OrgClaim).unwrap(),
            ),
        ],
    );

    write(
        dir,
        "misc-responses",
        &[
            (
                "publish",
                serde_json::to_value(PublishResponse {
                    org: "acme".into(),
                    name: "http-kit".into(),
                    version: "1.0.0".into(),
                    sha256: "e".repeat(64),
                })
                .unwrap(),
            ),
            (
                "claim_org_request",
                serde_json::to_value(ClaimOrgRequest {
                    slug: "acme".into(),
                })
                .unwrap(),
            ),
            (
                "claim_org_response",
                serde_json::to_value(ClaimOrgResponse {
                    slug: "acme".into(),
                    created: true,
                })
                .unwrap(),
            ),
            (
                "yank_request",
                serde_json::to_value(YankRequest { yanked: true }).unwrap(),
            ),
            (
                "yank_response",
                serde_json::to_value(YankResponse {
                    org: "acme".into(),
                    name: "http-kit".into(),
                    version: "1.0.0".into(),
                    yanked: true,
                })
                .unwrap(),
            ),
            (
                "audit_integrity",
                serde_json::to_value(AuditIntegrityResponse {
                    org: "acme".into(),
                    intact: true,
                    entries_checked: 1,
                    first_bad_seq: None,
                    problem: None,
                    head_hash: Some("a".repeat(64)),
                })
                .unwrap(),
            ),
            (
                "semantic_search_request",
                serde_json::to_value(SemanticSearchRequest {
                    model: "openai/text-embedding-3-small".into(),
                    embedding: vec![0.1, 0.2, 0.3],
                    limit: 5,
                    tags: vec!["http".into()],
                })
                .unwrap(),
            ),
            (
                "semantic_search_response",
                serde_json::to_value(SemanticSearchResponse {
                    items: vec![SemanticHit {
                        org: "acme".into(),
                        name: "http-kit".into(),
                        description: None,
                        distance: 0.25,
                        tags: vec!["http".into()],
                    }],
                })
                .unwrap(),
            ),
        ],
    );
}
