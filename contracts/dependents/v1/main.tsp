namespace Zed.Dependents.V1;

union ConsumerKind {
  server: "server",
  cli: "cli",
  worker: "worker",
  app: "app",
  library: "library",
  other: "other",
}

union AutomationMode {
  off: "off",
  notifyOnly: "notify_only",
  draftPr: "draft_pr",
}

union ReleaseClass {
  patchSuppressed: "patch_suppressed",
  patchSecurityNotify: "patch_security_notify",
  minor: "minor",
  major: "major",
  calendar: "calendar",
}

model LockedDependencySnapshotV1 {
  package: string;
  resolved_version: string;
  requirement?: string;
  source: string;
  checksum_sha256?: string;
  direct: boolean;
}

model ConsumerRegistrationRequestV1 {
  protocol: "zed.consumer-registration.v1";
  repository: string;
  default_branch: string;
  source_commit: string;
  consumer_kind: ConsumerKind;
  automation_mode: AutomationMode;
  @minValue(300)
  @maxValue(2592000)
  registration_ttl_seconds: uint64;
  notify_major: boolean;
  notify_minor: boolean;
  notify_patch: boolean;
  security_patch_notify: boolean;
  dependencies: LockedDependencySnapshotV1[];
  validation_commands: string[];
}

model ReleaseImpactRequestV1 {
  protocol: "zed.release-impact.v1";
  release_event_id: string;
  package: string;
  previous_version?: string;
  published_version: string;
  release_class: ReleaseClass;
  security_critical: boolean;
  published_source_commit: string;
}
