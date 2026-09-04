namespace Zed.Validation.Server;

// @orm-schema {"schema":"zed_pkg"}

// @orm {"foreignKeys":[],"indexes":[{"columns":["updatedAt"],"name":"packages_updated_at_idx"}],"model":"RegistryPackageRow","primaryKey":["id"],"table":"packages","unique":[{"columns":["namespace","name"],"name":"packages_namespace_name_key"}]}
model RegistryPackageRow {
  @format("uuid") id: string;
  @minLength(1) @maxLength(128) @pattern("^[a-z0-9][a-z0-9._-]*$") namespace: string;
  @minLength(1) @maxLength(128) @pattern("^[a-z0-9][a-z0-9._-]*$") name: string;
  @maxLength(4096) description?: string;
  @format("date-time") createdAt: string;
  @format("date-time") updatedAt: string;
}

// @orm {"foreignKeys":[{"columns":["packageId"],"name":"package_versions_package_id_fkey","onDelete":"cascade","references":{"columns":["id"],"table":"packages"}}],"indexes":[{"columns":["publishedAt"],"name":"package_versions_published_at_idx"},{"columns":["artifactSha256"],"name":"package_versions_artifact_sha256_idx"}],"model":"RegistryVersionRow","primaryKey":["id"],"table":"package_versions","unique":[{"columns":["packageId","version"],"name":"package_versions_package_version_key"}]}
model RegistryVersionRow {
  @format("uuid") id: string;
  @format("uuid") packageId: string;
  @minLength(1) @maxLength(128) version: string;
  @minLength(64) @maxLength(64) @pattern("^[0-9a-f]{64}$") artifactSha256: string;
  @minValue(0) artifactSizeBytes: safeint;
  yanked: boolean = false;
  @format("date-time") publishedAt: string;
}

// @orm {"foreignKeys":[],"indexes":[{"columns":["expiresAt"],"name":"registry_leases_expires_at_idx"}],"model":"RegistryLeaseRow","primaryKey":["resourceKey"],"table":"registry_leases","unique":[]}
model RegistryLeaseRow {
  @minLength(1) @maxLength(256) resourceKey: string;
  @minLength(1) @maxLength(128) holderId: string;
  @minValue(1) fencingToken: safeint;
  @format("date-time") acquiredAt: string;
  @format("date-time") expiresAt: string;
}

alias ServerPersistenceContract =
  RegistryPackageRow | RegistryVersionRow | RegistryLeaseRow;
