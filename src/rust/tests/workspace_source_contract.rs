use zed_interfaces::manifest::{Manifest, WorkspaceSourceMode};

fn manifest(extra: &str) -> String {
    format!(
        r#"[package]
org = "acme"
name = "app"
version = "1.0.0"

[package.repository]
vcs = "git"
url = "https://github.com/acme/app"

{extra}
"#
    )
}

#[test]
fn path_overrides_accept_env_templates_without_shell_execution() {
    let parsed = Manifest::parse(&manifest(
        r#"[dependencies]
"acme/core" = "^1"

[overrides.path]
"acme/core" = "${HOME}/codes/acme/core"
"#,
    ))
    .expect("portable env interpolation is valid");
    assert_eq!(
        parsed.overrides.path["acme/core"],
        "${HOME}/codes/acme/core"
    );

    for hostile in ["$(touch /tmp/pwned)", "`touch /tmp/pwned`", "$", "${BAD-NAME}/x"] {
        let raw = manifest(&format!(
            "[dependencies]\n\"acme/core\" = \"^1\"\n\n[overrides.path]\n\"acme/core\" = {hostile:?}\n"
        ));
        let error = Manifest::parse(&raw).expect_err("hostile override must fail");
        assert!(
            error.to_string().contains("local path override"),
            "unexpected error for {hostile:?}: {error}"
        );
    }
}

#[test]
fn workspace_sources_derive_disjoint_mode_specific_paths() {
    let parsed = Manifest::parse(&manifest(
        r#"[install]
dir = ".zed/pkg"

[workspace]
checkout_dir = ".zed/vcs"
git_submodule_dir = "submodules"

[workspace.sources.api]
vcs = "git"
url = "https://github.com/acme/api"
package = "acme/api"
mode = "git-submodule"

[workspace.sources.docs]
vcs = "hg"
url = "https://example.invalid/hg/docs"
package = "acme/docs"
mode = "checkout"
"#,
    ))
    .expect("disjoint source composition is valid");

    let workspace = parsed.workspace.as_ref().expect("workspace");
    assert_eq!(
        workspace.source_path("api", &workspace.sources["api"]),
        "submodules/api"
    );
    assert_eq!(
        workspace.source_path("docs", &workspace.sources["docs"]),
        ".zed/vcs/docs"
    );
    assert_eq!(
        workspace.sources["api"].mode,
        WorkspaceSourceMode::GitSubmodule
    );
}

#[test]
fn workspace_sources_reject_install_tree_overlap() {
    let error = Manifest::parse(&manifest(
        r#"[install]
dir = ".zed/pkg"

[workspace]
checkout_dir = ".zed/pkg/vcs"

[workspace.sources.api]
vcs = "git"
url = "https://github.com/acme/api"
package = "acme/api"
"#,
    ))
    .expect_err("source root inside package tree must fail");
    assert!(error.to_string().contains("must not overlap"));
}

#[test]
fn mercurial_cannot_be_projected_as_a_git_submodule() {
    let error = Manifest::parse(&manifest(
        r#"[workspace]

[workspace.sources.docs]
vcs = "hg"
url = "https://example.invalid/hg/docs"
package = "acme/docs"
mode = "git-submodule"
"#,
    ))
    .expect_err("hg is not a Git submodule transport");
    assert!(error.to_string().contains("Git-compatible VCS"));
}

#[test]
fn workspace_role_requires_a_package_identity() {
    let error = Manifest::parse(&manifest(
        r#"[workspace]

[workspace.sources.api]
vcs = "git"
url = "https://github.com/acme/api"
"#,
    ))
    .expect_err("workspace source without package identity must fail");
    assert!(error.to_string().contains("must declare the Zed package identity"));
}
