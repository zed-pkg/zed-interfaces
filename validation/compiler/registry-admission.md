# Private registry admission audit — DEN-3908

This draft adds two independent checks without editing either peer authority or
any generated runtime artifact. The canonical validator compiles the actual
private TypeSpec source and compares it with the independently authored JSON
Schema. A separate metadata-only PostgreSQL check materializes the pinned
`zed-lib-core` registry DDL and compares explicit schema/table/column names with
JSON Schema `x-orm` declarations. It does not guess aliases or naming conversions.

The existing private models describe `zed_pkg.packages`, `package_versions`, and
`registry_leases`, while the active owned schema uses `public.zed_*`. The inventory
also requires users, organizations, projects, packages, versions, licenses,
embeddings, uploads, downloads, and CLI metadata. Therefore admission is expected
to stop until actual coverage and mappings are reconciled. Do not suppress these
findings or rewrite the deployed schema merely to make this workflow green.

The metadata gate's explicit scope is `schema-table-column-inventory-only`.
Even a pass does not certify SQL types, defaults, nullability, keys, constraints,
RLS/grants, function security, SeaORM or Diesel runtime behavior, or safe migration.
Those require separate catalog parity and real two-ORM transaction tests before
SQL generation or deployment can be admitted. No SQL is generated or deployed by
this workflow, and no application rows, credentials, or sessions are read.

Local unit checks:

```sh
node --test validation/compiler/registry-sql-admission.test.mjs
```

CI retains compiler reports and the disposable database catalog with SHA-256
input digests. A failed check is a finding, not a successful certification. Keep
this PR draft while the authorities or compiler admission remain unresolved.
The independently releasable promotion-policy security patch lives in
`zed-pkg/zed-lib-core` PR #42 and does not depend on these new model projections.
