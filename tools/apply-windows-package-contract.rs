use std::{fs, process::ExitCode};

fn replace_once(source: &mut String, old: &str, new: &str, label: &str) -> Result<(), String> {
    let count = source.matches(old).count();
    if count != 1 {
        return Err(format!(
            "expected exactly one {label} replacement target, found {count}"
        ));
    }
    *source = source.replacen(old, new, 1);
    return Ok(());
}

fn run() -> Result<(), String> {
    let path = "src/rust/manifest.rs";
    let mut source = fs::read_to_string(path).map_err(|error| error.to_string())?;

    if source.contains("pub smoke_test_windows: Option<String>")
        && source.contains("pub command_windows: Option<String>")
        && source.contains("pub outputs_windows: Vec<String>")
    {
        return Ok(());
    }

    replace_once(
        &mut source,
        "    /// consumer project that has this package installed the same way a real\n    /// consumer would.\n    pub smoke_test: Option<String>,\n    /// VCS tag template that must exist and point at the published commit.\n",
        "    /// consumer project that has this package installed the same way a real\n    /// consumer would.\n    pub smoke_test: Option<String>,\n    /// Optional native-Windows smoke command. When present, Windows hosts run\n    /// this command through the native PowerShell execution contract instead\n    /// of the POSIX `smoke_test`. Omitted manifests retain the legacy fallback.\n    #[serde(default, skip_serializing_if = \"Option::is_none\")]\n    pub smoke_test_windows: Option<String>,\n    /// VCS tag template that must exist and point at the published commit.\n",
        "publish smoke_test_windows field",
    )?;

    replace_once(
        &mut source,
        "            include_readme: false,\n            smoke_test: None,\n            tag_format: \"v{version}\".to_string(),\n",
        "            include_readme: false,\n            smoke_test: None,\n            smoke_test_windows: None,\n            tag_format: \"v{version}\".to_string(),\n",
        "publish default",
    )?;

    replace_once(
        &mut source,
        "}\n\n#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]\n#[serde(default)]\npub struct ScriptsSection {\n",
        "}\n\nimpl PublishSection {\n    /// Resolve the smoke command for a host without consulting ambient shell state.\n    /// Native Windows overrides are additive; older manifests fall back to the\n    /// existing POSIX/default command so the schema change is backwards-compatible.\n    pub fn smoke_test_for_host(&self, windows: bool) -> Option<&str> {\n        if windows {\n            return self\n                .smoke_test_windows\n                .as_deref()\n                .or(self.smoke_test.as_deref());\n        }\n        return self.smoke_test.as_deref();\n    }\n}\n\n#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]\n#[serde(default)]\npub struct ScriptsSection {\n",
        "publish host resolver",
    )?;

    replace_once(
        &mut source,
        "/// A post-extract build step. Because compiled output is OS/arch-specific,\n/// zed-pkg runs `command` via `sh -c` inside a sandboxed staging copy of the\n/// source and caches the result in a build cache keyed by\n/// `(source sha256, target triple, command)` — separate from the universal,\n/// platform-independent source store (zed-docs issue #5).\n#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]\npub struct BuildSection {\n    /// Command executed after extraction, e.g. `make` or `cargo build --release`.\n    pub command: String,\n    /// Paths (relative to the package root) to keep from the staging build.\n    /// Empty means keep everything.\n    #[serde(default, skip_serializing_if = \"Vec::is_empty\")]\n    pub outputs: Vec<String>,\n}\n",
        "/// A post-extract build step. Compiled output is OS/arch-specific. The\n/// default command retains the historical POSIX execution contract; Windows\n/// may opt into an explicit native command and output list. The effective host\n/// command is part of the build-cache identity.\n#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]\npub struct BuildSection {\n    /// Default/Unix command executed after extraction.\n    pub command: String,\n    /// Optional native-Windows build command. Zed executes this through its\n    /// PowerShell contract when running on Windows.\n    #[serde(default, skip_serializing_if = \"Option::is_none\")]\n    pub command_windows: Option<String>,\n    /// Paths (relative to the package root) to keep from the staging build.\n    /// Empty means keep everything.\n    #[serde(default, skip_serializing_if = \"Vec::is_empty\")]\n    pub outputs: Vec<String>,\n    /// Optional Windows-specific output paths, for example Cargo `.exe` files.\n    /// An empty list falls back to `outputs` for backwards compatibility.\n    #[serde(default, skip_serializing_if = \"Vec::is_empty\")]\n    pub outputs_windows: Vec<String>,\n}\n\nimpl BuildSection {\n    pub fn command_for_host(&self, windows: bool) -> &str {\n        if windows {\n            return self.command_windows.as_deref().unwrap_or(&self.command);\n        }\n        return &self.command;\n    }\n\n    pub fn outputs_for_host(&self, windows: bool) -> &[String] {\n        if windows && !self.outputs_windows.is_empty() {\n            return &self.outputs_windows;\n        }\n        return &self.outputs;\n    }\n}\n",
        "build platform contract",
    )?;

    replace_once(
        &mut source,
        "    #[error(\"invalid build section: {0}\")]\n    InvalidBuild(String),\n    #[error(\"invalid native dependency declaration for `{0}`: {1}\")]\n",
        "    #[error(\"invalid build section: {0}\")]\n    InvalidBuild(String),\n    #[error(\"invalid publish section: {0}\")]\n    InvalidPublish(String),\n    #[error(\"invalid native dependency declaration for `{0}`: {1}\")]\n",
        "publish validation error",
    )?;

    replace_once(
        &mut source,
        "        let overriding = self.overrides.build.values();\n        for build in self.build.iter().chain(overriding) {\n            if build.command.trim().is_empty() {\n                return Err(ManifestError::InvalidBuild(\n                    \"command must not be empty\".to_string(),\n                ));\n            }\n            for output in &build.outputs {\n                if !is_safe_relative_path(output) {\n                    return Err(ManifestError::InvalidBuild(format!(\n                        \"output `{output}` must be a relative path without `..`\"\n                    )));\n                }\n            }\n        }\n",
        "        if let Some(command) = self.publish.smoke_test_windows.as_deref() {\n            if command.trim().is_empty() {\n                return Err(ManifestError::InvalidPublish(\n                    \"smoke_test_windows must not be empty\".to_string(),\n                ));\n            }\n            if command.contains('\\0') || command.len() > 32 * 1024 {\n                return Err(ManifestError::InvalidPublish(\n                    \"smoke_test_windows must be at most 32768 bytes and contain no NUL\"\n                        .to_string(),\n                ));\n            }\n        }\n        let overriding = self.overrides.build.values();\n        for build in self.build.iter().chain(overriding) {\n            if build.command.trim().is_empty() {\n                return Err(ManifestError::InvalidBuild(\n                    \"command must not be empty\".to_string(),\n                ));\n            }\n            if let Some(command) = build.command_windows.as_deref() {\n                if command.trim().is_empty() {\n                    return Err(ManifestError::InvalidBuild(\n                        \"command_windows must not be empty\".to_string(),\n                    ));\n                }\n                if command.contains('\\0') || command.len() > 32 * 1024 {\n                    return Err(ManifestError::InvalidBuild(\n                        \"command_windows must be at most 32768 bytes and contain no NUL\"\n                            .to_string(),\n                    ));\n                }\n            }\n            for output in build.outputs.iter().chain(&build.outputs_windows) {\n                if !is_safe_relative_path(output) {\n                    return Err(ManifestError::InvalidBuild(format!(\n                        \"output `{output}` must be a relative path without `..`\"\n                    )));\n                }\n            }\n        }\n",
        "platform command validation",
    )?;

    fs::write(path, source).map_err(|error| error.to_string())?;
    return Ok(());
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!("windows package contract patch failed: {error}");
            return ExitCode::FAILURE;
        }
    }
}
