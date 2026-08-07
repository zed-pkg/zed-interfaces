use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::language::{Ecosystem, Language};
use crate::nix::NixExportSection;
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
    /// Host-native packages required before this package's install hooks or
    /// build step can run. Keys are supported package-manager names (`apt`,
    /// `apk`, `brew`, `nix`, ...); values are package specs passed as argv,
    /// never interpolated into a shell command. Installing these prerequisites
    /// is an explicitly consented zed operation, separate from build-hook
    /// consent.
    #[serde(
        default,
        rename = "native-dependencies",
        alias = "native_dependencies",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub native_dependencies: NativeDependencies,
    /// Package-local lifecycle hooks. They run in a writable staging copy,
    /// never in the immutable source store and never in the consumer project.
    /// `pre-install` runs before `[build]`; `post-install` runs after it and
    /// before the finalized artifact is promoted to the platform cache.
    #[serde(default, skip_serializing_if = "InstallHooksSection::is_empty")]
    pub hooks: InstallHooksSection,
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
    /// The language this package's code targets. A multi-language repository
    /// publishes one package per language, all at one version, and each names
    /// its own — `acme-clients-java`, `acme-clients-nodejs`. Defaults to
    /// `universal` (language-agnostic), which is never subject to the install
    /// ecosystem guard.
    #[serde(default, skip_serializing_if = "Language::is_default")]
    pub language: Language,
    /// How consumers take this package in: `jvm`, `npm`, `gomod`, … Drives the
    /// install-time guard that refuses to drop a Java client into a Node
    /// project, and picks the toolchain wiring `zed install` writes.
    ///
    /// Omit it and it is derived from `language` (see
    /// [`Language::ecosystem`]) — declare it only when the language does not
    /// determine consumption. Read through [`PackageSection::ecosystem`]
    /// rather than touching this field, so the fallback always applies.
    #[serde(default, skip_serializing_if = "Ecosystem::is_default")]
    pub ecosystem: Ecosystem,
}

impl PackageSection {
    /// The effective ecosystem: the explicit `ecosystem` when declared, else
    /// the one implied by `language`.
    ///
    /// Always use this instead of reading the field directly — a manifest that
    /// says only `language = "java"` must still guard as `jvm`.
    pub fn ecosystem(&self) -> Ecosystem {
        if self.ecosystem.is_default() {
            self.language.ecosystem()
        } else {
            self.ecosystem
        }
    }
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
    /// Optional native-registry route for a single-language package whose
    /// package-manager manifest lives at the repository root. Polyglot
    /// packages declare this metadata on each `[targets.*.native]` section
    /// instead.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native: Option<NativeReleaseSection>,
    /// Optional deterministic export of this single-language package as a
    /// standalone Nix flake. Nix is a typed interop adapter, not a native
    /// registry destination, so its intent remains separate from `native`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nix: Option<NixExportSection>,
}

impl Default for PublishSection {
    fn default() -> Self {
        Self {
            exclude: Vec::new(),
            include_readme: false,
            smoke_test: None,
            tag_format: "v{version}".to_string(),
            native: None,
            nix: None,
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
    /// Optional routing to this target's native ecosystem registry. This is
    /// declarative metadata only: the native manifest remains authoritative,
    /// and arbitrary commands are intentionally not representable here.
    ///
    /// Note the difference from [`TargetSection::ecosystem`]: `native` is
    /// **outbound** (where this slice is mirrored to — npm, crates.io), while
    /// `ecosystem` is **inbound** (what toolchain a consumer must have to
    /// install it from zed). They describe the same ecosystem from two sides,
    /// so when both are set they must agree — see
    /// [`NativeRegistry::ecosystem`] and the check in [`Manifest::validate`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native: Option<NativeReleaseSection>,
    /// Optional deterministic Nix export intent for this isolated target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nix: Option<NixExportSection>,
    /// Native prerequisites added by this target. Entries merge with the
    /// package-level `[native-dependencies]` table when this target is selected.
    #[serde(
        default,
        rename = "native-dependencies",
        alias = "native_dependencies",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub native_dependencies: NativeDependencies,
    /// Target-specific lifecycle hooks, appended after package-level hooks in
    /// each phase when the target is selected.
    #[serde(default, skip_serializing_if = "InstallHooksSection::is_empty")]
    pub hooks: InstallHooksSection,
    /// Override the ecosystem this target publishes into. Omit it (the normal
    /// case) and it is derived from the target key via [`Language::ecosystem`].
    /// Declare it when the key does not determine consumption — a `rust-wasm`
    /// target is consumed by a JS bundler, not by Cargo.
    #[serde(default, skip_serializing_if = "Ecosystem::is_default")]
    pub ecosystem: Ecosystem,
}

impl TargetSection {
    /// The language this target ships, from its explicit key. `universal` when
    /// the key is not a language zed knows — such a target still publishes and
    /// installs, it just is not ecosystem-gated.
    pub fn language_for(&self, target_key: &str) -> Language {
        Language::from_token(target_key).unwrap_or_default()
    }

    /// The ecosystem a consumer must have to install this target: the explicit
    /// `ecosystem`, else the one implied by the target key.
    pub fn ecosystem_for(&self, target_key: &str) -> Ecosystem {
        if !self.ecosystem.is_default() {
            return self.ecosystem;
        }
        self.language_for(target_key).ecosystem()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NativeReleaseSection {
    pub registry: NativeRegistry,
    pub package: String,
    /// Optional VCS tag template for native ecosystems whose package is
    /// resolved from a tag rather than uploaded. It must contain `{version}`.
    /// Go modules below a repository subdirectory need the directory prefix,
    /// for example `clients/go/v{version}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag_format: Option<String>,
    /// Optional copies in package registries run by source forges. The native
    /// registry remains the canonical ecosystem destination; these mirrors
    /// use the same native package format and version.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forge: Vec<ForgeRegistry>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum NativeRegistry {
    Npm,
    CratesIo,
    #[serde(rename = "pub.dev")]
    PubDev,
    #[serde(rename = "pypi")]
    PyPi,
    MavenCentral,
    #[serde(rename = "rubygems")]
    RubyGems,
    #[serde(rename = "nuget")]
    NuGet,
    Packagist,
    GoModules,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum ForgeRegistry {
    GithubPackages,
    GitlabPackages,
    BitbucketPackages,
}

impl ForgeRegistry {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GithubPackages => "github-packages",
            Self::GitlabPackages => "gitlab-packages",
            Self::BitbucketPackages => "bitbucket-packages",
        }
    }

    /// Package-manager protocols currently documented by each forge. Failing
    /// closed here prevents a manifest from promising e.g. Cargo support in
    /// GitHub Packages when that registry has no Cargo endpoint.
    pub fn supports(self, native: NativeRegistry) -> bool {
        match self {
            Self::GithubPackages => matches!(
                native,
                NativeRegistry::Npm
                    | NativeRegistry::MavenCentral
                    | NativeRegistry::RubyGems
                    | NativeRegistry::NuGet
            ),
            Self::GitlabPackages => matches!(
                native,
                NativeRegistry::Npm
                    | NativeRegistry::PyPi
                    | NativeRegistry::MavenCentral
                    | NativeRegistry::RubyGems
                    | NativeRegistry::NuGet
                    | NativeRegistry::Packagist
                    | NativeRegistry::GoModules
            ),
            Self::BitbucketPackages => {
                matches!(native, NativeRegistry::Npm | NativeRegistry::MavenCentral)
            }
        }
    }
}

impl std::fmt::Display for ForgeRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl NativeRegistry {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Npm => "npm",
            Self::CratesIo => "crates-io",
            Self::PubDev => "pub.dev",
            Self::PyPi => "pypi",
            Self::MavenCentral => "maven-central",
            Self::RubyGems => "rubygems",
            Self::NuGet => "nuget",
            Self::Packagist => "packagist",
            Self::GoModules => "go-modules",
        }
    }

    fn validate_package(self, package: &str) -> Result<(), String> {
        let valid = match self {
            Self::Npm => is_valid_npm_package(package),
            Self::CratesIo => is_valid_crates_package(package),
            Self::PubDev => is_valid_pubdev_package(package),
            Self::PyPi => is_valid_pypi_package(package),
            Self::MavenCentral => is_valid_maven_package(package),
            Self::RubyGems => is_valid_rubygems_package(package),
            Self::NuGet => is_valid_nuget_package(package),
            Self::Packagist => is_valid_packagist_package(package),
            Self::GoModules => is_valid_go_module(package),
        };
        if valid {
            Ok(())
        } else {
            Err(format!(
                "package `{package}` is not a valid {} package identity",
                self.as_str()
            ))
        }
    }

    fn canonical_package(self, package: &str) -> String {
        match self {
            Self::PyPi => normalize_pypi_package(package),
            Self::NuGet => package.to_ascii_lowercase(),
            _ => package.to_string(),
        }
    }

    /// The zed [`Ecosystem`] this native registry corresponds to.
    ///
    /// The two enums describe the same thing from opposite directions —
    /// `NativeRegistry` is where a slice is *mirrored to*, `Ecosystem` is what a
    /// consumer needs to *install* it — so a target declaring both must not
    /// disagree. Mapping them here keeps that check in one place instead of
    /// letting each caller re-derive it.
    pub fn ecosystem(self) -> Ecosystem {
        match self {
            Self::Npm => Ecosystem::Npm,
            Self::CratesIo => Ecosystem::Cargo,
            Self::PubDev => Ecosystem::Pub,
            Self::PyPi => Ecosystem::Pypi,
            Self::MavenCentral => Ecosystem::Jvm,
            Self::RubyGems => Ecosystem::Gem,
            Self::NuGet => Ecosystem::Nuget,
            Self::Packagist => Ecosystem::Composer,
            Self::GoModules => Ecosystem::Gomod,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NativeReleaseRoute {
    pub target: String,
    pub dir: String,
    pub registry: NativeRegistry,
    pub package: String,
    pub vcs_tag: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ForgeReleaseRoute {
    pub target: String,
    pub dir: String,
    pub registry: ForgeRegistry,
    pub format: NativeRegistry,
    pub package: String,
    pub vcs_tag: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NixExportRoute {
    pub target: String,
    pub dir: String,
    pub package: String,
    pub intent: NixExportSection,
}

fn is_valid_npm_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 214
        && !value.starts_with('.')
        && !value.starts_with('_')
        && !value.contains("..")
        && value.chars().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_' | '.' | '~')
        })
}

fn is_valid_npm_package(value: &str) -> bool {
    if let Some(scoped) = value.strip_prefix('@') {
        let Some((scope, package)) = scoped.split_once('/') else {
            return false;
        };
        !package.contains('/') && is_valid_npm_component(scope) && is_valid_npm_component(package)
    } else {
        !value.contains('/') && is_valid_npm_component(value)
    }
}

fn is_valid_crates_package(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && !value.starts_with('-')
        && !value.starts_with('_')
        && !value.ends_with('-')
        && !value.ends_with('_')
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

fn is_valid_pypi_package(value: &str) -> bool {
    !value.is_empty()
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value.as_bytes()[value.len() - 1].is_ascii_alphanumeric()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn normalize_pypi_package(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let mut separator = false;
    for byte in value.bytes() {
        if matches!(byte, b'.' | b'_' | b'-') {
            separator = true;
            continue;
        }
        if separator && !normalized.is_empty() {
            normalized.push('-');
        }
        separator = false;
        normalized.push((byte as char).to_ascii_lowercase());
    }
    normalized
}

fn is_valid_pubdev_package(value: &str) -> bool {
    const RESERVED: &[&str] = &[
        "assert", "break", "case", "catch", "class", "const", "continue", "default", "do", "else",
        "enum", "extends", "false", "final", "finally", "for", "if", "in", "is", "new", "null",
        "rethrow", "return", "super", "switch", "this", "throw", "true", "try", "var", "void",
        "while", "with", "async", "await", "yield",
    ];
    !value.is_empty()
        && !value.as_bytes()[0].is_ascii_digit()
        && !RESERVED.contains(&value)
        && value
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn is_valid_maven_component(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with(['.', '-'])
        && !value.ends_with(['.', '-'])
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

fn is_valid_maven_package(value: &str) -> bool {
    let Some((group, artifact)) = value.split_once(':') else {
        return false;
    };
    !artifact.contains(':') && is_valid_maven_component(group) && is_valid_maven_component(artifact)
}

fn is_valid_rubygems_package(value: &str) -> bool {
    !value.is_empty()
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value.as_bytes()[value.len() - 1].is_ascii_alphanumeric()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn is_valid_nuget_package(value: &str) -> bool {
    value.len() <= 100 && is_valid_rubygems_package(value)
}

fn is_valid_packagist_package(value: &str) -> bool {
    let Some((vendor, package)) = value.split_once('/') else {
        return false;
    };
    !package.contains('/') && is_valid_npm_component(vendor) && is_valid_npm_component(package)
}

fn is_valid_go_module(value: &str) -> bool {
    !value.is_empty()
        && value.contains('/')
        && !value.starts_with('/')
        && !value.ends_with('/')
        && !value.contains("..")
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '/' | '-' | '_' | '~'))
}

/// Supported host package managers for `[native-dependencies]`.
///
/// The manifest names *packages*, never an installer command. zed maps these
/// stable identifiers to fixed argv templates and rejects unknown keys, so a
/// package cannot disguise arbitrary privileged shell execution as dependency
/// installation.
pub const NATIVE_PACKAGE_MANAGERS: &[&str] = &[
    "apk", "apt", "brew", "choco", "dnf", "nix", "pacman", "pkg", "port", "scoop", "winget",
    "xbps", "yum", "zypper",
];

/// Manager name to package specs. A manager may intentionally map to an empty
/// list to state that it is supported without adding target-specific packages.
pub type NativeDependencies = BTreeMap<String, Vec<String>>;

/// Package-local lifecycle hooks. Commands are author-controlled shell code,
/// but execution is separately consented by the installer and occurs only in
/// a writable staging tree.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct InstallHooksSection {
    #[serde(
        rename = "pre-install",
        alias = "pre_install",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub pre_install: Vec<String>,
    #[serde(
        rename = "post-install",
        alias = "post_install",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub post_install: Vec<String>,
}

impl InstallHooksSection {
    pub fn is_empty(&self) -> bool {
        self.pre_install.is_empty() && self.post_install.is_empty()
    }

    /// Package hooks run before target-specific hooks in each phase.
    pub fn merged(&self, target: &Self) -> Self {
        let mut merged = self.clone();
        merged
            .pre_install
            .extend(target.pre_install.iter().cloned());
        merged
            .post_install
            .extend(target.post_install.iter().cloned());
        merged
    }
}

fn merged_native_dependencies(
    package: &NativeDependencies,
    target: &NativeDependencies,
) -> NativeDependencies {
    let mut merged = package.clone();
    for (manager, packages) in target {
        let existing = merged.entry(manager.clone()).or_default();
        let mut seen: BTreeSet<String> = existing.iter().cloned().collect();
        for package in packages {
            if seen.insert(package.clone()) {
                existing.push(package.clone());
            }
        }
    }
    merged
}

fn validate_native_dependencies(
    dependencies: &NativeDependencies,
    context: &str,
) -> Result<(), ManifestError> {
    for (manager, packages) in dependencies {
        if !NATIVE_PACKAGE_MANAGERS.contains(&manager.as_str()) {
            return Err(ManifestError::InvalidNativeDependency(
                context.to_string(),
                format!(
                    "package manager `{manager}` is unsupported; expected one of {}",
                    NATIVE_PACKAGE_MANAGERS.join(", ")
                ),
            ));
        }
        let mut seen = BTreeSet::new();
        for package in packages {
            if package.trim() != package
                || package.is_empty()
                || package.len() > 256
                || package.starts_with('-')
                || package.chars().any(char::is_whitespace)
                || package.chars().any(char::is_control)
            {
                return Err(ManifestError::InvalidNativeDependency(
                    context.to_string(),
                    format!(
                        "invalid `{manager}` package spec `{package}`; specs must be 1-256 non-whitespace, non-control characters and cannot begin with `-`"
                    ),
                ));
            }
            if !seen.insert(package) {
                return Err(ManifestError::InvalidNativeDependency(
                    context.to_string(),
                    format!("duplicate `{manager}` package spec `{package}`"),
                ));
            }
        }
    }
    Ok(())
}

fn validate_install_hooks(hooks: &InstallHooksSection, context: &str) -> Result<(), ManifestError> {
    for (phase, commands) in [
        ("pre-install", &hooks.pre_install),
        ("post-install", &hooks.post_install),
    ] {
        for (index, command) in commands.iter().enumerate() {
            if command.trim().is_empty() {
                return Err(ManifestError::InvalidInstallHook(
                    context.to_string(),
                    format!("{phase} command {} must not be empty", index + 1),
                ));
            }
            if command.contains('\0') {
                return Err(ManifestError::InvalidInstallHook(
                    context.to_string(),
                    format!("{phase} command {} contains NUL", index + 1),
                ));
            }
            if command.len() > 32 * 1024 {
                return Err(ManifestError::InvalidInstallHook(
                    context.to_string(),
                    format!("{phase} command {} exceeds 32768 bytes", index + 1),
                ));
            }
        }
    }
    Ok(())
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
    #[error("invalid native dependency declaration for `{0}`: {1}")]
    InvalidNativeDependency(String, String),
    #[error("invalid install hook declaration for `{0}`: {1}")]
    InvalidInstallHook(String, String),
    #[error("invalid workspace member pattern `{0}`")]
    InvalidWorkspaceMember(String),
    #[error("invalid install dir `{0}`: {1}")]
    InvalidInstallDir(String, String),
    #[error("invalid target `{0}`: {1}")]
    InvalidTarget(String, String),
    #[error("invalid native release route for target `{0}`: {1}")]
    InvalidNativeRoute(String, String),
    #[error("invalid Nix export route for target `{0}`: {1}")]
    InvalidNixRoute(String, String),
    #[error("manifest toml error: {0}")]
    Toml(String),
}

/// Ecosystem adapter names accepted by `[install].adapter` and
/// `[targets.*].adapter`. Kept here rather than in the CLI so a manifest is
/// validated the same way by the CLI, the registry, and the web UI.
///
/// Each name is a toolchain zed can wire dependencies into. `none` opts out,
/// installing to `zed_modules/` only. This is deliberately smaller than
/// [`crate::language::Ecosystem`]: an ecosystem says what a package *is*, an
/// adapter says what zed can *wire*, and the second list grows more slowly.
pub const ADAPTERS: &[&str] = &["node", "java", "go", "python", "rust", "dart", "none"];

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

fn validate_native_release_section(
    native: &NativeReleaseSection,
    route_name: &str,
    route_dir: &str,
    inbound: Ecosystem,
) -> Result<(), ManifestError> {
    native
        .registry
        .validate_package(&native.package)
        .map_err(|reason| ManifestError::InvalidNativeRoute(route_name.to_string(), reason))?;

    // The outbound mirror and the inbound install gate describe the same
    // ecosystem. If they disagree, one of them is wrong and the package would
    // either be mirrored to the wrong registry or refused for the wrong
    // consumers — both silent until someone hits it, so fail here instead.
    let outbound = native.registry.ecosystem();
    if !inbound.is_default() && inbound != outbound {
        return Err(ManifestError::InvalidNativeRoute(
            route_name.to_string(),
            format!(
                "routes to {} (ecosystem `{outbound}`) but installs as `{inbound}`; \
                 set the package or target ecosystem if the mirror is right",
                native.registry.as_str()
            ),
        ));
    }

    if let Some(tag_format) = &native.tag_format {
        if !tag_format.contains("{version}") {
            return Err(ManifestError::InvalidNativeRoute(
                route_name.to_string(),
                "native `tag_format` must contain `{version}`".to_string(),
            ));
        }
        if tag_format.trim() != tag_format
            || tag_format.is_empty()
            || tag_format.chars().any(char::is_whitespace)
        {
            return Err(ManifestError::InvalidNativeRoute(
                route_name.to_string(),
                "native `tag_format` must be a non-empty VCS ref without whitespace".to_string(),
            ));
        }
    }
    if native.registry == NativeRegistry::GoModules && route_dir != "." {
        let required_prefix = format!("{}/", route_dir.trim_end_matches('/'));
        let Some(tag_format) = &native.tag_format else {
            return Err(ManifestError::InvalidNativeRoute(
                route_name.to_string(),
                format!(
                    "Go module in `{route_dir}` requires `tag_format = \"{required_prefix}v{{version}}\"`"
                ),
            ));
        };
        if !tag_format.starts_with(&required_prefix) {
            return Err(ManifestError::InvalidNativeRoute(
                route_name.to_string(),
                format!(
                    "Go module in `{route_dir}` requires a native tag prefixed by `{required_prefix}`"
                ),
            ));
        }
    }

    let mut forge_registries = std::collections::BTreeSet::new();
    for forge in &native.forge {
        if !forge_registries.insert(*forge) {
            return Err(ManifestError::InvalidNativeRoute(
                route_name.to_string(),
                format!("forge registry `{forge}` is listed more than once"),
            ));
        }
        if !forge.supports(native.registry) {
            return Err(ManifestError::InvalidNativeRoute(
                route_name.to_string(),
                format!(
                    "forge registry `{forge}` does not support {} packages",
                    native.registry.as_str()
                ),
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
        let mut native_routes = BTreeMap::<(NativeRegistry, String), &str>::new();
        let mut nix_attributes = BTreeMap::<String, &str>::new();
        if let Some(native) = &self.publish.native {
            if !self.targets.is_empty() {
                return Err(ManifestError::InvalidNativeRoute(
                    "repository".to_string(),
                    "root `[publish.native]` is only valid for a single-language package; \
                     polyglot packages declare `[targets.<language>.native]`"
                        .to_string(),
                ));
            }
            validate_native_release_section(native, "repository", ".", self.package.ecosystem())?;
            native_routes.insert(
                (
                    native.registry,
                    native.registry.canonical_package(&native.package),
                ),
                "repository",
            );
        }
        if let Some(nix) = &self.publish.nix {
            if !self.targets.is_empty() {
                return Err(ManifestError::InvalidNixRoute(
                    "repository".to_string(),
                    "root `[publish.nix]` is only valid for a single-language package;                      polyglot packages declare `[targets.<language>.nix]`"
                        .to_string(),
                ));
            }
            nix.validate(&self.package.name).map_err(|error| {
                ManifestError::InvalidNixRoute("repository".to_string(), error.to_string())
            })?;
            nix_attributes.insert(nix.resolved_attribute(&self.package.name), "repository");
        }
        validate_native_dependencies(&self.native_dependencies, "package")?;
        validate_install_hooks(&self.hooks, "package")?;
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
            if let Some(nix) = &target.nix {
                nix.validate(&published_name).map_err(|error| {
                    ManifestError::InvalidNixRoute(name.clone(), error.to_string())
                })?;
                let attribute = nix.resolved_attribute(&published_name);
                if let Some(previous) = nix_attributes.insert(attribute.clone(), name.as_str()) {
                    return Err(ManifestError::InvalidNixRoute(
                        name.clone(),
                        format!(
                            "Nix attribute `{attribute}` is already used by target `{previous}`"
                        ),
                    ));
                }
            }
            if let Some(adapter) = target.adapter.as_deref()
                && !ADAPTERS.contains(&adapter)
            {
                return Err(ManifestError::InvalidTarget(
                    name.clone(),
                    format!(
                        "adapter `{adapter}` is unsupported; expected one of {}",
                        ADAPTERS.join(", ")
                    ),
                ));
            }
            validate_native_dependencies(&target.native_dependencies, &format!("target `{name}`"))?;
            validate_install_hooks(&target.hooks, &format!("target `{name}`"))?;
            if let Some(native) = &target.native {
                if target.dir == "." {
                    return Err(ManifestError::InvalidNativeRoute(
                        name.clone(),
                        "the whole-repository target cannot publish to a native registry"
                            .to_string(),
                    ));
                }
                validate_native_release_section(
                    native,
                    name,
                    &target.dir,
                    target.ecosystem_for(name),
                )?;
                let canonical_package = native.registry.canonical_package(&native.package);
                let route = (native.registry, canonical_package);
                if let Some(previous) = native_routes.insert(route, name.as_str()) {
                    return Err(ManifestError::InvalidNativeRoute(
                        name.clone(),
                        format!(
                            "{} package `{}` is already routed by target `{previous}`",
                            native.registry.as_str(),
                            native.package
                        ),
                    ));
                }
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

    /// Nix export routes sorted by target name. These contain author intent only;
    /// realization hashes and store paths live in versioned lock provenance.
    pub fn nix_export_routes(&self) -> Vec<NixExportRoute> {
        let mut routes = Vec::new();
        if let Some(intent) = &self.publish.nix {
            routes.push(NixExportRoute {
                target: "repository".to_string(),
                dir: ".".to_string(),
                package: self.package.name.clone(),
                intent: intent.clone(),
            });
        }
        for (target, section) in &self.targets {
            if let Some(intent) = &section.nix {
                routes.push(NixExportRoute {
                    target: target.clone(),
                    dir: section.dir.clone(),
                    package: section
                        .name
                        .clone()
                        .unwrap_or_else(|| format!("{}-{target}", self.package.name)),
                    intent: intent.clone(),
                });
            }
        }
        routes
    }

    /// Native release routes sorted by target name, suitable for deterministic
    /// credential-free planning before any registry adapter executes.
    pub fn native_release_routes(&self) -> Vec<NativeReleaseRoute> {
        self.publish
            .native
            .iter()
            .map(|native| NativeReleaseRoute {
                target: "repository".to_string(),
                dir: ".".to_string(),
                registry: native.registry,
                package: native.package.clone(),
                vcs_tag: native
                    .tag_format
                    .as_deref()
                    .unwrap_or(&self.publish.tag_format)
                    .replace("{version}", &self.package.version),
            })
            .chain(self.targets.iter().filter_map(|(target, section)| {
                section.native.as_ref().map(|native| NativeReleaseRoute {
                    target: target.clone(),
                    dir: section.dir.clone(),
                    registry: native.registry,
                    package: native.package.clone(),
                    vcs_tag: native
                        .tag_format
                        .as_deref()
                        .unwrap_or(&self.publish.tag_format)
                        .replace("{version}", &self.package.version),
                })
            }))
            .collect()
    }

    /// Forge package-registry mirrors, flattened and sorted by target then
    /// registry for deterministic release plans and CI matrices.
    pub fn forge_release_routes(&self) -> Vec<ForgeReleaseRoute> {
        self.publish
            .native
            .iter()
            .flat_map(|native| {
                native
                    .forge
                    .iter()
                    .copied()
                    .map(move |registry| ForgeReleaseRoute {
                        target: "repository".to_string(),
                        dir: ".".to_string(),
                        registry,
                        format: native.registry,
                        package: native.package.clone(),
                        vcs_tag: native
                            .tag_format
                            .as_deref()
                            .unwrap_or(&self.publish.tag_format)
                            .replace("{version}", &self.package.version),
                    })
            })
            .chain(self.targets.iter().flat_map(|(target, section)| {
                section.native.iter().flat_map(move |native| {
                    native
                        .forge
                        .iter()
                        .copied()
                        .map(move |registry| ForgeReleaseRoute {
                            target: target.clone(),
                            dir: section.dir.clone(),
                            registry,
                            format: native.registry,
                            package: native.package.clone(),
                            vcs_tag: native
                                .tag_format
                                .as_deref()
                                .unwrap_or(&self.publish.tag_format)
                                .replace("{version}", &self.package.version),
                        })
                })
            }))
            .collect()
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
        // Once re-rooted, a polyglot slice is a standalone package. Preserve
        // its outbound native/forge routing under the single-package shape so
        // the manifest inside the Zed artifact remains self-describing.
        derived.publish.native = section.native.clone();
        derived.publish.nix = section.nix.clone();
        derived.native_dependencies =
            merged_native_dependencies(&self.native_dependencies, &section.native_dependencies);
        derived.hooks = self.hooks.merged(&section.hooks);
        derived.targets = BTreeMap::new();
        derived.workspace = None;
        // The consumer-facing wiring for this ecosystem.
        derived.install.adapter = section.adapter.clone().or(self.install.adapter.clone());
        derived.install.target = None;
        // Stamp the slice's identity so the *published* package self-describes
        // as single-language. This is what lets a consumer's install refuse to
        // drop `-java` into a Node-only project: the artifact says `jvm`, and
        // the guard has something to compare against.
        if let Some(language) = Language::from_token(target) {
            derived.package.language = language;
            derived.package.ecosystem = section.ecosystem_for(target);
        }
        Some(derived)
    }

    /// The target key matching `requested`, honoring language synonyms.
    ///
    /// Target keys are chosen by the package author (`nodejs`, `node`, `ts`)
    /// while a consumer's request comes from a flag, their manifest, or
    /// inference — so the two spellings routinely differ for the same language.
    /// Exact match wins; otherwise `requested` and each key are normalized
    /// through [`Language::from_token`] and compared, which is what makes a
    /// project detected as `node` resolve a `[targets.nodejs]` package (and
    /// `go` reach `golang`).
    pub fn resolve_target_key(&self, requested: &str) -> Option<&str> {
        if let Some((key, _)) = self.targets.get_key_value(requested) {
            return Some(key.as_str());
        }
        let wanted = Language::from_token(requested).filter(|l| !l.is_default())?;
        let mut matches = self
            .targets
            .keys()
            .filter(|key| Language::from_token(key) == Some(wanted));
        let first = matches.next()?;
        if matches.next().is_none() {
            return Some(first.as_str());
        }
        // Several targets share this language — a repo shipping separate `ts`
        // and `js` clients has two `nodejs` targets. A synonym request like
        // `node` must land somewhere predictable rather than on whichever key
        // sorts first, so prefer the one named after the language itself.
        self.targets
            .keys()
            .find(|key| key.as_str() == wanted.as_str())
            .or_else(|| {
                self.targets
                    .keys()
                    .find(|key| Language::from_token(key) == Some(wanted))
            })
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
        // Synonym-aware: a project inferred as `node` must resolve a package
        // that spells its target `nodejs`, or every such consumer would hit the
        // error below despite the package shipping exactly what they need.
        match self
            .resolve_target_key(requested)
            .and_then(|key| self.targets.get(key))
        {
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

    /// Native prerequisites for the selected polyglot target. Package-level
    /// entries are inherited; target entries append in declaration order and
    /// are deterministically de-duplicated per manager.
    pub fn effective_native_dependencies(
        &self,
        requested: Option<&str>,
    ) -> Result<NativeDependencies, ManifestError> {
        let Some(requested) = requested else {
            return Ok(self.native_dependencies.clone());
        };
        if self.targets.is_empty() {
            return Ok(self.native_dependencies.clone());
        }
        let key = self.resolve_target_key(requested).ok_or_else(|| {
            let mut available: Vec<&str> = self.targets.keys().map(String::as_str).collect();
            available.sort_unstable();
            ManifestError::InvalidTarget(
                requested.to_string(),
                format!(
                    "package `{}/{}` publishes no such target; it provides: {}",
                    self.package.org,
                    self.package.name,
                    available.join(", ")
                ),
            )
        })?;
        let target = self.targets.get(key).expect("resolved target exists");
        Ok(merged_native_dependencies(
            &self.native_dependencies,
            &target.native_dependencies,
        ))
    }

    /// Lifecycle hooks for the selected target. Package hooks run before
    /// target hooks in each phase.
    pub fn effective_install_hooks(
        &self,
        requested: Option<&str>,
    ) -> Result<InstallHooksSection, ManifestError> {
        let Some(requested) = requested else {
            return Ok(self.hooks.clone());
        };
        if self.targets.is_empty() {
            return Ok(self.hooks.clone());
        }
        let key = self.resolve_target_key(requested).ok_or_else(|| {
            let mut available: Vec<&str> = self.targets.keys().map(String::as_str).collect();
            available.sort_unstable();
            ManifestError::InvalidTarget(
                requested.to_string(),
                format!(
                    "package `{}/{}` publishes no such target; it provides: {}",
                    self.package.org,
                    self.package.name,
                    available.join(", ")
                ),
            )
        })?;
        Ok(self
            .hooks
            .merged(&self.targets.get(key).expect("resolved target exists").hooks))
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
