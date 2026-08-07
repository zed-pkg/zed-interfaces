from pathlib import Path


def add_native_dependency_initializer(path: Path) -> int:
    lines = path.read_text(encoding="utf-8").splitlines(keepends=True)
    output: list[str] = []
    inserted = 0

    for line in lines:
        if "nix_adapters: Vec::new()," in line:
            previous = next(
                (candidate.strip() for candidate in reversed(output) if candidate.strip()),
                "",
            )
            if not previous.startswith("native_dependencies:"):
                indentation = line[: len(line) - len(line.lstrip())]
                output.append(f"{indentation}native_dependencies: Vec::new(),\n")
                inserted += 1
        output.append(line)

    if inserted == 0:
        raise SystemExit(f"{path}: no remaining Lockfile literals required the new field")

    path.write_text("".join(output), encoding="utf-8")
    return inserted


def update_legacy_fixture_revision(path: Path) -> None:
    source = path.read_text(encoding="utf-8")
    old = '''vcs_tag = "v1.0.0"
source = "file:///tmp/registry"
'''
    new = '''vcs_tag = "v1.0.0"
vcs_commit = "0123456789abcdef0123456789abcdef01234567"
source = "file:///tmp/registry"
'''
    count = source.count(old)
    if count != 1:
        raise SystemExit(
            f"{path}: expected one legacy immutable-revision fixture, found {count}"
        )
    path.write_text(source.replace(old, new, 1), encoding="utf-8")


internal = add_native_dependency_initializer(Path("src/lockfile.rs"))
compatibility = add_native_dependency_initializer(
    Path("tests/lockfile_content_addressed_provenance.rs")
)
update_legacy_fixture_revision(Path("tests/native_dependency_lockfile_contract.rs"))
print(
    f"inserted native dependency defaults into {internal} internal and "
    f"{compatibility} compatibility Lockfile literals; refreshed the legacy "
    "fixture with current immutable-revision metadata"
)
