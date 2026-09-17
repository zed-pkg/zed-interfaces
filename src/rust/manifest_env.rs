use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Value type declared for one `.zpkg.toml` `[[env]]` entry.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum EnvKind {
    String,
    Bool,
    Integer,
    Double,
    Json,
    Url,
    DurationMs,
    StringList,
    IntegerList,
}

/// Whether an environment binding is process-only or may also have an argv form.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum EnvExposure {
    EnvOnly,
    ArgvAndEnv,
}

/// Deployment profiles in which an environment binding is meaningful.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum EnvEnvironment {
    Dev,
    Stage,
    Prod,
}

/// Machine-readable declaration for one environment variable consumed by a package.
///
/// The field vocabulary intentionally matches the ORES env-manifest v1 contract.
/// This is package/input metadata: it documents and validates the environment
/// surface but does not inject values into a process and must never contain a
/// secret value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EnvDeclaration {
    /// Stable snake_case logical binding name.
    #[schemars(length(min = 1, max = 64), regex(pattern = r"^[a-z][a-z0-9_]*$"))]
    pub name: String,
    /// Portable uppercase process environment-variable key.
    #[schemars(length(min = 1, max = 128), regex(pattern = r"^[A-Z_][A-Z0-9_]*$"))]
    pub key: String,
    pub kind: EnvKind,
    pub required: bool,
    pub secret: bool,
    pub exposure: EnvExposure,
    /// Human-readable explanation suitable for generated inventories.
    #[schemars(length(min = 1, max = 512))]
    pub description: String,
    /// Semantic/config paths controlled by this variable.
    #[schemars(length(min = 1, max = 32))]
    pub overrides: Vec<String>,
    /// Exact deployment profiles where the key belongs.
    #[schemars(length(min = 1, max = 3))]
    pub environments: Vec<EnvEnvironment>,
    /// Optional non-secret fallback, encoded as text for downstream coercion.
    #[serde(
        default,
        rename = "defaultValue",
        alias = "default_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub default_value: Option<String>,
}

impl EnvDeclaration {
    /// Validate semantics that JSON Schema cannot express on its own.
    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty()
            || self.name.len() > 64
            || !self.name.chars().enumerate().all(|(index, character)| {
                character == '_'
                    || character.is_ascii_lowercase()
                    || (index > 0 && character.is_ascii_digit())
            })
            || !self.name.as_bytes()[0].is_ascii_lowercase()
        {
            return Err(format!(
                "env name `{}` must match [a-z][a-z0-9_]* and be at most 64 bytes",
                self.name
            ));
        }
        if self.key.is_empty()
            || self.key.len() > 128
            || !self.key.chars().enumerate().all(|(index, character)| {
                character == '_'
                    || character.is_ascii_uppercase()
                    || (index > 0 && character.is_ascii_digit())
            })
        {
            return Err(format!(
                "env key `{}` must match [A-Z_][A-Z0-9_]* and be at most 128 bytes",
                self.key
            ));
        }
        if self.description.trim().is_empty() || self.description.len() > 512 {
            return Err(format!(
                "env `{}` description must be 1-512 bytes",
                self.key
            ));
        }
        if self.overrides.is_empty() || self.overrides.len() > 32 {
            return Err(format!(
                "env `{}` must declare 1-32 semantic override paths",
                self.key
            ));
        }
        let mut override_paths = BTreeSet::new();
        for path in &self.overrides {
            if path.trim().is_empty() || path.len() > 512 || path.chars().any(char::is_control) {
                return Err(format!(
                    "env `{}` has an invalid semantic override path",
                    self.key
                ));
            }
            if !override_paths.insert(path) {
                return Err(format!(
                    "env `{}` repeats semantic override path `{path}`",
                    self.key
                ));
            }
        }
        if self.environments.is_empty() || self.environments.len() > 3 {
            return Err(format!(
                "env `{}` must declare 1-3 deployment environments",
                self.key
            ));
        }
        let unique_environments = self.environments.iter().copied().collect::<BTreeSet<_>>();
        if unique_environments.len() != self.environments.len() {
            return Err(format!(
                "env `{}` repeats a deployment environment",
                self.key
            ));
        }
        if self.secret && self.exposure != EnvExposure::EnvOnly {
            return Err(format!(
                "secret env `{}` must use exposure = `env-only`",
                self.key
            ));
        }
        if self.secret && self.default_value.is_some() {
            return Err(format!(
                "secret env `{}` must not declare defaultValue",
                self.key
            ));
        }
        if self.default_value.as_ref().is_some_and(|value| value.len() > 8192) {
            return Err(format!(
                "env `{}` defaultValue exceeds 8192 bytes",
                self.key
            ));
        }
        Ok(())
    }
}

/// Validate a package's whole env inventory, including cross-entry uniqueness.
pub fn validate_env_declarations(declarations: &[EnvDeclaration]) -> Result<(), String> {
    if declarations.len() > 512 {
        return Err("manifest declares more than 512 environment variables".to_string());
    }

    let mut names = BTreeSet::new();
    let mut keys = BTreeSet::new();
    for declaration in declarations {
        declaration.validate()?;
        if !names.insert(declaration.name.as_str()) {
            return Err(format!(
                "duplicate env logical name `{}`",
                declaration.name
            ));
        }
        if !keys.insert(declaration.key.as_str()) {
            return Err(format!("duplicate env key `{}`", declaration.key));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid() -> EnvDeclaration {
        EnvDeclaration {
            name: "signal_handlers".to_string(),
            key: "ORES_CLIS_SIGNAL_HANDLERS".to_string(),
            kind: EnvKind::Bool,
            required: false,
            secret: false,
            exposure: EnvExposure::EnvOnly,
            description: "Enable opt-in signal handling.".to_string(),
            overrides: vec!["signal_handlers.enabled".to_string()],
            environments: vec![
                EnvEnvironment::Dev,
                EnvEnvironment::Stage,
                EnvEnvironment::Prod,
            ],
            default_value: Some("true".to_string()),
        }
    }

    #[test]
    fn valid_env_declaration_roundtrips_ores_wire_names() {
        let declaration = valid();
        let text = toml::to_string(&declaration).unwrap();
        assert!(text.contains("defaultValue = \"true\""));
        let parsed: EnvDeclaration = toml::from_str(&text).unwrap();
        assert_eq!(parsed, declaration);
    }

    #[test]
    fn secret_defaults_and_argv_exposure_fail_closed() {
        let mut declaration = valid();
        declaration.secret = true;
        assert!(declaration.validate().unwrap_err().contains("env-only"));
        declaration.exposure = EnvExposure::EnvOnly;
        assert!(declaration.validate().unwrap_err().contains("defaultValue"));
    }

    #[test]
    fn duplicate_keys_and_names_fail_closed() {
        let first = valid();
        let mut second = valid();
        second.name = "other".to_string();
        assert!(validate_env_declarations(&[first.clone(), second]).is_err());

        let mut second = first.clone();
        second.key = "OTHER_ENV".to_string();
        assert!(validate_env_declarations(&[first, second]).is_err());
    }
}
