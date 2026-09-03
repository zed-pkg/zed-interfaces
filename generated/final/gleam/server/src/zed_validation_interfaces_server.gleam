// Generated only after independent JSON Schema and TypeSpec agreement. DO NOT EDIT.
import gleam/option.{type Option}

pub const contract_version="ores.validation.v2"

pub type RegistryLeaseRow {
  RegistryLeaseRow(
    acquired_at: String,
    expires_at: String,
    fencing_token: Int,
    holder_id: String,
    resource_key: String,
  )
}

pub type RegistryPackageRow {
  RegistryPackageRow(
    created_at: String,
    description: Option(String),
    id: String,
    name: String,
    namespace: String,
    updated_at: String,
  )
}

pub type RegistryVersionRow {
  RegistryVersionRow(
    artifact_sha256: String,
    artifact_size_bytes: Int,
    id: String,
    package_id: String,
    published_at: String,
    version: String,
    yanked: Bool,
  )
}
