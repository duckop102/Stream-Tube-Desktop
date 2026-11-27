use crate::{
    entities::{Channel, Video},
    features::{
        account,
        library::Library,
        player,
        services::{self, Extras},
        updater,
        youtube::{self, Category, Client, Cursor, Interests, Page},
    },
    shared::i18n::{self, Language},
};
use slint::{ComponentHandle, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, VecModel};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::{
        Arc, LazyLock, Mutex,
        atomic::{AtomicI32, AtomicU64, AtomicUsize, Ordering},
    },
};

slint::include_modules!();

type State = Rc<RefCell<AppState>>;

struct AppState {
    library: Library,
    device_language: Language,
    storage_writable: bool,
    request: Arc<AtomicU64>,
    query: String,
    channel: Option<Channel>,
    return_from_channel: Screen,
    playlist: Option<String>,
    next_page: Arc<Mutex<Option<FeedCursor>>>,
    playback_number: i32,
    playback: Option<Playback>,
    account_request: i32,
    account_channels: Vec<Channel>,
}

struct Playback {
    number: i32,
    feed_generation: Option<u64>,
    shorts: bool,
    videos: Vec<Video>,
}

enum Feed {
    Home(Category),
    ForYou(Interests),
    /// A personal page of the signed-in account, read as the YouTube TV app reads
    /// it. The fallback is shown when the account's page cannot be read; the
    /// channels the account follows lend their pictures to its videos.
    Account(TvPage, Box<Feed>, Arc<Vec<Channel>>),
    Shorts,
    Following(Vec<Channel>),
    Channel(Channel),
    Search(String),
    Playlist(String),
    Local(Vec<Video>),
    Next(FeedCursor),
}

#[derive(Clone)]
enum FeedCursor {
    YouTube(Cursor),
    Following(Vec<Channel>),
    Account(TvPage, String, Arc<Vec<Channel>>),
}

/// The account's pages that stand in for the anonymous feeds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TvPage {
    Home,
    Topic(&'static str),
    Subscriptions,
    Shorts,
}

impl TvPage {
    fn topic(category: Category) -> Self {
        match category {
            Category::All => Self::Home,
            Category::Music => Self::Topic("FEtopics_music"),
            Category::Games => Self::Topic("FEtopics_gaming"),
            Category::News => Self::Topic("FEtopics_news"),
            Category::Live => Self::Topic("FEtopics_live"),
        }
    }
}

struct FeedPage {
    videos: Vec<Video>,
    next: Option<FeedCursor>,
}

impl From<Page> for FeedPage {
    fn from(page: Page) -> Self {
        Self {
            videos: page.videos,
            next: page.next.map(FeedCursor::YouTube),
        }
    }
}

pub fn run() -> Result<(), slint::PlatformError> {
    let window = StreamTube::new()?;
    let device_language = sys_locale::get_locale()
        .as_deref()
        .map_or(Language::English, Language::from_locale);
    let stored = Library::load();
    let storage_writable = stored.is_ok();
    let library = stored.unwrap_or_default();
    window
        .global::<UiText>()
        .on_lookup(|language, source| i18n::translate(&language, &source).into());
    window.set_languages(
        Rc::new(VecModel::from(
            Language::OPTIONS
                .iter()
                .map(|locale| LanguageRow {
                    code: locale.code().into(),
                    name: locale.name().into(),
                    credit: locale.credit().into(),
                })
                .collect::<Vec<_>>(),
        ))
        .into(),
    );
    select_language(&window, library.language, device_language);
    window.set_thumbnails(library.thumbnails);
    apply_extras(&window, library.extras);
    // TV sign-in needs only a browser, so every build can connect an account
    window.set_account_available(true);
    let reconnect = library.connected_account;
    window.set_videos(Rc::new(VecModel::<VideoRow>::default()).into());
    window.set_shorts_shelf(Rc::new(VecModel::<VideoRow>::default()).into());
    window.set_history_preview(Rc::new(VecModel::<VideoRow>::default()).into());
    window.set_saved_preview(Rc::new(VecModel::<VideoRow>::default()).into());
    window.set_channels(Rc::new(VecModel::<ChannelRow>::default()).into());
    window.set_suggestions(Rc::new(VecModel::<slint::SharedString>::default()).into());
    window.set_recent_searches(Rc::new(VecModel::<slint::SharedString>::default()).into());
    if !storage_writable {
        notify(&window, "Local library is unavailable");
    }

    let state = Rc::new(RefCell::new(AppState {
        library,
        device_language,
        storage_writable,
        request: Arc::new(AtomicU64::new(0)),
        query: String::new(),
        channel: None,
        return_from_channel: Screen::Home,
        playlist: None,
        next_page: Arc::default(),
        playback_number: 0,
        playback: None,
        account_request: 0,
        account_channels: Vec::new(),
    }));
    bind_navigation(&window, &state);
    bind_videos(&window, &state);
    bind_player(&window, &state);
    bind_settings(&window, &state);
    bind_account(&window, &state);
    bind_updates(&window);
    reload(&window, &state);
    if reconnect && window.get_account_available() {
        authorize(&window, &state, false, false);
    }
    window.run()
}

fn bind_navigation(window: &StreamTube, state: &State) {
    let (target, app) = (window.as_weak(), state.clone());
    window.on_navigate(move |screen| {
        if let Some(ui) = target.upgrade() {
            if matches!(
                screen,
                Screen::Home | Screen::Shorts | Screen::Following | Screen::You
            ) {
                ui.set_tab(screen);
            }
            ui.set_screen(screen);
            reload(&ui, &app);
        }
    });

    let (target, app) = (window.as_weak(), state.clone());
    window.on_back(move || {
        if let Some(ui) = target.upgrade() {
            let screen = match ui.get_screen() {
                Screen::Channel => app.borrow().return_from_channel,
                Screen::Playlist => Screen::Search,
                Screen::Language => Screen::Settings,
                _ => ui.get_tab(),
            };
            ui.set_screen(screen);
            reload(&ui, &app);
        }
    });

    let (target, app) = (window.as_weak(), state.clone());
    window.on_reload(move || {
        if let Some(ui) = target.upgrade() {
            reload(&ui, &app);
        }
    });

    let (target, app) = (window.as_weak(), state.clone());
    window.on_submit(move |input| {
        let Some(ui) = target.upgrade() else { return };
        let input = input.trim();
        if input.is_empty() {
            return;
        }
        if let Some(id) = youtube::video_id(input) {
            let language = current_language(&ui);
            let title = i18n::translate(language.code(), "YouTube video");
            open_player(
                &ui,
                &app,
                vec![Video {
                    id,
                    title: title.into(),
                    ..Video::default()
                }],
                0,
                false,
                false,
            );
            return;
        }
        if let Some(playlist) = youtube::playlist_id(input) {
            app.borrow_mut().playlist = Some(playlist);
            ui.set_screen(Screen::Playlist);
            reload(&ui, &app);
            return;
        }
        if input.contains("://") || input.starts_with("www.") {
            notify(&ui, "Enter a YouTube video link");
            return;
        }
        ui.set_notice("".into());
        let query: String = input.chars().take(200).collect();
        app.borrow_mut().query = query.clone();
        edit_library(&ui, &app, |library| library.searched(query));
        show_recent_searches(&ui, &app);
        reload(&ui, &app);
    });

    let target = window.as_weak();
    window.on_query_changed(move |text| {
        let Some(ui) = target.upgrade() else { return };
        request_suggestions(&ui, text.to_string());
    });

    let (target, app) = (window.as_weak(), state.clone());
    window.on_open_channel(move |id, name| {
        let Some(ui) = target.upgrade() else { return };
        if !youtube::is_channel_id(&id) {
            return;
        }
        {
            let mut state = app.borrow_mut();
            if ui.get_screen() != Screen::Channel {
                state.return_from_channel = ui.get_screen();
            }
            let avatar = channel_avatar(&ui, &state, &id);
            state.channel = Some(Channel {
                id: id.to_string(),
                name: name.to_string(),
                avatar,
            });
        }
        ui.set_screen(Screen::Channel);
        reload(&ui, &app);
    });
}

fn bind_videos(window: &StreamTube, state: &State) {
    let (target, app) = (window.as_weak(), state.clone());
    window.on_watch(move |id| {
        let Some(ui) = target.upgrade() else { return };
        let videos = feed_videos(&ui);
        if let Some(first) = videos.iter().position(|video| video.id == id.as_str()) {
            let shorts = videos[first].short;
            open_player(&ui, &app, videos, first, shorts, true);
        }
    });

    let (target, app) = (window.as_weak(), state.clone());
    window.on_open_shorts(move || {
        let Some(ui) = target.upgrade() else { return };
        let videos = feed_videos(&ui);
        if ui.get_screen() == Screen::Shorts && !videos.is_empty() {
            open_player(&ui, &app, videos, 0, true, true);
        }
    });

    let (target, app) = (window.as_weak(), state.clone());
    window.on_watch_short(move |id| {
        let Some(ui) = target.upgrade() else { return };
        let videos: Vec<Video> = ui.get_shorts_shelf().iter().map(video_from_row).collect();
        if let Some(first) = videos.iter().position(|video| video.id == id.as_str()) {
            open_player(&ui, &app, videos, first, true, false);
        }
    });

    let (target, app) = (window.as_weak(), state.clone());
    window.on_watch_preview(move |history, id| {
        let Some(ui) = target.upgrade() else { return };
        let videos = {
            let state = app.borrow();
            if history {
                state.library.history.clone()
            } else {
                state.library.saved.clone()
            }
        };
        if let Some(first) = videos.iter().position(|video| video.id == id.as_str()) {
            let shorts = videos[first].short;
            open_player(&ui, &app, videos, first, shorts, false);
        }
    });

    let target = window.as_weak();
    window.on_hide_video(move |id| {
        let Some(ui) = target.upgrade() else { return };
        if let Ok(mut hidden) = HIDDEN.lock() {
            hidden.insert(id.to_string());
        }
        let model = ui.get_videos();
        if let Some(index) = model.iter().position(|row| row.id == id) {
            if let Some(rows) = model.as_any().downcast_ref::<VecModel<VideoRow>>() {
                rows.remove(index);
            }
        }
        notify(&ui, "Video removed from your feed");
    });

    let (target, app) = (window.as_weak(), state.clone());
    window.on_save(move |id| {
        let Some(ui) = target.upgrade() else { return };
        let video = find_video(&ui, &id);
        let Some(video) = video else { return };
        let saved = save_video(&ui, &app, video);
        notify(
            &ui,
            if saved {
                "Saved to Watch later"
            } else {
                "Removed from Watch later"
            },
        );
    });

    let (target, app) = (window.as_weak(), state.clone());
    window.on_follow(move || {
        let Some(ui) = target.upgrade() else { return };
        let channel = app.borrow().channel.clone();
        let Some(channel) = channel else { return };
        if app
            .borrow()
            .account_channels
            .iter()
            .any(|remote| remote.id == channel.id)
        {
            return;
        }
        let id = channel.id.clone();
        edit_library(&ui, &app, |library| library.toggle_following(channel));
        ui.set_channel_followed(app.borrow().library.is_following(&id));
    });
}

fn bind_player(window: &StreamTube, state: &State) {
    let target = window.as_weak();
    player::listen(move |number, kind, id| {
        let _ = target.upgrade_in_event_loop(move |ui| {
            ui.invoke_player_event(number, kind, id.into());
        });
    });

    let (target, app) = (window.as_weak(), state.clone());
    window.on_player_event(move |number, kind, id| {
        use player::event;
        let Some(ui) = target.upgrade() else { return };
        let id = id.to_string();
        let shorts = {
            let mut state = app.borrow_mut();
            let Some(playback) = state
                .playback
                .as_ref()
                .filter(|player| player.number == number)
            else {
                return;
            };
            let shorts = playback.shorts;
            if kind == event::CLOSED {
                state.playback = None;
                return;
            }
            shorts
        };
        match kind {
            event::MORE => {
                if !load_more(&ui, &app) && !ui.get_loading_more() {
                    player::append(number, "[]", false, false);
                }
                return;
            }
            event::STREAM => {
                load_stream(&ui, number, id);
                return;
            }
            event::COMMENTS => {
                if let Some((video, token)) = id.split_once('\n') {
                    load_comments(&ui, number, video.to_owned(), token.to_owned());
                }
                return;
            }
            event::RELATED => {
                add_related_video(&app, number, &id);
                return;
            }
            event::SUBSCRIBE | event::FOLLOWING => {
                if let Some(channel) = player_channel(&id) {
                    let channel_id = channel.id.clone();
                    let following = player_following(&ui, &app, channel, kind == event::SUBSCRIBE);
                    player::set_following(number, &channel_id, following);
                }
                return;
            }
            _ => {}
        }
        let (video_id, channel) = match id.split_once('\n') {
            Some((video, channel)) => (video, player_channel(channel)),
            None => (id.as_str(), None),
        };
        let video = app.borrow().playback.as_ref().and_then(|playback| {
            playback
                .videos
                .iter()
                .find(|video| video.id == video_id)
                .cloned()
        });
        let Some(video) = video else { return };
        match kind {
            event::WATCHED => {
                edit_library(&ui, &app, |library| library.watched(video));
                load_playback_extras(
                    &ui,
                    number,
                    video_id.to_owned(),
                    app.borrow().library.extras,
                );
                if !shorts {
                    load_video_description(&ui, number, video_id.to_owned());
                }
            }
            event::SAVE => {
                let saved = save_video(&ui, &app, video);
                player::update_saved(number, video_id, saved);
            }
            event::CHANNEL => {
                let channel = channel.unwrap_or(video.channel);
                ui.invoke_open_channel(channel.id.into(), channel.name.into());
            }
            _ => {}
        }
    });

    let (target, app) = (window.as_weak(), state.clone());
    window.on_load_more(move || {
        if let Some(ui) = target.upgrade() {
            load_more(&ui, &app);
        }
    });

    let (target, app) = (window.as_weak(), state.clone());
    window.on_page_appended(move |failed| {
        let Some(ui) = target.upgrade() else { return };
        let mut state = app.borrow_mut();
        let generation = state.request.load(Ordering::Relaxed);
        let state = &mut *state;
        let Some(playback) = state
            .playback
            .as_mut()
            .filter(|player| player.feed_generation == Some(generation))
        else {
            return;
        };
        let ids: HashSet<_> = playback
            .videos
            .iter()
            .map(|video| video.id.as_str())
            .collect();
        let extra: Vec<_> = feed_videos(&ui)
            .into_iter()
            .filter(|video| !ids.contains(video.id.as_str()))
            .collect();
        let tracks = player::tracks(&extra, |id| state.library.is_saved(id));
        playback.videos.extend(extra);
        player::append(playback.number, &tracks, ui.get_has_more(), failed);
    });
}

fn bind_settings(window: &StreamTube, state: &State) {
    let (target, app) = (window.as_weak(), state.clone());
    window.on_choose_language(move |choice| {
        let Some(ui) = target.upgrade() else { return };
        let Some(language) = Language::OPTIONS.get(choice as usize).copied() else {
            return;
        };
        edit_library(&ui, &app, |library| library.language = language);
        let state = app.borrow();
        select_language(&ui, state.library.language, state.device_language);
        ui.set_screen(Screen::Settings);
    });

    let (target, app) = (window.as_weak(), state.clone());
    window.on_toggle_thumbnails(move || {
        let Some(ui) = target.upgrade() else { return };
        edit_library(&ui, &app, |library| {
            library.thumbnails = !library.thumbnails
        });
        let enabled = app.borrow().library.thumbnails;
        ui.set_thumbnails(enabled);
    });

    let target = window.as_weak();
    window.on_check_network(move || {
        let Some(ui) = target.upgrade() else { return };
        ui.set_network_busy(true);
        ui.set_network_report("".into());
        let language = current_language(&ui);
        let target = ui.as_weak();
        let _ = std::thread::Builder::new()
            .name("stream-tube-network-check".into())
            .spawn(move || {
                let report = youtube::diagnose::report(language);
                let _ = target.upgrade_in_event_loop(move |ui| {
                    ui.set_network_busy(false);
                    ui.set_network_report(report.into());
                });
            });
    });

    let target = window.as_weak();
    window.on_open_source(move || {
        if let Some(ui) = target.upgrade() {
            if rustferry::system::open_url(
                "https://github.com/ShiroKSH/Stream-Tube-Desktop/tree/android/mobile/android",
            )
            .is_err()
            {
                notify(&ui, "Could not open the source");
            }
        }
    });

    let (target, app) = (window.as_weak(), state.clone());
    window.on_toggle_extra(move |choice| {
        let Some(ui) = target.upgrade() else { return };
        edit_library(&ui, &app, |library| match choice {
            0 => library.extras.sponsorblock = !library.extras.sponsorblock,
            1 => library.extras.dearrow_titles = !library.extras.dearrow_titles,
            2 => library.extras.dearrow_thumbnails = !library.extras.dearrow_thumbnails,
            3 => library.extras.dislike = !library.extras.dislike,
            _ => {}
        });
        apply_extras(&ui, app.borrow().library.extras);
    });
}

const UPDATE_IDLE: i32 = 0;
const UPDATE_CHECKING: i32 = 1;
const UPDATE_CURRENT: i32 = 2;
const UPDATE_AVAILABLE: i32 = 3;
const UPDATE_DOWNLOADING: i32 = 4;
const UPDATE_CONFIRM: i32 = 5;
const UPDATE_FAILED: i32 = 6;

/// The release found by the last check, waiting for the user to install it.
static PENDING_UPDATE: Mutex<Option<updater::Release>> = Mutex::new(None);

fn bind_updates(window: &StreamTube) {
    window.set_app_version(updater::version_name().into());

    let target = window.as_weak();
    updater::listen(move |status, progress, message| {
        let _ = target.upgrade_in_event_loop(move |ui| match status {
            updater::status::PROGRESS => {
                ui.set_update_state(UPDATE_DOWNLOADING);
                ui.set_update_progress(progress);
            }
            updater::status::CONFIRM => ui.set_update_state(UPDATE_CONFIRM),
            updater::status::INSTALLED => ui.set_update_state(UPDATE_IDLE),
            _ => {
                ui.set_update_error(message.into());
                ui.set_update_state(UPDATE_FAILED);
            }
        });
    });

    let target = window.as_weak();
    window.on_check_updates(move || {
        if let Some(ui) = target.upgrade() {
            check_updates(&ui, true);
        }
    });

    let target = window.as_weak();
    window.on_install_update(move || {
        let Some(ui) = target.upgrade() else { return };
        let Some(release) = pending_update() else {
            check_updates(&ui, true);
            return;
        };
        ui.set_update_progress(0);
        ui.set_update_state(UPDATE_DOWNLOADING);
        if !updater::install(&release) {
            ui.set_update_error("The installer is unavailable".into());
            ui.set_update_state(UPDATE_FAILED);
        }
    });

    check_updates(window, false);
}

fn pending_update() -> Option<updater::Release> {
    PENDING_UPDATE.lock().ok().and_then(|update| update.clone())
}

/// Looks for a newer build. The startup check waits a few seconds so the first feed
/// loads first, and stays quiet when it fails.
fn check_updates(ui: &StreamTube, manual: bool) {
    let busy = [UPDATE_CHECKING, UPDATE_DOWNLOADING, UPDATE_CONFIRM];
    if busy.contains(&ui.get_update_state()) {
        return;
    }
    let current = updater::version_code();
    if current == 0 && !manual {
        return;
    }
    if manual {
        ui.set_update_state(UPDATE_CHECKING);
    }
    let target = ui.as_weak();
    let _ = std::thread::Builder::new()
        .name("stream-tube-update-check".into())
        .spawn(move || {
            if !manual {
                std::thread::sleep(std::time::Duration::from_secs(5));
            }
            let result = updater::check(current);
            let _ = target.upgrade_in_event_loop(move |ui| match result {
                Ok(Some(release)) => {
                    ui.set_update_name(release.name.clone().into());
                    if let Ok(mut pending) = PENDING_UPDATE.lock() {
                        *pending = Some(release);
                    }
                    ui.set_update_dismissed(false);
                    ui.set_update_state(UPDATE_AVAILABLE);
                }
                Ok(None) => ui.set_update_state(if manual { UPDATE_CURRENT } else { UPDATE_IDLE }),
                Err(error) if manual => {
                    ui.set_update_error(error.into());
                    ui.set_update_state(UPDATE_FAILED);
                }
                Err(_) => ui.set_update_state(UPDATE_IDLE),
            });
        });
}

fn apply_extras(ui: &StreamTube, settings: Extras) {
    ui.set_sponsorblock(settings.sponsorblock);
    ui.set_dearrow_titles(settings.dearrow_titles);
    ui.set_dearrow_thumbnails(settings.dearrow_thumbnails);
    ui.set_dislike_count(settings.dislike);
}

fn bind_account(window: &StreamTube, state: &State) {
    let (target, app) = (window.as_weak(), state.clone());
    window.on_cancel_sign_in(move || {
        let Some(ui) = target.upgrade() else { return };
        let mut state = app.borrow_mut();
        state.account_request = state.account_request.wrapping_add(1);
        ACCOUNT_REQUEST.store(state.account_request, Ordering::SeqCst);
        ui.set_account_busy(false);
        ui.set_account_code("".into());
    });

    let target = window.as_weak();
    window.on_open_activation(move || {
        let Some(ui) = target.upgrade() else { return };
        let code = ui.get_account_code();
        if !code.is_empty() && rustferry::system::open_url(account::tv::activate_url(&code)).is_err() {
            notify(&ui, "Open youtube.com/activate in a browser");
        }
    });

    let (target, app) = (window.as_weak(), state.clone());
    window.on_authorize_account(move || {
        if let Some(ui) = target.upgrade() {
            authorize(&ui, &app, true, true);
        }
    });

    let (target, app) = (window.as_weak(), state.clone());
    window.on_refresh_account(move || {
        if let Some(ui) = target.upgrade() {
            authorize(&ui, &app, true, false);
        }
    });

    let (target, app) = (window.as_weak(), state.clone());
    window.on_disconnect_account(move || {
        let Some(ui) = target.upgrade() else { return };
        {
            let mut state = app.borrow_mut();
            state.account_request = state.account_request.wrapping_add(1);
            ACCOUNT_REQUEST.store(state.account_request, Ordering::SeqCst);
            state.account_channels.clear();
        }
        account::tv::forget_token();
        account::tv::forget_access();
        edit_library(&ui, &app, |library| library.connected_account = false);
        ui.set_account_code("".into());
        ui.set_account_busy(false);
        ui.set_account_connected(false);
        ui.set_account_has_photo(false);
        ui.set_account_name("".into());
        ui.set_account_message("".into());
        if ui.get_screen() == Screen::Following {
            reload(&ui, &app);
        }
    });

    let (target, app) = (window.as_weak(), state.clone());
    window.on_account_result(move |request, status, name, photo, channels| {
        let Some(ui) = target.upgrade() else { return };
        if request != app.borrow().account_request {
            return;
        }
        ui.set_account_busy(false);
        ui.set_account_code("".into());
        if status == 0 {
            let Ok(channels) = serde_json::from_str::<Vec<Channel>>(&channels) else {
                ui.set_account_message("Could not read subscriptions".into());
                return;
            };
            let mut seen = HashSet::new();
            app.borrow_mut().account_channels = channels
                .into_iter()
                .filter(|channel| youtube::is_channel_id(&channel.id))
                .filter(|channel| seen.insert(channel.id.clone()))
                .take(500)
                .collect();
            edit_library(&ui, &app, |library| library.connected_account = true);
            ui.set_account_initial(initial(&name).into());
            ui.set_account_name(name);
            ui.set_account_photo_url(photo.clone());
            ui.set_account_has_photo(false);
            if !photo.is_empty() {
                load_images(&ui, Vec::new(), vec![photo.to_string()], None, false, Extras::default());
            }
            ui.set_account_connected(true);
            ui.set_account_message("".into());
            if matches!(ui.get_screen(), Screen::Home | Screen::Following) {
                reload(&ui, &app);
            }
            return;
        }
        if status == 5 {
            app.borrow_mut().account_channels.clear();
            account::tv::forget_token();
            account::tv::forget_access();
            edit_library(&ui, &app, |library| library.connected_account = false);
            ui.set_account_connected(false);
            ui.set_account_has_photo(false);
            ui.set_account_name("".into());
            if ui.get_screen() == Screen::Following {
                reload(&ui, &app);
            }
        }
        let message = match status {
            1 => "Sign-in cancelled",
            3 => "This build needs an OAuth registration in Google Cloud",
            4 => "Could not reach Google",
            5 => "Reconnect your YouTube account",
            6 => "This account does not have a YouTube channel yet",
            7 => "YouTube limited the requests. Try again later",
            8 => "The sign-in code expired. Try again",
            _ => "YouTube sign-in failed. Try again",
        };
        ui.set_account_message(message.into());
    });
}

fn authorize(ui: &StreamTube, state: &State, interactive: bool, choose_account: bool) {
    if ui.get_account_busy() {
        return;
    }
    let request = {
        let mut state = state.borrow_mut();
        state.account_request = state.account_request.wrapping_add(1);
        state.account_request
    };
    ACCOUNT_REQUEST.store(request, Ordering::SeqCst);
    ui.set_account_busy(true);
    ui.set_account_message("".into());
    ui.set_account_code("".into());
    let saved = account::tv::saved_token().filter(|_| !choose_account);
    let target = ui.as_weak();
    let _ = std::thread::Builder::new()
        .name("stream-tube-account".into())
        .spawn(move || {
            let outcome = match saved {
                Some(refresh) => account::tv::refresh(&refresh).and_then(|access| read_account(&access)),
                None if !interactive => Err(account::tv::Failure::Reconnect),
                None => sign_in(request, &target),
            };
            let (status, name, photo, channels) = match outcome {
                Ok((name, photo, channels)) => (0, name, photo, channels),
                Err(failure) => (failure as i32, String::new(), String::new(), "[]".to_owned()),
            };
            let _ = target.upgrade_in_event_loop(move |ui| {
                ui.invoke_account_result(request, status, name.into(), photo.into(), channels.into());
            });
        });
}

/// The sign-in attempt still wanted by the interface; a new attempt or Cancel changes it.
static ACCOUNT_REQUEST: AtomicI32 = AtomicI32::new(0);

/// Shows a code for youtube.com/activate, opens that page, and waits for approval.
fn sign_in(
    request: i32,
    target: &slint::Weak<StreamTube>,
) -> Result<(String, String, String), account::tv::Failure> {
    use account::tv::{Failure, Poll};
    let code = account::tv::start()?;
    let (shown, link) = (code.user_code.clone(), code.activate_url());
    let _ = target.upgrade_in_event_loop(move |ui| {
        ui.set_account_code(shown.into());
        let _ = rustferry::system::open_url(link);
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(code.expires_in);
    let mut interval = code.interval;
    loop {
        std::thread::sleep(std::time::Duration::from_secs(interval));
        if ACCOUNT_REQUEST.load(Ordering::SeqCst) != request {
            return Err(Failure::Cancelled);
        }
        if std::time::Instant::now() > deadline {
            return Err(Failure::Expired);
        }
        match account::tv::poll(&code) {
            Ok(Poll::Pending) | Err(Failure::Network) => {}
            Ok(Poll::SlowDown) => interval += 5,
            Ok(Poll::Approved { access_token, refresh_token }) => {
                if !account::tv::save_token(&refresh_token) {
                    return Err(Failure::Unavailable);
                }
                return read_account(&access_token);
            }
            Err(failure) => return Err(failure),
        }
    }
}

/// The account name, its picture URL and its subscriptions as the JSON the interface imports.
fn read_account(access_token: &str) -> Result<(String, String, String), account::tv::Failure> {
    account::tv::remember_access(access_token);
    let channels = account::tv::subscriptions(access_token)?;
    let (name, photo) = account::tv::account_profile(access_token);
    let channels = serde_json::to_string(&channels).map_err(|_| account::tv::Failure::Unavailable)?;
    Ok((name, photo, channels))
}

fn following_channels(state: &AppState) -> Vec<Channel> {
    let mut ids = std::collections::HashSet::new();
    state
        .library
        .following
        .iter()
        .chain(&state.account_channels)
        .filter(|channel| ids.insert(channel.id.as_str()))
        .cloned()
        .collect()
}

fn reload(ui: &StreamTube, state: &State) {
    let screen = ui.get_screen();
    let (feed, empty) = {
        let state = state.borrow();
        state.request.fetch_add(1, Ordering::Relaxed);
        *state.next_page.lock().expect("feed cursor is available") = None;
        let account = state.library.connected_account && account::tv::saved_token().is_some();
        let feed = match screen {
            Screen::Home if ui.get_category() == 0 => {
                let channels = following_channels(&state);
                let interests = Interests::new(
                    &state.library.history,
                    &state.library.recent_searches,
                    channels.clone(),
                );
                ui.set_personal_home(account || interests.available());
                let anonymous = if interests.available() {
                    Feed::ForYou(interests)
                } else {
                    Feed::Home(Category::All)
                };
                Some(personal(account, TvPage::Home, anonymous, channels))
            }
            Screen::Home => {
                let category = match ui.get_category() {
                    1 => Category::Music,
                    2 => Category::Games,
                    3 => Category::News,
                    4 => Category::Live,
                    _ => Category::All,
                };
                Some(personal(account, TvPage::topic(category), Feed::Home(category), following_channels(&state)))
            }
            Screen::Shorts => Some(personal(account, TvPage::Shorts, Feed::Shorts, Vec::new())),
            Screen::Following if !ui.get_channels_mode() => {
                let channels = following_channels(&state);
                if account {
                    let anonymous = if channels.is_empty() {
                        Feed::Local(Vec::new())
                    } else {
                        Feed::Following(channels.clone())
                    };
                    Some(personal(true, TvPage::Subscriptions, anonymous, channels))
                } else {
                    (!channels.is_empty()).then_some(Feed::Following(channels))
                }
            }
            Screen::Library => Some(Feed::Local(if ui.get_history_mode() {
                state.library.history.clone()
            } else {
                state.library.saved.clone()
            })),
            Screen::Search if !state.query.is_empty() && !ui.get_search_editing() => {
                Some(Feed::Search(state.query.clone()))
            }
            Screen::Channel => state.channel.clone().map(Feed::Channel),
            Screen::Playlist => state.playlist.clone().map(Feed::Playlist),
            _ => None,
        };
        if screen == Screen::Following {
            let channels = following_channels(&state);
            let urls = channels
                .iter()
                .map(|channel| channel.avatar.clone())
                .collect::<Vec<_>>();
            let rows = channels.iter().map(channel_row).collect::<Vec<_>>();
            set_rows(&ui.get_channels(), rows);
            if state.library.thumbnails {
                load_images(ui, Vec::new(), urls, None, false, Extras::default());
            }
        }
        if screen == Screen::You {
            let thumbnails = state.library.thumbnails;
            show_previews(ui, &state.library.history, &state.library.saved, thumbnails);
        }
        let empty = match screen {
            Screen::Following => "Open a channel under a video to follow it",
            Screen::Library if ui.get_history_mode() => "Videos you open will appear here",
            Screen::Library => "Save a video from any feed",
            Screen::Search if state.query.is_empty() => "Search by video or channel name",
            _ => "No videos found",
        };
        if let Some(Feed::Channel(channel)) = &feed {
            ui.set_channel_name(channel.name.clone().into());
            ui.set_channel_initial(initial(&channel.name).into());
            ui.set_channel_avatar_url(channel.avatar.clone().into());
            match cached_avatar(&channel.avatar) {
                Some(pixels) => {
                    ui.set_channel_avatar(slint::Image::from_rgba8(pixels));
                    ui.set_channel_has_avatar(true);
                }
                None => {
                    ui.set_channel_has_avatar(false);
                    if state.library.thumbnails && !channel.avatar.is_empty() {
                        let urls = vec![channel.avatar.clone()];
                        load_images(ui, Vec::new(), urls, None, false, Extras::default());
                    }
                }
            }
            ui.set_channel_followed(state.library.is_following(&channel.id));
            ui.set_channel_remote(
                state
                    .account_channels
                    .iter()
                    .any(|remote| remote.id == channel.id),
            );
        }
        (feed, empty)
    };

    if screen == Screen::Search {
        show_recent_searches(ui, state);
    }
    replace_rows(ui, Vec::new());
    ui.set_failed(false);
    ui.set_busy(false);
    ui.set_has_more(false);
    ui.set_loading_more(false);
    ui.set_load_more_failed(false);
    ui.set_message(if matches!(screen, Screen::Settings | Screen::You) {
        "".into()
    } else {
        empty.into()
    });
    let Some(feed) = feed else { return };
    ui.set_busy(true);
    ui.set_message("Loading…".into());

    if !spawn_feed(ui, state, feed, empty) {
        feed_error(ui);
    }
}

fn load_more(ui: &StreamTube, state: &State) -> bool {
    if ui.get_loading_more() {
        return false;
    }
    let cursor = state
        .borrow()
        .next_page
        .lock()
        .expect("feed cursor is available")
        .clone();
    let Some(cursor) = cursor else { return false };
    ui.set_loading_more(true);
    ui.set_load_more_failed(false);
    if !spawn_feed(ui, state, Feed::Next(cursor), "") {
        ui.set_loading_more(false);
        ui.set_load_more_failed(true);
        ui.invoke_page_appended(true);
    }
    true
}

fn spawn_feed(ui: &StreamTube, state: &State, feed: Feed, empty: &'static str) -> bool {
    let state = state.borrow();
    let transaction = FeedTransaction {
        target: ui.as_weak(),
        request: state.request.clone(),
        generation: state.request.load(Ordering::Relaxed),
        next_page: state.next_page.clone(),
        thumbnails: state.library.thumbnails && ui.get_screen() != Screen::Shorts,
        extras: state.library.extras,
        saved: state
            .library
            .saved
            .iter()
            .map(|video| video.id.clone())
            .collect(),
        language: current_language(ui),
        empty,
    };
    let result = std::thread::Builder::new()
        .name("stream-tube-feed".into())
        .spawn(move || {
            fetch_feed(feed, transaction);
        });
    result.is_ok()
}

struct FeedTransaction {
    target: slint::Weak<StreamTube>,
    request: Arc<AtomicU64>,
    generation: u64,
    next_page: Arc<Mutex<Option<FeedCursor>>>,
    thumbnails: bool,
    extras: Extras,
    saved: HashSet<String>,
    language: Language,
    empty: &'static str,
}

fn fetch_feed(feed: Feed, transaction: FeedTransaction) {
    let FeedTransaction {
        target,
        request,
        generation,
        next_page,
        thumbnails,
        extras,
        saved,
        language,
        empty,
    } = transaction;
    let client = Client::new(language);
    let append = matches!(feed, Feed::Next(_));
    let shorts = matches!(feed, Feed::Shorts | Feed::Account(TvPage::Shorts, ..));
    let account_feed = matches!(feed, Feed::Account(..) | Feed::Next(FeedCursor::Account(..)));
    // Shorts that came with the account's page; they fill the Shorts shelf
    let mut account_shorts = Vec::new();
    let mut account_page = None;
    let feed = match feed {
        Feed::Account(page, anonymous, channels) => match account_feed_page(page, None, &channels) {
            Some((found, shorts)) => {
                account_shorts = shorts;
                account_page = Some(found);
                Feed::Local(Vec::new())
            }
            None => *anonymous,
        },
        feed => feed,
    };
    // the first page of the main Home feed gets a Shorts shelf, fetched alongside it
    let shelf = if matches!(feed, Feed::ForYou(_) | Feed::Home(Category::All)) {
        std::thread::Builder::new()
            .name("stream-tube-shorts-shelf".into())
            .spawn(move || Client::new(language).shorts())
            .ok()
    } else {
        None
    };
    let page = match feed {
        _ if account_page.is_some() => account_page,
        Feed::Home(category) => client.home(category).map(FeedPage::from),
        Feed::ForYou(interests) => interests.load(language).map(FeedPage::from),
        Feed::Account(..) => None,
        Feed::Shorts => client.shorts().map(FeedPage::from),
        Feed::Following(channels) => follow_feed(language, &channels, &request, generation),
        Feed::Channel(channel) => client.channel(&channel).map(FeedPage::from),
        Feed::Search(query) => client.search(&query, false).map(FeedPage::from),
        Feed::Playlist(id) => client.playlist(&id).map(FeedPage::from),
        Feed::Local(videos) => Some(FeedPage { videos, next: None }),
        Feed::Next(FeedCursor::YouTube(cursor)) => client.next_page(cursor).map(FeedPage::from),
        Feed::Next(FeedCursor::Following(channels)) => {
            follow_feed(language, &channels, &request, generation)
        }
        Feed::Next(FeedCursor::Account(page, token, channels)) => {
            account_feed_page(page, Some(&token), &channels).map(|(page, _)| page)
        }
    };
    let mut ids = page
        .iter()
        .flat_map(|page| &page.videos)
        .map(|video| video.id.clone())
        .collect::<Vec<_>>();
    let mut avatars = page
        .iter()
        .flat_map(|page| &page.videos)
        .map(|video| video.avatar.clone())
        .collect::<Vec<_>>();
    // videos of the account's Home from channels it does not follow, one per channel
    let mut faceless = Vec::new();
    if account_feed {
        let mut names = HashSet::new();
        for video in page.iter().flat_map(|page| &page.videos) {
            let name = video.channel.name.trim();
            if !video.short && video.avatar.is_empty() && !name.is_empty() && names.insert(name.to_owned()) {
                faceless.push((name.to_owned(), video.id.clone()));
            }
        }
    }
    let check = request.clone();
    let shelf_saved = saved.clone();
    if target
        .upgrade_in_event_loop(move |ui| {
            if check.load(Ordering::Relaxed) != generation {
                return;
            }
            let Some(page) = page else {
                if append {
                    ui.set_loading_more(false);
                    ui.set_load_more_failed(true);
                    ui.invoke_page_appended(true);
                } else {
                    feed_error(&ui);
                }
                return;
            };
            let more = page.next.is_some();
            *next_page.lock().expect("feed cursor is available") = page.next;
            ui.set_has_more(more);
            if append {
                let appended = append_rows(&ui, page.videos, &saved);
                let missed = appended == 0 && more;
                ui.set_load_more_failed(missed);
                ui.set_loading_more(false);
                ui.invoke_page_appended(missed);
            } else {
                let available = !page.videos.is_empty();
                ui.set_message(if available { "".into() } else { empty.into() });
                replace_rows(
                    &ui,
                    page.videos
                        .iter()
                        .filter(|video| !is_hidden(&video.id))
                        .map(|video| video_row(video, saved.contains(&video.id)))
                        .collect(),
                );
                set_rows(&ui.get_shorts_shelf(), Vec::new());
                ui.set_busy(false);
                if available && shorts {
                    ui.invoke_open_shorts();
                }
            }
        })
        .is_err()
        || shorts
    {
        return;
    }

    let shelf_videos = if account_shorts.len() >= 3 {
        Some(account_shorts)
    } else {
        shelf.map(|shelf| shelf.join().ok().flatten().map(|page| page.videos).unwrap_or_default())
    };
    if let Some(videos) = shelf_videos {
        let videos = videos
            .into_iter()
            .filter(|video| !is_hidden(&video.id))
            .take(12)
            .collect::<Vec<_>>();
        if videos.len() >= 3 && request.load(Ordering::Relaxed) == generation {
            ids.extend(videos.iter().map(|video| video.id.clone()));
            let marks = videos
                .iter()
                .map(|video| shelf_saved.contains(&video.id))
                .collect::<Vec<_>>();
            let check = request.clone();
            let _ = target.upgrade_in_event_loop(move |ui| {
                if check.load(Ordering::Relaxed) != generation {
                    return;
                }
                let rows = videos
                    .iter()
                    .zip(marks)
                    .map(|(video, saved)| video_row(video, saved))
                    .collect::<Vec<_>>();
                let model = ui.get_videos();
                let Some(feed) = model.as_any().downcast_ref::<VecModel<VideoRow>>() else {
                    return;
                };
                if feed.row_count() < 2 {
                    return;
                }
                set_rows(&ui.get_shorts_shelf(), rows);
                feed.insert(
                    feed.row_count().min(3),
                    VideoRow {
                        id: SHELF_ID.into(),
                        kind: 1,
                        ..Default::default()
                    },
                );
            });
        }
    }

    if thumbnails && !faceless.is_empty() {
        find_owners(target.clone(), faceless, (request.clone(), generation), language, extras);
    }
    if !thumbnails && !extras.dearrow_titles {
        return;
    }
    if !thumbnails {
        avatars.clear();
    }
    fetch_images(target, ids, avatars, Some((request, generation)), thumbnails, extras);
}

/// Channels of the account's Home, by name, found from one of their videos.
static OWNERS: LazyLock<Mutex<HashMap<String, youtube::watch::Owner>>> = LazyLock::new(Mutex::default);

/// The TV reply names a video's channel but gives no picture or id. For channels the
/// account does not follow, both are read from one video's watch page, a few at a
/// time, after the feed is on screen; each channel is looked up once per session.
fn find_owners(
    target: slint::Weak<StreamTube>,
    faceless: Vec<(String, String)>,
    check: (Arc<AtomicU64>, u64),
    language: Language,
    extras: Extras,
) {
    let _ = std::thread::Builder::new()
        .name("stream-tube-owners".into())
        .spawn(move || {
            let current = |check: &(Arc<AtomicU64>, u64)| check.0.load(Ordering::Relaxed) == check.1;
            let mut known = Vec::new();
            let mut lookups = Vec::new();
            for (name, id) in faceless.into_iter().take(24) {
                match OWNERS.lock().ok().and_then(|owners| owners.get(&name).cloned()) {
                    Some(owner) => known.push((name, owner)),
                    None => lookups.push((name, id)),
                }
            }
            show_owners(&target, known, &check, extras);
            for batch in lookups.chunks(4) {
                if !current(&check) {
                    return;
                }
                let found: Vec<(String, youtube::watch::Owner)> = std::thread::scope(|scope| {
                    let tasks: Vec<_> = batch
                        .iter()
                        .map(|(name, id)| {
                            scope.spawn(move || {
                                youtube::watch::details(id, language)
                                    .map(|details| details.channel)
                                    .filter(|owner| !owner.avatar.is_empty())
                                    .map(|owner| (name.clone(), owner))
                            })
                        })
                        .collect();
                    tasks.into_iter().filter_map(|task| task.join().ok().flatten()).collect()
                });
                if let Ok(mut owners) = OWNERS.lock() {
                    for (name, owner) in &found {
                        owners.insert(name.clone(), owner.clone());
                    }
                }
                show_owners(&target, found, &check, extras);
            }
        });
}

/// Gives feed rows of these channels their id and picture, then loads the pictures.
fn show_owners(
    target: &slint::Weak<StreamTube>,
    owners: Vec<(String, youtube::watch::Owner)>,
    check: &(Arc<AtomicU64>, u64),
    extras: Extras,
) {
    if owners.is_empty() {
        return;
    }
    let check = check.clone();
    let _ = target.upgrade_in_event_loop(move |ui| {
        if check.0.load(Ordering::Relaxed) != check.1 {
            return;
        }
        let model = ui.get_videos();
        let mut avatars = Vec::new();
        for index in 0..model.row_count() {
            let Some(mut row) = model.row_data(index) else { continue };
            if !row.avatar_url.is_empty() {
                continue;
            }
            let Some((_, owner)) = owners.iter().find(|(name, _)| row.channel.as_str().trim() == name) else {
                continue;
            };
            if row.channel_id.is_empty() {
                row.channel_id = owner.id.clone().into();
            }
            row.avatar_url = owner.avatar.clone().into();
            avatars.push(owner.avatar.clone());
            model.set_row_data(index, row);
        }
        if !avatars.is_empty() {
            load_images(&ui, Vec::new(), avatars, Some(check), true, extras);
        }
    });
}

/// A page of one of the account's pages and the Shorts that came with it, or None
/// to fall back to the anonymous feed. The TV reply names channels without
/// pictures, so videos from channels the account follows get those channels'
/// ids and pictures.
fn account_feed_page(
    source: TvPage,
    continuation: Option<&str>,
    channels: &Arc<Vec<Channel>>,
) -> Option<(FeedPage, Vec<Video>)> {
    let token = account::tv::access_token().ok()?;
    let page = match source {
        TvPage::Home => account::tv::browse(&token, account::tv::HOME, continuation),
        TvPage::Topic(id) => account::tv::browse(&token, id, continuation),
        TvPage::Subscriptions => account::tv::subscription_feed(&token, continuation),
        TvPage::Shorts => account::tv::shorts(&token, continuation),
    }
    .ok()?;
    if continuation.is_none() && page.videos.is_empty() {
        return None;
    }
    let by_name: HashMap<String, &Channel> = channels
        .iter()
        .map(|channel| (channel.name.trim().to_lowercase(), channel))
        .collect();
    let by_id: HashMap<&str, &Channel> = channels.iter().map(|channel| (channel.id.as_str(), channel)).collect();
    let enrich = |mut video: Video| {
        let known = by_id
            .get(video.channel.id.as_str())
            .or_else(|| by_name.get(&video.channel.name.trim().to_lowercase()))
            .copied();
        if let Some(channel) = known {
            video.channel.id = channel.id.clone();
            video.avatar = channel.avatar.clone();
            video.channel.avatar = channel.avatar.clone();
        }
        video
    };
    let videos = page.videos.into_iter().map(enrich).collect();
    let shorts = page.shorts.into_iter().map(enrich).collect();
    let next = page.next.map(|token| FeedCursor::Account(source, token, channels.clone()));
    Some((FeedPage { videos, next }, shorts))
}

/// The account's page when one is connected, else the anonymous feed.
fn personal(account: bool, page: TvPage, anonymous: Feed, channels: Vec<Channel>) -> Feed {
    if account {
        Feed::Account(page, Box::new(anonymous), Arc::new(channels))
    } else {
        anonymous
    }
}

/// Placeholder row the feed shows as the Shorts shelf.
const SHELF_ID: &str = "#shorts-shelf";

/// Videos hidden with "Not interested" during this session.
static HIDDEN: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Mutex::default);

fn is_hidden(id: &str) -> bool {
    HIDDEN.lock().is_ok_and(|hidden| hidden.contains(id))
}

/// Channel avatars already downloaded, by URL. They are small, so they stay for the session.
static AVATARS: LazyLock<Mutex<HashMap<String, SharedPixelBuffer<Rgba8Pixel>>>> =
    LazyLock::new(Mutex::default);

fn cached_avatar(url: &str) -> Option<SharedPixelBuffer<Rgba8Pixel>> {
    if url.is_empty() {
        return None;
    }
    AVATARS.lock().ok()?.get(url).cloned()
}

enum ImageJob {
    Thumbnail(String),
    Avatar(String),
}

fn load_images(
    ui: &StreamTube,
    ids: Vec<String>,
    avatars: Vec<String>,
    check: Option<(Arc<AtomicU64>, u64)>,
    thumbnails: bool,
    extras: Extras,
) {
    let target = ui.as_weak();
    let _ = std::thread::Builder::new()
        .name("stream-tube-images".into())
        .spawn(move || fetch_images(target, ids, avatars, check, thumbnails, extras));
}

/// Downloads thumbnails (with DeArrow branding) and channel avatars on a few threads,
/// and puts each picture into every list that shows it.
fn fetch_images(
    target: slint::Weak<StreamTube>,
    ids: Vec<String>,
    avatars: Vec<String>,
    check: Option<(Arc<AtomicU64>, u64)>,
    thumbnails: bool,
    extras: Extras,
) {
    let mut seen = HashSet::new();
    let mut jobs = Vec::new();
    for url in avatars {
        if url.is_empty() || !seen.insert(url.clone()) {
            continue;
        }
        match cached_avatar(&url) {
            Some(pixels) => {
                let _ = target.upgrade_in_event_loop(move |ui| apply_avatar(&ui, &url, pixels));
            }
            None => jobs.push(ImageJob::Avatar(url)),
        }
    }
    // interleave so the first cards get both their picture and their avatar early
    let mut thumbs = ids
        .into_iter()
        .filter(|id| id != SHELF_ID && seen.insert(id.clone()))
        .map(ImageJob::Thumbnail)
        .collect::<Vec<_>>();
    let avatar_jobs = std::mem::take(&mut jobs);
    let mut avatar_jobs = avatar_jobs.into_iter();
    for thumb in thumbs.drain(..) {
        jobs.push(thumb);
        if let Some(avatar) = avatar_jobs.next() {
            jobs.push(avatar);
        }
    }
    jobs.extend(avatar_jobs);

    let current = || {
        check
            .as_ref()
            .is_none_or(|(request, generation)| request.load(Ordering::Relaxed) == *generation)
    };
    let next = AtomicUsize::new(0);
    let (loaded, missed) = (AtomicUsize::new(0), AtomicUsize::new(0));
    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| {
                let http = youtube::thumbnail_agent();
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(job) = jobs.get(index) else { break };
                    if !current() {
                        break;
                    }
                    match job {
                        ImageJob::Avatar(url) => {
                            let Some(rgba) = youtube::avatar(&http, url) else {
                                continue;
                            };
                            let pixels = SharedPixelBuffer::clone_from_slice(
                                rgba.as_raw(),
                                rgba.width(),
                                rgba.height(),
                            );
                            if let Ok(mut cache) = AVATARS.lock() {
                                if cache.len() > 600 {
                                    cache.clear();
                                }
                                cache.insert(url.clone(), pixels.clone());
                            }
                            let url = url.clone();
                            let _ = target
                                .upgrade_in_event_loop(move |ui| apply_avatar(&ui, &url, pixels));
                        }
                        ImageJob::Thumbnail(id) => {
                            let branding = if extras.dearrow_titles
                                || (thumbnails && extras.dearrow_thumbnails)
                            {
                                services::branding(id).unwrap_or_default()
                            } else {
                                services::Branding::default()
                            };
                            let alternate = branding
                                .thumbnail_time
                                .filter(|_| extras.dearrow_thumbnails)
                                .and_then(|time| services::branded_thumbnail(id, time));
                            let pixels = thumbnails
                                .then(|| youtube::thumbnail(&http, id, alternate))
                                .flatten();
                            if thumbnails {
                                let counter = if pixels.is_some() { &loaded } else { &missed };
                                counter.fetch_add(1, Ordering::Relaxed);
                            }
                            let pixels = pixels
                                .map(|rgba| {
                                    SharedPixelBuffer::clone_from_slice(
                                        rgba.as_raw(),
                                        rgba.width(),
                                        rgba.height(),
                                    )
                                });
                            let title = branding.title.filter(|_| extras.dearrow_titles);
                            if pixels.is_none() && title.is_none() {
                                continue;
                            }
                            let id = id.clone();
                            let _ = target.upgrade_in_event_loop(move |ui| {
                                apply_thumbnail(&ui, &id, pixels, title)
                            });
                        }
                    }
                }
            });
        }
    });
    // say why when none of a feed's pictures arrived, so a screenshot shows the cause
    let (loaded, missed) = (loaded.into_inner(), missed.into_inner());
    if check.is_some() && loaded == 0 && missed >= 3 && current() {
        let error = youtube::last_image_error();
        let _ = target.upgrade_in_event_loop(move |ui| {
            notify(&ui, &format!("Thumbnails did not load ({missed}): {error}"));
        });
    }
}

fn video_models(ui: &StreamTube) -> [ModelRc<VideoRow>; 4] {
    [
        ui.get_videos(),
        ui.get_shorts_shelf(),
        ui.get_history_preview(),
        ui.get_saved_preview(),
    ]
}

fn apply_thumbnail(
    ui: &StreamTube,
    id: &str,
    pixels: Option<SharedPixelBuffer<Rgba8Pixel>>,
    title: Option<String>,
) {
    let image = pixels.map(slint::Image::from_rgba8);
    for model in video_models(ui) {
        for index in 0..model.row_count() {
            let Some(mut row) = model.row_data(index) else {
                continue;
            };
            if row.id != id {
                continue;
            }
            if let Some(image) = &image {
                row.thumbnail = image.clone();
                row.has_thumbnail = true;
            }
            if let Some(title) = &title {
                row.title = title.into();
            }
            model.set_row_data(index, row);
        }
    }
}

fn apply_avatar(ui: &StreamTube, url: &str, pixels: SharedPixelBuffer<Rgba8Pixel>) {
    let image = slint::Image::from_rgba8(pixels);
    for model in video_models(ui) {
        for index in 0..model.row_count() {
            let Some(mut row) = model.row_data(index) else {
                continue;
            };
            if row.avatar_url == url && !row.has_avatar {
                row.avatar = image.clone();
                row.has_avatar = true;
                model.set_row_data(index, row);
            }
        }
    }
    let channels = ui.get_channels();
    for index in 0..channels.row_count() {
        let Some(mut row) = channels.row_data(index) else {
            continue;
        };
        if row.avatar_url == url && !row.has_avatar {
            row.avatar = image.clone();
            row.has_avatar = true;
            channels.set_row_data(index, row);
        }
    }
    if ui.get_account_photo_url() == url {
        ui.set_account_photo(image.clone());
        ui.set_account_has_photo(true);
    }
    if ui.get_channel_avatar_url() == url {
        ui.set_channel_avatar(image);
        ui.set_channel_has_avatar(true);
    }
}

fn follow_feed(
    language: Language,
    channels: &[Channel],
    request: &AtomicU64,
    generation: u64,
) -> Option<FeedPage> {
    let (channels, remaining) = channels.split_at(channels.len().min(12));
    let mut result = Vec::new();
    let mut received = false;
    for batch in channels.chunks(6) {
        if request.load(Ordering::Relaxed) != generation {
            return None;
        }
        std::thread::scope(|scope| {
            let feeds = batch
                .iter()
                .map(|channel| scope.spawn(move || Client::new(language).channel(channel)))
                .collect::<Vec<_>>();
            let mut recent = Vec::new();
            for feed in feeds {
                if let Ok(Some(page)) = feed.join() {
                    received = true;
                    recent.push(page.videos.into_iter());
                }
            }
            for _ in 0..4 {
                result.extend(recent.iter_mut().filter_map(Iterator::next));
            }
        });
    }
    received.then(|| FeedPage {
        videos: result,
        next: (!remaining.is_empty()).then(|| FeedCursor::Following(remaining.to_vec())),
    })
}

fn replace_rows(ui: &StreamTube, rows: Vec<VideoRow>) {
    set_rows(&ui.get_videos(), rows);
}

fn set_rows<T: Clone + 'static>(model: &ModelRc<T>, rows: Vec<T>) {
    if let Some(model) = model.as_any().downcast_ref::<VecModel<T>>() {
        model.set_vec(rows);
    }
}

fn append_rows(ui: &StreamTube, videos: Vec<Video>, saved: &HashSet<String>) -> usize {
    let model = ui.get_videos();
    let Some(rows) = model.as_any().downcast_ref::<VecModel<VideoRow>>() else {
        return 0;
    };
    let mut ids: HashSet<_> = model.iter().map(|row| row.id).collect();
    let new: Vec<_> = videos
        .into_iter()
        .filter(|video| !is_hidden(&video.id))
        .filter_map(|video| {
            let row = video_row(&video, saved.contains(&video.id));
            ids.insert(row.id.clone()).then_some(row)
        })
        .collect();
    let count = new.len();
    rows.extend(new);
    count
}

fn video_row(video: &Video, saved: bool) -> VideoRow {
    let cached = cached_avatar(&video.avatar);
    VideoRow {
        id: video.id.clone().into(),
        title: video.title.clone().into(),
        channel: video.channel.name.clone().into(),
        channel_id: video.channel.id.clone().into(),
        duration: video.duration.clone().into(),
        views: video.views.clone().into(),
        published: video.published.clone().into(),
        saved,
        short: video.short,
        initial: initial(&video.channel.name).into(),
        avatar_url: video.avatar.clone().into(),
        avatar: cached
            .clone()
            .map(slint::Image::from_rgba8)
            .unwrap_or_default(),
        has_avatar: cached.is_some(),
        ..Default::default()
    }
}

fn channel_row(channel: &Channel) -> ChannelRow {
    let cached = cached_avatar(&channel.avatar);
    ChannelRow {
        id: channel.id.clone().into(),
        name: channel.name.clone().into(),
        initial: initial(&channel.name).into(),
        avatar_url: channel.avatar.clone().into(),
        avatar: cached
            .clone()
            .map(slint::Image::from_rgba8)
            .unwrap_or_default(),
        has_avatar: cached.is_some(),
    }
}

/// The first letter of a name, for the circle shown until a picture loads.
fn initial(name: &str) -> String {
    name.trim()
        .trim_start_matches('@')
        .chars()
        .next()
        .map(|letter| letter.to_uppercase().collect())
        .unwrap_or_default()
}

/// The avatar of a channel as some list on screen knows it.
fn channel_avatar(ui: &StreamTube, state: &AppState, id: &str) -> String {
    ui.get_videos()
        .iter()
        .find(|row| row.channel_id == id && !row.avatar_url.is_empty())
        .map(|row| row.avatar_url.to_string())
        .or_else(|| {
            following_channels(state)
                .into_iter()
                .find(|channel| channel.id == id && !channel.avatar.is_empty())
                .map(|channel| channel.avatar)
        })
        .unwrap_or_default()
}

fn show_previews(ui: &StreamTube, history: &[Video], saved: &[Video], thumbnails: bool) {
    let mut ids = Vec::new();
    let mut avatars = Vec::new();
    let mut rows = |videos: &[Video]| {
        videos
            .iter()
            .take(12)
            .map(|video| {
                ids.push(video.id.clone());
                avatars.push(video.avatar.clone());
                video_row(video, false)
            })
            .collect::<Vec<_>>()
    };
    let (history, saved) = (rows(history), rows(saved));
    set_rows(&ui.get_history_preview(), history);
    set_rows(&ui.get_saved_preview(), saved);
    if thumbnails {
        load_images(ui, ids, avatars, None, true, Extras::default());
    }
}

fn show_recent_searches(ui: &StreamTube, state: &State) {
    let recent = state
        .borrow()
        .library
        .recent_searches
        .iter()
        .map(|query| slint::SharedString::from(query.as_str()))
        .collect::<Vec<_>>();
    set_rows(&ui.get_recent_searches(), recent);
}

/// Latest text typed in the search box; older suggestion requests give up.
static SUGGESTION_REQUEST: AtomicU64 = AtomicU64::new(0);

fn request_suggestions(ui: &StreamTube, text: String) {
    let request = SUGGESTION_REQUEST.fetch_add(1, Ordering::SeqCst) + 1;
    if text.trim().is_empty() {
        set_rows(&ui.get_suggestions(), Vec::new());
        return;
    }
    let language = current_language(ui);
    let target = ui.as_weak();
    let _ = std::thread::Builder::new()
        .name("stream-tube-suggestions".into())
        .spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(160));
            if SUGGESTION_REQUEST.load(Ordering::SeqCst) != request {
                return;
            }
            let mut found = youtube::suggestions(&text, language);
            if found.is_empty() {
                found.push(text.trim().to_owned());
            }
            let _ = target.upgrade_in_event_loop(move |ui| {
                if SUGGESTION_REQUEST.load(Ordering::SeqCst) != request {
                    return;
                }
                let rows = found.into_iter().map(slint::SharedString::from).collect();
                set_rows(&ui.get_suggestions(), rows);
            });
        });
}

/// Shows a short message in the snackbar, again even when the text repeats.
fn notify(ui: &StreamTube, text: &str) {
    ui.set_notice(text.into());
    ui.set_notice_hidden(false);
}

fn find_video(ui: &StreamTube, id: &str) -> Option<Video> {
    ui.get_videos()
        .iter()
        .find(|row| row.kind == 0 && row.id == id)
        .map(video_from_row)
}

fn feed_videos(ui: &StreamTube) -> Vec<Video> {
    ui.get_videos()
        .iter()
        .filter(|row| row.kind == 0)
        .map(video_from_row)
        .collect()
}

fn video_from_row(row: VideoRow) -> Video {
    Video {
        id: row.id.to_string(),
        title: row.title.to_string(),
        channel: Channel {
            id: row.channel_id.to_string(),
            name: row.channel.to_string(),
            avatar: row.avatar_url.to_string(),
        },
        duration: row.duration.to_string(),
        views: row.views.to_string(),
        published: row.published.to_string(),
        short: row.short,
        avatar: row.avatar_url.to_string(),
    }
}

fn open_player(
    ui: &StreamTube,
    state: &State,
    videos: Vec<Video>,
    first: usize,
    shorts: bool,
    feed: bool,
) {
    let has_more = feed && ui.get_has_more();
    let (number, tracks) = {
        let mut state = state.borrow_mut();
        state.playback_number = state.playback_number.wrapping_add(1);
        let number = state.playback_number;
        let tracks = player::tracks(&videos, |id| state.library.is_saved(id));
        state.playback = Some(Playback {
            number,
            videos,
            shorts,
            feed_generation: feed.then(|| state.request.load(Ordering::Relaxed)),
        });
        (number, tracks)
    };
    if player::open(
        number,
        &tracks,
        first,
        current_language(ui),
        shorts,
        has_more,
    )
    .is_err()
    {
        state.borrow_mut().playback = None;
        notify(&ui, "Could not open the player");
    }
}

fn load_video_description(ui: &StreamTube, number: i32, id: String) {
    let language = current_language(ui);
    let target = ui.as_weak();
    let _ = std::thread::Builder::new()
        .name("stream-tube-video-details".into())
        .spawn(move || {
            let packet = youtube::watch::details(&id, language)
                .and_then(|details| serde_json::to_string(&details).ok())
                .unwrap_or_else(|| r#"{"error":true}"#.to_owned());
            let _ = target.upgrade_in_event_loop(move |_| {
                player::set_details(number, &id, &packet);
            });
        });
}

fn load_playback_extras(ui: &StreamTube, number: i32, id: String, settings: Extras) {
    if !(settings.sponsorblock || settings.dearrow_titles || settings.dislike) {
        return;
    }
    let target = ui.as_weak();
    let _ = std::thread::Builder::new()
        .name("stream-tube-playback-extras".into())
        .spawn(move || {
            let extras = services::playback(&id, settings);
            if let Ok(packet) = serde_json::to_string(&extras) {
                let _ = target.upgrade_in_event_loop(move |_| {
                    player::set_extras(number, &id, &packet);
                });
            }
        });
}

fn load_stream(ui: &StreamTube, number: i32, id: String) {
    let language = current_language(ui);
    let target = ui.as_weak();
    let _ = std::thread::Builder::new()
        .name("stream-tube-stream".into())
        .spawn(move || {
            let packet = match youtube::watch::stream(&id, language) {
                Ok(stream) => serde_json::to_string(&stream).unwrap_or_default(),
                Err(error) => serde_json::json!({ "error": error }).to_string(),
            };
            let _ = target.upgrade_in_event_loop(move |_| {
                player::set_stream(number, &id, &packet);
            });
        });
}

fn load_comments(ui: &StreamTube, number: i32, id: String, token: String) {
    let language = current_language(ui);
    let target = ui.as_weak();
    let _ = std::thread::Builder::new()
        .name("stream-tube-comments".into())
        .spawn(move || {
            let page = youtube::watch::comments(&token, language);
            let packet = youtube::watch::comments_packet(&token, page.as_ref());
            let _ = target.upgrade_in_event_loop(move |_| {
                player::set_comments(number, &id, &packet);
            });
        });
}

/// A related video the watch page started: it joins the playback so saving,
/// history and the channel link work for it like for queue videos.
fn add_related_video(state: &State, number: i32, packet: &str) {
    let Some(video) = serde_json::from_str::<youtube::watch::Related>(packet)
        .ok()
        .and_then(youtube::watch::Related::into_video)
    else {
        return;
    };
    let saved = {
        let mut state = state.borrow_mut();
        let state = &mut *state;
        let saved = state.library.is_saved(&video.id);
        let Some(playback) = state
            .playback
            .as_mut()
            .filter(|player| player.number == number)
        else {
            return;
        };
        if !playback.videos.iter().any(|known| known.id == video.id) {
            playback.videos.push(video.clone());
        }
        saved
    };
    player::update_saved(number, &video.id, saved);
}

/// `channelId\nchannelName` from the player.
fn player_channel(packet: &str) -> Option<Channel> {
    let (id, name) = packet.split_once('\n').unwrap_or((packet, ""));
    youtube::is_channel_id(id).then(|| Channel {
        id: id.to_owned(),
        name: name.to_owned(),
        ..Channel::default()
    })
}

/// Whether the channel is followed, after toggling it when asked. Channels the
/// connected account subscribes to are followed through the account and stay so.
fn player_following(ui: &StreamTube, state: &State, channel: Channel, toggle: bool) -> bool {
    let id = channel.id.clone();
    if state
        .borrow()
        .account_channels
        .iter()
        .any(|remote| remote.id == id)
    {
        return true;
    }
    if toggle {
        edit_library(ui, state, |library| library.toggle_following(channel));
    }
    let following = state.borrow().library.is_following(&id);
    if toggle
        && state
            .borrow()
            .channel
            .as_ref()
            .is_some_and(|open| open.id == id)
    {
        ui.set_channel_followed(following);
    }
    following
}

fn save_video(ui: &StreamTube, state: &State, video: Video) -> bool {
    let id = video.id.clone();
    edit_library(ui, state, |library| library.toggle_saved(video));
    let saved = state.borrow().library.is_saved(&id);
    let model = ui.get_videos();
    if let Some((index, mut row)) = model.iter().enumerate().find(|(_, row)| row.id == id) {
        if !saved && ui.get_screen() == Screen::Library && !ui.get_history_mode() {
            if let Some(rows) = model.as_any().downcast_ref::<VecModel<VideoRow>>() {
                rows.remove(index);
            }
            if model.row_count() == 0 {
                ui.set_message("Save a video from any feed".into());
            }
        } else {
            row.saved = saved;
            model.set_row_data(index, row);
        }
    }
    saved
}

fn edit_library(ui: &StreamTube, state: &State, edit: impl FnOnce(&mut Library)) {
    let mut state = state.borrow_mut();
    let mut updated = state.library.clone();
    edit(&mut updated);
    if !state.storage_writable || updated.write().is_err() {
        notify(&ui, "Could not save changes");
        return;
    }
    state.library = updated;
}

fn feed_error(ui: &StreamTube) {
    ui.set_busy(false);
    ui.set_failed(true);
    ui.set_message("Could not load videos".into());
}

fn current_language(ui: &StreamTube) -> Language {
    Language::from_locale(&ui.global::<UiText>().get_language())
}

fn select_language(ui: &StreamTube, language: Language, device: Language) {
    let choice = Language::OPTIONS
        .iter()
        .position(|option| *option == language)
        .unwrap_or_default();
    ui.set_language_choice(choice as i32);
    ui.set_language_label(language.name().into());
    ui.global::<UiText>()
        .set_language(language.effective(device).code().into());
}
