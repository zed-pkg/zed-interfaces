# Clean polyglot baseline provenance

This layout is reconstructed from canonical `main`, not merged from the historical `baseline/polyglot-layout` branch.

Reviewed semantic sources preserved:

- `762071627a6a8ddfc3391cb7ca8ecf7277f21faa` — isolated Rust, Dart and TypeScript slices;
- `0639a55609eb0927c2bd3a9c7301c2f3dbd601e3` — layout documentation and generated-slice CI;
- `6ad1ad3c0584a60778bf0d451d9d091ad90e6c0c` — generator ownership, wire-format and runtime hardening.

The successor starts from `f793e7c592373242dff9ab855845c42f526db9b4`, so later canonical work is preserved, including dependency-graph v1 (#52) and canonical whole-repository identity (#53). Derived schemas, fixtures, Dart and TypeScript are regenerated from that current Rust source rather than copied as authority.

Historical PR #51 remains preserved as source evidence but is superseded by this clean reconstruction because it carries unrelated autosave ancestry and an obsolete conflicting root-target name.
