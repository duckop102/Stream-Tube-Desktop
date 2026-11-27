//! Watch page data: an anonymous HLS stream from the Apple Innertube clients
//! (no PO token, no JS challenge), the WEB `next` details (likes, owner,
//! related videos, comment entry) and comment pages.

use super::api::{is_channel_id, is_video_id};
use crate::{
    entities::{Channel, Video},
    shared::i18n::Language,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

const ORIGIN: &str = "https://www.youtube.com";
const WEB_VERSION: &str = "2.20251120.01.00";
const BROWSER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const VISION_VERSION: &str = "1.02";
const VISION_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15";
const IOS_VERSION: &str = "20.10.4";
const IOS_AGENT: &str =
    "com.google.ios.youtube/20.10.4 (iPhone16,2; U; CPU iOS 18_3_2 like Mac OS X;)";
const MAX_DATA_BYTES: u64 = 8 * 1024 * 1024;
const MAX_RELATED: usize = 40;
const VISITOR_LIFETIME: Duration = Duration::from_secs(6 * 3600);

static VISITOR: Mutex<Option<(String, Instant)>> = Mutex::new(None);
/// The visitor that last got the iOS client an HLS manifest: some visitors land
/// in an experiment whose responses leave the manifest out.
static IOS_VISITOR: Mutex<String> = Mutex::new(String::new());

/// What ExoPlayer needs to play a video: the HLS master playlist and the
/// User-Agent of the client that produced it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Stream {
    pub url: String,
    pub agent: &'static str,
    pub live: bool,
    pub duration: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Details {
    pub title: String,
    pub views: String,
    pub date: String,
    pub likes: String,
    pub live: bool,
    pub channel: Owner,
    pub description: String,
    pub comments_count: String,
    pub comments_token: Option<String>,
    pub related: Vec<Related>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Owner {
    pub id: String,
    pub name: String,
    pub avatar: String,
    pub subscribers: String,
}

/// A related video as the watch page lists it; the page hands it back when the
/// viewer taps it, so it is also read from JSON.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Related {
    pub id: String,
    pub title: String,
    pub channel: String,
    pub channel_id: String,
    pub duration: String,
    pub views: String,
    pub published: String,
}

impl Related {
    pub fn into_video(self) -> Option<Video> {
        if !is_video_id(&self.id) || self.title.is_empty() {
            return None;
        }
        Some(Video {
            id: self.id,
            title: self.title,
            channel: Channel {
                id: Some(self.channel_id)
                    .filter(|id| is_channel_id(id))
                    .unwrap_or_default(),
                name: self.channel,
                ..Channel::default()
            },
            duration: self.duration,
            views: self.views,
            published: self.published,
            ..Video::default()
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct CommentPage {
    pub count: String,
    pub comments: Vec<Comment>,
    pub next: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Comment {
    pub author: String,
    pub avatar: String,
    pub text: String,
    pub published: String,
    pub likes: String,
    pub replies: String,
    pub pinned: bool,
    pub creator: bool,
}

struct HlsClient {
    label: &'static str,
    id: &'static str,
    version: &'static str,
    agent: &'static str,
    client: Value,
    /// iOS leaves the manifest out of responses tied to the web visitor id
    web_visitor: bool,
}

/// The HLS stream of a video: visionOS first, iOS for what visionOS refuses
/// (videos made for kids).
pub fn stream(id: &str, language: Language) -> Result<Stream, String> {
    if !is_video_id(id) {
        return Err("Invalid video".to_owned());
    }
    let http = agent(Duration::from_secs(12));
    let vision = hls_client(&http, id, language, visionos());
    vision.or_else(|first| {
        hls_client(&http, id, language, ios()).map_err(|second| format!("{first}\n{second}"))
    })
}

fn visionos() -> HlsClient {
    HlsClient {
        label: "visionOS",
        id: "101",
        version: VISION_VERSION,
        agent: VISION_AGENT,
        client: json!({
            "clientName": "VISIONOS", "clientVersion": VISION_VERSION,
            "deviceMake": "Apple", "deviceModel": "RealityDevice17,1",
            "osName": "visionOS", "osVersion": "26.5.23O471",
        }),
        web_visitor: true,
    }
}

fn ios() -> HlsClient {
    HlsClient {
        label: "iOS",
        id: "5",
        version: IOS_VERSION,
        agent: IOS_AGENT,
        client: json!({
            "clientName": "IOS", "clientVersion": IOS_VERSION,
            "deviceMake": "Apple", "deviceModel": "iPhone16,2",
            "osName": "iPhone", "osVersion": "18.3.2.22D82",
        }),
        web_visitor: false,
    }
}

fn hls_client(
    http: &ureq::Agent,
    id: &str,
    language: Language,
    c: HlsClient,
) -> Result<Stream, String> {
    let label = c.label;
    let call = |visitor: &str| -> Result<Value, String> {
        let mut client = c.client.clone();
        client["userAgent"] = json!(c.agent);
        client["hl"] = json!(hl(language));
        if !visitor.is_empty() {
            client["visitorData"] = json!(visitor);
        }
        let body = json!({
            "videoId": id,
            "contentCheckOk": true,
            "racyCheckOk": true,
            "context": { "client": client }
        });
        let mut headers = vec![
            ("User-Agent", c.agent),
            ("X-YouTube-Client-Name", c.id),
            ("X-YouTube-Client-Version", c.version),
            ("Origin", ORIGIN),
        ];
        if !visitor.is_empty() {
            headers.push(("X-Goog-Visitor-Id", visitor));
        }
        let response = post(
            http,
            &format!("{ORIGIN}/youtubei/v1/player?prettyPrint=false"),
            &body,
            &headers,
        )
        .ok_or_else(|| format!("{label}: no response"))?;
        if let Some(error) = playability_error(&response) {
            return Err(format!("{label}: {error}"));
        }
        Ok(response)
    };
    let response = if c.web_visitor {
        call(&visitor_data(http).unwrap_or_default())?
    } else {
        // the kept visitor first, then fresh ones (a request without one is a new visitor)
        let mut last = None;
        for attempt in 0..5 {
            let kept = IOS_VISITOR.lock().map(|v| v.clone()).unwrap_or_default();
            let visitor = if attempt % 2 == 0 {
                kept.clone()
            } else {
                String::new()
            };
            let response = call(&visitor)?;
            if response.pointer("/streamingData/hlsManifestUrl").is_some() {
                if visitor.is_empty() {
                    if let (Some(fresh), Ok(mut store)) = (
                        response
                            .pointer("/responseContext/visitorData")
                            .and_then(Value::as_str),
                        IOS_VISITOR.lock(),
                    ) {
                        *store = fresh.to_owned();
                    }
                }
                last = Some(response);
                break;
            }
            if !kept.is_empty() && visitor == kept && attempt >= 2 {
                if let Ok(mut store) = IOS_VISITOR.lock() {
                    store.clear();
                }
            }
            last = Some(response);
        }
        last.ok_or_else(|| format!("{label}: no response"))?
    };
    parse_stream(&response, c.agent).ok_or_else(|| format!("{label}: no HLS manifest"))
}

fn visitor_data(http: &ureq::Agent) -> Option<String> {
    if let Some((visitor, time)) = VISITOR.lock().ok()?.clone() {
        if time.elapsed() < VISITOR_LIFETIME {
            return Some(visitor);
        }
    }
    let body = json!({ "context": { "client": {
        "clientName": "WEB", "clientVersion": WEB_VERSION, "hl": "en", "gl": "US"
    } } });
    let response = post(
        http,
        &format!("{ORIGIN}/youtubei/v1/visitor_id?prettyPrint=false"),
        &body,
        &[("User-Agent", BROWSER_AGENT), ("Origin", ORIGIN)],
    )?;
    let visitor = response
        .pointer("/responseContext/visitorData")?
        .as_str()?
        .to_owned();
    if let Ok(mut store) = VISITOR.lock() {
        *store = Some((visitor.clone(), Instant::now()));
    }
    Some(visitor)
}

fn parse_stream(response: &Value, agent: &'static str) -> Option<Stream> {
    let url = response
        .pointer("/streamingData/hlsManifestUrl")?
        .as_str()?;
    if !url.starts_with("https://") {
        return None;
    }
    let details = response.get("videoDetails");
    Some(Stream {
        url: url.to_owned(),
        agent,
        live: details
            .and_then(|d| d.get("isLive"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        duration: details
            .and_then(|d| d.get("lengthSeconds"))
            .and_then(Value::as_str)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0),
    })
}

fn playability_error(response: &Value) -> Option<String> {
    let field = |pointer| {
        response
            .pointer(pointer)
            .and_then(Value::as_str)
            .unwrap_or_default()
    };
    let status = field("/playabilityStatus/status");
    if status == "OK" {
        return None;
    }
    let reason = field("/playabilityStatus/reason");
    Some(match (status.is_empty(), reason.is_empty()) {
        (true, true) => "Video unavailable".to_owned(),
        (_, true) => status.to_owned(),
        (true, false) => reason.to_owned(),
        (false, false) => format!("{reason} ({status})"),
    })
}

/// Likes, owner, view and date text, the comment entry and related videos.
pub fn details(id: &str, language: Language) -> Option<Details> {
    if !is_video_id(id) {
        return None;
    }
    let response = web(
        &agent(Duration::from_secs(10)),
        language,
        "next",
        json!({ "videoId": id, "contentCheckOk": true, "racyCheckOk": true }),
    )?;
    Some(parse_details(&response))
}

/// A page of comments from a continuation token (the watch page's entry token
/// for the first page, then each page's `next`).
pub fn comments(token: &str, language: Language) -> Option<CommentPage> {
    if token.is_empty() || token.len() > 4096 {
        return None;
    }
    let response = web(
        &agent(Duration::from_secs(10)),
        language,
        "next",
        json!({ "continuation": token }),
    )?;
    Some(parse_comments(&response))
}

/// The JSON the watch page reads for a comment request: the token it answers,
/// so a late page for an earlier request is ignored.
pub fn comments_packet(token: &str, page: Option<&CommentPage>) -> String {
    match page {
        Some(page) => json!({ "from": token, "error": false, "page": page }),
        None => json!({ "from": token, "error": true }),
    }
    .to_string()
}

fn parse_details(response: &Value) -> Details {
    let mut details = Details::default();
    let results = response
        .pointer("/contents/twoColumnWatchNextResults")
        .unwrap_or(&Value::Null);
    if let Some(primary) = find(results, "videoPrimaryInfoRenderer") {
        details.title = text(primary.get("title"));
        let views = primary.pointer("/viewCount/videoViewCountRenderer");
        details.live = views
            .and_then(|v| v.get("isLive"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        details.views = views
            .map(|v| {
                let short = text(v.get("shortViewCount"));
                if short.is_empty() {
                    text(v.get("viewCount"))
                } else {
                    short
                }
            })
            .unwrap_or_default();
        let relative = text(primary.get("relativeDateText"));
        details.date = if relative.is_empty() {
            text(primary.get("dateText"))
        } else {
            relative
        };
        details.likes = find(primary, "likeButtonViewModel")
            .and_then(|like| find(like, "defaultButtonViewModel"))
            .and_then(|button| button.pointer("/buttonViewModel/title"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
    }
    if let Some(secondary) = find(results, "videoSecondaryInfoRenderer") {
        if let Some(owner) = secondary.pointer("/owner/videoOwnerRenderer") {
            details.channel = parse_owner(owner);
        }
        details.description = secondary
            .pointer("/attributedDescription/content")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| text(secondary.get("description")));
        details.description = details.description.chars().take(12000).collect();
    }
    let mut sections = Vec::new();
    find_all(results, "itemSectionRenderer", &mut sections);
    details.comments_token = sections
        .iter()
        .filter(|section| {
            section.get("sectionIdentifier").and_then(Value::as_str) == Some("comment-item-section")
        })
        .find_map(|section| {
            section
                .pointer("/contents/0/continuationItemRenderer")
                .and_then(continuation_token)
        });
    if let Some(entry) = find(results, "commentsEntryPointHeaderRenderer") {
        details.comments_count = text(entry.get("commentCount"));
    }
    if let Some(panels) = response.get("engagementPanels").and_then(Value::as_array) {
        let comments = panels
            .iter()
            .filter_map(|p| p.get("engagementPanelSectionListRenderer"))
            .find(|p| {
                p.get("panelIdentifier").and_then(Value::as_str)
                    == Some("engagement-panel-comments-section")
            });
        if let Some(panel) = comments {
            if details.comments_token.is_none() {
                details.comments_token =
                    find(panel, "continuationItemRenderer").and_then(continuation_token);
            }
            if details.comments_count.is_empty() {
                details.comments_count = panel
                    .pointer("/header/engagementPanelTitleHeaderRenderer/contextualInfo")
                    .map(|info| text(Some(info)))
                    .unwrap_or_default();
            }
        }
    }
    if let Some(secondary) = results.get("secondaryResults") {
        let mut seen = Vec::new();
        collect_related(secondary, &mut seen, &mut details.related);
    }
    details
}

fn parse_owner(owner: &Value) -> Owner {
    let mut name = text(owner.get("title"));
    if name.is_empty() {
        name = owner
            .pointer("/attributedTitle/content")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
    }
    let id = owner
        .pointer("/navigationEndpoint/browseEndpoint/browseId")
        .or_else(|| owner.pointer("/title/runs/0/navigationEndpoint/browseEndpoint/browseId"))
        .and_then(Value::as_str)
        .or_else(|| {
            find(owner, "browseEndpoint")
                .and_then(|endpoint| endpoint.get("browseId"))
                .and_then(Value::as_str)
        })
        .filter(|id| is_channel_id(id))
        .unwrap_or_default()
        .to_owned();
    let avatar = owner
        .pointer("/thumbnail/thumbnails")
        .and_then(Value::as_array)
        .and_then(|list| list.last())
        .and_then(|thumb| thumb.get("url"))
        .or_else(|| {
            owner.pointer(
                "/avatarStack/avatarStackViewModel/avatars/0/avatarViewModel/image/sources/0/url",
            )
        })
        .and_then(Value::as_str)
        .map(absolute_url)
        .unwrap_or_default();
    Owner {
        id,
        name,
        avatar,
        subscribers: text(owner.get("subscriberCountText")),
    }
}

fn collect_related(value: &Value, seen: &mut Vec<String>, related: &mut Vec<Related>) {
    if related.len() >= MAX_RELATED {
        return;
    }
    match value {
        Value::Array(items) => {
            for item in items {
                collect_related(item, seen, related);
            }
        }
        Value::Object(fields) => {
            for (key, child) in fields {
                if related.len() >= MAX_RELATED {
                    return;
                }
                let item = match key.as_str() {
                    "compactVideoRenderer" | "videoRenderer" => compact_video(child),
                    "lockupViewModel" => video_lockup(child),
                    "adSlotRenderer"
                    | "promotedSparklesWebRenderer"
                    | "menu"
                    | "trackingParams" => None,
                    _ => {
                        collect_related(child, seen, related);
                        None
                    }
                };
                if let Some(item) = item {
                    if !seen.contains(&item.id) {
                        seen.push(item.id.clone());
                        related.push(item);
                    }
                }
            }
        }
        _ => {}
    }
}

fn compact_video(item: &Value) -> Option<Related> {
    let id = item.get("videoId")?.as_str()?.to_owned();
    let title = text(item.get("title"));
    if !is_video_id(&id) || title.is_empty() {
        return None;
    }
    let owner = item
        .get("longBylineText")
        .or_else(|| item.get("shortBylineText"))
        .or_else(|| item.get("ownerText"));
    let live = text(item.get("lengthText")).is_empty()
        && find(item, "thumbnailOverlayTimeStatusRenderer")
            .and_then(|s| s.get("style"))
            .and_then(Value::as_str)
            == Some("LIVE");
    Some(Related {
        id,
        title,
        channel: text(owner),
        channel_id: owner
            .and_then(|o| o.pointer("/runs/0/navigationEndpoint/browseEndpoint/browseId"))
            .and_then(Value::as_str)
            .filter(|id| is_channel_id(id))
            .unwrap_or_default()
            .to_owned(),
        duration: if live {
            "LIVE".to_owned()
        } else {
            text(item.get("lengthText"))
        },
        views: text(
            item.get("shortViewCountText")
                .or_else(|| item.get("viewCountText")),
        ),
        published: text(item.get("publishedTimeText")),
    })
}

fn video_lockup(item: &Value) -> Option<Related> {
    if item.get("contentType")?.as_str()? != "LOCKUP_CONTENT_TYPE_VIDEO" {
        return None;
    }
    let id = item.get("contentId")?.as_str()?.to_owned();
    let metadata = item.pointer("/metadata/lockupMetadataViewModel")?;
    let title = metadata.pointer("/title/content")?.as_str()?.to_owned();
    if !is_video_id(&id) || title.is_empty() {
        return None;
    }
    let rows = metadata
        .pointer("/metadata/contentMetadataViewModel/metadataRows")
        .and_then(Value::as_array);
    let part = |row: usize, index: usize| -> Option<&Value> {
        rows?
            .get(row)?
            .get("metadataParts")?
            .get(index)?
            .get("text")
    };
    let content = |node: Option<&Value>| -> String {
        node.and_then(|n| n.get("content"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let owner = part(0, 0);
    Some(Related {
        id,
        title,
        channel: content(owner),
        channel_id: owner
            .and_then(|o| find(o, "browseEndpoint"))
            .and_then(|e| e.get("browseId"))
            .and_then(Value::as_str)
            .filter(|id| is_channel_id(id))
            .unwrap_or_default()
            .to_owned(),
        duration: item
            .get("contentImage")
            .and_then(|image| find(image, "thumbnailBadgeViewModel"))
            .and_then(|badge| badge.get("text"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        views: content(part(1, 0)),
        published: content(part(1, 1)),
    })
}

fn parse_comments(response: &Value) -> CommentPage {
    let mut page = CommentPage::default();
    let mut payloads = std::collections::HashMap::new();
    if let Some(mutations) = response
        .pointer("/frameworkUpdates/entityBatchUpdate/mutations")
        .and_then(Value::as_array)
    {
        for mutation in mutations {
            if let (Some(key), Some(payload)) = (
                mutation.get("entityKey").and_then(Value::as_str),
                mutation.pointer("/payload/commentEntityPayload"),
            ) {
                payloads.insert(key.to_owned(), payload);
            }
        }
    }
    let mut items = Vec::new();
    for key in ["onResponseReceivedEndpoints", "onResponseReceivedActions"] {
        for endpoint in response
            .get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            for command in [
                "reloadContinuationItemsCommand",
                "appendContinuationItemsAction",
            ] {
                if let Some(list) = endpoint
                    .pointer(&format!("/{command}/continuationItems"))
                    .and_then(Value::as_array)
                {
                    items.extend(list);
                }
            }
        }
    }
    for item in items {
        if let Some(header) = item.get("commentsHeaderRenderer") {
            let count = text(header.get("countText"));
            page.count = if count.is_empty() {
                text(header.get("commentsCount"))
            } else {
                count
            };
        }
        let thread = item.get("commentThreadRenderer");
        let view = thread
            .and_then(|t| t.get("commentViewModel"))
            .map(|c| c.get("commentViewModel").unwrap_or(c))
            .or_else(|| item.get("commentViewModel"));
        let legacy = thread
            .and_then(|t| t.pointer("/comment/commentRenderer"))
            .or_else(|| item.get("commentRenderer"));
        let comment = if let Some(view) = view {
            view.get("commentKey")
                .and_then(Value::as_str)
                .and_then(|key| payloads.get(key))
                .map(|payload| entity_comment(payload, view))
        } else {
            legacy.map(legacy_comment)
        };
        if let Some(comment) = comment.filter(|c| !c.text.is_empty() || !c.author.is_empty()) {
            page.comments.push(comment);
        }
        if let Some(next) = item.get("continuationItemRenderer") {
            page.next = continuation_token(next);
        }
    }
    page
}

fn entity_comment(payload: &Value, view: &Value) -> Comment {
    let field = |pointer: &str| -> String {
        payload
            .pointer(pointer)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_owned()
    };
    let mut avatar = field("/author/avatarThumbnailUrl");
    if avatar.is_empty() {
        avatar = field("/avatar/image/sources/0/url");
    }
    Comment {
        author: field("/author/displayName"),
        avatar: if avatar.is_empty() {
            avatar
        } else {
            absolute_url(&avatar)
        },
        text: field("/properties/content/content"),
        published: field("/properties/publishedTime"),
        likes: field("/toolbar/likeCountNotliked"),
        replies: field("/toolbar/replyCount"),
        pinned: view.get("pinnedText").is_some(),
        creator: payload
            .pointer("/author/isCreator")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

fn legacy_comment(renderer: &Value) -> Comment {
    Comment {
        author: text(renderer.get("authorText")).trim().to_owned(),
        avatar: renderer
            .pointer("/authorThumbnail/thumbnails")
            .and_then(Value::as_array)
            .and_then(|list| list.last())
            .and_then(|thumb| thumb.get("url"))
            .and_then(Value::as_str)
            .map(absolute_url)
            .unwrap_or_default(),
        text: text(renderer.get("contentText")),
        published: text(renderer.get("publishedTimeText")),
        likes: text(renderer.get("voteCount")),
        replies: renderer
            .get("replyCount")
            .and_then(|count| {
                count
                    .as_u64()
                    .map(|n| n.to_string())
                    .or_else(|| count.as_str().map(str::to_owned))
            })
            .unwrap_or_default(),
        pinned: renderer.get("pinnedCommentBadge").is_some(),
        creator: renderer
            .get("authorIsChannelOwner")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

fn continuation_token(item: &Value) -> Option<String> {
    item.pointer("/continuationEndpoint/continuationCommand/token")
        .or_else(|| item.pointer("/button/buttonRenderer/command/continuationCommand/token"))
        .or_else(|| find(item, "continuationCommand").and_then(|c| c.get("token")))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn absolute_url(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("//") {
        format!("https://{rest}")
    } else {
        url.to_owned()
    }
}

fn find<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(fields) => fields
            .get(key)
            .or_else(|| fields.values().find_map(|child| find(child, key))),
        Value::Array(items) => items.iter().find_map(|child| find(child, key)),
        _ => None,
    }
}

fn find_all<'a>(value: &'a Value, key: &str, found: &mut Vec<&'a Value>) {
    match value {
        Value::Object(fields) => {
            for (name, child) in fields {
                if name == key {
                    found.push(child);
                } else {
                    find_all(child, key, found);
                }
            }
        }
        Value::Array(items) => {
            for child in items {
                find_all(child, key, found);
            }
        }
        _ => {}
    }
}

fn text(value: Option<&Value>) -> String {
    let Some(value) = value else {
        return String::new();
    };
    if let Some(simple) = value.get("simpleText").and_then(Value::as_str) {
        return simple.to_owned();
    }
    if let Some(content) = value.get("content").and_then(Value::as_str) {
        return content.to_owned();
    }
    value
        .get("runs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|run| run.get("text").and_then(Value::as_str))
        .collect()
}

fn hl(language: Language) -> &'static str {
    match language {
        Language::System => "en",
        other => other.code(),
    }
}

fn web(http: &ureq::Agent, language: Language, endpoint: &str, mut body: Value) -> Option<Value> {
    body["context"] = json!({
        "client": { "clientName": "WEB", "clientVersion": WEB_VERSION, "hl": hl(language) }
    });
    post(
        http,
        &format!("{ORIGIN}/youtubei/v1/{endpoint}?prettyPrint=false"),
        &body,
        &[
            ("User-Agent", BROWSER_AGENT),
            ("X-YouTube-Client-Name", "1"),
            ("X-YouTube-Client-Version", WEB_VERSION),
            ("Origin", ORIGIN),
        ],
    )
}

fn post(http: &ureq::Agent, url: &str, body: &Value, headers: &[(&str, &str)]) -> Option<Value> {
    let mut request = http.post(url).header("Content-Type", "application/json");
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let bytes = request
        .send(body.to_string().as_bytes())
        .ok()?
        .body_mut()
        .with_config()
        .limit(MAX_DATA_BYTES)
        .read_to_vec()
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .max_redirects(0)
        .max_redirects_will_error(false)
        .build()
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_hls_manifest_and_refuses_unplayable_videos() {
        let ok = json!({
            "playabilityStatus": { "status": "OK" },
            "streamingData": { "hlsManifestUrl": "https://manifest.googlevideo.com/api/manifest/hls_variant/x" },
            "videoDetails": { "lengthSeconds": "212", "isLive": false }
        });
        assert_eq!(playability_error(&ok), None);
        let stream = parse_stream(&ok, IOS_AGENT).unwrap();
        assert_eq!(stream.duration, 212);
        assert!(!stream.live);
        assert_eq!(stream.agent, IOS_AGENT);

        let insecure = json!({ "streamingData": { "hlsManifestUrl": "http://example.com/x" } });
        assert_eq!(parse_stream(&insecure, IOS_AGENT), None);
        assert_eq!(parse_stream(&json!({}), IOS_AGENT), None);

        let refused = json!({ "playabilityStatus": { "status": "LOGIN_REQUIRED", "reason": "Sign in to confirm your age" } });
        assert_eq!(
            playability_error(&refused).as_deref(),
            Some("Sign in to confirm your age (LOGIN_REQUIRED)")
        );
    }

    #[test]
    fn parses_watch_details_with_related_videos_and_comment_entry() {
        let response = json!({
            "contents": { "twoColumnWatchNextResults": {
                "results": { "results": { "contents": [
                    { "videoPrimaryInfoRenderer": {
                        "title": { "runs": [{ "text": "Big " }, { "text": "Video" }] },
                        "viewCount": { "videoViewCountRenderer": {
                            "viewCount": { "simpleText": "1,234,567 views" },
                            "shortViewCount": { "simpleText": "1.2M views" }
                        } },
                        "dateText": { "simpleText": "Nov 3, 2026" },
                        "relativeDateText": { "simpleText": "3 weeks ago" },
                        "videoActions": { "menuRenderer": { "topLevelButtons": [{
                            "segmentedLikeDislikeButtonViewModel": { "likeButtonViewModel": { "likeButtonViewModel": {
                                "toggleButtonViewModel": { "toggleButtonViewModel": {
                                    "defaultButtonViewModel": { "buttonViewModel": { "title": "48K" } }
                                } }
                            } } }
                        }] } }
                    } },
                    { "videoSecondaryInfoRenderer": {
                        "owner": { "videoOwnerRenderer": {
                            "title": { "runs": [{ "text": "Channel", "navigationEndpoint": { "browseEndpoint": { "browseId": "UCabcdefghijklmnopqrstuv" } } }] },
                            "navigationEndpoint": { "browseEndpoint": { "browseId": "UCabcdefghijklmnopqrstuv" } },
                            "thumbnail": { "thumbnails": [{ "url": "//yt3.ggpht.com/a=s48" }, { "url": "//yt3.ggpht.com/a=s88" }] },
                            "subscriberCountText": { "simpleText": "2.5M subscribers" }
                        } },
                        "attributedDescription": { "content": "Hello\nworld" }
                    } },
                    { "itemSectionRenderer": {
                        "sectionIdentifier": "comment-item-section",
                        "contents": [{ "continuationItemRenderer": {
                            "continuationEndpoint": { "continuationCommand": { "token": "COMMENTS1" } }
                        } }]
                    } },
                    { "itemSectionRenderer": { "contents": [{ "commentsEntryPointHeaderRenderer": {
                        "commentCount": { "simpleText": "3.1K" }
                    } }] } }
                ] } },
                "secondaryResults": { "secondaryResults": { "results": [
                    { "adSlotRenderer": { "videoId": "zzzzzzzzzzz" } },
                    { "compactVideoRenderer": {
                        "videoId": "aaaaaaaaaaa",
                        "title": { "simpleText": "First related" },
                        "longBylineText": { "runs": [{ "text": "Other", "navigationEndpoint": { "browseEndpoint": { "browseId": "UCzyxwvutsrqponmlkjihgfe" } } }] },
                        "lengthText": { "simpleText": "4:20" },
                        "shortViewCountText": { "simpleText": "10K views" },
                        "publishedTimeText": { "simpleText": "1 day ago" }
                    } },
                    { "lockupViewModel": {
                        "contentType": "LOCKUP_CONTENT_TYPE_VIDEO",
                        "contentId": "bbbbbbbbbbb",
                        "contentImage": { "thumbnailViewModel": { "overlays": [{ "thumbnailOverlayBadgeViewModel": {
                            "thumbnailBadges": [{ "thumbnailBadgeViewModel": { "text": "12:01" } }]
                        } }] } },
                        "metadata": { "lockupMetadataViewModel": {
                            "title": { "content": "Lockup related" },
                            "metadata": { "contentMetadataViewModel": { "metadataRows": [
                                { "metadataParts": [{ "text": { "content": "Lockup channel" } }] },
                                { "metadataParts": [{ "text": { "content": "5K views" } }, { "text": { "content": "2 years ago" } }] }
                            ] } }
                        } }
                    } },
                    { "lockupViewModel": { "contentType": "LOCKUP_CONTENT_TYPE_PLAYLIST", "contentId": "PLx" } },
                    { "compactVideoRenderer": { "videoId": "aaaaaaaaaaa", "title": { "simpleText": "Duplicate" } } }
                ] } }
            } }
        });
        let details = parse_details(&response);
        assert_eq!(details.title, "Big Video");
        assert_eq!(details.views, "1.2M views");
        assert_eq!(details.date, "3 weeks ago");
        assert_eq!(details.likes, "48K");
        assert_eq!(details.channel.id, "UCabcdefghijklmnopqrstuv");
        assert_eq!(details.channel.name, "Channel");
        assert_eq!(details.channel.avatar, "https://yt3.ggpht.com/a=s88");
        assert_eq!(details.channel.subscribers, "2.5M subscribers");
        assert_eq!(details.description, "Hello\nworld");
        assert_eq!(details.comments_token.as_deref(), Some("COMMENTS1"));
        assert_eq!(details.comments_count, "3.1K");
        assert_eq!(details.related.len(), 2);
        assert_eq!(details.related[0].id, "aaaaaaaaaaa");
        assert_eq!(details.related[0].channel_id, "UCzyxwvutsrqponmlkjihgfe");
        assert_eq!(details.related[0].duration, "4:20");
        assert_eq!(details.related[1].title, "Lockup related");
        assert_eq!(details.related[1].channel, "Lockup channel");
        assert_eq!(details.related[1].duration, "12:01");
        assert_eq!(details.related[1].views, "5K views");
        assert_eq!(details.related[1].published, "2 years ago");
    }

    #[test]
    fn falls_back_to_the_comments_panel_for_the_entry_token() {
        let response = json!({
            "contents": { "twoColumnWatchNextResults": {} },
            "engagementPanels": [{ "engagementPanelSectionListRenderer": {
                "panelIdentifier": "engagement-panel-comments-section",
                "header": { "engagementPanelTitleHeaderRenderer": { "contextualInfo": { "runs": [{ "text": "812" }] } } },
                "content": { "sectionListRenderer": { "contents": [{ "itemSectionRenderer": { "contents": [{
                    "continuationItemRenderer": { "continuationEndpoint": { "continuationCommand": { "token": "PANEL" } } }
                }] } }] } }
            } }]
        });
        let details = parse_details(&response);
        assert_eq!(details.comments_token.as_deref(), Some("PANEL"));
        assert_eq!(details.comments_count, "812");
    }

    #[test]
    fn parses_entity_and_legacy_comments_with_the_next_page() {
        let response = json!({
            "onResponseReceivedEndpoints": [
                { "reloadContinuationItemsCommand": { "continuationItems": [
                    { "commentsHeaderRenderer": { "countText": { "runs": [{ "text": "1,024" }, { "text": " Comments" }] } } },
                    { "commentThreadRenderer": { "commentViewModel": { "commentViewModel": {
                        "commentKey": "key-1", "pinnedText": "Pinned by Channel"
                    } } } },
                    { "commentThreadRenderer": { "comment": { "commentRenderer": {
                        "authorText": { "simpleText": "@old " },
                        "authorThumbnail": { "thumbnails": [{ "url": "https://yt3.ggpht.com/old" }] },
                        "contentText": { "runs": [{ "text": "Legacy " }, { "text": "text" }] },
                        "publishedTimeText": { "runs": [{ "text": "2 days ago" }] },
                        "voteCount": { "simpleText": "7" },
                        "replyCount": 3
                    } } } },
                    { "commentThreadRenderer": { "commentViewModel": { "commentViewModel": { "commentKey": "missing" } } } },
                    { "continuationItemRenderer": { "continuationEndpoint": { "continuationCommand": { "token": "NEXT" } } } }
                ] } }
            ],
            "frameworkUpdates": { "entityBatchUpdate": { "mutations": [
                { "entityKey": "key-1", "payload": { "commentEntityPayload": {
                    "properties": { "content": { "content": "First!" }, "publishedTime": "1 hour ago" },
                    "author": { "displayName": "@new", "avatarThumbnailUrl": "https://yt3.ggpht.com/new", "isCreator": true },
                    "toolbar": { "likeCountNotliked": " 52 ", "replyCount": "4" }
                } } },
                { "entityKey": "other", "payload": { "engagementToolbarStateEntityPayload": {} } }
            ] } }
        });
        let page = parse_comments(&response);
        assert_eq!(page.count, "1,024 Comments");
        assert_eq!(page.next.as_deref(), Some("NEXT"));
        assert_eq!(page.comments.len(), 2);
        let first = &page.comments[0];
        assert_eq!(first.author, "@new");
        assert_eq!(first.text, "First!");
        assert_eq!(first.likes, "52");
        assert_eq!(first.replies, "4");
        assert!(first.pinned && first.creator);
        let second = &page.comments[1];
        assert_eq!(second.author, "@old");
        assert_eq!(second.text, "Legacy text");
        assert_eq!(second.published, "2 days ago");
        assert_eq!(second.likes, "7");
        assert_eq!(second.replies, "3");
        assert!(!second.pinned);

        let more = json!({ "onResponseReceivedEndpoints": [{ "appendContinuationItemsAction": {
            "continuationItems": [{ "commentRenderer": { "authorText": { "simpleText": "a" }, "contentText": { "simpleText": "b" } } }]
        } }] });
        let page = parse_comments(&more);
        assert_eq!(page.comments.len(), 1);
        assert_eq!(page.next, None);
    }

    #[test]
    fn related_videos_from_the_page_become_queue_videos() {
        let related: Related = serde_json::from_str(
            r#"{"id":"aaaaaaaaaaa","title":"T","channel":"C","channel_id":"bad","extra":1}"#,
        )
        .unwrap();
        let video = related.into_video().unwrap();
        assert_eq!(video.id, "aaaaaaaaaaa");
        assert_eq!(video.channel.name, "C");
        assert_eq!(video.channel.id, "");
        assert!(Related::default().into_video().is_none());
        let packet: Value = serde_json::from_str(&comments_packet("T", None)).unwrap();
        assert_eq!(packet["error"], true);
        assert_eq!(packet["from"], "T");
    }

    #[test]
    #[ignore = "calls YouTube"]
    fn live_stream_details_and_comments() {
        let id = "jNQXAC9IVRw";
        let stream = stream(id, Language::English).unwrap();
        assert!(stream.url.starts_with("https://"));
        let details = details(id, Language::English).unwrap();
        eprintln!(
            "{} | {} | {} | likes {} | {:?} | {} comments | {} related",
            details.title,
            details.views,
            details.date,
            details.likes,
            details.channel,
            details.comments_count,
            details.related.len()
        );
        assert!(!details.title.is_empty());
        assert!(!details.likes.is_empty());
        assert!(is_channel_id(&details.channel.id));
        assert!(details.channel.avatar.starts_with("https://"));
        assert!(!details.related.is_empty());
        let page = comments(&details.comments_token.unwrap(), Language::English).unwrap();
        eprintln!("{:?} next={}", page.comments.first(), page.next.is_some());
        assert!(!page.comments.is_empty());
        let more = comments(&page.next.unwrap(), Language::English).unwrap();
        assert!(!more.comments.is_empty());
    }
}
