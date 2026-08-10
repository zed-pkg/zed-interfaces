// Checks the generated TypeScript types against the fixtures in ../../fixtures
// — real serde output from the Rust types.
//
//   node --test wire-format.test.ts
//
// TypeScript interfaces are erased at runtime, so a plain `as PackageMetadata`
// proves nothing on its own. Two things here are worth checking anyway, and
// both are real failure modes:
//
//   * the *compiler* sees each fixture assigned to its interface, so a missing
//     or misspelled required field fails `tsc` (that is the type-level half);
//   * the emitted `*_VALUES` arrays are runtime data, so every enum spelling
//     serde produces can be checked against them here (the runtime half).
//
// Regenerate the fixtures with:
//   cargo run --locked --example generate_fixtures

import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { test } from "node:test";

import type {
  ApiError,
  AuditIntegrityResponse,
  AuditLogResponse,
  ClaimOrgRequest,
  ClaimOrgResponse,
  PackageListResponse,
  PackageMetadata,
  PublishResponse,
  SearchResponse,
  SemanticSearchRequest,
  SemanticSearchResponse,
  SyncChangeEvent,
  VersionMetadata,
  YankRequest,
  YankResponse,
} from "./index.ts";
import {
  ARTIFACT_FORMAT_VALUES,
  AUDIT_ACTION_VALUES,
  SYNC_CONFLICT_RESOLUTION_VALUES,
  SYNC_ERROR_POLICY_VALUES,
  SYNC_OP_VALUES,
  SYNC_WRITE_MODE_VALUES,
  VCS_VALUES,
  VERSION_SCHEME_VALUES,
} from "./index.ts";

const fixture = <T>(name: string): T =>
  JSON.parse(
    fs.readFileSync(path.join(import.meta.dirname, "../../fixtures", `${name}.json`), "utf8"),
  ) as T;

test("PackageMetadata: every optional field present", () => {
  const { full } = fixture<{ full: PackageMetadata }>("package-metadata");
  assert.equal(full.org, "acme");
  assert.equal(full.vcs, "jj");
  assert.equal(full.version_scheme, "calver");
  assert.deepEqual(full.tags, ["http", "client"]);
});

test("PackageMetadata: serde omitted every optional key", () => {
  // `skip_serializing_if` means these keys are *absent*, not null. The
  // interface marks them optional, which is what makes this assignment legal.
  const { minimal } = fixture<{ minimal: PackageMetadata }>("package-metadata");
  assert.equal("version_scheme" in minimal, false);
  assert.equal("description" in minimal, false);
  assert.equal(minimal.version_scheme ?? "semver", "semver");
});

test("VersionMetadata: numbers and defaulted fields", () => {
  const { published } = fixture<{ published: VersionMetadata }>("version-metadata");
  assert.equal(published.size, 4096);
  assert.equal(published.format ?? "tar.gz", "tar.gz");
  assert.equal(published.yanked ?? false, false);
});

test("responses that share PackageSummary decode consistently", () => {
  const { page } = fixture<{ page: PackageListResponse }>("package-list-response");
  const { hit } = fixture<{ hit: SearchResponse }>("search-response");
  assert.equal(page.total, 1);
  assert.equal(page.items[0]?.name, "http-kit");
  assert.equal(hit.query, "http");
  assert.equal(hit.items[0]?.org, "acme");
});

test("AuditLogResponse: nested entries with an optional enum", () => {
  const { chain } = fixture<{ chain: AuditLogResponse }>("audit-log-response");
  const entry = chain.entries[0];
  assert.ok(entry);
  assert.equal(entry.action_kind, "publish");
  assert.equal("prev_hash" in entry, false, "the first entry has no predecessor");
  assert.equal(entry.seq, 1);
});

test("SyncChangeEvent: opaque row and nested Hlc", () => {
  const { upsert } = fixture<{ upsert: SyncChangeEvent }>("sync-change-event");
  assert.equal(upsert.op, "upsert");
  assert.equal(upsert.version.counter, 3);
  assert.deepEqual(upsert.row, { org: "acme" });
  assert.ok(SYNC_OP_VALUES.includes(upsert.op));
});

test("every enum spelling serde emits is a member of the generated union", () => {
  // The union type is erased at runtime; the VALUES array is not. If a Rust
  // rename drifted from the generated spelling, this is where it surfaces —
  // a front end would otherwise narrow against a value that cannot occur.
  const enums = fixture<Record<string, string>>("enums");
  const pairs: ReadonlyArray<readonly [string, readonly string[]]> = [
    ["write_mode", SYNC_WRITE_MODE_VALUES],
    ["error_policy", SYNC_ERROR_POLICY_VALUES],
    ["conflict_resolution", SYNC_CONFLICT_RESOLUTION_VALUES],
    ["artifact_format", ARTIFACT_FORMAT_VALUES],
    ["vcs", VCS_VALUES],
    ["version_scheme", VERSION_SCHEME_VALUES],
    ["audit_action", AUDIT_ACTION_VALUES],
  ];
  for (const [key, values] of pairs) {
    const emitted = enums[key];
    assert.ok(emitted, `fixture is missing ${key}`);
    assert.ok(values.includes(emitted), `${key}="${emitted}" is not in [${values.join(", ")}]`);
  }
});

test("the remaining request and response bodies match their interfaces", () => {
  const misc = fixture<{
    publish: PublishResponse;
    claim_org_request: ClaimOrgRequest;
    claim_org_response: ClaimOrgResponse;
    yank_request: YankRequest;
    yank_response: YankResponse;
    audit_integrity: AuditIntegrityResponse;
    semantic_search_request: SemanticSearchRequest;
    semantic_search_response: SemanticSearchResponse;
  }>("misc-responses");

  assert.equal(misc.publish.version, "1.0.0");
  assert.equal(misc.claim_org_request.slug, "acme");
  assert.equal(misc.claim_org_response.created, true);
  assert.equal(misc.yank_request.yanked, true);
  assert.equal(misc.yank_response.name, "http-kit");
  assert.equal(misc.audit_integrity.intact, true);
  assert.equal(misc.audit_integrity.entries_checked, 1);
  assert.equal(misc.semantic_search_request.limit, 5);
  assert.equal(misc.semantic_search_request.embedding.length, 3);
  assert.equal(misc.semantic_search_response.items[0]?.distance, 0.25);
});

test("api-error", () => {
  const { not_found } = fixture<{ not_found: ApiError }>("api-error");
  assert.equal(not_found.code, "not_found");
});
