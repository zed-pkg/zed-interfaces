# zed-interfaces

Core interface definitions for [zed-pkg](https://github.com/zed-pkg), the
universal package manager backed by the VCS hosts you already use.

This crate is the contract everything else builds against:

- **`.zpkg.toml`** — the package manifest at the repo root, TOML only (`manifest` module)
- **Interop intent** — typed `[interop.git].consume_gitmodules` ownership in the
  manifest, so consumers do not infer authority from file detection alone
- **`.zpkg.lock`** — the lockfile with artifact hashes and VCS provenance (`lockfile`)
- **Static inspection protocol** — schema-versioned diagnostics, safe action
  metadata, interop status, and update recommendations shared by `zed-lib`,
  `zed-cli`, and editor integrations (`inspection`)
- **Registry REST API** — URL scheme and JSON DTOs shared by `zed-api-server`,
  `zed-cli`, `zed-web-server`, and the SDKs in `zed-clients` (`registry`)
- **Publish excludes** — the default rules that strip tests, CI config,
  `.github/`, and READMEs from published artifacts (`excludes`)
- **Filesystem layout** — `$HOME/.zed-pkg` store, `zed_modules/` symlink dir,
  archive structure (`paths`)
- **VCS + artifact enums** — `git`/`hg`, `tar.gz`/`zip` (`vcs`, `artifact`)

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

## JSON Schemas

`schemas/` holds generated JSON Schema files for every wire type, used by the
non-Rust SDKs in [zed-clients](https://github.com/zed-pkg/zed-clients).
Regenerate after changing any type:

```sh
cargo run --example generate_schemas
```

## Development

This repo is developed side by side with its siblings; the other Rust repos
depend on it via `zed-interfaces = { path = "../zed-interfaces" }`:

```sh
git clone https://github.com/zed-pkg/zed-interfaces
git clone https://github.com/zed-pkg/zed-lib
git clone https://github.com/zed-pkg/zed-cli
# ... siblings in the same parent directory
```

```sh
cargo test
```

## License

MIT
