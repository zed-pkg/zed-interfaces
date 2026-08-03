//! Reproducible development-environment contracts shared by Zed and its
//! interoperability adapters.
//!
//! Zed remains the resolver for `.zpkg.toml` package dependencies. This module
//! models the adjacent developer-environment plane: runtime/tool pins,
//! environment variables, task graphs, system packages, and source-manager
//! provenance. mise, asdf, Devbox, Flox, Nix, and native Zed implementations
//! translate through this schema instead of translating directly into one
//! another.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Current on-disk/wire schema for [`EnvironmentPlan`].
pub const ENVIRONMENT_PLAN_SCHEMA_VERSION: u32 = 1;

fn default_schema_version() -> u32 {
    ENVIRONMENT_PLAN_SCHEMA_VERSION
}

/// How strictly an environment plan should be validated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentValidationMode {
    /// Accept unresolved requirements while still enforcing structural safety.
    Permissive,
    /// Require exact, non-floating resolutions and immutable provenance.
    Frozen,
}

/// A schema-versioned, manager-neutral development environment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EnvironmentPlan {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,

    /// Tool/runtime name to one or more requested versions.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tools: BTreeMap<String, ToolSpec>,

    /// Environment values applied during activation and task execution.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, EnvironmentValue>,

    /// Named task DAG. Command order inside a task is semantic and preserved.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tasks: BTreeMap<String, TaskSpec>,

    /// System packages that are not language runtimes or Zed dependencies.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub system_packages: BTreeMap<String, SystemPackageSpec>,

    /// Project-local configuration and lock data that produced this plan.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provenance: Vec<EnvironmentProvenance>,
}

impl Default for EnvironmentPlan {
    fn default() -> Self {
        Self {
            schema_version: ENVIRONMENT_PLAN_SCHEMA_VERSION,
            tools: BTreeMap::new(),
            env: BTreeMap::new(),
            tasks: BTreeMap::new(),
            system_packages: BTreeMap::new(),
            provenance: Vec::new(),
        }
    }
}

/// TOML-compatible value used for environment variables and backend options.
///
/// Scalar values cover ordinary environment variables; arrays/tables preserve
/// manager-specific structured forms without reducing them to lossy strings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum EnvironmentValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Array(Vec<EnvironmentValue>),
    Table(BTreeMap<String, EnvironmentValue>),
}

impl EnvironmentValue {
    fn validate(&self, path: &str) -> Result<(), EnvironmentPlanError> {
        match self {
            Self::Float(value) if !value.is_finite() => {
                Err(EnvironmentPlanError::NonFiniteFloat {
                    path: path.to_string(),
                })
            }
            Self::Array(values) => {
                for (index, value) in values.iter().enumerate() {
                    value.validate(&format!("{path}[{index}]"))?;
                }
                Ok(())
            }
            Self::Table(values) => {
                for (key, value) in values {
                    value.validate(&format!("{path}.{key}"))?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

/// Backend plus all versions requested for one logical tool name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ToolSpec {
    /// Backend/provider identity, such as `core`, `aqua`, `github`, `npm`,
    /// `cargo`, `ubi`, `asdf`, or `vfox`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub versions: Vec<ToolVersion>,

    /// Lossless adapter metadata not yet promoted into the shared schema.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: BTreeMap<String, serde_json::Value>,
}

/// One requested and, when locked, resolved version of a tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ToolVersion {
    /// Original user requirement (`24`, `^3.12`, `latest`, or an exact pin).
    pub requirement: String,

    /// Exact manager-resolved version used by frozen installs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<ToolSourceIdentity>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub platforms: Vec<PlatformSelector>,

    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub options: BTreeMap<String, EnvironmentValue>,
}

/// Kind of source from which a tool or system package was resolved.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ToolSourceKind {
    Registry,
    Vcs,
    Http,
    Path,
    Other,
}

/// Immutable source identity used for offline replay and tamper detection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ToolSourceIdentity {
    pub kind: ToolSourceKind,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry: Option<String>,

    /// Commit, immutable tag/object identity, package revision, or equivalent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,

    /// Digest including its algorithm, for example `sha256:abc...`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,

    /// True only when the named revision cannot move under manager semantics.
    #[serde(default)]
    pub immutable: bool,
}

impl ToolSourceIdentity {
    fn has_text(value: &Option<String>) -> bool {
        value.as_deref().is_some_and(|value| !value.trim().is_empty())
    }

    fn frozen_identity_is_immutable(&self) -> bool {
        match self.kind {
            ToolSourceKind::Path => false,
            ToolSourceKind::Http => Self::has_text(&self.url) && Self::has_text(&self.checksum),
            ToolSourceKind::Vcs => {
                Self::has_text(&self.url)
                    && Self::has_text(&self.revision)
                    && self.immutable
            }
            ToolSourceKind::Registry | ToolSourceKind::Other => {
                Self::has_text(&self.checksum)
                    || (Self::has_text(&self.revision) && self.immutable)
            }
        }
    }
}

/// OS/architecture constraints. Empty selectors are rejected.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub struct PlatformSelector {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub libc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abi: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

impl PlatformSelector {
    fn is_empty(&self) -> bool {
        [&self.os, &self.arch, &self.libc, &self.abi, &self.target]
            .into_iter()
            .all(|value| value.as_deref().is_none_or(|value| value.trim().is_empty()))
    }
}

/// A system package resolved by an external manager such as Nixpkgs, Flox, or
/// Devbox. Zed package dependencies do not belong in this map.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SystemPackageSpec {
    pub manager: String,
    pub requirement: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<ToolSourceIdentity>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub platforms: Vec<PlatformSelector>,

    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: BTreeMap<String, serde_json::Value>,
}

/// A named task, compatible with mise-style command arrays and task DAGs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TaskSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub run: Vec<TaskStep>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_post: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wait_for: Vec<String>,

    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, EnvironmentValue>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirm: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,

    #[serde(default)]
    pub hide: bool,
    #[serde(default)]
    pub quiet: bool,
    #[serde(default)]
    pub silent: bool,
    #[serde(default)]
    pub raw: bool,

    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: BTreeMap<String, serde_json::Value>,
}

impl TaskSpec {
    fn referenced_tasks(&self) -> Vec<&str> {
        let mut tasks = Vec::new();
        tasks.extend(self.depends.iter().map(String::as_str));
        tasks.extend(self.depends_post.iter().map(String::as_str));
        tasks.extend(self.wait_for.iter().map(String::as_str));
        for step in &self.run {
            match step {
                TaskStep::Command(_) => {}
                TaskStep::Task(invocation) => tasks.push(invocation.task.as_str()),
                TaskStep::Tasks(group) => tasks.extend(group.tasks.iter().map(String::as_str)),
            }
        }
        tasks
    }
}

/// A task command, one task invocation, or a parallel task group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum TaskStep {
    Command(String),
    Task(TaskInvocation),
    Tasks(TaskGroup),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TaskInvocation {
    pub task: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,

    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, EnvironmentValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TaskGroup {
    pub tasks: Vec<String>,
}

/// Provenance for one imported or generated environment-manager view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EnvironmentProvenance {
    /// Manager/backend name such as `mise`, `asdf`, `devbox`, `flox`, `nix`,
    /// or `zed-native`.
    pub manager: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manager_version: Option<String>,

    /// Project-relative configuration files. User-global configuration must be
    /// represented only after an explicit opt-in by the caller.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub config_files: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lock_files: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_digest: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock_digest: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_revision: Option<String>,
}

/// Validation and serialization failures for environment plans.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EnvironmentPlanError {
    #[error(
        "unsupported environment schema version {found}; this build supports {supported}"
    )]
    UnsupportedSchemaVersion { found: u32, supported: u32 },

    #[error("{kind} name cannot be empty")]
    EmptyName { kind: &'static str },

    #[error("tool `{tool}` must declare at least one version")]
    ToolWithoutVersions { tool: String },

    #[error("{kind} `{name}` has an empty requirement")]
    EmptyRequirement { kind: &'static str, name: String },

    #[error("{kind} `{name}` is missing an exact resolved version in frozen mode")]
    MissingResolvedVersion { kind: &'static str, name: String },

    #[error("{kind} `{name}` resolved to floating selector `{value}` in frozen mode")]
    FloatingResolvedVersion {
        kind: &'static str,
        name: String,
        value: String,
    },

    #[error("{kind} `{name}` has mutable or incomplete source provenance")]
    MutableSource { kind: &'static str, name: String },

    #[error("platform selector for {kind} `{name}` is empty")]
    EmptyPlatform { kind: &'static str, name: String },

    #[error("environment value `{path}` contains a non-finite float")]
    NonFiniteFloat { path: String },

    #[error("task alias `{alias}` is claimed by both `{first}` and `{second}`")]
    DuplicateTaskAlias {
        alias: String,
        first: String,
        second: String,
    },

    #[error("task `{task}` references unknown task `{dependency}`")]
    UnknownTaskDependency { task: String, dependency: String },

    #[error("task dependency cycle detected: {cycle}")]
    TaskDependencyCycle { cycle: String },

    #[error("environment provenance manager cannot be empty")]
    EmptyProvenanceManager,

    #[error("environment provenance path must be project-relative: `{path}`")]
    NonProjectLocalProvenancePath { path: String },

    #[error("invalid environment plan TOML: {message}")]
    TomlParse { message: String },

    #[error("could not serialize environment plan as TOML: {message}")]
    TomlSerialize { message: String },

    #[error("could not serialize normalized environment plan: {message}")]
    JsonSerialize { message: String },
}

impl EnvironmentPlan {
    /// Parse and structurally validate a TOML environment plan.
    pub fn parse_toml(input: &str) -> Result<Self, EnvironmentPlanError> {
        let plan: Self = toml::from_str(input).map_err(|error| EnvironmentPlanError::TomlParse {
            message: error.to_string(),
        })?;
        plan.validate(EnvironmentValidationMode::Permissive)?;
        Ok(plan)
    }

    /// Emit a deterministic, pretty TOML representation.
    pub fn to_toml_string(&self) -> Result<String, EnvironmentPlanError> {
        self.validate(EnvironmentValidationMode::Permissive)?;
        toml::to_string_pretty(&self.normalized()).map_err(|error| {
            EnvironmentPlanError::TomlSerialize {
                message: error.to_string(),
            }
        })
    }

    /// Validate structural integrity and, in frozen mode, exact immutable pins.
    pub fn validate(
        &self,
        mode: EnvironmentValidationMode,
    ) -> Result<(), EnvironmentPlanError> {
        if self.schema_version != ENVIRONMENT_PLAN_SCHEMA_VERSION {
            return Err(EnvironmentPlanError::UnsupportedSchemaVersion {
                found: self.schema_version,
                supported: ENVIRONMENT_PLAN_SCHEMA_VERSION,
            });
        }

        for (name, tool) in &self.tools {
            validate_name("tool", name)?;
            if tool.versions.is_empty() {
                return Err(EnvironmentPlanError::ToolWithoutVersions {
                    tool: name.clone(),
                });
            }
            if tool.backend.as_deref().is_some_and(|value| value.trim().is_empty()) {
                return Err(EnvironmentPlanError::EmptyName { kind: "tool backend" });
            }
            validate_json_extensions(&tool.extensions, &format!("tools.{name}.extensions"))?;
            for version in &tool.versions {
                validate_requirement("tool", name, &version.requirement)?;
                validate_resolution("tool", name, version.resolved.as_deref(), mode)?;
                validate_source("tool", name, version.source.as_ref(), mode)?;
                validate_platforms("tool", name, &version.platforms)?;
                for (key, value) in &version.options {
                    value.validate(&format!("tools.{name}.options.{key}"))?;
                }
            }
        }

        for (name, package) in &self.system_packages {
            validate_name("system package", name)?;
            validate_name("system package manager", &package.manager)?;
            validate_requirement("system package", name, &package.requirement)?;
            validate_resolution(
                "system package",
                name,
                package.resolved.as_deref(),
                mode,
            )?;
            validate_source("system package", name, package.source.as_ref(), mode)?;
            validate_platforms("system package", name, &package.platforms)?;
            validate_json_extensions(
                &package.extensions,
                &format!("system_packages.{name}.extensions"),
            )?;
        }

        for (key, value) in &self.env {
            validate_name("environment variable", key)?;
            value.validate(&format!("env.{key}"))?;
        }

        self.validate_tasks()?;
        self.validate_provenance()?;
        Ok(())
    }

    /// Return a canonical clone suitable for drift checks and hashing.
    pub fn normalized(&self) -> Self {
        let mut plan = self.clone();

        for tool in plan.tools.values_mut() {
            tool.versions.sort_by_cached_key(stable_json_key);
            for version in &mut tool.versions {
                sort_dedup(&mut version.platforms);
            }
        }

        for package in plan.system_packages.values_mut() {
            sort_dedup(&mut package.platforms);
        }

        for task in plan.tasks.values_mut() {
            sort_dedup(&mut task.aliases);
            sort_dedup(&mut task.depends);
            sort_dedup(&mut task.depends_post);
            sort_dedup(&mut task.wait_for);
            sort_dedup(&mut task.sources);
            sort_dedup(&mut task.outputs);
            // `run` is intentionally not sorted: command order is semantic.
        }

        for provenance in &mut plan.provenance {
            sort_dedup(&mut provenance.config_files);
            sort_dedup(&mut provenance.lock_files);
        }
        plan.provenance.sort_by_cached_key(stable_json_key);

        plan
    }

    /// SHA-256 over canonical JSON. BTreeMap ordering plus normalization makes
    /// the digest independent of source-manager formatting and map insertion.
    pub fn normalized_digest_sha256(&self) -> Result<String, EnvironmentPlanError> {
        self.validate(EnvironmentValidationMode::Permissive)?;
        let bytes = serde_json::to_vec(&self.normalized()).map_err(|error| {
            EnvironmentPlanError::JsonSerialize {
                message: error.to_string(),
            }
        })?;
        Ok(hex::encode(Sha256::digest(bytes)))
    }

    fn validate_tasks(&self) -> Result<(), EnvironmentPlanError> {
        let mut aliases = BTreeMap::<String, String>::new();
        for (name, task) in &self.tasks {
            validate_name("task", name)?;
            for alias in &task.aliases {
                validate_name("task alias", alias)?;
                if let Some(first) = aliases.insert(alias.clone(), name.clone()) {
                    if first != *name {
                        return Err(EnvironmentPlanError::DuplicateTaskAlias {
                            alias: alias.clone(),
                            first,
                            second: name.clone(),
                        });
                    }
                }
            }
            for (key, value) in &task.env {
                validate_name("task environment variable", key)?;
                value.validate(&format!("tasks.{name}.env.{key}"))?;
            }
            validate_json_extensions(&task.extensions, &format!("tasks.{name}.extensions"))?;
        }

        for (name, task) in &self.tasks {
            for dependency in task.referenced_tasks() {
                if is_task_pattern(dependency) {
                    continue;
                }
                if !self.tasks.contains_key(dependency) && !aliases.contains_key(dependency) {
                    return Err(EnvironmentPlanError::UnknownTaskDependency {
                        task: name.clone(),
                        dependency: dependency.to_string(),
                    });
                }
            }
        }

        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();
        let mut stack = Vec::new();
        for task in self.tasks.keys() {
            visit_task(
                task,
                &self.tasks,
                &aliases,
                &mut visiting,
                &mut visited,
                &mut stack,
            )?;
        }
        Ok(())
    }

    fn validate_provenance(&self) -> Result<(), EnvironmentPlanError> {
        for provenance in &self.provenance {
            if provenance.manager.trim().is_empty() {
                return Err(EnvironmentPlanError::EmptyProvenanceManager);
            }
            for path in provenance
                .config_files
                .iter()
                .chain(provenance.lock_files.iter())
            {
                if !is_project_local(path) {
                    return Err(EnvironmentPlanError::NonProjectLocalProvenancePath {
                        path: path.clone(),
                    });
                }
            }
        }
        Ok(())
    }
}

fn validate_name(kind: &'static str, name: &str) -> Result<(), EnvironmentPlanError> {
    if name.trim().is_empty() {
        Err(EnvironmentPlanError::EmptyName { kind })
    } else {
        Ok(())
    }
}

fn validate_requirement(
    kind: &'static str,
    name: &str,
    requirement: &str,
) -> Result<(), EnvironmentPlanError> {
    if requirement.trim().is_empty() {
        Err(EnvironmentPlanError::EmptyRequirement {
            kind,
            name: name.to_string(),
        })
    } else {
        Ok(())
    }
}

fn validate_resolution(
    kind: &'static str,
    name: &str,
    resolved: Option<&str>,
    mode: EnvironmentValidationMode,
) -> Result<(), EnvironmentPlanError> {
    if mode == EnvironmentValidationMode::Permissive {
        return Ok(());
    }
    let Some(resolved) = resolved.map(str::trim).filter(|value| !value.is_empty()) else {
        return Err(EnvironmentPlanError::MissingResolvedVersion {
            kind,
            name: name.to_string(),
        });
    };
    if looks_floating(resolved) {
        return Err(EnvironmentPlanError::FloatingResolvedVersion {
            kind,
            name: name.to_string(),
            value: resolved.to_string(),
        });
    }
    Ok(())
}

fn validate_source(
    kind: &'static str,
    name: &str,
    source: Option<&ToolSourceIdentity>,
    mode: EnvironmentValidationMode,
) -> Result<(), EnvironmentPlanError> {
    let Some(source) = source else {
        return Ok(());
    };

    for value in [
        &source.url,
        &source.registry,
        &source.revision,
        &source.checksum,
    ] {
        if value.as_deref().is_some_and(|value| value.trim().is_empty()) {
            return Err(EnvironmentPlanError::MutableSource {
                kind,
                name: name.to_string(),
            });
        }
    }

    if mode == EnvironmentValidationMode::Frozen && !source.frozen_identity_is_immutable() {
        return Err(EnvironmentPlanError::MutableSource {
            kind,
            name: name.to_string(),
        });
    }
    Ok(())
}

fn validate_platforms(
    kind: &'static str,
    name: &str,
    platforms: &[PlatformSelector],
) -> Result<(), EnvironmentPlanError> {
    if platforms.iter().any(PlatformSelector::is_empty) {
        Err(EnvironmentPlanError::EmptyPlatform {
            kind,
            name: name.to_string(),
        })
    } else {
        Ok(())
    }
}

fn validate_json_extensions(
    extensions: &BTreeMap<String, serde_json::Value>,
    path: &str,
) -> Result<(), EnvironmentPlanError> {
    serde_json::to_vec(extensions).map_err(|error| EnvironmentPlanError::JsonSerialize {
        message: format!("{path}: {error}"),
    })?;
    Ok(())
}

fn looks_floating(value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty() {
        return true;
    }
    if matches!(
        value.as_str(),
        "latest"
            | "stable"
            | "current"
            | "system"
            | "present"
            | "head"
            | "main"
            | "master"
            | "nightly"
            | "canary"
            | "beta"
            | "alpha"
            | "lts"
    ) {
        return true;
    }
    value.contains('*')
        || value.ends_with(".x")
        || value.starts_with(['^', '~', '>', '<', '='])
        || value.starts_with("lts/")
        || value.starts_with("prefix:")
        || value.starts_with("path:")
        || value.starts_with("file:")
        || value.starts_with("ref:main")
        || value.starts_with("ref:master")
        || value.contains(" || ")
        || value.contains(" && ")
}

fn is_project_local(path: &str) -> bool {
    let trimmed = path.trim();
    !trimmed.is_empty()
        && !Path::new(trimmed).is_absolute()
        && !trimmed.starts_with('~')
        && !trimmed.starts_with("$HOME")
        && !trimmed.starts_with("${HOME}")
        && !trimmed.starts_with("%USERPROFILE%")
        && !trimmed.split(['/', '\\']).any(|part| part == "..")
}

fn is_task_pattern(task: &str) -> bool {
    task.contains(['*', '?', '['])
}

fn sort_dedup<T: Ord>(values: &mut Vec<T>) {
    values.sort();
    values.dedup();
}

fn stable_json_key<T: Serialize + std::fmt::Debug>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| format!("{value:?}"))
}

fn visit_task(
    task: &str,
    tasks: &BTreeMap<String, TaskSpec>,
    aliases: &BTreeMap<String, String>,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
    stack: &mut Vec<String>,
) -> Result<(), EnvironmentPlanError> {
    let canonical = aliases.get(task).map(String::as_str).unwrap_or(task);
    if visited.contains(canonical) {
        return Ok(());
    }
    if visiting.contains(canonical) {
        let start = stack
            .iter()
            .position(|entry| entry == canonical)
            .unwrap_or(0);
        let mut cycle = stack[start..].to_vec();
        cycle.push(canonical.to_string());
        return Err(EnvironmentPlanError::TaskDependencyCycle {
            cycle: cycle.join(" -> "),
        });
    }

    let Some(spec) = tasks.get(canonical) else {
        return Ok(());
    };
    visiting.insert(canonical.to_string());
    stack.push(canonical.to_string());
    for dependency in spec.referenced_tasks() {
        if is_task_pattern(dependency) {
            continue;
        }
        visit_task(dependency, tasks, aliases, visiting, visited, stack)?;
    }
    stack.pop();
    visiting.remove(canonical);
    visited.insert(canonical.to_string());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exact_tool(requirement: &str, resolved: &str) -> ToolSpec {
        ToolSpec {
            backend: Some("core".to_string()),
            versions: vec![ToolVersion {
                requirement: requirement.to_string(),
                resolved: Some(resolved.to_string()),
                source: None,
                platforms: Vec::new(),
                options: BTreeMap::new(),
            }],
            extensions: BTreeMap::new(),
        }
    }

    fn command_task(command: &str) -> TaskSpec {
        TaskSpec {
            description: None,
            aliases: Vec::new(),
            run: vec![TaskStep::Command(command.to_string())],
            depends: Vec::new(),
            depends_post: Vec::new(),
            wait_for: Vec::new(),
            env: BTreeMap::new(),
            dir: None,
            sources: Vec::new(),
            outputs: Vec::new(),
            confirm: None,
            shell: None,
            hide: false,
            quiet: false,
            silent: false,
            raw: false,
            extensions: BTreeMap::new(),
        }
    }

    #[test]
    fn default_plan_uses_current_schema() {
        assert_eq!(
            EnvironmentPlan::default().schema_version,
            ENVIRONMENT_PLAN_SCHEMA_VERSION
        );
    }

    #[test]
    fn exact_resolution_can_preserve_original_range() {
        let mut plan = EnvironmentPlan::default();
        plan.tools
            .insert("node".to_string(), exact_tool("24", "24.18.0"));
        assert_eq!(plan.validate(EnvironmentValidationMode::Frozen), Ok(()));
    }

    #[test]
    fn frozen_mode_rejects_floating_resolutions() {
        let mut plan = EnvironmentPlan::default();
        plan.tools
            .insert("node".to_string(), exact_tool("latest", "latest"));
        assert!(matches!(
            plan.validate(EnvironmentValidationMode::Frozen),
            Err(EnvironmentPlanError::FloatingResolvedVersion { .. })
        ));
    }

    #[test]
    fn frozen_mode_rejects_mutable_vcs_source() {
        let mut tool = exact_tool("1.0", "1.0.0");
        tool.versions[0].source = Some(ToolSourceIdentity {
            kind: ToolSourceKind::Vcs,
            url: Some("https://github.com/acme/tool".to_string()),
            registry: None,
            revision: Some("main".to_string()),
            checksum: None,
            immutable: false,
        });
        let mut plan = EnvironmentPlan::default();
        plan.tools.insert("acme-tool".to_string(), tool);
        assert!(matches!(
            plan.validate(EnvironmentValidationMode::Frozen),
            Err(EnvironmentPlanError::MutableSource { .. })
        ));
    }

    #[test]
    fn normalized_digest_ignores_nonsemantic_collection_order() {
        let mut first = EnvironmentPlan::default();
        let mut build = command_task("cargo build");
        build.aliases = vec!["b".to_string(), "build-all".to_string()];
        first.tasks.insert("build".to_string(), build);
        first.provenance = vec![
            EnvironmentProvenance {
                manager: "mise".to_string(),
                manager_version: Some("2026.5.15".to_string()),
                config_files: vec!["mise.toml".to_string(), ".tool-versions".to_string()],
                lock_files: vec!["mise.lock".to_string()],
                config_digest: Some("sha256:a".to_string()),
                lock_digest: Some("sha256:b".to_string()),
                source_revision: None,
            },
            EnvironmentProvenance {
                manager: "zed-native".to_string(),
                manager_version: None,
                config_files: vec![".zpkg.env.toml".to_string()],
                lock_files: Vec::new(),
                config_digest: None,
                lock_digest: None,
                source_revision: None,
            },
        ];

        let mut second = first.clone();
        second.tasks.get_mut("build").unwrap().aliases.reverse();
        second.provenance.reverse();
        second.provenance[1].config_files.reverse();

        assert_eq!(
            first.normalized_digest_sha256().unwrap(),
            second.normalized_digest_sha256().unwrap()
        );
    }

    #[test]
    fn task_cycles_are_rejected() {
        let mut plan = EnvironmentPlan::default();
        let mut a = command_task("echo a");
        a.depends.push("b".to_string());
        let mut b = command_task("echo b");
        b.depends.push("a".to_string());
        plan.tasks.insert("a".to_string(), a);
        plan.tasks.insert("b".to_string(), b);
        assert!(matches!(
            plan.validate(EnvironmentValidationMode::Permissive),
            Err(EnvironmentPlanError::TaskDependencyCycle { .. })
        ));
    }

    #[test]
    fn task_aliases_resolve_in_dependency_graph() {
        let mut plan = EnvironmentPlan::default();
        let mut build = command_task("cargo build");
        build.aliases.push("b".to_string());
        let mut test = command_task("cargo test");
        test.depends.push("b".to_string());
        plan.tasks.insert("build".to_string(), build);
        plan.tasks.insert("test".to_string(), test);
        assert_eq!(
            plan.validate(EnvironmentValidationMode::Permissive),
            Ok(())
        );
    }

    #[test]
    fn project_global_provenance_is_rejected() {
        let mut plan = EnvironmentPlan::default();
        plan.provenance.push(EnvironmentProvenance {
            manager: "mise".to_string(),
            manager_version: None,
            config_files: vec!["~/.config/mise/config.toml".to_string()],
            lock_files: Vec::new(),
            config_digest: None,
            lock_digest: None,
            source_revision: None,
        });
        assert!(matches!(
            plan.validate(EnvironmentValidationMode::Permissive),
            Err(EnvironmentPlanError::NonProjectLocalProvenancePath { .. })
        ));
    }

    #[test]
    fn toml_roundtrip_preserves_nested_values() {
        let mut plan = EnvironmentPlan::default();
        plan.env.insert(
            "ZED_EXAMPLE".to_string(),
            EnvironmentValue::Table(BTreeMap::from([
                (
                    "path".to_string(),
                    EnvironmentValue::String(".zed/dev".to_string()),
                ),
                (
                    "enabled".to_string(),
                    EnvironmentValue::Boolean(true),
                ),
            ])),
        );
        let text = plan.to_toml_string().unwrap();
        assert_eq!(EnvironmentPlan::parse_toml(&text).unwrap(), plan.normalized());
    }
}