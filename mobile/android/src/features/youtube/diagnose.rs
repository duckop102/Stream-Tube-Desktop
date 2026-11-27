//! Settings → Network check: fetches each kind of YouTube resource the app uses
//! the way the app fetches it, and reports status, size, time or the error.
//! It exists to see on a real phone what the desktop and the emulator cannot.

use super::{Client, api::thumbnail_agent, watch};
use crate::shared::i18n::Language;
use std::time::{Duration, Instant};
use ureq::config::IpFamily;

/// One line per check, in the order they ran.
pub fn report(language: Language) -> String {
    let mut lines = Vec::new();
    let started = Instant::now();
    let shorts = Client::new(language).shorts();
    let feed = Client::new(language).search("popular videos this week", false);
    lines.push(format!(
        "API: feed {} videos, shorts {} ({:.1}s)",
        feed.as_ref().map_or(0, |page| page.videos.len()),
        shorts.as_ref().map_or(0, |page| page.videos.len()),
        started.elapsed().as_secs_f32()
    ));
    let video = feed.as_ref().and_then(|page| page.videos.first());
    if let Some(video) = video {
        let thumb = format!("https://i.ytimg.com/vi/{}/hqdefault.jpg", video.id);
        lines.push(format!("Thumbnail: {}", probe(&thumbnail_agent(), &thumb)));
        lines.push(format!("Thumbnail IPv4: {}", probe(&ipv4_agent(), &thumb)));
        lines.push(format!(
            "Thumbnail decode: {}",
            if super::thumbnail(&thumbnail_agent(), &video.id, None).is_some() { "ok" } else { "failed" }
        ));
    }
    if let Some(avatar) = feed
        .as_ref()
        .and_then(|page| page.videos.iter().find(|video| !video.avatar.is_empty()))
        .map(|video| video.avatar.clone())
    {
        lines.push(format!("Avatar: {}", probe(&thumbnail_agent(), &avatar)));
    }
    if let Some(short) = shorts.as_ref().and_then(|page| page.videos.first()) {
        let started = Instant::now();
        match watch::stream(&short.id, language) {
            Ok(stream) => {
                lines.push(format!("Short stream: ok ({:.1}s)", started.elapsed().as_secs_f32()));
                let agent = media_agent();
                let (summary, body) = fetch(&agent, &stream.url, stream.agent);
                lines.push(format!("HLS playlist: {summary}"));
                // follow the first variant, then its first segment, as ExoPlayer would
                if let Some(variant) = body.as_deref().and_then(first_uri) {
                    let (summary, body) = fetch(&agent, &variant, stream.agent);
                    lines.push(format!("HLS variant: {summary}"));
                    if let Some(segment) = body.as_deref().and_then(first_uri) {
                        let (summary, _) = fetch(&agent, &segment, stream.agent);
                        let host = url::Url::parse(&segment)
                            .ok()
                            .and_then(|url| url.host_str().map(str::to_owned))
                            .unwrap_or_default();
                        lines.push(format!("HLS segment ({host}): {summary}"));
                    }
                }
            }
            Err(error) => lines.push(format!("Short stream: {error}")),
        }
    }
    lines.join("\n")
}

fn ipv4_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(12)))
        .ip_family(IpFamily::Ipv4Only)
        .max_redirects(0)
        .max_redirects_will_error(false)
        .build()
        .into()
}

fn media_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .build()
        .into()
}

fn probe(agent: &ureq::Agent, url: &str) -> String {
    fetch(agent, url, "").0
}

/// "200, 34 KB, 0.21s" or the error, plus the body when it is text.
fn fetch(agent: &ureq::Agent, url: &str, user_agent: &str) -> (String, Option<String>) {
    let started = Instant::now();
    let mut request = agent.get(url);
    if !user_agent.is_empty() {
        request = request.header("User-Agent", user_agent);
    }
    let seconds = || started.elapsed().as_secs_f32();
    match request.call() {
        Ok(mut response) => {
            let status = response.status().as_u16();
            match response.body_mut().with_config().limit(4 * 1024 * 1024).read_to_vec() {
                Ok(bytes) => {
                    let text = String::from_utf8(bytes.clone()).ok();
                    (format!("{status}, {} KB, {:.2}s", bytes.len() / 1024, seconds()), text)
                }
                Err(error) => (format!("{status}, body error: {error} ({:.1}s)", seconds()), None),
            }
        }
        Err(error) => (format!("error: {error} ({:.1}s)", seconds()), None),
    }
}

fn first_uri(playlist: &str) -> Option<String> {
    playlist
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("https://"))
        .map(str::to_owned)
}
