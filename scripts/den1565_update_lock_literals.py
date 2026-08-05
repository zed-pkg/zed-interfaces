from pathlib import Path

lockfile = Path("src/lockfile.rs")
source = lockfile.read_text(encoding="utf-8")
needle = "            packages: vec![package_without_commit(digest)],\n            nix_adapters: Vec::new(),"
replacement = "            packages: vec![package_without_commit(digest)],\n            native_dependencies: Vec::new(),\n            nix_adapters: Vec::new(),"
count = source.count(needle)
if count < 1:
    raise SystemExit("internal Lockfile literals: no remaining matches found")
lockfile.write_text(source.replace(needle, replacement), encoding="utf-8")

compatibility = Path("tests/lockfile_content_addressed_provenance.rs")
source = compatibility.read_text(encoding="utf-8")
needle = "        nix_adapters: Vec::new(),"
replacement = "        native_dependencies: Vec::new(),\n        nix_adapters: Vec::new(),"
count = source.count(needle)
if count != 3:
    raise SystemExit(f"content-addressed Lockfile literals: expected 3 matches, found {count}")
compatibility.write_text(source.replace(needle, replacement), encoding="utf-8")
