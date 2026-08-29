//! Generated from a route-map JSON. Do not edit by hand.
//! Exhaustive `RouteKey` match is the backend compile check.
#![allow(dead_code)]

pub const SERVICE: &str = "zed-api-server";

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum RouteKey {
    Healthz,
    GetPackage,
    GetVersion,
    GetArtifact,
    Search,
    ListPackages,
    SemanticSearch,
    UpsertEmbedding,
    GetFile,
    Yank,
    ClaimOrg,
    GetAudit,
    VerifyAudit,
    RegistryEvents,
    CdnGithubObject,
    CdnPackageObject,
    CdnContentObject,
    GithubReleaseAsset,
}

impl RouteKey {
    pub const ALL: &'static [Self] = &[Self::Healthz, Self::GetPackage, Self::GetVersion, Self::GetArtifact, Self::Search, Self::ListPackages, Self::SemanticSearch, Self::UpsertEmbedding, Self::GetFile, Self::Yank, Self::ClaimOrg, Self::GetAudit, Self::VerifyAudit, Self::RegistryEvents, Self::CdnGithubObject, Self::CdnPackageObject, Self::CdnContentObject, Self::GithubReleaseAsset];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Healthz => "healthz",
            Self::GetPackage => "get_package",
            Self::GetVersion => "get_version",
            Self::GetArtifact => "get_artifact",
            Self::Search => "search",
            Self::ListPackages => "list_packages",
            Self::SemanticSearch => "semantic_search",
            Self::UpsertEmbedding => "upsert_embedding",
            Self::GetFile => "get_file",
            Self::Yank => "yank",
            Self::ClaimOrg => "claim_org",
            Self::GetAudit => "get_audit",
            Self::VerifyAudit => "verify_audit",
            Self::RegistryEvents => "registry_events",
            Self::CdnGithubObject => "cdn_github_object",
            Self::CdnPackageObject => "cdn_package_object",
            Self::CdnContentObject => "cdn_content_object",
            Self::GithubReleaseAsset => "github_release_asset",
        }
    }

    #[must_use]
    pub fn parse(key: &str) -> Option<Self> {
        match key {
            "healthz" => Some(Self::Healthz),
            "get_package" => Some(Self::GetPackage),
            "get_version" => Some(Self::GetVersion),
            "get_artifact" => Some(Self::GetArtifact),
            "search" => Some(Self::Search),
            "list_packages" => Some(Self::ListPackages),
            "semantic_search" => Some(Self::SemanticSearch),
            "upsert_embedding" => Some(Self::UpsertEmbedding),
            "get_file" => Some(Self::GetFile),
            "yank" => Some(Self::Yank),
            "claim_org" => Some(Self::ClaimOrg),
            "get_audit" => Some(Self::GetAudit),
            "verify_audit" => Some(Self::VerifyAudit),
            "registry_events" => Some(Self::RegistryEvents),
            "cdn_github_object" => Some(Self::CdnGithubObject),
            "cdn_package_object" => Some(Self::CdnPackageObject),
            "cdn_content_object" => Some(Self::CdnContentObject),
            "github_release_asset" => Some(Self::GithubReleaseAsset),
            _ => None,
        }
    }

    #[must_use]
    pub fn path(self) -> &'static str {
        match self {
            Self::Healthz => "/healthz",
            Self::GetPackage => "/v1/packages/{org}/{name}",
            Self::GetVersion => "/v1/packages/{org}/{name}/versions/{version}",
            Self::GetArtifact => "/v1/artifacts/{sha256}",
            Self::Search => "/v1/search",
            Self::ListPackages => "/v1/packages",
            Self::SemanticSearch => "/v1/search/semantic",
            Self::UpsertEmbedding => "/v1/packages/{org}/{name}/embedding",
            Self::GetFile => "/v1/files/{org}/{name}/{version}/{path}",
            Self::Yank => "/v1/packages/{org}/{name}/versions/{version}/yank",
            Self::ClaimOrg => "/v1/orgs",
            Self::GetAudit => "/v1/orgs/{org}/audit",
            Self::VerifyAudit => "/v1/orgs/{org}/audit/verify",
            Self::RegistryEvents => "/v1/ws",
            Self::CdnGithubObject => "/github/{owner}/{repo}/{tag}/{filename}",
            Self::CdnPackageObject => "/packages/{org}/{name}/{version}/{filename}",
            Self::CdnContentObject => "/artifacts/{sha256}.{ext}",
            Self::GithubReleaseAsset => "/{owner}/{repo}/releases/download/{tag}/{asset}",
        }
    }

    #[must_use]
    pub fn methods(self) -> &'static [&'static str] {
        match self {
            Self::Healthz => &["GET"],
            Self::GetPackage => &["GET"],
            Self::GetVersion => &["GET", "PUT"],
            Self::GetArtifact => &["GET"],
            Self::Search => &["GET"],
            Self::ListPackages => &["GET"],
            Self::SemanticSearch => &["POST"],
            Self::UpsertEmbedding => &["PUT"],
            Self::GetFile => &["GET"],
            Self::Yank => &["POST"],
            Self::ClaimOrg => &["POST"],
            Self::GetAudit => &["GET"],
            Self::VerifyAudit => &["GET"],
            Self::RegistryEvents => &["GET"],
            Self::CdnGithubObject => &["GET"],
            Self::CdnPackageObject => &["GET"],
            Self::CdnContentObject => &["GET"],
            Self::GithubReleaseAsset => &["GET"],
        }
    }

    #[must_use]
    pub fn transports(self) -> &'static [&'static str] {
        match self {
            Self::Healthz => &["http"],
            Self::GetPackage => &["http"],
            Self::GetVersion => &["http", "tcp"],
            Self::GetArtifact => &["http"],
            Self::Search => &["http"],
            Self::ListPackages => &["http"],
            Self::SemanticSearch => &["http"],
            Self::UpsertEmbedding => &["http"],
            Self::GetFile => &["http"],
            Self::Yank => &["http"],
            Self::ClaimOrg => &["http"],
            Self::GetAudit => &["http"],
            Self::VerifyAudit => &["http"],
            Self::RegistryEvents => &["websocket"],
            Self::CdnGithubObject => &["http"],
            Self::CdnPackageObject => &["http"],
            Self::CdnContentObject => &["http"],
            Self::GithubReleaseAsset => &["http"],
        }
    }
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct GetPackagePath {
    pub org: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct GetVersionPath {
    pub org: String,
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct GetArtifactPath {
    pub sha256: String,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
    pub tag: Option<Vec<String>>,
    pub limit: Option<i64>,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct UpsertEmbeddingPath {
    pub org: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct GetFilePath {
    pub org: String,
    pub name: String,
    pub version: String,
    pub path: String,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct YankPath {
    pub org: String,
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct GetAuditPath {
    pub org: String,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct VerifyAuditPath {
    pub org: String,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct CdnGithubObjectPath {
    pub owner: String,
    pub repo: String,
    pub tag: String,
    pub filename: String,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct CdnPackageObjectPath {
    pub org: String,
    pub name: String,
    pub version: String,
    pub filename: String,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct CdnContentObjectPath {
    pub sha256: String,
    pub ext: String,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct GithubReleaseAssetPath {
    pub owner: String,
    pub repo: String,
    pub tag: String,
    pub asset: String,
}

