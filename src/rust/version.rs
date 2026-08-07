//! Polyglot version handling (zed-docs issue #3).
//!
//! zed-pkg standardizes on **semver** as its resolution algebra, but the
//! ecosystems it federates label releases inconsistently: Go tags `v1.2.3`
//! (and `+incompatible`), Python uses PEP 440 (`1.2.3rc1`, `.postN`), and many
//! tools use calendar versions (`2026.07.24`). A package declares its
//! [`VersionScheme`] so `package.version` is validated the right way, and the
//! resolver normalizes foreign tag spellings to a total order at the edges.
//!
//! The scheme is a property of the *published package*, not of the consumer's
//! requirement — a dependency on an opaque-versioned package is written as an
//! exact string, everything else as a semver range.

use schemars::JsonSchema;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

/// How a package's `version` string (and its published tags) should be
/// interpreted. Defaults to [`VersionScheme::Semver`], which covers the vast
/// majority of modern ecosystems.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum VersionScheme {
    /// Semantic Versioning. `package.version` must be valid semver; ranges
    /// (`^1.2`, `>=0.2 <0.5`) resolve to the max satisfying stable version.
    #[default]
    Semver,
    /// Calendar Versioning (`2026.07.24`, `2026.07`). Normalized to a semver
    /// total order (leading zeros dropped, padded to major.minor.patch) so the
    /// same range algebra applies. See [`normalize_calver`].
    Calver,
    /// Arbitrary tags (`release-candidate-1`, `legacy-api`). No range algebra:
    /// a requirement must match a published version **exactly**.
    Opaque,
}

impl VersionScheme {
    pub fn as_str(&self) -> &'static str {
        match self {
            VersionScheme::Semver => "semver",
            VersionScheme::Calver => "calver",
            VersionScheme::Opaque => "opaque",
        }
    }

    /// True for the default scheme (semver); lets manifests omit the field.
    pub fn is_default(&self) -> bool {
        matches!(self, VersionScheme::Semver)
    }

    /// Parse a scheme name, defaulting to semver for empty/unknown input so a
    /// stored value can never wedge a read.
    pub fn from_str_lenient(s: &str) -> VersionScheme {
        match s {
            "calver" => VersionScheme::Calver,
            "opaque" => VersionScheme::Opaque,
            _ => VersionScheme::Semver,
        }
    }

    /// Validate that `version` is a legal identity under this scheme.
    pub fn validate_version(&self, version: &str) -> Result<(), String> {
        match self {
            VersionScheme::Semver => Version::parse(version)
                .map(|_| ())
                .map_err(|e| e.to_string()),
            VersionScheme::Calver => normalize_calver(version)
                .map(|_| ())
                .ok_or_else(|| format!("`{version}` is not a calendar version (e.g. 2026.07.24)")),
            VersionScheme::Opaque => {
                if version.trim().is_empty() {
                    Err("opaque version must be non-empty".to_string())
                } else {
                    Ok(())
                }
            }
        }
    }
}

/// Normalize a calendar version to a semver string with a total order.
///
/// `2026.07.24` → `2026.7.24`, `2026.07` → `2026.7.0`, `2026` → `2026.0.0`.
/// Leading zeros are stripped (semver forbids them); 1–3 dot-separated numeric
/// segments are accepted and padded to `major.minor.patch`. An optional
/// `-prerelease` suffix is preserved. Returns `None` if the shape isn't
/// calendar-like.
pub fn normalize_calver(raw: &str) -> Option<String> {
    let raw = raw.strip_prefix('v').unwrap_or(raw);
    let (core, pre) = match raw.split_once('-') {
        Some((core, pre)) if !pre.is_empty() => (core, Some(pre)),
        _ => (raw, None),
    };
    let mut parts: Vec<u64> = Vec::new();
    for seg in core.split('.') {
        if seg.is_empty() || !seg.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        parts.push(seg.parse().ok()?);
    }
    if parts.is_empty() || parts.len() > 3 {
        return None;
    }
    while parts.len() < 3 {
        parts.push(0);
    }
    let core = format!("{}.{}.{}", parts[0], parts[1], parts[2]);
    match pre {
        Some(pre) => Some(format!("{core}-{pre}")),
        None => Some(core),
    }
}

/// Drop Go's `+incompatible` (and any other build metadata) so a bare Go tag
/// like `v2.0.0+incompatible` parses. Build metadata is ignored by semver
/// precedence anyway, but we strip it for a clean identity.
fn normalize_go(raw: &str) -> Option<String> {
    let raw = raw.strip_prefix('v').unwrap_or(raw);
    let core = raw.split('+').next().unwrap_or(raw);
    Version::parse(core).ok().map(|v| v.to_string())
}

/// Map a PEP 440 pre/dev/post spelling onto a semver prerelease so ordering
/// stays sane: `1.2.3rc1` → `1.2.3-rc.1`, `1.2a1` → `1.2.0-a.1`,
/// `1.2.3.dev4` → `1.2.3-dev.4`. Post-releases (`1.2.3.post1`) become build
/// metadata (`1.2.3+post.1`) since semver has no "after the release" ordering.
/// Conservative by design — only the common shapes; returns `None` otherwise.
fn normalize_pep440(raw: &str) -> Option<String> {
    let raw = raw.strip_prefix('v').unwrap_or(raw).to_ascii_lowercase();
    // Split the numeric release from the first non-digit/non-dot marker.
    let marker = raw.find(|c: char| c.is_ascii_alphabetic())?;
    let (release, suffix) = raw.split_at(marker);
    let release = release.trim_end_matches('.');
    let mut nums: Vec<u64> = Vec::new();
    for seg in release.split('.') {
        if seg.is_empty() {
            continue;
        }
        nums.push(seg.parse().ok()?);
    }
    while nums.len() < 3 {
        nums.push(0);
    }
    if nums.len() > 3 {
        return None;
    }
    let core = format!("{}.{}.{}", nums[0], nums[1], nums[2]);

    // suffix is e.g. "rc1", "a1", "b2", "dev4", "post1" (dots already gone).
    let suffix = suffix.trim_start_matches('.');
    let split = suffix
        .find(|c: char| c.is_ascii_digit())
        .unwrap_or(suffix.len());
    let (label, num) = suffix.split_at(split);
    let num = if num.is_empty() { "0" } else { num };
    if num.bytes().any(|b| !b.is_ascii_digit()) {
        return None;
    }
    let (label, kind) = match label {
        "a" | "alpha" => ("alpha", '-'),
        "b" | "beta" => ("beta", '-'),
        "c" | "rc" | "pre" | "preview" => ("rc", '-'),
        "dev" => ("dev", '-'),
        "post" | "rev" | "r" => ("post", '+'),
        _ => return None,
    };
    Some(format!("{core}{kind}{label}.{num}"))
}

/// Parse a version string to a comparable semver [`Version`], tolerating the
/// common foreign spellings (bare `v`, calendar versions, Go `+incompatible`,
/// a subset of PEP 440). Build metadata is dropped so `2.0.0+incompatible` and
/// `2.0.0` share one identity (semver precedence ignores build metadata).
/// Returns `None` for genuinely opaque strings.
pub fn parse_version(raw: &str) -> Option<Version> {
    let mut version = parse_version_inner(raw)?;
    version.build = semver::BuildMetadata::EMPTY;
    Some(version)
}

fn parse_version_inner(raw: &str) -> Option<Version> {
    if let Ok(v) = Version::parse(raw) {
        return Some(v);
    }
    if let Some(stripped) = raw.strip_prefix('v')
        && let Ok(v) = Version::parse(stripped)
    {
        return Some(v);
    }
    if let Some(go) = normalize_go(raw)
        && let Ok(v) = Version::parse(&go)
    {
        return Some(v);
    }
    if let Some(cal) = normalize_calver(raw)
        && let Ok(v) = Version::parse(&cal)
    {
        return Some(v);
    }
    if let Some(pep) = normalize_pep440(raw)
        && let Ok(v) = Version::parse(&pep)
    {
        return Some(v);
    }
    None
}

/// A resolved dependency requirement. Semver and calendar requirements are
/// ranges; opaque requirements are exact strings.
#[derive(Debug, Clone, PartialEq)]
pub enum Requirement {
    Range(VersionReq),
    Exact(String),
}

impl Requirement {
    /// Interpret a manifest requirement string. A valid semver range becomes
    /// [`Requirement::Range`]; anything else is an exact (opaque) match.
    ///
    /// npm-style whitespace-AND ranges (`>=1.2 <2`) are accepted by
    /// normalizing the whitespace between comparators to the comma the semver
    /// crate expects — but only as a fallback, so an opaque tag containing a
    /// space is still treated as exact when the normalized form is not a valid
    /// range.
    pub fn parse(input: &str) -> Requirement {
        if let Ok(req) = VersionReq::parse(input) {
            return Requirement::Range(req);
        }
        if input.split_whitespace().count() > 1 {
            let normalized = input.split_whitespace().collect::<Vec<_>>().join(",");
            if let Ok(req) = VersionReq::parse(&normalized) {
                return Requirement::Range(req);
            }
        }
        Requirement::Exact(input.to_string())
    }

    /// Does a published version string satisfy this requirement?
    pub fn matches(&self, version: &str) -> bool {
        match self {
            Requirement::Range(req) => parse_version(version).is_some_and(|v| req.matches(&v)),
            Requirement::Exact(want) => want == version,
        }
    }

    /// Reject requirement strings that *look* like semver ranges but fail to
    /// parse (e.g. `^1.x.y`, `>= banana`). Without this, a typo'd range would
    /// silently degrade to an opaque exact-match tag and never resolve.
    /// Strings with no range operators are legitimate opaque tags and pass.
    pub fn validate(input: &str) -> Result<(), String> {
        let looks_like_range = input.starts_with(['^', '~', '>', '<', '='])
            || input.contains(['*', ','])
            || input.split_whitespace().count() > 1;
        if !looks_like_range {
            return Ok(());
        }
        match Self::parse(input) {
            Requirement::Range(_) => Ok(()),
            Requirement::Exact(_) => Err(format!(
                "`{input}` looks like a version range but is not a valid one"
            )),
        }
    }
}

/// Pick the version string satisfying `req` from `versions`.
///
/// For a range: the highest stable (non-prerelease) parseable version that
/// matches, returned in its **original** spelling so the store address and tag
/// stay faithful to what the publisher tagged. For an exact requirement: the
/// verbatim match if present.
pub fn resolve<'a>(req: &Requirement, versions: &'a [String]) -> Option<&'a str> {
    match req {
        Requirement::Exact(want) => versions
            .iter()
            .find(|v| v.as_str() == want)
            .map(String::as_str),
        Requirement::Range(range) => versions
            .iter()
            .filter_map(|v| parse_version(v).map(|parsed| (parsed, v)))
            .filter(|(parsed, _)| parsed.pre.is_empty() && range.matches(parsed))
            .max_by(|a, b| a.0.cmp(&b.0))
            .map(|(_, original)| original.as_str()),
    }
}

/// Sort version strings newest-first, tolerant of foreign spellings.
/// Unparseable (opaque) versions sort after parseable ones, lexically.
pub fn sort_desc(versions: &mut [String]) {
    versions.sort_by(|a, b| match (parse_version(a), parse_version(b)) {
        (Some(va), Some(vb)) => vb.cmp(&va),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => b.cmp(a),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calver_normalizes_to_semver_order() {
        assert_eq!(normalize_calver("2026.07.24").as_deref(), Some("2026.7.24"));
        assert_eq!(normalize_calver("2026.07").as_deref(), Some("2026.7.0"));
        assert_eq!(normalize_calver("2026").as_deref(), Some("2026.0.0"));
        assert_eq!(normalize_calver("v2026.01.02").as_deref(), Some("2026.1.2"));
        assert_eq!(normalize_calver("not-a-date"), None);
        assert_eq!(normalize_calver("1.2.3.4"), None);

        // Total order holds across months and days.
        let older = parse_version("2026.07.24").unwrap();
        let newer = parse_version("2026.08.01").unwrap();
        assert!(newer > older);
    }

    #[test]
    fn parse_version_tolerates_foreign_spellings() {
        assert_eq!(parse_version("1.2.3").unwrap().to_string(), "1.2.3");
        assert_eq!(parse_version("v1.2.3").unwrap().to_string(), "1.2.3");
        // Go +incompatible is stripped to a clean identity.
        assert_eq!(
            parse_version("v2.0.0+incompatible").unwrap().to_string(),
            "2.0.0"
        );
        // PEP 440 pre-releases map onto semver prereleases (order preserved).
        assert_eq!(parse_version("1.2.3rc1").unwrap().to_string(), "1.2.3-rc.1");
        assert_eq!(parse_version("1.2a1").unwrap().to_string(), "1.2.0-alpha.1");
        assert!(parse_version("1.2.3rc1").unwrap() < parse_version("1.2.3").unwrap());
        assert_eq!(parse_version("legacy-api"), None);
    }

    #[test]
    fn resolve_range_picks_max_satisfying_stable() {
        let versions: Vec<String> = ["1.0.0", "1.2.3", "1.3.0-alpha.1", "2.0.0"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let req = Requirement::parse("^1.2");
        assert_eq!(resolve(&req, &versions), Some("1.2.3"));
        assert_eq!(resolve(&Requirement::parse("^3"), &versions), None);
    }

    #[test]
    fn resolve_calver_range() {
        let versions: Vec<String> = ["2025.12.01", "2026.07.24", "2026.08.01"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        // A semver range over calendar versions: pick the newest in 2026.
        let req = Requirement::parse(">=2026.0.0, <2027.0.0");
        assert!(matches!(req, Requirement::Range(_)));
        assert_eq!(resolve(&req, &versions), Some("2026.08.01"));
    }

    #[test]
    fn opaque_requires_exact_match() {
        let versions: Vec<String> = ["release-candidate-1", "legacy-api", "beta"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let req = Requirement::parse("legacy-api");
        assert!(matches!(req, Requirement::Exact(_)));
        assert_eq!(resolve(&req, &versions), Some("legacy-api"));
        assert_eq!(resolve(&Requirement::parse("nope"), &versions), None);
    }

    #[test]
    fn scheme_validation() {
        assert!(VersionScheme::Semver.validate_version("1.2.3").is_ok());
        assert!(
            VersionScheme::Semver
                .validate_version("2026.07.24")
                .is_err()
        );
        assert!(VersionScheme::Calver.validate_version("2026.07.24").is_ok());
        assert!(VersionScheme::Opaque.validate_version("legacy-api").is_ok());
        assert!(VersionScheme::Opaque.validate_version("  ").is_err());
    }
}
