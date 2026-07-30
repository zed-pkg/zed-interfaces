use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::publish::PublishRegistry;
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
    /// Native package-manager format (`npm`, `cargo`, `pypi`, `maven`, ...).
    /// Zed stores its own deterministic artifact regardless of this value;
    /// forge registries use it to reject unsupported routes before upload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// Registries that should receive this package. Omitted preserves the
    /// historical behavior: publish only to the configured Zed registry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registries: Option<Vec<PublishRegistry>>,
    /// Optional endpoint overrides, keyed by registry family. Secrets never
    /// belong here; authentication comes from the environment/credential
    /// store used by the relevant publisher.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub registry_urls: BTreeMap<PublishRegistry, String>,
}

impl Default for PublishSection {
    fn default() -> Self {
        Self {
            exclude: Vec::new(),
            include_readme: false,
            smoke_test: None,
            tag_format: "v{version}".to_string(),
            format: None,
            registries: None,
            registry_urls: BTreeMap::new(),
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

/// One ecosystem's slice of a polyglot package — and, on publish, its own
/// independently installable package.
///
/// A repo like `fiducia-clients` carrying `clients/ts`, `clients/java`, and
/// `clients/go` declares one target each. `zed publish` then emits **one
/// artifact per target**, named `<name>-<target>` by default:
///
/// ```text
/// fiducia/fiducia-clients-nodejs@1.1.2   <- clients/ts only
/// fiducia/fiducia-clients-java@1.1.2     <- clients/java only
/// fiducia/fiducia-clients-golang@1.1.2   <- clients/go only
/// ```
///
/// One source of truth and one version in the repo; N packages on the wire.
/// A Java consumer downloads only Java bytes — the decisive advantage over
/// shipping one fat artifact and slicing it at install time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TargetSection {
    /// Package-relative directory that is this ecosystem's package root, e.g.
    /// `python` or `clients/go`. Must be a safe relative path (no leading `/`,
    /// no `..`) so a target can never escape the package.
    pub dir: String,
    /// Published package name for this target. Defaults to
    /// `<package.name>-<target key>` (e.g. `fiducia-clients-java`). Set it to
    /// break out of the suffix convention when an ecosystem expects a
    /// different spelling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Ecosystem adapter consumers of THIS target should use (`node`, `java`,
    /// `none`). Recorded in the published per-target manifest so a consumer
    /// gets the right wiring without configuring it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter: Option<String>,
    /// This target's native package-manager format. It overrides
    /// `[publish].format` when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// This target's registry fan-out. It overrides `[publish].registries`
    /// when present; omission inherits the package default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registries: Option<Vec<PublishRegistry>>,
    /// Target-specific endpoint overrides layered over
    /// `[publish.registry_urls]`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub registry_urls: BTreeMap<PublishRegistry, String>,
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
    #[error("invalid publish route `{0}`: {1}")]
    InvalidPublishRoute(String, String),
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

/// True for a well-formed polyglot target name (`node`, `python`, `go`, …).
/// Same shape as a slug: these appear in manifests, CLI flags, and messages.
pub fn is_target_name(s: &str) -> bool {
    is_slug(s)
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

fn is_allowed_registry_url(url: &str) -> bool {
    (url.starts_with("https://") || url.starts_with("http://") || url.starts_with("file://"))
        && !url.chars().any(char::is_whitespace)
}

fn validate_publish_route(
    label: &str,
    format: Option<&str>,
    registries: Option<&[PublishRegistry]>,
    registry_urls: &BTreeMap<PublishRegistry, String>,
) -> Result<(), ManifestError> {
    let selected = registries.unwrap_or(&[PublishRegistry::Zed]);
    if selected.is_empty() {
        return Err(ManifestError::InvalidPublishRoute(
            label.to_string(),
            "registries cannot be empty; omit the field for the default Zed registry".to_string(),
        ));
    }
    let mut unique = std::collections::BTreeSet::new();
    for registry in selected {
        if !unique.insert(*registry) {
            return Err(ManifestError::InvalidPublishRoute(
                label.to_string(),
                format!("registry `{registry}` is listed more than once"),
            ));
        }
    }

    let format = format.map(str::trim).filter(|value| !value.is_empty());
    if let Some(format) = format
        && !is_target_name(format)
    {
        return Err(ManifestError::InvalidPublishRoute(
            label.to_string(),
            format!("format `{format}` must use [a-z0-9][a-z0-9-]*"),
        ));
    }
    for registry in selected {
        if *registry != PublishRegistry::Zed && format.is_none() {
            return Err(ManifestError::InvalidPublishRoute(
                label.to_string(),
                format!("registry `{registry}` requires an explicit package format"),
            ));
        }
        if let Some(format) = format
            && !registry.supports_format(format)
        {
            return Err(ManifestError::InvalidPublishRoute(
                label.to_string(),
                format!("registry `{registry}` does not support format `{format}`"),
            ));
        }
    }
    for (registry, url) in registry_urls {
        if !selected.contains(registry) {
            return Err(ManifestError::InvalidPublishRoute(
                label.to_string(),
                format!("URL override for `{registry}` has no matching entry in registries"),
            ));
        }
        if !is_allowed_registry_url(url) {
            return Err(ManifestError::InvalidPublishRoute(
                label.to_string(),
                format!("registry URL `{url}` must be an http(s) or file URL without whitespace"),
            ));
        }
    }
    Ok(())
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
        let mut target_dirs = BTreeMap::<&str, &str>::new();
        let mut published_names = BTreeMap::<String, &str>::new();
        for (name, target) in &self.targets {
            if !is_target_name(name) {
                return Err(ManifestError::InvalidTarget(
                    name.clone(),
                    "target names use [a-z0-9][a-z0-9-]* (e.g. `node`, `python`, `go`)".to_string(),
                ));
            }
            if !is_safe_relative_path(&target.dir) {
                return Err(ManifestError::InvalidTarget(
                    name.clone(),
                    format!("dir `{}` must be a relative path without `..`", target.dir),
                ));
            }
            if let Some(previous) = target_dirs.insert(target.dir.as_str(), name.as_str()) {
                return Err(ManifestError::InvalidTarget(
                    name.clone(),
                    format!(
                        "dir `{}` is already owned by target `{previous}`; every target must have an isolated source root",
                        target.dir
                    ),
                ));
            }
            let published_name = target
                .name
                .clone()
                .unwrap_or_else(|| format!("{}-{name}", self.package.name));
            if !is_slug(&published_name) {
                return Err(ManifestError::InvalidTarget(
                    name.clone(),
                    format!(
                        "published name `{published_name}` must match [a-z0-9][a-z0-9-]*[a-z0-9]"
                    ),
                ));
            }
            if published_name == self.package.name {
                return Err(ManifestError::InvalidTarget(
                    name.clone(),
                    format!(
                        "published name `{published_name}` collides with the polyglot source package"
                    ),
                ));
            }
            if let Some(previous) = published_names.insert(published_name.clone(), name.as_str()) {
                return Err(ManifestError::InvalidTarget(
                    name.clone(),
                    format!(
                        "published name `{published_name}` is already used by target `{previous}`"
                    ),
                ));
            }
            if let Some(adapter) = target.adapter.as_deref()
                && !matches!(adapter, "node" | "java" | "none")
            {
                return Err(ManifestError::InvalidTarget(
                    name.clone(),
                    format!(
                        "adapter `{adapter}` is unsupported; expected `node`, `java`, or `none`"
                    ),
                ));
            }
        }
        if self.targets.is_empty() {
            validate_publish_route(
                "package",
                self.publish.format.as_deref(),
                self.publish.registries.as_deref(),
                &self.publish.registry_urls,
            )?;
        } else {
            for (name, target) in &self.targets {
                let format = target.format.as_deref().or(self.publish.format.as_deref());
                let registries = target
                    .registries
                    .as_deref()
                    .or(self.publish.registries.as_deref());
                let mut urls = self.publish.registry_urls.clone();
                urls.extend(target.registry_urls.clone());
                validate_publish_route(name, format, registries, &urls)?;
            }
        }
        // A blank request means "no target", the same way a blank
        // `[install].dir` falls back to the default rather than erroring.
        if let Some(requested) = self.requested_target()
            && !is_target_name(requested)
        {
            return Err(ManifestError::InvalidTarget(
                requested.to_string(),
                "target names use [a-z0-9][a-z0-9-]*".to_string(),
            ));
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

    /// The consumer's requested polyglot target, if it named one explicitly.
    pub fn requested_target(&self) -> Option<&str> {
        self.install
            .target
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }

    /// True when this package ships per-ecosystem subtrees.
    pub fn is_polyglot(&self) -> bool {
        !self.targets.is_empty()
    }

    /// The package name a target publishes under: its explicit `name`, else
    /// the `<name>-<target>` convention (`fiducia-clients` + `java` →
    /// `fiducia-clients-java`).
    pub fn target_package_name(&self, target: &str) -> Option<String> {
        self.targets.get(target).map(|t| {
            t.name
                .clone()
                .unwrap_or_else(|| format!("{}-{}", self.package.name, target))
        })
    }

    /// Every `(target, published name)` pair this manifest fans out to, sorted
    /// by target for deterministic publish order and output.
    pub fn target_package_names(&self) -> Vec<(String, String)> {
        let mut names: Vec<(String, String)> = self
            .targets
            .keys()
            .filter_map(|target| {
                self.target_package_name(target)
                    .map(|name| (target.clone(), name))
            })
            .collect();
        names.sort();
        names
    }

    /// Derive the per-target manifest that ships *inside* that target's
    /// artifact: same org/version/repo, the target's own package name, its
    /// adapter, and no `[targets]` (the slice is single-language by
    /// construction). Dependencies are carried over so a target's own zed
    /// deps still resolve.
    pub fn manifest_for_target(&self, target: &str) -> Option<Manifest> {
        let name = self.target_package_name(target)?;
        let section = self.targets.get(target)?;
        let mut derived = self.clone();
        derived.package.name = name;
        derived.package.description = Some(match &self.package.description {
            Some(base) => format!("{base} ({target})"),
            None => format!("{} ({target} client)", self.package.name),
        });
        derived.targets = BTreeMap::new();
        derived.workspace = None;
        // The consumer-facing wiring for this ecosystem.
        derived.install.adapter = section.adapter.clone().or(self.install.adapter.clone());
        derived.install.target = None;
        derived.publish.format = section.format.clone().or(self.publish.format.clone());
        if section.registries.is_some() {
            derived.publish.registries = section.registries.clone();
        }
        derived
            .publish
            .registry_urls
            .extend(section.registry_urls.clone());
        Some(derived)
    }

    /// The effective registry fan-out for this package. Manifests written
    /// before multi-registry support continue to resolve to Zed only.
    pub fn publish_registries(&self) -> Vec<PublishRegistry> {
        self.publish
            .registries
            .clone()
            .unwrap_or_else(|| vec![PublishRegistry::Zed])
    }

    /// An endpoint override for a registry family, when the manifest names
    /// one. The CLI's `--registry` remains the default for `zed`.
    pub fn publish_registry_url(&self, registry: PublishRegistry) -> Option<&str> {
        self.publish
            .registry_urls
            .get(&registry)
            .map(String::as_str)
    }

    /// Resolve which subdirectory of *this* (dependency) package a consumer
    /// asking for `requested` should get.
    ///
    /// * Not polyglot → `Ok(None)`: the whole tree, exactly as before.
    /// * Polyglot + a matching target → `Ok(Some(dir))`.
    /// * Polyglot + `requested` names a target this package does not publish
    ///   → `Err` listing what it does publish. An explicit request that cannot
    ///   be honored is a mistake worth surfacing, not something to paper over
    ///   by installing a tree the consumer's toolchain cannot read.
    /// * Polyglot + nothing requested → `Ok(None)` (whole tree), so a consumer
    ///   that has not opted in keeps working.
    pub fn target_subdir(&self, requested: Option<&str>) -> Result<Option<&str>, ManifestError> {
        if self.targets.is_empty() {
            return Ok(None);
        }
        let Some(requested) = requested else {
            return Ok(None);
        };
        match self.targets.get(requested) {
            Some(target) => Ok(Some(target.dir.as_str())),
            None => {
                let mut available: Vec<&str> = self.targets.keys().map(String::as_str).collect();
                available.sort_unstable();
                Err(ManifestError::InvalidTarget(
                    requested.to_string(),
                    format!(
                        "package `{}/{}` publishes no such target; it provides: {}",
                        self.package.org,
                        self.package.name,
                        available.join(", ")
                    ),
                ))
            }
        }
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
