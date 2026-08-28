//! GitHub, GitHub Packages (GHCR), and public R2 artifact locators.
//!
//! `registry.zpkg.net` is the primary metadata and download host. When it is
//! unreachable, clients reconstruct fetch URLs from a package's GitHub identity
//! and, for public artifacts, from `https://cdn.zpkg.net` (Cloudflare R2 custom
//! domain — independent of the registry origin). The guessed layout is
//! deterministic so a publisher does not have to declare it.
//! `[package.artifacts]` overrides the guess when the object lives at a
//! non-standard key.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::artifact::ArtifactFormat;
use crate::paths::MANIFEST_FILE;

/// Cloudflare-proxied public origin for the production R2 bucket
/// (`zed-pkg-artifacts`). This hostname is an R2 custom domain: it is served
/// at the Cloudflare edge and does not go through `registry.zpkg.net`,
/// `web.zpkg.net`, or the GitHub Pages apex. When those origins are down,
/// clients still GET objects from `https://cdn.zpkg.net/<key>`.
pub const DEFAULT_R2_PUBLIC_BASE: &str = "https://cdn.zpkg.net";

/// GitHub REST API origin used when the registry cannot list versions.
pub const DEFAULT_GITHUB_API: &str = "https://api.github.com";

/// GitHub web origin used for release assets, archives, and raw files.
pub const DEFAULT_GITHUB_WEB: &str = "https://github.com";

/// GitHub Container Registry origin. Packed Zed artifacts published here
/// appear on `https://github.com/orgs/{owner}/packages` as container packages
/// until GitHub hosts a native Zed registry on that page.
pub const DEFAULT_GHCR: &str = "https://ghcr.io";

/// Content-addressed prefix inside the R2 bucket (`artifacts/<sha256>.<ext>`).
pub const R2_CONTENT_PREFIX: &str = "artifacts";

/// Guessable prefix from a Zed identity (`packages/<org>/<name>/<version>/…`).
pub const R2_PACKAGE_PREFIX: &str = "packages";

/// Guessable prefix from a GitHub identity (`github/<owner>/<repo>/<tag>/…`).
pub const R2_GITHUB_PREFIX: &str = "github";

/// Where a published artifact can be fetched from when the primary registry
/// host is down. Order in [`artifact_locators`] is the client retry order.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum ArtifactSourceKind {
    /// `zed-api-server` (`/v1/artifacts/<sha256>` or a presigned redirect).
    Registry,
    /// Direct GET against the public R2/CDN origin.
    R2,
    /// Packed tarball attached to a GitHub Release.
    GithubRelease,
    /// Packed artifact stored as an OCI artifact on GitHub Packages (GHCR).
    /// This is the GitHub surface that shows on
    /// `https://github.com/orgs/{org}/packages`.
    GithubPackages,
    /// Source archive of the tagged commit. Last resort: the bytes are the
    /// VCS tree, not the pruned publish artifact, so a lockfile sha256 will
    /// not match unless the client re-packs with the same rules.
    GithubArchive,
}

impl ArtifactSourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Registry => "registry",
            Self::R2 => "r2",
            Self::GithubRelease => "github-release",
            Self::GithubPackages => "github-packages",
            Self::GithubArchive => "github-archive",
        }
    }
}

/// One concrete fetch location for a published artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ArtifactLocator {
    pub kind: ArtifactSourceKind,
    pub url: String,
}

/// Optional `[package.artifacts]` table. Omit the whole table and clients
/// guess GitHub + R2 paths from `[package.repository]` and `org`/`name`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ArtifactsSection {
    /// Public HTTPS origin for this package's R2/CDN objects. When omitted,
    /// clients use `ZED_PKG_R2_PUBLIC_BASE`, then `ZED_PKG_R2_PUBLIC_KEY`,
    /// then [`DEFAULT_R2_PUBLIC_BASE`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r2_public_base: Option<String>,
    /// Full object key, possibly with `{org}`, `{name}`, `{version}`, `{tag}`,
    /// `{sha256}`, `{ext}`, `{github_owner}`, `{github_repo}` placeholders.
    /// Use this when the artifact is not at a standard guessed path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r2_key: Option<String>,
    /// Directory prefix in the bucket; the file name
    /// `{name}-{version}.{ext}` is appended. Ignored when `r2_key` is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r2_prefix: Option<String>,
    /// Attach (and later fetch) a packed GitHub Release asset. Defaults to
    /// true when `repository.url` is a github.com repo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github_release: Option<bool>,
    /// Push (and later fetch) a packed OCI artifact on GitHub Packages / GHCR.
    /// Defaults to true when `repository.url` is a github.com repo. This is
    /// what makes a Zed package show up on github.com/orgs/{org}/packages
    /// until GitHub hosts a native Zed registry there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github_packages: Option<bool>,
}

impl Default for ArtifactsSection {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl ArtifactsSection {
    pub const EMPTY: Self = Self {
        r2_public_base: None,
        r2_key: None,
        r2_prefix: None,
        github_release: None,
        github_packages: None,
    };

    pub fn is_empty(&self) -> bool {
        self.r2_public_base.is_none()
            && self.r2_key.is_none()
            && self.r2_prefix.is_none()
            && self.github_release.is_none()
            && self.github_packages.is_none()
    }

    pub fn github_release_enabled(&self, repo_url: Option<&str>) -> bool {
        match self.github_release {
            Some(value) => value,
            None => repo_url.is_some_and(|url| parse_github_identity(url).is_some()),
        }
    }

    pub fn github_packages_enabled(&self, repo_url: Option<&str>) -> bool {
        match self.github_packages {
            Some(value) => value,
            None => repo_url.is_some_and(|url| parse_github_identity(url).is_some()),
        }
    }
}

/// Inputs needed to guess every fallback location for one published version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtifactQuery<'a> {
    pub org: &'a str,
    pub name: &'a str,
    pub version: &'a str,
    pub vcs_tag: &'a str,
    pub sha256: Option<&'a str>,
    pub format: ArtifactFormat,
    pub repo_url: Option<&'a str>,
    pub artifacts: Option<&'a ArtifactsSection>,
    pub registry_base: Option<&'a str>,
    pub r2_public_base: Option<&'a str>,
    pub r2_public_key: Option<&'a str>,
}

/// Owner/repo pair extracted from a GitHub remote URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubIdentity {
    pub owner: String,
    pub repo: String,
}

impl GithubIdentity {
    pub fn guessed_from_package(org: &str, name: &str) -> Self {
        Self {
            owner: org.to_string(),
            repo: name.to_string(),
        }
    }

    pub fn web_url(&self) -> String {
        format!("{DEFAULT_GITHUB_WEB}/{}/{}", self.owner, self.repo)
    }
}

/// Parse a GitHub owner/repo out of common remote spellings. Returns `None`
/// for non-GitHub hosts and for owner/repo segments that could traverse.
pub fn parse_github_identity(url: &str) -> Option<GithubIdentity> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut normalized = trimmed.replace('\\', "/");
    if let Some(rest) = normalized.strip_prefix("git+") {
        normalized = rest.to_string();
    }
    let rest = if let Some(rest) = strip_ignore_ascii(&normalized, "git@github.com:") {
        rest
    } else if let Some(rest) = strip_ignore_ascii(&normalized, "ssh://git@github.com/") {
        rest
    } else if let Some(rest) = strip_ignore_ascii(&normalized, "https://github.com/") {
        rest
    } else if let Some(rest) = strip_ignore_ascii(&normalized, "http://github.com/") {
        rest
    } else if let Some(rest) = strip_ignore_ascii(&normalized, "git://github.com/") {
        rest
    } else if let Some(rest) = strip_ignore_ascii(&normalized, "ssh://github.com/") {
        rest
    } else {
        return None;
    };
    let rest = rest.trim_end_matches('/').trim_end_matches(".git");
    let mut parts = rest.split('/');
    let owner = parts.next()?;
    let repo = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    if !valid_github_segment(owner) || !valid_github_segment(repo) {
        return None;
    }
    Some(GithubIdentity {
        owner: owner.to_string(),
        repo: repo.to_string(),
    })
}

/// GitHub identity for a package: the declared repository URL when it is a
/// github.com remote, otherwise the conventional `github.com/{org}/{name}`.
pub fn github_identity_for(org: &str, name: &str, repo_url: Option<&str>) -> GithubIdentity {
    repo_url
        .and_then(parse_github_identity)
        .unwrap_or_else(|| GithubIdentity::guessed_from_package(org, name))
}

/// Resolve the public R2/CDN origin.
///
/// `ZED_PKG_R2_PUBLIC_KEY` is the public bucket identifier: a full `https://`
/// origin, a hostname, or a Cloudflare `pub-<id>` id served at
/// `https://<id>.r2.dev`.
pub fn resolve_r2_public_base(
    declared: Option<&str>,
    env_base: Option<&str>,
    env_key: Option<&str>,
) -> String {
    if let Some(base) = first_non_empty(declared) {
        return trim_slash(base);
    }
    if let Some(base) = first_non_empty(env_base) {
        return trim_slash(base);
    }
    if let Some(key) = first_non_empty(env_key) {
        return origin_from_public_key(key);
    }
    DEFAULT_R2_PUBLIC_BASE.to_string()
}

/// Standard object keys inside the R2 bucket, declared key first.
pub fn r2_object_keys(query: &ArtifactQuery<'_>) -> Vec<String> {
    let ext = query.format.extension();
    let github = query
        .repo_url
        .and_then(parse_github_identity)
        .unwrap_or_else(|| GithubIdentity::guessed_from_package(query.org, query.name));
    let artifacts = query.artifacts.unwrap_or(&ArtifactsSection::EMPTY);
    let mut keys = Vec::new();

    if let Some(template) = artifacts
        .r2_key
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        keys.push(expand_key_template(template, query, &github, ext));
    } else if let Some(prefix) = artifacts
        .r2_prefix
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let prefix = prefix.trim_matches('/');
        keys.push(format!("{prefix}/{}-{}.{}", query.name, query.version, ext));
    }

    keys.push(format!(
        "{R2_GITHUB_PREFIX}/{}/{}/{}/{}-{}.{}",
        github.owner, github.repo, query.vcs_tag, query.name, query.version, ext
    ));
    keys.push(format!(
        "{R2_PACKAGE_PREFIX}/{}/{}/{}/{}-{}.{}",
        query.org, query.name, query.version, query.name, query.version, ext
    ));
    if let Some(sha256) = query.sha256.filter(|digest| !digest.is_empty()) {
        keys.push(format!("{R2_CONTENT_PREFIX}/{sha256}.{ext}"));
    }
    keys
}

/// GitHub Release asset file names, longest (org-qualified) first.
pub fn github_release_asset_names(org: &str, name: &str, version: &str, ext: &str) -> Vec<String> {
    let mut names = vec![format!("zpkg-{org}-{name}-{version}.{ext}")];
    let short = format!("zpkg-{name}-{version}.{ext}");
    if !names.contains(&short) {
        names.push(short);
    }
    names
}

/// Sidecar JSON next to a GitHub Release artifact (VersionMetadata).
pub fn github_release_sidecar_names(org: &str, name: &str, version: &str) -> Vec<String> {
    let mut names = vec![format!("zpkg-{org}-{name}-{version}.json")];
    let short = format!("zpkg-{name}-{version}.json");
    if !names.contains(&short) {
        names.push(short);
    }
    names
}

pub fn github_release_download_url(identity: &GithubIdentity, tag: &str, asset: &str) -> String {
    format!(
        "{DEFAULT_GITHUB_WEB}/{}/{}/releases/download/{tag}/{asset}",
        identity.owner, identity.repo
    )
}

pub fn github_archive_url(identity: &GithubIdentity, tag: &str) -> String {
    format!(
        "{DEFAULT_GITHUB_WEB}/{}/{}/archive/refs/tags/{tag}.tar.gz",
        identity.owner, identity.repo
    )
}

pub fn github_archive_commit_url(identity: &GithubIdentity, commit: &str) -> String {
    format!(
        "{DEFAULT_GITHUB_WEB}/{}/{}/archive/{commit}.tar.gz",
        identity.owner, identity.repo
    )
}

pub fn github_raw_manifest_url(identity: &GithubIdentity, git_ref: &str) -> String {
    format!(
        "https://raw.githubusercontent.com/{}/{}/{git_ref}/{MANIFEST_FILE}",
        identity.owner, identity.repo
    )
}

pub fn github_api_repo_url(identity: &GithubIdentity) -> String {
    format!(
        "{DEFAULT_GITHUB_API}/repos/{}/{}",
        identity.owner, identity.repo
    )
}

pub fn github_api_tags_url(identity: &GithubIdentity) -> String {
    format!("{}/tags?per_page=100", github_api_repo_url(identity))
}

pub fn github_api_release_url(identity: &GithubIdentity, tag: &str) -> String {
    format!("{}/releases/tags/{tag}", github_api_repo_url(identity))
}

pub fn github_api_contents_manifest_url(identity: &GithubIdentity, git_ref: &str) -> String {
    format!(
        "{}/contents/{MANIFEST_FILE}?ref={git_ref}",
        github_api_repo_url(identity)
    )
}

pub fn github_api_git_refs_url(identity: &GithubIdentity) -> String {
    format!("{}/git/refs", github_api_repo_url(identity))
}

pub fn github_api_git_ref_url(identity: &GithubIdentity, ref_name: &str) -> String {
    format!("{}/git/ref/{ref_name}", github_api_repo_url(identity))
}

pub fn github_api_git_tag_url(identity: &GithubIdentity, tag: &str) -> String {
    github_api_git_ref_url(identity, &format!("tags/{tag}"))
}

/// Lowercase `owner/repo` path GitHub Container Registry requires.
pub fn ghcr_repository(identity: &GithubIdentity) -> String {
    format!(
        "{}/{}",
        identity.owner.to_ascii_lowercase(),
        identity.repo.to_ascii_lowercase()
    )
}

/// `ghcr.io/{owner}/{repo}:{tag}` — the GitHub Packages container identity.
pub fn ghcr_reference(identity: &GithubIdentity, tag: &str) -> String {
    format!("ghcr.io/{}:{tag}", ghcr_repository(identity))
}

pub fn ghcr_manifest_url(identity: &GithubIdentity, tag: &str) -> String {
    format!(
        "{DEFAULT_GHCR}/v2/{}/manifests/{tag}",
        ghcr_repository(identity)
    )
}

pub fn ghcr_blob_url(identity: &GithubIdentity, digest: &str) -> String {
    format!(
        "{DEFAULT_GHCR}/v2/{}/blobs/{digest}",
        ghcr_repository(identity)
    )
}

pub fn ghcr_uploads_url(identity: &GithubIdentity) -> String {
    format!(
        "{DEFAULT_GHCR}/v2/{}/blobs/uploads/",
        ghcr_repository(identity)
    )
}

/// Org packages page for a GHCR container. User-owned packages use
/// [`github_packages_user_web_url`].
pub fn github_packages_web_url(identity: &GithubIdentity) -> String {
    format!(
        "{DEFAULT_GITHUB_WEB}/orgs/{}/packages/container/{}",
        identity.owner,
        identity.repo.to_ascii_lowercase()
    )
}

pub fn github_packages_user_web_url(identity: &GithubIdentity) -> String {
    format!(
        "{DEFAULT_GITHUB_WEB}/users/{}/packages/container/{}",
        identity.owner,
        identity.repo.to_ascii_lowercase()
    )
}

pub fn github_api_org_package_url(identity: &GithubIdentity) -> String {
    format!(
        "{DEFAULT_GITHUB_API}/orgs/{}/packages/container/{}",
        identity.owner,
        identity.repo.to_ascii_lowercase()
    )
}

pub fn github_api_user_package_url(identity: &GithubIdentity) -> String {
    format!(
        "{DEFAULT_GITHUB_API}/users/{}/packages/container/{}",
        identity.owner,
        identity.repo.to_ascii_lowercase()
    )
}

/// Default Zed tag (`v{version}`) plus the bare version, so a repo that tags
/// `1.2.0` instead of `v1.2.0` still resolves.
pub fn git_tags_for_version(version: &str) -> Vec<String> {
    let version = version.trim();
    if version.is_empty() {
        return Vec::new();
    }
    let mut tags = Vec::new();
    if !version.starts_with('v') {
        tags.push(format!("v{version}"));
    }
    tags.push(version.to_string());
    tags
}

/// Inverse of [`git_tags_for_version`]: `v1.2.0` and `1.2.0` both become `1.2.0`.
pub fn version_from_git_tag(tag: &str) -> Option<String> {
    let name = tag.trim();
    if name.is_empty() {
        return None;
    }
    let version = name.strip_prefix('v').unwrap_or(name);
    if version.is_empty() {
        None
    } else {
        Some(version.to_string())
    }
}

/// Every HTTP location a client should try, registry first, then public R2,
/// then GitHub Release assets, then GitHub Packages (GHCR), then the source archive.
pub fn artifact_locators(query: &ArtifactQuery<'_>) -> Vec<ArtifactLocator> {
    let mut locators = Vec::new();
    let ext = query.format.extension();
    let identity = github_identity_for(query.org, query.name, query.repo_url);
    let artifacts = query.artifacts.unwrap_or(&ArtifactsSection::EMPTY);

    if let Some(base) = query.registry_base.map(str::trim).filter(|s| !s.is_empty())
        && let Some(sha256) = query.sha256.filter(|digest| !digest.is_empty())
    {
        locators.push(ArtifactLocator {
            kind: ArtifactSourceKind::Registry,
            url: format!(
                "{}{}",
                trim_slash(base),
                crate::registry::artifact_path(sha256)
            ),
        });
    }

    let r2_base = resolve_r2_public_base(
        artifacts.r2_public_base.as_deref(),
        query.r2_public_base,
        query.r2_public_key,
    );
    for key in r2_object_keys(query) {
        locators.push(ArtifactLocator {
            kind: ArtifactSourceKind::R2,
            url: format!("{r2_base}/{key}"),
        });
    }

    if artifacts.github_release_enabled(query.repo_url) {
        for asset in github_release_asset_names(query.org, query.name, query.version, ext) {
            locators.push(ArtifactLocator {
                kind: ArtifactSourceKind::GithubRelease,
                url: github_release_download_url(&identity, query.vcs_tag, &asset),
            });
        }
    }

    if artifacts.github_packages_enabled(query.repo_url) {
        locators.push(ArtifactLocator {
            kind: ArtifactSourceKind::GithubPackages,
            url: ghcr_manifest_url(&identity, query.vcs_tag),
        });
        if let Some(sha256) = query.sha256.filter(|digest| !digest.is_empty()) {
            locators.push(ArtifactLocator {
                kind: ArtifactSourceKind::GithubPackages,
                url: ghcr_blob_url(&identity, &format!("sha256:{sha256}")),
            });
        }
    }

    if !query.vcs_tag.is_empty() {
        locators.push(ArtifactLocator {
            kind: ArtifactSourceKind::GithubArchive,
            url: github_archive_url(&identity, query.vcs_tag),
        });
    }

    locators
}

/// Reject `[package.artifacts]` values that could escape the bucket or
/// point at a non-HTTPS origin in production spelling.
pub fn validate_artifacts_section(section: &ArtifactsSection) -> Result<(), String> {
    if let Some(base) = section.r2_public_base.as_deref() {
        validate_public_base(base)?;
    }
    if let Some(key) = section.r2_key.as_deref() {
        validate_key_template("r2_key", key)?;
    }
    if let Some(prefix) = section.r2_prefix.as_deref() {
        validate_key_template("r2_prefix", prefix)?;
    }
    Ok(())
}

fn validate_public_base(base: &str) -> Result<(), String> {
    let base = base.trim();
    if base.is_empty() {
        return Err("r2_public_base must not be empty".to_string());
    }
    let https = base.starts_with("https://");
    let http_loopback = base.starts_with("http://127.0.0.1")
        || base.starts_with("http://localhost")
        || base.starts_with("http://[::1]");
    if !https && !http_loopback {
        return Err(
            "r2_public_base must be an https:// origin (http is allowed only for loopback)"
                .to_string(),
        );
    }
    if base.contains("..") || base.contains('\\') {
        return Err("r2_public_base must not contain `..` or backslashes".to_string());
    }
    Ok(())
}

fn validate_key_template(field: &str, template: &str) -> Result<(), String> {
    let template = template.trim();
    if template.is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    if template.starts_with('/') || template.starts_with('\\') {
        return Err(format!(
            "{field} must be a bucket-relative key, not an absolute path"
        ));
    }
    let without_placeholders = template
        .replace("{org}", "x")
        .replace("{name}", "x")
        .replace("{version}", "x")
        .replace("{tag}", "x")
        .replace("{sha256}", "x")
        .replace("{ext}", "x")
        .replace("{github_owner}", "x")
        .replace("{github_repo}", "x");
    if without_placeholders.contains('{') || without_placeholders.contains('}') {
        return Err(format!(
            "{field} has an unknown placeholder; expected {{org}}, {{name}}, {{version}}, {{tag}}, {{sha256}}, {{ext}}, {{github_owner}}, {{github_repo}}"
        ));
    }
    if without_placeholders
        .split(['/', '\\'])
        .any(|seg| seg == ".." || seg.is_empty())
    {
        return Err(format!(
            "{field} must not contain empty or `..` path segments"
        ));
    }
    Ok(())
}

fn expand_key_template(
    template: &str,
    query: &ArtifactQuery<'_>,
    github: &GithubIdentity,
    ext: &str,
) -> String {
    template
        .replace("{org}", query.org)
        .replace("{name}", query.name)
        .replace("{version}", query.version)
        .replace("{tag}", query.vcs_tag)
        .replace("{sha256}", query.sha256.unwrap_or(""))
        .replace("{ext}", ext)
        .replace("{github_owner}", &github.owner)
        .replace("{github_repo}", &github.repo)
        .trim_matches('/')
        .to_string()
}

fn origin_from_public_key(key: &str) -> String {
    let key = key.trim();
    if key.starts_with("https://") || key.starts_with("http://") {
        return trim_slash(key);
    }
    if key.contains('.') {
        return format!("https://{key}");
    }
    format!("https://{key}.r2.dev")
}

fn first_non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|s| !s.is_empty())
}

fn trim_slash(value: &str) -> String {
    value.trim().trim_end_matches('/').to_string()
}

fn strip_ignore_ascii<'a>(input: &'a str, prefix: &str) -> Option<&'a str> {
    let (head, tail) = input.split_at_checked(prefix.len())?;
    if head.eq_ignore_ascii_case(prefix) {
        Some(tail)
    } else {
        None
    }
}

fn valid_github_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment != "."
        && segment != ".."
        && !segment.starts_with('.')
        && segment
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        && !segment.trim_matches('.').is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query<'a>(repo: &'a str, artifacts: Option<&'a ArtifactsSection>) -> ArtifactQuery<'a> {
        ArtifactQuery {
            org: "acme",
            name: "http-kit",
            version: "1.2.0",
            vcs_tag: "v1.2.0",
            sha256: None,
            format: ArtifactFormat::TarGz,
            repo_url: Some(repo),
            artifacts,
            registry_base: Some("https://registry.zpkg.net"),
            r2_public_base: None,
            r2_public_key: None,
        }
    }

    #[test]
    fn github_identity_parses_common_remotes() {
        let expected = GithubIdentity {
            owner: "acme".into(),
            repo: "http-kit".into(),
        };
        for url in [
            "https://github.com/acme/http-kit",
            "https://github.com/acme/http-kit.git",
            "git@github.com:acme/http-kit.git",
            "ssh://git@github.com/acme/http-kit.git",
            "git+https://github.com/acme/http-kit",
        ] {
            assert_eq!(
                parse_github_identity(url).as_ref(),
                Some(&expected),
                "{url}"
            );
        }
        assert_eq!(
            parse_github_identity("https://gitlab.com/acme/http-kit"),
            None
        );
        assert_eq!(
            parse_github_identity("https://github.com/../evil/foo"),
            None
        );
        assert_eq!(parse_github_identity("https://github.com/acme"), None);
    }

    #[test]
    fn guessed_keys_cover_github_package_and_content_layouts() {
        let sha = "ab".repeat(32);
        let q = ArtifactQuery {
            org: "acme",
            name: "http-kit",
            version: "1.2.0",
            vcs_tag: "v1.2.0",
            sha256: Some(sha.as_str()),
            format: ArtifactFormat::TarGz,
            repo_url: Some("https://github.com/acme/http-kit"),
            artifacts: None,
            registry_base: None,
            r2_public_base: None,
            r2_public_key: None,
        };
        let keys = r2_object_keys(&q);
        assert_eq!(
            keys,
            vec![
                "github/acme/http-kit/v1.2.0/http-kit-1.2.0.tar.gz".to_string(),
                "packages/acme/http-kit/1.2.0/http-kit-1.2.0.tar.gz".to_string(),
                format!("artifacts/{sha}.tar.gz"),
            ]
        );
    }

    #[test]
    fn declared_r2_key_wins_and_expands_placeholders() {
        let artifacts = ArtifactsSection {
            r2_key: Some("vendor/{github_owner}/{name}/{version}/pkg.{ext}".into()),
            ..ArtifactsSection::EMPTY
        };
        let sha = "ab".repeat(32);
        let q = ArtifactQuery {
            sha256: Some(sha.as_str()),
            artifacts: Some(&artifacts),
            ..query("https://github.com/acme/http-kit", Some(&artifacts))
        };
        assert_eq!(
            r2_object_keys(&q)[0],
            "vendor/acme/http-kit/1.2.0/pkg.tar.gz"
        );
    }

    #[test]
    fn public_key_becomes_r2_dev_origin() {
        assert_eq!(
            resolve_r2_public_base(None, None, Some("pub-abc123")),
            "https://pub-abc123.r2.dev"
        );
        assert_eq!(
            resolve_r2_public_base(None, None, Some("cdn.example.test")),
            "https://cdn.example.test"
        );
        assert_eq!(
            resolve_r2_public_base(None, Some("https://cdn.example.test/"), None),
            "https://cdn.example.test"
        );
        assert_eq!(
            resolve_r2_public_base(Some("https://mine.example/"), None, None),
            "https://mine.example"
        );
    }

    #[test]
    fn locators_try_registry_then_r2_then_github() {
        let sha = "ab".repeat(32);
        let q = ArtifactQuery {
            sha256: Some(sha.as_str()),
            ..query("https://github.com/acme/http-kit", None)
        };
        let urls: Vec<_> = artifact_locators(&q)
            .into_iter()
            .map(|locator| (locator.kind, locator.url))
            .collect();
        assert_eq!(urls[0].0, ArtifactSourceKind::Registry);
        assert!(urls[0].1.ends_with(&format!("/v1/artifacts/{sha}")));
        assert!(urls.iter().any(|(kind, url)| {
            *kind == ArtifactSourceKind::R2
                && url == "https://cdn.zpkg.net/github/acme/http-kit/v1.2.0/http-kit-1.2.0.tar.gz"
        }));
        assert!(urls.iter().any(|(kind, url)| {
            *kind == ArtifactSourceKind::GithubRelease
                && url.ends_with("/releases/download/v1.2.0/zpkg-acme-http-kit-1.2.0.tar.gz")
        }));
        assert!(urls.iter().any(|(kind, url)| {
            *kind == ArtifactSourceKind::GithubPackages
                && url == "https://ghcr.io/v2/acme/http-kit/manifests/v1.2.0"
        }));
        assert!(urls.iter().any(|(kind, url)| {
            *kind == ArtifactSourceKind::GithubPackages
                && *url == format!("https://ghcr.io/v2/acme/http-kit/blobs/sha256:{sha}")
        }));
        assert_eq!(
            urls.last().map(|row| row.0),
            Some(ArtifactSourceKind::GithubArchive)
        );
    }

    #[test]
    fn git_tags_round_trip_with_and_without_v_prefix() {
        assert_eq!(
            git_tags_for_version("1.2.0"),
            vec!["v1.2.0".to_string(), "1.2.0".to_string()]
        );
        assert_eq!(git_tags_for_version("v1.2.0"), vec!["v1.2.0".to_string()]);
        assert_eq!(version_from_git_tag("v1.2.0").as_deref(), Some("1.2.0"));
        assert_eq!(version_from_git_tag("1.2.0").as_deref(), Some("1.2.0"));
        assert_eq!(version_from_git_tag("  ").as_deref(), None);
    }

    #[test]
    fn ghcr_identity_is_lowercase_and_shows_on_org_packages_page() {
        let identity = GithubIdentity {
            owner: "Cliptown".into(),
            repo: "HTTP-Kit".into(),
        };
        assert_eq!(ghcr_repository(&identity), "cliptown/http-kit");
        assert_eq!(ghcr_reference(&identity, "v1.2.0"), "ghcr.io/cliptown/http-kit:v1.2.0");
        assert_eq!(
            github_packages_web_url(&identity),
            "https://github.com/orgs/Cliptown/packages/container/http-kit"
        );
        assert_eq!(
            ghcr_manifest_url(&identity, "v1.2.0"),
            "https://ghcr.io/v2/cliptown/http-kit/manifests/v1.2.0"
        );
    }

    #[test]
    fn artifacts_section_rejects_escapes_and_unknown_placeholders() {
        let bad_key = ArtifactsSection {
            r2_key: Some("../etc/passwd".into()),
            ..ArtifactsSection::EMPTY
        };
        assert!(validate_artifacts_section(&bad_key).is_err());
        let bad_placeholder = ArtifactsSection {
            r2_key: Some("x/{nope}/y".into()),
            ..ArtifactsSection::EMPTY
        };
        assert!(validate_artifacts_section(&bad_placeholder).is_err());
        let ok = ArtifactsSection {
            r2_public_base: Some("https://cdn.zpkg.net".into()),
            r2_key: Some("custom/{org}/{name}/{version}.{ext}".into()),
            ..ArtifactsSection::EMPTY
        };
        assert!(validate_artifacts_section(&ok).is_ok());
    }
}
