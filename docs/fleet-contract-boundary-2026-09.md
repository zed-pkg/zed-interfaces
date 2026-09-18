# Fleet contract dependency boundary

`zed-interfaces` owns zed-pkg domain contracts for package coordinates, resolution, lock identity, publishing, and related interoperability. It does not become the owner of application/domain contracts merely because zed-pkg distributes them.

## Fleet dependency rules

- A `.zpkg.toml` requirement is dependency intent; the resolved `.zpkg.lock` is execution truth and must retain exact version plus immutable artifact/VCS provenance.
- Fleet-generic application semantic primitives belong in `oresoftware/ores-interfaces`; product-specific contracts stay with their owning org.
- Lock/lease/fencing semantics belong in `oresoftware/ores-locks-and-leases`, not zed-pkg.
- Kubernetes and deployment topology belongs in `oresoftware/k8s-libs-and-shared-defs`.
- Independently authored TypeSpec and JSON Schema Draft 2020-12 pairs are admitted with `oresoftware/typespec-json-schema-validator` without regenerating one authority from the other to force parity.
- `oresoftware/ores-cli` may consume the zed resolved graph for repo/org/fleet policy and cross-repo validation; it must not implement a divergent package solver.
- Blocking cross-repo compatibility checks use the exact resolved artifact. Default-branch/tip checks are separately labeled forward-compatibility canaries.

## Anti-cycle rule

Zed contracts and resolution metadata may be consumed by governance tooling and product repositories, but zed-pkg packages must not depend on product runtime implementations merely to validate or resolve them.