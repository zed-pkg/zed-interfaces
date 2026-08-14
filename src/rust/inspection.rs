//! Stable DTOs for the read-only, offline `zed inspect` wire contract.
//!
//! Version 1 is intentionally narrower than an arbitrary task runner: reports
//! never load credentials or use the network, and recommended actions are
//! structured `zed` argv rather than shell strings. Unknown additive fields
//! remain forward-compatible through Serde's default map handling.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const INSPECTION_SCHEMA_VERSION: &str = "1.1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InspectionReport {
    #[schemars(regex(pattern = r"^1\.[0-9]+$"))]
    pub schema_version: String,
    pub root: String,
    pub cli: InspectionCliIdentity,
    pub package: InspectedPackageState,
    pub workspace_members: Vec<String>,
    pub adapter_outputs: Vec<InspectionAdapterOutput>,
    pub locked_packages: Vec<InspectedLockedPackage>,
    pub interop: InteropInspection,
    pub summary: InspectionSummary,
    pub diagnostics: Vec<InspectionDiagnostic>,
}

impl InspectionReport {
    /// Whether this report preserves the non-mutating, credential-free v1
    /// execution boundary. Schema validation pins these values as constants;
    /// this helper gives Rust consumers the same explicit check.
    pub fn preserves_offline_contract(&self) -> bool {
        let compatible_version =
            self.schema_version
                .split_once('.')
                .is_some_and(|(major, minor)| {
                    major == "1"
                        && !minor.is_empty()
                        && minor.bytes().all(|byte| byte.is_ascii_digit())
                });
        compatible_version
            && self.cli.implementation == "zed-pkg"
            && self.cli.command == "inspect"
            && self.cli.offline
            && !self.cli.mutates_project
            && !self.cli.loads_credentials
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InspectionCliIdentity {
    pub implementation: String,
    pub command: String,
    pub offline: bool,
    pub mutates_project: bool,
    pub loads_credentials: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InspectedPackageState {
    pub manifest: String,
    pub lockfile: String,
    pub materialization_dir: String,
    pub identity: Option<InspectedPackageIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InspectedPackageIdentity {
    pub org: String,
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InspectionAdapterOutput {
    pub kind: String,
    pub path: String,
    pub present: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InspectedLockedPackage {
    pub org: String,
    pub name: String,
    pub version: String,
    #[schemars(regex(pattern = r"^[0-9a-f]{64}$"))]
    pub sha256: String,
    pub store_present: Option<bool>,
    pub materialized: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InteropInspection {
    pub git_submodules: InteropStatus,
    pub mise: InteropStatus,
    pub nix_develop: InteropStatus,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InteropStatus {
    pub detected: bool,
    pub declared: bool,
    pub verified: bool,
    pub source: Option<String>,
}

impl InteropStatus {
    pub fn absent() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InspectionSummary {
    pub health: InspectionHealth,
    pub errors: u32,
    pub warnings: u32,
    pub frozen_ready: bool,
    pub recovery_pending: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum InspectionHealth {
    Healthy,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InspectionDiagnostic {
    #[schemars(regex(pattern = r"^[A-Z][A-Z0-9_]*$"))]
    pub code: String,
    pub severity: InspectionSeverity,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub location: InspectionLocation,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<InspectionRecommendedAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum InspectionSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InspectionLocation {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1))]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1))]
    pub column: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InspectionRecommendedAction {
    pub id: String,
    pub title: String,
    pub kind: InspectionActionKind,
    #[schemars(length(min = 1))]
    pub argv: Vec<String>,
    pub cwd: String,
    pub mutates_project: bool,
    pub requires_network: bool,
    pub executes_package_code: bool,
}

impl InspectionRecommendedAction {
    /// Editors must ask before offering any action that mutates project state,
    /// reaches the network, or executes package code. This is derived rather
    /// than serialized so the wire contract cannot carry contradictory flags.
    pub fn requires_confirmation(&self) -> bool {
        self.mutates_project || self.requires_network || self.executes_package_code
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum InspectionActionKind {
    ZedCommand,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canonical_report() -> serde_json::Value {
        serde_json::json!({
            "schema_version": "1.1",
            "root": "/workspace",
            "cli": {
                "implementation": "zed-pkg",
                "command": "inspect",
                "offline": true,
                "mutates_project": false,
                "loads_credentials": false
            },
            "package": {
                "manifest": "/workspace/.zpkg.toml",
                "lockfile": "/workspace/.zpkg.lock",
                "materialization_dir": "/workspace/zed_modules",
                "identity": {"org": "acme", "name": "demo", "version": "1.0.0"}
            },
            "workspace_members": [],
            "adapter_outputs": [],
            "locked_packages": [],
            "interop": {
                "git_submodules": {"detected": false, "declared": false, "verified": false, "source": null},
                "mise": {"detected": false, "declared": false, "verified": false, "source": null},
                "nix_develop": {"detected": false, "declared": false, "verified": false, "source": null}
            },
            "summary": {
                "health": "warning",
                "errors": 0,
                "warnings": 1,
                "frozen_ready": false,
                "recovery_pending": false
            },
            "diagnostics": [{
                "code": "LOCK_MISSING",
                "severity": "warning",
                "message": "No lockfile exists.",
                "location": {"path": "/workspace/.zpkg.lock"},
                "actions": [{
                    "id": "create-lock",
                    "title": "Resolve and create the lockfile",
                    "kind": "zed-command",
                    "argv": ["zed", "install"],
                    "cwd": "/workspace",
                    "mutates_project": true,
                    "requires_network": true,
                    "executes_package_code": true
                }]
            }]
        })
    }

    #[test]
    fn canonical_cli_v1_report_roundtrips() {
        let report: InspectionReport = serde_json::from_value(canonical_report()).unwrap();
        assert!(report.preserves_offline_contract());
        assert_eq!(report.interop.git_submodules, InteropStatus::absent());
        assert!(report.diagnostics[0].actions[0].requires_confirmation());
        assert_eq!(
            serde_json::to_value(InspectionActionKind::ZedCommand).unwrap(),
            "zed-command"
        );
    }

    #[test]
    fn additive_unknown_fields_remain_forward_compatible() {
        let mut value = canonical_report();
        value.as_object_mut().unwrap().insert(
            "future_addition".to_string(),
            serde_json::json!({"safe": true}),
        );
        let report: InspectionReport = serde_json::from_value(value).unwrap();
        assert!(report.preserves_offline_contract());
    }

    #[test]
    fn incompatible_version_or_cli_identity_fails_the_safety_check() {
        let mut value = canonical_report();
        value["cli"]["offline"] = serde_json::Value::Bool(false);
        let report: InspectionReport = serde_json::from_value(value).unwrap();
        assert!(!report.preserves_offline_contract());

        let mut value = canonical_report();
        value["schema_version"] = serde_json::Value::String("1.".to_string());
        let report: InspectionReport = serde_json::from_value(value).unwrap();
        assert!(!report.preserves_offline_contract());
    }
}
