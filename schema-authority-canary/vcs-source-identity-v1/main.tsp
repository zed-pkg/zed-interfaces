// Independently authored TypeSpec authority for portable VCS source identity.
// Generated JSON Schema is comparison evidence only; authored.schema.json is a peer authority.
import "@typespec/json-schema";
using TypeSpec.JsonSchema;

namespace ZedVcsSourceIdentity;

enum VcsKindV1 {
  git,
  mercurial,
  jujutsu,
}

enum VcsSelectorKindV1 {
  commit,
  tag,
  branch,
  bookmark,
  changeId: "change_id",
}

enum VcsRelationKindV1 {
  direct,
  gitSubmodule: "git_submodule",
  hgSubrepo: "hg_subrepo",
  jjColocatedGit: "jj_colocated_git",
}

enum VcsArtifactFormatV1 {
  tarGz: "tar.gz",
  zip,
}

@id("VcsSourceIdentityV1")
model VcsSourceIdentityV1 {
  contract_version: "zed.vcs-source-identity/v1";

  @minLength(1)
  @maxLength(128)
  @pattern("^[a-z][a-z0-9._-]{0,127}$")
  source_id: string;

  vcs_kind: VcsKindV1;

  @minLength(9)
  @maxLength(512)
  @pattern("^https://[^\\s/@]+(?:/[^\\s]*)?$")
  repository_url: string;

  @minLength(40)
  @maxLength(64)
  @pattern("^[a-f0-9]{40,64}$")
  revision: string;

  selector_kind?: VcsSelectorKindV1;

  @minLength(1)
  @maxLength(256)
  selector?: string;

  relation_kind: VcsRelationKindV1;

  @minLength(1)
  @maxLength(512)
  destination_path: string;

  @minLength(3)
  @maxLength(256)
  @pattern("^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$")
  package?: string;

  @minLength(1)
  @maxLength(128)
  package_version?: string;

  @minLength(64)
  @maxLength(64)
  @pattern("^[a-f0-9]{64}$")
  artifact_sha256: string;

  @minValue(1)
  artifact_size: uint64;

  artifact_format: VcsArtifactFormatV1;

  @minLength(1)
  @maxLength(128)
  @pattern("^[a-z][a-z0-9._-]{0,127}$")
  parent_source_id?: string;
}
