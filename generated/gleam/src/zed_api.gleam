//// Generated from a route-map JSON. Do not edit by hand.
//// Exhaustive `RouteKey` case is the backend compile check.

pub const service: String = "zed-api-server"

pub type RouteKey {
  Healthz
  GetPackage
  GetVersion
  GetArtifact
  Search
  ListPackages
  SemanticSearch
  UpsertEmbedding
  GetFile
  Yank
  ClaimOrg
  GetAudit
  VerifyAudit
  RegistryEvents
  CdnGithubObject
  CdnPackageObject
  CdnContentObject
  GithubReleaseAsset
}

pub fn all() -> List(RouteKey) {
  [Healthz, GetPackage, GetVersion, GetArtifact, Search, ListPackages, SemanticSearch, UpsertEmbedding, GetFile, Yank, ClaimOrg, GetAudit, VerifyAudit, RegistryEvents, CdnGithubObject, CdnPackageObject, CdnContentObject, GithubReleaseAsset]
}

pub fn to_string(key: RouteKey) -> String {
  case key {
    Healthz -> "healthz"
    GetPackage -> "get_package"
    GetVersion -> "get_version"
    GetArtifact -> "get_artifact"
    Search -> "search"
    ListPackages -> "list_packages"
    SemanticSearch -> "semantic_search"
    UpsertEmbedding -> "upsert_embedding"
    GetFile -> "get_file"
    Yank -> "yank"
    ClaimOrg -> "claim_org"
    GetAudit -> "get_audit"
    VerifyAudit -> "verify_audit"
    RegistryEvents -> "registry_events"
    CdnGithubObject -> "cdn_github_object"
    CdnPackageObject -> "cdn_package_object"
    CdnContentObject -> "cdn_content_object"
    GithubReleaseAsset -> "github_release_asset"
  }
}

pub fn parse(key: String) -> Result(RouteKey, Nil) {
  case key {
    "healthz" -> Ok(Healthz)
    "get_package" -> Ok(GetPackage)
    "get_version" -> Ok(GetVersion)
    "get_artifact" -> Ok(GetArtifact)
    "search" -> Ok(Search)
    "list_packages" -> Ok(ListPackages)
    "semantic_search" -> Ok(SemanticSearch)
    "upsert_embedding" -> Ok(UpsertEmbedding)
    "get_file" -> Ok(GetFile)
    "yank" -> Ok(Yank)
    "claim_org" -> Ok(ClaimOrg)
    "get_audit" -> Ok(GetAudit)
    "verify_audit" -> Ok(VerifyAudit)
    "registry_events" -> Ok(RegistryEvents)
    "cdn_github_object" -> Ok(CdnGithubObject)
    "cdn_package_object" -> Ok(CdnPackageObject)
    "cdn_content_object" -> Ok(CdnContentObject)
    "github_release_asset" -> Ok(GithubReleaseAsset)
    _ -> Error(Nil)
  }
}

pub fn path(key: RouteKey) -> String {
  case key {
    Healthz -> "/healthz"
    GetPackage -> "/v1/packages/{org}/{name}"
    GetVersion -> "/v1/packages/{org}/{name}/versions/{version}"
    GetArtifact -> "/v1/artifacts/{sha256}"
    Search -> "/v1/search"
    ListPackages -> "/v1/packages"
    SemanticSearch -> "/v1/search/semantic"
    UpsertEmbedding -> "/v1/packages/{org}/{name}/embedding"
    GetFile -> "/v1/files/{org}/{name}/{version}/{path}"
    Yank -> "/v1/packages/{org}/{name}/versions/{version}/yank"
    ClaimOrg -> "/v1/orgs"
    GetAudit -> "/v1/orgs/{org}/audit"
    VerifyAudit -> "/v1/orgs/{org}/audit/verify"
    RegistryEvents -> "/v1/ws"
    CdnGithubObject -> "/github/{owner}/{repo}/{tag}/{filename}"
    CdnPackageObject -> "/packages/{org}/{name}/{version}/{filename}"
    CdnContentObject -> "/artifacts/{sha256}.{ext}"
    GithubReleaseAsset -> "/{owner}/{repo}/releases/download/{tag}/{asset}"
  }
}

pub fn methods(key: RouteKey) -> List(String) {
  case key {
    Healthz -> ["GET"]
    GetPackage -> ["GET"]
    GetVersion -> ["GET", "PUT"]
    GetArtifact -> ["GET"]
    Search -> ["GET"]
    ListPackages -> ["GET"]
    SemanticSearch -> ["POST"]
    UpsertEmbedding -> ["PUT"]
    GetFile -> ["GET"]
    Yank -> ["POST"]
    ClaimOrg -> ["POST"]
    GetAudit -> ["GET"]
    VerifyAudit -> ["GET"]
    RegistryEvents -> ["GET"]
    CdnGithubObject -> ["GET"]
    CdnPackageObject -> ["GET"]
    CdnContentObject -> ["GET"]
    GithubReleaseAsset -> ["GET"]
  }
}

pub fn transports(key: RouteKey) -> List(String) {
  case key {
    Healthz -> ["http"]
    GetPackage -> ["http"]
    GetVersion -> ["http", "tcp"]
    GetArtifact -> ["http"]
    Search -> ["http"]
    ListPackages -> ["http"]
    SemanticSearch -> ["http"]
    UpsertEmbedding -> ["http"]
    GetFile -> ["http"]
    Yank -> ["http"]
    ClaimOrg -> ["http"]
    GetAudit -> ["http"]
    VerifyAudit -> ["http"]
    RegistryEvents -> ["websocket"]
    CdnGithubObject -> ["http"]
    CdnPackageObject -> ["http"]
    CdnContentObject -> ["http"]
    GithubReleaseAsset -> ["http"]
  }
}
