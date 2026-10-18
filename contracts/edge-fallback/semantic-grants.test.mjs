import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import {
  EdgeGrantIdentityError,
  assertUniqueEdgeFallbackGrants,
} from "./semantic-grants.mjs";

const fixture = (name) => JSON.parse(
  readFileSync(new URL(`./fixtures/${name}.json`, import.meta.url), "utf8"),
);
const v1 = fixture("duplicate-grants-v1");
const v3 = fixture("duplicate-grants-v3");
const v2 = {
  ...v1,
  zed_edge_capability: 2,
  sid: "session:abc-123",
  parent_jti: "parent-token-0001",
};

function expectCode(document, code) {
  assert.throws(
    () => assertUniqueEdgeFallbackGrants(document),
    (error) => error instanceof EdgeGrantIdentityError && error.code === code,
  );
}

test("rejects duplicate authorization keys with different credential refs in v1/v2/v3", () => {
  for (const document of [v1, v2, {
    ...v3, grants: v1.grants,
  }]) {
    expectCode(document, "duplicate_grant");
    expectCode({ ...document, grants: [...document.grants].reverse() }, "duplicate_grant");
    expectCode({ ...document, grants: [document.grants[0], { ...document.grants[0] }] }, "duplicate_grant");
  }
});

test("rejects same Cargo crate tuple with distinct registry origins", () => {
  expectCode(v3, "duplicate_grant");
  expectCode({ ...v3, grants: [...v3.grants].reverse() }, "duplicate_grant");
});

test("permits multiple distinct provider/package/resource tuples", () => {
  const distinct = {
    ...v1,
    grants: [
      v1.grants[0],
      { ...v1.grants[1], resource: "acme/other-private-lib" },
    ],
  };
  assert.doesNotThrow(() => assertUniqueEdgeFallbackGrants(distinct));
});

test("rejects missing and malformed grant identities", () => {
  expectCode({ grants: [] }, "invalid_grants");
  expectCode({ grants: [{}] }, "invalid_grant");
  expectCode({ grants: [{ provider: "github", package: "acme/p" }] }, "invalid_grant");
});
