from pathlib import Path


def add_native_dependency_initializer(path: Path) -> int:
    lines = path.read_text(encoding="utf-8").splitlines(keepends=True)
    output: list[str] = []
    inserted = 0
    for line in lines:
        if "nix_adapters: Vec::new()," in line:
            previous = next((item.strip() for item in reversed(output) if item.strip()), "")
            if not previous.startswith("native_dependencies:"):
                indent = line[: len(line) - len(line.lstrip())]
                output.append(f"{indent}native_dependencies: Vec::new(),\n")
                inserted += 1
        output.append(line)
    if inserted == 0:
        raise SystemExit(f"{path}: no Lockfile literals required the additive field")
    path.write_text("".join(output), encoding="utf-8")
    return inserted


def refresh_legacy_fixture(path: Path) -> None:
    source = path.read_text(encoding="utf-8")
    old = 'vcs_tag = "v1.0.0"\nsource = "file:///tmp/registry"\n'
    new = (
        'vcs_tag = "v1.0.0"\n'
        'vcs_commit = "0123456789abcdef0123456789abcdef01234567"\n'
        'source = "file:///tmp/registry"\n'
    )
    if source.count(old) != 1:
        raise SystemExit("legacy fixture revision marker drifted")
    path.write_text(source.replace(old, new, 1), encoding="utf-8")


internal = add_native_dependency_initializer(Path("src/lockfile.rs"))
external = add_native_dependency_initializer(Path("tests/lockfile_content_addressed_provenance.rs"))
refresh_legacy_fixture(Path("tests/native_dependency_lockfile_contract.rs"))
print(f"updated {internal} internal and {external} external Lockfile literals")
