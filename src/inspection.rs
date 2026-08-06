//! Stable DTOs for read-only workspace inspection.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const INSPECTION_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InspectionReport {
    pub schema_version: u32,
    pub zed_version: String,
    pub workspace_root: String,
    pub package: PackageInspection,
    pub interop: InteropInspection,
    pub network: NetworkInspection,
    pub updates: Vec<VersionRecommendation>,
    pub summary: InspectionSummary,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PackageInspection {
    pub identity: Option<String>,
    pub version: Option<String>,
    pub manifest_path: String,
    pub lock_path: String,
    pub materialization_dir: String,
    pub manifest_valid: bool,
    pub lock_valid: bool,
    pub frozen_ready: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InteropInspection {
    pub git_submodules: InteropStatus,
    pub mise: InteropStatus,
    pub nix_develop: InteropStatus,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub struct NetworkInspection {
    pub enabled: bool,
    pub registry: Option<String>,
    pub update_check_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VersionRecommendation {
    pub package: String,
    pub current: String,
    pub latest: String,
    pub change: VersionChange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum VersionChange {
    Major,
    Minor,
    Patch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InspectionSummary {
    pub health: Health,
    pub errors: usize,
    pub warnings: usize,
    pub information: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Health {
    Healthy,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub title: String,
    pub detail: String,
    pub location: Option<DiagnosticLocation>,
    pub actions: Vec<RecommendedAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticLocation {
    pub path: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecommendedAction {
    pub id: String,
    pub title: String,
    pub kind: ActionKind,
    pub argv: Vec<String>,
    pub working_directory: String,
    pub mutates_project: bool,
    pub requires_network: bool,
    pub executes_package_code: bool,
    pub requires_confirmation: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ActionKind {
    ZedCommand,
    ExternalCommand,
    EditFile,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_wire_tokens_are_explicit() {
        assert_eq!(serde_json::to_value(Severity::Info).unwrap(), "info");
        assert_eq!(serde_json::to_value(VersionChange::Major).unwrap(), "major");
        assert_eq!(
            serde_json::to_value(ActionKind::ExternalCommand).unwrap(),
            "external-command"
        );
    }
}
