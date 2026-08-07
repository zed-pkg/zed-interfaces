use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Version-control systems a package's source repository can live on.
///
/// zed-pkg is VCS-agnostic by design: what gets installed is always a
/// registry artifact, and the VCS is where provenance (tags) is anchored.
/// Authors must create a matching tag on their declared backing repo
/// (GitHub, GitLab, Bitbucket, Codeberg, SourceHut, Forgejo, Gitea, Azure
/// DevOps, CodeCommit, Radicle, or self-hosted) before publishing.
///
/// `jj` and Sapling are git-compatible and push to git remotes, so their
/// provenance is verified through git tags.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Vcs {
    #[default]
    Git,
    Hg,
    Jj,
    Sapling,
    Fossil,
    Pijul,
}

impl Vcs {
    pub fn as_str(&self) -> &'static str {
        match self {
            Vcs::Git => "git",
            Vcs::Hg => "hg",
            Vcs::Jj => "jj",
            Vcs::Sapling => "sapling",
            Vcs::Fossil => "fossil",
            Vcs::Pijul => "pijul",
        }
    }

    /// True for systems whose tags live in git ref namespaces (git itself
    /// plus the git-compatible layers), so git-based tag verification works.
    pub fn uses_git_tags(&self) -> bool {
        matches!(self, Vcs::Git | Vcs::Jj | Vcs::Sapling)
    }
}

impl fmt::Display for Vcs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, thiserror::Error)]
#[error("unknown vcs `{0}` (expected git, hg, jj, sapling, fossil, or pijul)")]
pub struct ParseVcsError(pub String);

impl FromStr for Vcs {
    type Err = ParseVcsError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "git" => Ok(Vcs::Git),
            "hg" => Ok(Vcs::Hg),
            "jj" => Ok(Vcs::Jj),
            "sapling" => Ok(Vcs::Sapling),
            "fossil" => Ok(Vcs::Fossil),
            "pijul" => Ok(Vcs::Pijul),
            other => Err(ParseVcsError(other.to_string())),
        }
    }
}
