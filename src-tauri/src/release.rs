use semver::Version;
use serde::Deserialize;
use url::Url;

use crate::{
    error::{AppError, AppResult},
    models::{AvailableRelease, ProxyProfile, ReleaseCheck},
    network::Network,
};

const MAX_RELEASE_RESPONSE_BYTES: usize = 512 * 1024;
const RELEASE_REPOSITORY: Option<&str> = option_env!("VEILFEED_RELEASE_REPOSITORY");

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    name: Option<String>,
    html_url: String,
    published_at: Option<String>,
    draft: bool,
    prerelease: bool,
}

pub async fn check(network: &Network, profile: Option<&ProxyProfile>) -> AppResult<ReleaseCheck> {
    let repository = RELEASE_REPOSITORY.ok_or_else(|| {
        AppError::ReleaseUnavailable(
            "this build does not identify a GitHub release repository".into(),
        )
    })?;
    check_repository(network, profile, repository).await
}

async fn check_repository(
    network: &Network,
    profile: Option<&ProxyProfile>,
    repository: &str,
) -> AppResult<ReleaseCheck> {
    validate_repository(repository)?;
    let url = format!("https://api.github.com/repos/{repository}/releases/latest");
    let body = network
        .download_text_limited(&url, profile, MAX_RELEASE_RESPONSE_BYTES)
        .await?;
    parse_release(&body, env!("CARGO_PKG_VERSION"), repository)
}

fn parse_release(body: &str, current: &str, repository: &str) -> AppResult<ReleaseCheck> {
    if body.len() > MAX_RELEASE_RESPONSE_BYTES {
        return Err(AppError::ContentTooLarge);
    }
    validate_repository(repository)?;
    let release: GitHubRelease =
        serde_json::from_str(body).map_err(|error| AppError::InvalidRelease(error.to_string()))?;
    if release.draft || release.prerelease {
        return Err(AppError::InvalidRelease(
            "latest release must be published and stable".into(),
        ));
    }

    let version_text = release
        .tag_name
        .strip_prefix('v')
        .unwrap_or(&release.tag_name);
    let latest = Version::parse(version_text)
        .map_err(|error| AppError::InvalidRelease(error.to_string()))?;
    if !latest.pre.is_empty()
        || !latest.build.is_empty()
        || release.tag_name != format!("v{latest}")
    {
        return Err(AppError::InvalidRelease(
            "release tag must be stable SemVer in the form vX.Y.Z".into(),
        ));
    }
    validate_release_url(&release.html_url, repository, &release.tag_name)?;

    let current_version = Version::parse(current)
        .map_err(|error| AppError::InvalidRelease(format!("invalid app version: {error}")))?;
    let available = (latest > current_version).then(|| AvailableRelease {
        version: latest.to_string(),
        title: release
            .name
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| format!("Veilfeed v{latest}")),
        release_url: release.html_url,
        published_at: release.published_at,
    });
    Ok(ReleaseCheck {
        current_version: current_version.to_string(),
        available,
    })
}

fn validate_repository(repository: &str) -> AppResult<()> {
    let Some((owner, name)) = repository.split_once('/') else {
        return Err(AppError::ReleaseUnavailable(
            "repository must be in owner/name form".into(),
        ));
    };
    let valid = |part: &str| {
        !part.is_empty()
            && part.len() <= 100
            && part
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || ".-_".contains(character))
    };
    if !valid(owner) || !valid(name) || name.contains('/') {
        return Err(AppError::ReleaseUnavailable(
            "repository contains unsupported characters".into(),
        ));
    }
    Ok(())
}

fn validate_release_url(url: &str, repository: &str, tag: &str) -> AppResult<()> {
    let parsed = Url::parse(url).map_err(|error| AppError::InvalidRelease(error.to_string()))?;
    let expected_path = format!("/{repository}/releases/tag/{tag}");
    if parsed.scheme() != "https"
        || parsed.host_str() != Some("github.com")
        || parsed.port().is_some()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !parsed.path().eq_ignore_ascii_case(&expected_path)
    {
        return Err(AppError::InvalidRelease(
            "release URL does not belong to the configured GitHub repository".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(tag: &str, url: &str) -> String {
        serde_json::json!({
            "tag_name": tag,
            "name": "Veilfeed release",
            "html_url": url,
            "published_at": "2026-08-31T12:00:00Z",
            "draft": false,
            "prerelease": false
        })
        .to_string()
    }

    #[test]
    fn reports_only_newer_stable_versions() {
        let newer = parse_release(
            &document(
                "v1.2.0",
                "https://github.com/acme/veilfeed/releases/tag/v1.2.0",
            ),
            "1.1.9",
            "acme/veilfeed",
        )
        .unwrap();
        assert_eq!(newer.available.unwrap().version, "1.2.0");

        let equal = parse_release(
            &document(
                "v1.2.0",
                "https://github.com/acme/veilfeed/releases/tag/v1.2.0",
            ),
            "1.2.0",
            "acme/veilfeed",
        )
        .unwrap();
        assert!(equal.available.is_none());

        let older = parse_release(
            &document(
                "v1.1.9",
                "https://github.com/acme/veilfeed/releases/tag/v1.1.9",
            ),
            "1.2.0",
            "acme/veilfeed",
        )
        .unwrap();
        assert!(older.available.is_none());
    }

    #[test]
    fn rejects_prerelease_malformed_and_untrusted_metadata() {
        let prerelease = document(
            "v1.2.0-beta.1",
            "https://github.com/acme/veilfeed/releases/tag/v1.2.0-beta.1",
        );
        assert!(parse_release(&prerelease, "1.1.0", "acme/veilfeed").is_err());
        assert!(parse_release("not json", "1.1.0", "acme/veilfeed").is_err());
        assert!(
            parse_release(
                &document(
                    "v1.2.0",
                    "https://evil.test/acme/veilfeed/releases/tag/v1.2.0"
                ),
                "1.1.0",
                "acme/veilfeed",
            )
            .is_err()
        );
        assert!(
            parse_release(
                &document(
                    "v1.2.0",
                    "https://github.com/acme/other/releases/tag/v1.2.0"
                ),
                "1.1.0",
                "acme/veilfeed",
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_oversized_release_metadata() {
        let oversized = "x".repeat(MAX_RELEASE_RESPONSE_BYTES + 1);
        assert!(matches!(
            parse_release(&oversized, "1.0.0", "acme/veilfeed"),
            Err(AppError::ContentTooLarge)
        ));
    }

    #[test]
    fn rejects_invalid_repository_names() {
        for repository in ["missing-slash", "acme/veilfeed/extra", "acme/veil feed"] {
            assert!(validate_repository(repository).is_err());
        }
    }

    #[tokio::test]
    async fn reports_an_unavailable_configured_proxy() {
        let profile = ProxyProfile {
            id: "unavailable".into(),
            name: "Unavailable".into(),
            kind: "socks5".into(),
            endpoint: Some("socks5h://127.0.0.1:1".into()),
            username: None,
            remote_dns: true,
        };
        let error = check_repository(&Network::new(), Some(&profile), "acme/veilfeed")
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::ProxyUnavailable(_)));
    }
}
