# Claritas Viz dependency-graph boundary

Tracking: DEN-3477; Claritas umbrella DEN-617.

Zed dependency graph visualization uses `claritas-viz`. This contract lives inside the existing `validation/` peer-authority area and does not replace manifest, lockfile, resolver, registry, or persistence contracts.

The topology is supplied by Zed dependency resolution. Claritas provides stable display layout only; it must not add/remove dependency edges, resolve packages, change lockfiles, or infer installability.

TypeSpec and Draft 2020-12 JSON Schema are independent peer authorities. TJSV-generated schema is comparison evidence only.
