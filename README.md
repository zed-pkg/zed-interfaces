# zed-interfaces

Core interface definitions for [zed-pkg](https://github.com/zed-pkg), the
universal package manager backed by the VCS hosts you already use.

One contract, three language slices, each published as its own zed-package:

| slice      | path       | what it is                                            |
| ---------- | ---------- | ----------------------------------------------------- |
| Rust       | `src/rust` | hand-written source of truth (the `zed-interfaces` crate) |
| Dart       | `src/dart` | generated front-end types (`package:zed_interfaces`)  |
| TypeScript | `src/ts`   | generated front-end types (`@zed-pkg/zed-interfaces`) |

Rust covers the whole contract. Dart and TypeScript cover only the part a
browser or Flutter client decodes — the registry API and the sync stream — as
demarcated in [`schemas/index.json`](schemas/index.json). See
[docs/multi-language-layout.md](docs/multi-language-layout.md) for the layout,
the generation pipeline, and why the crate manifest lives in `src/rust/`.

Implementations that compose these types live in
[`zed-lib`](https://github.com/zed-pkg/zed-lib); this repository stays types
and validation.

The Rust crate is the contract everything else builds against:

- **`.zpkg.toml`** — the package manifest at the repo root, TOML only (`manifest` module)
- **`.zpkg.lock`** — the lockfile with artifact hashes and VCS provenance (`lockfile`)
- **Registry REST API** — URL scheme and JSON DTOs shared by `zed-api-server`,
  `zed-cli`, `zed-web-server`, and the SDKs in `zed-clients` (`registry`)
- **Publish excludes** — the default rules that strip tests, CI config,
  `.github/`, and READMEs from published artifacts (`excludes`)
- **Filesystem layout** — `$HOME/.zed-pkg` store, `zed_modules/` symlink dir,
  archive structure (`paths`)
- **VCS + artifact enums** — `git`/`hg`, `tar.gz`/`zip` (`vcs`, `artifact`)
- **Polyglot release routes** — language/ecosystem identity plus canonical
  native registries and compatible GitHub/GitLab/Bitbucket package mirrors
  (`language`, `manifest`)

## The model in one page

A package is `<org>/<name>`. Its source of truth is a repository on any VCS
host — GitHub, GitLab, Bitbucket, Codeberg, SourceHut, Forgejo/Gitea, Azure
DevOps, CodeCommit, Radicle, or a server you run — using git, hg, jj, sapling,
fossil, or pijul (jj and sapling verify through git tags since they push to
git remotes). The registry at zpkg.tech is the primary artifact host; the
declared backing repo doubles as mirror/backup. What gets installed is never
a clone: `zed publish` packs a pruned artifact (no tests, no
CI config, no README unless opted in; licenses always kept), verifies that a
VCS tag matching `publish.tag_format` (default `v{version}`) points at the
published commit, and uploads the archive to the registry, which stores it in
S3-compatible object storage (Cloudflare R2, S3, MinIO).

`zed install` resolves semver requirements against registry metadata, downloads
each artifact once into the global content-addressed store
(`$HOME/.zed-pkg/store/v1/<aa>/<sha256>/pkg`), verifies its sha256, and
symlinks it into the project's `zed_modules/<org>/<name>` — pnpm-style, one
copy per machine no matter how many projects use it. In containers,
`--install-mode copy` materializes files instead of symlinking so image layers
stay self-contained across multi-stage builds.

The lockfile pins `sha256`, `size`, `vcs_tag`, and `vcs_commit` per package:
installs are reproducible and every artifact traces back to source.

For a polyglot repository, each `[targets.<language>]` slice becomes its own
Zed artifact. An optional `[targets.<language>.native]` block declares the
same version for the ecosystem's canonical registry, and `forge` declares
additional package-registry copies:

```toml
[targets.nodejs]
dir = "clients/typescript"

[targets.nodejs.native]
registry = "npm"
package = "@acme/client"
forge = ["github-packages", "gitlab-packages", "bitbucket-packages"]
```

Forge compatibility is validated in the interface contract. A manifest cannot
claim Cargo support in GitHub Packages or PyPI support in Bitbucket Packages;
those combinations fail during manifest parsing, before any release job sees
credentials.

Tag-resolved ecosystems can override the repository release tag. A Go module
below `clients/go`, for example, declares
`tag_format = "clients/go/v{version}"`; validation rejects a subdirectory Go
route without that prefix.

A single-language package whose native manifest is at the repository root uses
the same shape under `[publish.native]`:

```toml
[publish.native]
registry = "npm"
package = "r2g"
forge = ["github-packages", "gitlab-packages", "bitbucket-packages"]
```

## Registry API surface

| Method | Path | Body / response |
| --- | --- | --- |
| GET | `/v1/packages/{org}/{name}` | `PackageMetadata` |
| GET | `/v1/packages/{org}/{name}/versions/{version}` | `VersionMetadata` |
| PUT | `/v1/packages/{org}/{name}/versions/{version}` | multipart `meta` (`PublishMeta` JSON) + `artifact` (bytes) → `PublishResponse` |
| GET | `/v1/artifacts/{sha256}` | artifact bytes or redirect to presigned URL |
| GET | `/v1/search?q=` | `SearchResponse` |
| POST | `/v1/orgs` | `ClaimOrgRequest` → `ClaimOrgResponse` |
| GET | `/healthz` | liveness |

Errors use `ApiError { code, message }`. Authenticated routes take
`Authorization: Bearer <token>`.

## JSON Schemas and the generated slices

`schemas/` holds generated JSON Schema files for every wire type. They are the
source of truth for every non-Rust consumer: the SDKs in
[zed-clients](https://github.com/zed-pkg/zed-clients) codegen and validate
against them, and `codegen/generate.mjs` turns the front-end-facing subset into
the Dart and TypeScript slices.

```sh
cargo run --locked --example generate_schemas   # src/rust  -> schemas/
npm run codegen                                 # schemas/  -> src/dart, src/ts
npm run codegen:check                           # what CI runs; fails on drift
```

`schemas/index.json` decides which schemas cross the language boundary. Every
file in `schemas/` must be listed there — the generator errors on an unlisted
schema rather than skipping it — with `"targets": ["dart", "ts"]` for the
registry/sync DTOs a front end decodes, or `"targets": []` for the toolchain
formats (`manifest`, `lockfile`, environment plans, nix/oci/native records)
that only `zed-cli` and the servers read.

Generated files are never hand-edited: change the Rust type, regenerate both
hops, commit the result.

## Development

This repo is developed side by side with its siblings. Rust consumers depend on
the crate slice:

```toml
zed-interfaces = { path = "../zed-interfaces/src/rust" }
```

```sh
git clone https://github.com/zed-pkg/zed-interfaces
git clone https://github.com/zed-pkg/zed-cli
# ... siblings in the same parent directory
```

```sh
cargo test                                 # Rust slice (virtual workspace root)
npm test                                   # generator unit tests + drift check
cd src/dart && dart pub get && dart analyze
cd src/ts   && npm install && npx tsc --noEmit
```

## License

MIT
