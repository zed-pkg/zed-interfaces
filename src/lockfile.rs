use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::artifact::ArtifactFormat;
use crate::native_dependency::NativeDependencyLock;
use crate::native_registry::NativeRegistry;
use crate::nix::NixAdapterRecord;

type NativeDependencyKey = (NativeRegistry, String);
type NixAdapterKey = (String, String, String, Option<String>, u8, String, String);

/// The `.zpkg.lock` file written next to `.zpkg.toml` after resolution.
///
/// Serialized as TOML with one `[[package]]` table per locked Zed package,
/// optional `[[native-dependency]]` tables for exact npm/Cargo resolutions,
/// and optional `[[nix-adapter]]` tables for completed Nix translations.
/// Every entry pins exact immutable identity so frozen restore never needs to
/// reinterpret a native range or repeat an environment translation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Lockfile {
    pub version: u32,
    #[serde(default, rename = "package", skip_serializing_if = "Vec::is_empty")]
    pub packages: Vec<LockedPackage>,
    /// Exact source-aware npm/Cargo resolutions. This additive field keeps
    /// existing lockfile version 1 documents readable while newer writers can
    /// preserve native requirement translation and immutable artifact identity.
    #[serde(
        default,
        rename = "native-dependency",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub native_dependencies: Vec<NativeDependencyLock>,
    /// Optional immutable provenance for completed Nix interoperability
    /// translations. This additive field keeps lockfile version 1 readable by
    /// current consumers while allowing newer writers to preserve evidence.
    #[serde(default, rename = "nix-adapter", skip_serializing_if = "Vec::is_empty")]
    pub nix_adapters: Vec<NixAdapterRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LockedPackage {
    pub org: String,
    pub name: String,
    pub version: String,
    /// Hex sha256 of the artifact archive; also its store address.
    pub sha256: String,
    /// Artifact size in bytes.
    pub size: u64,
    #[serde(default)]
    pub format: ArtifactFormat,
    /// VCS tag the version was published from, e.g. `v1.2.0`.
    pub vcs_tag: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vcs_commit: Option<String>,
    /// Base URL of the registry the artifact was resolved from.
    pub source: String,
}

#[derive(Debug, thiserror::Error)]
pub enum LockfileError {
    #[error("lockfile toml error: {0}")]
    Toml(String),
    #[error("unsupported lockfile version {0} (this build supports {1})")]
    UnsupportedVersion(u32, u32),
    #[error("invalid native dependency provenance: {0}")]
    InvalidNativeDependency(String),
    #[error("duplicate native dependency key `{0}`")]
    DuplicateNativeDependency(String),
    #[error("invalid Nix adapter provenance: {0}")]
    InvalidNixAdapter(String),
    #[error("duplicate Nix adapter provenance key `{0}`")]
    DuplicateNixAdapter(String),
}

impl Default for Lockfile {
    fn default() -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            packages: Vec::new(),
            native_dependencies: Vec::new(),
            nix_adapters: Vec::new(),
        }
    }
}

impl Lockfile {
    pub const CURRENT_VERSION: u32 = 1;

    pub fn parse(input: &str) -> Result<Self, LockfileError> {
        let lockfile: Lockfile =
            toml::from_str(input).map_err(|error| LockfileError::Toml(error.to_string()))?;
        if lockfile.version > Self::CURRENT_VERSION {
            return Err(LockfileError::UnsupportedVersion(
                lockfile.version,
                Self::CURRENT_VERSION,
            ));
        }
        lockfile.validate_native_dependencies()?;
        lockfile.validate_nix_adapters()?;
        Ok(lockfile)
    }

    pub fn to_toml_string(&self) -> Result<String, LockfileError> {
        self.validate_native_dependencies()?;
        self.validate_nix_adapters()?;
        let mut normalized = self.clone();
        normalized
            .packages
            .sort_by(|left, right| (&left.org, &left.name).cmp(&(&right.org, &right.name)));
        normalized
            .native_dependencies
            .sort_by_key(native_dependency_key);
        normalized.nix_adapters.sort_by_key(nix_adapter_key);
        toml::to_string_pretty(&normalized).map_err(|error| LockfileError::Toml(error.to_string()))
    }

    pub fn find(&self, org: &str, name: &str) -> Option<&LockedPackage> {
        self.packages
            .iter()
            .find(|package| package.org == org && package.name == name)
    }

    /// Insert or replace the entry for `org/name`, keeping entries sorted.
    pub fn upsert(&mut self, package: LockedPackage) {
        self.packages
            .retain(|existing| !(existing.org == package.org && existing.name == package.name));
        self.packages.push(package);
        self.packages
            .sort_by(|a, b| (&a.org, &a.name).cmp(&(&b.org, &b.name)));
    }

    /// Return one exact native resolution by source registry and package name.
    pub fn find_native_dependency(
        &self,
        registry: NativeRegistry,
        package_name: &str,
    ) -> Option<&NativeDependencyLock> {
        self.native_dependencies.iter().find(|dependency| {
            dependency.requirement.registry == registry && dependency.package.name == package_name
        })
    }

    /// Validate and insert or replace one exact native resolution. V1 identity
    /// is `(registry, package.name)`, so a project cannot silently carry two
    /// different exact resolutions of the same native package.
    pub fn upsert_native_dependency(
        &mut self,
        dependency: NativeDependencyLock,
    ) -> Result<(), LockfileError> {
        dependency
            .validate()
            .map_err(|error| LockfileError::InvalidNativeDependency(error.to_string()))?;
        let key = native_dependency_key(&dependency);
        self.native_dependencies
            .retain(|existing| native_dependency_key(existing) != key);
        self.native_dependencies.push(dependency);
        self.native_dependencies.sort_by_key(native_dependency_key);
        Ok(())
    }

    /// Insert or replace one completed Nix translation. Identity includes
    /// package/target, direction, system, and selected output, so platform
    /// variants never overwrite each other.
    pub fn upsert_nix_adapter(&mut self, adapter: NixAdapterRecord) -> Result<(), LockfileError> {
        adapter
            .validate()
            .map_err(|error| LockfileError::InvalidNixAdapter(error.to_string()))?;
        let key = nix_adapter_key(&adapter);
        self.nix_adapters
            .retain(|existing| nix_adapter_key(existing) != key);
        self.nix_adapters.push(adapter);
        self.nix_adapters.sort_by_key(nix_adapter_key);
        Ok(())
    }

    fn validate_native_dependencies(&self) -> Result<(), LockfileError> {
        let mut seen = BTreeSet::new();
        for dependency in &self.native_dependencies {
            dependency
                .validate()
                .map_err(|error| LockfileError::InvalidNativeDependency(error.to_string()))?;
            let key = native_dependency_key(dependency);
            if !seen.insert(key) {
                return Err(LockfileError::DuplicateNativeDependency(
                    native_dependency_label(dependency),
                ));
            }
        }
        Ok(())
    }

    fn validate_nix_adapters(&self) -> Result<(), LockfileError> {
        let mut seen = BTreeSet::new();
        for adapter in &self.nix_adapters {
            adapter
                .validate()
                .map_err(|error| LockfileError::InvalidNixAdapter(error.to_string()))?;
            let key = nix_adapter_key(adapter);
            if !seen.insert(key) {
                return Err(LockfileError::DuplicateNixAdapter(nix_adapter_label(
                    adapter,
                )));
            }
        }
        Ok(())
    }
}

impl LockedPackage {
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.org, self.name)
    }
}

fn native_dependency_key(dependency: &NativeDependencyLock) -> NativeDependencyKey {
    (
        dependency.requirement.registry,
        dependency.package.name.clone(),
    )
}

fn native_dependency_label(dependency: &NativeDependencyLock) -> String {
    format!(
        "{:?}:{}",
        dependency.requirement.registry, dependency.package.name
    )
}

fn nix_adapter_key(adapter: &NixAdapterRecord) -> NixAdapterKey {
    match adapter {
        NixAdapterRecord::ZedToNix { package, .. } => (
            package.org.clone(),
            package.name.clone(),
            package.version.clone(),
            package.target.clone(),
            0,
            String::new(),
            String::new(),
        ),
        NixAdapterRecord::NixToZed {
            package, source, ..
        } => (
            package.org.clone(),
            package.name.clone(),
            package.version.clone(),
            package.target.clone(),
            1,
            source.realized.system.clone(),
            source.realized.output.clone(),
        ),
    }
}

fn nix_adapter_label(adapter: &NixAdapterRecord) -> String {
    let (direction, package, system, output) = match adapter {
        NixAdapterRecord::ZedToNix { package, .. } => ("zed-to-nix", package, "-", "-"),
        NixAdapterRecord::NixToZed {
            package, source, ..
        } => (
            "nix-to-zed",
            package,
            source.realized.system.as_str(),
            source.realized.output.as_str(),
        ),
    };
    format!(
        "{}/{}@{} target={} direction={direction} system={system} output={output}",
        package.org,
        package.name,
        package.version,
        package.target.as_deref().unwrap_or("-")
    )
}
