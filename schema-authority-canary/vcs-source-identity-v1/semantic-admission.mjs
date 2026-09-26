import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(import.meta.url));

function readFixture(relativePath) {
  return JSON.parse(readFileSync(resolve(root, relativePath), "utf8"));
}

function isPortableRelativePath(value) {
  if (!value || value.startsWith("/") || value.startsWith("\\") || value.includes("\\")) {
    return false;
  }
  if (/^[A-Za-z]:/.test(value)) {
    return false;
  }
  const segments = value.split("/");
  return segments.every((segment) => segment.length > 0 && segment !== "." && segment !== "..");
}

function semanticErrors(document) {
  const errors = [];

  let url;
  try {
    url = new URL(document.repository_url);
  } catch {
    errors.push("invalid_repository_url");
  }
  if (url) {
    if (url.protocol !== "https:") {
      errors.push("repository_url_not_https");
    }
    if (url.username || url.password || url.search || url.hash || url.port) {
      errors.push("repository_url_not_portable_credential_free_identity");
    }
  }

  if (!isPortableRelativePath(document.destination_path)) {
    errors.push("destination_path_not_portable_relative");
  }

  const hasSelectorKind = document.selector_kind !== undefined;
  const hasSelector = document.selector !== undefined;
  if (hasSelectorKind !== hasSelector) {
    errors.push("selector_kind_value_pair_required");
  }

  const hasPackage = document.package !== undefined;
  const hasPackageVersion = document.package_version !== undefined;
  if (hasPackage !== hasPackageVersion) {
    errors.push("package_version_pair_required");
  }

  const relationByKind = {
    git: new Set(["direct", "git_submodule"]),
    mercurial: new Set(["direct", "hg_subrepo"]),
    jujutsu: new Set(["direct", "jj_colocated_git"]),
  };
  if (!relationByKind[document.vcs_kind]?.has(document.relation_kind)) {
    errors.push("backend_relation_mismatch");
  }

  const selectorByKind = {
    git: new Set(["commit", "tag", "branch"]),
    mercurial: new Set(["commit", "tag", "branch", "bookmark"]),
    jujutsu: new Set(["commit", "bookmark", "change_id"]),
  };
  if (hasSelectorKind && !selectorByKind[document.vcs_kind]?.has(document.selector_kind)) {
    errors.push("backend_selector_mismatch");
  }

  if ((document.vcs_kind === "git" || document.vcs_kind === "mercurial") &&
      !/^[a-f0-9]{40}$/.test(document.revision || "")) {
    errors.push("git_hg_revision_must_be_full_40_hex");
  }
  if (document.vcs_kind === "jujutsu" && !/^[a-f0-9]{40,64}$/.test(document.revision || "")) {
    errors.push("jujutsu_revision_must_be_40_to_64_hex");
  }

  if (document.selector_kind === "commit" && document.selector !== document.revision) {
    errors.push("commit_selector_must_equal_revision");
  }

  const nestedRelation = document.relation_kind === "git_submodule" || document.relation_kind === "hg_subrepo";
  if (nestedRelation && !document.parent_source_id) {
    errors.push("nested_source_requires_parent_source_id");
  }
  if (!nestedRelation && document.parent_source_id !== undefined) {
    errors.push("non_nested_source_forbids_parent_source_id");
  }
  if (document.parent_source_id && document.parent_source_id === document.source_id) {
    errors.push("source_cannot_parent_itself");
  }

  return errors;
}

for (const fixture of [
  "instances/VcsSourceIdentityV1/valid/git-submodule.json",
  "instances/VcsSourceIdentityV1/valid/mercurial-subrepo.json",
  "instances/VcsSourceIdentityV1/valid/jujutsu-colocated.json",
]) {
  assert.deepEqual(semanticErrors(readFixture(fixture)), [], `${fixture} must be admitted`);
}

assert.ok(
  semanticErrors(
    readFixture("instances/VcsSourceIdentityV1/invalid/backend-relation-mismatch.json"),
  ).includes("backend_relation_mismatch"),
);
assert.ok(
  semanticErrors(readFixture("instances/VcsSourceIdentityV1/invalid/path-traversal.json"))
    .includes("destination_path_not_portable_relative"),
);

const commitSelectorMismatch = readFixture(
  "instances/VcsSourceIdentityV1/valid/git-submodule.json",
);
commitSelectorMismatch.selector_kind = "commit";
commitSelectorMismatch.selector = "2222222222222222222222222222222222222222";
assert.ok(semanticErrors(commitSelectorMismatch).includes("commit_selector_must_equal_revision"));

const missingParent = readFixture(
  "instances/VcsSourceIdentityV1/valid/mercurial-subrepo.json",
);
delete missingParent.parent_source_id;
assert.ok(semanticErrors(missingParent).includes("nested_source_requires_parent_source_id"));

console.log("VcsSourceIdentityV1 semantic admission fixtures passed");
