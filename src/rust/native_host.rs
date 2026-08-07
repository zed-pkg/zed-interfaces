//! Where a native package actually goes on the wire, and how a release
//! candidate is spelled once it gets there.
//!
//! [`crate::manifest::NativeRegistry`] answers *which ecosystem* a target is
//! mirrored into. That is enough to plan a release, but not to perform one: two
//! packages bound for the same ecosystem can land on different hosts (Maven
//! Central vs. an Artifactory mirror), and every ecosystem spells "release
//! candidate" differently. This module supplies the missing half.
//!
//! Three facts are kept apart because they vary independently:
//!
//! * [`NativeHost`] — the concrete registry a package is uploaded to or
//!   resolved from, including the decentralized non-hosts (Zig, plain VCS).
//! * [`RegistryProtocol`] — the wire shape used to talk to it. Hosts outnumber
//!   protocols: Clojars and Maven Central are both [`RegistryProtocol::Maven2`],
//!   and every enterprise proxy in [`UniversalHost`] re-serves an existing
//!   protocol at a different base URL. One protocol implementation therefore
//!   serves many hosts, which is the whole reason this axis exists.
//! * [`ReleaseChannel`] — stable, rc, beta, alpha, nightly, snapshot. How a
//!   channel becomes something a registry will accept is *not* uniform, so
//!   [`NativeHost::channel_route`] resolves it per host rather than letting
//!   callers append `-rc.1` and hope.
//!
//! ## Why channels are not just a version suffix
//!
//! Every one of these is a real, incompatible way to ship a candidate:
//!
//! | Host | Mechanism |
//! | --- | --- |
//! | npm | SemVer prerelease **plus** a dist-tag that is not `latest` |
//! | Hackage | a physically separate endpoint (`/packages/candidates/`) |
//! | Maven | `-SNAPSHOT` into a **different repository** from releases |
//! | PyPI | PEP 440 (`1.2.3rc1`), which is not SemVer, plus TestPyPI |
//! | RubyGems | any letter in the version makes it a prerelease |
//! | Conan | the channel is part of the package reference itself |
//! | CPAN | a TRIAL release, marked by an underscore in the version |
//! | crates.io | SemVer prerelease only — there is no channel at all |
//!
//! A caller that assumed the npm rule would publish an unusable Maven artifact
//! and a rejected PyPI one. [`ChannelRoute`] is the resolved answer, and it
//! carries the endpoint and dist-tag alongside the version so no caller has to
//! re-derive any of it.
//!
//! This module is types, tables, and validation only — no I/O, per the crate's
//! contract boundary. `zed-cli` drives the resulting [`ChannelRoute`] and
//! [`HostEndpoints`] over HTTP.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::language::Ecosystem;

/// The wire protocol used to publish to and resolve from a registry.
///
/// Keyed by protocol rather than by host so that one client implementation
/// covers every host that speaks it — the enterprise proxies in
/// [`UniversalHost`] add hosts without adding protocols.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum RegistryProtocol {
    /// npm registry API: `PUT /{package}` with a base64 `_attachments`
    /// tarball, `GET /{package}` for the packument.
    Npm,
    /// crates.io API: `PUT /api/v1/crates/new` with a length-prefixed
    /// metadata+tarball body, plus a sparse HTTP index.
    CargoSparse,
    /// Maven 2 repository layout: plain `PUT`/`GET` of
    /// `{group-path}/{artifact}/{version}/{artifact}-{version}.{ext}` with a
    /// sibling `maven-metadata.xml`. Also Clojars and every JVM proxy.
    Maven2,
    /// The Central Portal bundle upload that fronts Maven Central releases.
    /// Distinct from [`RegistryProtocol::Maven2`]: it takes one zipped bundle
    /// and returns a deployment id to poll, rather than per-file PUTs.
    MavenCentralPortal,
    /// PyPI legacy upload (`POST /legacy/`, multipart, `:action=file_upload`)
    /// paired with a PEP 503/691 simple index for resolution.
    PypiLegacyUpload,
    /// RubyGems API: `POST /api/v1/gems` with raw `.gem` bytes; versions from
    /// `/api/v1/versions/{name}.json` or the compact index.
    RubyGemsApi,
    /// NuGet V3: `PUT /api/v2/package` to push, service index -> flat
    /// container to resolve.
    NuGetV3,
    /// Hex: `POST /api/publish` with a tarball; tarballs served from
    /// `repo.hex.pm`.
    HexApi,
    /// pub.dev: an authenticated handshake returns a signed upload form, the
    /// archive goes to object storage, then a finalize URL is fetched.
    PubDev,
    /// Hackage: multipart upload to `/packages/`, with `/packages/candidates/`
    /// as a first-class candidate track.
    HackageApi,
    /// LuaRocks: `POST /api/1/{api_key}/upload` with a rockspec.
    LuaRocksApi,
    /// PAUSE, the CPAN upload gateway: multipart `authenquery?ACTION=add_uri`.
    /// Resolution goes through MetaCPAN rather than PAUSE.
    CpanPause,
    /// CRAN's moderated web submission. Uploads are reviewed by humans, so
    /// this protocol can plan and resolve but never auto-publish.
    CranSubmit,
    /// Conan v2 REST, where `user/channel` is part of the package reference.
    ConanRest,
    /// Swift Package Registry (SE-0292): `PUT /{scope}/{name}/{version}`.
    SwiftRegistry,
    /// CocoaPods Trunk: `POST /api/v1/pods` with a podspec.
    CocoapodsTrunk,
    /// PowerShell Gallery, which is NuGet V2 under the hood but with its own
    /// push endpoint and metadata conventions.
    #[serde(rename = "powershell-gallery")]
    PowerShellGallery,
    /// Julia's General registry: a Git-backed registry updated by pull
    /// request, with package sources resolved from VCS.
    JuliaGeneral,
    /// opam: a Git-backed repository of package definitions, updated by pull
    /// request; tarballs are fetched from the URLs those definitions carry.
    OpamRepository,
    /// A registry that stores no artifact and only records where the source
    /// lives — Packagist, the Swift Package Index, the DUB and Nimble
    /// directories, Shards, the Racket catalog, Go's proxy in its
    /// direct-from-VCS mode. Publishing means pushing a tag and (sometimes)
    /// pinging an update endpoint.
    VcsIndexed,
    /// Go module proxy: `/{module}/@v/list`, `/@v/{version}.{info,mod,zip}`.
    /// Read-only by design; publication is a VCS tag the proxy then caches.
    GoProxy,
    /// No registry at all. Dependencies are URLs or Git revisions checked by
    /// hash (Zig), so "publish" is a VCS tag and "pull" is a fetch.
    DirectUrl,
}

impl RegistryProtocol {
    pub fn as_str(self) -> &'static str {
        use RegistryProtocol::*;
        match self {
            Npm => "npm",
            CargoSparse => "cargo-sparse",
            Maven2 => "maven2",
            MavenCentralPortal => "maven-central-portal",
            PypiLegacyUpload => "pypi-legacy-upload",
            RubyGemsApi => "rubygems-api",
            NuGetV3 => "nuget-v3",
            HexApi => "hex-api",
            PubDev => "pub-dev",
            HackageApi => "hackage-api",
            LuaRocksApi => "luarocks-api",
            CpanPause => "cpan-pause",
            CranSubmit => "cran-submit",
            ConanRest => "conan-rest",
            SwiftRegistry => "swift-registry",
            CocoapodsTrunk => "cocoapods-trunk",
            PowerShellGallery => "powershell-gallery",
            JuliaGeneral => "julia-general",
            OpamRepository => "opam-repository",
            VcsIndexed => "vcs-indexed",
            GoProxy => "go-proxy",
            DirectUrl => "direct-url",
        }
    }

    /// Whether this protocol moves artifact bytes over HTTP at publish time.
    ///
    /// False means the "publish" step is a VCS tag (plus, for some hosts, an
    /// unauthenticated index ping) or a pull request against a registry repo.
    /// Callers must not prompt for an upload credential in that case — asking
    /// for a token that is never used is how CI ends up with unnecessary
    /// secrets in scope.
    ///
    /// Orthogonal to [`RegistryProtocol::is_moderated`]: CRAN does upload a
    /// tarball, but a human decides whether it lands.
    pub fn uploads_artifact(self) -> bool {
        use RegistryProtocol::*;
        !matches!(
            self,
            VcsIndexed | GoProxy | DirectUrl | JuliaGeneral | OpamRepository
        )
    }

    /// Whether publication completes without a human in the loop.
    ///
    /// CRAN reviews every submission, and the Git-backed registries merge a
    /// pull request. Release automation must stop at "submitted" for these
    /// rather than reporting a published version that does not exist yet.
    pub fn is_moderated(self) -> bool {
        use RegistryProtocol::*;
        matches!(self, CranSubmit | JuliaGeneral | OpamRepository)
    }
}

/// How a request to a registry is authenticated.
///
/// Distinguishes the schemes that look alike but are not: `Authorization: Bearer
/// <t>` (npm, pub.dev) is rejected by crates.io and RubyGems, which want the
/// bare token in the same header, and NuGet ignores `Authorization` entirely in
/// favour of its own header.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum RegistryAuth {
    /// `Authorization: Bearer <token>`.
    Bearer,
    /// `Authorization: <token>` with no scheme prefix.
    BareToken,
    /// HTTP Basic. PyPI uses the literal username `__token__`; see
    /// [`NativeHost::basic_auth_username`].
    Basic,
    /// A token in a host-specific header, e.g. `X-NuGet-ApiKey`.
    Header(ApiKeyHeader),
    /// The token is a path or query component of the URL, so it must never be
    /// logged as part of the request line.
    UrlEmbedded,
    /// Whatever credential the VCS remote already uses; no registry token.
    VcsCredential,
    /// Anonymous. Public read paths, and the fully decentralized hosts.
    None,
}

/// Header names used by [`RegistryAuth::Header`]. An enum rather than a string
/// so the schema stays closed and a typo cannot silently disable auth.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum ApiKeyHeader {
    /// `X-NuGet-ApiKey`, used by NuGet and the PowerShell Gallery.
    XNuGetApiKey,
    /// `Authorization: Token <token>`, CocoaPods Trunk's spelling.
    AuthorizationToken,
}

impl ApiKeyHeader {
    pub fn header_name(self) -> &'static str {
        match self {
            Self::XNuGetApiKey => "X-NuGet-ApiKey",
            Self::AuthorizationToken => "Authorization",
        }
    }

    /// Prefix placed before the token value, if any.
    pub fn value_prefix(self) -> &'static str {
        match self {
            Self::XNuGetApiKey => "",
            Self::AuthorizationToken => "Token ",
        }
    }
}

/// A release track. `Stable` is the default everywhere; the rest are the
/// pre-release tracks a language ecosystem exposes, in descending maturity.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum ReleaseChannel {
    #[default]
    Stable,
    /// Release candidate: feature-complete, expected to become the release.
    Rc,
    Beta,
    Alpha,
    /// Built from the tip of a branch on a schedule. Immutable per build.
    Nightly,
    /// Deliberately **mutable**: one coordinate, republished repeatedly.
    /// Maven `-SNAPSHOT`, Packagist `dev-*`, LuaRocks `dev`. The release
    /// invariant that rejects same-version/different-content output has to be
    /// relaxed here — see [`ReleaseChannel::is_mutable`].
    Snapshot,
}

impl ReleaseChannel {
    pub fn as_str(self) -> &'static str {
        use ReleaseChannel::*;
        match self {
            Stable => "stable",
            Rc => "rc",
            Beta => "beta",
            Alpha => "alpha",
            Nightly => "nightly",
            Snapshot => "snapshot",
        }
    }

    /// True for the default (`stable`); lets manifests omit the field. Takes
    /// `&self` to match the other manifest defaults, which serde's
    /// `skip_serializing_if` calls by reference.
    pub fn is_default(&self) -> bool {
        matches!(self, ReleaseChannel::Stable)
    }

    /// True for the one channel whose coordinate may be overwritten.
    pub fn is_mutable(self) -> bool {
        matches!(self, ReleaseChannel::Snapshot)
    }

    pub fn from_token(token: &str) -> Option<ReleaseChannel> {
        use ReleaseChannel::*;
        Some(match token.trim().to_ascii_lowercase().as_str() {
            "stable" | "release" | "latest" => Stable,
            "rc" | "candidate" | "release-candidate" => Rc,
            "beta" => Beta,
            "alpha" => Alpha,
            "nightly" | "dev" | "canary" => Nightly,
            "snapshot" => Snapshot,
            _ => return None,
        })
    }

    /// Parse a stored value, degrading unknown input to `stable`. Mirrors
    /// [`crate::language::Language::from_str_lenient`]: a value written by a
    /// newer zed must never wedge an older one.
    ///
    /// Degrading to `stable` is the safe direction for a *read*, but a
    /// publisher must not silently promote an unknown track to stable — the
    /// CLI parses publish-time input with [`ReleaseChannel::from_token`].
    pub fn from_str_lenient(s: &str) -> ReleaseChannel {
        ReleaseChannel::from_token(s).unwrap_or(ReleaseChannel::Stable)
    }

    /// The lowercase identifier an ecosystem conventionally uses inside a
    /// version string. `None` for [`ReleaseChannel::Stable`], which adds
    /// nothing, and for [`ReleaseChannel::Snapshot`], whose spelling is
    /// host-specific rather than a plain identifier.
    fn version_identifier(self) -> Option<&'static str> {
        use ReleaseChannel::*;
        match self {
            Stable | Snapshot => None,
            Rc => Some("rc"),
            Beta => Some("beta"),
            Alpha => Some("alpha"),
            Nightly => Some("nightly"),
        }
    }
}

impl std::fmt::Display for ReleaseChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How a channel is encoded into the version string a host will store.
///
/// The distinctions are load-bearing: PEP 440 forbids the hyphen SemVer
/// requires, RubyGems keys off the presence of a letter, and Maven treats
/// `-SNAPSHOT` as a mutable coordinate rather than an ordinary qualifier.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum PrereleaseSyntax {
    /// SemVer 2.0.0: `1.2.3-rc.1`.
    Semver,
    /// PEP 440: `1.2.3rc1`, `1.2.3b1`, `1.2.3a1`, `1.2.3.dev1`. No hyphen, and
    /// beta/alpha contract to single letters.
    Pep440,
    /// RubyGems: `1.2.3.rc.1`. Any letter anywhere makes the gem a prerelease.
    RubyGemsDotted,
    /// Maven qualifier: `1.2.3-RC1`, uppercase and unseparated, with
    /// `1.2.3-SNAPSHOT` as the mutable track.
    MavenQualifier,
    /// CPAN TRIAL release: an underscore in the version (`1.2.3_01`) marks a
    /// developer release that the indexer will not treat as current.
    CpanTrial,
    /// The version never changes; the channel lives in the package reference
    /// instead (Conan's `name/version@user/channel`).
    ReferenceChannel,
    /// The host has no prerelease concept. Requesting one is an error rather
    /// than a silently-stable publish.
    Unsupported,
}

impl PrereleaseSyntax {
    pub fn as_str(self) -> &'static str {
        use PrereleaseSyntax::*;
        match self {
            Semver => "semver",
            Pep440 => "pep440",
            RubyGemsDotted => "rubygems-dotted",
            MavenQualifier => "maven-qualifier",
            CpanTrial => "cpan-trial",
            ReferenceChannel => "reference-channel",
            Unsupported => "unsupported",
        }
    }

    /// Render `base` on `channel`, given a 1-based `iteration` (rc.1, rc.2, …).
    ///
    /// `base` is the stable version the candidate is aiming at, and is returned
    /// unchanged for [`ReleaseChannel::Stable`].
    fn apply(
        self,
        base: &str,
        channel: ReleaseChannel,
        iteration: u32,
    ) -> Result<String, NativeHostError> {
        use PrereleaseSyntax::*;
        if channel.is_default() {
            return Ok(base.to_string());
        }
        match self {
            Unsupported => Err(NativeHostError::ChannelUnsupported { channel }),
            ReferenceChannel => Ok(base.to_string()),
            Semver => match channel {
                ReleaseChannel::Snapshot => Ok(format!("{base}-SNAPSHOT")),
                _ => {
                    let id = channel
                        .version_identifier()
                        .ok_or(NativeHostError::ChannelUnsupported { channel })?;
                    Ok(format!("{base}-{id}.{iteration}"))
                }
            },
            Pep440 => match channel {
                // PEP 440 spells beta `b` and alpha `a`, and has no hyphen.
                // `.devN` is the closest thing to a mutable track it allows.
                ReleaseChannel::Rc => Ok(format!("{base}rc{iteration}")),
                ReleaseChannel::Beta => Ok(format!("{base}b{iteration}")),
                ReleaseChannel::Alpha => Ok(format!("{base}a{iteration}")),
                ReleaseChannel::Nightly | ReleaseChannel::Snapshot => {
                    Ok(format!("{base}.dev{iteration}"))
                }
                ReleaseChannel::Stable => Ok(base.to_string()),
            },
            RubyGemsDotted => match channel {
                ReleaseChannel::Snapshot => Ok(format!("{base}.pre.{iteration}")),
                _ => {
                    let id = channel
                        .version_identifier()
                        .ok_or(NativeHostError::ChannelUnsupported { channel })?;
                    Ok(format!("{base}.{id}.{iteration}"))
                }
            },
            MavenQualifier => match channel {
                ReleaseChannel::Snapshot => Ok(format!("{base}-SNAPSHOT")),
                _ => {
                    let id = channel
                        .version_identifier()
                        .ok_or(NativeHostError::ChannelUnsupported { channel })?;
                    Ok(format!("{base}-{}{iteration}", id.to_ascii_uppercase()))
                }
            },
            // A TRIAL release is marked by the underscore itself, so the
            // channel identifier would be redundant noise in the version.
            CpanTrial => Ok(format!("{base}_{iteration:02}")),
        }
    }
}

/// The URLs a host exposes. Relative paths are templates completed by the
/// client; absolute URLs are used as-is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HostEndpoints {
    /// Base URL for writes. `None` for read-only and VCS-published hosts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publish: Option<String>,
    /// Base URL for version discovery.
    pub index: String,
    /// Base URL artifacts are downloaded from, when it differs from `index`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download: Option<String>,
}

impl HostEndpoints {
    fn new(publish: Option<&str>, index: &str, download: Option<&str>) -> Self {
        Self {
            publish: publish.map(str::to_string),
            index: index.to_string(),
            download: download.map(str::to_string),
        }
    }

    /// Where artifacts are fetched from: the explicit download base, else the
    /// index base.
    pub fn download_base(&self) -> &str {
        self.download.as_deref().unwrap_or(&self.index)
    }
}

/// A concrete registry host.
///
/// One variant per row of the ecosystem table, including the three that host
/// nothing — [`NativeHost::Zig`], [`NativeHost::GoProxy`] in direct mode, and
/// [`NativeHost::Vcs`] — because a release plan still has to say where those
/// packages come from.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum NativeHost {
    /// npmjs.com
    Npm,
    /// crates.io
    CratesIo,
    /// Maven Central via the Sonatype Central Portal.
    MavenCentral,
    /// pypi.org
    PyPi,
    /// test.pypi.org — a full mirror used as a rehearsal track.
    TestPyPi,
    /// rubygems.org
    RubyGems,
    /// packagist.org
    Packagist,
    /// nuget.org
    NuGet,
    /// proxy.golang.org
    GoProxy,
    /// conan.io/center
    ConanCenter,
    /// swiftpackageindex.com
    SwiftPackageIndex,
    /// cocoapods.org trunk
    CocoaPods,
    /// pub.dev
    PubDev,
    /// hex.pm
    Hex,
    /// PAUSE/CPAN, resolved through MetaCPAN.
    Cpan,
    /// cran.r-project.org
    Cran,
    /// hackage.haskell.org
    Hackage,
    /// stackage.org — curated snapshots of Hackage; resolve-only.
    Stackage,
    /// clojars.org
    Clojars,
    /// luarocks.org
    LuaRocks,
    /// The Julia General registry.
    JuliaGeneral,
    /// opam.ocaml.org
    Opam,
    /// code.dlang.org
    Dub,
    /// nimble.directory
    Nimble,
    /// shards.info
    Shards,
    /// pkgs.racket-lang.org
    Racket,
    /// powershellgallery.com
    #[serde(rename = "powershell-gallery")]
    PowerShellGallery,
    /// Zig, which has no registry: dependencies are URLs pinned by hash.
    Zig,
    /// A plain Git or Mercurial remote on GitHub, GitLab, or Bitbucket. Zed
    /// supports these first-class, so a target may declare no registry at all
    /// and still be a complete, resolvable route.
    Vcs,
}

impl NativeHost {
    pub fn as_str(self) -> &'static str {
        use NativeHost::*;
        match self {
            Npm => "npm",
            CratesIo => "crates-io",
            MavenCentral => "maven-central",
            PyPi => "pypi",
            TestPyPi => "test-pypi",
            RubyGems => "rubygems",
            Packagist => "packagist",
            NuGet => "nuget",
            GoProxy => "go-proxy",
            ConanCenter => "conan-center",
            SwiftPackageIndex => "swift-package-index",
            CocoaPods => "cocoapods",
            PubDev => "pub-dev",
            Hex => "hex",
            Cpan => "cpan",
            Cran => "cran",
            Hackage => "hackage",
            Stackage => "stackage",
            Clojars => "clojars",
            LuaRocks => "luarocks",
            JuliaGeneral => "julia-general",
            Opam => "opam",
            Dub => "dub",
            Nimble => "nimble",
            Shards => "shards",
            Racket => "racket",
            PowerShellGallery => "powershell-gallery",
            Zig => "zig",
            Vcs => "vcs",
        }
    }

    pub fn from_token(token: &str) -> Option<NativeHost> {
        use NativeHost::*;
        let normalized: String = token
            .trim()
            .to_ascii_lowercase()
            .chars()
            .map(|c| if c == '_' || c == ' ' { '-' } else { c })
            .collect();
        Some(match normalized.as_str() {
            "npm" | "npmjs" | "npmjs.com" => Npm,
            "crates-io" | "crates.io" | "cargo" => CratesIo,
            "maven-central" | "maven" | "central" | "sonatype" => MavenCentral,
            "pypi" | "pypi.org" => PyPi,
            "test-pypi" | "testpypi" => TestPyPi,
            "rubygems" | "rubygems.org" | "gem" => RubyGems,
            "packagist" | "packagist.org" | "composer" => Packagist,
            "nuget" | "nuget.org" => NuGet,
            "go-proxy" | "goproxy" | "go-modules" | "golang" => GoProxy,
            "conan-center" | "conancenter" | "conan" => ConanCenter,
            "swift-package-index" | "swiftpackageindex" | "spi" => SwiftPackageIndex,
            "cocoapods" | "cocoapods-trunk" | "trunk" => CocoaPods,
            "pub-dev" | "pub.dev" | "pub" => PubDev,
            "hex" | "hex.pm" => Hex,
            "cpan" | "pause" | "metacpan" => Cpan,
            "cran" => Cran,
            "hackage" => Hackage,
            "stackage" => Stackage,
            "clojars" => Clojars,
            "luarocks" => LuaRocks,
            "julia-general" | "julia" | "general" => JuliaGeneral,
            "opam" => Opam,
            "dub" | "code.dlang.org" | "dlang" => Dub,
            "nimble" | "nimble.directory" => Nimble,
            "shards" | "shards.info" => Shards,
            "racket" | "racket-catalog" => Racket,
            "powershell-gallery" | "powershellgallery" | "psgallery" => PowerShellGallery,
            "zig" => Zig,
            "vcs" | "git" | "hg" | "mercurial" => Vcs,
            _ => return None,
        })
    }

    /// The wire protocol this host speaks.
    pub fn protocol(self) -> RegistryProtocol {
        use NativeHost::*;
        use RegistryProtocol as P;
        match self {
            Npm => P::Npm,
            CratesIo => P::CargoSparse,
            MavenCentral => P::MavenCentralPortal,
            // Clojars predates the Central Portal and still takes plain
            // Maven 2 PUTs, so it is a different protocol from Maven Central
            // despite serving the same artifacts to the same consumers.
            Clojars => P::Maven2,
            PyPi | TestPyPi => P::PypiLegacyUpload,
            RubyGems => P::RubyGemsApi,
            NuGet => P::NuGetV3,
            PowerShellGallery => P::PowerShellGallery,
            Hex => P::HexApi,
            PubDev => P::PubDev,
            Hackage | Stackage => P::HackageApi,
            LuaRocks => P::LuaRocksApi,
            Cpan => P::CpanPause,
            Cran => P::CranSubmit,
            ConanCenter => P::ConanRest,
            SwiftPackageIndex => P::SwiftRegistry,
            CocoaPods => P::CocoapodsTrunk,
            JuliaGeneral => P::JuliaGeneral,
            Opam => P::OpamRepository,
            GoProxy => P::GoProxy,
            Packagist | Dub | Nimble | Shards | Racket => P::VcsIndexed,
            Zig | Vcs => P::DirectUrl,
        }
    }

    /// The zed [`Ecosystem`] a consumer needs in order to install from this
    /// host. Mirrors [`crate::manifest::NativeRegistry::ecosystem`] so the
    /// inbound and outbound views of one target can be cross-checked.
    pub fn ecosystem(self) -> Ecosystem {
        use NativeHost::*;
        match self {
            Npm => Ecosystem::Npm,
            CratesIo => Ecosystem::Cargo,
            MavenCentral | Clojars => Ecosystem::Jvm,
            PyPi | TestPyPi => Ecosystem::Pypi,
            RubyGems => Ecosystem::Gem,
            Packagist => Ecosystem::Composer,
            NuGet => Ecosystem::Nuget,
            GoProxy => Ecosystem::Gomod,
            ConanCenter | Dub => Ecosystem::Cmake,
            SwiftPackageIndex | CocoaPods => Ecosystem::Swiftpm,
            PubDev => Ecosystem::Pub,
            Hex => Ecosystem::Hex,
            Cpan | Cran => Ecosystem::Cran,
            Hackage | Stackage => Ecosystem::Hackage,
            LuaRocks => Ecosystem::Luarocks,
            JuliaGeneral => Ecosystem::Julia,
            Opam => Ecosystem::Opam,
            Nimble => Ecosystem::Nimble,
            Shards => Ecosystem::Shards,
            PowerShellGallery => Ecosystem::Psgallery,
            Zig => Ecosystem::Zig,
            Racket | Vcs => Ecosystem::Universal,
        }
    }

    /// Canonical endpoints. Enterprise mirrors override these wholesale via
    /// [`UniversalHost::endpoints_for`] while keeping the same protocol.
    pub fn endpoints(self) -> HostEndpoints {
        use NativeHost::*;
        match self {
            Npm => HostEndpoints::new(
                Some("https://registry.npmjs.org"),
                "https://registry.npmjs.org",
                None,
            ),
            CratesIo => HostEndpoints::new(
                Some("https://crates.io/api/v1"),
                "https://index.crates.io",
                Some("https://static.crates.io/crates"),
            ),
            MavenCentral => HostEndpoints::new(
                Some("https://central.sonatype.com/api/v1/publisher"),
                "https://repo.maven.apache.org/maven2",
                None,
            ),
            Clojars => HostEndpoints::new(
                Some("https://repo.clojars.org"),
                "https://repo.clojars.org",
                None,
            ),
            PyPi => HostEndpoints::new(
                Some("https://upload.pypi.org/legacy/"),
                "https://pypi.org/simple",
                Some("https://files.pythonhosted.org"),
            ),
            TestPyPi => HostEndpoints::new(
                Some("https://test.pypi.org/legacy/"),
                "https://test.pypi.org/simple",
                Some("https://test-files.pythonhosted.org"),
            ),
            RubyGems => HostEndpoints::new(
                Some("https://rubygems.org/api/v1"),
                "https://rubygems.org/api/v1",
                Some("https://rubygems.org/gems"),
            ),
            Packagist => HostEndpoints::new(
                Some("https://packagist.org/api"),
                "https://repo.packagist.org/p2",
                None,
            ),
            NuGet => HostEndpoints::new(
                Some("https://www.nuget.org/api/v2/package"),
                "https://api.nuget.org/v3/index.json",
                Some("https://api.nuget.org/v3-flatcontainer"),
            ),
            PowerShellGallery => HostEndpoints::new(
                Some("https://www.powershellgallery.com/api/v2/package"),
                "https://www.powershellgallery.com/api/v2",
                None,
            ),
            GoProxy => HostEndpoints::new(None, "https://proxy.golang.org", None),
            ConanCenter => HostEndpoints::new(None, "https://center.conan.io", None),
            SwiftPackageIndex => HostEndpoints::new(
                None,
                "https://swiftpackageindex.com/api",
                None,
            ),
            CocoaPods => HostEndpoints::new(
                Some("https://trunk.cocoapods.org/api/v1"),
                "https://cdn.cocoapods.org",
                None,
            ),
            PubDev => HostEndpoints::new(
                Some("https://pub.dev/api"),
                "https://pub.dev/api",
                None,
            ),
            Hex => HostEndpoints::new(
                Some("https://hex.pm/api"),
                "https://repo.hex.pm",
                Some("https://repo.hex.pm/tarballs"),
            ),
            Cpan => HostEndpoints::new(
                Some("https://pause.perl.org/pause/authenquery"),
                "https://fastapi.metacpan.org/v1",
                Some("https://cpan.metacpan.org/authors/id"),
            ),
            Cran => HostEndpoints::new(
                Some("https://xmpalantir.wu.ac.at/cransubmit/index2.php"),
                "https://cran.r-project.org/src/contrib",
                None,
            ),
            Hackage => HostEndpoints::new(
                Some("https://hackage.haskell.org/packages"),
                "https://hackage.haskell.org",
                Some("https://hackage.haskell.org/package"),
            ),
            // Stackage curates Hackage; it publishes nothing of its own.
            Stackage => HostEndpoints::new(
                None,
                "https://stackage.org",
                Some("https://hackage.haskell.org/package"),
            ),
            LuaRocks => HostEndpoints::new(
                Some("https://luarocks.org/api/1"),
                "https://luarocks.org/manifest",
                Some("https://luarocks.org"),
            ),
            JuliaGeneral => HostEndpoints::new(
                None,
                "https://pkg.julialang.org/registry",
                None,
            ),
            Opam => HostEndpoints::new(None, "https://opam.ocaml.org/packages", None),
            Dub => HostEndpoints::new(None, "https://code.dlang.org/api/packages", None),
            Nimble => HostEndpoints::new(None, "https://nimble.directory/api/v1", None),
            Shards => HostEndpoints::new(None, "https://shards.info/api", None),
            Racket => HostEndpoints::new(None, "https://pkgs.racket-lang.org/pkgs-all", None),
            // No registry: the index *is* the VCS remote, supplied per package.
            Zig | Vcs => HostEndpoints::new(None, "", None),
        }
    }

    /// Auth scheme for writes.
    pub fn publish_auth(self) -> RegistryAuth {
        use NativeHost::*;
        use RegistryAuth as A;
        match self {
            Npm | PubDev => A::Bearer,
            // crates.io and RubyGems both want the raw token in
            // `Authorization` with no scheme word; Bearer is rejected.
            CratesIo | RubyGems | Hex => A::BareToken,
            MavenCentral | Clojars | PyPi | TestPyPi | Hackage | Cpan => A::Basic,
            NuGet | PowerShellGallery => A::Header(ApiKeyHeader::XNuGetApiKey),
            CocoaPods => A::Header(ApiKeyHeader::AuthorizationToken),
            // LuaRocks puts the API key in the path, and Packagist in the
            // query string; both must be redacted from logs.
            LuaRocks | Packagist => A::UrlEmbedded,
            ConanCenter | SwiftPackageIndex => A::Bearer,
            Cran => A::None,
            GoProxy | Stackage | JuliaGeneral | Opam | Dub | Nimble | Shards | Racket | Zig
            | Vcs => A::VcsCredential,
        }
    }

    /// Auth scheme for reads. Public registries are anonymous; an enterprise
    /// mirror overrides this.
    pub fn index_auth(self) -> RegistryAuth {
        RegistryAuth::None
    }

    /// The fixed username HTTP Basic requires, where the host mandates one.
    ///
    /// PyPI's is the literal string `__token__` — passing the account name
    /// instead is the single most common upload failure, so it is encoded here
    /// rather than left to each caller.
    pub fn basic_auth_username(self) -> Option<&'static str> {
        match self {
            NativeHost::PyPi | NativeHost::TestPyPi => Some("__token__"),
            _ => None,
        }
    }

    /// How this host spells a prerelease version.
    pub fn prerelease_syntax(self) -> PrereleaseSyntax {
        use NativeHost::*;
        use PrereleaseSyntax as S;
        match self {
            Npm | CratesIo | NuGet | PubDev | Hex | SwiftPackageIndex | CocoaPods | GoProxy
            | Packagist | Dub | Nimble | Shards | Zig | Vcs | Hackage | JuliaGeneral => S::Semver,
            PyPi | TestPyPi => S::Pep440,
            RubyGems => S::RubyGemsDotted,
            MavenCentral | Clojars => S::MavenQualifier,
            Cpan => S::CpanTrial,
            ConanCenter => S::ReferenceChannel,
            // LuaRocks versions carry a rockspec revision rather than a
            // prerelease, PowerShell Gallery predates SemVer prerelease in its
            // V2 API, and Racket/opam/CRAN/Stackage have no candidate track.
            LuaRocks | PowerShellGallery | Racket | Opam | Cran | Stackage => S::Unsupported,
        }
    }

    /// A distribution tag or channel label applied at publish time, if the host
    /// has one. npm requires it: publishing a candidate without a non-`latest`
    /// dist-tag makes `npm install <pkg>` serve the candidate.
    pub fn dist_tag(self, channel: ReleaseChannel) -> Option<String> {
        if channel.is_default() {
            return match self {
                NativeHost::Npm => Some("latest".to_string()),
                NativeHost::ConanCenter => Some("stable".to_string()),
                _ => None,
            };
        }
        match self {
            NativeHost::Npm => Some(channel.as_str().to_string()),
            // Conan's channel is part of the reference, so it is always
            // present and `testing` is the conventional non-stable one.
            NativeHost::ConanCenter => Some(match channel {
                ReleaseChannel::Stable => "stable",
                _ => "testing",
            }
            .to_string()),
            _ => None,
        }
    }

    /// A host-level endpoint override for a channel, where the track is a
    /// different place rather than a different version string.
    ///
    /// Returns `None` when the channel shares the stable endpoints.
    pub fn channel_endpoints(self, channel: ReleaseChannel) -> Option<HostEndpoints> {
        if channel.is_default() {
            return None;
        }
        match self {
            // Hackage candidates are a separate resource with their own
            // upload path and their own listing.
            NativeHost::Hackage => Some(HostEndpoints::new(
                Some("https://hackage.haskell.org/packages/candidates"),
                "https://hackage.haskell.org",
                Some("https://hackage.haskell.org/package"),
            )),
            // Maven snapshots live in a physically separate repository;
            // pushing one to the release repo is rejected.
            NativeHost::MavenCentral if channel == ReleaseChannel::Snapshot => {
                Some(HostEndpoints::new(
                    Some("https://central.sonatype.com/repository/maven-snapshots"),
                    "https://central.sonatype.com/repository/maven-snapshots",
                    None,
                ))
            }
            NativeHost::Clojars if channel == ReleaseChannel::Snapshot => None,
            _ => None,
        }
    }

    /// Whether a consumer must explicitly opt in before a resolver will select
    /// a package from this channel.
    ///
    /// True almost everywhere — that is the point of a candidate track — but
    /// worth carrying explicitly, because a publisher reading a plan needs to
    /// know whether shipping a candidate can move an unpinned consumer.
    pub fn channel_requires_opt_in(self, channel: ReleaseChannel) -> bool {
        !channel.is_default()
    }

    /// Resolve a channel to the exact coordinates a publish or fetch will use.
    ///
    /// `base` is the stable version the release is aiming at; `iteration` is
    /// the 1-based candidate number within the channel.
    pub fn channel_route(
        self,
        base: &str,
        channel: ReleaseChannel,
        iteration: u32,
    ) -> Result<ChannelRoute, NativeHostError> {
        if base.trim().is_empty() {
            return Err(NativeHostError::EmptyVersion { host: self });
        }
        if !channel.is_default() && iteration == 0 {
            return Err(NativeHostError::ZeroIteration { channel });
        }
        let syntax = self.prerelease_syntax();
        let version = syntax
            .apply(base, channel, iteration)
            .map_err(|error| match error {
                NativeHostError::ChannelUnsupported { channel } => {
                    NativeHostError::HostChannelUnsupported {
                        host: self,
                        channel,
                    }
                }
                other => other,
            })?;
        Ok(ChannelRoute {
            host: self,
            protocol: self.protocol(),
            channel,
            version,
            dist_tag: self.dist_tag(channel),
            endpoints: self
                .channel_endpoints(channel)
                .unwrap_or_else(|| self.endpoints()),
            requires_opt_in: self.channel_requires_opt_in(channel),
            mutable: channel.is_mutable(),
            moderated: self.protocol().is_moderated(),
        })
    }

    /// Every host, for exhaustive validation and for error messages that list
    /// what a manifest could have meant.
    pub const ALL: &'static [NativeHost] = &[
        NativeHost::Npm,
        NativeHost::CratesIo,
        NativeHost::MavenCentral,
        NativeHost::PyPi,
        NativeHost::TestPyPi,
        NativeHost::RubyGems,
        NativeHost::Packagist,
        NativeHost::NuGet,
        NativeHost::GoProxy,
        NativeHost::ConanCenter,
        NativeHost::SwiftPackageIndex,
        NativeHost::CocoaPods,
        NativeHost::PubDev,
        NativeHost::Hex,
        NativeHost::Cpan,
        NativeHost::Cran,
        NativeHost::Hackage,
        NativeHost::Stackage,
        NativeHost::Clojars,
        NativeHost::LuaRocks,
        NativeHost::JuliaGeneral,
        NativeHost::Opam,
        NativeHost::Dub,
        NativeHost::Nimble,
        NativeHost::Shards,
        NativeHost::Racket,
        NativeHost::PowerShellGallery,
        NativeHost::Zig,
        NativeHost::Vcs,
    ];
}

impl std::fmt::Display for NativeHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The fully resolved destination for one (host, channel, version) triple.
///
/// This is what a publisher acts on and what `zed release plan` prints: it
/// already accounts for the endpoint override, the dist-tag, and the
/// ecosystem's version spelling, so no downstream caller re-derives any of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ChannelRoute {
    pub host: NativeHost,
    pub protocol: RegistryProtocol,
    pub channel: ReleaseChannel,
    /// The version exactly as the host will store it.
    pub version: String,
    /// Distribution tag applied at publish time, where the host has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dist_tag: Option<String>,
    pub endpoints: HostEndpoints,
    /// Consumers must opt in before a resolver selects this version.
    pub requires_opt_in: bool,
    /// The coordinate may be republished with different content.
    pub mutable: bool,
    /// Publication completes only after human review or a merged pull request.
    pub moderated: bool,
}

/// A multi-format registry that re-serves an existing [`RegistryProtocol`] at
/// its own base URL.
///
/// These add no protocol of their own, which is exactly why they are a separate
/// axis: an org that proxies npm through Artifactory needs a different URL and
/// credential, not a different client.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum UniversalHost {
    Artifactory,
    Nexus,
    GithubPackages,
    GitlabPackages,
    BitbucketPackages,
    AwsCodeArtifact,
    Cloudsmith,
    AzureArtifacts,
}

impl UniversalHost {
    pub fn as_str(self) -> &'static str {
        use UniversalHost::*;
        match self {
            Artifactory => "artifactory",
            Nexus => "nexus",
            GithubPackages => "github-packages",
            GitlabPackages => "gitlab-packages",
            BitbucketPackages => "bitbucket-packages",
            AwsCodeArtifact => "aws-codeartifact",
            Cloudsmith => "cloudsmith",
            AzureArtifacts => "azure-artifacts",
        }
    }

    pub fn from_token(token: &str) -> Option<UniversalHost> {
        use UniversalHost::*;
        Some(
            match token.trim().to_ascii_lowercase().replace('_', "-").as_str() {
                "artifactory" | "jfrog" => Artifactory,
                "nexus" | "sonatype-nexus" => Nexus,
                "github-packages" | "ghcr-packages" => GithubPackages,
                "gitlab-packages" => GitlabPackages,
                "bitbucket-packages" => BitbucketPackages,
                "aws-codeartifact" | "codeartifact" => AwsCodeArtifact,
                "cloudsmith" => Cloudsmith,
                "azure-artifacts" | "azure-devops-artifacts" => AzureArtifacts,
                _ => return None,
            },
        )
    }

    /// Protocols this host documents support for.
    ///
    /// Failing closed keeps a manifest from promising, say, Cargo through
    /// GitHub Packages, which has no Cargo endpoint. Support here means the
    /// route can be represented and planned.
    pub fn supports(self, protocol: RegistryProtocol) -> bool {
        use RegistryProtocol as P;
        use UniversalHost::*;
        match self {
            // The two universal binary repositories: everything with an
            // HTTP-upload protocol, plus Conan, which Artifactory originated.
            Artifactory | Nexus => matches!(
                protocol,
                P::Npm
                    | P::CargoSparse
                    | P::Maven2
                    | P::PypiLegacyUpload
                    | P::RubyGemsApi
                    | P::NuGetV3
                    | P::ConanRest
                    | P::GoProxy
                    | P::PowerShellGallery
                    | P::LuaRocksApi
                    | P::CocoapodsTrunk
                    | P::SwiftRegistry
            ),
            // No Cargo and no PyPI endpoint: GitHub Packages serves npm,
            // Maven/Gradle, RubyGems, NuGet, and containers only.
            GithubPackages => matches!(
                protocol,
                P::Npm | P::Maven2 | P::RubyGemsApi | P::NuGetV3
            ),
            GitlabPackages => matches!(
                protocol,
                P::Npm
                    | P::Maven2
                    | P::PypiLegacyUpload
                    | P::RubyGemsApi
                    | P::NuGetV3
                    | P::GoProxy
                    | P::ConanRest
                    | P::VcsIndexed
            ),
            BitbucketPackages => matches!(protocol, P::Npm | P::Maven2),
            AwsCodeArtifact => matches!(
                protocol,
                P::Npm
                    | P::Maven2
                    | P::PypiLegacyUpload
                    | P::NuGetV3
                    | P::CargoSparse
                    | P::GoProxy
                    | P::RubyGemsApi
                    | P::SwiftRegistry
            ),
            Cloudsmith => matches!(
                protocol,
                P::Npm
                    | P::CargoSparse
                    | P::Maven2
                    | P::PypiLegacyUpload
                    | P::RubyGemsApi
                    | P::NuGetV3
                    | P::GoProxy
                    | P::ConanRest
                    | P::HexApi
                    | P::LuaRocksApi
                    | P::CpanPause
                    | P::CranSubmit
            ),
            AzureArtifacts => matches!(
                protocol,
                P::Npm | P::Maven2 | P::PypiLegacyUpload | P::NuGetV3 | P::CargoSparse
            ),
        }
    }

    /// Rewrite a host's endpoints onto this mirror.
    ///
    /// `base` is the org-specific root (`https://acme.jfrog.io/artifactory/npm-local`).
    /// The protocol is unchanged, so a client keyed on
    /// [`ChannelRoute::protocol`] needs no new code path.
    pub fn endpoints_for(
        self,
        host: NativeHost,
        base: &str,
    ) -> Result<HostEndpoints, NativeHostError> {
        let protocol = host.protocol();
        if !self.supports(protocol) {
            return Err(NativeHostError::UniversalHostUnsupported {
                universal: self,
                host,
                protocol,
            });
        }
        let base = base.trim_end_matches('/');
        if !base.starts_with("https://") {
            return Err(NativeHostError::InsecureBase {
                universal: self,
                base: base.to_string(),
            });
        }
        Ok(HostEndpoints::new(Some(base), base, Some(base)))
    }

    pub const ALL: &'static [UniversalHost] = &[
        UniversalHost::Artifactory,
        UniversalHost::Nexus,
        UniversalHost::GithubPackages,
        UniversalHost::GitlabPackages,
        UniversalHost::BitbucketPackages,
        UniversalHost::AwsCodeArtifact,
        UniversalHost::Cloudsmith,
        UniversalHost::AzureArtifacts,
    ];
}

impl std::fmt::Display for UniversalHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NativeHostError {
    #[error("release channel `{channel}` cannot be encoded in this version syntax")]
    ChannelUnsupported { channel: ReleaseChannel },
    #[error("`{host}` has no `{channel}` track; publish to the stable channel or pick another host")]
    HostChannelUnsupported {
        host: NativeHost,
        channel: ReleaseChannel,
    },
    #[error("a `{channel}` release needs an iteration of at least 1")]
    ZeroIteration { channel: ReleaseChannel },
    #[error("`{host}` requires a non-empty base version")]
    EmptyVersion { host: NativeHost },
    #[error("`{universal}` does not serve the `{protocol}` protocol used by `{host}`")]
    UniversalHostUnsupported {
        universal: UniversalHost,
        host: NativeHost,
        protocol: RegistryProtocol,
    },
    #[error("`{universal}` base URL `{base}` must use https")]
    InsecureBase {
        universal: UniversalHost,
        base: String,
    },
}

impl std::fmt::Display for RegistryProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_host_round_trips_through_its_own_token() {
        // Guards the parallel match arms. A variant added to `as_str` but not
        // `from_token` would make a manifest that zed itself printed
        // unreadable by zed.
        for host in NativeHost::ALL {
            assert_eq!(
                NativeHost::from_token(host.as_str()),
                Some(*host),
                "`{host}` does not parse back from its own as_str()"
            );
        }
        for universal in UniversalHost::ALL {
            assert_eq!(
                UniversalHost::from_token(universal.as_str()),
                Some(*universal),
                "`{universal}` does not parse back from its own as_str()"
            );
        }
    }

    #[test]
    fn all_covers_every_host_variant() {
        // `ALL` drives validation and diagnostics; a host missing from it
        // would never be checked.
        assert_eq!(NativeHost::ALL.len(), 29);
        let mut seen: Vec<&str> = NativeHost::ALL.iter().map(|h| h.as_str()).collect();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), NativeHost::ALL.len(), "duplicate host token");
    }

    #[test]
    fn host_tokens_accept_the_domain_people_actually_type() {
        assert_eq!(NativeHost::from_token("crates.io"), Some(NativeHost::CratesIo));
        assert_eq!(NativeHost::from_token("pub.dev"), Some(NativeHost::PubDev));
        assert_eq!(NativeHost::from_token("hex.pm"), Some(NativeHost::Hex));
        assert_eq!(NativeHost::from_token("Maven"), Some(NativeHost::MavenCentral));
        assert_eq!(NativeHost::from_token("go_modules"), Some(NativeHost::GoProxy));
        assert_eq!(NativeHost::from_token("cobol-central"), None);
    }

    #[test]
    fn serde_uses_the_canonical_kebab_token() {
        // The manifest is the wire format; a rename invalidates published
        // manifests, so pin the spelling.
        assert_eq!(
            serde_json::to_string(&NativeHost::CratesIo).unwrap(),
            "\"crates-io\""
        );
        assert_eq!(
            serde_json::from_str::<NativeHost>("\"swift-package-index\"").unwrap(),
            NativeHost::SwiftPackageIndex
        );
        assert_eq!(
            serde_json::to_string(&ReleaseChannel::Rc).unwrap(),
            "\"rc\""
        );
    }

    #[test]
    fn each_ecosystem_spells_a_candidate_its_own_way() {
        // The reason this module exists: one `--channel rc` must not become
        // one version string.
        let cases = [
            (NativeHost::Npm, "1.4.0-rc.1"),
            (NativeHost::CratesIo, "1.4.0-rc.1"),
            (NativeHost::PyPi, "1.4.0rc1"),
            (NativeHost::RubyGems, "1.4.0.rc.1"),
            (NativeHost::MavenCentral, "1.4.0-RC1"),
            (NativeHost::Cpan, "1.4.0_01"),
            // Conan keeps the version and moves the channel into the
            // reference instead.
            (NativeHost::ConanCenter, "1.4.0"),
        ];
        for (host, expected) in cases {
            let route = host.channel_route("1.4.0", ReleaseChannel::Rc, 1).unwrap();
            assert_eq!(route.version, expected, "{host} candidate version");
            assert!(route.requires_opt_in, "{host} candidate must be opt-in");
        }
    }

    #[test]
    fn pep440_contracts_beta_and_alpha_to_single_letters() {
        // `1.4.0-beta.1` is not a legal PEP 440 version; PyPI would reject it.
        let beta = NativeHost::PyPi
            .channel_route("1.4.0", ReleaseChannel::Beta, 2)
            .unwrap();
        assert_eq!(beta.version, "1.4.0b2");
        let alpha = NativeHost::PyPi
            .channel_route("1.4.0", ReleaseChannel::Alpha, 1)
            .unwrap();
        assert_eq!(alpha.version, "1.4.0a1");
        let nightly = NativeHost::PyPi
            .channel_route("1.4.0", ReleaseChannel::Nightly, 7)
            .unwrap();
        assert_eq!(nightly.version, "1.4.0.dev7");
    }

    #[test]
    fn npm_candidates_get_a_dist_tag_that_is_not_latest() {
        // Without this, `npm install pkg` serves the candidate.
        let stable = NativeHost::Npm
            .channel_route("2.0.0", ReleaseChannel::Stable, 1)
            .unwrap();
        assert_eq!(stable.dist_tag.as_deref(), Some("latest"));
        assert!(!stable.requires_opt_in);

        let rc = NativeHost::Npm
            .channel_route("2.0.0", ReleaseChannel::Rc, 3)
            .unwrap();
        assert_eq!(rc.version, "2.0.0-rc.3");
        assert_eq!(rc.dist_tag.as_deref(), Some("rc"));
    }

    #[test]
    fn hackage_candidates_and_maven_snapshots_move_the_endpoint() {
        // These two tracks are a different *place*, not a different version.
        let candidate = NativeHost::Hackage
            .channel_route("1.0.0", ReleaseChannel::Rc, 1)
            .unwrap();
        assert_eq!(
            candidate.endpoints.publish.as_deref(),
            Some("https://hackage.haskell.org/packages/candidates")
        );
        assert_ne!(
            candidate.endpoints.publish,
            NativeHost::Hackage.endpoints().publish
        );

        let snapshot = NativeHost::MavenCentral
            .channel_route("1.0.0", ReleaseChannel::Snapshot, 1)
            .unwrap();
        assert_eq!(snapshot.version, "1.0.0-SNAPSHOT");
        assert!(snapshot.mutable, "snapshots are republishable by design");
        assert_eq!(
            snapshot.endpoints.publish.as_deref(),
            Some("https://central.sonatype.com/repository/maven-snapshots")
        );

        // A Maven *release candidate* is an ordinary immutable version and
        // must stay on the release repository.
        let rc = NativeHost::MavenCentral
            .channel_route("1.0.0", ReleaseChannel::Rc, 2)
            .unwrap();
        assert_eq!(rc.version, "1.0.0-RC2");
        assert!(!rc.mutable);
        assert_eq!(rc.endpoints.publish, NativeHost::MavenCentral.endpoints().publish);
    }

    #[test]
    fn hosts_without_a_candidate_track_fail_loudly() {
        // Silently publishing a candidate as stable is the dangerous
        // alternative: it would move every unpinned consumer.
        for host in [
            NativeHost::LuaRocks,
            NativeHost::Cran,
            NativeHost::Racket,
            NativeHost::Opam,
            NativeHost::PowerShellGallery,
            NativeHost::Stackage,
        ] {
            let error = host
                .channel_route("1.0.0", ReleaseChannel::Rc, 1)
                .unwrap_err();
            assert!(
                matches!(error, NativeHostError::HostChannelUnsupported { .. }),
                "{host} should reject a candidate, got {error:?}"
            );
            // …but the stable track still works.
            assert!(host.channel_route("1.0.0", ReleaseChannel::Stable, 1).is_ok());
        }
    }

    #[test]
    fn stable_never_rewrites_the_version_or_demands_opt_in() {
        for host in NativeHost::ALL {
            let route = host.channel_route("3.1.4", ReleaseChannel::Stable, 1).unwrap();
            assert_eq!(route.version, "3.1.4", "{host} rewrote a stable version");
            assert!(!route.requires_opt_in);
            assert!(!route.mutable);
            assert!(
                host.channel_endpoints(ReleaseChannel::Stable).is_none(),
                "{host} moved the stable endpoint"
            );
        }
    }

    #[test]
    fn a_candidate_needs_a_real_iteration_and_a_real_base() {
        assert!(matches!(
            NativeHost::Npm.channel_route("1.0.0", ReleaseChannel::Rc, 0),
            Err(NativeHostError::ZeroIteration { .. })
        ));
        assert!(matches!(
            NativeHost::Npm.channel_route("  ", ReleaseChannel::Stable, 1),
            Err(NativeHostError::EmptyVersion { .. })
        ));
        // Iteration is irrelevant to a stable release, so 0 is fine there.
        assert!(NativeHost::Npm
            .channel_route("1.0.0", ReleaseChannel::Stable, 0)
            .is_ok());
    }

    #[test]
    fn vcs_published_hosts_never_ask_for_an_upload_credential() {
        // Prompting for a registry token that is never used puts an
        // unnecessary secret into CI scope.
        for host in [
            NativeHost::GoProxy,
            NativeHost::Packagist,
            NativeHost::Dub,
            NativeHost::Nimble,
            NativeHost::Shards,
            NativeHost::Racket,
            NativeHost::Zig,
            NativeHost::Vcs,
        ] {
            assert!(
                !host.protocol().uploads_artifact(),
                "{host} should publish by VCS tag"
            );
        }
        for host in [
            NativeHost::Npm,
            NativeHost::CratesIo,
            NativeHost::PyPi,
            NativeHost::Hex,
            NativeHost::NuGet,
        ] {
            assert!(host.protocol().uploads_artifact(), "{host} uploads bytes");
            assert!(host.endpoints().publish.is_some(), "{host} needs an upload URL");
        }
    }

    #[test]
    fn moderated_hosts_are_flagged_so_automation_stops_at_submitted() {
        for host in [NativeHost::Cran, NativeHost::JuliaGeneral, NativeHost::Opam] {
            let route = host.channel_route("1.0.0", ReleaseChannel::Stable, 1).unwrap();
            assert!(route.moderated, "{host} publication is reviewed");
        }
        assert!(
            !NativeHost::Npm
                .channel_route("1.0.0", ReleaseChannel::Stable, 1)
                .unwrap()
                .moderated
        );
    }

    #[test]
    fn read_only_hosts_declare_no_publish_endpoint() {
        for host in [
            NativeHost::Stackage,
            NativeHost::GoProxy,
            NativeHost::ConanCenter,
            NativeHost::JuliaGeneral,
            NativeHost::Opam,
        ] {
            assert!(
                host.endpoints().publish.is_none(),
                "{host} must not advertise an upload URL"
            );
        }
    }

    #[test]
    fn every_host_endpoint_is_https_or_deliberately_empty() {
        // An http:// default would silently downgrade a token-bearing upload.
        for host in NativeHost::ALL {
            let endpoints = host.endpoints();
            if matches!(host, NativeHost::Zig | NativeHost::Vcs) {
                assert!(endpoints.index.is_empty(), "{host} resolves from the VCS remote");
                continue;
            }
            assert!(
                endpoints.index.starts_with("https://"),
                "{host} index is not https"
            );
            for url in [endpoints.publish.as_deref(), endpoints.download.as_deref()]
                .into_iter()
                .flatten()
            {
                assert!(url.starts_with("https://"), "{host} url `{url}` is not https");
            }
        }
    }

    #[test]
    fn pypi_pins_the_basic_auth_username_registries_actually_require() {
        assert_eq!(NativeHost::PyPi.basic_auth_username(), Some("__token__"));
        assert_eq!(NativeHost::TestPyPi.basic_auth_username(), Some("__token__"));
        assert_eq!(NativeHost::MavenCentral.basic_auth_username(), None);
    }

    #[test]
    fn tokens_that_ride_in_the_url_are_marked_for_redaction() {
        // These two put the secret in the request line, so a logger that
        // prints URLs would leak it.
        for host in [NativeHost::LuaRocks, NativeHost::Packagist] {
            assert_eq!(host.publish_auth(), RegistryAuth::UrlEmbedded);
        }
        // And the schemes that look alike but are not interchangeable.
        assert_eq!(NativeHost::Npm.publish_auth(), RegistryAuth::Bearer);
        assert_eq!(NativeHost::CratesIo.publish_auth(), RegistryAuth::BareToken);
        assert_eq!(
            NativeHost::NuGet.publish_auth(),
            RegistryAuth::Header(ApiKeyHeader::XNuGetApiKey)
        );
        assert_eq!(
            ApiKeyHeader::AuthorizationToken.header_name(),
            "Authorization"
        );
        assert_eq!(ApiKeyHeader::AuthorizationToken.value_prefix(), "Token ");
    }

    #[test]
    fn one_protocol_serves_many_hosts() {
        // The property that keeps the client small: adding Clojars or an
        // enterprise mirror must not add a protocol implementation.
        assert_eq!(NativeHost::Clojars.protocol(), RegistryProtocol::Maven2);
        assert_eq!(NativeHost::PyPi.protocol(), NativeHost::TestPyPi.protocol());
        assert_eq!(
            NativeHost::Hackage.protocol(),
            NativeHost::Stackage.protocol()
        );
        for host in [
            NativeHost::Packagist,
            NativeHost::Dub,
            NativeHost::Nimble,
            NativeHost::Shards,
            NativeHost::Racket,
        ] {
            assert_eq!(host.protocol(), RegistryProtocol::VcsIndexed);
        }
    }

    #[test]
    fn universal_hosts_rewrite_the_url_and_keep_the_protocol() {
        let endpoints = UniversalHost::Artifactory
            .endpoints_for(NativeHost::Npm, "https://acme.jfrog.io/artifactory/npm-local/")
            .unwrap();
        assert_eq!(
            endpoints.publish.as_deref(),
            Some("https://acme.jfrog.io/artifactory/npm-local")
        );
        assert_eq!(endpoints.download_base(), "https://acme.jfrog.io/artifactory/npm-local");
    }

    #[test]
    fn universal_hosts_reject_formats_they_do_not_serve() {
        // GitHub Packages has no PyPI endpoint; promising one produces a plan
        // that cannot execute.
        let error = UniversalHost::GithubPackages
            .endpoints_for(NativeHost::PyPi, "https://npm.pkg.github.com")
            .unwrap_err();
        assert!(matches!(
            error,
            NativeHostError::UniversalHostUnsupported { .. }
        ));
        assert!(
            UniversalHost::GitlabPackages
                .endpoints_for(NativeHost::PyPi, "https://gitlab.com/api/v4/projects/1/packages/pypi")
                .is_ok(),
            "GitLab does serve PyPI"
        );
    }

    #[test]
    fn universal_host_bases_must_be_https() {
        let error = UniversalHost::Nexus
            .endpoints_for(NativeHost::Npm, "http://nexus.internal/repository/npm")
            .unwrap_err();
        assert!(matches!(error, NativeHostError::InsecureBase { .. }));
    }

    #[test]
    fn host_and_ecosystem_agree_on_the_same_target() {
        // `ecosystem()` is what the install guard checks; a mismatch here
        // would let a package publish somewhere its consumers cannot install
        // from.
        assert_eq!(NativeHost::Npm.ecosystem(), Ecosystem::Npm);
        assert_eq!(NativeHost::Clojars.ecosystem(), Ecosystem::Jvm);
        assert_eq!(NativeHost::MavenCentral.ecosystem(), Ecosystem::Jvm);
        assert_eq!(NativeHost::TestPyPi.ecosystem(), Ecosystem::Pypi);
        assert_eq!(NativeHost::Nimble.ecosystem(), Ecosystem::Nimble);
        assert_eq!(NativeHost::Zig.ecosystem(), Ecosystem::Zig);
    }

    #[test]
    fn channel_tokens_accept_the_words_release_engineers_use() {
        assert_eq!(
            ReleaseChannel::from_token("release-candidate"),
            Some(ReleaseChannel::Rc)
        );
        assert_eq!(ReleaseChannel::from_token("canary"), Some(ReleaseChannel::Nightly));
        assert_eq!(ReleaseChannel::from_token("latest"), Some(ReleaseChannel::Stable));
        // Publish-time input must not degrade: an unknown track is an error,
        // not a silent stable release.
        assert_eq!(ReleaseChannel::from_token("gamma"), None);
        assert_eq!(ReleaseChannel::from_str_lenient("gamma"), ReleaseChannel::Stable);
    }
}
