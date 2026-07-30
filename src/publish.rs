use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A registry family that can receive one package produced by `zed publish`.
///
/// `zed` is zpkg.tech (or the CLI's configured Zed-compatible registry),
/// `native` is the ecosystem's canonical registry (npmjs, crates.io, PyPI,
/// and so on), and the remaining variants are package registries operated by
/// source forges. The source repository itself remains configured separately
/// under `[package.repository]`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum PublishRegistry {
    Zed,
    Native,
    GithubPackages,
    GitlabPackages,
    BitbucketPackages,
}

impl PublishRegistry {
    pub const ALL: [Self; 5] = [
        Self::Zed,
        Self::Native,
        Self::GithubPackages,
        Self::GitlabPackages,
        Self::BitbucketPackages,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Zed => "zed",
            Self::Native => "native",
            Self::GithubPackages => "github-packages",
            Self::GitlabPackages => "gitlab-packages",
            Self::BitbucketPackages => "bitbucket-packages",
        }
    }

    /// Whether this registry family currently accepts the package-manager
    /// format. Zed accepts every format because it stores the deterministic
    /// zpkg artifact; forge registries intentionally fail closed to the
    /// formats their public APIs document.
    pub fn supports_format(self, format: &str) -> bool {
        match self {
            Self::Zed => true,
            Self::Native => !matches!(format, "zpkg" | "generic"),
            Self::GithubPackages => {
                matches!(format, "npm" | "rubygems" | "maven" | "nuget" | "container")
            }
            Self::GitlabPackages => matches!(
                format,
                "composer"
                    | "conan"
                    | "debian"
                    | "generic"
                    | "go"
                    | "helm"
                    | "maven"
                    | "npm"
                    | "nuget"
                    | "pypi"
                    | "rubygems"
                    | "terraform"
            ),
            Self::BitbucketPackages => matches!(format, "npm" | "maven" | "container"),
        }
    }
}

impl fmt::Display for PublishRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsePublishRegistryError(pub String);

impl fmt::Display for ParsePublishRegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown publish registry `{}` (expected zed, native, github-packages, gitlab-packages, or bitbucket-packages)",
            self.0
        )
    }
}

impl std::error::Error for ParsePublishRegistryError {}

impl FromStr for PublishRegistry {
    type Err = ParsePublishRegistryError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "zed" => Ok(Self::Zed),
            "native" => Ok(Self::Native),
            "github-packages" | "github" => Ok(Self::GithubPackages),
            "gitlab-packages" | "gitlab" => Ok(Self::GitlabPackages),
            "bitbucket-packages" | "bitbucket" => Ok(Self::BitbucketPackages),
            other => Err(ParsePublishRegistryError(other.to_string())),
        }
    }
}
