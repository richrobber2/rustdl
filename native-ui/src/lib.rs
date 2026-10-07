//! Native application interface. Inspection privacy redacts media metadata,
//! artwork and pairing details before rendering and frame acknowledgement.

mod accessibility;
mod activity;
mod anime;
mod artwork;
mod diagnostics;
mod discovery;
mod history;
mod library;
mod peers;
mod player;
mod progress;
mod queue;
mod scroll;
mod settings;
mod storage;
mod streaming;
mod streaming_decoder;
mod ui;
mod updates;
mod release_notes {
    include!(concat!(env!("OUT_DIR"), "/release_notes.rs"));
}
use progress::transfer_progress;

use gpui_kit::base::StyledExt;
use gpui_kit::component::{
    ActiveTheme, Sizable, Theme, ThemeMode,
    button::{Button, ButtonVariants},
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use gpui_mobile::android::{host, jni as mobile_jni};
use jni::EnvUnowned;
use jni::objects::{JClass, JObject, JValue};
use jni::sys::{jboolean, jfloat, jint, jlong};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

#[derive(Debug)]
struct NativeInitError(String);

impl std::fmt::Display for NativeInitError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for NativeInitError {}

impl From<jni::errors::Error> for NativeInitError {
    fn from(error: jni::errors::Error) -> Self {
        Self(error.to_string())
    }
}

// The mobile dependency also links android-activity's legacy entry point.
// RustDL uses host::start with a plain Activity and never invokes this path.
#[unsafe(no_mangle)]
fn android_main(_app: android_activity::AndroidApp) {}

/// Native destinations. Discriminants are shared with Java and accessibility
/// context through `SCREEN`, so they must not change.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
enum Screen {
    #[default]
    Home = 0,
    History = 1,
    Settings = 2,
    Queue = 3,
    Diagnostics = 4,
    Library = 5,
    Discovery = 6,
    Peers = 7,
    Anime = 8,
    Storage = 9,
    Activity = 10,
    Updates = 11,
    Player = 12,
    Streaming = 13,
    StreamingDecoder = 14,
    MiniPlayer = 15,
}

impl Screen {
    const ALL: [Screen; 16] = [
        Screen::Home,
        Screen::History,
        Screen::Settings,
        Screen::Queue,
        Screen::Diagnostics,
        Screen::Library,
        Screen::Discovery,
        Screen::Peers,
        Screen::Anime,
        Screen::Storage,
        Screen::Activity,
        Screen::Updates,
        Screen::Player,
        Screen::Streaming,
        Screen::StreamingDecoder,
        Screen::MiniPlayer,
    ];

    fn from_u8(value: u8) -> Self {
        Self::ALL
            .get(usize::from(value))
            .copied()
            .unwrap_or_default()
    }

    fn current() -> Self {
        Self::from_u8(SCREEN.load(Ordering::Acquire))
    }

    /// Scroll handle slot; the mini-player keeps the library's position.
    fn scroll_index(self) -> usize {
        match self {
            Screen::MiniPlayer => Screen::Library as usize,
            screen => screen as usize,
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct HomeSnapshot {
    dark: bool,
    active: u32,
    downloaded: u64,
    total: u64,
}

static SNAPSHOT: Mutex<HomeSnapshot> = Mutex::new(HomeSnapshot {
    dark: true,
    active: 0,
    downloaded: 0,
    total: 0,
});
static SIGNAL: OnceLock<(async_channel::Sender<()>, async_channel::Receiver<()>)> = OnceLock::new();
static SCREEN: AtomicU8 = AtomicU8::new(0);
static ANIME_SCROLL: Mutex<scroll::ScrollDistance> = Mutex::new(scroll::ScrollDistance::new());
static LIBRARY: Mutex<Option<library::LibraryPage>> = Mutex::new(None);
static DISCOVERY: Mutex<Option<discovery::Page>> = Mutex::new(None);
static STREAMING_DECODER: Mutex<Option<streaming_decoder::Page>> = Mutex::new(None);
static STREAMING: Mutex<Option<streaming::Page>> = Mutex::new(None);
static PLAYER: Mutex<Option<player::Page>> = Mutex::new(None);
static UPDATES: Mutex<Option<updates::Page>> = Mutex::new(None);
static ACTIVITY: Mutex<Option<activity::Page>> = Mutex::new(None);
static STORAGE: Mutex<Option<storage::Page>> = Mutex::new(None);
static ANIME: Mutex<Option<anime::Page>> = Mutex::new(None);
static PEERS: Mutex<Option<peers::Page>> = Mutex::new(None);
static DIAGNOSTICS: Mutex<Option<diagnostics::Diagnostics>> = Mutex::new(None);
static QUEUE: Mutex<Option<queue::QueuePage>> = Mutex::new(None);
static SETTINGS: Mutex<Option<settings::Settings>> = Mutex::new(None);
static PRIVACY_READY: AtomicBool = AtomicBool::new(false);
static PRIVACY_REVISION: AtomicU64 = AtomicU64::new(0);
static PRIVACY_ACK_REVISION: AtomicU64 = AtomicU64::new(u64::MAX);
static ACCESSIBILITY_TREE: OnceLock<Mutex<accessibility::SemanticTree>> = OnceLock::new();
static ACCESSIBILITY_CONTEXT: Mutex<Option<(u64, u8)>> = Mutex::new(None);
static READY: AtomicBool = AtomicBool::new(false);

fn semantic_text(id: impl Into<ElementId>, value: impl Into<SharedString>) -> Text {
    Text::new(id.into(), value.into())
}

fn semantic_scroll(id: &'static str, handle: &ScrollHandle) -> Stateful<Div> {
    let down = handle.clone();
    let up = handle.clone();
    div()
        .id(id)
        .role(accesskit::Role::ScrollView)
        .track_scroll(handle)
        .on_a11y_action(accesskit::Action::ScrollDown, move |_, window, _| {
            let mut offset = down.offset();
            offset.y =
                (offset.y - down.bounds().size.height * 0.8).max(-down.max_offset().y.max(px(0.)));
            down.set_offset(offset);
            window.refresh();
        })
        .on_a11y_action(accesskit::Action::ScrollUp, move |_, window, _| {
            let mut offset = up.offset();
            offset.y = (offset.y + up.bounds().size.height * 0.8).min(px(0.));
            up.set_offset(offset);
            window.refresh();
        })
}

fn signal() -> &'static (async_channel::Sender<()>, async_channel::Receiver<()>) {
    SIGNAL.get_or_init(|| async_channel::bounded(1))
}

fn snapshot() -> HomeSnapshot {
    *SNAPSHOT
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn change_theme(dark: bool, cx: &mut App) {
    Theme::change(
        if dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        },
        None,
        cx,
    );
}

struct Home {
    transfers: HomeSnapshot,
    navigation_error: bool,
    screen: Screen,
    settings: Option<settings::Settings>,
    queue: Option<queue::QueuePage>,
    diagnostics: Option<diagnostics::Diagnostics>,
    library: Option<library::LibraryPage>,
    discovery: Option<discovery::Page>,
    peers: Option<peers::Page>,
    anime: Option<anime::Page>,
    storage: Option<storage::Page>,
    activity: Option<activity::Page>,
    updates: Option<updates::Page>,
    streaming: Option<streaming::Page>,
    streaming_decoder: Option<streaming_decoder::Page>,
    player: Option<player::Page>,
    player_slider: Entity<gpui_kit::component::slider::SliderState>,
    player_dragging: bool,
    player_volume: Entity<gpui_kit::component::slider::SliderState>,
    player_volume_dragging: bool,
    _player_volume_events: Subscription,
    _player_slider_events: Subscription,
    activity_filter: u8,
    discovery_picks: std::collections::BTreeMap<String, String>,
    discovery_token: String,
    discovery_format: u8,
    history_list: ListState,
    history_rows: std::sync::Arc<Vec<history::Row>>,
    history_limit: usize,
    history_note_limits: std::collections::BTreeMap<usize, usize>,
    scrolls: [ScrollHandle; 16],
    artwork_state: artwork::State,
    _updates: Task<()>,
}

impl Home {
    fn new(cx: &mut Context<Self>) -> Self {
        let player_slider = cx.new(|_| {
            gpui_kit::component::slider::SliderState::new()
                .min(0.)
                .max(100.)
                .step(0.1)
        });
        let player_slider_events = cx.subscribe(&player_slider, |this, _, event, cx| {
            use gpui_kit::component::slider::{SliderEvent, SliderValue};
            match event {
                SliderEvent::Change(SliderValue::Single(value)) => {
                    this.player_dragging = true;
                    if let Some(page) = &this.player {
                        if page.duration_seconds.is_finite() && page.duration_seconds > 0. {
                            let _ = player::request(
                                "preview",
                                (*value as f64 / 100.).clamp(0., 1.) * page.duration_seconds,
                            );
                        }
                    }
                }
                SliderEvent::Release(SliderValue::Single(value)) => {
                    this.player_dragging = false;
                    let _ = player::request("preview-hide", 0.);
                    if let Some(page) = &this.player {
                        if page.duration_seconds.is_finite() && page.duration_seconds > 0. {
                            this.navigation_error = player::request(
                                "seek",
                                (*value as f64 / 100.).clamp(0., 1.) * page.duration_seconds,
                            )
                            .is_err();
                        }
                    }
                }
                _ => {
                    this.player_dragging = false;
                    let _ = player::request("preview-hide", 0.);
                }
            }
            cx.notify();
        });
        let player_volume = cx.new(|_| {
            gpui_kit::component::slider::SliderState::new()
                .min(0.)
                .max(100.)
                .step(1.)
                .default_value(100.)
        });
        let player_volume_events = cx.subscribe(&player_volume, |this, _, event, cx| {
            use gpui_kit::component::slider::{SliderEvent, SliderValue};
            match event {
                SliderEvent::Change(_) => this.player_volume_dragging = true,
                SliderEvent::Release(SliderValue::Single(value)) => {
                    this.player_volume_dragging = false;
                    if this.screen == Screen::Player {
                        this.navigation_error =
                            player::request("volume", (*value as f64 / 100.).clamp(0., 1.))
                                .is_err();
                    }
                }
                _ => this.player_volume_dragging = false,
            }
            cx.notify();
        });
        let transfers = snapshot();
        change_theme(transfers.dark, cx);
        let receiver = signal().1.clone();
        // Notifications coalesce into a single pending update. No polling,
        // HTTP, directory scans, or work while the state is unchanged.
        let updates = cx.spawn(async move |this, cx| {
            while receiver.recv().await.is_ok() {
                let next = snapshot();
                if this
                    .update(cx, |this, cx| {
                        let screen = Screen::current();
                        let delta = ANIME_SCROLL
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .take(screen as u8);
                        if delta != 0. {
                            let handle = &this.scrolls[Screen::Anime as usize];
                            let previous = handle.offset();
                            let mut offset = previous;
                            offset.y = (offset.y + px(delta))
                                .clamp(-handle.max_offset().y.max(px(0.)), px(0.));
                            if offset != previous {
                                handle.set_offset(offset);
                                cx.notify();
                            }
                        }
                        let settings = SETTINGS.lock().unwrap_or_else(|p| p.into_inner()).clone();
                        let queue = QUEUE.lock().unwrap_or_else(|p| p.into_inner()).clone();
                        let diagnostics = DIAGNOSTICS
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .clone();
                        let library = LIBRARY.lock().unwrap_or_else(|p| p.into_inner()).clone();
                        let discovery = DISCOVERY.lock().unwrap_or_else(|p| p.into_inner()).clone();
                        let streaming_decoder = STREAMING_DECODER
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .clone();
                        let streaming = STREAMING.lock().unwrap_or_else(|p| p.into_inner()).clone();
                        let player = PLAYER.lock().unwrap_or_else(|p| p.into_inner()).clone();
                        let updates = UPDATES.lock().unwrap_or_else(|p| p.into_inner()).clone();
                        let activity = ACTIVITY.lock().unwrap_or_else(|p| p.into_inner()).clone();
                        let storage = STORAGE.lock().unwrap_or_else(|p| p.into_inner()).clone();
                        let anime = ANIME.lock().unwrap_or_else(|p| p.into_inner()).clone();
                        let peers = PEERS.lock().unwrap_or_else(|p| p.into_inner()).clone();
                        if this.transfers == next
                            && this.screen == screen
                            && this.settings == settings
                            && this.queue == queue
                            && this.diagnostics == diagnostics
                            && this.library == library
                            && this.discovery == discovery
                            && this.streaming_decoder == streaming_decoder
                            && this.streaming == streaming
                            && this.player == player
                            && this.updates == updates
                            && this.activity == activity
                            && this.storage == storage
                            && this.anime == anime
                            && this.peers == peers
                        {
                            return;
                        }
                        if this.transfers.dark != next.dark {
                            change_theme(next.dark, cx);
                        }
                        this.screen = screen;
                        this.settings = settings;
                        this.queue = queue;
                        this.diagnostics = diagnostics;
                        this.library = library;
                        if let Some(page) = &discovery
                            && this.discovery != discovery
                        {
                            if (page.kind == "qualities" || page.kind == "playlist")
                                && this.discovery_token != page.token
                            {
                                this.discovery_picks.clear();
                                this.discovery_token = page.token.clone();
                            }
                            if let Some(selection) = &page.selection {
                                this.discovery_picks.clear();
                                for pick in selection {
                                    let id = if page.kind == "playlist" {
                                        pick.clone()
                                    } else {
                                        pick.rsplit_once(':')
                                            .map(|(id, _)| id.to_owned())
                                            .unwrap_or_default()
                                    };
                                    this.discovery_picks.insert(id, pick.clone());
                                }
                            }
                        }
                        this.discovery = discovery;
                        this.peers = peers;
                        this.anime = anime;
                        this.storage = storage;
                        this.activity = activity;
                        this.updates = updates;
                        this.streaming_decoder = streaming_decoder;
                        this.streaming = streaming;
                        this.player = player;
                        this.transfers = next;
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        let history_rows = std::sync::Arc::new(history::rows(
            release_notes::CHANGELOG,
            5,
            &Default::default(),
        ));
        let history_list = ListState::new(history_rows.len(), ListAlignment::Top, px(150.));
        Self {
            transfers,
            navigation_error: false,
            screen: Screen::current(),
            library: LIBRARY.lock().unwrap_or_else(|p| p.into_inner()).clone(),
            discovery: DISCOVERY.lock().unwrap_or_else(|p| p.into_inner()).clone(),
            streaming_decoder: STREAMING_DECODER
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .clone(),
            streaming: STREAMING.lock().unwrap_or_else(|p| p.into_inner()).clone(),
            player: PLAYER.lock().unwrap_or_else(|p| p.into_inner()).clone(),
            updates: UPDATES.lock().unwrap_or_else(|p| p.into_inner()).clone(),
            activity: ACTIVITY.lock().unwrap_or_else(|p| p.into_inner()).clone(),
            player_slider,
            player_dragging: false,
            player_volume,
            player_volume_dragging: false,
            _player_volume_events: player_volume_events,
            _player_slider_events: player_slider_events,
            activity_filter: 0,
            storage: STORAGE.lock().unwrap_or_else(|p| p.into_inner()).clone(),
            anime: ANIME.lock().unwrap_or_else(|p| p.into_inner()).clone(),
            peers: PEERS.lock().unwrap_or_else(|p| p.into_inner()).clone(),
            discovery_picks: Default::default(),
            discovery_token: String::new(),
            discovery_format: 0,
            diagnostics: DIAGNOSTICS
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .clone(),
            queue: QUEUE.lock().unwrap_or_else(|p| p.into_inner()).clone(),
            settings: SETTINGS.lock().unwrap_or_else(|p| p.into_inner()).clone(),
            history_list,
            history_rows,
            history_limit: 5,
            history_note_limits: std::collections::BTreeMap::new(),
            scrolls: std::array::from_fn(|_| ScrollHandle::new()),
            artwork_state: artwork::State::default(),
            _updates: updates,
        }
    }

    /// Switches screens and tells the Android host which route is visible.
    fn navigate(&mut self, screen: Screen, name: &'static str, cx: &mut Context<Self>) {
        self.screen = screen;
        set_screen(screen, Ordering::Release);
        let _ = notify_screen(name);
        cx.notify();
    }

    fn back_button(&self, id: &'static str, cx: &mut Context<Self>) -> Button {
        Button::new(id)
            .label("Back")
            .large()
            .on_click(cx.listener(|this, _, _, cx| this.navigate(Screen::Home, "home", cx)))
    }

    fn destination(&self, id: &'static str, label: &'static str, cx: &mut Context<Self>) -> Button {
        Button::new(id)
            .label(label)
            .large()
            .w_full()
            .when(id == "library", |button| button.primary())
            .on_click(cx.listener(move |this, _, _, cx| {
                let _ = notify_screen(id);
                if id == "changelog" {
                    this.screen = Screen::History;
                    set_screen(Screen::History, Ordering::Release);
                    this.navigation_error = false;
                } else if id == "updates" {
                    this.settings = read_settings().ok();
                    *SETTINGS.lock().unwrap_or_else(|p| p.into_inner()) = this.settings.clone();
                    set_screen(Screen::Updates, Ordering::Release);
                    this.screen = Screen::Updates;
                    this.navigation_error = updates::request("snapshot").is_err();
                } else if id == "activity" {
                    this.settings = read_settings().ok();
                    *SETTINGS.lock().unwrap_or_else(|p| p.into_inner()) = this.settings.clone();
                    set_screen(Screen::Activity, Ordering::Release);
                    this.screen = Screen::Activity;
                    this.navigation_error = activity::request(
                        ["all", "active", "issue", "complete"][this.activity_filter as usize % 4],
                        0,
                    )
                    .is_err();
                } else if id == "storage" {
                    this.settings = read_settings().ok();
                    *SETTINGS.lock().unwrap_or_else(|p| p.into_inner()) = this.settings.clone();
                    set_screen(Screen::Storage, Ordering::Release);
                    this.screen = Screen::Storage;
                    this.navigation_error = storage::request("snapshot", "", 0).is_err();
                } else if id == "anime" {
                    this.settings = read_settings().ok();
                    *SETTINGS.lock().unwrap_or_else(|p| p.into_inner()) = this.settings.clone();
                    set_screen(Screen::Anime, Ordering::Release);
                    this.screen = Screen::Anime;
                    this.navigation_error = anime::request("catalog", "{}", 0).is_err();
                } else if id == "library" {
                    this.settings = read_settings().ok();
                    *SETTINGS.lock().unwrap_or_else(|p| p.into_inner()) = this.settings.clone();
                    this.screen = Screen::Library;
                    set_screen(Screen::Library, Ordering::Release);
                    this.library = None;
                    *LIBRARY.lock().unwrap_or_else(|p| p.into_inner()) = None;
                    this.navigation_error = request_library(0, "").is_err();
                } else if id == "diagnostics" {
                    match read_diagnostics() {
                        Ok(diagnostics) => {
                            *DIAGNOSTICS.lock().unwrap_or_else(|p| p.into_inner()) =
                                Some(diagnostics.clone());
                            this.diagnostics = Some(diagnostics);
                            this.screen = Screen::Diagnostics;
                            set_screen(Screen::Diagnostics, Ordering::Release);
                            this.navigation_error = false;
                        }
                        Err(_) => this.navigation_error = true,
                    }
                } else if id == "queue" {
                    match read_queue(0) {
                        Ok(queue) => {
                            this.settings = read_settings().ok();
                            *SETTINGS.lock().unwrap_or_else(|p| p.into_inner()) =
                                this.settings.clone();
                            *QUEUE.lock().unwrap_or_else(|p| p.into_inner()) = Some(queue.clone());
                            this.queue = Some(queue);
                            this.screen = Screen::Queue;
                            set_screen(Screen::Queue, Ordering::Release);
                            this.navigation_error = false;
                        }
                        Err(_) => this.navigation_error = true,
                    }
                } else if id == "settings" {
                    match read_settings() {
                        Ok(settings) => {
                            *SETTINGS.lock().unwrap_or_else(|p| p.into_inner()) =
                                Some(settings.clone());
                            this.settings = Some(settings);
                            this.screen = Screen::Settings;
                            set_screen(Screen::Settings, Ordering::Release);
                            this.navigation_error = false;
                        }
                        Err(_) => this.navigation_error = true,
                    }
                } else {
                    this.navigation_error = open_destination(id).is_err();
                }
                cx.notify();
            }))
    }
}

impl Home {
    fn refresh_history_rows(&mut self) {
        let rows = history::rows(
            release_notes::CHANGELOG,
            self.history_limit,
            &self.history_note_limits,
        );
        let prefix = self
            .history_rows
            .iter()
            .zip(&rows)
            .take_while(|(old, new)| old == new)
            .count();
        let suffix = self.history_rows[prefix..]
            .iter()
            .rev()
            .zip(rows[prefix..].iter().rev())
            .take_while(|(old, new)| old == new)
            .count();
        self.history_list.splice(
            prefix..self.history_rows.len() - suffix,
            rows.len() - prefix - suffix,
        );
        self.history_rows = std::sync::Arc::new(rows);
    }

    fn render_history(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = self.history_rows.clone();
        let entity = cx.entity();
        let down = self.history_list.clone();
        let up = self.history_list.clone();
        let distance = window.viewport_size().height * 0.8;
        let content = div()
            .id("release-history")
            .role(accesskit::Role::ScrollView)
            .on_a11y_action(accesskit::Action::ScrollDown, move |_, window, _| {
                down.scroll_by(distance);
                window.refresh();
            })
            .on_a11y_action(accesskit::Action::ScrollUp, move |_, window, _| {
                up.scroll_by(-distance);
                window.refresh();
            })
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .child(
                list(self.history_list.clone(), move |index, _, cx| {
                    let item = div().id(("release-row", index)).px_4().pb_2();
                    match &rows[index] {
                        history::Row::Version(version) => item
                            .pt_4()
                            .text_lg()
                            .font_semibold()
                            .child(semantic_text(
                                "release-version",
                                format!("Version {version}"),
                            ))
                            .into_any_element(),
                        history::Row::Note(note) => item
                            .pb_4()
                            .child(ui::card(cx).child(semantic_text("release-note", *note)))
                            .into_any_element(),
                        history::Row::MoreNotes { version, remaining } => {
                            let version = *version;
                            let entity = entity.clone();
                            item.child(
                                Button::new("more-release-notes")
                                    .label(format!("Load more notes ({remaining} remaining)"))
                                    .large()
                                    .w_full()
                                    .on_click(move |_, _, cx| {
                                        entity.update(cx, |this, cx| {
                                            let limit = this
                                                .history_note_limits
                                                .entry(version)
                                                .or_insert(8);
                                            *limit = limit.saturating_add(8);
                                            this.refresh_history_rows();
                                            cx.notify();
                                        })
                                    }),
                            )
                            .into_any_element()
                        }
                        history::Row::Older => {
                            let entity = entity.clone();
                            item.child(
                                Button::new("older-releases")
                                    .label("Load older releases")
                                    .large()
                                    .w_full()
                                    .on_click(move |_, _, cx| {
                                        entity.update(cx, |this, cx| {
                                            this.history_limit =
                                                this.history_limit.saturating_add(8);
                                            this.refresh_history_rows();
                                            cx.notify();
                                        })
                                    }),
                            )
                            .into_any_element()
                        }
                    }
                })
                .size_full(),
            );
        ui::page(cx)
            .child(ui::header(
                self.back_button("history-back", cx),
                "lib-remaining-text-1",
                "What's new",
            ))
            .child(content)
    }
}

impl Home {
    fn setting_button(
        &self,
        key: &'static str,
        label: String,
        value: String,
        cx: &mut Context<Self>,
    ) -> Button {
        Button::new(key)
            .label(label)
            .large()
            .w_full()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.navigation_error = change_setting(key, &value).is_err();
                cx.notify();
            }))
    }

    fn settings_group(title: &'static str, cx: &Context<Self>) -> gpui::Div {
        ui::card(cx).child(ui::card_title(title, title))
    }

    fn render_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut content = ui::body("native-settings", &self.scrolls[Screen::Settings as usize]);
        if let Some(settings) = &self.settings {
            let folder = if settings.inspection_privacy {
                "Hidden during inspection"
            } else {
                &settings.download_folder
            };
            content = content.child(ui::muted_text(
                "settings-autosave",
                "Changes save immediately.",
                cx,
            ));
            let mut downloads = Self::settings_group("Downloads", cx).child(self.setting_button(
                "downloadFolder",
                format!("Download folder: {folder}"),
                String::new(),
                cx,
            ));
            let mut appearance = Self::settings_group("Appearance and motion", cx);
            let mut privacy = Self::settings_group("Privacy", cx);
            let mut playback = Self::settings_group("Playback", cx);
            for (key, label, current, choices) in [
                (
                    "appearance",
                    "Appearance",
                    settings.appearance.as_str(),
                    &["system", "light", "dark"][..],
                ),
                (
                    "mobileDownloadPolicy",
                    "Mobile data",
                    settings.mobile_download_policy.as_str(),
                    &["ask", "allow", "block"][..],
                ),
                (
                    "backgroundTheme",
                    "Background",
                    settings.background_theme.as_str(),
                    &["space", "rainy-city"][..],
                ),
            ] {
                let button = self.setting_button(
                    key,
                    format!("{label}: {current}"),
                    settings::next_choice(current, choices).to_owned(),
                    cx,
                );
                if key == "mobileDownloadPolicy" {
                    downloads = downloads.child(button);
                } else {
                    appearance = appearance.child(button);
                }
            }
            for (key, label, current) in [
                (
                    "keepScreenAwake",
                    "Keep screen awake during playback",
                    settings.keep_screen_awake,
                ),
                (
                    "allowScreenshots",
                    "Allow screenshots",
                    settings.allow_screenshots,
                ),
                (
                    "inspectionPrivacy",
                    "Inspection privacy",
                    settings.inspection_privacy,
                ),
                ("reduceMotion", "Reduce motion", settings.reduce_motion),
                (
                    "spaceEffectEnabled",
                    "Moving space background",
                    settings.space_effect_enabled,
                ),
            ] {
                let button = self.setting_button(
                    key,
                    format!("{label}: {}", if current { "On" } else { "Off" }),
                    (!current).to_string(),
                    cx,
                );
                match key {
                    "keepScreenAwake" => playback = playback.child(button),
                    "allowScreenshots" | "inspectionPrivacy" => privacy = privacy.child(button),
                    _ => appearance = appearance.child(button),
                }
            }
            let refresh = settings.diagnostics_refresh_seconds.to_string();
            let maintenance = Self::settings_group("Maintenance", cx)
                .child(self.setting_button(
                    "diagnosticsRefreshSeconds",
                    format!("Diagnostics refresh: {refresh} seconds"),
                    settings::next_choice(&refresh, &["3", "5", "10", "30"]).to_owned(),
                    cx,
                ))
                .child(self.setting_button(
                    "reset",
                    "Restore defaults".to_owned(),
                    String::new(),
                    cx,
                ));
            content = content
                .child(downloads)
                .child(appearance)
                .child(playback)
                .child(privacy)
                .child(maintenance)
                .child(
                    div()
                        .text_color(if settings.ok {
                            cx.theme().muted_foreground
                        } else {
                            cx.theme().danger
                        })
                        .child(semantic_text("settings-detail", settings.detail.clone())),
                );
        }
        if self.navigation_error {
            content = content.child(ui::error_text(
                "settings-error",
                "Could not save settings. Try again.",
                cx,
            ));
        }
        ui::page(cx)
            .child(ui::header(
                self.back_button("settings-back", cx),
                "settings-heading",
                "Settings",
            ))
            .child(content)
    }
}

impl Home {
    fn queue_page_button(
        &self,
        id: &'static str,
        label: &'static str,
        offset: i32,
        cx: &mut Context<Self>,
    ) -> Button {
        Button::new(id)
            .label(label)
            .large()
            .on_click(cx.listener(move |this, _, _, cx| {
                match read_queue(offset) {
                    Ok(queue) => {
                        *QUEUE.lock().unwrap_or_else(|p| p.into_inner()) = Some(queue.clone());
                        this.queue = Some(queue);
                        this.navigation_error = false;
                    }
                    Err(_) => this.navigation_error = true,
                }
                cx.notify();
            }))
    }

    fn render_queue(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut content = ui::body("native-queue", &self.scrolls[Screen::Queue as usize]);
        let privacy = self
            .settings
            .as_ref()
            .is_none_or(|settings| settings.inspection_privacy);
        if let Some(queue) = &self.queue {
            if queue.network_state != 0 {
                let mut network = ui::card(cx).child(ui::card_title(
                    "queue-network-state",
                    match queue.network_state {
                        2 => "Waiting for unmetered Wi-Fi",
                        3 => "Waiting for mobile-data approval",
                        _ => "Waiting for an internet connection",
                    },
                ));
                if queue.network_state == 3 {
                    network = network.child(
                        Button::new("queue-mobile-approval")
                            .label("Allow mobile data…")
                            .large()
                            .w_full()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.navigation_error = approve_mobile_downloads().is_err();
                                cx.notify();
                            })),
                    );
                }
                content = content.child(network.child(self.destination(
                    "settings",
                    "Download settings",
                    cx,
                )));
            }
            if queue.items.is_empty() {
                content = content.child(ui::card(cx).child(ui::muted_text(
                    "queue-empty",
                    "The download queue is empty.",
                    cx,
                )));
            } else {
                content = content.child(ui::muted_text(
                    "lib-remaining-text-2",
                    format!(
                        "{}–{} of {} downloads",
                        queue.offset + 1,
                        queue.offset + queue.items.len(),
                        queue.total
                    ),
                    cx,
                ));
            }
            for item in &queue.items {
                let title = if privacy {
                    "Downloaded media".to_owned()
                } else {
                    item.title.clone()
                };
                let mut row = ui::card(cx)
                    .id(format!("queue-row-{}", item.id))
                    .gap_2()
                    .child(
                        div()
                            .font_semibold()
                            .child(semantic_text("queue-title", title)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_x_3()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(semantic_text("queue-phase", item.phase_label.clone()))
                            .child(semantic_text(
                                "queue-progress",
                                transfer_progress(item.downloaded, item.total),
                            )),
                    );
                if !privacy && let Some(quality) = &item.quality {
                    row = row.child(ui::muted_text("queue-quality", quality.clone(), cx));
                }
                let mut actions = ui::button_row();
                for action in item
                    .actions
                    .iter()
                    .map(String::as_str)
                    .chain(item.can_play.then_some("play"))
                {
                    let label = match action {
                        "pause" => "Pause",
                        "resume" => "Resume",
                        "cancel" => "Cancel",
                        "play" => "Open player",
                        _ => continue,
                    };
                    let id = item.id.clone();
                    let primary = action == "play" || action == "resume";
                    let action = action.to_owned();
                    actions = actions.child(
                        Button::new(SharedString::from(format!("queue-{id}-{action}")))
                            .label(label)
                            .large()
                            .when(primary, |button| button.primary())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.navigation_error = change_queue_item(&id, &action).is_err();
                                cx.notify();
                            })),
                    );
                }
                if item.issue {
                    row = row.child(ui::error_text(
                        "queue-issue",
                        if privacy {
                            "Download failed. Try resuming.".to_owned()
                        } else {
                            item.issue_detail
                                .clone()
                                .unwrap_or_else(|| "Download failed. Try resuming.".to_owned())
                        },
                        cx,
                    ));
                }
                content = content.child(row.child(actions));
            }
            let mut pages = ui::button_row();
            if let Some(offset) = queue.previous_offset {
                pages = pages.child(self.queue_page_button(
                    "queue-previous",
                    "Previous downloads",
                    offset,
                    cx,
                ));
            }
            if let Some(offset) = queue.next_offset {
                pages =
                    pages.child(self.queue_page_button("queue-next", "Next downloads", offset, cx));
            }
            content = content.child(pages.child(self.queue_page_button(
                "queue-refresh",
                "Refresh",
                queue.offset as i32,
                cx,
            )));
            if !queue.ok {
                content = content.child(ui::error_text("queue-detail", queue.detail.clone(), cx));
            }
        }
        if self.navigation_error {
            content = content.child(ui::error_text(
                "queue-action-error",
                "Queue action unavailable. Try again.",
                cx,
            ));
        }
        ui::page(cx)
            .child(ui::header(
                self.back_button("queue-back", cx),
                "home-queue-heading",
                "Download queue",
            ))
            .child(content)
    }
}

impl Home {
    fn render_diagnostics(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut content = ui::body(
            "native-diagnostics",
            &self.scrolls[Screen::Diagnostics as usize],
        );
        if let Some(diagnostics) = &self.diagnostics {
            let data = &diagnostics.data;
            let mut metrics = ui::card(cx).gap_2();
            for (index, text) in [
                diagnostics.detail.clone(),
                format!(
                    "{} / {} diagnostic sources",
                    data.available_sources, data.total_sources
                ),
                format!("Uptime: {:.0} seconds", data.uptime_seconds),
                format!("CPU cores: {}", data.processors),
                format!(
                    "CPU load (1 / 5 / 15 minutes): {} / {} / {}",
                    diagnostics::metric(data.load1),
                    diagnostics::metric(data.load5),
                    diagnostics::metric(data.load15)
                ),
                format!(
                    "Memory available / total: {} / {}",
                    diagnostics::bytes(data.memory_available_bytes),
                    diagnostics::bytes(data.memory_total_bytes)
                ),
                format!(
                    "Storage available / total: {} / {}",
                    diagnostics::bytes(data.storage_available_bytes),
                    diagnostics::bytes(data.storage_total_bytes)
                ),
                format!(
                    "Battery: {}",
                    if data.battery_level < 0 {
                        "Unavailable".to_owned()
                    } else {
                        format!("{}% ({})", data.battery_level, data.battery_status)
                    }
                ),
                format!(
                    "Battery temperature: {} °C",
                    diagnostics::metric(data.battery_temperature_c)
                ),
                format!(
                    "Thermal status: {}",
                    match data.thermal_status {
                        0 => "None",
                        1 => "Light",
                        2 => "Moderate",
                        3 => "Severe",
                        4 => "Critical",
                        5 => "Emergency",
                        6 => "Shutdown",
                        _ => "Unavailable",
                    }
                ),
            ]
            .into_iter()
            .enumerate()
            {
                let id = format!("diagnostic-metric-{index}");
                metrics = metrics.child(if index == 0 {
                    ui::muted_text(id, text, cx)
                } else {
                    div().child(semantic_text(id, text))
                });
            }
            content = content.child(metrics);
            if !diagnostics.ok {
                content = content.child(ui::error_text(
                    "diagnostics-unavailable",
                    "Some diagnostics are unavailable.",
                    cx,
                ));
            }
            content = content.child(
                Button::new("copy-diagnostics")
                    .label("Copy diagnostics")
                    .large()
                    .w_full()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.navigation_error = copy_diagnostics().is_err();
                        cx.notify();
                    })),
            );
        }
        if self.navigation_error {
            content = content.child(ui::error_text(
                "diagnostics-error",
                "Diagnostics action unavailable. Try again.",
                cx,
            ));
        }
        ui::page(cx)
            .child(ui::header(
                self.back_button("diagnostics-back", cx),
                "diagnostics-heading",
                "Diagnostics",
            ))
            .child(content)
    }
}

impl Home {
    fn library_page_button(
        &self,
        id: &'static str,
        label: &'static str,
        offset: i32,
        location: String,
        cx: &mut Context<Self>,
    ) -> Button {
        Button::new(id)
            .label(label)
            .large()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.navigation_error = request_library(offset, &location).is_err();
                cx.notify();
            }))
    }

    fn render_library(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let privacy = self
            .settings
            .as_ref()
            .is_none_or(|settings| settings.inspection_privacy);
        let mut content = ui::body("native-library", &self.scrolls[Screen::Library as usize])
            .child(
                Button::new("library-search")
                    .label("Search and filter library")
                    .large()
                    .w_full()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.navigation_error = search_library().is_err();
                        cx.notify();
                    })),
            );
        if let Some(page) = &self.library {
            if page.title == "Continue watching" || page.title == "Up Next" {
                content = content.child(ui::card_title("library-filter-title", page.title.clone()));
            }
            if !page.ok {
                content = content.child(ui::error_text("library-detail", page.detail.clone(), cx));
            } else if page.items.is_empty() {
                content = content.child(ui::card(cx).child(ui::muted_text(
                    "library-empty",
                    "No matching library items.",
                    cx,
                )));
            } else {
                content = content.child(ui::muted_text(
                    "lib-remaining-text-3",
                    format!(
                        "{}–{} of {} items",
                        page.offset + 1,
                        page.offset + page.items.len(),
                        page.total
                    ),
                    cx,
                ));
            }
            for item in &page.items {
                let title = if privacy {
                    if item.kind == "playlist" {
                        "Playlist"
                    } else {
                        "Downloaded media"
                    }
                    .to_owned()
                } else {
                    item.title.clone()
                };
                let subtitle = if privacy {
                    item.state.clone()
                } else {
                    item.subtitle.clone()
                };
                let mut card = ui::card(cx)
                    .id(format!("library-card-{}", item.id))
                    .gap_2()
                    .child(
                        div()
                            .font_semibold()
                            .child(semantic_text("library-title", title)),
                    )
                    .child(ui::muted_text("library-subtitle", subtitle, cx));
                if !privacy
                    && item.has_thumbnail
                    && let Ok(path) = library_thumbnail(&item.id)
                    && !path.is_empty()
                {
                    card = card.child(
                        img(std::path::PathBuf::from(path))
                            .h_40()
                            .w_full()
                            .rounded_md()
                            .object_fit(ObjectFit::Cover),
                    );
                }
                if item.watched {
                    card = card.child(ui::muted_text("library-watched", "Watched", cx));
                } else if let Some(progress) =
                    progress::playback_progress(item.position_seconds, item.duration_seconds)
                {
                    card = card.child(ui::muted_text("library-progress", progress, cx));
                }
                let id = item.id.clone();
                if item.kind == "playlist" {
                    card = card.child(
                        Button::new(SharedString::from(format!("folder-{id}")))
                            .label("Open playlist")
                            .primary()
                            .large()
                            .w_full()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.navigation_error = request_library(0, &id).is_err();
                                cx.notify();
                            })),
                    );
                } else {
                    let mut actions = ui::button_row();
                    for (action, label, visible) in [
                        ("play", "Open player", true),
                        ("share", "Share", item.can_share),
                        ("send", "Send to device", item.can_share),
                        ("source", "Open source", item.can_source),
                        ("quality", "Choose another quality", item.can_source),
                        ("watched", "Mark watched", !item.watched),
                        ("enqueue", "Add to Up Next", !item.queued),
                        ("dequeue", "Remove from Up Next", item.queued),
                        ("delete", "Delete", item.can_delete),
                    ] {
                        if !visible {
                            continue;
                        }
                        let id = item.id.clone();
                        actions = actions.child(
                            Button::new(SharedString::from(format!("library-{id}-{action}")))
                                .label(label)
                                .large()
                                .when(action == "play", |button| button.primary())
                                .when(action == "delete", |button| button.danger())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.navigation_error = library_action(&id, action).is_err();
                                    cx.notify();
                                })),
                        );
                    }
                    card = card.child(actions);
                }
                content = content.child(card);
            }
            let mut pages = ui::button_row();
            if let Some(offset) = page.previous_offset {
                pages = pages.child(self.library_page_button(
                    "library-previous",
                    "Previous items",
                    offset,
                    page.location.clone(),
                    cx,
                ));
            }
            if let Some(offset) = page.next_offset {
                pages = pages.child(self.library_page_button(
                    "library-next",
                    "Next items",
                    offset,
                    page.location.clone(),
                    cx,
                ));
            }
            content = content.child(pages.child(self.library_page_button(
                "library-refresh",
                "Refresh",
                page.offset as i32,
                page.location.clone(),
                cx,
            )));
        } else {
            content = content.child(ui::card(cx).child(ui::muted_text(
                "library-loading",
                "Loading library…",
                cx,
            )));
        }
        content = content.child(
            Button::new("native-discover")
                .label("Discover downloads")
                .large()
                .w_full()
                .on_click(
                    cx.listener(|this, _, _, cx| this.navigate(Screen::Discovery, "discovery", cx)),
                ),
        );
        content = content.child(
            Button::new("library-receive")
                .label("Receive from another device")
                .large()
                .w_full()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.navigation_error = mobile_jni::with_env(|env| {
                        let activity = mobile_jni::activity(env)?;
                        env.call_method(
                            &activity,
                            jni::jni_str!("openNativeReceiver"),
                            jni::jni_sig!("()V"),
                            &[],
                        )
                        .map_err(|e| e.to_string())?;
                        Ok(())
                    })
                    .is_err();
                    cx.notify();
                })),
        );
        let tools = [
            ("anime", "Anime catalog"),
            ("activity", "Activity Center"),
            ("queue", "Download queue"),
            ("storage", "Storage manager"),
            ("diagnostics", "Diagnostics"),
            ("settings", "Settings"),
            ("changelog", "What's new"),
        ]
        .into_iter()
        .map(|(id, label)| self.destination(id, label, cx))
        .collect();
        content = content.child(
            ui::card(cx)
                .child(ui::card_title("library-tools-heading", "Library tools"))
                .child(ui::button_grid(tools)),
        );
        if self.navigation_error {
            content = content.child(ui::error_text(
                "library-action-error",
                "Library action unavailable. Try again.",
                cx,
            ));
        }
        let heading = if privacy {
            "Library".to_owned()
        } else {
            self.library
                .as_ref()
                .map(|page| page.title.clone())
                .unwrap_or_else(|| "Library".to_owned())
        };
        let back = Button::new("library-back")
            .label("Back")
            .large()
            .on_click(cx.listener(|this, _, _, cx| {
                if this
                    .library
                    .as_ref()
                    .is_some_and(|page| !page.location.is_empty())
                {
                    this.navigation_error = request_library(0, "").is_err();
                    cx.notify();
                } else {
                    this.navigate(Screen::Home, "home", cx);
                }
            }));
        ui::page(cx)
            .child(ui::header(back, "library-heading", heading))
            .child(content)
    }
}

impl Home {
    fn render_content(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let privacy = if self.screen == Screen::StreamingDecoder {
            self.streaming_decoder
                .as_ref()
                .is_none_or(|page| page.privacy)
        } else {
            self.settings
                .as_ref()
                .is_none_or(|settings| settings.inspection_privacy)
        };
        let revision = PRIVACY_REVISION.load(Ordering::Acquire);
        // Acknowledgment needs one protected frame per revision, not a new
        // frame-source wake after every unchanged render or animation tick.
        if PRIVACY_ACK_REVISION.load(Ordering::Acquire) != revision {
            window.on_next_frame(move |_, _| {
                if PRIVACY_REVISION.load(Ordering::Acquire) == revision {
                    PRIVACY_READY.store(privacy, Ordering::Release);
                    PRIVACY_ACK_REVISION.store(revision, Ordering::Release);
                }
            });
        }
        match self.screen {
            Screen::Home => self.render_home(cx).into_any_element(),
            Screen::History => self.render_history(window, cx).into_any_element(),
            Screen::Settings => self.render_settings(cx).into_any_element(),
            Screen::Queue => self.render_queue(cx).into_any_element(),
            Screen::Diagnostics => self.render_diagnostics(cx).into_any_element(),
            Screen::Library => self.render_library(cx).into_any_element(),
            Screen::Discovery => self.render_discovery(cx).into_any_element(),
            Screen::Peers => self.render_peers(cx).into_any_element(),
            Screen::Anime => self.render_anime(window, cx).into_any_element(),
            Screen::Storage => self.render_storage(cx).into_any_element(),
            Screen::Activity => self.render_activity(cx).into_any_element(),
            Screen::Updates => self.render_updates(cx).into_any_element(),
            Screen::Player => self.render_player(window, cx).into_any_element(),
            Screen::Streaming => self
                .streaming
                .clone()
                .unwrap_or_default()
                .view(
                    self.settings.as_ref().is_none_or(|s| s.inspection_privacy),
                    cx,
                    &self.scrolls[Screen::Streaming as usize],
                )
                .into_any_element(),
            Screen::StreamingDecoder => self
                .streaming_decoder
                .clone()
                .unwrap_or_default()
                .view(cx, &self.scrolls[Screen::StreamingDecoder as usize])
                .into_any_element(),
            Screen::MiniPlayer => self.render_mini_player(cx).into_any_element(),
        }
    }

    fn render_home(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let transfers = self.transfers;
        let status = if transfers.active == 0 {
            "Queue is idle".to_owned()
        } else {
            format!(
                "{} {}",
                transfers.active,
                if transfers.active == 1 {
                    "queued download"
                } else {
                    "queued downloads"
                }
            )
        };
        let tools = [
            ("storage", "Storage"),
            ("activity", "Activity Center"),
            ("updates", "Updates"),
            ("settings", "Settings"),
            ("diagnostics", "Diagnostics"),
            ("changelog", "What's new"),
        ]
        .into_iter()
        .map(|(id, label)| self.destination(id, label, cx))
        .collect();
        ui::page(cx)
            .relative()
            .overflow_hidden()
            .child(
                div()
                    .relative()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_2xl()
                            .font_semibold()
                            .child(semantic_text("home-title", "RustDL")),
                    )
                    .child(ui::muted_text(
                        "home-subtitle",
                        "Your library. Your next episode.",
                        cx,
                    )),
            )
            .child(
                ui::body("home-content", &self.scrolls[Screen::Home as usize])
                    .relative()
                    .child(
                        ui::card(cx)
                            .child(ui::card_title("home-library-heading", "Watch"))
                            .child(self.destination("library", "Open library", cx))
                            .child(self.destination("anime", "Browse anime", cx)),
                    )
                    .child(
                        ui::card(cx)
                            .child(ui::card_title("home-queue-heading", "Download queue"))
                            .child(
                                div()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(semantic_text("home-queue-status", status)),
                            )
                            .when(transfers.active > 0, |view| {
                                view.child(ui::muted_text(
                                    "lib-remaining-text-4",
                                    transfer_progress(transfers.downloaded, transfers.total),
                                    cx,
                                ))
                            })
                            .child(self.destination("queue", "Open download queue", cx)),
                    )
                    .child(
                        ui::card(cx)
                            .child(ui::card_title("home-tools-heading", "Tools"))
                            .child(ui::button_grid(tools)),
                    )
                    .when(self.navigation_error, |view| {
                        view.child(ui::error_text(
                            "home-route-error",
                            "Could not open that screen. Try again.",
                            cx,
                        ))
                    }),
            )
    }
}

fn open_destination(destination: &str) -> Result<(), String> {
    mobile_jni::with_env(|env| {
        let activity = mobile_jni::activity(env)?;
        let text = env
            .new_string(destination)
            .map_err(|error| error.to_string())?;
        env.call_method(
            &activity,
            jni::jni_str!("openNativeDestination"),
            jni::jni_sig!("(Ljava/lang/String;)V"),
            &[JValue::Object(&text)],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeCreate<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    activity: JObject<'local>,
) {
    unowned
        .with_env(|env| -> Result<(), NativeInitError> {
            mobile_jni::set_host_activity(env, &activity).map_err(NativeInitError)?;
            host::start(|cx| {
                gpui_kit::init(cx);
                let _ = gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                    window.on_next_frame(|_, _| READY.store(true, Ordering::Release));
                    cx.new(Home::new)
                });
            });
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeReady(
    _env: EnvUnowned,
    _class: JClass,
) -> jboolean {
    READY.load(Ordering::Acquire).into()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeSurface<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    surface: JObject<'local>,
    scale: jfloat,
) {
    unowned
        .with_env(|env| -> Result<(), jni::errors::Error> {
            // SAFETY: JNI supplied this local Surface on the attached UI thread.
            let window = unsafe {
                ndk::native_window::NativeWindow::from_surface(
                    env.get_raw().cast(),
                    surface.as_raw().cast(),
                )
            };
            if let Some(window) = window {
                host::surface_created(window, scale);
            }
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeReleaseSurface(
    _env: EnvUnowned,
    _class: JClass,
) {
    host::surface_destroyed();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeActive(
    _env: EnvUnowned,
    _class: JClass,
    active: jboolean,
) {
    ANIME_SCROLL
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .active(active);
    if active {
        host::resumed();
    } else {
        host::paused();
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeAnimeScrollEpoch(
    _env: EnvUnowned,
    _class: JClass,
) -> jlong {
    ANIME_SCROLL
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .token(SCREEN.load(Ordering::Acquire)) as jlong
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeAnimeScroll(
    _env: EnvUnowned,
    _class: JClass,
    epoch: jlong,
    delta: jfloat,
) {
    if ANIME_SCROLL.lock().unwrap_or_else(|p| p.into_inner()).push(
        SCREEN.load(Ordering::Acquire),
        epoch as u64,
        delta,
    ) {
        let _ = signal().0.try_send(());
    }
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeStopAnimeScroll(
    _env: EnvUnowned,
    _class: JClass,
    epoch: jlong,
) {
    ANIME_SCROLL
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .stop(epoch as u64);
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeTouch(
    _env: EnvUnowned,
    _class: JClass,
    action: jint,
    pointer: jint,
    x: jfloat,
    y: jfloat,
) {
    host::motion_event(action as u32, 0, &[host::Pointer { id: pointer, x, y }]);
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeKey(
    _env: EnvUnowned,
    _class: JClass,
    key: jint,
    action: jint,
    modifiers: jint,
) {
    host::key(key, action, modifiers);
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeUpdate(
    _env: EnvUnowned,
    _class: JClass,
    dark: jboolean,
    count: jint,
    downloaded: jlong,
    total: jlong,
) {
    let next = HomeSnapshot {
        dark,
        active: count.max(0) as u32,
        downloaded: downloaded.max(0) as u64,
        total: total.max(0) as u64,
    };
    {
        let mut current = SNAPSHOT
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *current == next {
            return;
        }
        *current = next;
    }
    let _ = signal().0.try_send(());
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeBack(
    _env: EnvUnowned,
    _class: JClass,
) -> jboolean {
    if Screen::current() == Screen::Library {
        let in_folder = LIBRARY
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .is_some_and(|page| !page.location.is_empty());
        if in_folder {
            let _ = request_library(0, "");
            return true;
        }
    }
    if SCREEN.swap(Screen::Home as u8, Ordering::AcqRel) != Screen::Home as u8 {
        let _ = signal().0.try_send(());
        true
    } else {
        false
    }
}

fn read_settings() -> Result<settings::Settings, String> {
    mobile_jni::with_env(|env| {
        let activity = mobile_jni::activity(env)?;
        let object = env
            .call_method(
                &activity,
                jni::jni_str!("nativeSettingsSnapshot"),
                jni::jni_sig!("()Ljava/lang/String;"),
                &[],
            )
            .map_err(|error| error.to_string())?
            .l()
            .map_err(|error| error.to_string())?;
        serde_json::from_str(&mobile_jni::get_string(env, &object))
            .map_err(|error| error.to_string())
    })
}

fn change_setting(key: &str, value: &str) -> Result<(), String> {
    mobile_jni::with_env(|env| {
        let activity = mobile_jni::activity(env)?;
        let key = env.new_string(key).map_err(|error| error.to_string())?;
        let value = env.new_string(value).map_err(|error| error.to_string())?;
        env.call_method(
            &activity,
            jni::jni_str!("changeNativeSetting"),
            jni::jni_sig!("(Ljava/lang/String;Ljava/lang/String;)V"),
            &[JValue::Object(&key), JValue::Object(&value)],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeSettings<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    object: JObject<'local>,
) {
    unowned
        .with_env(|env| -> Result<(), jni::errors::Error> {
            if let Ok(settings) = serde_json::from_str(&mobile_jni::get_string(env, &object)) {
                let mut current = SETTINGS.lock().unwrap_or_else(|p| p.into_inner());
                if current.as_ref() == Some(&settings) {
                    return Ok(());
                }
                PRIVACY_READY.store(false, Ordering::Release);
                PRIVACY_REVISION.fetch_add(1, Ordering::AcqRel);
                gpui::rustdl_a11y_action_policy::invalidate();
                *current = Some(settings);
                drop(current);
                let _ = signal().0.try_send(());
            }
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

fn set_screen(screen: Screen, ordering: Ordering) {
    if SCREEN.swap(screen as u8, ordering) != screen as u8 {
        ANIME_SCROLL
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .invalidate();
        gpui::rustdl_a11y_action_policy::invalidate();
    }
}

fn notify_screen(screen: &str) -> Result<(), String> {
    mobile_jni::with_env(|env| {
        let activity = mobile_jni::activity(env)?;
        let screen = env.new_string(screen).map_err(|error| error.to_string())?;
        env.call_method(
            &activity,
            jni::jni_str!("nativeScreenChanged"),
            jni::jni_sig!("(Ljava/lang/String;)V"),
            &[JValue::Object(&screen)],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    })
}

fn read_queue(offset: i32) -> Result<queue::QueuePage, String> {
    mobile_jni::with_env(|env| {
        let activity = mobile_jni::activity(env)?;
        let object = env
            .call_method(
                &activity,
                jni::jni_str!("nativeQueueSnapshot"),
                jni::jni_sig!("(I)Ljava/lang/String;"),
                &[JValue::Int(offset)],
            )
            .map_err(|error| error.to_string())?
            .l()
            .map_err(|error| error.to_string())?;
        serde_json::from_str(&mobile_jni::get_string(env, &object))
            .map_err(|error| error.to_string())
    })
}

fn change_queue_item(id: &str, action: &str) -> Result<(), String> {
    mobile_jni::with_env(|env| {
        let activity = mobile_jni::activity(env)?;
        let id = env.new_string(id).map_err(|error| error.to_string())?;
        let action = env.new_string(action).map_err(|error| error.to_string())?;
        env.call_method(
            &activity,
            jni::jni_str!("changeNativeQueueItem"),
            jni::jni_sig!("(Ljava/lang/String;Ljava/lang/String;)V"),
            &[JValue::Object(&id), JValue::Object(&action)],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeQueue<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    object: JObject<'local>,
) {
    unowned
        .with_env(|env| -> Result<(), jni::errors::Error> {
            if let Ok(queue) = serde_json::from_str(&mobile_jni::get_string(env, &object)) {
                *QUEUE.lock().unwrap_or_else(|p| p.into_inner()) = Some(queue);
                let _ = signal().0.try_send(());
            }
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeShowQueue(
    _env: EnvUnowned,
    _class: JClass,
) {
    set_screen(Screen::Queue, Ordering::Release);
    let _ = signal().0.try_send(());
}

fn read_diagnostics() -> Result<diagnostics::Diagnostics, String> {
    mobile_jni::with_env(|env| {
        let activity = mobile_jni::activity(env)?;
        let object = env
            .call_method(
                &activity,
                jni::jni_str!("nativeDiagnosticsSnapshot"),
                jni::jni_sig!("()Ljava/lang/String;"),
                &[],
            )
            .map_err(|error| error.to_string())?
            .l()
            .map_err(|error| error.to_string())?;
        serde_json::from_str(&mobile_jni::get_string(env, &object))
            .map_err(|error| error.to_string())
    })
}

fn copy_diagnostics() -> Result<(), String> {
    mobile_jni::with_env(|env| {
        let activity = mobile_jni::activity(env)?;
        env.call_method(
            &activity,
            jni::jni_str!("copyNativeDiagnostics"),
            jni::jni_sig!("()V"),
            &[],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeDiagnostics<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    object: JObject<'local>,
) {
    unowned
        .with_env(|env| -> Result<(), jni::errors::Error> {
            if let Ok(diagnostics) = serde_json::from_str(&mobile_jni::get_string(env, &object)) {
                *DIAGNOSTICS.lock().unwrap_or_else(|p| p.into_inner()) = Some(diagnostics);
                let _ = signal().0.try_send(());
            }
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

fn approve_mobile_downloads() -> Result<(), String> {
    mobile_jni::with_env(|env| {
        let activity = mobile_jni::activity(env)?;
        env.call_method(
            &activity,
            jni::jni_str!("approveNativeMobileDownloads"),
            jni::jni_sig!("()V"),
            &[],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    })
}

fn request_library(offset: i32, location: &str) -> Result<(), String> {
    mobile_jni::with_env(|env| {
        let activity = mobile_jni::activity(env)?;
        let location = env
            .new_string(location)
            .map_err(|error| error.to_string())?;
        env.call_method(
            &activity,
            jni::jni_str!("requestNativeLibrary"),
            jni::jni_sig!("(ILjava/lang/String;)V"),
            &[JValue::Int(offset), JValue::Object(&location)],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    })
}

fn library_action(id: &str, action: &str) -> Result<(), String> {
    mobile_jni::with_env(|env| {
        let activity = mobile_jni::activity(env)?;
        let id = env.new_string(id).map_err(|error| error.to_string())?;
        let action = env.new_string(action).map_err(|error| error.to_string())?;
        env.call_method(
            &activity,
            jni::jni_str!("changeNativeLibraryItem"),
            jni::jni_sig!("(Ljava/lang/String;Ljava/lang/String;)V"),
            &[JValue::Object(&id), JValue::Object(&action)],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    })
}

fn search_library() -> Result<(), String> {
    mobile_jni::with_env(|env| {
        let activity = mobile_jni::activity(env)?;
        env.call_method(
            &activity,
            jni::jni_str!("searchNativeLibrary"),
            jni::jni_sig!("()V"),
            &[],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    })
}

fn library_thumbnail(id: &str) -> Result<String, String> {
    mobile_jni::with_env(|env| {
        let activity = mobile_jni::activity(env)?;
        let id = env.new_string(id).map_err(|error| error.to_string())?;
        let value = env
            .call_method(
                &activity,
                jni::jni_str!("nativeLibraryThumbnailPath"),
                jni::jni_sig!("(Ljava/lang/String;)Ljava/lang/String;"),
                &[JValue::Object(&id)],
            )
            .map_err(|error| error.to_string())?
            .l()
            .map_err(|error| error.to_string())?;
        Ok(mobile_jni::get_string(env, &value))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeScreen(
    _env: EnvUnowned,
    _class: JClass,
) -> jint {
    SCREEN.load(Ordering::Acquire) as jint
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeLibrary<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    object: JObject<'local>,
) {
    unowned
        .with_env(|env| -> Result<(), jni::errors::Error> {
            if let Ok(library) = serde_json::from_str(&mobile_jni::get_string(env, &object)) {
                *LIBRARY.lock().unwrap_or_else(|p| p.into_inner()) = Some(library);
                let _ = signal().0.try_send(());
            }
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativePrivacyReady(
    _env: EnvUnowned,
    _class: JClass,
) -> jboolean {
    PRIVACY_READY.load(Ordering::Acquire)
        && PRIVACY_ACK_REVISION.load(Ordering::Acquire) == PRIVACY_REVISION.load(Ordering::Acquire)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeDiscovery<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    object: JObject<'local>,
) {
    unowned
        .with_env(|env| -> Result<(), jni::errors::Error> {
            if let Ok(page) = serde_json::from_str(&mobile_jni::get_string(env, &object)) {
                *DISCOVERY.lock().unwrap_or_else(|p| p.into_inner()) = Some(page);
                let _ = signal().0.try_send(());
            }
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeShowDiscovery(
    _env: EnvUnowned,
    _class: JClass,
) {
    set_screen(Screen::Discovery, Ordering::Release);
    let _ = notify_screen("discovery");
    let _ = signal().0.try_send(());
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeShowPeers(
    _env: EnvUnowned,
    _class: JClass,
) {
    *PEERS.lock().unwrap_or_else(|p| p.into_inner()) = None;
    set_screen(Screen::Peers, Ordering::Release);
    let _ = notify_screen("peers");
    let _ = signal().0.try_send(());
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativePeers<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    object: JObject<'local>,
) {
    unowned
        .with_env(|env| -> Result<(), jni::errors::Error> {
            if let Ok(page) = serde_json::from_str(&mobile_jni::get_string(env, &object)) {
                *PEERS.lock().unwrap_or_else(|p| p.into_inner()) = Some(page);
                let _ = signal().0.try_send(());
            }
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeAnime<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    object: JObject<'local>,
) {
    unowned
        .with_env(|env| -> Result<(), jni::errors::Error> {
            if let Ok(page) = serde_json::from_str(&mobile_jni::get_string(env, &object)) {
                *ANIME.lock().unwrap_or_else(|p| p.into_inner()) = Some(page);
                let _ = signal().0.try_send(());
            }
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeStorage<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    object: JObject<'local>,
) {
    unowned
        .with_env(|env| -> Result<(), jni::errors::Error> {
            if let Ok(page) = serde_json::from_str(&mobile_jni::get_string(env, &object)) {
                *STORAGE.lock().unwrap_or_else(|p| p.into_inner()) = Some(page);
                let _ = signal().0.try_send(());
            }
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeActivity<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    object: JObject<'local>,
) {
    unowned
        .with_env(|env| -> Result<(), jni::errors::Error> {
            if let Ok(page) = serde_json::from_str(&mobile_jni::get_string(env, &object)) {
                *ACTIVITY.lock().unwrap_or_else(|p| p.into_inner()) = Some(page);
                let _ = signal().0.try_send(());
            }
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeUpdates<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    object: JObject<'local>,
) {
    unowned
        .with_env(|env| -> Result<(), jni::errors::Error> {
            if let Ok(page) = serde_json::from_str(&mobile_jni::get_string(env, &object)) {
                *UPDATES.lock().unwrap_or_else(|p| p.into_inner()) = Some(page);
                let _ = signal().0.try_send(());
            }
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativePlayer<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    object: JObject<'local>,
) {
    unowned
        .with_env(|env| -> Result<(), jni::errors::Error> {
            if let Ok(page) = serde_json::from_str(&mobile_jni::get_string(env, &object)) {
                *PLAYER.lock().unwrap_or_else(|p| p.into_inner()) = Some(page);
                let _ = signal().0.try_send(());
            }
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeShowPlayer(
    _env: EnvUnowned,
    _class: JClass,
) {
    *PLAYER.lock().unwrap_or_else(|p| p.into_inner()) = None;
    set_screen(Screen::Player, Ordering::Release);
    let _ = notify_screen("player");
    let _ = signal().0.try_send(());
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeStreaming<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    object: JObject<'local>,
) {
    unowned
        .with_env(|env| -> Result<(), jni::errors::Error> {
            if let Ok(page) = serde_json::from_str(&mobile_jni::get_string(env, &object)) {
                *STREAMING.lock().unwrap_or_else(|p| p.into_inner()) = Some(page);
                let _ = signal().0.try_send(());
            }
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeShowStreaming(
    _env: EnvUnowned,
    _class: JClass,
) {
    *STREAMING.lock().unwrap_or_else(|p| p.into_inner()) = None;
    set_screen(Screen::Streaming, Ordering::Release);
    let _ = notify_screen("streaming");
    let _ = signal().0.try_send(());
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeSelectStreamingDecoder(
    _env: EnvUnowned,
    _class: JClass,
) {
    set_screen(Screen::StreamingDecoder, Ordering::Release);
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeStreamingDecoder<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass,
    data: JObject<'local>,
) {
    unowned
        .with_env(|env| -> Result<(), jni::errors::Error> {
            let data = mobile_jni::get_string(env, &data);
            if let Ok(page) = serde_json::from_str::<streaming_decoder::Page>(&data) {
                let mut previous = STREAMING_DECODER.lock().unwrap_or_else(|p| p.into_inner());
                if previous.as_ref().is_none_or(|old| {
                    old.privacy != page.privacy || old.generation != page.generation
                }) {
                    PRIVACY_REVISION.fetch_add(1, Ordering::AcqRel);
                    gpui::rustdl_a11y_action_policy::invalidate();
                }
                *previous = Some(page);
                let _ = signal().0.try_send(());
            }
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativePlayerLayout(
    _env: EnvUnowned,
    _class: JClass,
    layout: jint,
) {
    let (screen, name) = match layout {
        0 => (Screen::Player, "player"),
        1 => (Screen::MiniPlayer, "mini-player"),
        2 => (Screen::Library, "library"),
        _ => return,
    };
    set_screen(screen, Ordering::Release);
    let _ = notify_screen(name);
    let _ = signal().0.try_send(());
}

impl Render for Home {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        gpui_mobile::rustdl_accessibility::set_context(
            PRIVACY_REVISION.load(Ordering::Acquire),
            self.screen as u8,
        );
        let content = self.render_content(window, cx);
        if self.screen == Screen::StreamingDecoder {
            return content;
        }
        let scroll = &self.scrolls[self.screen.scroll_index()];
        let (scroll_offset, scroll_max) = if self.screen == Screen::History {
            (
                self.history_list.scroll_px_offset_for_scrollbar(),
                self.history_list.max_offset_for_scrollbar(),
            )
        } else {
            (scroll.offset(), scroll.max_offset())
        };
        div()
            .relative()
            .size_full()
            .overflow_hidden()
            .bg(cx.theme().background)
            .child(artwork::background(
                &mut self.artwork_state,
                self.settings.as_ref(),
                self.transfers.dark,
                window.viewport_size(),
                scroll_offset,
                scroll_max,
            ))
            .child(div().relative().size_full().child(content))
            .into_any_element()
    }
}

fn accessibility_tree() -> &'static Mutex<accessibility::SemanticTree> {
    ACCESSIBILITY_TREE.get_or_init(|| Mutex::new(accessibility::SemanticTree::default()))
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeAccessibilityActive(
    _env: EnvUnowned,
    _class: JClass,
    active: jboolean,
) {
    accessibility_tree()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clear();
    *ACCESSIBILITY_CONTEXT
        .lock()
        .unwrap_or_else(|p| p.into_inner()) = None;
    gpui_mobile::rustdl_accessibility::set_active(active);
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeAccessibilitySnapshot<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass,
) -> jni::sys::jstring {
    unowned.with_env(|env| -> Result<jni::sys::jstring, jni::errors::Error> {
        let current = (PRIVACY_REVISION.load(Ordering::Acquire), SCREEN.load(Ordering::Acquire));
        let changed = {
            let mut context = ACCESSIBILITY_CONTEXT.lock().unwrap_or_else(|p| p.into_inner());
            let changed = *context != Some(current);
            *context = Some(current);
            changed
        };
        if changed || gpui_mobile::rustdl_accessibility::take_reset_required() {
            accessibility_tree().lock().unwrap_or_else(|p| p.into_inner()).clear();
            gpui_mobile::rustdl_accessibility::set_active(false);
            gpui_mobile::rustdl_accessibility::set_active(true);
        }
        let mut tree = accessibility_tree().lock().unwrap_or_else(|p| p.into_inner());
        for (revision, screen, update) in gpui_mobile::rustdl_accessibility::take_updates() {
            if (revision, screen) == current { tree.apply(revision, screen, update); }
        }
        let data = serde_json::json!({"revision":current.0.to_string(), "screen":current.1,
            "root":tree.root.map(|id|id.0.to_string()), "focus":tree.focus.map(|id|id.0.to_string()), "nodes":tree.project()});
        Ok(env.new_string(data.to_string())?.into_raw())
    }).resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeHomeHost_nativeAccessibilityAction<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass,
    id: JObject<'local>,
    action: jint,
    value: jni::sys::jdouble,
    revision: jlong,
    screen: jint,
) -> jboolean {
    unowned
        .with_env(|env| -> Result<jboolean, jni::errors::Error> {
            if revision < 0 || !(0..=255).contains(&screen) || !(1..=11).contains(&action) {
                return Ok(false);
            }
            let current = (
                PRIVACY_REVISION.load(Ordering::Acquire),
                SCREEN.load(Ordering::Acquire),
            );
            if current != (revision as u64, screen as u8) {
                return Ok(false);
            }
            let id = mobile_jni::get_string(env, &id);
            let request = accessibility_tree()
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .action_request(current, &id, action as u8, value);
            Ok(request.is_some_and(|request| {
                gpui_mobile::rustdl_accessibility::dispatch(|callbacks| (callbacks.action)(request))
            }))
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

/// Source-only visual fixtures in an isolated Activity, without engine data access.
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_NativeVisualHost_nativeVisual<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass,
    screen: JObject<'local>,
) {
    unowned.with_env(|env| -> Result<(), jni::errors::Error> {
        let screen=mobile_jni::get_string(env,&screen);
        *SETTINGS.lock().unwrap_or_else(|p|p.into_inner())=
            serde_json::from_str(r#"{"ok":true,"detail":"","downloadFolder":"Synthetic","keepScreenAwake":false,"allowScreenshots":true,"inspectionPrivacy":true,"diagnosticsRefreshSeconds":15,"appearance":"dark","backgroundTheme":"plain","spaceEffectEnabled":false,"reduceMotion":true,"mobileDownloadPolicy":"wifi"}"#).ok();
        let selected=match screen.as_str() {
            "history"=>Screen::History, "settings"=>Screen::Settings, "anime"=>{
                *ANIME.lock().unwrap_or_else(|p|p.into_inner())=Some(anime::Page{
                    ok:true,kind:"catalog".into(),page:1,pages:3,
                    items:(0..12).map(|index|anime::Item{id:format!("synthetic-{index}"),title:"Synthetic anime".into(),poster_width: match index % 3 {0=>2,1=>1,_=>16},poster_height: match index % 3 {0=>3,1=>1,_=>9},sub:Some("12".into()),dub:Some("6".into()),..Default::default()}).collect(),
                    ..Default::default()
                });Screen::Anime
            }, _=>Screen::Home
        };
        set_screen(selected,Ordering::Release);
        PRIVACY_REVISION.fetch_add(1,Ordering::AcqRel);
        let _=signal().0.try_send(());
        Ok(())
    }).resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}
