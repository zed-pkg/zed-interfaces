#!/usr/bin/env python3
"""Apply the DEN-100 native release routing schema to src/manifest.rs.

Temporary branch-local helper. It is intentionally deterministic and refuses to
continue when an expected insertion point has changed.
"""

from pathlib import Path


PATH = Path("src/manifest.rs")
text = PATH.read_text(encoding="utf-8")


def replace_once(old: str, new: str, label: str) -> None:
    global text
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected exactly one insertion point, found {count}")
    text = text.replace(old, new, 1)


replace_once(
    '''    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter: Option<String>,
}

/// A post-extract build step.''',
    '''    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter: Option<String>,
    /// Optional routing to this target's native ecosystem registry. This is
    /// declarative metadata only: the native manifest remains authoritative,
    /// and arbitrary commands are intentionally not representable here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native: Option<NativeReleaseSection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NativeReleaseSection {
    pub registry: NativeRegistry,
    pub package: String,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum NativeRegistry {
    Npm,
    CratesIo,
}

impl NativeRegistry {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Npm => "npm",
            Self::CratesIo => "crates-io",
        }
    }

    fn validate_package(self, package: &str) -> Result<(), String> {
        let valid = match self {
            Self::Npm => is_valid_npm_package(package),
            Self::CratesIo => is_valid_crates_package(package),
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NativeReleaseRoute {
    pub target: String,
    pub dir: String,
    pub registry: NativeRegistry,
    pub package: String,
}

fn is_valid_npm_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 214
        && !value.starts_with('.')
        && !value.starts_with('_')
        && !value.contains("..")
        && value.chars().all(|c| {
            c.is_ascii_lowercase()
                || c.is_ascii_digit()
                || matches!(c, '-' | '_' | '.' | '~')
        })
}

fn is_valid_npm_package(value: &str) -> bool {
    if let Some(scoped) = value.strip_prefix('@') {
        let Some((scope, package)) = scoped.split_once('/') else {
            return false;
        };
        !package.contains('/')
            && is_valid_npm_component(scope)
            && is_valid_npm_component(package)
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

/// A post-extract build step.''',
    "TargetSection/native types",
)

replace_once(
    '''    #[error("invalid target `{0}`: {1}")]
    InvalidTarget(String, String),
    #[error("manifest toml error: {0}")]''',
    '''    #[error("invalid target `{0}`: {1}")]
    InvalidTarget(String, String),
    #[error("invalid native release route for target `{0}`: {1}")]
    InvalidNativeRoute(String, String),
    #[error("manifest toml error: {0}")]''',
    "ManifestError",
)

replace_once(
    '''        let mut target_dirs = BTreeMap::<&str, &str>::new();
        let mut published_names = BTreeMap::<String, &str>::new();''',
    '''        let mut target_dirs = BTreeMap::<&str, &str>::new();
        let mut published_names = BTreeMap::<String, &str>::new();
        let mut native_routes = BTreeMap::<(NativeRegistry, String), &str>::new();''',
    "target validation maps",
)

replace_once(
    '''            if let Some(adapter) = target.adapter.as_deref()
                && !matches!(adapter, "node" | "java" | "none")
            {
                return Err(ManifestError::InvalidTarget(
                    name.clone(),
                    format!(
                        "adapter `{adapter}` is unsupported; expected `node`, `java`, or `none`"
                    ),
                ));
            }
        }''',
    '''            if let Some(adapter) = target.adapter.as_deref()
                && !matches!(adapter, "node" | "java" | "none")
            {
                return Err(ManifestError::InvalidTarget(
                    name.clone(),
                    format!(
                        "adapter `{adapter}` is unsupported; expected `node`, `java`, or `none`"
                    ),
                ));
            }
            if let Some(native) = &target.native {
                if target.dir == "." {
                    return Err(ManifestError::InvalidNativeRoute(
                        name.clone(),
                        "the whole-repository target cannot publish to a native registry"
                            .to_string(),
                    ));
                }
                native
                    .registry
                    .validate_package(&native.package)
                    .map_err(|reason| ManifestError::InvalidNativeRoute(name.clone(), reason))?;
                let route = (native.registry, native.package.clone());
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
        }''',
    "native route validation",
)

replace_once(
    '''    /// Every `(target, published name)` pair this manifest fans out to, sorted
    /// by target for deterministic publish order and output.
    pub fn target_package_names(&self) -> Vec<(String, String)> {''',
    '''    /// Native release routes sorted by target name, suitable for deterministic
    /// credential-free planning before any registry adapter executes.
    pub fn native_release_routes(&self) -> Vec<NativeReleaseRoute> {
        self.targets
            .iter()
            .filter_map(|(target, section)| {
                section.native.as_ref().map(|native| NativeReleaseRoute {
                    target: target.clone(),
                    dir: section.dir.clone(),
                    registry: native.registry,
                    package: native.package.clone(),
                })
            })
            .collect()
    }

    /// Every `(target, published name)` pair this manifest fans out to, sorted
    /// by target for deterministic publish order and output.
    pub fn target_package_names(&self) -> Vec<(String, String)> {''',
    "native_release_routes method",
)

PATH.write_text(text, encoding="utf-8")
print(f"updated {PATH}")
