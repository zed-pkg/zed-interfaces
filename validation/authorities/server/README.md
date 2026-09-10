# Server persistence authorities

`contracts.json` and `contracts.tsp` are independent, peer-authored descriptions of private server persistence shapes. Neither file is generated from the other.

`ORESoftware/typespec-json-schema-validator` (TJSV) is the fail-closed cross-authority admission gate for this boundary. The pinned workflow compares the independently authored TypeSpec and Draft 2020-12 JSON Schema, synthesizes differential probes, emits a digest-bound Contract IR, and requires zero unexplained findings before downstream persistence work may treat the pair as converged. Generated JSON Schema and Contract IR are comparison evidence only; neither becomes a third authority or replaces either authored source.

The existing structural comparator must also agree on model names, requiredness, scalar kinds, defaults, formats, bounds, and patterns. The ORM annotation comparator separately requires JSON Schema `x-orm` metadata to match the machine-readable `// @orm-schema` and `// @orm` comments in TypeSpec. TJSV admission is additive to those checks, not a replacement for ORM metadata verification.

These files do not expose a database client and do not authorize browser or edge exports. They feed the private `zed-pkg/zed-orm-core` Diesel code-first and SeaORM database-first witnesses only after the TJSV, structural, and ORM-annotation receipts are green.
