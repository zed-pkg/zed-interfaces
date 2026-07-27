#!/usr/bin/env python3
"""Extend typed native release routing with PyPI semantics."""

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
    '''    #[serde(rename = "pub.dev")]
    PubDev,
}''',
    '''    #[serde(rename = "pub.dev")]
    PubDev,
    PyPi,
}''',
    "NativeRegistry::PyPi",
)
replace_once(
    '''            Self::PubDev => "pub.dev",
        }''',
    '''            Self::PubDev => "pub.dev",
            Self::PyPi => "pypi",
        }''',
    "PyPI display name",
)
replace_once(
    '''            Self::PubDev => is_valid_pubdev_package(package),
        };''',
    '''            Self::PubDev => is_valid_pubdev_package(package),
            Self::PyPi => is_valid_pypi_package(package),
        };''',
    "PyPI syntax validation",
)
replace_once(
    '''    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NativeReleaseRoute {''',
    '''    }

    fn canonical_package(self, package: &str) -> String {
        match self {
            Self::PyPi => normalize_pypi_package(package),
            _ => package.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NativeReleaseRoute {''',
    "registry canonical package method",
)
replace_once(
    '''fn is_valid_pubdev_package(value: &str) -> bool {''',
    '''fn is_valid_pypi_package(value: &str) -> bool {
    !value.is_empty()
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value.as_bytes()[value.len() - 1].is_ascii_alphanumeric()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn normalize_pypi_package(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let mut separator = false;
    for byte in value.bytes() {
        if matches!(byte, b'.' | b'_' | b'-') {
            separator = true;
            continue;
        }
        if separator && !normalized.is_empty() {
            normalized.push('-');
        }
        separator = false;
        normalized.push((byte as char).to_ascii_lowercase());
    }
    normalized
}

fn is_valid_pubdev_package(value: &str) -> bool {''',
    "PyPI helpers",
)
replace_once(
    '''                let route = (native.registry, native.package.clone());
                if let Some(previous) = native_routes.insert(route, name.as_str()) {''',
    '''                let canonical_package = native.registry.canonical_package(&native.package);
                let route = (native.registry, canonical_package);
                if let Some(previous) = native_routes.insert(route, name.as_str()) {''',
    "normalized native route collision key",
)
MANIFEST.write_text(text, encoding="utf-8")

TEST = Path("tests/native_release.rs")
tests = TEST.read_text(encoding="utf-8")

anchor = '''[targets.dart.native]
registry = "pub.dev"
package = "acme_client"
"#,'''
replacement = '''[targets.dart.native]
registry = "pub.dev"
package = "acme_client"

[targets.python]
dir = "clients/python"

[targets.python.native]
registry = "py-pi"
package = "Acme.Client"
"#,'''
# Serde rename_all kebab-case would make PyPi -> py-pi, but desired wire spelling is pypi.
# Patch source enum with an explicit rename after initial insertion.
text = MANIFEST.read_text(encoding="utf-8")
text = text.replace('    PyPi,\n}', '    #[serde(rename = "pypi")]\n    PyPi,\n}', 1)
MANIFEST.write_text(text, encoding="utf-8")
replacement = replacement.replace('registry = "py-pi"', 'registry = "pypi"')
if tests.count(anchor) != 1:
    raise SystemExit("route fixture insertion point changed")
tests = tests.replace(anchor, replacement, 1)
tests = tests.replace('assert_eq!(routes.len(), 3);', 'assert_eq!(routes.len(), 4);', 1)
tests = tests.replace(
    '''    assert_eq!(routes[2].target, "rust");
    assert_eq!(routes[2].registry, NativeRegistry::CratesIo);''',
    '''    assert_eq!(routes[2].target, "python");
    assert_eq!(routes[2].registry, NativeRegistry::PyPi);
    assert_eq!(routes[2].package, "Acme.Client");
    assert_eq!(routes[3].target, "rust");
    assert_eq!(routes[3].registry, NativeRegistry::CratesIo);''',
    1,
)
tests = tests.replace(
    '''    assert!(encoded.contains("registry = \"pub.dev\""));''',
    '''    assert!(encoded.contains("registry = \"pub.dev\""));
    assert!(encoded.contains("registry = \"pypi\""));''',
    1,
)
invalid_anchor = '''        r#"
[targets.dart]
dir = "clients/dart"
[targets.dart.native]
registry = "pub.dev"
package = "class"
"#,
    ] {'''
invalid_replacement = '''        r#"
[targets.dart]
dir = "clients/dart"
[targets.dart.native]
registry = "pub.dev"
package = "class"
"#,
        r#"
[targets.python]
dir = "clients/python"
[targets.python.native]
registry = "pypi"
package = "-bad-name"
"#,
        r#"
[targets.python]
dir = "clients/python"
[targets.python.native]
registry = "pypi"
package = "bad name"
"#,
    ] {'''
if tests.count(invalid_anchor) != 1:
    raise SystemExit("invalid package fixture insertion point changed")
tests = tests.replace(invalid_anchor, invalid_replacement, 1)

append = r'''

#[test]
fn pypi_duplicate_destinations_use_normalized_names() {
    let error = Manifest::parse(&manifest(
        r#"
[targets.python]
dir = "clients/python"
[targets.python.native]
registry = "pypi"
package = "Friendly_Bard"

[targets.python-async]
dir = "clients/python-async"
[targets.python-async.native]
registry = "pypi"
package = "friendly...bard"
"#,
    ))
    .unwrap_err();
    let message = error.to_string();
    assert!(message.contains("already routed"), "{message}");
}
'''
tests += append
TEST.write_text(tests, encoding="utf-8")
print("added PyPI routing and normalized collision checks")
