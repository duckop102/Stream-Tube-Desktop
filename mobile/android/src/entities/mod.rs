use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Channel {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub avatar: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Video {
    pub id: String,
    pub title: String,
    pub channel: Channel,
    pub duration: String,
    pub views: String,
    pub published: String,
    pub short: bool,
    /// The channel's avatar image URL, when the feed provides one.
    #[serde(default)]
    pub avatar: String,
}
