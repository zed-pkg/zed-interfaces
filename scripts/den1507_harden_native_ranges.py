from pathlib import Path
import re

path = Path("src/native_dependency.rs")
source = path.read_text(encoding="utf-8")

old_constant = "const MAX_REQUIREMENT_LEN: usize = 512;"
new_constant = (
    "const MAX_REQUIREMENT_LEN: usize = 512;\n"
    "const NPM_MAX_SAFE_COMPONENT: u64 = 9_007_199_254_740_991;"
)
if old_constant not in source:
    raise SystemExit("requirement-length constant marker not found")
source = source.replace(old_constant, new_constant, 1)

replacements = {
    'let version = parse_strict_version("version", version)?;':
        'let version = parse_native_version(self.registry, "version", version)?;',
    'let version = parse_strict_version("candidate.version", &candidate.version)?;':
        'let version = parse_native_version(registry, "candidate.version", &candidate.version)?;',
    'let resolved = parse_strict_version("package.version", &self.package.version)?;':
        'let resolved = parse_native_version(\n'
        '            self.requirement.registry,\n'
        '            "package.version",\n'
        '            &self.package.version,\n'
        '        )?;',
}
for old, new in replacements.items():
    if old not in source:
        raise SystemExit(f"version parser marker not found: {old}")
    source = source.replace(old, new, 1)

range_block = r'''fn translate_npm_requirement(declared: &str) -> Result<VersionReq, NativeDependencyError> {
    if declared.contains(',') {
        return Err(unsupported(
            NativeRegistry::Npm,
            declared,
            "npm comparator intersections use whitespace, not Cargo commas, in strict v1",
        ));
    }
    let normalized = coalesce_npm_tokens(declared)?
        .iter()
        .map(|token| normalize_npm_token(token))
        .collect::<Result<Vec<_>, _>>()?
        .join(", ");
    parse_requirement(NativeRegistry::Npm, declared, &normalized)
}

fn coalesce_npm_tokens(declared: &str) -> Result<Vec<String>, NativeDependencyError> {
    let mut normalized = Vec::new();
    let mut words = declared.split_whitespace();
    while let Some(word) = words.next() {
        if is_operator_token(word) {
            let body = words.next().ok_or_else(|| {
                invalid_requirement(
                    NativeRegistry::Npm,
                    declared,
                    "missing version after comparator",
                )
            })?;
            if !split_operator(body).0.is_empty() {
                return Err(invalid_requirement(
                    NativeRegistry::Npm,
                    declared,
                    "multiple comparator operators may not be separated by whitespace",
                ));
            }
            normalized.push(format!("{word}{body}"));
        } else {
            normalized.push(word.to_string());
        }
    }
    Ok(normalized)
}

fn normalize_npm_token(token: &str) -> Result<String, NativeDependencyError> {
    let (operator, body) = split_operator(token);
    if body.is_empty() {
        return Err(invalid_requirement(
            NativeRegistry::Npm,
            token,
            "missing version after comparator",
        ));
    }
    let body = strip_numeric_v_prefix(body);
    let body = normalize_x_components(body);

    if operator.is_empty() || operator == "=" {
        return normalize_npm_bare(&body).ok_or_else(|| {
            invalid_requirement(
                NativeRegistry::Npm,
                token,
                "expected an exact version, partial version, or wildcard",
            )
        });
    }
    normalize_npm_comparator(operator, &body, token)
}

fn normalize_npm_bare(body: &str) -> Option<String> {
    if let Ok(version) = Version::parse(body) {
        if version.build != BuildMetadata::EMPTY || !npm_components_supported(&version) {
            return None;
        }
        return Some(format!("={version}"));
    }

    let partial = parse_npm_partial(body)?;
    match partial.components.as_slice() {
        [] if partial.wildcard => Some("*".to_string()),
        [major] => Some(format!("{major}.*")),
        [major, minor] => Some(format!("{major}.{minor}.*")),
        [major, minor, patch] if !partial.wildcard => {
            Some(format!("={major}.{minor}.{patch}"))
        }
        _ => None,
    }
}

fn normalize_npm_comparator(
    operator: &str,
    body: &str,
    declared: &str,
) -> Result<String, NativeDependencyError> {
    if let Ok(version) = Version::parse(body) {
        if version.build != BuildMetadata::EMPTY {
            return Err(NativeDependencyError::BuildMetadataNotAllowed {
                field: "declared".to_string(),
                version: declared.to_string(),
            });
        }
        if !npm_components_supported(&version) {
            return Err(invalid_requirement(
                NativeRegistry::Npm,
                declared,
                "npm numeric components must not exceed Number.MAX_SAFE_INTEGER",
            ));
        }
        return Ok(format!("{operator}{version}"));
    }

    let partial = parse_npm_partial(body).ok_or_else(|| {
        invalid_requirement(
            NativeRegistry::Npm,
            declared,
            "expected a strict or partial numeric version after comparator",
        )
    })?;
    if partial.components.is_empty() {
        return Err(unsupported(
            NativeRegistry::Npm,
            declared,
            "comparators against an unconstrained wildcard are outside strict v1",
        ));
    }

    match operator {
        "^" | "~" => Ok(format!("{operator}{}", partial.numeric_prefix())),
        ">=" => Ok(format!(">={}", partial.lower_bound())),
        "<" => Ok(format!("<{}", partial.lower_bound())),
        ">" => Ok(format!(">={}", partial.next_prefix(declared)?)),
        "<=" => Ok(format!("<{}", partial.next_prefix(declared)?)),
        _ => Err(invalid_requirement(
            NativeRegistry::Npm,
            declared,
            "unsupported comparator operator",
        )),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NpmPartialVersion {
    components: Vec<u64>,
    wildcard: bool,
}

impl NpmPartialVersion {
    fn numeric_prefix(&self) -> String {
        self.components
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(".")
    }

    fn lower_bound(&self) -> String {
        match self.components.as_slice() {
            [major] => format!("{major}.0.0"),
            [major, minor] => format!("{major}.{minor}.0"),
            [major, minor, patch] => format!("{major}.{minor}.{patch}"),
            _ => unreachable!("validated npm partial has one to three numeric components"),
        }
    }

    fn next_prefix(&self, declared: &str) -> Result<String, NativeDependencyError> {
        match self.components.as_slice() {
            [major] => Ok(format!("{}.0.0", increment_npm_component(*major, declared)?)),
            [major, minor] => Ok(format!(
                "{major}.{}.0",
                increment_npm_component(*minor, declared)?
            )),
            [major, minor, patch] if !self.wildcard => Ok(format!(
                "{major}.{minor}.{}",
                increment_npm_component(*patch, declared)?
            )),
            _ => Err(invalid_requirement(
                NativeRegistry::Npm,
                declared,
                "cannot advance this partial comparator without changing its meaning",
            )),
        }
    }
}

fn parse_npm_partial(body: &str) -> Option<NpmPartialVersion> {
    let parts: Vec<&str> = body.split('.').collect();
    if parts.is_empty() || parts.len() > 3 {
        return None;
    }

    let mut components = Vec::new();
    let mut wildcard = false;
    for part in parts {
        if part == "*" {
            wildcard = true;
            continue;
        }
        if wildcard
            || part.is_empty()
            || !part.bytes().all(|byte| byte.is_ascii_digit())
            || (part.len() > 1 && part.starts_with('0'))
        {
            return None;
        }
        let component: u64 = part.parse().ok()?;
        if component > NPM_MAX_SAFE_COMPONENT {
            return None;
        }
        components.push(component);
    }

    Some(NpmPartialVersion {
        components,
        wildcard,
    })
}

fn increment_npm_component(
    component: u64,
    declared: &str,
) -> Result<u64, NativeDependencyError> {
    let incremented = component.checked_add(1).ok_or_else(|| {
        invalid_requirement(
            NativeRegistry::Npm,
            declared,
            "partial comparator component overflows SemVer",
        )
    })?;
    if incremented > NPM_MAX_SAFE_COMPONENT {
        return Err(invalid_requirement(
            NativeRegistry::Npm,
            declared,
            "partial comparator increment exceeds Number.MAX_SAFE_INTEGER",
        ));
    }
    Ok(incremented)
}

fn translate_cargo_requirement(declared: &str) -> Result<VersionReq, NativeDependencyError> {
    if cargo_contains_x_wildcard(declared) {
        return Err(unsupported(
            NativeRegistry::Cargo,
            declared,
            "Cargo wildcards use `*`; npm-style `x` and `X` are rejected",
        ));
    }

    let normalized = declared
        .split(',')
        .map(str::trim)
        .map(|segment| normalize_cargo_segment(declared, segment))
        .collect::<Result<Vec<_>, _>>()?
        .join(", ");
    parse_requirement(NativeRegistry::Cargo, declared, &normalized)
}

fn normalize_cargo_segment(
    declared: &str,
    segment: &str,
) -> Result<String, NativeDependencyError> {
    if segment.is_empty() {
        return Err(invalid_requirement(
            NativeRegistry::Cargo,
            declared,
            "empty comparator in comma-separated requirement",
        ));
    }
    let words: Vec<&str> = segment.split_whitespace().collect();
    match words.as_slice() {
        [single] => Ok((*single).to_string()),
        [operator, body] if is_operator_token(operator) && split_operator(body).0.is_empty() => {
            Ok(format!("{operator}{body}"))
        }
        _ => Err(unsupported(
            NativeRegistry::Cargo,
            declared,
            "multiple Cargo comparators require commas; whitespace is only allowed between one operator and its version",
        )),
    }
}

fn is_operator_token(token: &str) -> bool {
    matches!(token, ">=" | "<=" | "^" | "~" | ">" | "<" | "=")
}

fn split_operator(token: &str) -> (&str, &str) {
    for operator in [">=", "<=", "^", "~", ">", "<", "="] {
        if let Some(body) = token.strip_prefix(operator) {
            return (operator, body);
        }
    }
    ("", token)
}

fn strip_numeric_v_prefix(body: &str) -> &str {
    body.strip_prefix('v')
        .filter(|rest| {
            rest.bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_digit())
        })
        .unwrap_or(body)
}

fn normalize_x_components(body: &str) -> String {
    body.split('.')
        .map(|part| {
            if part.eq_ignore_ascii_case("x") {
                "*"
            } else {
                part
            }
        })
        .collect::<Vec<_>>()
        .join(".")
}

fn cargo_contains_x_wildcard(declared: &str) -> bool {
    declared
        .split(|character: char| character.is_whitespace() || character == ',')
        .filter(|token| !token.is_empty())
        .any(|token| {
            let (_, body) = split_operator(token);
            let core = body.split('-').next().unwrap_or(body);
            core.split('.').any(|part| part.eq_ignore_ascii_case("x"))
        })
}

fn parse_requirement'''

pattern = r"fn translate_npm_requirement\(declared: &str\).*?\nfn parse_requirement"
source, count = re.subn(pattern, range_block, source, flags=re.S)
if count != 1:
    raise SystemExit(f"expected one range parser block, found {count}")

old_parse = '''fn parse_strict_version(field: &str, raw: &str) -> Result<Version, NativeDependencyError> {
    let version = Version::parse(raw).map_err(|error| NativeDependencyError::InvalidVersion {
        field: field.to_string(),
        version: raw.to_string(),
        detail: error.to_string(),
    })?;
    if version.build != BuildMetadata::EMPTY {
        return Err(NativeDependencyError::BuildMetadataNotAllowed {
            field: field.to_string(),
            version: raw.to_string(),
        });
    }
    Ok(version)
}
'''
new_parse = '''fn parse_native_version(
    registry: NativeRegistry,
    field: &str,
    raw: &str,
) -> Result<Version, NativeDependencyError> {
    let version = parse_strict_version(field, raw)?;
    if registry == NativeRegistry::Npm && !npm_components_supported(&version) {
        return Err(NativeDependencyError::InvalidVersion {
            field: field.to_string(),
            version: raw.to_string(),
            detail: "npm numeric components must not exceed Number.MAX_SAFE_INTEGER".to_string(),
        });
    }
    Ok(version)
}

fn npm_components_supported(version: &Version) -> bool {
    version.major <= NPM_MAX_SAFE_COMPONENT
        && version.minor <= NPM_MAX_SAFE_COMPONENT
        && version.patch <= NPM_MAX_SAFE_COMPONENT
}

fn parse_strict_version(field: &str, raw: &str) -> Result<Version, NativeDependencyError> {
    let version = Version::parse(raw).map_err(|error| NativeDependencyError::InvalidVersion {
        field: field.to_string(),
        version: raw.to_string(),
        detail: error.to_string(),
    })?;
    if version.build != BuildMetadata::EMPTY {
        return Err(NativeDependencyError::BuildMetadataNotAllowed {
            field: field.to_string(),
            version: raw.to_string(),
        });
    }
    Ok(version)
}
'''
if old_parse not in source:
    raise SystemExit("strict version parser block not found")
source = source.replace(old_parse, new_parse, 1)

test_marker = '''    #[test]
    fn resolution_is_highest_satisfying_and_order_independent() {'''
new_tests = '''    #[test]
    fn npm_partial_comparators_follow_node_semver_boundaries() {
        let gt_major = NativeVersionRequirement::parse(NativeRegistry::Npm, ">1").unwrap();
        let gt_minor = NativeVersionRequirement::parse(NativeRegistry::Npm, ">1.2").unwrap();
        let lte_minor = NativeVersionRequirement::parse(NativeRegistry::Npm, "<=1.2").unwrap();
        let spaced = NativeVersionRequirement::parse(
            NativeRegistry::Npm,
            ">= 1.2.3 < 2.0.0",
        )
        .unwrap();

        assert_eq!(gt_major.canonical, ">=2.0.0");
        assert_eq!(gt_minor.canonical, ">=1.3.0");
        assert_eq!(lte_minor.canonical, "<1.3.0");
        assert_eq!(spaced.canonical, ">=1.2.3, <2.0.0");
        assert!(!gt_major.matches("1.99.99").unwrap());
        assert!(gt_major.matches("2.0.0").unwrap());
        assert!(!gt_minor.matches("1.2.999").unwrap());
        assert!(gt_minor.matches("1.3.0").unwrap());
        assert!(lte_minor.matches("1.2.999").unwrap());
        assert!(!lte_minor.matches("1.3.0").unwrap());
    }

    #[test]
    fn cargo_allows_operator_whitespace_but_still_requires_comma_intersections() {
        let requirement = NativeVersionRequirement::parse(
            NativeRegistry::Cargo,
            ">= 1.2, < 1.5",
        )
        .unwrap();
        assert_eq!(requirement.canonical, ">=1.2, <1.5");
        assert!(requirement.matches("1.4.99").unwrap());
        assert!(!requirement.matches("1.5.0").unwrap());
        assert!(NativeVersionRequirement::parse(
            NativeRegistry::Cargo,
            ">= 1.2 < 1.5",
        )
        .is_err());
    }

    #[test]
    fn npm_rejects_leading_zero_and_unsafe_integer_components() {
        for requirement in [
            "01.2",
            "1.02",
            ">1.02",
            "^01.2",
            "9007199254740992.0.0",
            ">9007199254740991",
        ] {
            assert!(NativeVersionRequirement::parse(NativeRegistry::Npm, requirement).is_err());
        }

        assert!(matches!(
            NativeDependencyLock::resolve(
                NativeRegistry::Npm,
                "@fiducia/core",
                "*",
                &[candidate("9007199254740992.0.0", 'a')],
            ),
            Err(NativeDependencyError::InvalidVersion { .. })
        ));
    }

''' + test_marker
if test_marker not in source:
    raise SystemExit("unit test insertion marker not found")
source = source.replace(test_marker, new_tests, 1)

path.write_text(source, encoding="utf-8")
