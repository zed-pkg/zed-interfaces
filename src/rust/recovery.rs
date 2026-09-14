//! Shared artifact recovery ordering after the configured registry path fails.
//!
//! `source::artifact_locators` inventories every known physical location and
//! preserves its historical order for compatibility. Recovery semantics are a
//! separate cross-client contract: when the normal registry request has already
//! failed, GitHub-owned sources are tried before Zed-operated R2/Cloudflare.
//! This keeps public restore available during a total Zed control-plane outage.

use crate::source::{ArtifactLocator, ArtifactQuery, ArtifactSourceKind, artifact_locators};

/// Return the retry chain for read-only artifact recovery after a registry
/// failure.
///
/// The order is intentionally:
///
/// 1. GitHub Release assets;
/// 2. GitHub Packages / GHCR;
/// 3. deterministic reconstruction from the GitHub tag archive;
/// 4. optional public R2/CDN mirrors.
///
/// Registry locators are removed because callers reach this function only
/// after that primary path has failed. Sorting is stable, so alternate Release
/// asset names and R2 object keys retain their deterministic relative order.
pub fn artifact_recovery_locators(query: &ArtifactQuery<'_>) -> Vec<ArtifactLocator> {
    let mut locators = artifact_locators(query);
    locators.retain(|locator| locator.kind != ArtifactSourceKind::Registry);
    locators.sort_by_key(|locator| recovery_priority(locator.kind));
    locators
}

fn recovery_priority(kind: ArtifactSourceKind) -> u8 {
    match kind {
        ArtifactSourceKind::GithubRelease => 0,
        ArtifactSourceKind::GithubPackages => 1,
        ArtifactSourceKind::GithubArchive => 2,
        ArtifactSourceKind::R2 => 3,
        ArtifactSourceKind::Registry => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::ArtifactFormat;
    use crate::source::ArtifactsSection;

    fn query<'a>(artifacts: &'a ArtifactsSection) -> ArtifactQuery<'a> {
        ArtifactQuery {
            org: "acme",
            name: "widget",
            version: "1.2.3",
            vcs_tag: "v1.2.3",
            sha256: Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            format: ArtifactFormat::TarGz,
            repo_url: Some("https://github.com/acme/widget"),
            artifacts: Some(artifacts),
            registry_base: Some("https://registry.zpkg.net"),
            r2_public_base: Some("https://cdn.zpkg.net"),
            r2_public_key: None,
        }
    }

    #[test]
    fn total_control_plane_recovery_is_github_first_and_r2_last() {
        let artifacts = ArtifactsSection::EMPTY;
        let locators = artifact_recovery_locators(&query(&artifacts));
        assert!(!locators.is_empty());
        assert!(
            locators
                .iter()
                .all(|locator| locator.kind != ArtifactSourceKind::Registry)
        );

        let kinds = locators
            .iter()
            .map(|locator| locator.kind)
            .collect::<Vec<_>>();
        let first_release = kinds
            .iter()
            .position(|kind| *kind == ArtifactSourceKind::GithubRelease)
            .unwrap();
        let first_packages = kinds
            .iter()
            .position(|kind| *kind == ArtifactSourceKind::GithubPackages)
            .unwrap();
        let first_archive = kinds
            .iter()
            .position(|kind| *kind == ArtifactSourceKind::GithubArchive)
            .unwrap();
        let first_r2 = kinds
            .iter()
            .position(|kind| *kind == ArtifactSourceKind::R2)
            .unwrap();

        assert!(first_release < first_packages);
        assert!(first_packages < first_archive);
        assert!(first_archive < first_r2);
        assert!(
            kinds[first_r2..]
                .iter()
                .all(|kind| *kind == ArtifactSourceKind::R2),
            "no Zed-owned R2 locator may precede or interrupt the GitHub recovery chain"
        );
    }

    #[test]
    fn disabled_github_surfaces_are_not_reintroduced_by_recovery_policy() {
        let artifacts = ArtifactsSection {
            github_release: Some(false),
            github_packages: Some(false),
            ..ArtifactsSection::EMPTY
        };
        let kinds = artifact_recovery_locators(&query(&artifacts))
            .into_iter()
            .map(|locator| locator.kind)
            .collect::<Vec<_>>();

        assert!(!kinds.contains(&ArtifactSourceKind::GithubRelease));
        assert!(!kinds.contains(&ArtifactSourceKind::GithubPackages));
        assert!(kinds.contains(&ArtifactSourceKind::GithubArchive));
        assert!(kinds.contains(&ArtifactSourceKind::R2));
    }
}
