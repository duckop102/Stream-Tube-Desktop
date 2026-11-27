use super::{
    Category, Client, Page,
    api::{shuffle, stir},
    is_channel_id, is_video_id,
};
use crate::{
    entities::{Channel, Video},
    shared::i18n::Language,
};
use std::collections::HashSet;

pub struct Interests {
    related: Vec<String>,
    searches: Vec<String>,
    subscribed: Vec<Channel>,
    watched: HashSet<String>,
}

impl Interests {
    pub fn new(history: &[Video], searches: &[String], subscribed: Vec<Channel>) -> Self {
        // each visit draws its seeds from the recent past, so Home changes like YouTube's does
        let mut channels = HashSet::new();
        let mut related: Vec<String> = history
            .iter()
            .filter(|video| !video.short && is_video_id(&video.id))
            .filter(|video| {
                let channel = &video.channel;
                let key = if channel.id.is_empty() {
                    &video.id
                } else {
                    &channel.id
                };
                channels.insert(key)
            })
            .take(12)
            .map(|video| video.id.clone())
            .collect();
        shuffle(&mut related);
        related.truncate(3);
        let mut searches: Vec<String> = searches.iter().take(8).cloned().collect();
        shuffle(&mut searches);
        searches.truncate(2);
        let mut subscribed: Vec<Channel> = subscribed
            .into_iter()
            .filter(|c| is_channel_id(&c.id))
            .collect();
        shuffle(&mut subscribed);
        subscribed.truncate(4);
        Self {
            related,
            searches,
            subscribed,
            watched: history.iter().map(|video| video.id.clone()).collect(),
        }
    }

    pub fn available(&self) -> bool {
        !(self.related.is_empty() && self.searches.is_empty() && self.subscribed.is_empty())
    }

    pub fn load(self, language: Language) -> Option<Page> {
        let (mut sources, home) = std::thread::scope(|scope| {
            let mixed = scope.spawn(move || Client::new(language).home(Category::All));
            let mut tasks = Vec::new();
            for id in &self.related {
                tasks.push(
                    scope.spawn(move || Client::new(language).related(id).unwrap_or_default()),
                );
            }
            for query in &self.searches {
                tasks.push(scope.spawn(move || {
                    Client::new(language)
                        .search(query, false)
                        .map(|page| page.videos)
                        .unwrap_or_default()
                }));
            }
            for channel in &self.subscribed {
                tasks.push(scope.spawn(move || {
                    Client::new(language)
                        .channel(channel)
                        .map(|page| page.videos)
                        .unwrap_or_default()
                }));
            }
            let home = mixed.join().ok().flatten();
            let sources = tasks
                .into_iter()
                .filter_map(|task| task.join().ok())
                .collect::<Vec<_>>();
            (sources, home)
        });

        if let Some(base) = home.as_ref() {
            if sources.iter().map(Vec::len).sum::<usize>() < 8 {
                sources.push(base.videos.clone());
            }
        }
        for feed in &mut sources {
            stir(feed);
        }
        shuffle(&mut sources);
        let mut seen = self.watched;
        let mut videos = Vec::new();
        for index in 0..12 {
            for feed in &sources {
                if let Some(video) = feed
                    .get(index)
                    .filter(|video| seen.insert(video.id.clone()))
                {
                    videos.push(video.clone());
                }
            }
        }
        if videos.is_empty() {
            home.filter(|page| !page.videos.is_empty() || page.next.is_some())
        } else {
            Some(Page {
                videos,
                next: home.and_then(|page| page.next),
            })
        }
    }
}
