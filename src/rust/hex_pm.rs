//! Hex.pm interoperability contract.
//!
//! Hex has two deliberately separate network surfaces:
//! - the HTTP API for browsing, authentication, publishing, retirement, and
//!   ownership administration;
//! - the read-only repository for signed Registry v2 resources and package
//!   tarballs.
//!
//! Zed must preserve that split. In particular, JSON returned by the HTTP API
//! is useful for human-facing discovery but is **not** a substitute for signed
//! repository metadata during dependency resolution.

use std::fmt;

/// Stable Zed repository identity for the public Hex.pm repository.
pub const HEX_PM_REPOSITORY_NAME: &str = "hexpm";

/// Canonical Hex.pm administrative/browse API origin.
pub const HEX_PM_API_BASE_URL: &str = "https://hex.pm/api";

/// Canonical Hex.pm read-only package repository origin.
pub const HEX_PM_REPOSITORY_BASE_URL: &str = "https://repo.hex.pm";

/// Hex repository metadata format used for dependency resolution.
pub const HEX_PM_REGISTRY_VERSION: u8 = 2;

/// Repository scope. Hex.pm organization packages are served from a named
/// private repository under `/repos/REPO/*`; public packages are served from
/// the repository root.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HexPmRepositoryScope {
    Public,
    Private(String),
}

impl HexPmRepositoryScope {
    pub fn private(repository: impl Into<String>) -> Result<Self, HexPmPathError> {
        let repository = repository.into();
        validate_segment("repository", &repository)?;
        Ok(Self::Private(repository))
    }

    fn prefix(&self) -> String {
        match self {
            Self::Public => String::new(),
            Self::Private(repository) => format!("/repos/{repository}"),
        }
    }
}

/// The trust evidence required before a fetched Hex repository resource may be
/// admitted into resolution/cache state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HexPmVerification {
    /// Registry v2 resource must pass the repository signature verification
    /// defined by the Hex Registry v2 specification.
    RegistryV2Signature,
    /// Tarball bytes must match the checksum obtained from already-verified
    /// signed package metadata. Transport success alone is insufficient.
    SignedMetadataChecksum,
    /// The repository `/public_key` endpoint is discovery material only. A key
    /// fetched from the same untrusted transport cannot bootstrap trust in that
    /// transport; the trust anchor must be supplied out-of-band.
    OutOfBandTrustAnchor,
}

/// Read-only resources served by a Hex repository.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HexPmRepositoryResource {
    Names,
    Versions,
    Package(String),
    Tarball { package: String, version: String },
    PublicKey,
}

impl HexPmRepositoryResource {
    pub fn package(package: impl Into<String>) -> Result<Self, HexPmPathError> {
        let package = package.into();
        validate_segment("package", &package)?;
        Ok(Self::Package(package))
    }

    pub fn tarball(
        package: impl Into<String>,
        version: impl Into<String>,
    ) -> Result<Self, HexPmPathError> {
        let package = package.into();
        let version = version.into();
        validate_segment("package", &package)?;
        validate_segment("version", &version)?;
        Ok(Self::Tarball { package, version })
    }

    pub fn path(&self, scope: &HexPmRepositoryScope) -> String {
        let prefix = scope.prefix();
        match self {
            Self::Names => format!("{prefix}/names"),
            Self::Versions => format!("{prefix}/versions"),
            Self::Package(package) => format!("{prefix}/packages/{package}"),
            Self::Tarball { package, version } => {
                format!("{prefix}/tarballs/{package}-{version}.tar")
            }
            Self::PublicKey => format!("{prefix}/public_key"),
        }
    }

    pub const fn verification(&self) -> HexPmVerification {
        match self {
            Self::Names | Self::Versions | Self::Package(_) => {
                HexPmVerification::RegistryV2Signature
            }
            Self::Tarball { .. } => HexPmVerification::SignedMetadataChecksum,
            Self::PublicKey => HexPmVerification::OutOfBandTrustAnchor,
        }
    }
}

/// Administrative/browse API route builders. These return relative paths so a
/// self-hosted Hex-compatible API can use the same contract with a different
/// base origin.
pub mod api {
    use super::{HexPmPathError, validate_segment};

    pub const OAUTH_DEVICE_AUTHORIZATION_PATH: &str = "/oauth/device_authorization";
    pub const OAUTH_TOKEN_PATH: &str = "/oauth/token";
    pub const OAUTH_REVOKE_PATH: &str = "/oauth/revoke";
    pub const OAUTH_REVOKE_BY_HASH_PATH: &str = "/oauth/revoke_by_hash";
    pub const KEYS_PATH: &str = "/keys";
    pub const AUTH_PATH: &str = "/auth";
    pub const CURRENT_USER_PATH: &str = "/users/me";
    pub const PUBLISH_PATH: &str = "/publish";

    pub fn package_path(name: &str) -> Result<String, HexPmPathError> {
        validate_segment("package", name)?;
        Ok(format!("/packages/{name}"))
    }

    pub fn release_path(name: &str, version: &str) -> Result<String, HexPmPathError> {
        validate_segment("package", name)?;
        validate_segment("version", version)?;
        Ok(format!("/packages/{name}/releases/{version}"))
    }

    pub fn releases_path(name: &str) -> Result<String, HexPmPathError> {
        validate_segment("package", name)?;
        Ok(format!("/packages/{name}/releases"))
    }

    pub fn retirement_path(name: &str, version: &str) -> Result<String, HexPmPathError> {
        Ok(format!("{}/retire", release_path(name, version)?))
    }

    pub fn release_docs_path(name: &str, version: &str) -> Result<String, HexPmPathError> {
        Ok(format!("{}/docs", release_path(name, version)?))
    }

    pub fn owners_path(name: &str) -> Result<String, HexPmPathError> {
        validate_segment("package", name)?;
        Ok(format!("/packages/{name}/owners"))
    }

    pub fn owner_path(name: &str, username: &str) -> Result<String, HexPmPathError> {
        validate_segment("username", username)?;
        Ok(format!("{}/{}", owners_path(name)?, username))
    }

    pub fn organization_keys_path(organization: &str) -> Result<String, HexPmPathError> {
        validate_segment("organization", organization)?;
        Ok(format!("/orgs/{organization}/keys"))
    }

    pub fn organization_key_path(
        organization: &str,
        key_name: &str,
    ) -> Result<String, HexPmPathError> {
        validate_segment("key name", key_name)?;
        Ok(format!(
            "{}/{}",
            organization_keys_path(organization)?,
            key_name
        ))
    }

    pub fn user_path(username: &str) -> Result<String, HexPmPathError> {
        validate_segment("username", username)?;
        Ok(format!("/users/{username}"))
    }

    pub fn user_reset_path(username: &str) -> Result<String, HexPmPathError> {
        Ok(format!("{}/reset", user_path(username)?))
    }
}

/// Route construction intentionally rejects path/control delimiters rather
/// than silently percent-encoding them. Ecosystem-specific package grammar is
/// a separate concern; this check only guarantees one caller value cannot
/// escape the endpoint segment it was assigned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HexPmPathError {
    field: &'static str,
}

impl HexPmPathError {
    pub const fn field(&self) -> &'static str {
        self.field
    }
}

impl fmt::Display for HexPmPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Hex.pm {} must be one non-empty URL path segment",
            self.field
        )
    }
}

impl std::error::Error for HexPmPathError {}

fn validate_segment(field: &'static str, value: &str) -> Result<(), HexPmPathError> {
    let unsafe_character = value
        .chars()
        .any(|character| matches!(character, '/' | '?' | '#' | '\\') || character.is_control());
    if value.is_empty() || unsafe_character {
        return Err(HexPmPathError { field });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_repository_paths_match_registry_v2_contract() {
        let public = HexPmRepositoryScope::Public;
        assert_eq!(HexPmRepositoryResource::Names.path(&public), "/names");
        assert_eq!(HexPmRepositoryResource::Versions.path(&public), "/versions");
        assert_eq!(
            HexPmRepositoryResource::package("plug")
                .unwrap()
                .path(&public),
            "/packages/plug"
        );
        assert_eq!(
            HexPmRepositoryResource::tarball("plug", "1.16.1")
                .unwrap()
                .path(&public),
            "/tarballs/plug-1.16.1.tar"
        );
    }

    #[test]
    fn private_repository_paths_are_scoped_without_changing_resource_shape() {
        let private = HexPmRepositoryScope::private("acme").unwrap();
        assert_eq!(
            HexPmRepositoryResource::Names.path(&private),
            "/repos/acme/names"
        );
        assert_eq!(
            HexPmRepositoryResource::package("secret")
                .unwrap()
                .path(&private),
            "/repos/acme/packages/secret"
        );
    }

    #[test]
    fn trust_requirements_keep_resolution_fail_closed() {
        assert_eq!(
            HexPmRepositoryResource::Names.verification(),
            HexPmVerification::RegistryV2Signature
        );
        assert_eq!(
            HexPmRepositoryResource::tarball("plug", "1.0.0")
                .unwrap()
                .verification(),
            HexPmVerification::SignedMetadataChecksum
        );
        assert_eq!(
            HexPmRepositoryResource::PublicKey.verification(),
            HexPmVerification::OutOfBandTrustAnchor
        );
    }

    #[test]
    fn admin_api_paths_cover_lifecycle_and_ownership() {
        assert_eq!(
            api::retirement_path("plug", "1.0.0").unwrap(),
            "/packages/plug/releases/1.0.0/retire"
        );
        assert_eq!(
            api::owner_path("plug", "alice").unwrap(),
            "/packages/plug/owners/alice"
        );
        assert_eq!(
            api::organization_keys_path("acme").unwrap(),
            "/orgs/acme/keys"
        );
    }

    #[test]
    fn route_values_cannot_escape_their_segment() {
        assert!(HexPmRepositoryResource::package("../secret").is_err());
        assert!(HexPmRepositoryScope::private("acme/other").is_err());
        assert!(api::owner_path("plug", "a?admin=true").is_err());
    }
}
