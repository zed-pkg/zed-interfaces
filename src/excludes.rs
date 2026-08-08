//! Publish-time exclusion rules.
//!
//! zed-pkg's core disk-space promise: published artifacts carry runtime files,
//! not development machinery. Tests, CI configuration, VCS metadata, and
//! release documentation are stripped by default; packages can opt into their
//! README and changelog together, while license files are always kept.

use std::collections::BTreeSet;

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
/// registry-facing doc patterns when `include_readme` is set), plus every
/// explicit exclusion supplied by the caller. The CLI supplies both
/// `[publish].exclude` and `.zedignore` rules here, so explicit exclusions are
/// combined as a union regardless of which source owns them.
///
/// A leading `!` re-includes a matching built-in default. It never cancels an
/// explicit exclusion: when one authored source excludes a path and another
/// re-includes it, exclusion wins. This makes the result independent of whether
/// the manifest or `.zedignore` was appended first and prevents a user-authored
/// negation from disabling CLI safety exclusions such as dependency or
/// transaction directories.
pub fn effective_excludes(extra: &[String], include_readme: bool) -> Vec<String> {
    let mut defaults: Vec<String> = Vec::new();
    for pattern in DEFAULT_EXCLUDES {
        if include_readme && REGISTRY_DOC_PATTERNS.iter().any(|d| pattern.starts_with(d)) {
            continue;
        }
        defaults.push((*pattern).to_string());
    }

    let mut negated_defaults = BTreeSet::new();
    let mut explicit = Vec::new();
    for pattern in extra {
        if let Some(negated) = pattern.strip_prefix('!') {
            let normalized = normalize_pattern(negated, false);
            if !normalized.is_empty() {
                negated_defaults.insert(normalized);
            }
        } else {
            explicit.push(pattern.clone());
        }
    }

    defaults.retain(|existing| {
        !negated_defaults.contains(&normalize_pattern(existing, true))
    });
    defaults.extend(explicit);
    defaults
}

fn normalize_pattern(pattern: &str, strip_recursive_prefix: bool) -> String {
    let mut value = pattern.trim().replace('\\', "/");
    if strip_recursive_prefix {
        value = value.strip_prefix("**/").unwrap_or(&value).to_string();
    }
    while let Some(stripped) = value.strip_suffix("/**") {
        value = stripped.to_string();
    }
    value.trim_matches('/').to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_negation_removes_root_and_recursive_defaults() {
        for negation in ["!target", "!target/", "!target/**"] {
            let excludes = effective_excludes(&[negation.to_string()], false);
            assert!(!excludes.iter().any(|pattern| pattern == "target/**"));
            assert!(!excludes.iter().any(|pattern| pattern == "**/target/**"));
            assert!(!excludes.iter().any(|pattern| pattern.starts_with('!')));
        }
    }

    #[test]
    fn negation_only_removes_the_matching_default_family() {
        let excludes = effective_excludes(&["!target".to_string()], false);
        assert!(excludes.iter().any(|pattern| pattern == "node_modules/**"));
        assert!(excludes.iter().any(|pattern| pattern == "build/**"));
    }

    #[test]
    fn explicit_exclusion_wins_over_negation_in_either_order() {
        for extra in [
            vec!["private/**".to_string(), "!private".to_string()],
            vec!["!private".to_string(), "private/**".to_string()],
        ] {
            let excludes = effective_excludes(&extra, false);
            assert!(excludes.iter().any(|pattern| pattern == "private/**"));
            assert!(!excludes.iter().any(|pattern| pattern.starts_with('!')));
        }
    }

    #[test]
    fn later_exclusion_can_narrow_a_reincluded_default() {
        let excludes = effective_excludes(
            &["!target".to_string(), "target/private/**".to_string()],
            false,
        );
        assert!(
            excludes
                .iter()
                .any(|pattern| pattern == "target/private/**")
        );
        assert!(!excludes.iter().any(|pattern| pattern == "target/**"));
    }
}
