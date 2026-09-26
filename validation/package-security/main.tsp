import "@typespec/json-schema";
using TypeSpec.JsonSchema;

@jsonSchema
namespace Zed.PackageSecurity;

@extension("additionalProperties", false)
model PackageArtifactIdentity {
  ecosystem: "npm" | "cargo" | "python" | "git";
  @minLength(1) @maxLength(256) package_name: string;
  @minLength(1) @maxLength(128) package_version: string;
  @minLength(1) @maxLength(2048) source_uri: string;
  @pattern("^[a-f0-9]{64}$") source_digest: string;
  @minLength(1) @maxLength(256) resolved_revision: string;
  @pattern("^[a-f0-9]{64}$") artifact_digest: string;
}

@extension("additionalProperties", false)
model PackageRiskFinding {
  @pattern("^[A-Za-z0-9_-]{8,128}$") finding_id: string;
  signal_class: "typosquatting" | "dependency_confusion" | "install_or_lifecycle_scripts" | "unexpected_network_behavior" | "maintainer_or_ownership_churn" | "new_package_or_name_age" | "source_artifact_mismatch" | "obfuscation_or_generated_payload" | "secret_or_credential_access" | "native_binary_or_dynamic_library" | "malware_static_analysis" | "historical_lineage_anomaly";
  producer_kind: "deterministic_scanner" | "model" | "heuristic";
  @minLength(1) @maxLength(128) producer_id: string;
  @minLength(1) @maxLength(128) producer_version: string;
  severity: "info" | "low" | "medium" | "high" | "critical";
  @minValue(0) @maxValue(1) confidence?: float64;
  @pattern("^[a-f0-9]{64}$") evidence_digest: string;
  @minLength(1) @maxLength(512) evidence_ref?: string;
}

@extension("additionalProperties", false)
model PackageRiskAssessment {
  @pattern("^[A-Za-z0-9_-]{8,128}$") assessment_id: string;
  artifact: PackageArtifactIdentity;
  findings: PackageRiskFinding[];
  @pattern("^[a-f0-9]{64}$") policy_digest: string;
  @minValue(1) @maxValue(9007199254740991) assessed_at: integer;
  @minValue(1) @maxValue(9007199254740991) expires_at?: integer;
}

@extension("additionalProperties", false)
model PackageRebuildReceipt {
  @pattern("^[A-Za-z0-9_-]{8,128}$") rebuild_id: string;
  artifact: PackageArtifactIdentity;
  @pattern("^[a-f0-9]{64}$") rebuilt_artifact_digest: string;
  @pattern("^[a-f0-9]{64}$") declared_file_manifest_digest: string;
  @pattern("^[a-f0-9]{64}$") dependency_lock_digest: string;
  matched: boolean;
  @minLength(1) @maxLength(192) builder_identity: string;
  @minValue(1) @maxValue(9007199254740991) built_at: integer;
}

@extension("additionalProperties", false)
model PackageIntakeDecision {
  @pattern("^[A-Za-z0-9_-]{8,128}$") decision_id: string;
  @pattern("^[a-f0-9]{64}$") artifact_digest: string;
  @pattern("^[a-f0-9]{64}$") source_digest: string;
  @pattern("^[a-f0-9]{64}$") assessment_digest: string;
  @pattern("^[a-f0-9]{64}$") rebuild_receipt_digest: string;
  @pattern("^[a-f0-9]{64}$") policy_digest: string;
  result: "quarantine" | "pending_approval" | "reject" | "publishable";
  @minValue(1) @maxValue(9007199254740991) decided_at: integer;
}

@extension("additionalProperties", false)
model PackageApprovalReceipt {
  @pattern("^[A-Za-z0-9_-]{8,128}$") approval_id: string;
  @pattern("^[A-Za-z0-9_-]{8,128}$") decision_id: string;
  @pattern("^[a-f0-9]{64}$") artifact_digest: string;
  @pattern("^[a-f0-9]{64}$") source_digest: string;
  @pattern("^[a-f0-9]{64}$") policy_digest: string;
  @pattern("^[a-f0-9]{64}$") risk_receipt_digest: string;
  @pattern("^[a-f0-9]{64}$") rebuild_receipt_digest: string;
  @minLength(1) @maxLength(192) reviewer_ref: string;
  result: "approved" | "rejected";
  @minValue(1) @maxValue(9007199254740991) reviewed_at: integer;
}
