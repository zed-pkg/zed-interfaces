#!/usr/bin/env python3
"""Materialize the semantic native-dependency merge onto current lockfile code.

This helper is intentionally branch-scoped. It asserts every source snippet it
changes so concurrent edits fail closed instead of producing an approximate
merge. The generated files are uploaded as CI evidence and are never committed
by the workflow itself.
"""

from pathlib import Path

PATH_CANDIDATES = (Path("src/rust/lockfile.rs"), Path("src/lockfile.rs"))
PATH = next((path for path in PATH_CANDIDATES if path.is_file()), None)
if PATH is None:
    expected = ", ".join(str(path) for path in PATH_CANDIDATES)
    raise SystemExit(f"lockfile source not found; expected one of: {expected}")
text = PATH.read_text(encoding="utf-8")

# The native lockfile product is already part of current main. Keep this
# branch-scoped materializer useful as a fail-closed CI check after the Rust
# crate moved under src/rust, while retaining its original ability to compose
# the older candidate branch.
if "pub native_dependencies: Vec<NativeDependencyLock>" in text:
    required_fragments = (
        "use crate::native_dependency::NativeDependencyLock;",
        "use crate::native_registry::NativeRegistry;",
        "lockfile.validate_native_dependencies()?;",
        "normalized.validate_native_dependencies()?;",
        "fn validate_native_dependencies(&self) -> Result<(), LockfileError>",
    )
    missing = [fragment for fragment in required_fragments if fragment not in text]
    if missing:
        raise SystemExit(
            "native dependency field exists without its complete contract: "
            + ", ".join(missing)
        )
    print(f"verified existing semantic native-dependency lockfile merge in {PATH}")
    raise SystemExit(0)


def replace_once(old: str, new: str, label: str) -> None:
    global text
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one source match, found {count}")
    text = text.replace(old, new, 1)


replace_once(
    "use crate::artifact::ArtifactFormat;\nuse crate::nix::NixAdapterRecord;\n\n"
    "type NixAdapterKey = (String, String, String, Option<String>, u8, String, String);",
    "use crate::artifact::ArtifactFormat;\n"
    "use crate::native_dependency::NativeDependencyLock;\n"
    "use crate::native_registry::NativeRegistry;\n"
    "use crate::nix::NixAdapterRecord;\n\n"
    "type NativeDependencyKey = (NativeRegistry, String);\n"
    "type NixAdapterKey = (String, String, String, Option<String>, u8, String, String);",
    "imports and key types",
)

replace_once(
    "/// Serialized as TOML with one `[[package]]` table per locked package,\n"
    "/// Cargo.lock-style. Every entry pins the exact artifact hash and the VCS\n"
    "/// tag it was published from, so installs are reproducible and every\n"
    "/// artifact is traceable back to source.",
    "/// Serialized as TOML with one `[[package]]` table per locked Zed package,\n"
    "/// optional `[[native-dependency]]` tables for exact npm/Cargo resolutions,\n"
    "/// and optional `[[nix-adapter]]` tables for completed Nix translations.\n"
    "/// Every entry pins exact immutable identity so frozen restore never needs to\n"
    "/// reinterpret a native range or repeat an environment translation.",
    "lockfile documentation",
)

replace_once(
    "    #[serde(default, rename = \"package\", skip_serializing_if = \"Vec::is_empty\")]\n"
    "    pub packages: Vec<LockedPackage>,\n"
    "    /// Optional immutable provenance for completed Nix interoperability",
    "    #[serde(default, rename = \"package\", skip_serializing_if = \"Vec::is_empty\")]\n"
    "    pub packages: Vec<LockedPackage>,\n"
    "    /// Exact source-aware npm/Cargo resolutions. This additive field keeps\n"
    "    /// existing lockfile version 1 documents readable while newer writers can\n"
    "    /// preserve native requirement translation and immutable artifact identity.\n"
    "    #[serde(\n"
    "        default,\n"
    "        rename = \"native-dependency\",\n"
    "        skip_serializing_if = \"Vec::is_empty\"\n"
    "    )]\n"
    "    pub native_dependencies: Vec<NativeDependencyLock>,\n"
    "    /// Optional immutable provenance for completed Nix interoperability",
    "native dependency field",
)

replace_once(
    "    #[error(\"duplicate locked package identity `{0}`\")]\n"
    "    DuplicatePackage(String),\n"
    "    #[error(\"invalid Nix adapter provenance: {0}\")]",
    "    #[error(\"duplicate locked package identity `{0}`\")]\n"
    "    DuplicatePackage(String),\n"
    "    #[error(\"invalid native dependency provenance: {0}\")]\n"
    "    InvalidNativeDependency(String),\n"
    "    #[error(\"duplicate native dependency key `{0}`\")]\n"
    "    DuplicateNativeDependency(String),\n"
    "    #[error(\"invalid Nix adapter provenance: {0}\")]",
    "native dependency errors",
)

replace_once(
    "            version: Self::CURRENT_VERSION,\n"
    "            packages: Vec::new(),\n"
    "            nix_adapters: Vec::new(),",
    "            version: Self::CURRENT_VERSION,\n"
    "            packages: Vec::new(),\n"
    "            native_dependencies: Vec::new(),\n"
    "            nix_adapters: Vec::new(),",
    "default lockfile",
)

replace_once(
    "        lockfile.validate_packages()?;\n"
    "        lockfile.validate_nix_adapters()?;",
    "        lockfile.validate_packages()?;\n"
    "        lockfile.validate_native_dependencies()?;\n"
    "        lockfile.validate_nix_adapters()?;",
    "parse validation order",
)

replace_once(
    "        normalized.normalize_missing_package_revisions()?;\n"
    "        normalized.validate_packages()?;\n"
    "        normalized.validate_nix_adapters()?;\n"
    "        normalized.nix_adapters.sort_by_key(nix_adapter_key);",
    "        normalized.normalize_missing_package_revisions()?;\n"
    "        normalized.validate_packages()?;\n"
    "        normalized.validate_native_dependencies()?;\n"
    "        normalized.validate_nix_adapters()?;\n"
    "        normalized\n"
    "            .packages\n"
    "            .sort_by(|left, right| (&left.org, &left.name).cmp(&(&right.org, &right.name)));\n"
    "        normalized\n"
    "            .native_dependencies\n"
    "            .sort_by_key(native_dependency_key);\n"
    "        normalized.nix_adapters.sort_by_key(nix_adapter_key);",
    "canonical write validation and ordering",
)

replace_once(
    "    /// Insert or replace one completed Nix translation. Identity includes\n",
    "    /// Return one exact native resolution by source registry and package name.\n"
    "    pub fn find_native_dependency(\n"
    "        &self,\n"
    "        registry: NativeRegistry,\n"
    "        package_name: &str,\n"
    "    ) -> Option<&NativeDependencyLock> {\n"
    "        self.native_dependencies.iter().find(|dependency| {\n"
    "            dependency.requirement.registry == registry && dependency.package.name == package_name\n"
    "        })\n"
    "    }\n\n"
    "    /// Validate and insert or replace one exact native resolution. V1 identity\n"
    "    /// is `(registry, package.name)`, so a project cannot silently carry two\n"
    "    /// different exact resolutions of the same native package.\n"
    "    pub fn upsert_native_dependency(\n"
    "        &mut self,\n"
    "        dependency: NativeDependencyLock,\n"
    "    ) -> Result<(), LockfileError> {\n"
    "        dependency\n"
    "            .validate()\n"
    "            .map_err(|error| LockfileError::InvalidNativeDependency(error.to_string()))?;\n"
    "        let key = native_dependency_key(&dependency);\n"
    "        self.native_dependencies\n"
    "            .retain(|existing| native_dependency_key(existing) != key);\n"
    "        self.native_dependencies.push(dependency);\n"
    "        self.native_dependencies.sort_by_key(native_dependency_key);\n"
    "        Ok(())\n"
    "    }\n\n"
    "    /// Insert or replace one completed Nix translation. Identity includes\n",
    "native dependency public helpers",
)

replace_once(
    "    fn validate_nix_adapters(&self) -> Result<(), LockfileError> {",
    "    fn validate_native_dependencies(&self) -> Result<(), LockfileError> {\n"
    "        let mut seen = BTreeSet::new();\n"
    "        for dependency in &self.native_dependencies {\n"
    "            dependency\n"
    "                .validate()\n"
    "                .map_err(|error| LockfileError::InvalidNativeDependency(error.to_string()))?;\n"
    "            let key = native_dependency_key(dependency);\n"
    "            if !seen.insert(key) {\n"
    "                return Err(LockfileError::DuplicateNativeDependency(\n"
    "                    native_dependency_label(dependency),\n"
    "                ));\n"
    "            }\n"
    "        }\n"
    "        Ok(())\n"
    "    }\n\n"
    "    fn validate_nix_adapters(&self) -> Result<(), LockfileError> {",
    "native dependency collection validation",
)

replace_once(
    "fn nix_adapter_key(adapter: &NixAdapterRecord) -> NixAdapterKey {",
    "fn native_dependency_key(dependency: &NativeDependencyLock) -> NativeDependencyKey {\n"
    "    (\n"
    "        dependency.requirement.registry,\n"
    "        dependency.package.name.clone(),\n"
    "    )\n"
    "}\n\n"
    "fn native_dependency_label(dependency: &NativeDependencyLock) -> String {\n"
    "    format!(\n"
    "        \"{:?}:{}\",\n"
    "        dependency.requirement.registry, dependency.package.name\n"
    "    )\n"
    "}\n\n"
    "fn nix_adapter_key(adapter: &NixAdapterRecord) -> NixAdapterKey {",
    "native dependency identity helpers",
)

literal = (
    "            packages: vec![package_without_commit(digest)],\n"
    "            nix_adapters: Vec::new(),"
)
replacement = (
    "            packages: vec![package_without_commit(digest)],\n"
    "            native_dependencies: Vec::new(),\n"
    "            nix_adapters: Vec::new(),"
)
count = text.count(literal)
if count != 2:
    raise SystemExit(f"test lockfile literals: expected two matches, found {count}")
text = text.replace(literal, replacement)

if "native_dependencies: Vec::new()" not in text:
    raise SystemExit("native dependency defaults were not materialized")
if text.count("pub native_dependencies: Vec<NativeDependencyLock>") != 1:
    raise SystemExit("native dependency field was not materialized exactly once")

PATH.write_text(text, encoding="utf-8")
print("materialized semantic native-dependency lockfile merge")
