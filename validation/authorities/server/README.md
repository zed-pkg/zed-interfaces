# Server persistence authorities

`contracts.json` and `contracts.tsp` are independent, peer-authored descriptions of private server persistence shapes. Neither file is generated from the other.

The shared structural comparator must agree on model names, requiredness, scalar kinds, defaults, formats, bounds, and patterns. The ORM annotation comparator separately requires JSON Schema `x-orm` metadata to match the machine-readable `// @orm-schema` and `// @orm` comments in TypeSpec.

These files do not expose a database client and do not authorize browser or edge exports. They feed the private `zed-pkg/zed-orm-core` Diesel code-first and SeaORM database-first witnesses only after both receipts are green.
