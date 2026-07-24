use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::vcs::Vcs;
use crate::version::{Requirement, VersionScheme};

/// The `.zpkg.toml` manifest at the root of every package repository.
/// TOML only — never YAML or JSON.
///
/// ```toml
/// [package]
/// org = "acme"
/// name = "http-kit"
/// version = "1.2.0"
/// description = "Tiny HTTP helpers"
/// license = "MIT"
///
/// [package.repository]
/// vcs = "git"
/// url = "https://github.com/acme/http-kit"
///
/// [dependencies]
/// "acme/logkit" = "^0.3"
///
/// [publish]
/// exclude = ["benches/**"]
/// smoke_test = "sh scripts/smoke.sh"
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Manifest {
    pub package: PackageSection,
    /// Monorepo workspace declaration (zed-docs issue #7). When present,
    /// `zed install` at this root resolves every member against one store and
    /// writes one `.zpkg.lock`; member→member dependencies link by path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<WorkspaceSection>,
    /// Dependencies keyed by `org/name`, valued by a semver requirement.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dependencies: BTreeMap<String, String>,
    /// Tools needed only to *build* this package (compilers, codegen). They
    /// are made available in the build sandbox and never linked into a
    /// consumer's `zed_modules/`. See [`BuildSection`] and zed-docs issue #5.
    #[serde(
        default,
        rename = "build-dependencies",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub build_dependencies: BTreeMap<String, String>,
    /// This package's own build step, run after extraction on the consumer's
    /// machine when the package ships source that needs compiling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<BuildSection>,
    /// Consumer-side patches to a *dependency's* build step, keyed by
    /// `org/name`. Lets a project fix a broken/missing upstream build locally.
    #[serde(
        default,
        rename = "build-overrides",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub build_overrides: BTreeMap<String, BuildSection>,
    /// Executables this package exposes, `name -> path relative to the package
    /// root`. On install they are hoisted into `zed_modules/.bin/` and run via
    /// `zed run <name>` (zed-docs issue #7).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bin: BTreeMap<String, String>,
    #[serde(default)]
    pub publish: PublishSection,
    #[serde(default)]
    pub scripts: ScriptsSection,
}

/// A build step: the command to run after extraction, and the artifacts to
/// expose. Because compiled output is OS/arch-specific, zed-pkg runs this in a
/// sandbox and caches the result in a build cache keyed by
/// `(source sha256, target triple, command)` — separate from the universal,
/// platform-independent source store (zed-docs issue #5).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BuildSection {
    /// Command executed with `sh -c` in the sandboxed copy of the source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Files (relative paths) to expose to consumers. When empty, the whole
    /// built tree is exposed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PackageSection {
    /// Namespace the package is published under. Lowercase slug.
    pub org: String,
    /// Package name, unique within the org. Lowercase slug.
    pub name: String,
    /// Version of this package, interpreted according to `version_scheme`
    /// (semver by default).
    pub version: String,
    /// How `version` (and published tags) should be interpreted. Semver by
    /// default; `calver` for calendar versions, `opaque` for arbitrary tags.
    #[serde(
        default,
        skip_serializing_if = "crate::version::VersionScheme::is_default"
    )]
    pub version_scheme: VersionScheme,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    pub repository: RepositorySection,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
}

/// Where the package's source of truth lives. Any Git or Mercurial host
/// works: GitHub, GitLab, Bitbucket, or a self-hosted server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RepositorySection {
    #[serde(default)]
    pub vcs: Vcs,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct PublishSection {
    /// Extra glob patterns to exclude on top of the built-in defaults.
    pub exclude: Vec<String>,
    /// Keep README files in the published artifact (stripped by default).
    pub include_readme: bool,
    /// Command run by `zed test-local` inside a throwaway consumer project
    /// that has this package installed the same way a real consumer would.
    pub smoke_test: Option<String>,
    /// VCS tag template that must exist and point at the published commit.
    /// `{version}` is substituted with `package.version`.
    pub tag_format: String,
}

impl Default for PublishSection {
    fn default() -> Self {
        Self {
            exclude: Vec::new(),
            include_readme: false,
            smoke_test: None,
            tag_format: "v{version}".to_string(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ScriptsSection {
    /// Test command run from the repository (not from published artifacts;
    /// tests are stripped at publish time).
    pub test: Option<String>,
}

/// Monorepo workspace membership. `members` are glob patterns (relative to
/// the workspace root) selecting directories that each contain a `.zpkg.toml`,
/// e.g. `["packages/*", "apps/*"]`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct WorkspaceSection {
    pub members: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("invalid org slug `{0}`: must match [a-z0-9][a-z0-9-]*[a-z0-9]")]
    InvalidOrg(String),
    #[error("invalid package name `{0}`: must match [a-z0-9][a-z0-9-]*[a-z0-9]")]
    InvalidName(String),
    #[error("invalid version `{0}`: {1}")]
    InvalidVersion(String, String),
    #[error("invalid dependency key `{0}`: expected `org/name`")]
    InvalidDependencyKey(String),
    #[error("invalid requirement `{1}` for dependency `{0}`: {2}")]
    InvalidDependencyReq(String, String, String),
    #[error("invalid bin `{0}`: {1}")]
    InvalidBin(String, String),
    #[error("invalid workspace member pattern `{0}`")]
    InvalidWorkspaceMember(String),
    #[error("manifest toml error: {0}")]
    Toml(String),
}

/// True for a relative path that stays within the package (no absolute paths,
/// no `..` traversal). Used to keep hoisted bins and build outputs contained.
pub fn is_safe_relative_path(path: &str) -> bool {
    let p = std::path::Path::new(path);
    !path.is_empty()
        && p.is_relative()
        && p.components().all(|c| {
            matches!(
                c,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
}

/// True for a well-formed `org/name` dependency key.
pub fn is_dependency_key(key: &str) -> bool {
    let mut parts = key.splitn(2, '/');
    match (parts.next(), parts.next()) {
        (Some(org), Some(name)) => is_slug(org) && is_slug(name),
        _ => false,
    }
}

/// True for the lowercase slugs zed-pkg accepts as org and package names.
pub fn is_slug(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('-')
        && !s.ends_with('-')
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

impl Manifest {
    /// Parse and validate a `.zpkg.toml` document.
    pub fn parse(input: &str) -> Result<Self, ManifestError> {
        let manifest: Manifest =
            toml::from_str(input).map_err(|e| ManifestError::Toml(e.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn to_toml_string(&self) -> Result<String, ManifestError> {
        toml::to_string_pretty(self).map_err(|e| ManifestError::Toml(e.to_string()))
    }

    pub fn validate(&self) -> Result<(), ManifestError> {
        if !is_slug(&self.package.org) {
            return Err(ManifestError::InvalidOrg(self.package.org.clone()));
        }
        if !is_slug(&self.package.name) {
            return Err(ManifestError::InvalidName(self.package.name.clone()));
        }
        self.package
            .version_scheme
            .validate_version(&self.package.version)
            .map_err(|e| ManifestError::InvalidVersion(self.package.version.clone(), e))?;
        for (key, req) in &self.dependencies {
            let mut parts = key.splitn(2, '/');
            let (org, name) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
            if !is_slug(org) || !is_slug(name) {
                return Err(ManifestError::InvalidDependencyKey(key.clone()));
            }
            // A requirement is either a semver range or an exact (opaque) tag;
            // only an empty string is invalid.
            if req.trim().is_empty() {
                return Err(ManifestError::InvalidDependencyReq(
                    key.clone(),
                    req.clone(),
                    "requirement must not be empty".to_string(),
                ));
            }
            let _ = Requirement::parse(req);
        }
        for (key, req) in &self.build_dependencies {
            if !is_dependency_key(key) {
                return Err(ManifestError::InvalidDependencyKey(key.clone()));
            }
            if req.trim().is_empty() {
                return Err(ManifestError::InvalidDependencyReq(
                    key.clone(),
                    req.clone(),
                    "build-dependency requirement must not be empty".to_string(),
                ));
            }
        }
        for key in self.build_overrides.keys() {
            if !is_dependency_key(key) {
                return Err(ManifestError::InvalidDependencyKey(key.clone()));
            }
        }
        for (name, path) in &self.bin {
            if name.trim().is_empty() || name.contains('/') || name.contains('\\') {
                return Err(ManifestError::InvalidBin(
                    name.clone(),
                    "bin name must be non-empty with no path separators".to_string(),
                ));
            }
            if !is_safe_relative_path(path) {
                return Err(ManifestError::InvalidBin(
                    name.clone(),
                    format!("bin path `{path}` must be relative and stay inside the package"),
                ));
            }
        }
        if let Some(ws) = &self.workspace {
            for pat in &ws.members {
                if pat.trim().is_empty() {
                    return Err(ManifestError::InvalidWorkspaceMember(pat.clone()));
                }
            }
        }
        Ok(())
    }

    /// True when this manifest declares a non-empty monorepo workspace.
    pub fn is_workspace_root(&self) -> bool {
        self.workspace
            .as_ref()
            .is_some_and(|w| !w.members.is_empty())
    }

    /// The effective build step for a dependency `org/name`: this manifest's
    /// `[build-overrides]` entry if present, else the dependency's own
    /// `[build]`. Returns `None` when neither declares a build command.
    pub fn effective_build(
        &self,
        dep_key: &str,
        dep_build: Option<&BuildSection>,
    ) -> Option<BuildSection> {
        self.build_overrides
            .get(dep_key)
            .or(dep_build)
            .filter(|b| b.command.is_some())
            .cloned()
    }

    /// `org/name`, the canonical package identifier.
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.package.org, self.package.name)
    }

    /// Parsed semver version. Only call after `validate()` has passed.
    pub fn version(&self) -> Result<semver::Version, ManifestError> {
        semver::Version::parse(&self.package.version)
            .map_err(|e| ManifestError::InvalidVersion(self.package.version.clone(), e.to_string()))
    }

    /// The VCS tag that must exist for this version, e.g. `v1.2.0`.
    pub fn vcs_tag(&self) -> String {
        self.publish
            .tag_format
            .replace("{version}", &self.package.version)
    }
}
