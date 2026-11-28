use crate::{entities::Video, features::youtube};
use serde::Serialize;

#[cfg(target_os = "android")]
mod android;

#[cfg(target_os = "android")]
pub use android::{
    append, listen, open, set_comments, set_details, set_extras, set_following, set_stream,
    set_theme, update_saved,
};

/// Event kinds the Java player reports through `VideoPlayer.event(session, kind, id)`.
pub mod event {
    /// A video started; `id` is the video id.
    pub const WATCHED: i32 = 0;
    /// Toggle saved; `id` is the video id.
    pub const SAVE: i32 = 1;
    /// Open the channel; `id` is `videoId` or `videoId\nchannelId\nchannelName`.
    pub const CHANNEL: i32 = 2;
    pub const CLOSED: i32 = 3;
    /// The queue ran out; load the next feed page.
    pub const MORE: i32 = 4;
    /// Resolve the HLS stream; `id` is the video id (sent again on retry).
    pub const STREAM: i32 = 5;
    /// Load a comment page; `id` is `videoId\ntoken`.
    pub const COMMENTS: i32 = 7;
    /// A related video replaced the current one; `id` is its JSON (`watch::Related`).
    pub const RELATED: i32 = 8;
    /// Toggle following; `id` is `channelId\nchannelName`.
    pub const SUBSCRIBE: i32 = 9;
    /// Ask whether a channel is followed; `id` is `channelId\nchannelName`.
    pub const FOLLOWING: i32 = 10;
}

#[derive(Serialize)]
struct Track<'a> {
    id: &'a str,
    title: &'a str,
    channel: &'a str,
    channel_link: bool,
    avatar: &'a str,
    saved: bool,
}

pub fn tracks(videos: &[Video], is_saved: impl Fn(&str) -> bool) -> String {
    let tracks: Vec<_> = videos
        .iter()
        .map(|video| Track {
            id: &video.id,
            title: &video.title,
            channel: &video.channel.name,
            channel_link: youtube::is_channel_id(&video.channel.id),
            avatar: &video.avatar,
            saved: is_saved(&video.id),
        })
        .collect();
    serde_json::to_string(&tracks).expect("track metadata contains only strings and flags")
}

#[cfg(not(target_os = "android"))]
pub fn open(
    _: i32,
    _: &str,
    _: usize,
    _: crate::shared::i18n::Language,
    _: bool,
    _: bool,
) -> Result<(), String> {
    Err("The embedded player needs Android".to_owned())
}

#[cfg(not(target_os = "android"))]
pub fn listen(_: impl Fn(i32, i32, String) + Send + Sync + 'static) {}

#[cfg(not(target_os = "android"))]
pub fn append(_: i32, _: &str, _: bool, _: bool) {}

#[cfg(not(target_os = "android"))]
pub fn update_saved(_: i32, _: &str, _: bool) {}

#[cfg(not(target_os = "android"))]
pub fn set_stream(_: i32, _: &str, _: &str) {}

#[cfg(not(target_os = "android"))]
pub fn set_details(_: i32, _: &str, _: &str) {}

#[cfg(not(target_os = "android"))]
pub fn set_comments(_: i32, _: &str, _: &str) {}

#[cfg(not(target_os = "android"))]
pub fn set_following(_: i32, _: &str, _: bool) {}

#[cfg(not(target_os = "android"))]
pub fn set_extras(_: i32, _: &str, _: &str) {}

#[cfg(not(target_os = "android"))]
pub fn set_theme(_: bool, _: u32, _: u32) {}
