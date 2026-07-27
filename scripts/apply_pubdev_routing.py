#!/usr/bin/env python3
"""Extend native release routing with pub.dev."""

from pathlib import Path


MANIFEST = Path("src/manifest.rs")
text = MANIFEST.read_text(encoding="utf-8")


def replace_once(old: str, new: str, label: str) -> None:
    global text
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one insertion point, found {count}")
    text = text.replace(old, new, 1)


replace_once(
    '''pub enum NativeRegistry {
    Npm,
    CratesIo,
}''',
    '''pub enum NativeRegistry {
    Npm,
    CratesIo,
    #[serde(rename = "pub.dev")]
    PubDev,
}''',
    "NativeRegistry enum",
)

replace_once(
    '''        match self {
            Self::Npm => "npm",
            Self::CratesIo => "crates-io",
        }''',
    '''        match self {
            Self::Npm => "npm",
            Self::CratesIo => "crates-io",
            Self::PubDev => "pub.dev",
        }''',
    "NativeRegistry::as_str",
)

replace_once(
    '''        let valid = match self {
            Self::Npm => is_valid_npm_package(package),
            Self::CratesIo => is_valid_crates_package(package),
        };''',
    '''        let valid = match self {
            Self::Npm => is_valid_npm_package(package),
            Self::CratesIo => is_valid_crates_package(package),
            Self::PubDev => is_valid_pubdev_package(package),
        };''',
    "native package validation dispatch",
)

replace_once(
    '''fn is_valid_crates_package(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && !value.starts_with('-')
        && !value.starts_with('_')
        && !value.ends_with('-')
        && !value.ends_with('_')
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

/// A post-extract build step.''',
    '''fn is_valid_crates_package(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && !value.starts_with('-')
        && !value.starts_with('_')
        && !value.ends_with('-')
        && !value.ends_with('_')
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

fn is_valid_pubdev_package(value: &str) -> bool {
    const RESERVED: &[&str] = &[
        "assert", "break", "case", "catch", "class", "const", "continue", "default",
        "do", "else", "enum", "extends", "false", "final", "finally", "for", "if",
        "in", "is", "new", "null", "rethrow", "return", "super", "switch", "this",
        "throw", "true", "try", "var", "void", "while", "with", "async", "await",
        "yield",
    ];
    !value.is_empty()
        && !value.as_bytes()[0].is_ascii_digit()
        && !RESERVED.contains(&value)
        && value
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// A post-extract build step.''',
    "pub.dev package validator",
)

MANIFEST.write_text(text, encoding="utf-8")

TEST = Path("tests/native_release.rs")
tests = TEST.read_text(encoding="utf-8")

old_routes = '''[targets.nodejs.native]
registry = "npm"
package = "@acme/client"
"#,'''
new_routes = '''[targets.nodejs.native]
registry = "npm"
package = "@acme/client"

[targets.dart]
dir = "clients/dart"

[targets.dart.native]
registry = "pub.dev"
package = "acme_client"
"#,'''
if tests.count(old_routes) != 1:
    raise SystemExit("route fixture insertion point changed")
tests = tests.replace(old_routes, new_routes, 1)

tests = tests.replace(
    '''    assert_eq!(routes.len(), 2);
    assert_eq!(routes[0].target, "nodejs");''',
    '''    assert_eq!(routes.len(), 3);
    assert_eq!(routes[0].target, "dart");
    assert_eq!(routes[0].registry, NativeRegistry::PubDev);
    assert_eq!(routes[0].package, "acme_client");
    assert_eq!(routes[1].target, "nodejs");''',
    1,
)
tests = tests.replace(
    '''    assert_eq!(routes[0].registry, NativeRegistry::Npm);
    assert_eq!(routes[0].package, "@acme/client");
    assert_eq!(routes[1].target, "rust");
    assert_eq!(routes[1].registry, NativeRegistry::CratesIo);''',
    '''    assert_eq!(routes[1].registry, NativeRegistry::Npm);
    assert_eq!(routes[1].package, "@acme/client");
    assert_eq!(routes[2].target, "rust");
    assert_eq!(routes[2].registry, NativeRegistry::CratesIo);''',
    1,
)
tests = tests.replace(
    '''    assert!(encoded.contains("registry = \\"npm\\""));
    Manifest::parse(&encoded).unwrap();''',
    '''    assert!(encoded.contains("registry = \\"npm\\""));
    assert!(encoded.contains("registry = \\"pub.dev\\""));
    Manifest::parse(&encoded).unwrap();''',
    1,
)

invalid_anchor = '''        r#"
[targets.rust]
dir = "clients/rust"
[targets.rust.native]
registry = "crates-io"
package = "acme/client"
"#,
    ] {'''
invalid_replacement = '''        r#"
[targets.rust]
dir = "clients/rust"
[targets.rust.native]
registry = "crates-io"
package = "acme/client"
"#,
        r#"
[targets.dart]
dir = "clients/dart"
[targets.dart.native]
registry = "pub.dev"
package = "Bad-Dart-Package"
"#,
        r#"
[targets.dart]
dir = "clients/dart"
[targets.dart.native]
registry = "pub.dev"
package = "123_client"
"#,
        r#"
[targets.dart]
dir = "clients/dart"
[targets.dart.native]
registry = "pub.dev"
package = "class"
"#,
    ] {'''
if tests.count(invalid_anchor) != 1:
    raise SystemExit("invalid-route fixture insertion point changed")
tests = tests.replace(invalid_anchor, invalid_replacement, 1)

TEST.write_text(tests, encoding="utf-8")
print("added pub.dev native routing")
