from pathlib import Path
import re

path = Path("src/lockfile.rs")
source = path.read_text(encoding="utf-8")


def replace_once(old: str, new: str, label: str) -> None:
    global source
    count = source.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    source = source.replace(old, new, 1)


def sub_once(pattern: str, replacement, label: str) -> None:
    global source
    source, count = re.subn(pattern, replacement, source, count=1, flags=re.DOTALL)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")


replace_once(
    "use crate::artifact::ArtifactFormat;\nuse crate::nix::NixAdapterRecord;\n\ntype NixAdapterKey =",
    "use crate::artifact::ArtifactFormat;\n"
    "use crate::native_dependency::NativeDependencyLock;\n"
    "use crate::native_registry::NativeRegistry;\n"
    "use crate::nix::NixAdapterRecord;\n\n"
    "type NativeDependencyKey = (NativeRegistry, String);\n"
    "type NixAdapterKey =",
    "native dependency imports and key",
)

replace_once(
    """/// Serialized as TOML with one `[[package]]` table per locked package,
/// Cargo.lock-style. Every entry pins the exact artifact hash and the VCS
/// tag it was published from, so installs are reproducible and every
/// artifact is traceable back to source.""",
    """/// Serialized as TOML with one `[[package]]` table per locked Zed package,
/// optional `[[native-dependency]]` tables for exact npm/Cargo resolutions,
/// and optional `[[nix-adapter]]` tables for completed Nix translations.
/// Every entry pins exact immutable identity so frozen restore never needs to
/// reinterpret a native range or repeat an environment translation.""",
    "lockfile format documentation",
)

sub_once(
    r'''(\s+#\[serde\(default, rename = "package", skip_serializing_if = "Vec::is_empty"\)\]\n\s+pub packages: Vec<LockedPackage>,\n)(\s+/// Optional immutable provenance for completed Nix interoperability)''',
    lambda match: match.group(1)
    + """    /// Exact source-aware npm/Cargo resolutions. This additive field keeps
    /// existing lockfile version 1 documents readable while newer writers can
    /// preserve native requirement translation and immutable artifact identity.
    #[serde(
        default,
        rename = "native-dependency",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub native_dependencies: Vec<NativeDependencyLock>,
"""
    + match.group(2),
    "native dependency lockfile field",
)

replace_once(
    """    #[error("duplicate locked package identity `{0}`")]
    DuplicatePackage(String),
    #[error("invalid Nix adapter provenance: {0}")]""",
    """    #[error("duplicate locked package identity `{0}`")]
    DuplicatePackage(String),
    #[error("invalid native dependency provenance: {0}")]
    InvalidNativeDependency(String),
    #[error("duplicate native dependency key `{0}`")]
    DuplicateNativeDependency(String),
    #[error("invalid Nix adapter provenance: {0}")]""",
    "native dependency errors",
)

sub_once(
    r'''(version:\s*Self::CURRENT_VERSION,\s*\n\s*packages:\s*Vec::new\(\),\s*\n)(\s*nix_adapters:\s*Vec::new\(\),)''',
    lambda match: match.group(1)
    + "            native_dependencies: Vec::new(),\n"
    + match.group(2),
    "lockfile default",
)

replace_once(
    "lockfile.validate_packages()?;\n        lockfile.validate_nix_adapters()?;",
    "lockfile.validate_packages()?;\n"
    "        lockfile.validate_native_dependencies()?;\n"
    "        lockfile.validate_nix_adapters()?;",
    "parse validation order",
)

sub_once(
    r'''    pub fn to_toml_string\(&self\) -> Result<String, LockfileError> \{.*?\n    \}\n\n    pub fn find\(''',
    """    pub fn to_toml_string(&self) -> Result<String, LockfileError> {
        let mut normalized = self.clone();
        normalized.normalize_missing_package_revisions()?;
        normalized.validate_packages()?;
        normalized.validate_native_dependencies()?;
        normalized.validate_nix_adapters()?;
        normalized
            .packages
            .sort_by(|left, right| (&left.org, &left.name).cmp(&(&right.org, &right.name)));
        normalized
            .native_dependencies
            .sort_by_key(native_dependency_key);
        normalized.nix_adapters.sort_by_key(nix_adapter_key);
        toml::to_string_pretty(&normalized)
            .map_err(|error| LockfileError::Toml(error.to_string()))
    }

    pub fn find(""",
    "canonical writer",
)

replace_once(
    """    /// Insert or replace one completed Nix translation. Identity includes
    /// package/target, direction, system, and selected output, so platform
    /// variants never overwrite each other.""",
    """    /// Return one exact native resolution by source registry and package name.
    pub fn find_native_dependency(
        &self,
        registry: NativeRegistry,
        package_name: &str,
    ) -> Option<&NativeDependencyLock> {
        self.native_dependencies.iter().find(|dependency| {
            dependency.requirement.registry == registry
                && dependency.package.name == package_name
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
    /// variants never overwrite each other.""",
    "native dependency lookup and upsert",
)

replace_once(
    "    fn validate_nix_adapters(&self) -> Result<(), LockfileError> {",
    """    fn validate_native_dependencies(&self) -> Result<(), LockfileError> {
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

    fn validate_nix_adapters(&self) -> Result<(), LockfileError> {""",
    "native dependency validation",
)

replace_once(
    "fn nix_adapter_key(adapter: &NixAdapterRecord) -> NixAdapterKey {",
    """fn native_dependency_key(dependency: &NativeDependencyLock) -> NativeDependencyKey {
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

fn nix_adapter_key(adapter: &NixAdapterRecord) -> NixAdapterKey {""",
    "native dependency key helpers",
)

path.write_text(source, encoding="utf-8")
