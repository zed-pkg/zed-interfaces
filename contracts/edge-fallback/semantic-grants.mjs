/**
 * Semantic admission rule shared by edge fallback capability v1/v2/v3.
 *
 * TypeSpec and JSON Schema independently enforce the shape of each grant.
 * JSON Schema's uniqueItems cannot enforce uniqueness by authorization tuple
 * when different credential_ref or origin fields are present.
 *
 * The edge provider planner matches grants by (provider, package, resource).
 * Keep this identity exactly aligned with the planner: the Cargo origin is a
 * constraint of an admitted tuple, not a second selection key.
 */
export class EdgeGrantIdentityError extends Error {
  constructor(code, message) {
    super(message);
    this.name = "EdgeGrantIdentityError";
    this.code = code;
  }
}

/**
 * @param {{grants?: unknown}} capability
 */
export function assertUniqueEdgeFallbackGrants(capability) {
  if (!capability || !Array.isArray(capability.grants)
      || capability.grants.length < 1 || capability.grants.length > 16) {
    throw new EdgeGrantIdentityError("invalid_grants", "expected 1..16 grants");
  }
  const seen = new Set();
  for (const grant of capability.grants) {
    if (!grant || typeof grant !== "object"
        || typeof grant.provider !== "string" || !grant.provider
        || typeof grant.package !== "string" || !grant.package
        || typeof grant.resource !== "string" || !grant.resource) {
      throw new EdgeGrantIdentityError("invalid_grant", "grant identity is missing");
    }
    const key = JSON.stringify([grant.provider, grant.package, grant.resource]);
    if (seen.has(key)) {
      throw new EdgeGrantIdentityError(
        "duplicate_grant",
        "same provider/package/resource appears more than once in the capability",
      );
    }
    seen.add(key);
  }
}
