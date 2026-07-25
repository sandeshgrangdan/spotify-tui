use super::user_config::UserConfig;
use crate::network::IoEvent;
use anyhow::anyhow;
use rspotify::prelude::Id;
use rspotify::model::{
  album::{FullAlbum, SavedAlbum, SimplifiedAlbum},
  artist::FullArtist,
  audio::AudioAnalysis,
  context::{CurrentPlaybackContext, CurrentUserQueue},
  device::DevicePayload,
  page::{CursorBasedPage, Page},
  playing::PlayHistory,
  playlist::{PlaylistItem, SimplifiedPlaylist},
  show::{FullShow, Show, SimplifiedEpisode, SimplifiedShow},
  track::{FullTrack, SavedTrack, SimplifiedTrack},
  user::PrivateUser,
  Country, PlayableItem,
};
use std::sync::mpsc::Sender;
use std::{
  cmp::{max, min},
  collections::{HashMap, HashSet},
  time::{Duration, Instant, SystemTime},
};
use ratatui::layout::Rect;

use arboard::Clipboard;

pub const LIBRARY_OPTIONS: [&str; 8] = [
  "Made For You",
  "Recently Played",
  "Liked Songs",
  "Albums",
  "Artists",
  "Podcasts",
  "New Releases",
  "Top Tracks",
];

const DEFAULT_ROUTE: Route = Route {
  id: RouteId::Home,
  active_block: ActiveBlock::Empty,
  hovered_block: ActiveBlock::Library,
};

#[derive(Clone)]
pub struct ScrollableResultPages<T> {
  index: usize,
  pub pages: Vec<T>,
}

impl<T> ScrollableResultPages<T> {
  pub fn new() -> ScrollableResultPages<T> {
    ScrollableResultPages {
      index: 0,
      pages: vec![],
    }
  }

  pub fn get_results(&self, at_index: Option<usize>) -> Option<&T> {
    self.pages.get(at_index.unwrap_or(self.index))
  }

  pub fn get_mut_results(&mut self, at_index: Option<usize>) -> Option<&mut T> {
    self.pages.get_mut(at_index.unwrap_or(self.index))
  }

  pub fn add_pages(&mut self, new_pages: T) {
    self.pages.push(new_pages);
    // Whenever a new page is added, set the active index to the end of the vector
    self.index = self.pages.len() - 1;
  }
}

#[derive(Default)]
pub struct SpotifyResultAndSelectedIndex<T> {
  pub index: usize,
  pub result: T,
}

#[derive(Clone)]
pub struct Library {
  pub selected_index: usize,
  pub saved_tracks: ScrollableResultPages<Page<SavedTrack>>,
  pub made_for_you_playlists: ScrollableResultPages<Page<SimplifiedPlaylist>>,
  pub saved_albums: ScrollableResultPages<Page<SavedAlbum>>,
  pub saved_shows: ScrollableResultPages<Page<Show>>,
  pub saved_artists: ScrollableResultPages<CursorBasedPage<FullArtist>>,
  pub show_episodes: ScrollableResultPages<Page<SimplifiedEpisode>>,
  pub new_releases: ScrollableResultPages<Page<SimplifiedAlbum>>,
}

#[derive(PartialEq, Debug)]
pub enum SearchResultBlock {
  AlbumSearch,
  SongSearch,
  ArtistSearch,
  PlaylistSearch,
  ShowSearch,
  EpisodeSearch,
  Empty,
}

#[derive(PartialEq, Debug, Clone)]
pub enum ArtistBlock {
  TopTracks,
  Albums,
  RelatedArtists,
  Empty,
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum HomeMode {
  Music,
  Podcast,
}

/// A shelf on the home screen. Music mode and podcast mode each own three or
/// four of these; see `home_sections::sections` for the order they appear in.
#[derive(PartialEq, Debug, Clone, Copy)]
pub enum HomeBlock {
  MadeForYou,
  RecommendedStations,
  JumpBackIn,
  TopArtists,
  YourShows,
  ContinueListening,
  LatestEpisodes,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum DialogContext {
  PlaylistWindow,
  PlaylistSearch,
}

/// What the shared text-input box is currently used for.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum InputMode {
  Search,
  NewPlaylist,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ActiveBlock {
  Analysis,
  PlayBar,
  AlbumTracks,
  AlbumList,
  ArtistBlock,
  Empty,
  HelpMenu,
  Home,
  Input,
  Library,
  MyPlaylists,
  Devices,
  Podcasts,
  Queue,
  EpisodeTable,
  RecentlyPlayed,
  SearchResultBlock,
  SelectDevice,
  TrackTable,
  MadeForYou,
  Artists,
  BasicView,
  Dialog(DialogContext),
  PlaylistPicker,
}

#[derive(Clone, PartialEq, Debug)]
pub enum RouteId {
  Analysis,
  AlbumTracks,
  AlbumList,
  Artist,
  BasicView,
  Home,
  RecentlyPlayed,
  Search,
  SelectedDevice,
  TrackTable,
  MadeForYou,
  Artists,
  Podcasts,
  Queue,
  PodcastEpisodes,
  Recommendations,
  Dialog,
}

#[derive(Debug)]
pub struct Route {
  pub id: RouteId,
  pub active_block: ActiveBlock,
  pub hovered_block: ActiveBlock,
}

// Is it possible to compose enums?
#[derive(PartialEq, Debug)]
pub enum TrackTableContext {
  MyPlaylists,
  AlbumSearch,
  PlaylistSearch,
  SavedTracks,
  RecommendedTracks,
  MadeForYou,
  TopTracks,
}

// Is it possible to compose enums?
#[derive(Clone, PartialEq, Debug, Copy)]
pub enum AlbumTableContext {
  Simplified,
  Full,
}

#[derive(Clone, PartialEq, Debug, Copy)]
pub enum EpisodeTableContext {
  Simplified,
  Full,
}

/// Which collection the album-list screen is currently showing.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AlbumListContext {
  SavedAlbums,
  NewReleases,
}

#[derive(Clone, PartialEq, Debug)]
pub enum RecommendationsContext {
  Artist,
  Song,
  /// A home-screen mix, whose name is already descriptive ("Rock Mix").
  Mix,
}

/// A transient corner notification.
///
/// Replaces the old full-screen error route: an API error is usually something
/// the user can ignore (a 403 from a command that no longer applies), so it
/// shouldn't take over the screen and wait to be dismissed.
#[derive(Clone, Debug)]
pub struct Toast {
  pub message: String,
  /// Short actionable line, kept from the troubleshooting text the old error
  /// screen used to show.
  pub hint: Option<&'static str>,
  created_at: Instant,
}

impl Toast {
  /// How long a toast stays on screen.
  const LIFETIME: Duration = Duration::from_secs(5);

  pub fn error(message: String) -> Self {
    let hint = if message.contains("403") {
      Some("Needs Premium and an active device (press d)")
    } else if message.contains("404") {
      Some("Device may be asleep — press d to re-select")
    } else {
      None
    };
    Toast {
      message,
      hint,
      created_at: Instant::now(),
    }
  }

  pub fn is_expired(&self) -> bool {
    self.created_at.elapsed() >= Self::LIFETIME
  }
}

#[derive(Clone, Debug)]
pub struct Lyrics {
  /// Synced lines parsed from LRC format. First field is milliseconds from track start.
  pub synced: Vec<(u32, String)>,
  /// Whole-track plain text. Used as a fallback when `synced` is empty.
  pub plain: Option<String>,
}

pub struct SearchResult {
  pub albums: Option<Page<SimplifiedAlbum>>,
  pub artists: Option<Page<FullArtist>>,
  pub playlists: Option<Page<SimplifiedPlaylist>>,
  pub tracks: Option<Page<FullTrack>>,
  pub shows: Option<Page<SimplifiedShow>>,
  pub episodes: Option<Page<SimplifiedEpisode>>,
  pub selected_album_index: Option<usize>,
  pub selected_artists_index: Option<usize>,
  pub selected_playlists_index: Option<usize>,
  pub selected_tracks_index: Option<usize>,
  pub selected_shows_index: Option<usize>,
  pub selected_episodes_index: Option<usize>,
  pub hovered_block: SearchResultBlock,
  pub selected_block: SearchResultBlock,
}

#[derive(Default)]
pub struct TrackTable {
  pub tracks: Vec<FullTrack>,
  pub selected_index: usize,
  pub context: Option<TrackTableContext>,
}

#[derive(Clone)]
pub struct SelectedShow {
  pub show: SimplifiedShow,
}

#[derive(Clone)]
pub struct SelectedFullShow {
  pub show: FullShow,
}

#[derive(Clone)]
pub struct SelectedAlbum {
  pub album: SimplifiedAlbum,
  pub tracks: Page<SimplifiedTrack>,
  pub selected_index: usize,
}

#[derive(Clone)]
pub struct SelectedFullAlbum {
  pub album: FullAlbum,
  pub selected_index: usize,
}

#[derive(Clone)]
pub struct Artist {
  pub artist_name: String,
  pub albums: Page<SimplifiedAlbum>,
  pub related_artists: Vec<FullArtist>,
  pub top_tracks: Vec<FullTrack>,
  pub selected_album_index: usize,
  pub selected_related_artist_index: usize,
  pub selected_top_track_index: usize,
  pub artist_hovered_block: ArtistBlock,
  pub artist_selected_block: ArtistBlock,
}

pub struct App {
  pub instant_since_last_current_playback_poll: Instant,
  navigation_stack: Vec<Route>,
  pub audio_analysis: Option<AudioAnalysis>,
  pub home_selected_block: HomeBlock,
  /// False while the cursor picks a whole section, true once it moves inside
  /// that section's list.
  pub home_section_entered: bool,
  pub home_jump_back_index: usize,
  pub home_made_for_you_index: usize,
  pub home_recommended_index: usize,
  pub home_top_artists_index: usize,
  pub home_mode: HomeMode,
  pub podcast_episodes_per_show: HashMap<String, Vec<SimplifiedEpisode>>,
  /// Shows whose episode fetch has already been dispatched. Without this the
  /// per-tick top-up would re-request shows whose fetch is still in flight (or
  /// failed silently) several times a second.
  podcast_episodes_requested: HashSet<String>,
  pub home_your_shows_index: usize,
  pub home_continue_listening_index: usize,
  pub home_latest_episodes_index: usize,
  pub podcast_home_fetched: bool,
  pub made_for_you_populated: bool,
  pub top_artists: Vec<FullArtist>,
  /// Short-term top tracks — the app's "On Repeat".
  pub on_repeat_tracks: Vec<FullTrack>,
  pub user_config: UserConfig,
  pub artists: Vec<FullArtist>,
  pub artist: Option<Artist>,
  pub album_table_context: AlbumTableContext,
  pub album_list_context: AlbumListContext,
  pub saved_album_tracks_index: usize,
  /// Last API error, read by the CLI to decide a command failed.
  pub api_error: String,
  /// Transient notification shown in the corner of the TUI.
  pub toast: Option<Toast>,
  pub current_playback_context: Option<CurrentPlaybackContext>,
  pub queue: Option<CurrentUserQueue>,
  pub queue_selected_index: usize,
  pub lyrics_visible: bool,
  pub lyrics: Option<Lyrics>,
  pub lyrics_for_track_id: Option<String>,
  pub lyrics_loading: bool,
  pub made_for_you_previews: HashMap<String, String>,
  pub devices: Option<DevicePayload>,
  // Inputs:
  // input is the string for input;
  // input_idx is the index of the cursor in terms of character;
  // input_cursor_position is the sum of the width of characters preceding the cursor.
  // Reason for this complication is due to non-ASCII characters, they may
  // take more than 1 bytes to store and more than 1 character width to display.
  pub input: Vec<char>,
  pub input_idx: usize,
  pub input_mode: InputMode,
  pub playlist_picker_uri: Option<String>,
  pub playlist_picker_index: usize,
  pub input_cursor_position: u16,
  pub liked_song_ids_set: HashSet<String>,
  pub followed_artist_ids_set: HashSet<String>,
  pub saved_album_ids_set: HashSet<String>,
  pub saved_show_ids_set: HashSet<String>,
  pub large_search_limit: u32,
  pub library: Library,
  pub playlist_offset: u32,
  pub made_for_you_offset: u32,
  pub playlist_tracks: Option<Page<PlaylistItem>>,
  pub made_for_you_tracks: Option<Page<PlaylistItem>>,
  pub playlists: Option<Page<SimplifiedPlaylist>>,
  pub recently_played: SpotifyResultAndSelectedIndex<Option<CursorBasedPage<PlayHistory>>>,
  pub recommended_tracks: Vec<FullTrack>,
  pub recommendations_seed: String,
  pub recommendations_context: Option<RecommendationsContext>,
  pub search_results: SearchResult,
  pub selected_album_simplified: Option<SelectedAlbum>,
  pub selected_album_full: Option<SelectedFullAlbum>,
  pub selected_device_index: Option<usize>,
  pub selected_playlist_index: Option<usize>,
  pub active_playlist_index: Option<usize>,
  pub active_playlist_id: Option<String>,
  pub size: Rect,
  pub small_search_limit: u32,
  pub song_progress_ms: u128,
  pub seek_ms: Option<u128>,
  pub track_table: TrackTable,
  pub episode_table_context: EpisodeTableContext,
  pub selected_show_simplified: Option<SelectedShow>,
  pub selected_show_full: Option<SelectedFullShow>,
  pub user: Option<PrivateUser>,
  pub album_list_index: usize,
  pub made_for_you_index: usize,
  pub artists_list_index: usize,
  pub clipboard: Option<Clipboard>,
  pub shows_list_index: usize,
  pub episode_list_index: usize,
  pub help_docs_size: u32,
  pub help_menu_page: u32,
  pub help_menu_max_lines: u32,
  pub help_menu_offset: u32,
  /// Drives the "Loading…" hint. Derived from `pending_io`.
  pub is_loading: bool,
  /// Dispatched IoEvents that haven't reported back yet. A plain flag flickered
  /// off as soon as the *first* of several queued requests finished.
  pending_io: usize,
  io_tx: Option<Sender<IoEvent>>,
  pub is_fetching_current_playback: bool,
  pub spotify_token_expiry: SystemTime,
  pub dialog: Option<String>,
  pub confirm: bool,
}

impl Default for App {
  fn default() -> Self {
    App {
      audio_analysis: None,
      album_table_context: AlbumTableContext::Full,
      album_list_context: AlbumListContext::SavedAlbums,
      album_list_index: 0,
      made_for_you_index: 0,
      artists_list_index: 0,
      shows_list_index: 0,
      episode_list_index: 0,
      artists: vec![],
      artist: None,
      user_config: UserConfig::new(),
      saved_album_tracks_index: 0,
      recently_played: Default::default(),
      size: Rect::default(),
      selected_album_simplified: None,
      selected_album_full: None,
      home_selected_block: HomeBlock::MadeForYou,
      home_section_entered: false,
      home_jump_back_index: 0,
      home_made_for_you_index: 0,
      home_recommended_index: 0,
      home_top_artists_index: 0,
      home_mode: HomeMode::Music,
      podcast_episodes_per_show: HashMap::new(),
      podcast_episodes_requested: HashSet::new(),
      home_your_shows_index: 0,
      home_continue_listening_index: 0,
      home_latest_episodes_index: 0,
      podcast_home_fetched: false,
      made_for_you_populated: false,
      top_artists: Vec::new(),
      on_repeat_tracks: Vec::new(),
      library: Library {
        saved_tracks: ScrollableResultPages::new(),
        made_for_you_playlists: ScrollableResultPages::new(),
        saved_albums: ScrollableResultPages::new(),
        saved_shows: ScrollableResultPages::new(),
        saved_artists: ScrollableResultPages::new(),
        show_episodes: ScrollableResultPages::new(),
        new_releases: ScrollableResultPages::new(),
        selected_index: 0,
      },
      liked_song_ids_set: HashSet::new(),
      followed_artist_ids_set: HashSet::new(),
      saved_album_ids_set: HashSet::new(),
      saved_show_ids_set: HashSet::new(),
      navigation_stack: vec![DEFAULT_ROUTE],
      large_search_limit: 20,
      small_search_limit: 4,
      api_error: String::new(),
      toast: None,
      current_playback_context: None,
      queue: None,
      queue_selected_index: 0,
      lyrics_visible: false,
      lyrics: None,
      lyrics_for_track_id: None,
      lyrics_loading: false,
      made_for_you_previews: HashMap::new(),
      devices: None,
      input: vec![],
      input_idx: 0,
      input_mode: InputMode::Search,
      playlist_picker_uri: None,
      playlist_picker_index: 0,
      input_cursor_position: 0,
      playlist_offset: 0,
      made_for_you_offset: 0,
      playlist_tracks: None,
      made_for_you_tracks: None,
      playlists: None,
      recommended_tracks: vec![],
      recommendations_context: None,
      recommendations_seed: "".to_string(),
      search_results: SearchResult {
        hovered_block: SearchResultBlock::SongSearch,
        selected_block: SearchResultBlock::Empty,
        albums: None,
        artists: None,
        playlists: None,
        shows: None,
        episodes: None,
        selected_album_index: None,
        selected_artists_index: None,
        selected_playlists_index: None,
        selected_tracks_index: None,
        selected_shows_index: None,
        selected_episodes_index: None,
        tracks: None,
      },
      song_progress_ms: 0,
      seek_ms: None,
      selected_device_index: None,
      selected_playlist_index: None,
      active_playlist_index: None,
      active_playlist_id: None,
      track_table: Default::default(),
      episode_table_context: EpisodeTableContext::Full,
      selected_show_simplified: None,
      selected_show_full: None,
      user: None,
      instant_since_last_current_playback_poll: Instant::now(),
      clipboard: Clipboard::new().ok(),
      help_docs_size: 0,
      help_menu_page: 0,
      help_menu_max_lines: 0,
      help_menu_offset: 0,
      is_loading: false,
      pending_io: 0,
      io_tx: None,
      is_fetching_current_playback: false,
      spotify_token_expiry: SystemTime::now(),
      dialog: None,
      confirm: false,
    }
  }
}

impl App {
  pub fn new(
    io_tx: Sender<IoEvent>,
    user_config: UserConfig,
    spotify_token_expiry: SystemTime,
  ) -> App {
    App {
      io_tx: Some(io_tx),
      user_config,
      spotify_token_expiry,
      ..App::default()
    }
  }

  // Send a network event to the network thread
  /// Queue work for the network thread. `io_finished` is called for each event
  /// once it has been handled, which is what turns the loading hint back off.
  pub fn dispatch(&mut self, action: IoEvent) {
    self.pending_io += 1;
    self.is_loading = true;
    if let Some(io_tx) = &self.io_tx {
      if let Err(e) = io_tx.send(action) {
        // Nothing will ever report back for this one.
        self.io_finished();
        println!("Error from dispatch {}", e);
      };
    }
  }

  /// One dispatched IoEvent has been handled. The loading hint clears only once
  /// the whole queue has drained.
  pub fn io_finished(&mut self) {
    self.pending_io = self.pending_io.saturating_sub(1);
    self.is_loading = self.pending_io > 0;
  }

  fn apply_seek(&mut self, seek_ms: u32) {
    if let Some(CurrentPlaybackContext {
      item: Some(item), ..
    }) = &self.current_playback_context
    {
      let duration_ms = match item {
        PlayableItem::Track(track) => track.duration.num_milliseconds() as u32,
        PlayableItem::Episode(episode) => episode.duration.num_milliseconds() as u32,
        PlayableItem::Unknown(_) => return,
      };

      let event = if seek_ms < duration_ms {
        IoEvent::Seek(seek_ms)
      } else {
        IoEvent::NextTrack
      };

      self.dispatch(event);
    }
  }

  fn poll_current_playback(&mut self) {
    // Poll every 5 seconds
    let poll_interval_ms = 5_000;

    let elapsed = self
      .instant_since_last_current_playback_poll
      .elapsed()
      .as_millis();

    if !self.is_fetching_current_playback && elapsed >= poll_interval_ms {
      self.is_fetching_current_playback = true;
      // Trigger the seek if the user has set a new position
      match self.seek_ms {
        Some(seek_ms) => self.apply_seek(seek_ms as u32),
        None => self.dispatch(IoEvent::GetCurrentPlayback),
      }
    }
  }

  pub fn update_on_tick(&mut self) {
    self.poll_current_playback();
    if matches!(self.home_mode, HomeMode::Podcast) {
      self.fetch_missing_podcast_episodes();
    }
    if matches!(&self.toast, Some(toast) if toast.is_expired()) {
      self.toast = None;
    }
    if let Some(CurrentPlaybackContext {
      item: Some(item),
      progress,
      is_playing,
      ..
    }) = &self.current_playback_context
    {
      if let Some(progress_duration) = progress {
        let progress_ms = progress_duration.num_milliseconds() as u32;
        // Update progress even when the song is not playing,
        // because seeking is possible while paused
        let elapsed = if *is_playing {
          self
            .instant_since_last_current_playback_poll
            .elapsed()
            .as_millis()
        } else {
          0u128
        } + u128::from(progress_ms);

        let duration_ms = match item {
          PlayableItem::Track(track) => track.duration.num_milliseconds() as u32,
          PlayableItem::Episode(episode) => episode.duration.num_milliseconds() as u32,
          PlayableItem::Unknown(_) => return,
        };

        if elapsed < u128::from(duration_ms) {
          self.song_progress_ms = elapsed;
        } else {
          self.song_progress_ms = duration_ms.into();
        }
      }
    }
    self.maybe_fetch_lyrics();

    // Lazily populate Made For You once the user's playlists arrive.
    // `made_for_you_populated` guard ensures we only run this once per session
    // (a re-run after the user adds new playlists requires a relaunch — rare).
    if !self.made_for_you_populated && self.playlists.is_some() {
      self.made_for_you_populated = true;
      self.populate_made_for_you_from_library();
    }
  }

  pub fn maybe_fetch_lyrics(&mut self) {
    if !self.lyrics_visible {
      return;
    }
    if self.lyrics_loading {
      return;
    }
    let context = match &self.current_playback_context {
      Some(c) => c.clone(),
      None => return,
    };
    let item = match context.item {
      Some(i) => i,
      None => return,
    };
    let (track_id, artist, track_name, album, duration_ms) = match item {
      rspotify::model::PlayableItem::Track(track) => {
        let id = match track.id.as_ref() {
          Some(i) => i.id().to_string(),
          None => return,
        };
        let artist = track
          .artists
          .first()
          .map(|a| a.name.clone())
          .unwrap_or_default();
        let album = if track.album.name.is_empty() {
          None
        } else {
          Some(track.album.name.clone())
        };
        (
          id,
          artist,
          track.name.clone(),
          album,
          track.duration.num_milliseconds() as u32,
        )
      }
      _ => return,
    };
    if self.lyrics_for_track_id.as_ref() == Some(&track_id) {
      return;
    }
    // Clear the stale cache (from the previous track) so the panel shows
    // the "fetching" placeholder during the network round-trip instead of
    // rendering yesterday's lyrics over today's track.
    self.lyrics = None;
    self.lyrics_for_track_id = None;
    self.lyrics_loading = true;
    self.dispatch(IoEvent::FetchLyrics {
      track_id,
      artist,
      track_name,
      album,
      duration_ms,
    });
  }

  /// Step back out of the home screen one level: out of an entered section
  /// first, then out to hover mode. Both `Esc` and the back key go through
  /// here, so they can't drift apart.
  ///
  /// Returns true when it consumed the key, so the back key doesn't *also* pop
  /// the navigation stack.
  pub fn back_out_of_home(&mut self) -> bool {
    if self.get_current_route().active_block != ActiveBlock::Home {
      return false;
    }
    if self.home_section_entered {
      self.home_section_entered = false;
    } else {
      self.set_current_route_state(Some(ActiveBlock::Empty), None);
    }
    true
  }

  pub fn toggle_home_mode(&mut self) {
    self.home_mode = match self.home_mode {
      HomeMode::Music => HomeMode::Podcast,
      HomeMode::Podcast => HomeMode::Music,
    };
    self.home_section_entered = false;
    self.home_selected_block = match self.home_mode {
      HomeMode::Music => HomeBlock::MadeForYou,
      HomeMode::Podcast => HomeBlock::YourShows,
    };

    // When entering podcast mode, dispatch the data fetches the home will
    // consume. The first-entry guard prevents a re-dispatch storm.
    if matches!(self.home_mode, HomeMode::Podcast) {
      if !self.podcast_home_fetched {
        self.podcast_home_fetched = true;
        if self.library.saved_shows.pages.is_empty() {
          self.dispatch(IoEvent::GetCurrentUserSavedShows(None));
        }
      }
      self.fetch_missing_podcast_episodes();
    }
  }

  /// Dispatch episode fetches for saved shows we don't have episodes for yet.
  ///
  /// Called on entering podcast mode *and* every tick while there, because the
  /// saved-show list is itself still in flight on the first entry — the old
  /// eager loop ran against an empty list, which left "Latest Episodes" and
  /// "Continue Listening" blank until the user toggled modes twice.
  pub fn fetch_missing_podcast_episodes(&mut self) {
    let country = self.get_user_country();
    let show_ids: Vec<String> = self
      .library
      .saved_shows
      .get_results(None)
      .map(|page| {
        page
          .items
          .iter()
          .map(|s| s.show.id.id().to_string())
          .collect()
      })
      .unwrap_or_default();
    for show_id in show_ids {
      if self.podcast_episodes_per_show.contains_key(&show_id)
        || self.podcast_episodes_requested.contains(&show_id)
      {
        continue;
      }
      self.podcast_episodes_requested.insert(show_id.clone());
      self.dispatch(IoEvent::FetchShowEpisodesForCache(show_id, country));
    }
  }

  pub fn seek_forwards(&mut self) {
    if let Some(CurrentPlaybackContext {
      item: Some(item), ..
    }) = &self.current_playback_context
    {
      let duration_ms = match item {
        PlayableItem::Track(track) => track.duration.num_milliseconds() as u32,
        PlayableItem::Episode(episode) => episode.duration.num_milliseconds() as u32,
        PlayableItem::Unknown(_) => return,
      };

      let old_progress = match self.seek_ms {
        Some(seek_ms) => seek_ms,
        None => self.song_progress_ms,
      };

      let new_progress = min(
        old_progress as u32 + self.user_config.behavior.seek_milliseconds,
        duration_ms,
      );

      self.seek_ms = Some(new_progress as u128);
    }
  }

  pub fn seek_backwards(&mut self) {
    let old_progress = match self.seek_ms {
      Some(seek_ms) => seek_ms,
      None => self.song_progress_ms,
    };
    let new_progress = if old_progress as u32 > self.user_config.behavior.seek_milliseconds {
      old_progress as u32 - self.user_config.behavior.seek_milliseconds
    } else {
      0u32
    };
    self.seek_ms = Some(new_progress as u128);
  }

  pub fn get_recommendations_for_seed(
    &mut self,
    seed_artists: Option<Vec<String>>,
    seed_tracks: Option<Vec<String>>,
    first_track: Option<FullTrack>,
  ) {
    let user_country = self.get_user_country();
    self.dispatch(IoEvent::GetRecommendationsForSeed(
      seed_artists,
      seed_tracks,
      Box::new(first_track),
      user_country,
    ));
  }

  pub fn get_recommendations_for_track_id(&mut self, id: String) {
    let user_country = self.get_user_country();
    self.dispatch(IoEvent::GetRecommendationsForTrackId(id, user_country));
  }

  pub fn increase_volume(&mut self) {
    if let Some(context) = self.current_playback_context.clone() {
      let current_volume = context.device.volume_percent.unwrap_or(0) as u8;
      let next_volume = min(
        current_volume + self.user_config.behavior.volume_increment,
        100,
      );

      if next_volume != current_volume {
        self.dispatch(IoEvent::ChangeVolume(next_volume));
      }
    }
  }

  pub fn decrease_volume(&mut self) {
    if let Some(context) = self.current_playback_context.clone() {
      let current_volume = context.device.volume_percent.unwrap_or(0) as i8;
      let next_volume = max(
        current_volume - self.user_config.behavior.volume_increment as i8,
        0,
      );

      if next_volume != current_volume {
        self.dispatch(IoEvent::ChangeVolume(next_volume as u8));
      }
    }
  }

  /// Report an API error as a toast rather than a screen the user has to
  /// dismiss. The message is also kept in `api_error` because the CLI reads it
  /// to decide whether a command failed.
  pub fn handle_error(&mut self, e: anyhow::Error) {
    let message = e.to_string();
    self.api_error = message.clone();
    self.toast = Some(Toast::error(message));
  }

  /// Pause or resume, and flip the local `is_playing` flag straight away.
  ///
  /// The real state is only re-polled every few seconds (`poll_current_playback`),
  /// so without the local flip a second press inside that window repeats the
  /// same command — and Spotify rejects pausing what is already paused with a
  /// 403, which used to surface as an error screen.
  pub fn toggle_playback(&mut self) {
    let was_playing = self
      .current_playback_context
      .as_ref()
      .map(|context| context.is_playing)
      .unwrap_or(false);

    if was_playing {
      self.dispatch(IoEvent::PausePlayback);
    } else {
      // When no offset or uris are passed, spotify will resume current playback
      self.dispatch(IoEvent::StartPlayback(None, None, None));
    }

    let shown_progress = self.song_progress_ms;
    if let Some(context) = &mut self.current_playback_context {
      if was_playing {
        // The progress bar is extrapolated from the last poll, so freeze it
        // where the user can see it rather than letting it snap back.
        context.progress = Some(chrono::TimeDelta::milliseconds(shown_progress as i64));
      }
      context.is_playing = !was_playing;
    }
    // Resume continues from `progress`, so restart the clock the bar counts from.
    self.instant_since_last_current_playback_poll = Instant::now();
  }

  pub fn previous_track(&mut self) {
    if self.song_progress_ms >= 3_000 {
      self.dispatch(IoEvent::Seek(0));
    } else {
      self.dispatch(IoEvent::PreviousTrack);
    }
  }

  // The navigation_stack actually only controls the large block to the right of `library` and
  // `playlists`
  pub fn push_navigation_stack(&mut self, next_route_id: RouteId, next_active_block: ActiveBlock) {
    if !self
      .navigation_stack
      .last()
      .map(|last_route| last_route.id == next_route_id)
      .unwrap_or(false)
    {
      self.navigation_stack.push(Route {
        id: next_route_id,
        active_block: next_active_block,
        hovered_block: next_active_block,
      });
    }
  }

  pub fn pop_navigation_stack(&mut self) -> Option<Route> {
    if self.navigation_stack.len() == 1 {
      None
    } else {
      self.navigation_stack.pop()
    }
  }

  pub fn get_current_route(&self) -> &Route {
    // if for some reason there is no route return the default
    self.navigation_stack.last().unwrap_or(&DEFAULT_ROUTE)
  }

  fn get_current_route_mut(&mut self) -> &mut Route {
    self.navigation_stack.last_mut().unwrap()
  }

  pub fn set_current_route_state(
    &mut self,
    active_block: Option<ActiveBlock>,
    hovered_block: Option<ActiveBlock>,
  ) {
    let mut current_route = self.get_current_route_mut();
    if let Some(active_block) = active_block {
      current_route.active_block = active_block;
    }
    if let Some(hovered_block) = hovered_block {
      current_route.hovered_block = hovered_block;
    }
  }

  pub fn copy_song_url(&mut self) {
    let clipboard = match &mut self.clipboard {
      Some(ctx) => ctx,
      None => return,
    };

    if let Some(CurrentPlaybackContext {
      item: Some(item), ..
    }) = &self.current_playback_context
    {
      match item {
        PlayableItem::Track(track) => {
          if let Err(e) = clipboard.set_text(format!(
            "https://open.spotify.com/track/{}",
            track.id.as_ref().map(|i| i.id().to_string()).unwrap_or_default()
          )) {
            self.handle_error(anyhow!("failed to set clipboard content: {}", e));
          }
        }
        PlayableItem::Episode(episode) => {
          if let Err(e) = clipboard.set_text(format!(
            "https://open.spotify.com/episode/{}",
            episode.id.id()
          )) {
            self.handle_error(anyhow!("failed to set clipboard content: {}", e));
          }
        }
        PlayableItem::Unknown(_) => {}
      }
    }
  }

  pub fn copy_album_url(&mut self) {
    let clipboard = match &mut self.clipboard {
      Some(ctx) => ctx,
      None => return,
    };

    if let Some(CurrentPlaybackContext {
      item: Some(item), ..
    }) = &self.current_playback_context
    {
      match item {
        PlayableItem::Track(track) => {
          if let Err(e) = clipboard.set_text(format!(
            "https://open.spotify.com/album/{}",
            track.album.id.as_ref().map(|i| i.id().to_string()).unwrap_or_default()
          )) {
            self.handle_error(anyhow!("failed to set clipboard content: {}", e));
          }
        }
        PlayableItem::Episode(episode) => {
          if let Err(e) = clipboard.set_text(format!(
            "https://open.spotify.com/show/{}",
            episode.show.id.id()
          )) {
            self.handle_error(anyhow!("failed to set clipboard content: {}", e));
          }
        }
        PlayableItem::Unknown(_) => {}
      }
    }
  }

  pub fn set_saved_tracks_to_table(&mut self, saved_track_page: &Page<SavedTrack>) {
    self.dispatch(IoEvent::SetTracksToTable(
      saved_track_page
        .items
        .clone()
        .into_iter()
        .map(|item| item.track)
        .collect::<Vec<FullTrack>>(),
    ));
  }

  pub fn set_saved_artists_to_table(&mut self, saved_artists_page: &CursorBasedPage<FullArtist>) {
    self.dispatch(IoEvent::SetArtistsToTable(
      saved_artists_page
        .items
        .clone()
        .into_iter()
        .collect::<Vec<FullArtist>>(),
    ))
  }

  pub fn get_current_user_saved_artists_next(&mut self) {
    match self
      .library
      .saved_artists
      .get_results(Some(self.library.saved_artists.index + 1))
      .cloned()
    {
      Some(saved_artists) => {
        self.set_saved_artists_to_table(&saved_artists);
        self.library.saved_artists.index += 1
      }
      None => {
        if let Some(saved_artists) = &self.library.saved_artists.clone().get_results(None) {
          if let Some(last_artist) = saved_artists.items.last() {
            self.dispatch(IoEvent::GetFollowedArtists(Some(last_artist.id.id().to_string())));
          }
        }
      }
    }
  }

  pub fn get_current_user_saved_artists_previous(&mut self) {
    if self.library.saved_artists.index > 0 {
      self.library.saved_artists.index -= 1;
    }

    if let Some(saved_artists) = &self.library.saved_artists.get_results(None).cloned() {
      self.set_saved_artists_to_table(saved_artists);
    }
  }

  pub fn get_current_user_saved_tracks_next(&mut self) {
    // Before fetching the next tracks, check if we have already fetched them
    match self
      .library
      .saved_tracks
      .get_results(Some(self.library.saved_tracks.index + 1))
      .cloned()
    {
      Some(saved_tracks) => {
        self.set_saved_tracks_to_table(&saved_tracks);
        self.library.saved_tracks.index += 1
      }
      None => {
        if let Some(saved_tracks) = &self.library.saved_tracks.get_results(None) {
          let offset = Some(saved_tracks.offset + saved_tracks.limit);
          self.dispatch(IoEvent::GetCurrentSavedTracks(offset));
        }
      }
    }
  }

  pub fn get_current_user_saved_tracks_previous(&mut self) {
    if self.library.saved_tracks.index > 0 {
      self.library.saved_tracks.index -= 1;
    }

    if let Some(saved_tracks) = &self.library.saved_tracks.get_results(None).cloned() {
      self.set_saved_tracks_to_table(saved_tracks);
    }
  }

  pub fn shuffle(&mut self) {
    if let Some(context) = &self.current_playback_context.clone() {
      self.dispatch(IoEvent::Shuffle(context.shuffle_state));
    };
  }

  pub fn get_current_user_saved_albums_next(&mut self) {
    match self
      .library
      .saved_albums
      .get_results(Some(self.library.saved_albums.index + 1))
      .cloned()
    {
      Some(_) => self.library.saved_albums.index += 1,
      None => {
        if let Some(saved_albums) = &self.library.saved_albums.get_results(None) {
          let offset = Some(saved_albums.offset + saved_albums.limit);
          self.dispatch(IoEvent::GetCurrentUserSavedAlbums(offset));
        }
      }
    }
  }

  pub fn get_current_user_saved_albums_previous(&mut self) {
    if self.library.saved_albums.index > 0 {
      self.library.saved_albums.index -= 1;
    }
  }

  pub fn get_new_releases_next(&mut self) {
    match self
      .library
      .new_releases
      .get_results(Some(self.library.new_releases.index + 1))
      .cloned()
    {
      Some(_) => self.library.new_releases.index += 1,
      None => {
        if let Some(page) = &self.library.new_releases.get_results(None) {
          let offset = Some(page.offset + page.limit);
          self.dispatch(IoEvent::GetNewReleases(offset));
        }
      }
    }
  }

  pub fn get_new_releases_previous(&mut self) {
    if self.library.new_releases.index > 0 {
      self.library.new_releases.index -= 1;
    }
  }

  pub fn current_user_saved_album_delete(&mut self, block: ActiveBlock) {
    match block {
      ActiveBlock::SearchResultBlock => {
        if let Some(albums) = &self.search_results.albums {
          if let Some(selected_index) = self.search_results.selected_album_index {
            let selected_album = &albums.items[selected_index];
            if let Some(album_id) = selected_album.id.as_ref() {
              self.dispatch(IoEvent::CurrentUserSavedAlbumDelete(album_id.id().to_string()));
            }
          }
        }
      }
      ActiveBlock::AlbumList => {
        if let Some(albums) = self.library.saved_albums.get_results(None) {
          if let Some(selected_album) = albums.items.get(self.album_list_index) {
            let album_id = selected_album.album.id.id().to_string();
            self.dispatch(IoEvent::CurrentUserSavedAlbumDelete(album_id));
          }
        }
      }
      ActiveBlock::ArtistBlock => {
        if let Some(artist) = &self.artist {
          if let Some(selected_album) = artist.albums.items.get(artist.selected_album_index) {
            if let Some(album_id) = selected_album.id.as_ref() {
              self.dispatch(IoEvent::CurrentUserSavedAlbumDelete(album_id.id().to_string()));
            }
          }
        }
      }
      _ => (),
    }
  }

  pub fn current_user_saved_album_add(&mut self, block: ActiveBlock) {
    match block {
      ActiveBlock::SearchResultBlock => {
        if let Some(albums) = &self.search_results.albums {
          if let Some(selected_index) = self.search_results.selected_album_index {
            let selected_album = &albums.items[selected_index];
            if let Some(album_id) = selected_album.id.as_ref() {
              self.dispatch(IoEvent::CurrentUserSavedAlbumAdd(album_id.id().to_string()));
            }
          }
        }
      }
      ActiveBlock::ArtistBlock => {
        if let Some(artist) = &self.artist {
          if let Some(selected_album) = artist.albums.items.get(artist.selected_album_index) {
            if let Some(album_id) = selected_album.id.as_ref() {
              self.dispatch(IoEvent::CurrentUserSavedAlbumAdd(album_id.id().to_string()));
            }
          }
        }
      }
      _ => (),
    }
  }

  pub fn get_current_user_saved_shows_next(&mut self) {
    match self
      .library
      .saved_shows
      .get_results(Some(self.library.saved_shows.index + 1))
      .cloned()
    {
      Some(_) => self.library.saved_shows.index += 1,
      None => {
        if let Some(saved_shows) = &self.library.saved_shows.get_results(None) {
          let offset = Some(saved_shows.offset + saved_shows.limit);
          self.dispatch(IoEvent::GetCurrentUserSavedShows(offset));
        }
      }
    }
  }

  pub fn get_current_user_saved_shows_previous(&mut self) {
    if self.library.saved_shows.index > 0 {
      self.library.saved_shows.index -= 1;
    }
  }

  pub fn get_episode_table_next(&mut self, show_id: String) {
    match self
      .library
      .show_episodes
      .get_results(Some(self.library.show_episodes.index + 1))
      .cloned()
    {
      Some(_) => self.library.show_episodes.index += 1,
      None => {
        if let Some(show_episodes) = &self.library.show_episodes.get_results(None) {
          let offset = Some(show_episodes.offset + show_episodes.limit);
          self.dispatch(IoEvent::GetCurrentShowEpisodes(show_id, offset));
        }
      }
    }
  }

  pub fn get_episode_table_previous(&mut self) {
    if self.library.show_episodes.index > 0 {
      self.library.show_episodes.index -= 1;
    }
  }

  pub fn user_unfollow_artists(&mut self, block: ActiveBlock) {
    match block {
      ActiveBlock::SearchResultBlock => {
        if let Some(artists) = &self.search_results.artists {
          if let Some(selected_index) = self.search_results.selected_artists_index {
            let selected_artist: &FullArtist = &artists.items[selected_index];
            let artist_id = selected_artist.id.id().to_string();
            self.dispatch(IoEvent::UserUnfollowArtists(vec![artist_id]));
          }
        }
      }
      ActiveBlock::AlbumList => {
        if let Some(artists) = self.library.saved_artists.get_results(None) {
          if let Some(selected_artist) = artists.items.get(self.artists_list_index) {
            let artist_id = selected_artist.id.id().to_string();
            self.dispatch(IoEvent::UserUnfollowArtists(vec![artist_id]));
          }
        }
      }
      ActiveBlock::ArtistBlock => {
        if let Some(artist) = &self.artist {
          let selected_artis = &artist.related_artists[artist.selected_related_artist_index];
          let artist_id = selected_artis.id.id().to_string();
          self.dispatch(IoEvent::UserUnfollowArtists(vec![artist_id]));
        }
      }
      _ => (),
    };
  }

  pub fn user_follow_artists(&mut self, block: ActiveBlock) {
    match block {
      ActiveBlock::SearchResultBlock => {
        if let Some(artists) = &self.search_results.artists {
          if let Some(selected_index) = self.search_results.selected_artists_index {
            let selected_artist: &FullArtist = &artists.items[selected_index];
            let artist_id = selected_artist.id.id().to_string();
            self.dispatch(IoEvent::UserFollowArtists(vec![artist_id]));
          }
        }
      }
      ActiveBlock::ArtistBlock => {
        if let Some(artist) = &self.artist {
          let selected_artis = &artist.related_artists[artist.selected_related_artist_index];
          let artist_id = selected_artis.id.id().to_string();
          self.dispatch(IoEvent::UserFollowArtists(vec![artist_id]));
        }
      }
      _ => (),
    }
  }

  pub fn user_follow_playlist(&mut self) {
    if let SearchResult {
      playlists: Some(ref playlists),
      selected_playlists_index: Some(selected_index),
      ..
    } = self.search_results
    {
      let selected_playlist: &SimplifiedPlaylist = &playlists.items[selected_index];
      let selected_id = selected_playlist.id.id().to_string();
      let selected_public = selected_playlist.public;
      let selected_owner_id = selected_playlist.owner.id.id().to_string();
      self.dispatch(IoEvent::UserFollowPlaylist(
        selected_owner_id,
        selected_id,
        selected_public,
      ));
    }
  }

  pub fn user_unfollow_playlist(&mut self) {
    if let (Some(playlists), Some(selected_index), Some(user)) =
      (&self.playlists, self.selected_playlist_index, &self.user)
    {
      let selected_playlist = &playlists.items[selected_index];
      let selected_id = selected_playlist.id.id().to_string();
      let user_id = user.id.id().to_string();
      self.dispatch(IoEvent::UserUnfollowPlaylist(user_id, selected_id))
    }
  }

  pub fn user_unfollow_playlist_search_result(&mut self) {
    if let (Some(playlists), Some(selected_index), Some(user)) = (
      &self.search_results.playlists,
      self.search_results.selected_playlists_index,
      &self.user,
    ) {
      let selected_playlist = &playlists.items[selected_index];
      let selected_id = selected_playlist.id.id().to_string();
      let user_id = user.id.id().to_string();
      self.dispatch(IoEvent::UserUnfollowPlaylist(user_id, selected_id))
    }
  }

  pub fn user_follow_show(&mut self, block: ActiveBlock) {
    match block {
      ActiveBlock::SearchResultBlock => {
        if let Some(shows) = &self.search_results.shows {
          if let Some(selected_index) = self.search_results.selected_shows_index {
            if let Some(show_id) = shows.items.get(selected_index).map(|item| item.id.id().to_string()) {
              self.dispatch(IoEvent::CurrentUserSavedShowAdd(show_id));
            }
          }
        }
      }
      ActiveBlock::EpisodeTable => match self.episode_table_context {
        EpisodeTableContext::Full => {
          if let Some(selected_episode) = self.selected_show_full.clone() {
            let show_id = selected_episode.show.id.id().to_string();
            self.dispatch(IoEvent::CurrentUserSavedShowAdd(show_id));
          }
        }
        EpisodeTableContext::Simplified => {
          if let Some(selected_episode) = self.selected_show_simplified.clone() {
            let show_id = selected_episode.show.id.id().to_string();
            self.dispatch(IoEvent::CurrentUserSavedShowAdd(show_id));
          }
        }
      },
      _ => (),
    }
  }

  pub fn user_unfollow_show(&mut self, block: ActiveBlock) {
    match block {
      ActiveBlock::Podcasts => {
        if let Some(shows) = self.library.saved_shows.get_results(None) {
          if let Some(selected_show) = shows.items.get(self.shows_list_index) {
            let show_id = selected_show.show.id.id().to_string();
            self.dispatch(IoEvent::CurrentUserSavedShowDelete(show_id));
          }
        }
      }
      ActiveBlock::SearchResultBlock => {
        if let Some(shows) = &self.search_results.shows {
          if let Some(selected_index) = self.search_results.selected_shows_index {
            let show_id = shows.items[selected_index].id.id().to_string();
            self.dispatch(IoEvent::CurrentUserSavedShowDelete(show_id));
          }
        }
      }
      ActiveBlock::EpisodeTable => match self.episode_table_context {
        EpisodeTableContext::Full => {
          if let Some(selected_episode) = self.selected_show_full.clone() {
            let show_id = selected_episode.show.id.id().to_string();
            self.dispatch(IoEvent::CurrentUserSavedShowDelete(show_id));
          }
        }
        EpisodeTableContext::Simplified => {
          if let Some(selected_episode) = self.selected_show_simplified.clone() {
            let show_id = selected_episode.show.id.id().to_string();
            self.dispatch(IoEvent::CurrentUserSavedShowDelete(show_id));
          }
        }
      },
      _ => (),
    }
  }

  pub fn get_made_for_you(&mut self) {
    // The PUBLIC Spotify Web API does NOT expose a "Made For You" endpoint.
    // Auto-generated playlists (Daily Mix 1-6, Discover Weekly, Release Radar)
    // are personalised per user and not in the public search catalog — so the
    // previous /search approach returned random public playlists with similar
    // names, not the user's own mixes.
    //
    // The realistic public-API path: those playlists DO appear in the user's
    // own playlist library (`current_user_playlists`) with `owner.id == "spotify"`
    // when Spotify has auto-saved them. We filter `self.playlists` for those.
    // If the user has un-followed them, they simply won't appear and the panel
    // shows an honest empty message.
    if !self.library.made_for_you_playlists.pages.is_empty() {
      return;
    }
    self.populate_made_for_you_from_library();
  }

  fn populate_made_for_you_from_library(&mut self) {
    let spotify_owned: Vec<SimplifiedPlaylist> = match &self.playlists {
      Some(page) => page
        .items
        .iter()
        .filter(|p| p.owner.id.id() == "spotify")
        .cloned()
        .collect(),
      None => Vec::new(),
    };

    // Always push exactly one Page (even if empty) so the render can
    // distinguish "still loading" (pages empty) from "loaded but no Spotify-
    // curated playlists in your library" (page exists, items empty).
    let n = spotify_owned.len() as u32;
    self.library.made_for_you_playlists.pages.clear();
    self.library.made_for_you_playlists.pages.push(Page {
      items: spotify_owned.clone(),
      href: String::new(),
      limit: n.max(1),
      next: None,
      offset: 0,
      previous: None,
      total: n,
    });

    // Dispatch preview fetches for each. Silent-fail per playlist.
    let user_country = self.get_user_country();
    for playlist in &spotify_owned {
      let playlist_id = playlist.id.id().to_string();
      self.dispatch(IoEvent::FetchMadeForYouPreview(
        playlist_id,
        user_country,
      ));
    }
  }

  /// Open the centered playlist-picker modal to add `uri` (a track or
  /// episode) to one of the user's playlists.
  /// Playlists the current user can add items to: ones they own, or
  /// collaborative ones. Adding to a followed (non-owned) playlist would 403,
  /// so the picker only offers these. Order matches `self.playlists` so the
  /// UI and the picker handler agree on indexing.
  pub fn modifiable_playlists(&self) -> Vec<&SimplifiedPlaylist> {
    let user_id = self.user.as_ref().map(|u| u.id.id().to_string());
    match (&self.playlists, user_id) {
      (Some(page), Some(uid)) => page
        .items
        .iter()
        .filter(|p| p.collaborative || p.owner.id.id() == uid)
        .collect(),
      // User profile not loaded yet — don't hide everything.
      (Some(page), None) => page.items.iter().collect(),
      _ => Vec::new(),
    }
  }

  pub fn open_playlist_picker(&mut self, uri: String) {
    if !self.modifiable_playlists().is_empty() {
      self.playlist_picker_uri = Some(uri);
      self.playlist_picker_index = 0;
      self.push_navigation_stack(RouteId::Dialog, ActiveBlock::PlaylistPicker);
    }
  }

  /// Open the analysis view, and fetch analysis if a real track is playing.
  ///
  /// The view opens whatever is playing — including nothing at all — so the key
  /// is never silently dead; the screen itself explains why data is missing
  /// (podcast episode, video/local file, or Spotify's removal of the
  /// `/audio-analysis` endpoint for third-party apps).
  pub fn get_audio_analysis(&mut self) {
    if self.get_current_route().id == RouteId::Analysis {
      return;
    }
    let track_uri = match &self.current_playback_context {
      Some(CurrentPlaybackContext {
        item: Some(PlayableItem::Track(track)),
        ..
      }) => track.id.as_ref().map(|id| id.uri()),
      _ => None,
    };
    match track_uri {
      Some(uri) => self.dispatch(IoEvent::GetAudioAnalysis(uri)),
      // Nothing analysable is playing, so drop any stale data rather than
      // showing the previous track's numbers under the current one.
      None => self.audio_analysis = None,
    }
    self.push_navigation_stack(RouteId::Analysis, ActiveBlock::Analysis);
  }

  pub fn repeat(&mut self) {
    if let Some(context) = &self.current_playback_context.clone() {
      self.dispatch(IoEvent::Repeat(context.repeat_state));
    }
  }

  pub fn get_artist(&mut self, artist_id: String, input_artist_name: String) {
    let user_country = self.get_user_country();
    self.dispatch(IoEvent::GetArtist(
      artist_id,
      input_artist_name,
      user_country,
    ));
  }

  pub fn get_user_country(&self) -> Option<Country> {
    self
      .user
      .as_ref()
      .and_then(|user| user.country)
  }

  pub fn calculate_help_menu_offset(&mut self) {
    let old_offset = self.help_menu_offset;

    if self.help_menu_max_lines < self.help_docs_size {
      self.help_menu_offset = self.help_menu_page * self.help_menu_max_lines;
    }
    if self.help_menu_offset > self.help_docs_size {
      self.help_menu_offset = old_offset;
      self.help_menu_page = self.help_menu_page.saturating_sub(1);
    }
  }
}

#[cfg(test)]
mod app_tests {
  use super::App;

  #[test]
  fn help_menu_offset_does_not_underflow_on_page_zero() {
    let mut app = App::default();
    app.help_menu_max_lines = 10;
    app.help_docs_size = 5;
    app.help_menu_offset = 7; // stale from a pre-resize state
    app.help_menu_page = 0;
    app.calculate_help_menu_offset(); // must not panic
    assert_eq!(app.help_menu_page, 0);
  }
}

#[cfg(test)]
mod analysis_key_tests {
  use super::{App, RouteId};

  #[test]
  fn the_analysis_key_opens_the_view_even_with_nothing_playing() {
    // Otherwise `v` is silently dead whenever playback is idle, which reads as
    // a broken key rather than an unavailable API.
    let mut app = App::default();
    assert!(app.current_playback_context.is_none());

    app.get_audio_analysis();
    assert_eq!(app.get_current_route().id, RouteId::Analysis);
  }

  #[test]
  fn the_analysis_key_does_not_stack_routes_when_already_open() {
    let mut app = App::default();
    app.get_audio_analysis();
    app.get_audio_analysis();
    assert_eq!(app.get_current_route().id, RouteId::Analysis);
    app.pop_navigation_stack();
    assert_eq!(app.get_current_route().id, RouteId::Home);
  }
}

#[cfg(test)]
mod playback_toggle_tests {
  use super::*;
  use rspotify::model::{
    context::{Actions, CurrentPlaybackContext},
    device::Device,
    CurrentlyPlayingType, DeviceType, RepeatState,
  };

  fn app_with_playback(is_playing: bool) -> (App, std::sync::mpsc::Receiver<IoEvent>) {
    let (tx, rx) = std::sync::mpsc::channel::<IoEvent>();
    let mut app = App::new(tx, UserConfig::new(), SystemTime::now());
    app.current_playback_context = Some(CurrentPlaybackContext {
      device: Device {
        id: Some("device".to_owned()),
        is_active: true,
        is_private_session: false,
        is_restricted: false,
        name: "Test".to_owned(),
        _type: DeviceType::Computer,
        volume_percent: Some(50),
      },
      repeat_state: RepeatState::Off,
      shuffle_state: false,
      context: None,
      timestamp: chrono::DateTime::from_timestamp(0, 0).unwrap(),
      progress: Some(chrono::TimeDelta::seconds(30)),
      is_playing,
      item: None,
      currently_playing_type: CurrentlyPlayingType::Track,
      actions: Actions { disallows: vec![] },
    });
    (app, rx)
  }

  #[test]
  fn a_second_toggle_sends_the_opposite_command() {
    // Playback state is only re-polled every few seconds. Without a local
    // flip, the second press repeats the first command and Spotify answers
    // 403 "Restriction violated".
    let (mut app, rx) = app_with_playback(true);

    app.toggle_playback();
    assert!(
      matches!(rx.try_recv(), Ok(IoEvent::PausePlayback)),
      "first press should pause"
    );

    app.toggle_playback();
    assert!(
      matches!(rx.try_recv(), Ok(IoEvent::StartPlayback(None, None, None))),
      "second press should resume, not pause again"
    );
  }

  #[test]
  fn toggling_updates_the_local_playing_flag_immediately() {
    let (mut app, _rx) = app_with_playback(true);
    app.toggle_playback();
    assert_eq!(
      app.current_playback_context.as_ref().map(|c| c.is_playing),
      Some(false)
    );
    app.toggle_playback();
    assert_eq!(
      app.current_playback_context.as_ref().map(|c| c.is_playing),
      Some(true)
    );
  }

  #[test]
  fn pausing_freezes_the_progress_where_it_is_shown() {
    let (mut app, _rx) = app_with_playback(true);
    // The bar has been extrapolated past the last poll's 30s.
    app.song_progress_ms = 34_000;
    app.toggle_playback();
    let progress = app
      .current_playback_context
      .as_ref()
      .and_then(|c| c.progress)
      .map(|p| p.num_milliseconds());
    assert_eq!(progress, Some(34_000), "progress must not jump backwards");
  }

  #[test]
  fn toggling_without_a_playback_context_still_asks_to_play() {
    let (tx, rx) = std::sync::mpsc::channel::<IoEvent>();
    let mut app = App::new(tx, UserConfig::new(), SystemTime::now());
    app.toggle_playback();
    assert!(matches!(
      rx.try_recv(),
      Ok(IoEvent::StartPlayback(None, None, None))
    ));
  }
}

#[cfg(test)]
mod toast_tests {
  use super::*;

  fn aged_toast(message: &str, age: Duration) -> Toast {
    let mut toast = Toast::error(message.to_owned());
    toast.created_at = Instant::now().checked_sub(age).expect("clock too young");
    toast
  }

  #[test]
  fn an_error_becomes_a_toast_without_changing_the_route() {
    let mut app = App::default();
    let before = app.get_current_route().id.clone();

    app.handle_error(anyhow!("http error: status code 403 Forbidden"));

    let toast = app.toast.as_ref().expect("error should raise a toast");
    assert!(toast.message.contains("403"));
    // The whole point: the UI is not taken over.
    assert_eq!(app.get_current_route().id, before);
    // The CLI still reads the message from here.
    assert!(app.api_error.contains("403"));
  }

  #[test]
  fn a_403_keeps_the_troubleshooting_hint_from_the_old_error_screen() {
    let toast = Toast::error("http error: status code 403 Forbidden".to_owned());
    assert!(toast.hint.unwrap().contains("Premium"));

    let toast = Toast::error("http error: status code 404 Not Found".to_owned());
    assert!(toast.hint.unwrap().contains("asleep"));

    // Anything else is shown as-is, with no invented advice.
    let toast = Toast::error("json parse error: expected value".to_owned());
    assert!(toast.hint.is_none());
  }

  #[test]
  fn a_toast_clears_itself_on_a_later_tick() {
    let mut app = App::default();
    app.toast = Some(aged_toast("boom", Duration::from_secs(1)));
    app.update_on_tick();
    assert!(app.toast.is_some(), "a fresh toast must stay put");

    app.toast = Some(aged_toast("boom", Toast::LIFETIME + Duration::from_secs(1)));
    app.update_on_tick();
    assert!(app.toast.is_none(), "an expired toast must clear itself");
  }

  #[test]
  fn a_new_error_replaces_the_one_on_screen() {
    let mut app = App::default();
    app.handle_error(anyhow!("first"));
    app.handle_error(anyhow!("second"));
    assert_eq!(app.toast.as_ref().map(|t| t.message.as_str()), Some("second"));
  }
}

#[cfg(test)]
mod podcast_cache_tests {
  use super::*;
  use rspotify::model::{Page, Show, ShowId, SimplifiedShow};

  #[allow(deprecated)]
  fn saved_show(id: &str) -> Show {
    Show {
      added_at: String::new(),
      show: SimplifiedShow {
        available_markets: vec![],
        copyrights: vec![],
        description: String::new(),
        explicit: false,
        external_urls: Default::default(),
        href: String::new(),
        id: ShowId::from_id(format!("{:0>22}", id)).unwrap(),
        images: vec![],
        is_externally_hosted: Some(false),
        languages: vec![],
        media_type: "audio".to_owned(),
        name: format!("Show {}", id),
        publisher: String::new(),
      },
    }
  }

  fn app_with_shows(count: usize) -> (App, std::sync::mpsc::Receiver<IoEvent>) {
    let (tx, rx) = std::sync::mpsc::channel::<IoEvent>();
    let mut app = App::new(tx, UserConfig::new(), SystemTime::now());
    app.home_mode = HomeMode::Podcast;
    let items: Vec<Show> = (0..count).map(|i| saved_show(&i.to_string())).collect();
    let total = items.len() as u32;
    app.library.saved_shows.pages.push(Page {
      items,
      href: String::new(),
      limit: total.max(1),
      next: None,
      offset: 0,
      previous: None,
      total,
    });
    (app, rx)
  }

  fn episode_fetches(rx: &std::sync::mpsc::Receiver<IoEvent>) -> usize {
    let mut count = 0;
    while let Ok(event) = rx.try_recv() {
      if matches!(event, IoEvent::FetchShowEpisodesForCache(_, _)) {
        count += 1;
      }
    }
    count
  }

  #[test]
  fn each_saved_show_is_fetched_once_however_many_ticks_pass() {
    // The top-up runs every tick while in podcast mode; without the requested
    // set that would be several requests per show per second.
    let (mut app, rx) = app_with_shows(3);

    app.update_on_tick();
    assert_eq!(episode_fetches(&rx), 3);

    for _ in 0..5 {
      app.update_on_tick();
    }
    assert_eq!(episode_fetches(&rx), 0, "shows must not be re-requested");
  }

  #[test]
  fn shows_that_arrive_later_are_picked_up_without_toggling_modes() {
    // The saved-show list is still in flight when podcast mode opens, which is
    // why the eager one-shot fetch used to leave the episode sections empty.
    let (mut app, rx) = app_with_shows(0);
    app.update_on_tick();
    assert_eq!(episode_fetches(&rx), 0);

    app.library.saved_shows.pages.clear();
    let (with_shows, _) = app_with_shows(2);
    app.library.saved_shows = with_shows.library.saved_shows;

    app.update_on_tick();
    assert_eq!(episode_fetches(&rx), 2);
  }

  #[test]
  fn music_mode_does_not_fetch_podcast_episodes() {
    let (mut app, rx) = app_with_shows(2);
    app.home_mode = HomeMode::Music;
    app.update_on_tick();
    assert_eq!(episode_fetches(&rx), 0);
  }
}

#[cfg(test)]
mod loading_hint_tests {
  use super::*;

  fn app() -> (App, std::sync::mpsc::Receiver<IoEvent>) {
    let (tx, rx) = std::sync::mpsc::channel::<IoEvent>();
    (App::new(tx, UserConfig::new(), SystemTime::now()), rx)
  }

  #[test]
  fn the_hint_clears_once_the_queue_drains() {
    let (mut app, _rx) = app();
    assert!(!app.is_loading, "nothing dispatched yet");

    app.dispatch(IoEvent::GetUser);
    assert!(app.is_loading);

    app.io_finished();
    assert!(!app.is_loading, "the hint used to stay on for the whole session");
  }

  #[test]
  fn queued_requests_keep_the_hint_on_until_the_last_one_reports() {
    let (mut app, _rx) = app();
    app.dispatch(IoEvent::GetUser);
    app.dispatch(IoEvent::GetPlaylists);
    app.dispatch(IoEvent::GetDevices(false));

    app.io_finished();
    assert!(app.is_loading, "two requests are still in flight");
    app.io_finished();
    assert!(app.is_loading, "one request is still in flight");
    app.io_finished();
    assert!(!app.is_loading);
  }

  #[test]
  fn extra_reports_cannot_underflow_the_counter() {
    // The CLI drives `handle_network_event` directly, without dispatching.
    let (mut app, _rx) = app();
    app.io_finished();
    app.io_finished();
    assert!(!app.is_loading);

    app.dispatch(IoEvent::GetUser);
    assert!(app.is_loading, "the counter must not have gone negative");
    app.io_finished();
    assert!(!app.is_loading);
  }

  #[test]
  fn a_failed_send_does_not_leave_the_hint_stuck_on() {
    let (mut app, rx) = app();
    drop(rx); // the network thread is gone
    app.dispatch(IoEvent::GetUser);
    assert!(!app.is_loading);
  }
}
