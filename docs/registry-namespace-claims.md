# Cross-registry namespace claims

A Zed organization name cannot be reserved through one universal registry
operation. The providers expose different identity primitives, proof systems,
and mutation boundaries. `zed.registry-namespace-plan/v1` preserves those
differences rather than converting every provider into a misleading Boolean
`reserved` field.

## Provider matrix

| Provider | Identity represented in a plan | Reservation model | Ordinary creation boundary |
| --- | --- | --- | --- |
| npm | organization scope, for example `@acme` | literal organization-owned scope | npm web organization flow |
| Maven Central | verified namespace/groupId prefix, preferably reverse DNS such as `com.example` | prove control of the namespace prefix | Central Portal namespace flow; verification may use DNS or an approved forge identity |
| crates.io | no organization namespace; optional advisory package prefix such as `acme-` | every crate name is globally unique and acquired by publishing that crate | first real crate publication, followed by owner/team management |
| pub.dev | verified publisher domain such as `example.com` | prove control of the domain; package names remain global | pub.dev publisher verification flow |
| GitHub | organization login such as `acme` | globally unique forge organization | GitHub organization web flow for ordinary public accounts |
| GitLab.com | top-level group path such as `acme` | globally unique top-level forge group | GitLab.com web flow; subgroup and self-managed automation are separate capabilities |
| Bitbucket Cloud | workspace ID such as `acme` | globally unique workspace identity | Atlassian Administration workspace flow |

Primary provider documentation:

- npm organizations and scopes: <https://docs.npmjs.com/about-organization-scopes-and-packages>
- Maven Central namespace registration: <https://central.sonatype.org/register/namespace/>
- crates.io publication and ownership: <https://doc.rust-lang.org/cargo/reference/publishing.html>
- pub.dev verified publishers: <https://dart.dev/tools/pub/verified-publishers>
- GitHub organization creation: <https://docs.github.com/organizations/collaborating-with-groups-in-organizations/creating-a-new-organization-from-scratch>
- GitLab groups and namespaces: <https://docs.gitlab.com/user/group/>
- Bitbucket workspaces: <https://support.atlassian.com/bitbucket-cloud/docs/what-is-a-workspace/>

The links explain provider behavior; the versioned Rust types are the wire
contract. A future provider-policy change requires a new planner implementation
or a versioned contract change rather than silent reinterpretation of stored
plans.

## Plan versus receipt

A `RegistryNamespacePlan` is deterministic pre-mutation intent. It contains:

- the portable lowercase brand slug;
- optional canonical domain and explicit GitHub owner;
- exactly one entry per requested provider;
- the provider's real namespace model;
- exact coordinate when one exists;
- whether work is manual, proof-gated, first-publication, or not reservable;
- proof requirements, ordered steps, and warnings.

A plan is never ownership evidence. In particular:

- a crates.io prefix is only a naming convention;
- a Maven groupId derived from a domain is not verified until the provider
  accepts proof;
- a pub.dev publisher coordinate is blocked without domain control;
- a forge login shown as available by a read-only check can be claimed by
  someone else before a human completes the provider flow.

A `RegistryNamespaceClaimReceipt` records one observed provider result. It
references the canonical plan SHA-256 and carries only non-secret evidence.
Successful outcomes require evidence. Credentials, challenge tokens, raw API
responses, and private account metadata do not belong in receipts.

## Portable brand slug

The v1 contract deliberately uses a conservative shared slug so one requested
identity can be represented by all three forges:

```text
[a-z0-9](?:[a-z0-9-]{0,37}[a-z0-9])?
```

The maximum length is 39 characters, uppercase and non-ASCII characters are
rejected, and consecutive hyphens are rejected. This is a portable input, not a
claim that every provider has identical grammar. Provider adapters may impose a
stricter rule and report it as a missing prerequisite or warning.

Domains are stored lowercase without scheme, port, path, credentials, or a
trailing dot. Internationalized domains must already be represented in their
canonical ASCII form before entering this v1 contract.

## Maven coordinate derivation

The planner behavior layer should prefer a verified domain:

```text
example.com       -> com.example
packages.acme.io  -> io.acme.packages
```

When no domain is available, an explicitly supplied GitHub owner may produce an
`io.github.<owner>` candidate. The planner must label this as a proof-dependent
fallback. It must not infer an owner from ambient Git credentials or claim that
Maven Central accepted the namespace.

## crates.io ownership

crates.io has no organization namespace that a planner can reserve. A plan may
recommend `brand-` as a naming prefix and may describe the later owner/team
step, but its provider entry must remain `not-reservable`. A receipt cannot use
the `reserved` outcome for crates.io. Ownership evidence applies to individual
crate names after publication.

## Execution boundary

This repository defines types, validation, canonicalization, and schemas only.
The behavior layer belongs in `zed-lib-core`; read-only availability checks and
mutating adapters remain separate operations.

A safe implementation sequence is:

1. build and hash a deterministic plan;
2. perform read-only availability checks without credentials where supported;
3. collect domain, registry-account, or forge-administrator proof;
4. require explicit consent for each mutating provider step;
5. perform one provider mutation at a time;
6. independently re-read the provider state;
7. emit a receipt tied to the plan digest;
8. stop on conflicts rather than selecting a different external name silently.

No adapter should create placeholder packages merely to squat on names. For
global package-name registries, reservation means the first genuine publication
that meets that ecosystem's policy.