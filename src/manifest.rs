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
    /// Monorepo workspace declaration (zed-docs issue #7); only meaningful in
    /// a workspace root manifest. When present, `zed install` at this root
    /// resolves every member against one store and writes one `.zpkg.lock`;
    /// member→member dependencies link by path instead of going through the
    /// registry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<WorkspaceSection>,
    /// Dependencies keyed by `org/name`, valued by a semver requirement.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dependencies: BTreeMap<String, String>,
    /// Tools needed only while running this package's `[build]` command
    /// (compilers, codegen). They are made available in the build sandbox and
    /// never linked into a consumer's `zed_modules/`. See [`BuildSection`]
    /// and zed-docs issue #5. Canonical TOML key is Cargo-style
    /// `[build-dependencies]`; the snake_case spelling is accepted on read.
    #[serde(
        default,
        rename = "build-dependencies",
        alias = "build_dependencies",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub build_dependencies: BTreeMap<String, String>,
    /// This package's own post-extract build step (compiled extensions,
    /// codegen), run when the package ships source that needs compiling.
    /// Builds run in an isolated staging copy — never inside the immutable
    /// source store — and results are cached per (sha256, target, command).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<BuildSection>,
    /// Consumer-side patches for dependencies (e.g. fixing a dependency's
    /// broken or missing `[build]` step without waiting on upstream).
    #[serde(default, skip_serializing_if = "OverridesSection::is_empty")]
    pub overrides: OverridesSection,
    /// Executables this package exposes, keyed by command name, valued by a
    /// path relative to the package root. On install they are hoisted into
    /// `zed_modules/.bin/` and runnable via `zed run <name>` (zed-docs
    /// issue #7).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bin: BTreeMap<String, String>,
    #[serde(default)]
    pub publish: PublishSection,
    #[serde(default)]
    pub scripts: ScriptsSection,
    /// Where zed materializes the (few, hand-picked) dependencies it sources —
    /// zed complements npm/maven/etc. rather than replacing them, so this dir
    /// sits alongside the native one and the ecosystem adapter wires it into
    /// the toolchain (NODE_PATH / node_modules, the JVM classpath, …). `dir`
    /// defaults to `zed_modules`; relocate it with e.g. `.vendor/.zed` or
    /// `.deps/.zed`.
    #[serde(default, skip_serializing_if = "InstallSection::is_empty")]
    pub install: InstallSection,
    /// Language subtrees for a **polyglot package** — one repo shipping the
    /// same library for several ecosystems (e.g. `node/`, `python/`, `go/`).
    /// Keyed by ecosystem name; the value says which subdirectory is that
    /// ecosystem's package root. On install the consumer resolves one target
    /// and only that subtree is materialized, so a Python project gets the
    /// Python source at its import root rather than a tree it has to reach
    /// into. Absent (the common case) = single-language package, whole tree.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub targets: BTreeMap<String, TargetSection>,
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
    /// Command run by `zed r2g` (alias `zed test-local`) inside a throwaway
    /// consumer project that has this package installed the same way a real
    /// consumer would.
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

/// Install-layout controls: where zed's dependency tree lands and which
/// ecosystem adapter to emit so those deps are visible to the native toolchain.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct InstallSection {
    /// Project-relative directory for the installed tree (`<dir>/<org>/<name>`).
    /// Defaults to `zed_modules`. Common overrides: `.vendor/.zed`, `.deps/.zed`.
    /// Must be a safe relative path (no leading `/`, no `..`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,
    /// Force the ecosystem adapter: `node`, `java`, or `none`. Omitted =
    /// auto-detect (or the CLI `--adapter`). Mirrors the CLI's values.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapter: Option<String>,
    /// Which language subtree to take from **polyglot** dependencies (see
    /// [`TargetSection`]). Omitted = infer from the project (`package.json` →
    /// `node`, `go.mod` → `go`, `pyproject.toml` → `python`, …). Naming a
    /// target a dependency does not publish is an error rather than a silent
    /// fallback: the consumer asked for something specific.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

impl InstallSection {
    pub fn is_empty(&self) -> bool {
        self.dir.is_none() && self.adapter.is_none() && self.target.is_none()
    }
}

/// One ecosystem's slice of a polyglot package.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TargetSection {
    /// Package-relative directory that is this ecosystem's package root, e.g.
    /// `python` or `clients/go`. Must be a safe relative path (no leading `/`,
    /// no `..`) so a target can never escape the package.
    pub dir: String,
}

/// A post-extract build step. Because compiled output is OS/arch-specific,
/// zed-pkg runs `command` via `sh -c` inside a sandboxed staging copy of the
/// source and caches the result in a build cache keyed by
/// `(source sha256, target triple, command)` — separate from the universal,
/// platform-independent source store (zed-docs issue #5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BuildSection {
    /// Command executed after extraction, e.g. `make` or `cargo build --release`.
    pub command: String,
    /// Paths (relative to the package root) to keep from the staging build.
    /// Empty means keep everything.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<String>,
}

/// Monorepo workspace membership. `members` are glob patterns (relative to
/// the workspace root) selecting directories that each contain a `.zpkg.toml`,
/// e.g. `["packages/*", "apps/*"]`. Dependencies that resolve to a member are
/// linked straight to the member's source directory instead of going through
/// the registry.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct WorkspaceSection {
    pub members: Vec<String>,
}

/// Consumer-side dependency patches, keyed by `org/name`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct OverridesSection {
    /// Replace or provide a dependency's `[build]` step.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub build: BTreeMap<String, BuildSection>,
}

impl OverridesSection {
    pub fn is_empty(&self) -> bool {
        self.build.is_empty()
    }
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
    #[error("invalid repository url `{0}`: {1}")]
    InvalidRepositoryUrl(String, String),
    #[error("invalid bin entry `{0}`: {1}")]
    InvalidBin(String, String),
    #[error("invalid build section: {0}")]
    InvalidBuild(String),
    #[error("invalid workspace member pattern `{0}`")]
    InvalidWorkspaceMember(String),
    #[error("invalid install dir `{0}`: {1}")]
    InvalidInstallDir(String, String),
    #[error("invalid target `{0}`: {1}")]
    InvalidTarget(String, String),
    #[error("manifest toml error: {0}")]
    Toml(String),
}

/// True for the lowercase slugs zed-pkg accepts as org and package names.
pub fn is_slug(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('-')
        && !s.ends_with('-')
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// True for a well-formed `org/name` dependency key.
pub fn is_dependency_key(key: &str) -> bool {
    let mut parts = key.splitn(2, '/');
    match (parts.next(), parts.next()) {
        (Some(org), Some(name)) => is_slug(org) && is_slug(name),
        _ => false,
    }
}

/// True for a 64-character lowercase-hex sha256 digest. Registry responses
/// and lockfiles feed digests into filesystem paths, so anything else is
/// rejected before it can reach the disk layer.
pub fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64
        && s.chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

/// True when `path` is a relative path with no `..` components — the only
/// shape allowed for manifest-declared paths (bin targets, build outputs).
/// Keeps hoisted bins and build outputs contained inside the package.
pub fn is_safe_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.starts_with('\\')
        && !path.contains('\0')
        // windows drive/UNC prefixes
        && !(path.len() >= 2 && path.as_bytes()[1] == b':')
        && !path
            .split(['/', '\\'])
            .any(|seg| seg == ".." || seg.is_empty())
}

fn is_allowed_repo_url(url: &str) -> bool {
    // The repo URL renders as a link in registry UIs and is shelled to VCS
    // tooling, so restrict it to the schemes those consumers expect.
    ["https://", "http://", "ssh://", "git://", "git+ssh://"]
        .iter()
        .any(|scheme| url.starts_with(scheme))
        // scp-like git syntax: git@github.com:org/repo.git
        || (url.contains('@') && url.contains(':') && !url.contains("://"))
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
        if !is_allowed_repo_url(&self.package.repository.url) {
            return Err(ManifestError::InvalidRepositoryUrl(
                self.package.repository.url.clone(),
                "expected an https/http/ssh/git URL or scp-like git syntax".to_string(),
            ));
        }
        for (key, req) in self.dependencies.iter().chain(&self.build_dependencies) {
            if !is_dependency_key(key) {
                return Err(ManifestError::InvalidDependencyKey(key.clone()));
            }
            // A requirement is either a semver range or an exact (opaque) tag.
            if req.trim().is_empty() {
                return Err(ManifestError::InvalidDependencyReq(
                    key.clone(),
                    req.clone(),
                    "requirement must not be empty".to_string(),
                ));
            }
            // Ranges that *look* like semver ranges but do not parse are
            // rejected rather than silently degrading to an opaque tag.
            if let Err(reason) = Requirement::validate(req) {
                return Err(ManifestError::InvalidDependencyReq(
                    key.clone(),
                    req.clone(),
                    reason,
                ));
            }
        }
        for (bin_name, target) in &self.bin {
            if bin_name.is_empty()
                || bin_name.starts_with('.')
                || bin_name
                    .chars()
                    .any(|c| !(c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.'))
            {
                return Err(ManifestError::InvalidBin(
                    bin_name.clone(),
                    "names use [A-Za-z0-9._-] and cannot start with `.`".to_string(),
                ));
            }
            if !is_safe_relative_path(target) {
                return Err(ManifestError::InvalidBin(
                    bin_name.clone(),
                    format!("target `{target}` must be a relative path without `..`"),
                ));
            }
        }
        let overriding = self.overrides.build.values();
        for build in self.build.iter().chain(overriding) {
            if build.command.trim().is_empty() {
                return Err(ManifestError::InvalidBuild(
                    "command must not be empty".to_string(),
                ));
            }
            for output in &build.outputs {
                if !is_safe_relative_path(output) {
                    return Err(ManifestError::InvalidBuild(format!(
                        "output `{output}` must be a relative path without `..`"
                    )));
                }
            }
        }
        for key in self.overrides.build.keys() {
            if !is_dependency_key(key) {
                return Err(ManifestError::InvalidDependencyKey(key.clone()));
            }
        }
        if let Some(ws) = &self.workspace {
            for pat in &ws.members {
                if pat.trim().is_empty() {
                    return Err(ManifestError::InvalidWorkspaceMember(pat.clone()));
                }
            }
        }
        if let Some(dir) = &self.install.dir
            && !is_safe_relative_path(dir)
        {
            return Err(ManifestError::InvalidInstallDir(
                dir.clone(),
                "must be a relative path without `..` or a leading `/`".to_string(),
            ));
        }
        Ok(())
    }

    /// Project-relative directory dependencies install into. Honors
    /// `[install].dir` (e.g. `.vendor/.zed`), else the default `zed_modules`.
    pub fn modules_dir(&self) -> &str {
        self.install
            .dir
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(crate::paths::MODULES_DIR)
    }

    /// True when this manifest declares a non-empty monorepo workspace.
    pub fn is_workspace_root(&self) -> bool {
        self.workspace
            .as_ref()
            .is_some_and(|w| !w.members.is_empty())
    }

    /// The effective build step for a dependency `org/name`: this manifest's
    /// `[overrides.build]` entry if present, else the dependency's own
    /// `[build]`. Returns `None` when neither declares one.
    pub fn effective_build(
        &self,
        dep_key: &str,
        dep_build: Option<&BuildSection>,
    ) -> Option<BuildSection> {
        self.overrides.build.get(dep_key).or(dep_build).cloned()
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
