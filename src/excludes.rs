//! Publish-time exclusion rules.
//!
//! zed-pkg's core disk-space promise: published artifacts carry what runs,
//! not what develops. Tests, CI configuration, VCS metadata, and READMEs
//! are stripped by default; license files are always kept.

/// Glob patterns excluded from every published artifact by default.
/// Matching is case-insensitive in the CLI.
pub const DEFAULT_EXCLUDES: &[&str] = &[
    ".git/**",
    ".hg/**",
    ".svn/**",
    "tests/**",
    "test/**",
    "spec/**",
    "src/test/**",
    "**/__tests__/**",
    "**/*.test.*",
    "**/*.spec.*",
    "**/*_test.go",
    "**/*_test.py",
    ".github/**",
    ".gitlab/**",
    ".gitlab-ci.yml",
    "bitbucket-pipelines.yml",
    ".circleci/**",
    ".travis.yml",
    "azure-pipelines.yml",
    "README*",
    "CHANGELOG*",
    ".zedignore",
    ".zed/**",
    ".zpkg.lock",
    "zed_modules/**",
    "node_modules/**",
    "**/node_modules/**",
    "target/**",
    "**/target/**",
    ".dart_tool/**",
    "**/.dart_tool/**",
    ".gradle/**",
    "**/.gradle/**",
    "build/**",
    "**/build/**",
    "__pycache__/**",
    "**/__pycache__/**",
    ".venv/**",
    "**/.venv/**",
    "_build/**",
    "**/_build/**",
    "deps/**",
    "**/deps/**",
];

/// Patterns that are always kept, even when an exclude matches them.
/// Shipping license texts with artifacts is non-negotiable.
pub const ALWAYS_INCLUDE: &[&str] = &["LICENSE*", "LICENCE*", "COPYING*", "NOTICE*", ".zpkg.toml"];

/// The effective exclusion list for a package: built-in defaults (minus
/// README patterns when `include_readme` is set), plus the manifest's own
/// `publish.exclude` globs. `.zedignore` lines are appended by the CLI on
/// top of this.
pub fn effective_excludes(extra: &[String], include_readme: bool) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for pattern in DEFAULT_EXCLUDES {
        if include_readme && pattern.starts_with("README") {
            continue;
        }
        out.push((*pattern).to_string());
    }
    out.extend(extra.iter().cloned());
    out
}
