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

#[derive(Debug, Default)]
struct EvaluatedSource {
    excludes: Vec<String>,
    negated_families: BTreeSet<String>,
}

/// The effective exclusion list for one authored source.
///
/// This preserves the existing ordered-negation contract within that source:
/// `!target` can remove the built-in `target/**` family or an earlier explicit
/// `target/**`, and a later positive rule can add it again.
pub fn effective_excludes(extra: &[String], include_readme: bool) -> Vec<String> {
    effective_excludes_union(&[extra], include_readme)
}

/// The effective exclusion list for independent authored sources such as
/// `[publish].exclude`, `.zedignore`, and CLI-owned safety rules.
///
/// Each source evaluates its own ordered `!` rules first. The resulting
/// positive sets are then unioned, so a positive exclusion that survives in
/// either source cannot be undone by a negation in another source. Negations
/// from any source still re-include matching built-in defaults when no source
/// explicitly excludes that family. This makes source order irrelevant while
/// retaining useful source-local negation semantics.
pub fn effective_excludes_union(
    sources: &[&[String]],
    include_readme: bool,
) -> Vec<String> {
    let evaluated = sources
        .iter()
        .map(|rules| evaluate_source(rules))
        .collect::<Vec<_>>();

    let mut negated_defaults = BTreeSet::new();
    for source in &evaluated {
        negated_defaults.extend(source.negated_families.iter().cloned());
    }

    let mut out = DEFAULT_EXCLUDES
        .iter()
        .filter(|pattern| {
            !(include_readme
                && REGISTRY_DOC_PATTERNS
                    .iter()
                    .any(|doc| pattern.starts_with(doc)))
        })
        .filter(|pattern| {
            !negated_defaults.contains(&normalize_pattern(pattern, true))
        })
        .map(|pattern| (*pattern).to_string())
        .collect::<Vec<_>>();

    for source in evaluated {
        out.extend(source.excludes);
    }
    out
}

fn evaluate_source(rules: &[String]) -> EvaluatedSource {
    let mut evaluated = EvaluatedSource::default();
    for raw in rules {
        let pattern = raw.trim();
        if pattern.is_empty() {
            continue;
        }
        if let Some(negated) = pattern.strip_prefix('!') {
            let normalized = normalize_pattern(negated, false);
            if normalized.is_empty() {
                continue;
            }
            evaluated
                .excludes
                .retain(|existing| normalize_pattern(existing, true) != normalized);
            evaluated.negated_families.insert(normalized);
        } else {
            let normalized = normalize_pattern(pattern, true);
            if !normalized.is_empty() {
                evaluated.negated_families.remove(&normalized);
            }
            evaluated.excludes.push(pattern.to_string());
        }
    }
    evaluated
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

    fn contains(excludes: &[String], pattern: &str) -> bool {
        excludes.iter().any(|candidate| candidate == pattern)
    }

    #[test]
    fn target_negation_removes_root_and_recursive_defaults() {
        for negation in ["!target", "!target/", "!target/**"] {
            let excludes = effective_excludes(&[negation.to_string()], false);
            assert!(!contains(&excludes, "target/**"));
            assert!(!contains(&excludes, "**/target/**"));
            assert!(!excludes.iter().any(|pattern| pattern.starts_with('!')));
        }
    }

    #[test]
    fn negation_only_removes_the_matching_default_family() {
        let excludes = effective_excludes(&["!target".to_string()], false);
        assert!(contains(&excludes, "node_modules/**"));
        assert!(contains(&excludes, "build/**"));
    }

    #[test]
    fn later_negation_removes_an_earlier_rule_in_the_same_source() {
        let excludes = effective_excludes(
            &["private/**".to_string(), "!private".to_string()],
            false,
        );
        assert!(!contains(&excludes, "private/**"));
    }

    #[test]
    fn later_exclusion_can_reapply_after_negation() {
        let excludes = effective_excludes(
            &["!target".to_string(), "target/private/**".to_string()],
            false,
        );
        assert!(contains(&excludes, "target/private/**"));
        assert!(!contains(&excludes, "target/**"));
    }

    #[test]
    fn cross_source_exclusion_wins_over_negation_regardless_of_source_order() {
        let excludes = vec!["private/**".to_string()];
        let reincludes = vec!["!private".to_string()];
        for sources in [
            [&excludes[..], &reincludes[..]],
            [&reincludes[..], &excludes[..]],
        ] {
            let effective = effective_excludes_union(&sources, false);
            assert!(contains(&effective, "private/**"));
            assert!(!effective.iter().any(|pattern| pattern.starts_with('!')));
        }
    }

    #[test]
    fn union_retains_source_local_negation_and_other_source_rules() {
        let manifest = vec!["generated/**".to_string(), "!generated".to_string()];
        let ignore = vec!["private/**".to_string()];
        let effective = effective_excludes_union(&[&manifest, &ignore], false);
        assert!(!contains(&effective, "generated/**"));
        assert!(contains(&effective, "private/**"));
    }

    #[test]
    fn protected_source_cannot_be_negated_by_an_authored_source() {
        let authored = vec!["!zed_modules".to_string()];
        let protected = vec!["zed_modules/**".to_string()];
        let effective = effective_excludes_union(&[&authored, &protected], false);
        assert!(contains(&effective, "zed_modules/**"));
    }
}
