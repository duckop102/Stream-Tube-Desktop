//! In-app updates from this fork's GitHub releases.
//!
//! CI publishes each Android build as a release tagged `android-<versionCode>` with one
//! signed APK attached. The app compares that number with its own `versionCode`, and
//! hands the APK's address and checksum to Android's package installer, which only
//! commits the install once the downloaded file matches the checksum GitHub reports.
use serde_json::Value;
use std::time::Duration;

#[cfg(target_os = "android")]
mod android;

#[cfg(target_os = "android")]
pub use android::{install, listen, version_code, version_name};

/// The GitHub repository whose releases carry the Android builds.
pub const REPO: &str = match option_env!("STREAMTUBE_UPDATE_REPO") {
    Some(repo) => repo,
    None => "duckop102/Stream-Tube-Desktop",
};
const TAG_PREFIX: &str = "android-";

/// What the Java installer reports back.
pub mod status {
    pub const PROGRESS: i32 = 0;
    pub const CONFIRM: i32 = 1;
    pub const INSTALLED: i32 = 2;
    // 3: failed, with the reason as the message
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub code: i64,
    pub name: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

/// The newest release with a usable APK, if its version code is above `current`.
pub fn check(current: i64) -> Result<Option<Release>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .build()
        .into();
    let body = agent
        .get(&format!("https://api.github.com/repos/{REPO}/releases?per_page=20"))
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "StreamTube-Android-Updater")
        .call()
        .map_err(|error| error.to_string())?
        .body_mut()
        .read_to_string()
        .map_err(|error| error.to_string())?;
    let releases: Value = serde_json::from_str(&body).map_err(|error| error.to_string())?;
    Ok(newest(&releases).filter(|release| release.code > current))
}

/// Picks the release with the highest `android-<code>` tag that has a signed APK from
/// this repository with a SHA-256 digest. Drafts are skipped.
pub fn newest(releases: &Value) -> Option<Release> {
    releases
        .as_array()?
        .iter()
        .filter(|release| !release.get("draft").and_then(Value::as_bool).unwrap_or(false))
        .filter_map(parse_release)
        .max_by_key(|release| release.code)
}

fn parse_release(release: &Value) -> Option<Release> {
    let tag = release.get("tag_name")?.as_str()?;
    let code = tag.strip_prefix(TAG_PREFIX)?.parse::<i64>().ok()?;
    let download = format!("https://github.com/{REPO}/releases/download/");
    let asset = release.get("assets")?.as_array()?.iter().find(|asset| {
        let name = asset.get("name").and_then(Value::as_str).unwrap_or_default();
        let url = asset.get("browser_download_url").and_then(Value::as_str).unwrap_or_default();
        name.to_ascii_lowercase().ends_with(".apk") && url.starts_with(&download)
    })?;
    let sha256 = asset
        .get("digest")?
        .as_str()?
        .strip_prefix("sha256:")
        .filter(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))?
        .to_ascii_lowercase();
    let name = release
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(tag)
        .to_owned();
    Some(Release {
        code,
        name,
        url: asset.get("browser_download_url")?.as_str()?.to_owned(),
        sha256,
        size: asset.get("size").and_then(Value::as_u64).unwrap_or(0),
    })
}

#[cfg(not(target_os = "android"))]
pub fn version_code() -> i64 {
    0
}

#[cfg(not(target_os = "android"))]
pub fn version_name() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

#[cfg(not(target_os = "android"))]
pub fn install(_: &Release) -> bool {
    false
}

#[cfg(not(target_os = "android"))]
pub fn listen(_: impl Fn(i32, i32, String) + Send + Sync + 'static) {}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn asset(name: &str, digest: &str) -> Value {
        json!({
            "name": name,
            "size": 42,
            "digest": digest,
            "browser_download_url": format!("https://github.com/{REPO}/releases/download/x/{name}"),
        })
    }

    const SHA: &str = "sha256:ABCDEF0123456789abcdef0123456789abcdef0123456789abcdef0123456789";

    #[test]
    fn picks_the_highest_android_build_with_a_signed_apk() {
        let releases = json!([
            { "tag_name": "android-10003", "name": "Stream Tube Android 0.1.0-b3", "assets": [asset("a.apk", SHA)] },
            { "tag_name": "android-10005", "name": "", "draft": true, "assets": [asset("b.apk", SHA)] },
            { "tag_name": "android-10004", "name": "", "assets": [asset("c.apk", "")] },
            { "tag_name": "v0.7.5-alpha", "name": "desktop", "assets": [asset("d.apk", SHA)] },
            { "tag_name": "android-10002", "name": "old", "assets": [asset("e.apk", SHA)] }
        ]);
        let release = newest(&releases).unwrap();
        assert_eq!(release.code, 10003);
        assert_eq!(release.name, "Stream Tube Android 0.1.0-b3");
        assert_eq!(release.size, 42);
        assert!(release.url.ends_with("/a.apk"));
        assert_eq!(release.sha256, SHA[7..].to_ascii_lowercase());
    }

    #[test]
    fn ignores_apks_hosted_elsewhere() {
        let releases = json!([{
            "tag_name": "android-7",
            "assets": [{ "name": "x.apk", "digest": SHA, "browser_download_url": "https://example.com/x.apk" }]
        }]);
        assert_eq!(newest(&releases), None);
    }

    #[test]
    fn falls_back_to_the_tag_for_a_missing_name() {
        let releases = json!([{ "tag_name": "android-8", "assets": [asset("x.APK", SHA)] }]);
        assert_eq!(newest(&releases).unwrap().name, "android-8");
    }
}
