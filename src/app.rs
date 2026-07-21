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
  time::{Instant, SystemTime},
};
use ratatui::layout::Rect;

use arboard::Clipboard;

pub const LIBRARY_OPTIONS: [&str; 6] = [
  "Made For You",
  "Recently Played",
  "Liked Songs",
  "Albums",
  "Artists",
  "Podcasts",
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
}

#[derive(PartialEq, Debug)]
pub enum SearchResultBlock {
  AlbumSearch,
  SongSearch,
  ArtistSearch,
  PlaylistSearch,
  ShowSearch,
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

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum HomeBlock {
  MadeForYou,
  RecommendedStations,
  JumpBackIn,
  YourShows,
  ContinueListening,
  EpisodesForYou,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum DialogContext {
  PlaylistWindow,
  PlaylistSearch,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ActiveBlock {
  Analysis,
  PlayBar,
  AlbumTracks,
  AlbumList,
  ArtistBlock,
  Empty,
  Error,
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
}

#[derive(Clone, PartialEq, Debug)]
pub enum RouteId {
  Analysis,
  AlbumTracks,
  AlbumList,
  Artist,
  BasicView,
  Error,
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

#[derive(Clone, PartialEq, Debug)]
pub enum RecommendationsContext {
  Artist,
  Song,
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
  pub selected_album_index: Option<usize>,
  pub selected_artists_index: Option<usize>,
  pub selected_playlists_index: Option<usize>,
  pub selected_tracks_index: Option<usize>,
  pub selected_shows_index: Option<usize>,
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
  pub home_scroll: u16,
  pub home_selected_block: HomeBlock,
  pub home_section_entered: bool,
  pub home_jump_back_index: usize,
  pub home_made_for_you_index: usize,
  pub home_recommended_index: usize,
  pub home_mode: HomeMode,
  pub podcast_episodes_per_show: HashMap<String, Vec<SimplifiedEpisode>>,
  pub home_your_shows_index: usize,
  pub home_continue_listening_index: usize,
  pub home_episodes_for_you_index: usize,
  pub podcast_home_fetched: bool,
  pub made_for_you_populated: bool,
  pub top_artists: Vec<FullArtist>,
  pub user_config: UserConfig,
  pub artists: Vec<FullArtist>,
  pub artist: Option<Artist>,
  pub album_table_context: AlbumTableContext,
  pub saved_album_tracks_index: usize,
  pub api_error: String,
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
  pub is_loading: bool,
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
      home_scroll: 0,
      home_selected_block: HomeBlock::MadeForYou,
      home_section_entered: false,
      home_jump_back_index: 0,
      home_made_for_you_index: 0,
      home_recommended_index: 0,
      home_mode: HomeMode::Music,
      podcast_episodes_per_show: HashMap::new(),
      home_your_shows_index: 0,
      home_continue_listening_index: 0,
      home_episodes_for_you_index: 0,
      podcast_home_fetched: false,
      made_for_you_populated: false,
      top_artists: Vec::new(),
      library: Library {
        saved_tracks: ScrollableResultPages::new(),
        made_for_you_playlists: ScrollableResultPages::new(),
        saved_albums: ScrollableResultPages::new(),
        saved_shows: ScrollableResultPages::new(),
        saved_artists: ScrollableResultPages::new(),
        show_episodes: ScrollableResultPages::new(),
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
        selected_album_index: None,
        selected_artists_index: None,
        selected_playlists_index: None,
        selected_tracks_index: None,
        selected_shows_index: None,
        tracks: None,
      },
      song_progress_ms: 0,
      seek_ms: None,
      selected_device_index: None,
      selected_playlist_index: None,
      active_playlist_index: None,
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
  pub fn dispatch(&mut self, action: IoEvent) {
    // `is_loading` will be set to false again after the async action has finished in network.rs
    self.is_loading = true;
    if let Some(io_tx) = &self.io_tx {
      if let Err(e) = io_tx.send(action) {
        self.is_loading = false;
        println!("Error from dispatch {}", e);
        // TODO: handle error
      };
    }
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
      // Always (re-)check per-show cache and dispatch fetches for shows we
      // don't yet have episodes for. This handles new shows added mid-session.
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
        if !self.podcast_episodes_per_show.contains_key(&show_id) {
          self.dispatch(IoEvent::FetchShowEpisodesForCache(show_id, country));
        }
      }
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

  pub fn handle_error(&mut self, e: anyhow::Error) {
    self.push_navigation_stack(RouteId::Error, ActiveBlock::Error);
    self.api_error = e.to_string();
  }

  pub fn toggle_playback(&mut self) {
    if let Some(CurrentPlaybackContext {
      is_playing: true, ..
    }) = &self.current_playback_context
    {
      self.dispatch(IoEvent::PausePlayback);
    } else {
      // When no offset or uris are passed, spotify will resume current playback
      self.dispatch(IoEvent::StartPlayback(None, None, None));
    }
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

  fn made_for_you_search_and_add(&mut self, search_string: &str) {
    let user_country = self.get_user_country();
    self.dispatch(IoEvent::MadeForYouSearchAndAdd(
      search_string.to_string(),
      user_country,
    ));
  }

  pub fn get_audio_analysis(&mut self) {
    if let Some(CurrentPlaybackContext {
      item: Some(item), ..
    }) = &self.current_playback_context
    {
      match item {
        PlayableItem::Track(track) => {
          if self.get_current_route().id != RouteId::Analysis {
            let uri = track.id.as_ref().map(|i| i.uri()).unwrap_or_default();
            self.dispatch(IoEvent::GetAudioAnalysis(uri));
            self.push_navigation_stack(RouteId::Analysis, ActiveBlock::Analysis);
          }
        }
        PlayableItem::Unknown(_) => {}
        PlayableItem::Episode(_episode) => {
          // No audio analysis available for podcast uris, so just default to the empty analysis
          // view to avoid a 400 error code
          self.push_navigation_stack(RouteId::Analysis, ActiveBlock::Analysis);
        }
      }
    }
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
      self.help_menu_page -= 1;
    }
  }
}
