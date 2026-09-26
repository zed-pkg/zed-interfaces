use zed_interfaces::manifest::Manifest;

#[allow(clippy::needless_return)]
fn manifest_with_platform_commands() -> &'static str {
    return r#"
[package]
org = "acme"
name = "portable-cli"
version = "1.2.3"
language = "rust"

[package.repository]
vcs = "git"
url = "https://github.com/acme/portable-cli"

[build]
command = "cargo build --release"
command_windows = "cargo build --release --target x86_64-pc-windows-msvc"
outputs = ["target/release/portable-cli"]
outputs_windows = ["target/release/portable-cli.exe"]

[bin]
portable-cli = "target/release/portable-cli"

[publish]
smoke_test = "./target/release/portable-cli --help"
smoke_test_windows = ".\\target\\release\\portable-cli.exe --help"
"#;
}

#[test]
fn windows_platform_commands_parse_and_roundtrip() {
    let manifest = Manifest::parse(manifest_with_platform_commands()).expect("manifest must parse");
    let build = manifest.build.as_ref().expect("build section");

    assert_eq!(build.command_for_host(false), "cargo build --release");
    assert_eq!(
        build.command_for_host(true),
        "cargo build --release --target x86_64-pc-windows-msvc"
    );
    assert_eq!(
        build.outputs_for_host(false),
        ["target/release/portable-cli"]
    );
    assert_eq!(
        build.outputs_for_host(true),
        ["target/release/portable-cli.exe"]
    );
    assert_eq!(
        manifest.publish.smoke_test_for_host(false),
        Some("./target/release/portable-cli --help")
    );
    assert_eq!(
        manifest.publish.smoke_test_for_host(true),
        Some(".\\target\\release\\portable-cli.exe --help")
    );

    let encoded = manifest.to_toml_string().expect("manifest must serialize");
    let reparsed = Manifest::parse(&encoded).expect("roundtrip manifest must parse");
    assert_eq!(reparsed, manifest);
}

#[test]
fn legacy_manifest_falls_back_to_default_commands_on_windows() {
    let source = manifest_with_platform_commands()
        .replace(
            "command_windows = \"cargo build --release --target x86_64-pc-windows-msvc\"\n",
            "",
        )
        .replace(
            "outputs_windows = [\"target/release/portable-cli.exe\"]\n",
            "",
        )
        .replace(
            "smoke_test_windows = \".\\\\target\\\\release\\\\portable-cli.exe --help\"\n",
            "",
        );
    let manifest = Manifest::parse(&source).expect("legacy manifest must remain valid");
    let build = manifest.build.as_ref().expect("build section");

    assert_eq!(build.command_for_host(true), "cargo build --release");
    assert_eq!(
        build.outputs_for_host(true),
        ["target/release/portable-cli"]
    );
    assert_eq!(
        manifest.publish.smoke_test_for_host(true),
        Some("./target/release/portable-cli --help")
    );
}

#[test]
fn empty_windows_build_command_is_rejected() {
    let source = manifest_with_platform_commands().replace(
        "command_windows = \"cargo build --release --target x86_64-pc-windows-msvc\"",
        "command_windows = \"   \"",
    );
    let error = Manifest::parse(&source).expect_err("blank windows build command must fail");
    assert!(
        error
            .to_string()
            .contains("command_windows must not be empty")
    );
}

#[test]
fn unsafe_windows_build_output_is_rejected() {
    let source = manifest_with_platform_commands().replace(
        "outputs_windows = [\"target/release/portable-cli.exe\"]",
        "outputs_windows = [\"../portable-cli.exe\"]",
    );
    let error = Manifest::parse(&source).expect_err("unsafe windows output must fail");
    assert!(error.to_string().contains("output `../portable-cli.exe`"));
}

#[test]
fn empty_windows_smoke_test_is_rejected() {
    let source = manifest_with_platform_commands().replace(
        "smoke_test_windows = \".\\\\target\\\\release\\\\portable-cli.exe --help\"",
        "smoke_test_windows = \"\"",
    );
    let error = Manifest::parse(&source).expect_err("blank windows smoke test must fail");
    assert!(
        error
            .to_string()
            .contains("smoke_test_windows must not be empty")
    );
}
