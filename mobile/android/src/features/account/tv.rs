//! YouTube sign-in through the TV device flow, the same method the desktop app uses.
//!
//! The phone opens youtube.com/activate with the code filled in; the user picks an
//! account there and allows access. Unlike Google Play services authorization, this
//! needs no OAuth client registered for the APK's signing key, so self-signed builds
//! can sign in. The refresh token is kept in the app's private storage.
use crate::entities::{Channel, Video};
use crate::features::youtube::{is_channel_id, is_video_id};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};
use url::Url;

// Public OAuth client of the YouTube TV app.
const CLIENT_ID: &str = "861556708454-d6dlm3lh05idd8npek18k6be8ba3oc68.apps.googleusercontent.com";
const CLIENT_SECRET: &str = "SboVhoG9s0rNafixCSGGKXAT";
const SCOPE: &str = "http://gdata.youtube.com https://www.googleapis.com/auth/youtube-paid-content";
const TV_VERSION: &str = "7.20251120.13.00";
const TV_UA: &str = "Mozilla/5.0 (ChromiumStylePlatform) Cobalt/25.lts.30.1034943-gold (unlike Gecko), Unknown_TV_Unknown_0/Unknown (Unknown, Unknown)";
const ACTIVATE_URL: &str = "https://www.youtube.com/activate";
const STORAGE_KEY: &str = "stream-tube-tv-account";
const MAX_CHANNELS: usize = 500;

/// Why a step failed, as the account status codes the interface understands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Cancelled = 1,
    Unavailable = 2,
    Network = 4,
    Reconnect = 5,
    Expired = 8,
}

#[derive(Clone, Debug)]
pub struct DeviceCode {
    device_code: String,
    pub user_code: String,
    pub interval: u64,
    pub expires_in: u64,
}

impl DeviceCode {
    /// The activation page with the code filled in.
    pub fn activate_url(&self) -> String {
        activate_url(&self.user_code)
    }
}

pub fn activate_url(user_code: &str) -> String {
    let mut url = Url::parse(ACTIVATE_URL).expect("valid activation URL");
    url.query_pairs_mut().append_pair("user_code", user_code);
    url.into()
}

pub enum Poll {
    Pending,
    SlowDown,
    Approved { access_token: String, refresh_token: String },
}

#[derive(Default, Serialize, Deserialize)]
struct Stored {
    refresh_token: String,
}

/// The saved refresh token, if an account is connected.
pub fn saved_token() -> Option<String> {
    rustferry::storage::get::<Stored>(STORAGE_KEY)
        .ok()
        .flatten()
        .map(|stored| stored.refresh_token)
        .filter(|token| !token.is_empty())
}

pub fn save_token(refresh_token: &str) -> bool {
    rustferry::storage::set(STORAGE_KEY, &Stored { refresh_token: refresh_token.to_owned() }).is_ok()
}

pub fn forget_token() {
    let _ = rustferry::storage::set(STORAGE_KEY, &Stored::default());
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .http_status_as_error(false)
        .build()
        .into()
}

fn post(url: &str, body: &Value, headers: &[(&str, &str)]) -> Result<(u16, Value), Failure> {
    let mut request = agent().post(url).header("User-Agent", TV_UA);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let mut response = request
        .header("Content-Type", "application/json")
        .send(body.to_string())
        .map_err(|_| Failure::Network)?;
    let status = response.status().as_u16();
    let text = response
        .body_mut()
        .with_config()
        .limit(8 * 1024 * 1024)
        .read_to_string()
        .map_err(|_| Failure::Network)?;
    Ok((status, serde_json::from_str(&text).unwrap_or(Value::Null)))
}

/// Step 1: a code for the user to approve on youtube.com/activate.
pub fn start() -> Result<DeviceCode, Failure> {
    let device_id: String = (0..32).map(|_| format!("{:x}", rand_nibble())).collect();
    let (_, reply) = post(
        "https://www.youtube.com/o/oauth2/device/code",
        &json!({ "client_id": CLIENT_ID, "scope": SCOPE, "device_id": device_id, "device_model": "ytlr::" }),
        &[],
    )?;
    let text = |key: &str| reply.get(key).and_then(Value::as_str).map(str::to_owned);
    Ok(DeviceCode {
        device_code: text("device_code").ok_or(Failure::Unavailable)?,
        user_code: text("user_code").ok_or(Failure::Unavailable)?,
        interval: reply.get("interval").and_then(Value::as_u64).unwrap_or(5).clamp(1, 30),
        expires_in: reply.get("expires_in").and_then(Value::as_u64).unwrap_or(1800),
    })
}

/// Step 2: asks whether the user has approved the code yet.
pub fn poll(code: &DeviceCode) -> Result<Poll, Failure> {
    let (_, reply) = post(
        "https://www.youtube.com/o/oauth2/token",
        &json!({
            "client_id": CLIENT_ID, "client_secret": CLIENT_SECRET, "code": code.device_code,
            "grant_type": "http://oauth.net/grant_type/device/1.0"
        }),
        &[],
    )?;
    parse_poll(&reply)
}

fn parse_poll(reply: &Value) -> Result<Poll, Failure> {
    if let Some(error) = reply.get("error").and_then(Value::as_str) {
        return match error {
            "authorization_pending" => Ok(Poll::Pending),
            "slow_down" => Ok(Poll::SlowDown),
            "expired_token" => Err(Failure::Expired),
            "access_denied" => Err(Failure::Cancelled),
            _ => Err(Failure::Unavailable),
        };
    }
    let token = |key: &str| {
        reply
            .get(key)
            .and_then(Value::as_str)
            .filter(|token| !token.is_empty())
            .map(str::to_owned)
    };
    Ok(Poll::Approved {
        access_token: token("access_token").ok_or(Failure::Unavailable)?,
        refresh_token: token("refresh_token").ok_or(Failure::Unavailable)?,
    })
}

/// A fresh access token from the saved refresh token.
pub fn refresh(refresh_token: &str) -> Result<String, Failure> {
    let (status, reply) = post(
        "https://www.youtube.com/o/oauth2/token",
        &json!({
            "client_id": CLIENT_ID, "client_secret": CLIENT_SECRET,
            "refresh_token": refresh_token, "grant_type": "refresh_token"
        }),
        &[],
    )?;
    if let Some(token) = reply.get("access_token").and_then(Value::as_str) {
        return Ok(token.to_owned());
    }
    // the grant was revoked from the Google account page
    Err(if status == 400 || status == 401 { Failure::Reconnect } else { Failure::Unavailable })
}

fn tv_call(access_token: &str, endpoint: &str, mut body: Value) -> Result<Value, Failure> {
    body["context"] = json!({ "client": { "clientName": "TVHTML5", "clientVersion": TV_VERSION, "hl": "en" } });
    let auth = format!("Bearer {access_token}");
    let (status, reply) = post(
        &format!("https://www.youtube.com/youtubei/v1/{endpoint}?prettyPrint=false"),
        &body,
        &[
            ("Authorization", &auth),
            ("X-YouTube-Client-Name", "7"),
            ("X-YouTube-Client-Version", TV_VERSION),
            ("Origin", "https://www.youtube.com"),
        ],
    )?;
    match status {
        200..=299 => Ok(reply),
        401 | 403 => {
            forget_access();
            Err(Failure::Reconnect)
        }
        _ => Err(Failure::Network),
    }
}

/// An access token for the saved account, refreshed when the cached one is old.
pub fn access_token() -> Result<String, Failure> {
    const LIFETIME: Duration = Duration::from_secs(45 * 60);
    if let Ok(cached) = ACCESS.lock() {
        if let Some((token, at)) = cached.as_ref() {
            if at.elapsed() < LIFETIME {
                return Ok(token.clone());
            }
        }
    }
    let token = refresh(&saved_token().ok_or(Failure::Reconnect)?)?;
    remember_access(&token);
    Ok(token)
}

/// Keeps a token the sign-in or reconnect just got, so the feed does not refresh again.
pub fn remember_access(token: &str) {
    if let Ok(mut cached) = ACCESS.lock() {
        *cached = Some((token.to_owned(), Instant::now()));
    }
}

pub fn forget_access() {
    if let Ok(mut cached) = ACCESS.lock() {
        *cached = None;
    }
}

static ACCESS: Mutex<Option<(String, Instant)>> = Mutex::new(None);

/// One page of the account's own YouTube Home, as the YouTube TV app gets it.
#[derive(Debug, Default)]
pub struct HomePage {
    pub videos: Vec<Video>,
    pub shorts: Vec<Video>,
    /// Token for the next row of shelves.
    pub next: Option<String>,
}

/// The account's Home page of the YouTube TV app.
pub const HOME: &str = "FEwhat_to_watch";

/// A personal page of the YouTube TV app: Home, or a topic such as
/// `FEtopics_music`. Each shelf's own "more" page is fetched too, so a page holds
/// about as many videos as the phone app shows before scrolling on.
pub fn browse(access_token: &str, page_id: &str, continuation: Option<&str>) -> Result<HomePage, Failure> {
    let body = match continuation {
        Some(token) => json!({ "continuation": token }),
        None => json!({ "browseId": page_id }),
    };
    let reply = tv_call(access_token, "browse", body)?;
    let (page, shelves) = parse_home(&reply);
    Ok(with_shelf_pages(access_token, page, &shelves))
}

/// Adds the "more" pages of the first few shelves, without repeats.
fn with_shelf_pages(access_token: &str, mut page: HomePage, shelves: &[String]) -> HomePage {
    let more: Vec<Vec<Video>> = std::thread::scope(|scope| {
        let tasks: Vec<_> = shelves
            .iter()
            .take(4)
            .filter_map(|token| {
                std::thread::Builder::new()
                    .spawn_scoped(scope, move || {
                        tv_call(access_token, "browse", json!({ "continuation": token }))
                            .map(|reply| parse_home(&reply).0.videos)
                            .unwrap_or_default()
                    })
                    .ok()
            })
            .collect();
        tasks.into_iter().filter_map(|task| task.join().ok()).collect()
    });
    for videos in more {
        page.videos.extend(videos);
    }
    let mut seen = std::collections::HashSet::new();
    page.videos.retain(|video| seen.insert(video.id.clone()));
    page.shorts.retain(|video| seen.insert(video.id.clone()));
    page
}

/// The newest videos of every channel the account follows, newest first, as the
/// Subscriptions feed of the phone app. The TV page puts a "most relevant" row
/// first (the one shelf with a header); it is left out so the feed stays in order.
pub fn subscription_feed(access_token: &str, continuation: Option<&str>) -> Result<HomePage, Failure> {
    let body = match continuation {
        Some(token) => json!({ "continuation": token }),
        None => json!({ "browseId": "FEsubscriptions" }),
    };
    let reply = tv_call(access_token, "browse", body)?;
    Ok(parse_subscription_feed(&reply))
}

fn parse_subscription_feed(reply: &Value) -> HomePage {
    // the first reply holds one tab for all channels, then one per channel
    const ALL: &str = "/contents/tvBrowseRenderer/content/tvSecondaryNavRenderer/sections/0/tvSecondaryNavSectionRenderer/tabs/0/tabRenderer/content/tvSurfaceContentRenderer/content/sectionListRenderer";
    let list = reply
        .pointer(ALL)
        .or_else(|| reply.pointer("/continuationContents/sectionListContinuation"));
    let Some(list) = list else {
        return parse_home(reply).0;
    };
    let mut page = HomePage::default();
    let mut relevant = Vec::new();
    for shelf in list.get("contents").and_then(Value::as_array).into_iter().flatten() {
        let Some(shelf) = shelf.get("shelfRenderer") else { continue };
        let mut tiles = Vec::new();
        find_all(shelf, "tileRenderer", &mut tiles);
        let headed = shelf.get("headerRenderer").is_some();
        for video in tiles.into_iter().filter_map(tile_video) {
            if video.short {
                page.shorts.push(video);
            } else if headed {
                relevant.push(video);
            } else {
                page.videos.push(video);
            }
        }
    }
    // should the page change its rows, the relevant ones are better than none
    if page.videos.is_empty() {
        page.videos = relevant;
    }
    page.next = list
        .pointer("/continuations/0/nextContinuationData/continuation")
        .and_then(Value::as_str)
        .map(str::to_owned);
    page
}

/// The account's own run of Shorts, as the Shorts tab of the phone app plays it.
/// The first page starts from the Shorts on the account's Home; later pages
/// continue that run with `continuation`.
pub fn shorts(access_token: &str, continuation: Option<&str>) -> Result<HomePage, Failure> {
    let mut page = HomePage::default();
    let sequence = match continuation {
        Some(token) => token.to_owned(),
        None => {
            let home = tv_call(access_token, "browse", json!({ "browseId": HOME }))?;
            page.videos = parse_home(&home).0.shorts;
            let mut starts = Vec::new();
            find_all(&home, "reelWatchEndpoint", &mut starts);
            let Some(sequence) = starts
                .iter()
                .find_map(|start| start.get("sequenceParams").and_then(Value::as_str))
            else {
                return Ok(page);
            };
            sequence.to_owned()
        }
    };
    let reply = match tv_call(access_token, "reel/reel_watch_sequence", json!({ "sequenceParams": sequence })) {
        Ok(reply) => reply,
        Err(error) if page.videos.is_empty() => return Err(error),
        Err(_) => return Ok(page),
    };
    let entries: Vec<(String, String)> = reply
        .get("entries")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.pointer("/command/reelWatchEndpoint"))
        .filter_map(|endpoint| {
            let id = endpoint.get("videoId").and_then(Value::as_str).filter(|id| is_video_id(id))?;
            let params = endpoint.get("params").and_then(Value::as_str).unwrap_or_default();
            Some((id.to_owned(), params.to_owned()))
        })
        .collect();
    // the run names only the videos; each Short's own page gives its title and channel
    for batch in entries.chunks(12) {
        let found: Vec<Video> = std::thread::scope(|scope| {
            let tasks: Vec<_> = batch
                .iter()
                .filter_map(|(id, params)| {
                    std::thread::Builder::new()
                        .spawn_scoped(scope, move || short_details(access_token, id, params))
                        .ok()
                })
                .collect();
            tasks.into_iter().filter_map(|task| task.join().ok()).collect()
        });
        page.videos.extend(found);
    }
    let mut seen = std::collections::HashSet::new();
    page.videos.retain(|video| seen.insert(video.id.clone()));
    page.next = reply.get("continuation").and_then(Value::as_str).map(str::to_owned);
    Ok(page)
}

/// A Short with the title and channel from its own page; the id alone if that fails.
fn short_details(access_token: &str, id: &str, params: &str) -> Video {
    let mut video = Video {
        id: id.to_owned(),
        short: true,
        ..Video::default()
    };
    let Ok(reply) = tv_call(
        access_token,
        "reel/reel_item_watch",
        json!({ "playerRequest": { "videoId": id }, "params": params }),
    ) else {
        return video;
    };
    read_short(&mut video, &reply);
    video
}

fn read_short(video: &mut Video, reply: &Value) {
    let mut titles = Vec::new();
    find_all(reply, "reelTitleText", &mut titles);
    video.title = titles.first().map(|title| text(title)).unwrap_or_default().trim().to_owned();
    let mut channels = Vec::new();
    find_all(reply, "channelTitleText", &mut channels);
    if let Some(channel) = channels.first() {
        video.channel.name = text(channel);
        let mut links = Vec::new();
        find_all(channel, "browseId", &mut links);
        video.channel.id = links
            .into_iter()
            .filter_map(Value::as_str)
            .find(|id| is_channel_id(id))
            .unwrap_or_default()
            .to_owned();
    }
}

/// The page and the "more" tokens of its video shelves.
fn parse_home(reply: &Value) -> (HomePage, Vec<String>) {
    let mut page = HomePage::default();
    let mut shelf_tokens = Vec::new();
    let mut shelves = Vec::new();
    find_all(reply, "shelfRenderer", &mut shelves);
    // a shelf's "more" reply has no shelves, only its tiles
    if shelves.is_empty() {
        shelves.push(reply);
    }
    for shelf in shelves {
        let mut tiles = Vec::new();
        find_all(shelf, "tileRenderer", &mut tiles);
        let mut has_videos = false;
        for tile in tiles {
            let Some(video) = tile_video(tile) else { continue };
            if video.short {
                page.shorts.push(video);
            } else {
                has_videos = true;
                page.videos.push(video);
            }
        }
        if has_videos {
            if let Some(token) = shelf
                .pointer("/content/horizontalListRenderer/continuations/0/nextContinuationData/continuation")
                .and_then(Value::as_str)
            {
                shelf_tokens.push(token.to_owned());
            }
        }
    }
    page.next = [
        "/contents/tvBrowseRenderer/content/tvSurfaceContentRenderer/content/sectionListRenderer/continuations/0/nextContinuationData/continuation",
        "/continuationContents/sectionListContinuation/continuations/0/nextContinuationData/continuation",
    ]
    .iter()
    .find_map(|pointer| reply.pointer(pointer).and_then(Value::as_str))
    .map(str::to_owned);
    (page, shelf_tokens)
}

/// A video or Short tile; ads, mixes and playlists are left out.
fn tile_video(tile: &Value) -> Option<Video> {
    let short = match tile.get("contentType").and_then(Value::as_str)? {
        "TILE_CONTENT_TYPE_VIDEO" => false,
        "TILE_CONTENT_TYPE_SHORTS" => true,
        _ => return None,
    };
    let select = tile.get("onSelectCommand")?;
    if select.get("watchEndpoint").is_none() && select.get("reelWatchEndpoint").is_none() {
        return None;
    }
    let id = tile.get("contentId").and_then(Value::as_str).filter(|id| is_video_id(id))?;
    let metadata = tile.pointer("/metadata/tileMetadataRenderer")?;
    let title = metadata.get("title").map(text).unwrap_or_default();
    let lines: Vec<Vec<String>> = metadata
        .get("lines")
        .and_then(Value::as_array)
        .map(|lines| {
            lines
                .iter()
                .map(|line| {
                    line.pointer("/lineRenderer/items")
                        .and_then(Value::as_array)
                        .map(|items| {
                            items
                                .iter()
                                .filter_map(|item| item.pointer("/lineItemRenderer/text"))
                                .map(|value| text(value).trim().to_owned())
                                .filter(|part| !part.is_empty() && part != "•")
                                .collect()
                        })
                        .unwrap_or_default()
                })
                .collect()
        })
        .unwrap_or_default();
    let line = |index: usize, part: usize| {
        lines.get(index).and_then(|line| line.get(part)).cloned().unwrap_or_default()
    };
    let duration = tile
        .pointer("/header/tileHeaderRenderer/thumbnailOverlays")
        .and_then(Value::as_array)
        .and_then(|overlays| {
            overlays.iter().find_map(|overlay| {
                overlay
                    .pointer("/thumbnailOverlayTimeStatusRenderer/text")
                    .map(text)
                    .filter(|time| !time.is_empty())
            })
        })
        .unwrap_or_default();
    // only Shorts carry the channel, in their long-press menu
    let mut links = Vec::new();
    find_all(tile, "browseId", &mut links);
    let channel_id = links
        .into_iter()
        .filter_map(Value::as_str)
        .find(|id| is_channel_id(id))
        .unwrap_or_default()
        .to_owned();
    Some(Video {
        id: id.to_owned(),
        title,
        channel: Channel {
            id: channel_id,
            name: lines.first().map(|parts| parts.join(", ")).unwrap_or_default(),
            avatar: String::new(),
        },
        duration,
        views: line(1, 0),
        published: line(1, 1),
        short,
        avatar: String::new(),
    })
}

#[cfg(test)]
/// The signed-in account's display name.
pub fn account_name(access_token: &str) -> String {
    account_profile(access_token).0
}

/// The account name and its profile picture URL (empty when the reply has none).
pub fn account_profile(access_token: &str) -> (String, String) {
    let reply = tv_call(access_token, "account/accounts_list", json!({ "accountReadMask": { "returnOwner": true } })).ok();
    let item = reply
        .as_ref()
        .and_then(|reply| find(reply, "accountItem").or_else(|| find(reply, "accountItemRenderer")));
    let name = item
        .and_then(|item| item.get("accountName").map(text))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "YouTube account".to_owned());
    let photo = item
        .and_then(|item| item.pointer("/accountPhoto/thumbnails"))
        .and_then(Value::as_array)
        .and_then(|thumbnails| thumbnails.last())
        .and_then(|thumbnail| thumbnail.get("url"))
        .and_then(Value::as_str)
        .map(|url| if url.starts_with("//") { format!("https:{url}") } else { url.to_owned() })
        .unwrap_or_default();
    (name, photo)
}

/// Channels the account is subscribed to: the per-channel tabs of the TV subscriptions page.
pub fn subscriptions(access_token: &str) -> Result<Vec<Channel>, Failure> {
    let reply = tv_call(access_token, "browse", json!({ "browseId": "FEsubscriptions" }))?;
    Ok(parse_subscriptions(&reply))
}

fn parse_subscriptions(reply: &Value) -> Vec<Channel> {
    let mut tabs = Vec::new();
    find_all(reply, "tabRenderer", &mut tabs);
    let mut channels: Vec<Channel> = Vec::new();
    for tab in tabs {
        let Some(endpoint) = tab
            .pointer("/endpoint/browseEndpoint")
            .or_else(|| find(tab, "browseEndpoint"))
        else {
            continue;
        };
        // older responses link straight to the channel; newer ones keep the channel id
        // inside the protobuf `params` of an FEsubscriptions link
        let id = endpoint
            .get("browseId")
            .and_then(Value::as_str)
            .filter(|id| id.starts_with("UC"))
            .map(str::to_owned)
            .or_else(|| endpoint.get("params").and_then(Value::as_str).and_then(channel_id_from_params));
        let Some(id) = id.filter(|id| is_channel_id(id)) else { continue };
        if channels.iter().any(|channel| channel.id == id) {
            continue;
        }
        let name = tab.get("title").map(text).unwrap_or_default();
        let avatar = tab
            .pointer("/thumbnail/thumbnails")
            .and_then(Value::as_array)
            .and_then(|sizes| sizes.last())
            .and_then(|size| size.get("url"))
            .and_then(Value::as_str)
            .map(|url| if let Some(rest) = url.strip_prefix("//") { format!("https://{rest}") } else { url.to_owned() })
            .unwrap_or_default();
        channels.push(Channel { id, name, avatar });
        if channels.len() == MAX_CHANNELS {
            break;
        }
    }
    channels
}

/// Pulls a "UC…" channel id out of base64 protobuf browse params.
fn channel_id_from_params(params: &str) -> Option<String> {
    use base64::Engine;
    let params = params.replace("%3D", "=").replace("%3d", "=");
    let params = params.trim_end_matches('=');
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(params)
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(params))
        .ok()?;
    let decoded = String::from_utf8_lossy(&bytes);
    let start = decoded.find("UC")?;
    let id: String = decoded[start..].chars().take(24).collect();
    is_channel_id(&id).then_some(id)
}

fn text(value: &Value) -> String {
    if let Some(simple) = value.get("simpleText").and_then(Value::as_str) {
        return simple.to_owned();
    }
    if let Some(runs) = value.get("runs").and_then(Value::as_array) {
        return runs.iter().filter_map(|run| run.get("text").and_then(Value::as_str)).collect();
    }
    value.as_str().unwrap_or_default().to_owned()
}

fn find<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(map) => map.get(key).or_else(|| map.values().find_map(|child| find(child, key))),
        Value::Array(items) => items.iter().find_map(|child| find(child, key)),
        _ => None,
    }
}

fn find_all<'a>(value: &'a Value, key: &str, out: &mut Vec<&'a Value>) {
    match value {
        Value::Object(map) => {
            for (name, child) in map {
                if name == key {
                    out.push(child);
                } else {
                    find_all(child, key, out);
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|child| find_all(child, key, out)),
        _ => {}
    }
}

fn rand_nibble() -> u8 {
    use std::hash::{BuildHasher, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
    (hasher.finish() & 0xf) as u8
}

#[cfg(test)]
mod home_tests {
    use super::*;

    fn tile(kind: &str, id: &str, select: &str) -> Value {
        json!({ "tileRenderer": {
            "contentType": kind,
            "contentId": id,
            "onSelectCommand": { select: { "videoId": id } },
            "header": { "tileHeaderRenderer": { "thumbnailOverlays": [
                { "thumbnailOverlayTimeStatusRenderer": { "text": { "simpleText": "8:56" } } }
            ] } },
            "metadata": { "tileMetadataRenderer": {
                "title": { "simpleText": "The Return of The King" },
                "lines": [
                    { "lineRenderer": { "items": [ { "lineItemRenderer": { "text": { "runs": [ { "text": "Rekkles" } ] } } } ] } },
                    { "lineRenderer": { "items": [
                        { "lineItemRenderer": { "text": { "simpleText": "" } } },
                        { "lineItemRenderer": { "text": { "simpleText": "250K views" } } },
                        { "lineItemRenderer": { "text": { "simpleText": "•" } } },
                        { "lineItemRenderer": { "text": { "simpleText": "7 hours ago" } } }
                    ] } }
                ]
            } },
            "onLongPressCommand": { "menu": { "browseEndpoint": { "browseId": "UC4gz07oIFoFXSQ4gp87XdIA" } } }
        } })
    }

    #[test]
    fn reads_personal_home_shelves_without_ads() {
        let reply = json!({
            "contents": { "tvBrowseRenderer": { "content": { "tvSurfaceContentRenderer": { "content": {
                "sectionListRenderer": {
                    "contents": [
                        { "shelfRenderer": { "content": { "horizontalListRenderer": {
                            "items": [
                                { "tileRenderer": { "contentId": "dJSDOjLsrEnUufYo", "onSelectCommand": { "commandExecutorCommand": {} } } },
                                tile("TILE_CONTENT_TYPE_VIDEO", "FU3LrO8i2dM", "watchEndpoint"),
                                tile("TILE_CONTENT_TYPE_PLAYLIST", "RDYphV1BeMDFg", "watchEndpoint")
                            ],
                            "continuations": [ { "nextContinuationData": { "continuation": "shelf-more" } } ]
                        } } } },
                        { "shelfRenderer": { "content": { "horizontalListRenderer": {
                            "items": [ tile("TILE_CONTENT_TYPE_SHORTS", "4UhjxJPpdGU", "reelWatchEndpoint") ],
                            "continuations": [ { "nextContinuationData": { "continuation": "shorts-more" } } ]
                        } } } }
                    ],
                    "continuations": [ { "nextContinuationData": { "continuation": "next-shelves" } } ]
                }
            } } } } }
        });
        let (page, shelves) = parse_home(&reply);
        assert_eq!(page.videos.len(), 1);
        let video = &page.videos[0];
        assert_eq!(video.id, "FU3LrO8i2dM");
        assert_eq!(video.title, "The Return of The King");
        assert_eq!(video.channel.name, "Rekkles");
        assert_eq!(video.views, "250K views");
        assert_eq!(video.published, "7 hours ago");
        assert_eq!(video.duration, "8:56");
        assert_eq!(page.shorts.len(), 1);
        assert!(page.shorts[0].short);
        assert_eq!(page.shorts[0].channel.id, "UC4gz07oIFoFXSQ4gp87XdIA");
        // only the video shelf's "more" page is worth fetching
        assert_eq!(shelves, vec!["shelf-more".to_owned()]);
        assert_eq!(page.next.as_deref(), Some("next-shelves"));
    }

    #[test]
    fn subscription_feed_keeps_the_newest_first_order() {
        let shelf = |header: bool, id: &str| {
            let mut shelf = json!({ "content": { "horizontalListRenderer": {
                "items": [ tile("TILE_CONTENT_TYPE_VIDEO", id, "watchEndpoint") ]
            } } });
            if header {
                shelf["headerRenderer"] = json!({ "shelfHeaderRenderer": {} });
            }
            json!({ "shelfRenderer": shelf })
        };
        let reply = json!({ "contents": { "tvBrowseRenderer": { "content": { "tvSecondaryNavRenderer": { "sections": [
            { "tvSecondaryNavSectionRenderer": { "tabs": [
                { "tabRenderer": { "content": { "tvSurfaceContentRenderer": { "content": { "sectionListRenderer": {
                    "contents": [
                        shelf(true, "relevant111"),
                        shelf(false, "newest11111"),
                        shelf(false, "older111111"),
                        { "shelfRenderer": { "headerRenderer": {}, "content": { "horizontalListRenderer": {
                            "items": [ tile("TILE_CONTENT_TYPE_SHORTS", "short111111", "reelWatchEndpoint") ]
                        } } } }
                    ],
                    "continuations": [ { "nextContinuationData": { "continuation": "older-page" } } ]
                } } } } } },
                // a channel's own tab is not part of the feed
                { "tabRenderer": { "content": { "tvSurfaceContentRenderer": { "content": { "sectionListRenderer": {
                    "contents": [ shelf(false, "channel1111") ]
                } } } } } }
            ] } }
        ] } } } } });
        let page = parse_subscription_feed(&reply);
        let ids: Vec<&str> = page.videos.iter().map(|video| video.id.as_str()).collect();
        assert_eq!(ids, ["newest11111", "older111111"]);
        assert_eq!(page.shorts.len(), 1);
        assert_eq!(page.next.as_deref(), Some("older-page"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activation_link_carries_the_code() {
        assert_eq!(activate_url("HDZ-QKX-JSMD"), "https://www.youtube.com/activate?user_code=HDZ-QKX-JSMD");
        assert_eq!(activate_url("A&B C"), "https://www.youtube.com/activate?user_code=A%26B+C");
    }

    #[test]
    fn reads_the_poll_replies() {
        assert!(matches!(parse_poll(&json!({ "error": "authorization_pending" })), Ok(Poll::Pending)));
        assert!(matches!(parse_poll(&json!({ "error": "slow_down" })), Ok(Poll::SlowDown)));
        assert!(matches!(parse_poll(&json!({ "error": "expired_token" })), Err(Failure::Expired)));
        assert!(matches!(parse_poll(&json!({ "error": "access_denied" })), Err(Failure::Cancelled)));
        assert!(matches!(parse_poll(&json!({ "access_token": "a" })), Err(Failure::Unavailable)));
        assert!(matches!(
            parse_poll(&json!({ "access_token": "a", "refresh_token": "r" })),
            Ok(Poll::Approved { .. })
        ));
    }

    #[test]
    fn collects_subscribed_channels_from_tabs() {
        use base64::Engine;
        let params = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b"\x12\x18UCabcdefghijklmnopqrstuv\x18\x01");
        let reply = json!({ "contents": { "tvBrowseRenderer": { "content": { "tvSecondaryNavRenderer": { "sections": [{
            "tvSecondaryNavSectionRenderer": { "tabs": [
                { "tabRenderer": { "title": { "simpleText": "All" }, "endpoint": { "browseEndpoint": { "browseId": "FEsubscriptions" } } } },
                { "tabRenderer": { "title": { "runs": [{ "text": "Old " }, { "text": "style" }] }, "endpoint": { "browseEndpoint": { "browseId": "UC0123456789abcdefghijkl" } } } },
                { "tabRenderer": { "title": { "simpleText": "New style" }, "endpoint": { "browseEndpoint": { "browseId": "FEsubscriptions", "params": params } } } },
                { "tabRenderer": { "title": { "simpleText": "Duplicate" }, "endpoint": { "browseEndpoint": { "browseId": "UC0123456789abcdefghijkl" } } } }
            ] }
        }] } } } } });
        let channels = parse_subscriptions(&reply);
        assert_eq!(
            channels,
            vec![
                Channel { id: "UC0123456789abcdefghijkl".into(), name: "Old style".into(), avatar: String::new() },
                Channel { id: "UCabcdefghijklmnopqrstuv".into(), name: "New style".into(), avatar: String::new() },
            ]
        );
    }

    /// Signs in for real: prints a code to approve at youtube.com/activate, then reads
    /// the account name and subscriptions. Run with
    /// `cargo test --lib tv_sign_in_live -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn tv_sign_in_live() {
        let code = start().expect("device code");
        println!("Approve at {}", code.activate_url());
        let deadline = std::time::Instant::now() + Duration::from_secs(code.expires_in.min(600));
        let mut interval = code.interval;
        let access = loop {
            assert!(std::time::Instant::now() < deadline, "not approved in time");
            std::thread::sleep(Duration::from_secs(interval));
            match poll(&code).expect("poll") {
                Poll::Pending => {}
                Poll::SlowDown => interval += 2,
                Poll::Approved { access_token, refresh_token } => {
                    assert!(refresh(&refresh_token).is_ok(), "refresh works");
                    break access_token;
                }
            }
        };
        println!("account: {}", account_name(&access));
        println!("subscriptions: {}", subscriptions(&access).expect("subscriptions").len());
    }

}

