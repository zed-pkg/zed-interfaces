//! Generated only after independent JSON Schema and TypeSpec agreement. DO NOT EDIT.

#[derive(Clone, Debug, PartialEq)]
pub struct RegistryLeaseRow {
    pub acquired_at: String,
    pub expires_at: String,
    pub fencing_token: i64,
    pub holder_id: String,
    pub resource_key: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RegistryPackageRow {
    pub created_at: String,
    pub description: Option<String>,
    pub id: String,
    pub name: String,
    pub namespace: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RegistryVersionRow {
    pub artifact_sha256: String,
    pub artifact_size_bytes: i64,
    pub id: String,
    pub package_id: String,
    pub published_at: String,
    pub version: String,
    pub yanked: bool,
}
