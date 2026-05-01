use crate::app::{
  ActiveBlock, AlbumTableContext, App, Artist, ArtistBlock, EpisodeTableContext, RouteId,
  ScrollableResultPages, SelectedAlbum, SelectedFullAlbum, SelectedFullShow, SelectedShow,
  TrackTableContext,
};
use crate::config::{ClientConfig, ConfigPaths};
use anyhow::anyhow;
use rspotify::{
  scopes, AuthCodeSpotify, Config, Credentials, OAuth, Token,
};
use rspotify::prelude::OAuthClient;
use rspotify::model::{
  Country, FullArtist, FullTrack, Page, PlaylistItem, Recommendations, RepeatState,
  SimplifiedAlbum, SimplifiedShow,
};
use serde_json::{map::Map, Value};
use std::{
  collections::HashSet,
  path::PathBuf,
  sync::Arc,
  time::{Duration, Instant, SystemTime},
};
use tokio::sync::Mutex;
use tokio::try_join;

#[derive(Debug)]
pub enum IoEvent {
  GetCurrentPlayback,
  RefreshAuthentication,
  GetPlaylists,
  GetDevices,
  GetSearchResults(String, Option<Country>),
  SetTracksToTable(Vec<FullTrack>),
  GetMadeForYouPlaylistTracks(String, u32),
  GetPlaylistTracks(String, u32),
  GetCurrentSavedTracks(Option<u32>),
  StartPlayback(Option<String>, Option<Vec<String>>, Option<usize>),
  UpdateSearchLimits(u32, u32),
  Seek(u32),
  NextTrack,
  PreviousTrack,
  Shuffle(bool),
  Repeat(RepeatState),
  PausePlayback,
  ChangeVolume(u8),
  GetArtist(String, String, Option<Country>),
  GetAlbumTracks(Box<SimplifiedAlbum>),
  GetRecommendationsForSeed(
    Option<Vec<String>>,
    Option<Vec<String>>,
    Box<Option<FullTrack>>,
    Option<Country>,
  ),
  GetCurrentUserSavedAlbums(Option<u32>),
  CurrentUserSavedAlbumsContains(Vec<String>),
  CurrentUserSavedAlbumDelete(String),
  CurrentUserSavedAlbumAdd(String),
  UserUnfollowArtists(Vec<String>),
  UserFollowArtists(Vec<String>),
  UserFollowPlaylist(String, String, Option<bool>),
  UserUnfollowPlaylist(String, String),
  MadeForYouSearchAndAdd(String, Option<Country>),
  GetAudioAnalysis(String),
  GetUser,
  ToggleSaveTrack(String),
  GetRecommendationsForTrackId(String, Option<Country>),
  GetRecentlyPlayed,
  GetFollowedArtists(Option<String>),
  SetArtistsToTable(Vec<FullArtist>),
  UserArtistFollowCheck(Vec<String>),
  GetAlbum(String),
  TransferPlaybackToDevice(String),
  GetAlbumForTrack(String),
  CurrentUserSavedTracksContains(Vec<String>),
  GetCurrentUserSavedShows(Option<u32>),
  CurrentUserSavedShowsContains(Vec<String>),
  CurrentUserSavedShowDelete(String),
  CurrentUserSavedShowAdd(String),
  GetShowEpisodes(Box<SimplifiedShow>),
  GetShow(String),
  GetCurrentShowEpisodes(String, Option<u32>),
  AddItemToQueue(String),
}

/// Construct an `AuthCodeSpotify` client from the user's credentials and
/// cache configuration.  The caller owns the returned client and is responsible
/// for running the OAuth flow (see `src/main.rs`).
pub fn get_spotify(client_config: &ClientConfig, paths: &ConfigPaths) -> AuthCodeSpotify {
  let creds = Credentials::new(&client_config.client_id, &client_config.client_secret);

  let scopes = scopes!(
    "playlist-read-collaborative",
    "playlist-read-private",
    "playlist-modify-private",
    "playlist-modify-public",
    "user-follow-read",
    "user-follow-modify",
    "user-library-modify",
    "user-library-read",
    "user-modify-playback-state",
    "user-read-currently-playing",
    "user-read-playback-state",
    "user-read-playback-position",
    "user-read-private",
    "user-read-recently-played"
  );

  let oauth = OAuth {
    redirect_uri: client_config.get_redirect_uri(),
    scopes,
    ..Default::default()
  };

  let config = Config {
    token_cached: true,
    token_refreshing: true,
    cache_path: paths.token_cache_path.clone(),
    ..Default::default()
  };

  AuthCodeSpotify::with_config(creds, oauth, config)
}

#[derive(Clone)]
pub struct Network {
  pub spotify: AuthCodeSpotify,
  large_search_limit: u32,
  small_search_limit: u32,
  pub client_config: ClientConfig,
  pub app: Arc<Mutex<App>>,
}

impl Network {
  pub fn new(
    spotify: AuthCodeSpotify,
    client_config: ClientConfig,
    app: Arc<Mutex<App>>,
  ) -> Self {
    Network {
      spotify,
      large_search_limit: 20,
      small_search_limit: 4,
      client_config,
      app,
    }
  }

  #[allow(clippy::cognitive_complexity)]
  pub async fn handle_network_event(&mut self, io_event: IoEvent) {
    unimplemented!()
  }

  async fn handle_error(&mut self, e: anyhow::Error) {
    let mut app = self.app.lock().await;
    app.handle_error(e);
  }

  async fn get_user(&mut self) {
    match self.spotify.current_user().await {
      Ok(user) => {
        let mut app = self.app.lock().await;
        app.user = Some(user);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn get_devices(&mut self) {
    unimplemented!()
  }

  async fn get_current_playback(&mut self) {
    unimplemented!()
  }

  async fn current_user_saved_tracks_contains(&mut self, ids: Vec<String>) {
    unimplemented!()
  }

  async fn get_playlist_tracks(&mut self, playlist_id: String, playlist_offset: u32) {
    unimplemented!()
  }

  async fn set_playlist_tracks_to_table(&mut self, playlist_track_page: &Page<PlaylistItem>) {
    unimplemented!()
  }

  async fn set_tracks_to_table(&mut self, tracks: Vec<FullTrack>) {
    unimplemented!()
  }

  async fn set_artists_to_table(&mut self, artists: Vec<FullArtist>) {
    unimplemented!()
  }

  async fn get_made_for_you_playlist_tracks(
    &mut self,
    playlist_id: String,
    made_for_you_offset: u32,
  ) {
    unimplemented!()
  }

  async fn get_current_user_saved_shows(&mut self, offset: Option<u32>) {
    unimplemented!()
  }

  async fn current_user_saved_shows_contains(&mut self, show_ids: Vec<String>) {
    unimplemented!()
  }

  async fn get_show_episodes(&mut self, show: Box<SimplifiedShow>) {
    unimplemented!()
  }

  async fn get_show(&mut self, show_id: String) {
    unimplemented!()
  }

  async fn get_current_show_episodes(&mut self, show_id: String, offset: Option<u32>) {
    unimplemented!()
  }

  async fn get_search_results(&mut self, search_term: String, country: Option<Country>) {
    unimplemented!()
  }

  async fn get_current_user_saved_tracks(&mut self, offset: Option<u32>) {
    unimplemented!()
  }

  async fn start_playback(
    &mut self,
    context_uri: Option<String>,
    uris: Option<Vec<String>>,
    offset: Option<usize>,
  ) {
    unimplemented!()
  }

  async fn seek(&mut self, position_ms: u32) {
    unimplemented!()
  }

  async fn next_track(&mut self) {
    unimplemented!()
  }

  async fn previous_track(&mut self) {
    unimplemented!()
  }

  async fn shuffle(&mut self, shuffle_state: bool) {
    unimplemented!()
  }

  async fn repeat(&mut self, repeat_state: RepeatState) {
    unimplemented!()
  }

  async fn pause_playback(&mut self) {
    unimplemented!()
  }

  async fn change_volume(&mut self, volume_percent: u8) {
    unimplemented!()
  }

  async fn get_artist(
    &mut self,
    artist_id: String,
    input_artist_name: String,
    country: Option<Country>,
  ) {
    unimplemented!()
  }

  async fn get_album_tracks(&mut self, album: Box<SimplifiedAlbum>) {
    unimplemented!()
  }

  async fn get_recommendations_for_seed(
    &mut self,
    seed_artists: Option<Vec<String>>,
    seed_tracks: Option<Vec<String>>,
    first_track: Box<Option<FullTrack>>,
    country: Option<Country>,
  ) {
    unimplemented!()
  }

  async fn extract_recommended_tracks(
    &mut self,
    recommendations: &Recommendations,
  ) -> Option<Vec<FullTrack>> {
    unimplemented!()
  }

  async fn get_recommendations_for_track_id(&mut self, id: String, country: Option<Country>) {
    unimplemented!()
  }

  async fn toggle_save_track(&mut self, track_id: String) {
    unimplemented!()
  }

  async fn get_followed_artists(&mut self, after: Option<String>) {
    unimplemented!()
  }

  async fn user_artist_check_follow(&mut self, artist_ids: Vec<String>) {
    unimplemented!()
  }

  async fn get_current_user_saved_albums(&mut self, offset: Option<u32>) {
    unimplemented!()
  }

  async fn current_user_saved_albums_contains(&mut self, album_ids: Vec<String>) {
    unimplemented!()
  }

  pub async fn current_user_saved_album_delete(&mut self, album_id: String) {
    unimplemented!()
  }

  async fn current_user_saved_album_add(&mut self, album_id: String) {
    unimplemented!()
  }

  async fn current_user_saved_shows_delete(&mut self, show_id: String) {
    unimplemented!()
  }

  async fn current_user_saved_shows_add(&mut self, show_id: String) {
    unimplemented!()
  }

  async fn user_unfollow_artists(&mut self, artist_ids: Vec<String>) {
    unimplemented!()
  }

  async fn user_follow_artists(&mut self, artist_ids: Vec<String>) {
    unimplemented!()
  }

  async fn user_follow_playlist(
    &mut self,
    playlist_owner_id: String,
    playlist_id: String,
    is_public: Option<bool>,
  ) {
    unimplemented!()
  }

  async fn user_unfollow_playlist(&mut self, user_id: String, playlist_id: String) {
    unimplemented!()
  }

  async fn made_for_you_search_and_add(&mut self, search_string: String, country: Option<Country>) {
    unimplemented!()
  }

  async fn get_audio_analysis(&mut self, uri: String) {
    unimplemented!()
  }

  async fn get_current_user_playlists(&mut self) {
    unimplemented!()
  }

  async fn get_recently_played(&mut self) {
    unimplemented!()
  }

  async fn get_album(&mut self, album_id: String) {
    unimplemented!()
  }

  async fn get_album_for_track(&mut self, track_id: String) {
    unimplemented!()
  }

  async fn transfert_playback_to_device(&mut self, device_id: String) {
    unimplemented!()
  }

  async fn refresh_authentication(&mut self) {
    unimplemented!()
  }

  async fn add_item_to_queue(&mut self, item: String) {
    unimplemented!()
  }
}
