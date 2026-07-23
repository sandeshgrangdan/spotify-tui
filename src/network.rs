use crate::app::{
  ActiveBlock, AlbumTableContext, App, Artist, ArtistBlock, EpisodeTableContext, RouteId,
  SelectedAlbum, SelectedFullAlbum, SelectedFullShow, SelectedShow, TrackTableContext,
};
use crate::config::{ClientConfig, ConfigPaths};
use anyhow::anyhow;
use rspotify::{scopes, AuthCodeSpotify, Config, Credentials, OAuth};
use rspotify::prelude::{BaseClient, Id, OAuthClient};
use rspotify::model::{
  AdditionalType, AlbumId, ArtistId, Country, EpisodeId, FullArtist, FullTrack, Market, Offset,
  Page, PlayableId, PlayableItem, PlaylistId, PlaylistItem, Recommendations, RepeatState,
  SearchMultipleResult, SearchType, ShowId, SimplifiedAlbum, SimplifiedShow, TimeRange, TrackId,
};
use std::{sync::Arc, time::{Duration, Instant, SystemTime}};
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
  FetchMadeForYouPreview(String, Option<Country>),
  GetTopArtists,
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
  FetchShowEpisodesForCache(String, Option<Country>),
  GetCurrentShowEpisodes(String, Option<u32>),
  AddItemToQueue(String),
  GetQueue,
  SkipToQueueIndex(usize),
  FetchLyrics {
    track_id: String,
    artist: String,
    track_name: String,
    album: Option<String>,
    duration_ms: u32,
  },
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
        match io_event {
      IoEvent::GetUser => self.get_user().await,
      IoEvent::GetDevices => self.get_devices().await,
      IoEvent::GetCurrentPlayback => self.get_current_playback().await,
      IoEvent::RefreshAuthentication => self.refresh_authentication().await,
      IoEvent::GetPlaylists => self.get_current_user_playlists().await,
      IoEvent::GetSearchResults(search_term, country) => {
        self.get_search_results(search_term, country).await
      }
      IoEvent::SetTracksToTable(tracks) => self.set_tracks_to_table(tracks).await,
      IoEvent::SetArtistsToTable(artists) => self.set_artists_to_table(artists).await,
      IoEvent::GetMadeForYouPlaylistTracks(playlist_id, offset) => {
        self.get_made_for_you_playlist_tracks(playlist_id, offset).await
      }
      IoEvent::GetPlaylistTracks(playlist_id, offset) => {
        self.get_playlist_tracks(playlist_id, offset).await
      }
      IoEvent::GetCurrentSavedTracks(offset) => {
        self.get_current_user_saved_tracks(offset).await
      }
      IoEvent::StartPlayback(context_uri, uris, offset) => {
        self.start_playback(context_uri, uris, offset).await
      }
      IoEvent::UpdateSearchLimits(large_limit, small_limit) => {
        let mut app = self.app.lock().await;
        app.large_search_limit = large_limit;
        app.small_search_limit = small_limit;
        self.large_search_limit = large_limit;
        self.small_search_limit = small_limit;
      }
      IoEvent::Seek(position_ms) => self.seek(position_ms).await,
      IoEvent::NextTrack => self.next_track().await,
      IoEvent::PreviousTrack => self.previous_track().await,
      IoEvent::Shuffle(state) => self.shuffle(state).await,
      IoEvent::Repeat(state) => self.repeat(state).await,
      IoEvent::PausePlayback => self.pause_playback().await,
      IoEvent::ChangeVolume(volume) => self.change_volume(volume).await,
      IoEvent::GetArtist(artist_id, artist_name, country) => {
        self.get_artist(artist_id, artist_name, country).await
      }
      IoEvent::GetAlbumTracks(album) => self.get_album_tracks(album).await,
      IoEvent::GetRecommendationsForSeed(seed_artists, seed_tracks, first_track, country) => {
        self
          .get_recommendations_for_seed(seed_artists, seed_tracks, first_track, country)
          .await
      }
      IoEvent::GetCurrentUserSavedAlbums(offset) => {
        self.get_current_user_saved_albums(offset).await
      }
      IoEvent::CurrentUserSavedAlbumsContains(album_ids) => {
        self.current_user_saved_albums_contains(album_ids).await
      }
      IoEvent::CurrentUserSavedAlbumDelete(album_id) => {
        self.current_user_saved_album_delete(album_id).await
      }
      IoEvent::CurrentUserSavedAlbumAdd(album_id) => {
        self.current_user_saved_album_add(album_id).await
      }
      IoEvent::UserUnfollowArtists(artist_ids) => {
        self.user_unfollow_artists(artist_ids).await
      }
      IoEvent::UserFollowArtists(artist_ids) => self.user_follow_artists(artist_ids).await,
      IoEvent::UserFollowPlaylist(owner_id, playlist_id, is_public) => {
        self
          .user_follow_playlist(owner_id, playlist_id, is_public)
          .await
      }
      IoEvent::UserUnfollowPlaylist(user_id, playlist_id) => {
        self.user_unfollow_playlist(user_id, playlist_id).await
      }
      IoEvent::FetchMadeForYouPreview(playlist_id, country) => {
        self.fetch_made_for_you_preview(playlist_id, country).await
      }
      IoEvent::GetTopArtists => self.get_top_artists().await,
      IoEvent::GetAudioAnalysis(uri) => self.get_audio_analysis(uri).await,
      IoEvent::ToggleSaveTrack(track_id) => self.toggle_save_track(track_id).await,
      IoEvent::GetRecommendationsForTrackId(id, country) => {
        self.get_recommendations_for_track_id(id, country).await
      }
      IoEvent::GetRecentlyPlayed => self.get_recently_played().await,
      IoEvent::GetFollowedArtists(after) => self.get_followed_artists(after).await,
      IoEvent::UserArtistFollowCheck(artist_ids) => {
        self.user_artist_check_follow(artist_ids).await
      }
      IoEvent::GetAlbum(album_id) => self.get_album(album_id).await,
      IoEvent::TransferPlaybackToDevice(device_id) => {
        self.transfert_playback_to_device(device_id).await
      }
      IoEvent::GetAlbumForTrack(track_id) => self.get_album_for_track(track_id).await,
      IoEvent::CurrentUserSavedTracksContains(track_ids) => {
        self.current_user_saved_tracks_contains(track_ids).await
      }
      IoEvent::GetCurrentUserSavedShows(offset) => {
        self.get_current_user_saved_shows(offset).await
      }
      IoEvent::CurrentUserSavedShowsContains(show_ids) => {
        self.current_user_saved_shows_contains(show_ids).await
      }
      IoEvent::CurrentUserSavedShowDelete(show_id) => {
        self.current_user_saved_shows_delete(show_id).await
      }
      IoEvent::CurrentUserSavedShowAdd(show_id) => {
        self.current_user_saved_shows_add(show_id).await
      }
      IoEvent::GetShowEpisodes(show) => self.get_show_episodes(show).await,
      IoEvent::GetShow(show_id) => self.get_show(show_id).await,
      IoEvent::FetchShowEpisodesForCache(show_id, country) => {
        self.fetch_show_episodes_for_cache(show_id, country).await
      }
      IoEvent::GetCurrentShowEpisodes(show_id, offset) => {
        self.get_current_show_episodes(show_id, offset).await
      }
      IoEvent::AddItemToQueue(item) => self.add_item_to_queue(item).await,
      IoEvent::GetQueue => self.get_queue().await,
      IoEvent::SkipToQueueIndex(index) => self.skip_to_queue_index(index).await,
      IoEvent::FetchLyrics {
        track_id,
        artist,
        track_name,
        album,
        duration_ms,
      } => self.fetch_lyrics(track_id, artist, track_name, album, duration_ms).await,
    }
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
    match self.spotify.device().await {
      Ok(devices) => {
        let mut app = self.app.lock().await;
        app.push_navigation_stack(RouteId::SelectedDevice, ActiveBlock::SelectDevice);
        if !devices.is_empty() {
          app.devices = Some(rspotify::model::DevicePayload { devices });
          app.selected_device_index = Some(0);
        }
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn get_current_playback(&mut self) {
    let additional_types = [AdditionalType::Episode];
    match self
      .spotify
      .current_playback(None, Some(&additional_types))
      .await
    {
      Ok(playback) => {
        let mut app = self.app.lock().await;
        app.current_playback_context = playback;
        app.instant_since_last_current_playback_poll = Instant::now();
        app.is_fetching_current_playback = false;
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  #[allow(deprecated)]
  async fn current_user_saved_tracks_contains(&mut self, ids: Vec<String>) {
    let track_ids: Vec<TrackId<'static>> = ids
      .iter()
      .filter_map(|id| TrackId::from_id_or_uri(id).ok().map(|t| t.into_static()))
      .collect();
    match self
      .spotify
      .current_user_saved_tracks_contains(track_ids)
      .await
    {
      Ok(results) => {
        let mut app = self.app.lock().await;
        for (id, is_saved) in ids.iter().zip(results.iter()) {
          if *is_saved {
            app.liked_song_ids_set.insert(id.clone());
          } else {
            app.liked_song_ids_set.remove(id);
          }
        }
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn get_playlist_tracks(&mut self, playlist_id: String, playlist_offset: u32) {
    match PlaylistId::from_id_or_uri(&playlist_id) {
      Ok(pid) => {
        match self
          .spotify
          .playlist_items_manual(pid.as_ref(), None, None, Some(100), Some(playlist_offset))
          .await
        {
          Ok(page) => {
            self.set_playlist_tracks_to_table(&page).await;
            let mut app = self.app.lock().await;
            app.playlist_tracks = Some(page);
            app.push_navigation_stack(RouteId::TrackTable, ActiveBlock::TrackTable);
          }
          Err(e) => {
            self.handle_error(anyhow!(e)).await;
          }
        }
      }
      Err(e) => {
        self.handle_error(anyhow!("Invalid playlist ID: {:?}", e)).await;
      }
    }
  }

  #[allow(deprecated)]
  async fn set_playlist_tracks_to_table(&mut self, playlist_track_page: &Page<PlaylistItem>) {
    let tracks: Vec<FullTrack> = playlist_track_page
      .items
      .iter()
      .filter_map(|item| {
        item.item.as_ref().and_then(|playable| match playable {
          PlayableItem::Track(track) => Some(track.clone()),
          PlayableItem::Episode(_) => None,
          PlayableItem::Unknown(_) => None,
        })
      })
      .collect();
    self.set_tracks_to_table(tracks).await;
  }

  async fn set_tracks_to_table(&mut self, tracks: Vec<FullTrack>) {
    let mut app = self.app.lock().await;
    app.track_table.tracks = tracks;
    app.track_table.selected_index = 0;
  }

  async fn set_artists_to_table(&mut self, artists: Vec<FullArtist>) {
    let mut app = self.app.lock().await;
    app.artists = artists;
    app.artists_list_index = 0;
  }

  async fn get_made_for_you_playlist_tracks(
    &mut self,
    playlist_id: String,
    made_for_you_offset: u32,
  ) {
    match PlaylistId::from_id_or_uri(&playlist_id) {
      Ok(pid) => {
        match self
          .spotify
          .playlist_items_manual(pid.as_ref(), None, None, Some(100), Some(made_for_you_offset))
          .await
        {
          Ok(page) => {
            self.set_playlist_tracks_to_table(&page).await;
            let mut app = self.app.lock().await;
            app.made_for_you_tracks = Some(page);
            app.push_navigation_stack(RouteId::TrackTable, ActiveBlock::TrackTable);
          }
          Err(e) => {
            self.handle_error(anyhow!(e)).await;
          }
        }
      }
      Err(e) => {
        self.handle_error(anyhow!("Invalid playlist ID: {:?}", e)).await;
      }
    }
  }

  async fn get_current_user_saved_shows(&mut self, offset: Option<u32>) {
    let offset = offset.unwrap_or(0);
    match self
      .spotify
      .get_saved_show_manual(Some(50), Some(offset))
      .await
    {
      Ok(page) => {
        let mut app = self.app.lock().await;
        app.library.saved_shows.add_pages(page);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  #[allow(deprecated)]
  async fn current_user_saved_shows_contains(&mut self, show_ids: Vec<String>) {
    let ids: Vec<ShowId<'static>> = show_ids
      .iter()
      .filter_map(|id| ShowId::from_id_or_uri(id).ok().map(|s| s.into_static()))
      .collect();
    match self.spotify.check_users_saved_shows(ids).await {
      Ok(results) => {
        let mut app = self.app.lock().await;
        for (id, is_saved) in show_ids.iter().zip(results.iter()) {
          if *is_saved {
            app.saved_show_ids_set.insert(id.clone());
          } else {
            app.saved_show_ids_set.remove(id);
          }
        }
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn get_show_episodes(&mut self, show: Box<SimplifiedShow>) {
    match self
      .spotify
      .get_shows_episodes_manual(show.id.as_ref(), None, Some(50), Some(0))
      .await
    {
      Ok(page) => {
        let mut app = self.app.lock().await;
        app.library.show_episodes.add_pages(page);
        app.selected_show_simplified = Some(SelectedShow { show: *show });
        app.episode_table_context = EpisodeTableContext::Simplified;
        app.push_navigation_stack(RouteId::PodcastEpisodes, ActiveBlock::EpisodeTable);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn get_show(&mut self, show_id: String) {
    match ShowId::from_id_or_uri(&show_id) {
      Ok(sid) => {
        match self.spotify.get_a_show(sid.as_ref(), None).await {
          Ok(full_show) => {
            let mut app = self.app.lock().await;
            app.selected_show_full = Some(SelectedFullShow { show: full_show });
            app.episode_table_context = EpisodeTableContext::Full;
            app.push_navigation_stack(RouteId::PodcastEpisodes, ActiveBlock::EpisodeTable);
          }
          Err(e) => {
            self.handle_error(anyhow!(e)).await;
          }
        }
      }
      Err(e) => {
        self.handle_error(anyhow!("Invalid show ID: {:?}", e)).await;
      }
    }
  }

  async fn fetch_show_episodes_for_cache(
    &mut self,
    show_id: String,
    country: Option<Country>,
  ) {
    let market = country.map(Market::Country);
    let sid = match ShowId::from_id_or_uri(&show_id) {
      Ok(s) => s,
      Err(_) => return,
    };
    if let Ok(page) = self
      .spotify
      .get_shows_episodes_manual(sid.as_ref(), market, Some(10), Some(0))
      .await
    {
      let mut app = self.app.lock().await;
      app
        .podcast_episodes_per_show
        .insert(show_id, page.items);
    }
    // Silent fail — missing entry just means that show won't appear in
    // Continue listening or Episodes for you. No error route push.
  }

  async fn get_current_show_episodes(&mut self, show_id: String, offset: Option<u32>) {
    let offset = offset.unwrap_or(0);
    match ShowId::from_id_or_uri(&show_id) {
      Ok(sid) => {
        match self
          .spotify
          .get_shows_episodes_manual(sid.as_ref(), None, Some(50), Some(offset))
          .await
        {
          Ok(page) => {
            let mut app = self.app.lock().await;
            app.library.show_episodes.add_pages(page);
          }
          Err(e) => {
            self.handle_error(anyhow!(e)).await;
          }
        }
      }
      Err(e) => {
        self.handle_error(anyhow!("Invalid show ID: {:?}", e)).await;
      }
    }
  }

  async fn get_search_results(&mut self, search_term: String, country: Option<Country>) {
    let market = country.map(Market::Country);
    let limit = self.large_search_limit;

    let search_types = vec![
      SearchType::Track,
      SearchType::Artist,
      SearchType::Album,
      SearchType::Playlist,
      SearchType::Show,
    ];

    match self
      .spotify
      .search_multiple(&search_term, search_types, market, None, Some(limit), None)
      .await
    {
      Ok(SearchMultipleResult {
        tracks,
        artists,
        albums,
        playlists,
        shows,
        ..
      }) => {
        let mut app = self.app.lock().await;
        app.search_results.tracks = tracks;
        app.search_results.artists = artists;
        app.search_results.albums = albums;
        app.search_results.playlists = playlists;
        app.search_results.shows = shows;
        app.push_navigation_stack(RouteId::Search, ActiveBlock::SearchResultBlock);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn get_current_user_saved_tracks(&mut self, offset: Option<u32>) {
    let offset = offset.unwrap_or(0);
    match self
      .spotify
      .current_user_saved_tracks_manual(None, Some(50), Some(offset))
      .await
    {
      Ok(page) => {
        let tracks: Vec<FullTrack> = page
          .items
          .iter()
          .map(|saved| saved.track.clone())
          .collect();
        let mut app = self.app.lock().await;
        app.library.saved_tracks.add_pages(page);
        app.track_table.tracks = tracks;
        app.track_table.context = Some(TrackTableContext::SavedTracks);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn start_playback(
    &mut self,
    context_uri: Option<String>,
    uris: Option<Vec<String>>,
    offset: Option<usize>,
  ) {
    // Resolve target device. The currently-active playback context's device is
    // the "truth" — that's what the TUI shows in the bottom bar — so prefer it
    // over any stored client_config.device_id (which may be stale).
    let context_device: Option<(Option<String>, String)> = {
      let app = self.app.lock().await;
      app
        .current_playback_context
        .as_ref()
        .map(|ctx| (ctx.device.id.clone(), ctx.device.name.clone()))
    };
        let active_device_id: Option<String> = context_device
      .as_ref()
      .and_then(|(id, _)| id.clone())
      .or_else(|| self.client_config.device_id.clone());
    // Persist whatever we resolved so subsequent calls have it.
    if let Some(ref id) = active_device_id {
      if self.client_config.device_id.as_deref() != Some(id.as_str()) {
        self.client_config.device_id = Some(id.clone());
        let _ = self.client_config.set_device_id(id.clone());
      }
    }
    let device_arg = active_device_id.as_deref();
    
    let result = if let Some(context) = context_uri {
      // Play a context (album, playlist, artist, show)
      // Build offset from uris if provided, otherwise use Uri-based offset or None
      let ctx_offset = offset.and_then(|idx| {
        // If we have a list of URIs, use the URI at that index as offset
        if let Some(ref uri_list) = uris {
          uri_list.get(idx).map(|u| Offset::Uri(u.clone()))
        } else {
          // No URI list — we can't easily construct an index-based Offset
          // without chrono. For now, skip the offset.
          // TODO(phase-2): add chrono as direct dep to use Offset::Position(Duration::milliseconds(idx as i64))
          None
        }
      });

      if let Ok(context_id) = rspotify::model::AlbumId::from_id_or_uri(&context) {
        self
          .spotify
          .start_context_playback(
            rspotify::model::PlayContextId::Album(context_id),
            device_arg,
            ctx_offset,
            None,
          )
          .await
      } else if let Ok(context_id) = PlaylistId::from_id_or_uri(&context) {
        self
          .spotify
          .start_context_playback(
            rspotify::model::PlayContextId::Playlist(context_id),
            device_arg,
            ctx_offset,
            None,
          )
          .await
      } else if let Ok(context_id) = ArtistId::from_id_or_uri(&context) {
        self
          .spotify
          .start_context_playback(
            rspotify::model::PlayContextId::Artist(context_id),
            device_arg,
            ctx_offset,
            None,
          )
          .await
      } else if let Ok(context_id) = ShowId::from_id_or_uri(&context) {
        self
          .spotify
          .start_context_playback(
            rspotify::model::PlayContextId::Show(context_id),
            device_arg,
            ctx_offset,
            None,
          )
          .await
      } else {
        // Try treating it as a generic URI with context — resume playback
        self.spotify.resume_playback(device_arg, None).await
      }
    } else if let Some(ref uri_list) = uris {
      // Play specific URIs (tracks/episodes) — offset is an index into the list
      let uri_offset = offset.and_then(|idx| {
        uri_list.get(idx).map(|u| Offset::Uri(u.clone()))
      });
      let playable_ids: Vec<PlayableId<'static>> = uri_list
        .iter()
        .filter_map(|uri| {
          if let Ok(id) = TrackId::from_id_or_uri(uri) {
            Some(PlayableId::Track(id.into_static()))
          } else if let Ok(id) = EpisodeId::from_id_or_uri(uri) {
            Some(PlayableId::Episode(id.into_static()))
          } else {
            None
          }
        })
        .collect();
      self
        .spotify
        .start_uris_playback(playable_ids, device_arg, uri_offset, None)
        .await
    } else {
      // Resume playback with no context/uris
      self
        .spotify
        .resume_playback(device_arg, None)
        .await
    };

        // If Spotify rejected with 404 and we have a target device, try transferring
    // playback to it first then retry the play. Spotify often returns 404 when
    // a device is technically present in the device list but hasn't been
    // "warmed up" with a recent transfer; the transfer call wakes it up.
    let final_result = if let (Err(ref e), Some(id)) = (&result, device_arg) {
      let err_str = e.to_string();
      if err_str.contains("404") {
                match self.spotify.transfer_playback(id, Some(true)).await {
          Ok(()) => {
            // Give Spotify a moment to register the transfer, then retry.
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            // Retry whichever flavor of playback we attempted.
            let retry = if let Some(uri_list) = uris.as_ref() {
              let uri_offset = offset.and_then(|idx| {
                uri_list.get(idx).map(|u| Offset::Uri(u.clone()))
              });
              let playable_ids: Vec<PlayableId<'static>> = uri_list
                .iter()
                .filter_map(|uri| {
                  if let Ok(tid) = TrackId::from_id_or_uri(uri) {
                    Some(PlayableId::Track(tid.into_static()))
                  } else if let Ok(eid) = EpisodeId::from_id_or_uri(uri) {
                    Some(PlayableId::Episode(eid.into_static()))
                  } else {
                    None
                  }
                })
                .collect();
              self
                .spotify
                .start_uris_playback(playable_ids, Some(id), uri_offset, None)
                .await
            } else {
              self.spotify.resume_playback(Some(id), None).await
            };
                        retry
          }
          Err(te) => {
                        result
          }
        }
      } else {
        result
      }
    } else {
      result
    };

    if let Err(e) = final_result {
      self.handle_error(anyhow!(e)).await;
    } else {
      self.get_current_playback().await;
    }
  }

  async fn seek(&mut self, position_ms: u32) {
    let pos = chrono::Duration::milliseconds(position_ms as i64);
    if let Err(e) = self
      .spotify
      .seek_track(pos, self.client_config.device_id.as_deref())
      .await
    {
      self.handle_error(anyhow!(e)).await;
    } else {
      let mut app = self.app.lock().await;
      app.seek_ms = None;
      app.song_progress_ms = position_ms as u128;
    }
  }

  async fn next_track(&mut self) {
    if let Err(e) = self.spotify.next_track(self.client_config.device_id.as_deref()).await {
      self.handle_error(anyhow!(e)).await;
    }
  }

  async fn previous_track(&mut self) {
    if let Err(e) = self.spotify.previous_track(self.client_config.device_id.as_deref()).await {
      self.handle_error(anyhow!(e)).await;
    }
  }

  async fn shuffle(&mut self, shuffle_state: bool) {
    if let Err(e) = self.spotify.shuffle(shuffle_state, self.client_config.device_id.as_deref()).await {
      self.handle_error(anyhow!(e)).await;
    }
  }

  async fn repeat(&mut self, repeat_state: RepeatState) {
    if let Err(e) = self.spotify.repeat(repeat_state, self.client_config.device_id.as_deref()).await {
      self.handle_error(anyhow!(e)).await;
    }
  }

  async fn pause_playback(&mut self) {
    if let Err(e) = self.spotify.pause_playback(self.client_config.device_id.as_deref()).await {
      self.handle_error(anyhow!(e)).await;
    }
  }

  async fn change_volume(&mut self, volume_percent: u8) {
    if let Err(e) = self.spotify.volume(volume_percent, self.client_config.device_id.as_deref()).await {
      self.handle_error(anyhow!(e)).await;
    }
  }

  #[allow(deprecated)]
  async fn get_artist(
    &mut self,
    artist_id: String,
    input_artist_name: String,
    country: Option<Country>,
  ) {
    match ArtistId::from_id_or_uri(&artist_id) {
      Ok(aid) => {
        let market = country.map(Market::Country);
        let top_tracks_fut = self.spotify.artist_top_tracks(aid.as_ref(), market);
        let albums_fut =
          self
            .spotify
            .artist_albums_manual(aid.as_ref(), [], market, Some(50), None);
        let related_artists_fut = self.spotify.artist_related_artists(aid.as_ref());

        let (top_tracks_result, albums_result, related_artists_result) =
          try_join!(top_tracks_fut, albums_fut, related_artists_fut)
            .map(|(a, b, c)| (Ok(a), Ok(b), Ok(c)))
            .unwrap_or_else(|e| (Err(e), Err(rspotify::ClientError::InvalidToken), Err(rspotify::ClientError::InvalidToken)));

        match (top_tracks_result, albums_result, related_artists_result) {
          (Ok(top_tracks), Ok(albums), Ok(related_artists)) => {
            let mut app = self.app.lock().await;
            app.artist = Some(Artist {
              artist_name: input_artist_name,
              albums,
              related_artists,
              top_tracks,
              selected_album_index: 0,
              selected_related_artist_index: 0,
              selected_top_track_index: 0,
              artist_hovered_block: ArtistBlock::TopTracks,
              artist_selected_block: ArtistBlock::Empty,
            });
            app.push_navigation_stack(RouteId::Artist, ActiveBlock::ArtistBlock);
          }
          _ => {
            self
              .handle_error(anyhow!("Failed to fetch artist data for {}", artist_id))
              .await;
          }
        }
      }
      Err(e) => {
        self.handle_error(anyhow!("Invalid artist ID: {:?}", e)).await;
      }
    }
  }

  async fn get_album_tracks(&mut self, album: Box<SimplifiedAlbum>) {
    if let Some(album_id) = &album.id {
      match self
        .spotify
        .album_track_manual(album_id.as_ref(), None, Some(50), Some(0))
        .await
      {
        Ok(tracks) => {
          let mut app = self.app.lock().await;
          app.selected_album_simplified = Some(SelectedAlbum {
            album: *album,
            tracks,
            selected_index: 0,
          });
          app.album_table_context = AlbumTableContext::Simplified;
          app.push_navigation_stack(RouteId::AlbumTracks, ActiveBlock::AlbumTracks);
        }
        Err(e) => {
          self.handle_error(anyhow!(e)).await;
        }
      }
    }
  }

  async fn get_recommendations_for_seed(
    &mut self,
    seed_artists: Option<Vec<String>>,
    seed_tracks: Option<Vec<String>>,
    _first_track: Box<Option<FullTrack>>,
    country: Option<Country>,
  ) {
    let market = country.map(Market::Country);

    let artist_ids: Option<Vec<ArtistId<'static>>> = seed_artists.as_ref().map(|ids| {
      ids
        .iter()
        .filter_map(|id| ArtistId::from_id_or_uri(id).ok().map(|a| a.into_static()))
        .collect()
    });
    let track_ids: Option<Vec<TrackId<'static>>> = seed_tracks.as_ref().map(|ids| {
      ids
        .iter()
        .filter_map(|id| TrackId::from_id_or_uri(id).ok().map(|t| t.into_static()))
        .collect()
    });

    let seed_string = if let Some(ref artists) = seed_artists {
      artists.join(", ")
    } else if let Some(ref tracks) = seed_tracks {
      tracks.join(", ")
    } else {
      String::new()
    };

    match self
      .spotify
      .recommendations(
        [],
        artist_ids.map(|ids| ids.into_iter()),
        None::<Vec<&str>>,
        track_ids.map(|ids| ids.into_iter()),
        market,
        Some(20),
      )
      .await
    {
      Ok(recommendations) => {
        if let Some(recommended_tracks) = self.extract_recommended_tracks(&recommendations).await {
          let mut app = self.app.lock().await;
          app.recommended_tracks = recommended_tracks;
          app.recommendations_seed = seed_string;
          app.recommendations_context =
            Some(crate::app::RecommendationsContext::Song);
          app.track_table.context = Some(TrackTableContext::RecommendedTracks);
          app.track_table.tracks = app.recommended_tracks.clone();
          app.push_navigation_stack(RouteId::Recommendations, ActiveBlock::TrackTable);
        }
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn extract_recommended_tracks(
    &mut self,
    recommendations: &Recommendations,
  ) -> Option<Vec<FullTrack>> {
    let track_ids: Vec<TrackId<'static>> = recommendations
      .tracks
      .iter()
      .filter_map(|t| t.id.as_ref().map(|id| id.clone_static()))
      .collect();

    if track_ids.is_empty() {
      return Some(vec![]);
    }

    let mut full_tracks = Vec::new();
    for track_id in track_ids {
      if let Ok(track) = self.spotify.track(track_id.as_ref(), None).await {
        full_tracks.push(track);
      }
    }
    Some(full_tracks)
  }

  async fn get_recommendations_for_track_id(&mut self, id: String, country: Option<Country>) {
    match TrackId::from_id_or_uri(&id) {
      Ok(track_id) => {
        let market = country.map(Market::Country);
        let track_id_static = track_id.into_static();
        match self
          .spotify
          .recommendations(
            [],
            None::<Vec<ArtistId<'_>>>,
            None::<Vec<&str>>,
            Some(vec![track_id_static.as_ref()]),
            market,
            Some(20),
          )
          .await
        {
          Ok(recommendations) => {
            if let Some(recommended_tracks) =
              self.extract_recommended_tracks(&recommendations).await
            {
              let mut app = self.app.lock().await;
              app.recommended_tracks = recommended_tracks;
              app.recommendations_seed = id.clone();
              app.recommendations_context =
                Some(crate::app::RecommendationsContext::Song);
              app.track_table.context = Some(TrackTableContext::RecommendedTracks);
              app.track_table.tracks = app.recommended_tracks.clone();
              app.push_navigation_stack(RouteId::Recommendations, ActiveBlock::TrackTable);
            }
          }
          Err(e) => {
            self.handle_error(anyhow!(e)).await;
          }
        }
      }
      Err(e) => {
        self.handle_error(anyhow!("Invalid track ID: {:?}", e)).await;
      }
    }
  }

  #[allow(deprecated)]
  async fn toggle_save_track(&mut self, track_id: String) {
    match TrackId::from_id_or_uri(&track_id) {
      Ok(tid) => {
        let is_liked = {
          let app = self.app.lock().await;
          app.liked_song_ids_set.contains(&track_id)
        };

        let tid_static = tid.into_static();
        let result = if is_liked {
          self
            .spotify
            .current_user_saved_tracks_delete([tid_static.as_ref()])
            .await
        } else {
          self
            .spotify
            .current_user_saved_tracks_add([tid_static.as_ref()])
            .await
        };

        match result {
          Ok(()) => {
            let mut app = self.app.lock().await;
            if is_liked {
              app.liked_song_ids_set.remove(&track_id);
            } else {
              app.liked_song_ids_set.insert(track_id);
            }
          }
          Err(e) => {
            self.handle_error(anyhow!(e)).await;
          }
        }
      }
      Err(e) => {
        self.handle_error(anyhow!("Invalid track ID: {:?}", e)).await;
      }
    }
  }

  async fn get_followed_artists(&mut self, after: Option<String>) {
    match self
      .spotify
      .current_user_followed_artists(after.as_deref(), Some(50))
      .await
    {
      Ok(page) => {
        let mut app = self.app.lock().await;
        app.library.saved_artists.add_pages(page.clone());
        app.artists = page.items;
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  #[allow(deprecated)]
  async fn user_artist_check_follow(&mut self, artist_ids: Vec<String>) {
    let ids: Vec<ArtistId<'static>> = artist_ids
      .iter()
      .filter_map(|id| ArtistId::from_id_or_uri(id).ok().map(|a| a.into_static()))
      .collect();
    match self.spotify.user_artist_check_follow(ids).await {
      Ok(results) => {
        let mut app = self.app.lock().await;
        for (id, is_followed) in artist_ids.iter().zip(results.iter()) {
          if *is_followed {
            app.followed_artist_ids_set.insert(id.clone());
          } else {
            app.followed_artist_ids_set.remove(id);
          }
        }
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn get_current_user_saved_albums(&mut self, offset: Option<u32>) {
    let offset = offset.unwrap_or(0);
    match self
      .spotify
      .current_user_saved_albums_manual(None, Some(50), Some(offset))
      .await
    {
      Ok(page) => {
        let mut app = self.app.lock().await;
        app.library.saved_albums.add_pages(page);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  #[allow(deprecated)]
  async fn current_user_saved_albums_contains(&mut self, album_ids: Vec<String>) {
    let ids: Vec<AlbumId<'static>> = album_ids
      .iter()
      .filter_map(|id| AlbumId::from_id_or_uri(id).ok().map(|a| a.into_static()))
      .collect();
    match self.spotify.current_user_saved_albums_contains(ids).await {
      Ok(results) => {
        let mut app = self.app.lock().await;
        for (id, is_saved) in album_ids.iter().zip(results.iter()) {
          if *is_saved {
            app.saved_album_ids_set.insert(id.clone());
          } else {
            app.saved_album_ids_set.remove(id);
          }
        }
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  #[allow(deprecated)]
  pub async fn current_user_saved_album_delete(&mut self, album_id: String) {
    match AlbumId::from_id_or_uri(&album_id) {
      Ok(aid) => {
        match self
          .spotify
          .current_user_saved_albums_delete([aid.as_ref()])
          .await
        {
          Ok(()) => {
            let mut app = self.app.lock().await;
            app.saved_album_ids_set.remove(&album_id);
          }
          Err(e) => {
            self.handle_error(anyhow!(e)).await;
          }
        }
      }
      Err(e) => {
        self.handle_error(anyhow!("Invalid album ID: {:?}", e)).await;
      }
    }
  }

  #[allow(deprecated)]
  async fn current_user_saved_album_add(&mut self, album_id: String) {
    match AlbumId::from_id_or_uri(&album_id) {
      Ok(aid) => {
        match self
          .spotify
          .current_user_saved_albums_add([aid.as_ref()])
          .await
        {
          Ok(()) => {
            let mut app = self.app.lock().await;
            app.saved_album_ids_set.insert(album_id);
          }
          Err(e) => {
            self.handle_error(anyhow!(e)).await;
          }
        }
      }
      Err(e) => {
        self.handle_error(anyhow!("Invalid album ID: {:?}", e)).await;
      }
    }
  }

  #[allow(deprecated)]
  async fn current_user_saved_shows_delete(&mut self, show_id: String) {
    match ShowId::from_id_or_uri(&show_id) {
      Ok(sid) => {
        match self
          .spotify
          .remove_users_saved_shows([sid.as_ref()], None)
          .await
        {
          Ok(()) => {
            let mut app = self.app.lock().await;
            app.saved_show_ids_set.remove(&show_id);
          }
          Err(e) => {
            self.handle_error(anyhow!(e)).await;
          }
        }
      }
      Err(e) => {
        self.handle_error(anyhow!("Invalid show ID: {:?}", e)).await;
      }
    }
  }

  #[allow(deprecated)]
  async fn current_user_saved_shows_add(&mut self, show_id: String) {
    match ShowId::from_id_or_uri(&show_id) {
      Ok(sid) => {
        match self.spotify.save_shows([sid.as_ref()]).await {
          Ok(()) => {
            let mut app = self.app.lock().await;
            app.saved_show_ids_set.insert(show_id);
          }
          Err(e) => {
            self.handle_error(anyhow!(e)).await;
          }
        }
      }
      Err(e) => {
        self.handle_error(anyhow!("Invalid show ID: {:?}", e)).await;
      }
    }
  }

  #[allow(deprecated)]
  async fn user_unfollow_artists(&mut self, artist_ids: Vec<String>) {
    let ids: Vec<ArtistId<'static>> = artist_ids
      .iter()
      .filter_map(|id| ArtistId::from_id_or_uri(id).ok().map(|a| a.into_static()))
      .collect();
    match self.spotify.user_unfollow_artists(ids).await {
      Ok(()) => {
        let mut app = self.app.lock().await;
        for id in &artist_ids {
          app.followed_artist_ids_set.remove(id);
        }
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  #[allow(deprecated)]
  async fn user_follow_artists(&mut self, artist_ids: Vec<String>) {
    let ids: Vec<ArtistId<'static>> = artist_ids
      .iter()
      .filter_map(|id| ArtistId::from_id_or_uri(id).ok().map(|a| a.into_static()))
      .collect();
    match self.spotify.user_follow_artists(ids).await {
      Ok(()) => {
        let mut app = self.app.lock().await;
        for id in &artist_ids {
          app.followed_artist_ids_set.insert(id.clone());
        }
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn user_follow_playlist(
    &mut self,
    _playlist_owner_id: String,
    playlist_id: String,
    is_public: Option<bool>,
  ) {
    match PlaylistId::from_id_or_uri(&playlist_id) {
      Ok(pid) => {
        if let Err(e) = self.spotify.playlist_follow(pid.as_ref(), is_public).await {
          self.handle_error(anyhow!(e)).await;
        }
      }
      Err(e) => {
        self.handle_error(anyhow!("Invalid playlist ID: {:?}", e)).await;
      }
    }
  }

  async fn user_unfollow_playlist(&mut self, _user_id: String, playlist_id: String) {
    match PlaylistId::from_id_or_uri(&playlist_id) {
      Ok(pid) => {
        if let Err(e) = self.spotify.playlist_unfollow(pid.as_ref()).await {
          self.handle_error(anyhow!(e)).await;
        }
      }
      Err(e) => {
        self.handle_error(anyhow!("Invalid playlist ID: {:?}", e)).await;
      }
    }
  }

  /// Fetch the artist preview for a playlist we already know the id of.
  /// Used by the new `populate_made_for_you_from_library` flow which gets
  /// the playlist list from `current_user_playlists` rather than search.
  async fn fetch_made_for_you_preview(
    &mut self,
    playlist_id: String,
    country: Option<Country>,
  ) {
    let market = country.map(Market::Country);
    if let Ok(pid) = PlaylistId::from_id_or_uri(&playlist_id) {
      if let Ok(track_page) = self
        .spotify
        .playlist_items_manual(pid.as_ref(), None, market, Some(10), Some(0))
        .await
      {
        let preview = build_artists_preview(&track_page);
        if !preview.is_empty() {
          let mut app = self.app.lock().await;
          app.made_for_you_previews.insert(playlist_id, preview);
        }
      }
    }
  }

  /// Fetch the user's top artists (medium-term) for the home page's
  /// "Your Top Artists" section. Surface errors so the user can see why
  /// the section is empty (missing scope, expired token, no listening
  /// history, etc.) instead of hanging on "Loading…" forever.
  async fn get_top_artists(&mut self) {
    match self
      .spotify
      .current_user_top_artists_manual(Some(TimeRange::MediumTerm), Some(10), Some(0))
      .await
    {
      Ok(page) => {
        let mut app = self.app.lock().await;
        app.top_artists = page.items;
      }
      Err(e) => {
        self.handle_error(anyhow!("Top artists fetch failed: {}", e)).await;
      }
    }
  }

  async fn get_audio_analysis(&mut self, uri: String) {
    match TrackId::from_id_or_uri(&uri) {
      Ok(track_id) => {
        match self.spotify.track_analysis(track_id.as_ref()).await {
          Ok(analysis) => {
            let mut app = self.app.lock().await;
            app.audio_analysis = Some(analysis);
          }
          Err(e) => {
            self.handle_error(anyhow!(e)).await;
          }
        }
      }
      Err(e) => {
        self.handle_error(anyhow!("Invalid track ID for audio analysis: {:?}", e)).await;
      }
    }
  }

  async fn get_current_user_playlists(&mut self) {
    // Paginate through ALL the user's playlists. Spotify's per-request cap is
    // 50, so accounts with more playlists need multiple fetches. We keep
    // accumulating until `next` is None.
    let mut all_items = Vec::new();
    let mut offset: u32 = 0;
    let limit: u32 = 50;
    loop {
      match self
        .spotify
        .current_user_playlists_manual(Some(limit), Some(offset))
        .await
      {
        Ok(page) => {
          let returned = page.items.len() as u32;
          all_items.extend(page.items);
          if page.next.is_none() || returned < limit {
            break;
          }
          offset = offset.saturating_add(limit);
        }
        Err(e) => {
          self.handle_error(anyhow!(e)).await;
          return;
        }
      }
    }

    let total = all_items.len() as u32;
    let mut app = self.app.lock().await;
    app.playlists = Some(Page {
      href: String::new(),
      items: all_items,
      limit: total.max(1),
      next: None,
      offset: 0,
      previous: None,
      total,
    });
  }

  async fn get_recently_played(&mut self) {
    match self
      .spotify
      .current_user_recently_played(Some(50), None)
      .await
    {
      Ok(page) => {
        let mut app = self.app.lock().await;
        app.recently_played.result = Some(page);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn get_album(&mut self, album_id: String) {
    match AlbumId::from_id_or_uri(&album_id) {
      Ok(aid) => {
        match self.spotify.album(aid.as_ref(), None).await {
          Ok(full_album) => {
            let mut app = self.app.lock().await;
            app.selected_album_full = Some(SelectedFullAlbum {
              album: full_album,
              selected_index: 0,
            });
            app.album_table_context = AlbumTableContext::Full;
            app.push_navigation_stack(RouteId::AlbumTracks, ActiveBlock::AlbumTracks);
          }
          Err(e) => {
            self.handle_error(anyhow!(e)).await;
          }
        }
      }
      Err(e) => {
        self.handle_error(anyhow!("Invalid album ID: {:?}", e)).await;
      }
    }
  }

  async fn get_album_for_track(&mut self, track_id: String) {
    match TrackId::from_id_or_uri(&track_id) {
      Ok(tid) => {
        match self.spotify.track(tid.as_ref(), None).await {
          Ok(track) => {
            if let Some(album_id) = track.album.id {
              match self.spotify.album(album_id.as_ref(), None).await {
                Ok(full_album) => {
                  let mut app = self.app.lock().await;
                  app.selected_album_full = Some(SelectedFullAlbum {
                    album: full_album,
                    selected_index: 0,
                  });
                  app.album_table_context = AlbumTableContext::Full;
                  app.push_navigation_stack(RouteId::AlbumTracks, ActiveBlock::AlbumTracks);
                }
                Err(e) => {
                  self.handle_error(anyhow!(e)).await;
                }
              }
            }
          }
          Err(e) => {
            self.handle_error(anyhow!(e)).await;
          }
        }
      }
      Err(e) => {
        self.handle_error(anyhow!("Invalid track ID: {:?}", e)).await;
      }
    }
  }

  async fn transfert_playback_to_device(&mut self, device_id: String) {
        match self.spotify.transfer_playback(&device_id, Some(true)).await {
      Ok(()) => {
                // Persist the selected device so future playback calls target it.
        self.client_config.device_id = Some(device_id.clone());
        if let Err(e) = self.client_config.set_device_id(device_id) {
                  }
        self.get_current_playback().await;
      }
      Err(e) => {
                self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn refresh_authentication(&mut self) {
    // The rspotify 0.16 client handles token refresh automatically via
    // `token_refreshing: true` in the Config. We can trigger a manual refresh.
    let is_expired = {
      let token_arc = self.spotify.get_token();
      let guard = token_arc.lock().await;
      let guard = guard.unwrap();
      guard.as_ref().map(|t| t.is_expired()).unwrap_or(true)
    };

    if is_expired {
      if let Err(e) = self.spotify.refresh_token().await {
        self.handle_error(anyhow!(e)).await;
      } else {
        // Update the token expiry in the app
        let maybe_expiry = {
          let token_arc = self.spotify.get_token();
          let guard = token_arc.lock().await;
          let guard = guard.unwrap();
          guard.as_ref().and_then(|token| token.expires_at).map(|expires_at| {
            SystemTime::UNIX_EPOCH + Duration::from_secs(expires_at.timestamp().max(0) as u64)
          })
        };
        if let Some(expiry) = maybe_expiry {
          let mut app = self.app.lock().await;
          app.spotify_token_expiry = expiry;
        }
      }
    }
  }

  async fn add_item_to_queue(&mut self, item: String) {
    let playable_id = if let Ok(id) = TrackId::from_id_or_uri(&item) {
      Some(PlayableId::Track(id.into_static()))
    } else if let Ok(id) = EpisodeId::from_id_or_uri(&item) {
      Some(PlayableId::Episode(id.into_static()))
    } else {
      None
    };

    if let Some(pid) = playable_id {
      if let Err(e) = self.spotify.add_item_to_queue(pid.as_ref(), self.client_config.device_id.as_deref()).await {
        self.handle_error(anyhow!(e)).await;
      }
    } else {
      self
        .handle_error(anyhow!("Invalid queue item URI: {}", item))
        .await;
    }
  }

  async fn get_queue(&mut self) {
    match self.spotify.current_user_queue().await {
      Ok(queue_payload) => {
        let mut app = self.app.lock().await;
        let new_len = queue_payload.queue.len();
        if app.queue_selected_index > new_len {
          app.queue_selected_index = new_len.saturating_sub(1);
        }
        app.queue = Some(queue_payload);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn skip_to_queue_index(&mut self, index: usize) {
    for _ in 0..index {
      if let Err(e) = self.spotify.next_track(self.client_config.device_id.as_deref()).await {
        self.handle_error(anyhow!(e)).await;
        break;
      }
    }
    self.get_queue().await;
  }

  async fn fetch_lyrics(
    &mut self,
    track_id: String,
    artist: String,
    track_name: String,
    _album: Option<String>,
    duration_ms: u32,
  ) {
    // Strategy: try /api/get first (fast, exact match). Note we deliberately
    // omit album_name from the query — including it makes lrclib reject
    // legitimate matches when the album-name strings differ between Spotify's
    // metadata and lrclib's stored entry (e.g. " (Deluxe)" suffix). On any
    // miss/404 we fall back to /api/search which does fuzzy matching and
    // returns a ranked list; we take the first hit.
    let duration_seconds = duration_ms / 1000;
    let duration_str = duration_seconds.to_string();
    let client = reqwest::Client::new();

    let lyrics_payload = fetch_lyrics_with_fallback(
      &client,
      &artist,
      &track_name,
      &duration_str,
    )
    .await;

    let mut app = self.app.lock().await;
    match lyrics_payload {
      Some((synced, plain)) if !synced.is_empty() || plain.is_some() => {
        app.lyrics = Some(crate::app::Lyrics { synced, plain });
      }
      _ => {
        app.lyrics = None;
      }
    }
    app.lyrics_for_track_id = Some(track_id);
    app.lyrics_loading = false;
  }
}

/// Fetch lyrics with a `/api/get` → `/api/search` fallback. Returns
/// `Some((synced, plain))` on either successful path; `None` on total failure.
async fn fetch_lyrics_with_fallback(
  client: &reqwest::Client,
  artist: &str,
  track_name: &str,
  duration_str: &str,
) -> Option<(Vec<(u32, String)>, Option<String>)> {
  // Path 1: /api/get with artist + track + duration (no album).
  let get_resp = client
    .get("https://lrclib.net/api/get")
    .query(&[
      ("artist_name", artist),
      ("track_name", track_name),
      ("duration", duration_str),
    ])
    .send()
    .await;
  if let Ok(resp) = get_resp {
    if resp.status().is_success() {
      if let Ok(body) = resp.json::<serde_json::Value>().await {
        let parsed = extract_lyrics_payload(&body);
        if parsed.is_some() {
          return parsed;
        }
      }
    }
  }

  // Path 2 (fallback): /api/search returns a ranked array. Take the first hit.
  let search_resp = client
    .get("https://lrclib.net/api/search")
    .query(&[("artist_name", artist), ("track_name", track_name)])
    .send()
    .await;
  if let Ok(resp) = search_resp {
    if resp.status().is_success() {
      if let Ok(body) = resp.json::<serde_json::Value>().await {
        if let Some(first) = body.as_array().and_then(|a| a.first()) {
          let parsed = extract_lyrics_payload(first);
          if parsed.is_some() {
            return parsed;
          }
        }
      }
    }
  }

  None
}

fn extract_lyrics_payload(
  body: &serde_json::Value,
) -> Option<(Vec<(u32, String)>, Option<String>)> {
  let synced_text = body
    .get("syncedLyrics")
    .and_then(|v| v.as_str())
    .map(|s| s.to_string());
  let plain_text = body
    .get("plainLyrics")
    .and_then(|v| v.as_str())
    .filter(|s| !s.is_empty())
    .map(|s| s.to_string());
  let synced = synced_text
    .as_deref()
    .map(parse_lrc)
    .unwrap_or_default();
  if synced.is_empty() && plain_text.is_none() {
    None
  } else {
    Some((synced, plain_text))
  }
}

/// Parse an LRC-format string into `(milliseconds, line)` pairs.
///
/// Lines that don't start with `[<min>:<sec>]` are silently dropped.
/// Multi-timestamp lines are not supported (only first match per line).
fn parse_lrc(text: &str) -> Vec<(u32, String)> {
  let mut out = Vec::new();
  for raw_line in text.lines() {
    let line = raw_line.trim_start();
    if !line.starts_with('[') {
      continue;
    }
    let close = match line.find(']') {
      Some(idx) => idx,
      None => continue,
    };
    let inside = &line[1..close];
    let rest = line[close + 1..].trim_start();
    let colon = match inside.find(':') {
      Some(idx) => idx,
      None => continue,
    };
    let minutes_str = &inside[..colon];
    let secs_full = &inside[colon + 1..];
    let (secs_str, frac_str) = match secs_full.find('.') {
      Some(dot) => (&secs_full[..dot], &secs_full[dot + 1..]),
      None => (secs_full, ""),
    };
    let minutes: u32 = match minutes_str.parse() {
      Ok(n) => n,
      Err(_) => continue,
    };
    let seconds: u32 = match secs_str.parse() {
      Ok(n) => n,
      Err(_) => continue,
    };
    let frac_trim = if frac_str.len() > 2 {
      &frac_str[..2]
    } else {
      frac_str
    };
    let centis: u32 = if frac_trim.is_empty() {
      0
    } else {
      match frac_trim.parse() {
        Ok(n) => n,
        Err(_) => continue,
      }
    };
    let ms = minutes * 60_000 + seconds * 1_000 + centis * 10;
    out.push((ms, rest.to_string()));
  }
  out
}

#[cfg(test)]
mod lyrics_tests {
  use super::parse_lrc;

  #[test]
  fn parse_lrc_basic() {
    let input = "[00:12.34]Hello world\n[01:05.00]Second line\n";
    assert_eq!(
      parse_lrc(input),
      vec![
        (12_340, "Hello world".to_string()),
        (65_000, "Second line".to_string()),
      ]
    );
  }

  #[test]
  fn parse_lrc_skips_garbage_lines() {
    let input = "garbage\n[bad:format]nope\n[01:00.00]ok\n";
    assert_eq!(parse_lrc(input), vec![(60_000, "ok".to_string())]);
  }

  #[test]
  fn parse_lrc_handles_three_digit_fraction() {
    let input = "[00:01.500]a\n[00:01.567]b\n";
    assert_eq!(
      parse_lrc(input),
      vec![
        (1_500, "a".to_string()),
        (1_560, "b".to_string()),
      ]
    );
  }
}

/// Build a comma-joined preview of up to 3 unique artist names across the
/// tracks in `track_page`. If there are 4 or more distinct artists overall,
/// append " and more"; if 1-3 distinct artists, return them as a plain join;
/// if zero (e.g. empty page or all-episode page), return an empty string.
fn build_artists_preview(track_page: &Page<PlaylistItem>) -> String {
  let mut unique: Vec<String> = Vec::new();
  let mut more = false;
  'outer: for item in track_page.items.iter() {
    if let Some(PlayableItem::Track(track)) = &item.track {
      for artist in &track.artists {
        if unique.iter().any(|n| n == &artist.name) {
          continue;
        }
        if unique.len() == 3 {
          more = true;
          break 'outer;
        }
        unique.push(artist.name.clone());
      }
    }
  }
  if unique.is_empty() {
    return String::new();
  }
  let joined = unique.join(", ");
  if more {
    format!("{} and more", joined)
  } else {
    joined
  }
}

#[cfg(test)]
mod artists_preview_tests {
  use super::build_artists_preview;
  use chrono::TimeDelta;
  use rspotify::model::{
    album::SimplifiedAlbum, artist::SimplifiedArtist, track::FullTrack,
    Page, PlayableItem, PlaylistItem,
  };

  fn make_artist(name: &str) -> SimplifiedArtist {
    SimplifiedArtist {
      external_urls: Default::default(),
      href: None,
      id: None,
      name: name.to_string(),
    }
  }

  fn make_track(artists: Vec<&str>) -> PlaylistItem {
    let full_track = FullTrack {
      album: SimplifiedAlbum {
        album_group: None,
        album_type: None,
        artists: vec![],
        available_markets: vec![],
        external_urls: Default::default(),
        href: None,
        id: None,
        images: vec![],
        name: String::new(),
        release_date: None,
        release_date_precision: None,
        restrictions: None,
      },
      artists: artists.into_iter().map(make_artist).collect(),
      available_markets: vec![],
      disc_number: 1,
      duration: TimeDelta::seconds(0),
      explicit: false,
      external_ids: Default::default(),
      external_urls: Default::default(),
      href: None,
      id: None,
      is_local: false,
      is_playable: None,
      linked_from: None,
      name: String::new(),
      popularity: 0,
      preview_url: None,
      restrictions: None,
      track_number: 0,
      r#type: rspotify::model::Type::Track,
    };
    PlaylistItem {
      added_at: None,
      added_by: None,
      is_local: false,
      track: Some(PlayableItem::Track(full_track)),
      item: None,
    }
  }

  fn page(items: Vec<PlaylistItem>) -> Page<PlaylistItem> {
    Page {
      href: String::new(),
      items,
      limit: 10,
      next: None,
      offset: 0,
      previous: None,
      total: 0,
    }
  }

  #[test]
  fn one_unique_artist() {
    let p = page(vec![
      make_track(vec!["Slayer"]),
      make_track(vec!["Slayer"]),
    ]);
    assert_eq!(build_artists_preview(&p), "Slayer");
  }

  #[test]
  fn exactly_three_unique_artists() {
    let p = page(vec![
      make_track(vec!["Slayer"]),
      make_track(vec!["Linkin Park"]),
      make_track(vec!["Metallica"]),
    ]);
    assert_eq!(build_artists_preview(&p), "Slayer, Linkin Park, Metallica");
  }

  #[test]
  fn four_or_more_unique_artists_appends_and_more() {
    let p = page(vec![
      make_track(vec!["Slayer"]),
      make_track(vec!["Linkin Park"]),
      make_track(vec!["Metallica"]),
      make_track(vec!["Pantera"]),
      make_track(vec!["Megadeth"]),
    ]);
    assert_eq!(
      build_artists_preview(&p),
      "Slayer, Linkin Park, Metallica and more"
    );
  }

  #[test]
  fn empty_page_returns_empty_string() {
    let p = page(vec![]);
    assert_eq!(build_artists_preview(&p), "");
  }
}
