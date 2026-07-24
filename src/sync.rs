//! Shared sync contract types for cross-language consumers.
//!
//! [`zed-sync`](https://github.com/zed-pkg/zed-sync) is the behavioral source
//! of truth (its `protocol/*.schema.json` and `conformance.json` are
//! canonical). This module re-declares the *wire* enums and the change
//! envelope so Rust services and `zed-clients` share one set of types, and so
//! the generated JSON Schemas in `schemas/` stay aligned with zed-sync. Keep
//! these in lockstep with zed-sync's protocol.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Per-write optimism level (mirrors zed-sync `WriteMode`). Enum, not a
/// boolean: a write names its exact behavior.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SyncWriteMode {
    LocalOnly,
    #[default]
    OptimisticQueue,
    OptimisticAwaitAck,
    ServerFirst,
    ServerOnly,
}

/// How write/flush errors are surfaced (mirrors zed-sync `ErrorPolicy`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SyncErrorPolicy {
    ThrowOnly,
    #[default]
    EmitOnly,
    ThrowAndEmit,
    Silent,
}

/// How a dirty-vs-newer-remote conflict resolves (mirrors zed-sync
/// `ConflictResolution`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SyncConflictResolution {
    #[default]
    ServerWins,
    LastWriteWins,
}

/// Hybrid Logical Clock stamp — the reconciliation key. Total order:
/// `wall_ms`, then `counter`, then `actor`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Hlc {
    pub wall_ms: u64,
    pub counter: u32,
    pub actor: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SyncOp {
    Upsert,
    Delete,
}

/// The change envelope both transports decode to (mirrors zed-sync
/// `ChangeEvent`). `version` is the ONLY reconciliation key; `sync_sequence`
/// is the catch-up cursor and never feeds reconciliation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SyncChangeEvent {
    pub table: String,
    pub op: SyncOp,
    pub id: String,
    pub version: Hlc,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row: Option<serde_json::Value>,
    pub at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub write_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_sequence: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_values_match_zed_sync_protocol() {
        assert_eq!(
            serde_json::to_string(&SyncWriteMode::OptimisticQueue).unwrap(),
            "\"optimistic_queue\""
        );
        assert_eq!(
            serde_json::to_string(&SyncErrorPolicy::EmitOnly).unwrap(),
            "\"emit_only\""
        );
        assert_eq!(
            serde_json::to_string(&SyncConflictResolution::ServerWins).unwrap(),
            "\"server_wins\""
        );
        assert_eq!(
            serde_json::to_string(&SyncOp::Upsert).unwrap(),
            "\"upsert\""
        );
    }
}
