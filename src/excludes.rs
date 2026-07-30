//! Publish-time exclusion rules.
//!
//! zed-pkg's core disk-space promise: published artifacts carry runtime files,
//! not development machinery. Tests, CI configuration, VCS metadata, and
//! release documentation are stripped by default; packages can opt into their
//! README and changelog together, while license files are always kept.

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
    // Python tests come in both spellings; `test_*.py` is what unittest
    // discovers by default, so omitting it leaks tests into every published
    // Python slice.
    "**/*_test.py",
    "**/test_*.py",
    "**/conftest.py",
    // Ruby tests living outside `spec/` (minitest's `*_test.rb`, and RSpec
    // files kept beside their subject).
    "**/*_test.rb",
    "**/*_spec.rb",
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
    ".zed-pack/**",
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

/// Doc patterns `include_readme` un-excludes: the human-facing files a package
/// registry expects to find in a published artifact.
///
/// `CHANGELOG` is here rather than stripped because native registries ask for it
/// by name — `dart pub publish` fails the package outright for its absence, and
/// `publish.exclude` can only *add* patterns, so a repo has no way to keep it
/// otherwise. A package that opted into shipping its README wants its changelog
/// shipped too.
const REGISTRY_DOC_PATTERNS: &[&str] = &["README", "CHANGELOG"];

/// The effective exclusion list for a package: built-in defaults (minus the
/// registry-facing doc patterns when `include_readme` is set), plus the
/// manifest's own `publish.exclude` globs. `.zedignore` lines are appended by
/// the CLI on top of this.
pub fn effective_excludes(extra: &[String], include_readme: bool) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for pattern in DEFAULT_EXCLUDES {
        if include_readme && REGISTRY_DOC_PATTERNS.iter().any(|d| pattern.starts_with(d)) {
            continue;
        }
        out.push((*pattern).to_string());
    }
    out.extend(extra.iter().cloned());
    out
}
