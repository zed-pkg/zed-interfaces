//! Version-2 developer-environment contract for native Zed environments and
//! environment-manager adapters.
//!
//! Schema v1 established immutable tool and system-package provenance. Schema
//! v2 keeps the v1 single-tool wire shape readable while adding the parts
//! required for practical mise compatibility: ordered multi-version tools,
//! typed environment values, task graphs, task-local tools, and lossless
//! manager extension fields.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::environment::{
    ActivationPolicy, Checksum, EnvironmentPlan as EnvironmentPlanV1,
    EnvironmentPlanError as EnvironmentPlanV1Error, EnvironmentSource, EnvironmentValidationMode,
    ImmutableSource, SystemPackageRequirement, ToolRequirement,
};

/// Typed activation value, task variable, or backend option.
///
/// Arrays retain order. Tables are deterministic because they use
/// [`BTreeMap`]. Non-finite floats are rejected before serialization.
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
    fn validate(&self, field: &str) -> Result<(), EnvironmentPlanV2Error> {
        match self {
            Self::Float(value) if !value.is_finite() => {
                Err(EnvironmentPlanV2Error::NonFiniteValue {
                    field: field.to_string(),
                })
            }
            Self::Array(values) => {
                for (index, value) in values.iter().enumerate() {
                    value.validate(&format!("{field}[{index}]"))?;
                }
                Ok(())
            }
            Self::Table(values) => {
                for (key, value) in values {
                    validate_table_key(field, key)?;
                    value.validate(&format!("{field}.{key}"))?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

/// One version of a logical tool.
///
/// Flattening the schema-v1 requirement keeps existing single-tool JSON and
/// TOML documents readable without migration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ToolVersion {
    #[serde(flatten)]
    pub requirement: ToolRequirement,
    /// Typed options whose semantics are implemented by Zed or an adapter.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub options: BTreeMap<String, EnvironmentValue>,
    /// Manager-qualified fields not yet promoted into the shared contract.
    /// Adapters preserve these instead of silently dropping them.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: BTreeMap<String, serde_json::Value>,
}

impl ToolVersion {
    pub fn new(requirement: ToolRequirement) -> Self {
        Self {
            requirement,
            options: BTreeMap::new(),
            extensions: BTreeMap::new(),
        }
    }

    fn normalized(&self) -> Self {
        Self {
            requirement: normalize_tool_requirement(&self.requirement),
            options: self.options.clone(),
            extensions: self.extensions.clone(),
        }
    }

    fn validate(
        &self,
        name: &str,
        field: &str,
        mode: EnvironmentValidationMode,
    ) -> Result<(), EnvironmentPlanV2Error> {
        validate_tool_requirement(name, field, &self.requirement, mode)?;
        validate_value_map(&format!("{field}.options"), &self.options, false)?;
        validate_extension_map(&format!("{field}.extensions"), &self.extensions)
    }

    fn uses_v2_features(&self) -> bool {
        !self.options.is_empty() || !self.extensions.is_empty()
    }
}

impl From<ToolRequirement> for ToolVersion {
    fn from(value: ToolRequirement) -> Self {
        Self::new(value)
    }
}

/// One logical tool may expose one active version or an ordered version list.
/// Version order is retained because it may determine the default executable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ToolSpec {
    One(Box<ToolVersion>),
    Many(Vec<ToolVersion>),
}

impl ToolSpec {
    pub fn one(requirement: impl Into<ToolVersion>) -> Self {
        Self::One(Box::new(requirement.into()))
    }

    pub fn versions(&self) -> &[ToolVersion] {
        match self {
            Self::One(version) => std::slice::from_ref(version.as_ref()),
            Self::Many(versions) => versions,
        }
    }

    pub fn versions_mut(&mut self) -> &mut [ToolVersion] {
        match self {
            Self::One(version) => std::slice::from_mut(version.as_mut()),
            Self::Many(versions) => versions,
        }
    }

    fn normalized(&self) -> Self {
        let mut versions = self
            .versions()
            .iter()
            .map(ToolVersion::normalized)
            .collect::<Vec<_>>();
        stable_dedup_by_json(&mut versions);
        if versions.len() == 1 {
            Self::One(Box::new(versions.remove(0)))
        } else {
            Self::Many(versions)
        }
    }

    fn validate(
        &self,
        name: &str,
        field: &str,
        mode: EnvironmentValidationMode,
    ) -> Result<(), EnvironmentPlanV2Error> {
        if self.versions().is_empty() {
            return Err(EnvironmentPlanV2Error::ToolWithoutVersions {
                field: field.to_string(),
            });
        }
        for (index, version) in self.versions().iter().enumerate() {
            version.validate(name, &format!("{field}.versions[{index}]"), mode)?;
        }
        Ok(())
    }

    fn uses_v2_features(&self) -> bool {
        matches!(self, Self::Many(_)) || self.versions().iter().any(ToolVersion::uses_v2_features)
    }
}

impl From<ToolRequirement> for ToolSpec {
    fn from(value: ToolRequirement) -> Self {
        Self::one(value)
    }
}

impl From<ToolVersion> for ToolSpec {
    fn from(value: ToolVersion) -> Self {
        Self::One(Box::new(value))
    }
}

/// Schema-v2 system package with typed options and lossless extensions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SystemPackageSpec {
    #[serde(flatten)]
    pub requirement: SystemPackageRequirement,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub options: BTreeMap<String, EnvironmentValue>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: BTreeMap<String, serde_json::Value>,
}

impl SystemPackageSpec {
    pub fn new(requirement: SystemPackageRequirement) -> Self {
        Self {
            requirement,
            options: BTreeMap::new(),
            extensions: BTreeMap::new(),
        }
    }

    fn normalized(&self) -> Self {
        Self {
            requirement: normalize_system_package_requirement(&self.requirement),
            options: self.options.clone(),
            extensions: self.extensions.clone(),
        }
    }

    fn validate(
        &self,
        name: &str,
        field: &str,
        mode: EnvironmentValidationMode,
    ) -> Result<(), EnvironmentPlanV2Error> {
        validate_system_package_requirement(name, field, &self.requirement, mode)?;
        validate_value_map(&format!("{field}.options"), &self.options, false)?;
        validate_extension_map(&format!("{field}.extensions"), &self.extensions)
    }

    fn uses_v2_features(&self) -> bool {
        !self.options.is_empty() || !self.extensions.is_empty()
    }
}

impl From<SystemPackageRequirement> for SystemPackageSpec {
    fn from(value: SystemPackageRequirement) -> Self {
        Self::new(value)
    }
}

/// A task command, one task invocation, or an explicit task group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum TaskStep {
    Command(String),
    Task(TaskInvocation),
    Tasks(TaskGroup),
}

impl TaskStep {
    fn normalized(&self) -> Self {
        match self {
            Self::Command(command) => Self::Command(command.clone()),
            Self::Task(invocation) => Self::Task(invocation.normalized()),
            Self::Tasks(group) => Self::Tasks(group.normalized()),
        }
    }

    fn validate(&self, field: &str) -> Result<(), EnvironmentPlanV2Error> {
        match self {
            Self::Command(command) => {
                if command.trim().is_empty() {
                    return Err(EnvironmentPlanV2Error::EmptyTaskCommand {
                        field: field.to_string(),
                    });
                }
                Ok(())
            }
            Self::Task(invocation) => invocation.validate(field),
            Self::Tasks(group) => group.validate(field),
        }
    }

    fn referenced_tasks<'a>(&'a self, output: &mut Vec<&'a str>) {
        match self {
            Self::Command(_) => {}
            Self::Task(invocation) => output.push(invocation.task.as_str()),
            Self::Tasks(group) => output.extend(group.tasks.iter().map(String::as_str)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TaskInvocation {
    pub task: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, EnvironmentValue>,
}

impl TaskInvocation {
    fn normalized(&self) -> Self {
        let mut invocation = self.clone();
        invocation.task = invocation.task.trim().to_string();
        invocation
    }

    fn validate(&self, field: &str) -> Result<(), EnvironmentPlanV2Error> {
        validate_name("task reference", &self.task)?;
        validate_value_map(&format!("{field}.env"), &self.env, true)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TaskGroup {
    pub tasks: Vec<String>,
    /// Explicit groups are parallel by default, matching mise grouped tasks.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub parallel: bool,
}

impl TaskGroup {
    fn normalized(&self) -> Self {
        let mut group = self.clone();
        group.tasks = group
            .tasks
            .iter()
            .map(|task| task.trim())
            .filter(|task| !task.is_empty())
            .map(ToOwned::to_owned)
            .collect();
        if group.parallel {
            group.tasks.sort();
            group.tasks.dedup();
        } else {
            stable_dedup_strings(&mut group.tasks);
        }
        group
    }

    fn validate(&self, field: &str) -> Result<(), EnvironmentPlanV2Error> {
        if self.tasks.is_empty() {
            return Err(EnvironmentPlanV2Error::EmptyTaskGroup {
                field: field.to_string(),
            });
        }
        for task in &self.tasks {
            validate_name("task reference", task)?;
        }
        Ok(())
    }
}

/// Confirmation may be a Boolean policy or a custom prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum TaskConfirmation {
    Enabled(bool),
    Prompt(String),
}

/// Manager-neutral task definition.
///
/// Ordered command fields, shell arguments, and invocation arguments are not
/// sorted. Set-like metadata is canonicalized.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TaskSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub run: Vec<TaskStep>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub run_windows: Vec<TaskStep>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_post: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wait_for: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, EnvironmentValue>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub vars: BTreeMap<String, EnvironmentValue>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tools: BTreeMap<String, ToolSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<String>,
    /// Shell program followed by arguments. Order is semantic.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shell: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirm: Option<TaskConfirmation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub hide: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub quiet: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub silent: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub raw: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: BTreeMap<String, serde_json::Value>,
}

impl TaskSpec {
    fn normalized(&self) -> Self {
        let mut task = self.clone();
        task.description = normalize_optional(&task.description);
        normalize_strings(&mut task.aliases);
        task.run = task.run.iter().map(TaskStep::normalized).collect();
        task.run_windows = task.run_windows.iter().map(TaskStep::normalized).collect();
        normalize_strings(&mut task.depends);
        normalize_strings(&mut task.depends_post);
        normalize_strings(&mut task.wait_for);
        task.tools = task
            .tools
            .iter()
            .map(|(name, spec)| (name.clone(), spec.normalized()))
            .collect();
        task.dir = task.dir.as_deref().map(normalize_project_path);
        task.sources = task
            .sources
            .iter()
            .map(|value| normalize_project_path(value))
            .collect();
        normalize_strings(&mut task.sources);
        task.outputs = task
            .outputs
            .iter()
            .map(|value| normalize_project_path(value))
            .collect();
        normalize_strings(&mut task.outputs);
        task.usage = normalize_optional(&task.usage);
        task.timeout = normalize_optional(&task.timeout);
        task.confirm = match &task.confirm {
            Some(TaskConfirmation::Prompt(prompt)) if prompt.trim().is_empty() => None,
            Some(TaskConfirmation::Prompt(prompt)) => {
                Some(TaskConfirmation::Prompt(prompt.trim().to_string()))
            }
            other => other.clone(),
        };
        task
    }

    fn validate(
        &self,
        name: &str,
        mode: EnvironmentValidationMode,
    ) -> Result<(), EnvironmentPlanV2Error> {
        let field = format!("tasks.{name}");
        for (index, step) in self.run.iter().enumerate() {
            step.validate(&format!("{field}.run[{index}]"))?;
        }
        for (index, step) in self.run_windows.iter().enumerate() {
            step.validate(&format!("{field}.run-windows[{index}]"))?;
        }
        for alias in &self.aliases {
            validate_name("task alias", alias)?;
        }
        for dependency in self
            .depends
            .iter()
            .chain(self.depends_post.iter())
            .chain(self.wait_for.iter())
        {
            validate_name("task reference", dependency)?;
        }
        validate_value_map(&format!("{field}.env"), &self.env, true)?;
        validate_value_map(&format!("{field}.vars"), &self.vars, false)?;
        validate_tool_map(&format!("{field}.tools"), &self.tools, mode)?;
        if let Some(dir) = &self.dir {
            validate_project_pattern(&format!("{field}.dir"), dir)?;
        }
        for (index, source) in self.sources.iter().enumerate() {
            validate_project_pattern(&format!("{field}.sources[{index}]"), source)?;
        }
        for (index, output) in self.outputs.iter().enumerate() {
            validate_project_pattern(&format!("{field}.outputs[{index}]"), output)?;
        }
        for (index, shell) in self.shell.iter().enumerate() {
            if shell.trim().is_empty() || shell.chars().any(char::is_control) {
                return Err(EnvironmentPlanV2Error::InvalidShell {
                    field: format!("{field}.shell[{index}]"),
                    value: shell.clone(),
                });
            }
        }
        if let Some(TaskConfirmation::Prompt(prompt)) = &self.confirm
            && prompt.trim().is_empty()
        {
            return Err(EnvironmentPlanV2Error::EmptyField {
                field: format!("{field}.confirm"),
            });
        }
        if let Some(timeout) = &self.timeout
            && (timeout.trim().is_empty() || timeout.chars().any(char::is_control))
        {
            return Err(EnvironmentPlanV2Error::InvalidTimeout {
                field: format!("{field}.timeout"),
                value: timeout.clone(),
            });
        }
        validate_extension_map(&format!("{field}.extensions"), &self.extensions)
    }

    fn referenced_tasks(&self) -> Vec<&str> {
        let mut tasks = Vec::new();
        tasks.extend(self.depends.iter().map(String::as_str));
        tasks.extend(self.depends_post.iter().map(String::as_str));
        tasks.extend(self.wait_for.iter().map(String::as_str));
        for step in self.run.iter().chain(self.run_windows.iter()) {
            step.referenced_tasks(&mut tasks);
        }
        tasks
    }
}

/// Manager-neutral desired and resolved developer-environment state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EnvironmentPlanV2 {
    #[serde(default = "current_environment_schema")]
    pub schema: u32,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tools: BTreeMap<String, ToolSpec>,
    #[serde(
        default,
        rename = "system-packages",
        alias = "system_packages",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub system_packages: BTreeMap<String, SystemPackageSpec>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, EnvironmentValue>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub vars: BTreeMap<String, EnvironmentValue>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tasks: BTreeMap<String, TaskSpec>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub platforms: Vec<String>,
    #[serde(default)]
    pub activation: ActivationPolicy,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<EnvironmentSource>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: BTreeMap<String, serde_json::Value>,
}

fn current_environment_schema() -> u32 {
    EnvironmentPlanV2::CURRENT_SCHEMA
}

impl Default for EnvironmentPlanV2 {
    fn default() -> Self {
        Self {
            schema: Self::CURRENT_SCHEMA,
            tools: BTreeMap::new(),
            system_packages: BTreeMap::new(),
            env: BTreeMap::new(),
            vars: BTreeMap::new(),
            tasks: BTreeMap::new(),
            platforms: Vec::new(),
            activation: ActivationPolicy::None,
            sources: Vec::new(),
            extensions: BTreeMap::new(),
        }
    }
}

impl EnvironmentPlanV2 {
    pub const CURRENT_SCHEMA: u32 = 2;

    /// Parse a JSON environment plan and validate its authoring structure.
    /// Schema-v1 single-tool documents remain accepted.
    pub fn parse_json(input: &str) -> Result<Self, EnvironmentPlanV2Error> {
        let plan: Self = serde_json::from_str(input).map_err(|error| {
            EnvironmentPlanV2Error::Deserialization {
                format: "JSON",
                detail: error.to_string(),
            }
        })?;
        plan.validate(EnvironmentValidationMode::Authoring)?;
        Ok(plan)
    }

    /// Parse a TOML environment plan and validate its authoring structure.
    /// Schema-v1 single-tool documents remain accepted.
    pub fn parse_toml(input: &str) -> Result<Self, EnvironmentPlanV2Error> {
        let plan: Self =
            toml::from_str(input).map_err(|error| EnvironmentPlanV2Error::Deserialization {
                format: "TOML",
                detail: error.to_string(),
            })?;
        plan.validate(EnvironmentValidationMode::Authoring)?;
        Ok(plan)
    }

    /// Emit deterministic TOML after normalization.
    pub fn to_toml_string(&self) -> Result<String, EnvironmentPlanV2Error> {
        self.validate(EnvironmentValidationMode::Authoring)?;
        toml::to_string_pretty(&self.normalized())
            .map_err(|error| EnvironmentPlanV2Error::Serialization(error.to_string()))
    }

    /// Canonical compact JSON bytes for semantic identity and drift checks.
    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>, EnvironmentPlanV2Error> {
        self.validate(EnvironmentValidationMode::Authoring)?;
        serde_json::to_vec(&self.normalized())
            .map_err(|error| EnvironmentPlanV2Error::Serialization(error.to_string()))
    }

    /// Return a presentation-independent form for generation and hashing.
    /// Invalid map keys remain unchanged, so normalization cannot hide them.
    pub fn normalized(&self) -> Self {
        let mut plan = self.clone();
        plan.tools = plan
            .tools
            .iter()
            .map(|(name, spec)| (name.clone(), spec.normalized()))
            .collect();
        plan.system_packages = plan
            .system_packages
            .iter()
            .map(|(name, spec)| (name.clone(), spec.normalized()))
            .collect();
        plan.tasks = plan
            .tasks
            .iter()
            .map(|(name, task)| (name.clone(), task.normalized()))
            .collect();
        normalize_strings(&mut plan.platforms);
        plan.sources = plan
            .sources
            .iter()
            .map(normalize_environment_source)
            .collect();
        plan.sources.sort_by(|left, right| {
            (left.manager, &left.path, &left.lock_path, &left.digest).cmp(&(
                right.manager,
                &right.path,
                &right.lock_path,
                &right.digest,
            ))
        });
        plan.sources.dedup();
        plan
    }

    pub fn validate(&self, mode: EnvironmentValidationMode) -> Result<(), EnvironmentPlanV2Error> {
        if self.schema == 0 || self.schema > Self::CURRENT_SCHEMA {
            return Err(EnvironmentPlanV2Error::UnsupportedSchema {
                found: self.schema,
                supported: Self::CURRENT_SCHEMA,
            });
        }
        if self.schema < 2 && self.uses_v2_features() {
            return Err(EnvironmentPlanV2Error::FeatureRequiresSchema {
                feature: "multi-version tools, env, vars, tasks, options, or extensions"
                    .to_string(),
                found: self.schema,
                required: 2,
            });
        }

        validate_legacy_plan_shell(self, mode)?;
        validate_tool_map("tools", &self.tools, mode)?;
        for (name, package) in &self.system_packages {
            validate_name("system package", name)?;
            package.validate(name, &format!("system-packages.{name}"), mode)?;
        }
        validate_value_map("env", &self.env, true)?;
        validate_value_map("vars", &self.vars, false)?;
        validate_extension_map("extensions", &self.extensions)?;
        self.validate_tasks(mode)
    }

    fn uses_v2_features(&self) -> bool {
        !self.env.is_empty()
            || !self.vars.is_empty()
            || !self.tasks.is_empty()
            || !self.extensions.is_empty()
            || self.tools.values().any(ToolSpec::uses_v2_features)
            || self
                .system_packages
                .values()
                .any(SystemPackageSpec::uses_v2_features)
    }

    fn validate_tasks(
        &self,
        mode: EnvironmentValidationMode,
    ) -> Result<(), EnvironmentPlanV2Error> {
        let mut aliases = BTreeMap::<String, String>::new();
        for (name, task) in &self.tasks {
            validate_name("task", name)?;
            task.validate(name, mode)?;
            for alias in &task.aliases {
                if let Some((real_task, _)) = self.tasks.get_key_value(alias)
                    && real_task != name
                {
                    return Err(EnvironmentPlanV2Error::DuplicateTaskAlias {
                        alias: alias.clone(),
                        first: real_task.to_string(),
                        second: name.clone(),
                    });
                }
                if let Some(first) = aliases.insert(alias.clone(), name.clone())
                    && first != *name
                {
                    return Err(EnvironmentPlanV2Error::DuplicateTaskAlias {
                        alias: alias.clone(),
                        first,
                        second: name.clone(),
                    });
                }
            }
        }

        for (name, task) in &self.tasks {
            for dependency in task.referenced_tasks() {
                if is_task_pattern(dependency) {
                    continue;
                }
                if !self.tasks.contains_key(dependency) && !aliases.contains_key(dependency) {
                    return Err(EnvironmentPlanV2Error::UnknownTaskDependency {
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
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EnvironmentPlanV2Error {
    #[error("unsupported environment plan schema {found}; this build supports {supported}")]
    UnsupportedSchema { found: u32, supported: u32 },
    #[error("environment schema {found} cannot represent {feature}; schema {required} is required")]
    FeatureRequiresSchema {
        feature: String,
        found: u32,
        required: u32,
    },
    #[error("{field}: {source}")]
    LegacyValidation {
        field: String,
        #[source]
        source: EnvironmentPlanV1Error,
    },
    #[error("{field} must not be empty")]
    EmptyField { field: String },
    #[error("invalid {kind} name `{name}`; names cannot contain whitespace or controls")]
    InvalidName { kind: &'static str, name: String },
    #[error("{field} contains an invalid table key `{key}`")]
    InvalidTableKey { field: String, key: String },
    #[error("{field} has an invalid environment key `{key}`")]
    InvalidEnvironmentKey { field: String, key: String },
    #[error("{field} must include at least one tool version")]
    ToolWithoutVersions { field: String },
    #[error("{field} contains a non-finite floating-point value")]
    NonFiniteValue { field: String },
    #[error("{field} contains a JSON null, which has no portable TOML representation")]
    NullExtensionValue { field: String },
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
    #[error("{field} contains an empty task command")]
    EmptyTaskCommand { field: String },
    #[error("{field} contains an empty task group")]
    EmptyTaskGroup { field: String },
    #[error("{field} must be a safe project-relative path or pattern, got `{value}`")]
    UnsafeRelativePath { field: String, value: String },
    #[error("{field} contains invalid shell entry `{value}`")]
    InvalidShell { field: String, value: String },
    #[error("{field} contains invalid timeout `{value}`")]
    InvalidTimeout { field: String, value: String },
    #[error("environment plan {format} deserialization failed: {detail}")]
    Deserialization {
        format: &'static str,
        detail: String,
    },
    #[error("environment plan serialization failed: {0}")]
    Serialization(String),
}

fn validate_legacy_plan_shell(
    plan: &EnvironmentPlanV2,
    mode: EnvironmentValidationMode,
) -> Result<(), EnvironmentPlanV2Error> {
    let legacy = EnvironmentPlanV1 {
        schema: EnvironmentPlanV1::CURRENT_SCHEMA,
        tools: BTreeMap::new(),
        system_packages: BTreeMap::new(),
        platforms: plan.platforms.clone(),
        activation: plan.activation,
        sources: plan.sources.clone(),
    };
    legacy
        .validate(mode)
        .map_err(|source| EnvironmentPlanV2Error::LegacyValidation {
            field: "environment plan".to_string(),
            source,
        })
}

fn validate_tool_map(
    field: &str,
    tools: &BTreeMap<String, ToolSpec>,
    mode: EnvironmentValidationMode,
) -> Result<(), EnvironmentPlanV2Error> {
    for (name, spec) in tools {
        validate_name("tool", name)?;
        spec.validate(name, &format!("{field}.{name}"), mode)?;
    }
    Ok(())
}

fn validate_tool_requirement(
    name: &str,
    field: &str,
    requirement: &ToolRequirement,
    mode: EnvironmentValidationMode,
) -> Result<(), EnvironmentPlanV2Error> {
    let mut legacy = EnvironmentPlanV1::default();
    legacy.tools.insert(name.to_string(), requirement.clone());
    legacy
        .validate(mode)
        .map_err(|source| EnvironmentPlanV2Error::LegacyValidation {
            field: field.to_string(),
            source,
        })
}

fn validate_system_package_requirement(
    name: &str,
    field: &str,
    requirement: &SystemPackageRequirement,
    mode: EnvironmentValidationMode,
) -> Result<(), EnvironmentPlanV2Error> {
    let mut legacy = EnvironmentPlanV1::default();
    legacy
        .system_packages
        .insert(name.to_string(), requirement.clone());
    legacy
        .validate(mode)
        .map_err(|source| EnvironmentPlanV2Error::LegacyValidation {
            field: field.to_string(),
            source,
        })
}

fn validate_name(kind: &'static str, name: &str) -> Result<(), EnvironmentPlanV2Error> {
    if name.is_empty()
        || name.trim() != name
        || name
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
    {
        return Err(EnvironmentPlanV2Error::InvalidName {
            kind,
            name: name.to_string(),
        });
    }
    Ok(())
}

fn validate_table_key(field: &str, key: &str) -> Result<(), EnvironmentPlanV2Error> {
    if key.trim().is_empty() || key.trim() != key || key.chars().any(char::is_control) {
        return Err(EnvironmentPlanV2Error::InvalidTableKey {
            field: field.to_string(),
            key: key.to_string(),
        });
    }
    Ok(())
}

fn validate_value_map(
    field: &str,
    values: &BTreeMap<String, EnvironmentValue>,
    environment_keys: bool,
) -> Result<(), EnvironmentPlanV2Error> {
    for (key, value) in values {
        let invalid = key.is_empty()
            || key.trim() != key
            || key.chars().any(char::is_control)
            || (environment_keys && (key.contains('=') || key.chars().any(char::is_whitespace)));
        if invalid {
            return Err(EnvironmentPlanV2Error::InvalidEnvironmentKey {
                field: field.to_string(),
                key: key.clone(),
            });
        }
        value.validate(&format!("{field}.{key}"))?;
    }
    Ok(())
}

fn validate_extension_map(
    field: &str,
    extensions: &BTreeMap<String, serde_json::Value>,
) -> Result<(), EnvironmentPlanV2Error> {
    for (key, value) in extensions {
        validate_table_key(field, key)?;
        validate_extension_value(&format!("{field}.{key}"), value)?;
    }
    Ok(())
}

fn validate_extension_value(
    field: &str,
    value: &serde_json::Value,
) -> Result<(), EnvironmentPlanV2Error> {
    match value {
        serde_json::Value::Null => Err(EnvironmentPlanV2Error::NullExtensionValue {
            field: field.to_string(),
        }),
        serde_json::Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                validate_extension_value(&format!("{field}[{index}]"), value)?;
            }
            Ok(())
        }
        serde_json::Value::Object(values) => {
            for (key, value) in values {
                validate_table_key(field, key)?;
                validate_extension_value(&format!("{field}.{key}"), value)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn validate_project_pattern(field: &str, value: &str) -> Result<(), EnvironmentPlanV2Error> {
    let trimmed = value.trim();
    let has_drive_prefix = trimmed.as_bytes().get(1) == Some(&b':');
    let segments = trimmed.split(['/', '\\']).collect::<Vec<_>>();
    let has_parent = segments.contains(&"..");
    let has_empty_middle = segments
        .iter()
        .enumerate()
        .any(|(index, segment)| segment.is_empty() && index != segments.len().saturating_sub(1));
    let unsafe_reference = trimmed.starts_with('/')
        || trimmed.starts_with('\\')
        || trimmed.starts_with('~')
        || trimmed.starts_with("$HOME")
        || trimmed.starts_with("${HOME}")
        || trimmed.starts_with("%USERPROFILE%")
        || has_drive_prefix
        || has_parent;
    if trimmed.is_empty()
        || trimmed != value
        || trimmed.chars().any(char::is_control)
        || has_empty_middle
        || unsafe_reference
    {
        return Err(EnvironmentPlanV2Error::UnsafeRelativePath {
            field: field.to_string(),
            value: value.to_string(),
        });
    }
    Ok(())
}

fn normalize_project_path(value: &str) -> String {
    let mut normalized = value.trim();
    while let Some(stripped) = normalized.strip_prefix("./") {
        normalized = stripped;
    }
    normalized.to_string()
}

fn normalize_tool_requirement(requirement: &ToolRequirement) -> ToolRequirement {
    let mut requirement = requirement.clone();
    requirement.requirement = requirement.requirement.trim().to_string();
    requirement.resolved = normalize_optional(&requirement.resolved);
    requirement.provider = normalize_optional(&requirement.provider);
    requirement.backend = normalize_optional(&requirement.backend);
    requirement.source = requirement.source.as_ref().map(normalize_immutable_source);
    normalize_checksums(&mut requirement.checksums);
    normalize_strings(&mut requirement.platforms);
    requirement
}

fn normalize_system_package_requirement(
    requirement: &SystemPackageRequirement,
) -> SystemPackageRequirement {
    let mut requirement = requirement.clone();
    requirement.requirement = requirement.requirement.trim().to_string();
    requirement.resolved = normalize_optional(&requirement.resolved);
    requirement.provider = normalize_optional(&requirement.provider);
    requirement.package_ref = normalize_optional(&requirement.package_ref);
    requirement.source = requirement.source.as_ref().map(normalize_immutable_source);
    normalize_checksums(&mut requirement.checksums);
    normalize_strings(&mut requirement.platforms);
    requirement
}

fn normalize_immutable_source(source: &ImmutableSource) -> ImmutableSource {
    let mut source = source.clone();
    source.url = source.url.trim().to_string();
    source.revision = source.revision.trim().to_ascii_lowercase();
    source.subdir = source.subdir.as_deref().map(normalize_project_path);
    normalize_checksums(&mut source.checksums);
    source
}

fn normalize_environment_source(source: &EnvironmentSource) -> EnvironmentSource {
    EnvironmentSource {
        manager: source.manager,
        path: normalize_project_path(&source.path),
        lock_path: source.lock_path.as_deref().map(normalize_project_path),
        digest: source.digest.as_ref().map(normalize_checksum),
    }
}

fn normalize_checksum(checksum: &Checksum) -> Checksum {
    Checksum {
        algorithm: checksum.algorithm,
        value: checksum.value.trim().to_ascii_lowercase(),
    }
}

fn normalize_checksums(checksums: &mut Vec<Checksum>) {
    *checksums = checksums.iter().map(normalize_checksum).collect();
    checksums.sort();
    checksums.dedup();
}

fn normalize_optional(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn normalize_strings(values: &mut Vec<String>) {
    *values = values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    values.sort();
    values.dedup();
}

fn stable_dedup_strings(values: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    values.retain(|value| seen.insert(value.clone()));
}

fn stable_dedup_by_json<T: Serialize>(values: &mut Vec<T>) {
    let mut seen = BTreeSet::new();
    values.retain(|value| {
        let key = serde_json::to_string(value)
            .unwrap_or_else(|error| format!("<serialization-error:{error}>"));
        seen.insert(key)
    });
}

fn is_task_pattern(task: &str) -> bool {
    task.contains(['*', '?', '['])
}

fn visit_task(
    task: &str,
    tasks: &BTreeMap<String, TaskSpec>,
    aliases: &BTreeMap<String, String>,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
    stack: &mut Vec<String>,
) -> Result<(), EnvironmentPlanV2Error> {
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
        return Err(EnvironmentPlanV2Error::TaskDependencyCycle {
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

fn default_true() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{ChecksumAlgorithm, EnvironmentManager};

    fn exact_tool(resolved: &str) -> ToolRequirement {
        ToolRequirement {
            requirement: "^22".to_string(),
            resolved: Some(resolved.to_string()),
            provider: Some("core".to_string()),
            backend: None,
            source: None,
            checksums: Vec::new(),
            platforms: vec!["x86_64-linux".to_string()],
        }
    }

    fn command_task(command: &str) -> TaskSpec {
        TaskSpec {
            run: vec![TaskStep::Command(command.to_string())],
            ..TaskSpec::default()
        }
    }

    fn sha256(digit: char) -> Checksum {
        Checksum {
            algorithm: ChecksumAlgorithm::Sha256,
            value: digit.to_string().repeat(64),
        }
    }

    #[test]
    fn default_plan_uses_schema_two() {
        assert_eq!(EnvironmentPlanV2::default().schema, 2);
    }

    #[test]
    fn schema_one_single_tool_shape_remains_valid() {
        let input = r#"{
          "schema": 1,
          "tools": {
            "node": {
              "requirement": "22",
              "resolved": "22.11.0",
              "provider": "core"
            }
          },
          "activation": "none"
        }"#;
        let plan = EnvironmentPlanV2::parse_json(input).unwrap();
        assert!(matches!(plan.tools.get("node"), Some(ToolSpec::One(_))));
        plan.validate(EnvironmentValidationMode::FrozenPortable)
            .unwrap();
    }

    #[test]
    fn schema_one_rejects_v2_features() {
        let mut plan = EnvironmentPlanV2 {
            schema: 1,
            ..EnvironmentPlanV2::default()
        };
        plan.env.insert(
            "NODE_ENV".to_string(),
            EnvironmentValue::String("test".to_string()),
        );
        assert!(matches!(
            plan.validate(EnvironmentValidationMode::Authoring),
            Err(EnvironmentPlanV2Error::FeatureRequiresSchema { .. })
        ));
    }

    #[test]
    fn authoring_accepts_ranges_but_frozen_requires_resolution() {
        let mut plan = EnvironmentPlanV2::default();
        let mut tool = exact_tool("22.11.0");
        tool.resolved = None;
        plan.tools.insert("node".to_string(), tool.into());

        plan.validate(EnvironmentValidationMode::Authoring).unwrap();
        assert!(matches!(
            plan.validate(EnvironmentValidationMode::FrozenPortable),
            Err(EnvironmentPlanV2Error::LegacyValidation { .. })
        ));
    }

    #[test]
    fn multiple_tool_versions_validate_in_frozen_mode() {
        let mut plan = EnvironmentPlanV2::default();
        plan.tools.insert(
            "node".to_string(),
            ToolSpec::Many(vec![
                ToolVersion::new(exact_tool("20.18.0")),
                ToolVersion::new(exact_tool("22.11.0")),
            ]),
        );
        plan.validate(EnvironmentValidationMode::FrozenPortable)
            .unwrap();
    }

    #[test]
    fn one_and_single_item_many_normalize_identically() {
        let mut one = EnvironmentPlanV2::default();
        one.tools.insert(
            "node".to_string(),
            ToolSpec::One(Box::new(ToolVersion::new(exact_tool("22.11.0")))),
        );
        let mut many = EnvironmentPlanV2::default();
        many.tools.insert(
            "node".to_string(),
            ToolSpec::Many(vec![ToolVersion::new(exact_tool("22.11.0"))]),
        );
        assert_eq!(
            one.canonical_json_bytes().unwrap(),
            many.canonical_json_bytes().unwrap()
        );
    }

    #[test]
    fn multiple_tool_version_order_is_preserved() {
        let mut plan = EnvironmentPlanV2::default();
        plan.tools.insert(
            "node".to_string(),
            ToolSpec::Many(vec![
                ToolVersion::new(exact_tool("22.11.0")),
                ToolVersion::new(exact_tool("20.18.0")),
            ]),
        );
        let normalized = plan.normalized();
        let versions = normalized.tools["node"].versions();
        assert_eq!(versions[0].requirement.resolved.as_deref(), Some("22.11.0"));
        assert_eq!(versions[1].requirement.resolved.as_deref(), Some("20.18.0"));
    }

    #[test]
    fn typed_values_roundtrip_through_toml() {
        let mut plan = EnvironmentPlanV2::default();
        plan.env.insert(
            "ZED_MATRIX".to_string(),
            EnvironmentValue::Table(BTreeMap::from([
                ("enabled".to_string(), EnvironmentValue::Boolean(true)),
                ("retries".to_string(), EnvironmentValue::Integer(3)),
                (
                    "ratios".to_string(),
                    EnvironmentValue::Array(vec![
                        EnvironmentValue::Float(0.5),
                        EnvironmentValue::Float(1.5),
                    ]),
                ),
            ])),
        );
        let text = plan.to_toml_string().unwrap();
        assert_eq!(
            EnvironmentPlanV2::parse_toml(&text).unwrap(),
            plan.normalized()
        );
    }

    #[test]
    fn non_finite_values_are_rejected() {
        let mut plan = EnvironmentPlanV2::default();
        plan.vars
            .insert("bad".to_string(), EnvironmentValue::Float(f64::NAN));
        assert!(matches!(
            plan.validate(EnvironmentValidationMode::Authoring),
            Err(EnvironmentPlanV2Error::NonFiniteValue { .. })
        ));
    }

    #[test]
    fn null_extensions_are_rejected_before_toml_serialization() {
        let mut plan = EnvironmentPlanV2::default();
        plan.extensions
            .insert("mise.future".to_string(), serde_json::Value::Null);
        assert!(matches!(
            plan.validate(EnvironmentValidationMode::Authoring),
            Err(EnvironmentPlanV2Error::NullExtensionValue { .. })
        ));
    }

    #[test]
    fn task_cycles_are_rejected() {
        let mut plan = EnvironmentPlanV2::default();
        let mut build = command_task("cargo build");
        build.depends.push("test".to_string());
        let mut test = command_task("cargo test");
        test.depends.push("build".to_string());
        plan.tasks.insert("build".to_string(), build);
        plan.tasks.insert("test".to_string(), test);
        assert!(matches!(
            plan.validate(EnvironmentValidationMode::Authoring),
            Err(EnvironmentPlanV2Error::TaskDependencyCycle { .. })
        ));
    }

    #[test]
    fn aliases_resolve_and_unknown_tasks_fail() {
        let mut plan = EnvironmentPlanV2::default();
        let mut build = command_task("cargo build");
        build.aliases.push("b".to_string());
        let mut test = command_task("cargo test");
        test.depends.push("b".to_string());
        plan.tasks.insert("build".to_string(), build);
        plan.tasks.insert("test".to_string(), test);
        plan.validate(EnvironmentValidationMode::Authoring).unwrap();

        plan.tasks
            .get_mut("test")
            .unwrap()
            .depends
            .push("missing".to_string());
        assert!(matches!(
            plan.validate(EnvironmentValidationMode::Authoring),
            Err(EnvironmentPlanV2Error::UnknownTaskDependency { .. })
        ));
    }

    #[test]
    fn duplicate_aliases_are_rejected() {
        let mut plan = EnvironmentPlanV2::default();
        let mut build = command_task("cargo build");
        build.aliases.push("x".to_string());
        let mut test = command_task("cargo test");
        test.aliases.push("x".to_string());
        plan.tasks.insert("build".to_string(), build);
        plan.tasks.insert("test".to_string(), test);
        assert!(matches!(
            plan.validate(EnvironmentValidationMode::Authoring),
            Err(EnvironmentPlanV2Error::DuplicateTaskAlias { .. })
        ));
    }

    #[test]
    fn task_command_order_survives_normalization() {
        let mut plan = EnvironmentPlanV2::default();
        let mut task = command_task("echo first");
        task.run.push(TaskStep::Command("echo second".to_string()));
        task.aliases = vec!["z".to_string(), "a".to_string()];
        plan.tasks.insert("ordered".to_string(), task);
        let normalized = plan.normalized();
        let task = &normalized.tasks["ordered"];
        assert_eq!(task.aliases, vec!["a".to_string(), "z".to_string()]);
        assert_eq!(
            task.run,
            vec![
                TaskStep::Command("echo first".to_string()),
                TaskStep::Command("echo second".to_string())
            ]
        );
    }

    #[test]
    fn sequential_task_group_order_survives_normalization() {
        let group = TaskGroup {
            tasks: vec!["second".to_string(), "first".to_string()],
            parallel: false,
        };
        assert_eq!(group.normalized().tasks, group.tasks);
    }

    #[test]
    fn task_local_tools_receive_frozen_validation() {
        let mut plan = EnvironmentPlanV2::default();
        let mut task = command_task("node test.js");
        task.tools
            .insert("node".to_string(), exact_tool("latest").into());
        plan.tasks.insert("test".to_string(), task);
        assert!(matches!(
            plan.validate(EnvironmentValidationMode::FrozenPortable),
            Err(EnvironmentPlanV2Error::LegacyValidation { .. })
        ));
    }

    #[test]
    fn task_paths_cannot_escape_the_project() {
        let mut plan = EnvironmentPlanV2::default();
        let mut task = command_task("cargo build");
        task.outputs.push("../outside/result".to_string());
        plan.tasks.insert("build".to_string(), task);
        assert!(matches!(
            plan.validate(EnvironmentValidationMode::FrozenPortable),
            Err(EnvironmentPlanV2Error::UnsafeRelativePath { .. })
        ));
        assert!(matches!(
            plan.validate(EnvironmentValidationMode::FrozenLocal),
            Err(EnvironmentPlanV2Error::UnsafeRelativePath { .. })
        ));
    }

    #[test]
    fn canonical_bytes_ignore_set_order_and_duplicates() {
        let mut first = EnvironmentPlanV2 {
            platforms: vec![
                "x86_64-linux".to_string(),
                "aarch64-darwin".to_string(),
                "x86_64-linux".to_string(),
            ],
            activation: ActivationPolicy::FrozenInstall,
            sources: vec![
                EnvironmentSource {
                    manager: EnvironmentManager::Mise,
                    path: "mise.toml".to_string(),
                    lock_path: Some("mise.lock".to_string()),
                    digest: Some(sha256('b')),
                },
                EnvironmentSource {
                    manager: EnvironmentManager::Mise,
                    path: "mise.toml".to_string(),
                    lock_path: Some("mise.lock".to_string()),
                    digest: Some(sha256('b')),
                },
            ],
            ..EnvironmentPlanV2::default()
        };
        let mut node = exact_tool("22.11.0");
        node.platforms = vec![
            "x86_64-linux".to_string(),
            "aarch64-darwin".to_string(),
            "x86_64-linux".to_string(),
        ];
        first.tools.insert("node".to_string(), node.into());
        let mut build = command_task("cargo build");
        build.aliases = vec!["b".to_string(), "build-all".to_string()];
        build.depends = vec!["lint".to_string(), "lint".to_string()];
        first.tasks.insert("build".to_string(), build);
        first
            .tasks
            .insert("lint".to_string(), command_task("cargo clippy"));

        let mut second = first.clone();
        second.platforms.reverse();
        second.sources.reverse();
        second.tools.get_mut("node").unwrap().versions_mut()[0]
            .requirement
            .platforms
            .reverse();
        second.tasks.get_mut("build").unwrap().aliases.reverse();
        second.tasks.get_mut("build").unwrap().depends.reverse();

        assert_eq!(
            first.canonical_json_bytes().unwrap(),
            second.canonical_json_bytes().unwrap()
        );
    }

    #[test]
    fn normalization_does_not_hide_invalid_map_keys() {
        let mut plan = EnvironmentPlanV2::default();
        plan.tools
            .insert(" node ".to_string(), exact_tool("22.11.0").into());
        assert!(matches!(
            plan.normalized()
                .validate(EnvironmentValidationMode::FrozenPortable),
            Err(EnvironmentPlanV2Error::InvalidName { .. })
        ));
    }
}
