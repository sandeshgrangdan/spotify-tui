use super::{
  super::app::{
    ActiveBlock, App, DialogContext, RecommendationsContext, RouteId, SearchResultBlock,
    TrackTableContext,
  },
  common_key_events,
};
use crate::event::Key;
use crate::network::IoEvent;
use rspotify::prelude::Id;

fn handle_down_press_on_selected_block(app: &mut App) {
  // Start selecting within the selected block
  match app.search_results.selected_block {
    SearchResultBlock::AlbumSearch => {
      if let Some(result) = &app.search_results.albums {
        let next_index = common_key_events::on_down_press_handler(
          &result.items,
          app.search_results.selected_album_index,
        );
        app.search_results.selected_album_index = Some(next_index);
      }
    }
    SearchResultBlock::SongSearch => {
      if let Some(result) = &app.search_results.tracks {
        let next_index = common_key_events::on_down_press_handler(
          &result.items,
          app.search_results.selected_tracks_index,
        );
        app.search_results.selected_tracks_index = Some(next_index);
      }
    }
    SearchResultBlock::ArtistSearch => {
      if let Some(result) = &app.search_results.artists {
        let next_index = common_key_events::on_down_press_handler(
          &result.items,
          app.search_results.selected_artists_index,
        );
        app.search_results.selected_artists_index = Some(next_index);
      }
    }
    SearchResultBlock::PlaylistSearch => {
      if let Some(result) = &app.search_results.playlists {
        let next_index = common_key_events::on_down_press_handler(
          &result.items,
          app.search_results.selected_playlists_index,
        );
        app.search_results.selected_playlists_index = Some(next_index);
      }
    }
    SearchResultBlock::ShowSearch => {
      if let Some(result) = &app.search_results.shows {
        let next_index = common_key_events::on_down_press_handler(
          &result.items,
          app.search_results.selected_shows_index,
        );
        app.search_results.selected_shows_index = Some(next_index);
      }
    }
    SearchResultBlock::EpisodeSearch => {
      if let Some(result) = &app.search_results.episodes {
        let next_index = common_key_events::on_down_press_handler(
          &result.items,
          app.search_results.selected_episodes_index,
        );
        app.search_results.selected_episodes_index = Some(next_index);
      }
    }
    SearchResultBlock::Empty => {}
  }
}

fn handle_down_press_on_hovered_block(app: &mut App) {
  match app.search_results.hovered_block {
    SearchResultBlock::AlbumSearch => {
      app.search_results.hovered_block = SearchResultBlock::ShowSearch;
    }
    SearchResultBlock::SongSearch => {
      app.search_results.hovered_block = SearchResultBlock::AlbumSearch;
    }
    SearchResultBlock::ArtistSearch => {
      app.search_results.hovered_block = SearchResultBlock::PlaylistSearch;
    }
    SearchResultBlock::PlaylistSearch => {
      app.search_results.hovered_block = SearchResultBlock::EpisodeSearch;
    }
    SearchResultBlock::ShowSearch => {
      app.search_results.hovered_block = SearchResultBlock::SongSearch;
    }
    SearchResultBlock::EpisodeSearch => {
      app.search_results.hovered_block = SearchResultBlock::ArtistSearch;
    }
    SearchResultBlock::Empty => {}
  }
}

fn handle_up_press_on_selected_block(app: &mut App) {
  // Start selecting within the selected block
  match app.search_results.selected_block {
    SearchResultBlock::AlbumSearch => {
      if let Some(result) = &app.search_results.albums {
        let next_index = common_key_events::on_up_press_handler(
          &result.items,
          app.search_results.selected_album_index,
        );
        app.search_results.selected_album_index = Some(next_index);
      }
    }
    SearchResultBlock::SongSearch => {
      if let Some(result) = &app.search_results.tracks {
        let next_index = common_key_events::on_up_press_handler(
          &result.items,
          app.search_results.selected_tracks_index,
        );
        app.search_results.selected_tracks_index = Some(next_index);
      }
    }
    SearchResultBlock::ArtistSearch => {
      if let Some(result) = &app.search_results.artists {
        let next_index = common_key_events::on_up_press_handler(
          &result.items,
          app.search_results.selected_artists_index,
        );
        app.search_results.selected_artists_index = Some(next_index);
      }
    }
    SearchResultBlock::PlaylistSearch => {
      if let Some(result) = &app.search_results.playlists {
        let next_index = common_key_events::on_up_press_handler(
          &result.items,
          app.search_results.selected_playlists_index,
        );
        app.search_results.selected_playlists_index = Some(next_index);
      }
    }
    SearchResultBlock::ShowSearch => {
      if let Some(result) = &app.search_results.shows {
        let next_index = common_key_events::on_up_press_handler(
          &result.items,
          app.search_results.selected_shows_index,
        );
        app.search_results.selected_shows_index = Some(next_index);
      }
    }
    SearchResultBlock::EpisodeSearch => {
      if let Some(result) = &app.search_results.episodes {
        let next_index = common_key_events::on_up_press_handler(
          &result.items,
          app.search_results.selected_episodes_index,
        );
        app.search_results.selected_episodes_index = Some(next_index);
      }
    }
    SearchResultBlock::Empty => {}
  }
}

fn handle_up_press_on_hovered_block(app: &mut App) {
  match app.search_results.hovered_block {
    SearchResultBlock::AlbumSearch => {
      app.search_results.hovered_block = SearchResultBlock::SongSearch;
    }
    SearchResultBlock::SongSearch => {
      app.search_results.hovered_block = SearchResultBlock::ShowSearch;
    }
    SearchResultBlock::ArtistSearch => {
      app.search_results.hovered_block = SearchResultBlock::EpisodeSearch;
    }
    SearchResultBlock::PlaylistSearch => {
      app.search_results.hovered_block = SearchResultBlock::ArtistSearch;
    }
    SearchResultBlock::ShowSearch => {
      app.search_results.hovered_block = SearchResultBlock::AlbumSearch;
    }
    SearchResultBlock::EpisodeSearch => {
      app.search_results.hovered_block = SearchResultBlock::PlaylistSearch;
    }
    SearchResultBlock::Empty => {}
  }
}

fn handle_high_press_on_selected_block(app: &mut App) {
  match app.search_results.selected_block {
    SearchResultBlock::AlbumSearch => {
      if let Some(_result) = &app.search_results.albums {
        let next_index = common_key_events::on_high_press_handler();
        app.search_results.selected_album_index = Some(next_index);
      }
    }
    SearchResultBlock::SongSearch => {
      if let Some(_result) = &app.search_results.tracks {
        let next_index = common_key_events::on_high_press_handler();
        app.search_results.selected_tracks_index = Some(next_index);
      }
    }
    SearchResultBlock::ArtistSearch => {
      if let Some(_result) = &app.search_results.artists {
        let next_index = common_key_events::on_high_press_handler();
        app.search_results.selected_artists_index = Some(next_index);
      }
    }
    SearchResultBlock::PlaylistSearch => {
      if let Some(_result) = &app.search_results.playlists {
        let next_index = common_key_events::on_high_press_handler();
        app.search_results.selected_playlists_index = Some(next_index);
      }
    }
    SearchResultBlock::ShowSearch => {
      if let Some(_result) = &app.search_results.shows {
        let next_index = common_key_events::on_high_press_handler();
        app.search_results.selected_shows_index = Some(next_index);
      }
    }
    SearchResultBlock::EpisodeSearch => {
      if let Some(_result) = &app.search_results.episodes {
        let next_index = common_key_events::on_high_press_handler();
        app.search_results.selected_episodes_index = Some(next_index);
      }
    }
    SearchResultBlock::Empty => {}
  }
}

fn handle_middle_press_on_selected_block(app: &mut App) {
  match app.search_results.selected_block {
    SearchResultBlock::AlbumSearch => {
      if let Some(result) = &app.search_results.albums {
        let next_index = common_key_events::on_middle_press_handler(&result.items);
        app.search_results.selected_album_index = Some(next_index);
      }
    }
    SearchResultBlock::SongSearch => {
      if let Some(result) = &app.search_results.tracks {
        let next_index = common_key_events::on_middle_press_handler(&result.items);
        app.search_results.selected_tracks_index = Some(next_index);
      }
    }
    SearchResultBlock::ArtistSearch => {
      if let Some(result) = &app.search_results.artists {
        let next_index = common_key_events::on_middle_press_handler(&result.items);
        app.search_results.selected_artists_index = Some(next_index);
      }
    }
    SearchResultBlock::PlaylistSearch => {
      if let Some(result) = &app.search_results.playlists {
        let next_index = common_key_events::on_middle_press_handler(&result.items);
        app.search_results.selected_playlists_index = Some(next_index);
      }
    }
    SearchResultBlock::ShowSearch => {
      if let Some(result) = &app.search_results.shows {
        let next_index = common_key_events::on_middle_press_handler(&result.items);
        app.search_results.selected_shows_index = Some(next_index);
      }
    }
    SearchResultBlock::EpisodeSearch => {
      if let Some(result) = &app.search_results.episodes {
        let next_index = common_key_events::on_middle_press_handler(&result.items);
        app.search_results.selected_episodes_index = Some(next_index);
      }
    }
    SearchResultBlock::Empty => {}
  }
}

fn handle_low_press_on_selected_block(app: &mut App) {
  match app.search_results.selected_block {
    SearchResultBlock::AlbumSearch => {
      if let Some(result) = &app.search_results.albums {
        let next_index = common_key_events::on_low_press_handler(&result.items);
        app.search_results.selected_album_index = Some(next_index);
      }
    }
    SearchResultBlock::SongSearch => {
      if let Some(result) = &app.search_results.tracks {
        let next_index = common_key_events::on_low_press_handler(&result.items);
        app.search_results.selected_tracks_index = Some(next_index);
      }
    }
    SearchResultBlock::ArtistSearch => {
      if let Some(result) = &app.search_results.artists {
        let next_index = common_key_events::on_low_press_handler(&result.items);
        app.search_results.selected_artists_index = Some(next_index);
      }
    }
    SearchResultBlock::PlaylistSearch => {
      if let Some(result) = &app.search_results.playlists {
        let next_index = common_key_events::on_low_press_handler(&result.items);
        app.search_results.selected_playlists_index = Some(next_index);
      }
    }
    SearchResultBlock::ShowSearch => {
      if let Some(result) = &app.search_results.shows {
        let next_index = common_key_events::on_low_press_handler(&result.items);
        app.search_results.selected_shows_index = Some(next_index);
      }
    }
    SearchResultBlock::EpisodeSearch => {
      if let Some(result) = &app.search_results.episodes {
        let next_index = common_key_events::on_low_press_handler(&result.items);
        app.search_results.selected_episodes_index = Some(next_index);
      }
    }
    SearchResultBlock::Empty => {}
  }
}

fn handle_add_item_to_queue(app: &mut App) {
  match &app.search_results.selected_block {
    SearchResultBlock::SongSearch => {
      if let (Some(index), Some(tracks)) = (
        app.search_results.selected_tracks_index,
        &app.search_results.tracks,
      ) {
        if let Some(track) = tracks.items.get(index) {
          let uri = track.id.as_ref().map(|i| i.uri()).unwrap_or_default();
          app.dispatch(IoEvent::AddItemToQueue(uri));
        }
      }
    }
    SearchResultBlock::ArtistSearch => {}
    SearchResultBlock::PlaylistSearch => {}
    SearchResultBlock::AlbumSearch => {}
    SearchResultBlock::ShowSearch => {}
    SearchResultBlock::EpisodeSearch => {
      if let (Some(index), Some(episodes)) = (
        app.search_results.selected_episodes_index,
        &app.search_results.episodes,
      ) {
        if let Some(episode) = episodes.items.get(index) {
          app.dispatch(IoEvent::AddItemToQueue(episode.id.uri()));
        }
      }
    }
    SearchResultBlock::Empty => {}
  };
}

fn handle_enter_event_on_selected_block(app: &mut App) {
  match &app.search_results.selected_block {
    SearchResultBlock::AlbumSearch => {
      if let (Some(index), Some(albums_result)) = (
        &app.search_results.selected_album_index,
        &app.search_results.albums,
      ) {
        if let Some(album) = albums_result.items.get(index.to_owned()).cloned() {
          app.track_table.context = Some(TrackTableContext::AlbumSearch);
          app.dispatch(IoEvent::GetAlbumTracks(Box::new(album)));
        };
      }
    }
    SearchResultBlock::SongSearch => {
      let index = app.search_results.selected_tracks_index;
      let tracks = app.search_results.tracks.clone();
      let track_uris = tracks.map(|tracks| {
        tracks
          .items
          .into_iter()
          .map(|track| track.id.as_ref().map(|i| i.uri()).unwrap_or_default())
          .collect::<Vec<String>>()
      });
      app.dispatch(IoEvent::StartPlayback(None, track_uris, index));
    }
    SearchResultBlock::ArtistSearch => {
      if let Some(index) = &app.search_results.selected_artists_index {
        if let Some(result) = app.search_results.artists.clone() {
          if let Some(artist) = result.items.get(index.to_owned()) {
            app.get_artist(artist.id.id().to_string(), artist.name.clone());
            app.push_navigation_stack(RouteId::Artist, ActiveBlock::ArtistBlock);
          };
        };
      };
    }
    SearchResultBlock::PlaylistSearch => {
      if let (Some(index), Some(playlists_result)) = (
        app.search_results.selected_playlists_index,
        &app.search_results.playlists,
      ) {
        if let Some(playlist) = playlists_result.items.get(index) {
          // Go to playlist tracks table
          app.track_table.context = Some(TrackTableContext::PlaylistSearch);
          let playlist_id = playlist.id.id().to_string();
          app.dispatch(IoEvent::GetPlaylistTracks(playlist_id, app.playlist_offset));
        };
      }
    }
    SearchResultBlock::ShowSearch => {
      if let (Some(index), Some(shows_result)) = (
        app.search_results.selected_shows_index,
        &app.search_results.shows,
      ) {
        if let Some(show) = shows_result.items.get(index).cloned() {
          // Go to show tracks table
          app.dispatch(IoEvent::GetShowEpisodes(Box::new(show)));
        };
      }
    }
    SearchResultBlock::EpisodeSearch => {
      if let (Some(index), Some(episodes)) = (
        app.search_results.selected_episodes_index,
        &app.search_results.episodes,
      ) {
        if let Some(episode) = episodes.items.get(index) {
          app.dispatch(IoEvent::StartPlayback(
            None,
            Some(vec![episode.id.uri()]),
            Some(0),
          ));
        }
      }
    }
    SearchResultBlock::Empty => {}
  };
}

fn handle_enter_event_on_hovered_block(app: &mut App) {
  match app.search_results.hovered_block {
    SearchResultBlock::AlbumSearch => {
      let next_index = app.search_results.selected_album_index.unwrap_or(0);

      app.search_results.selected_album_index = Some(next_index);
      app.search_results.selected_block = SearchResultBlock::AlbumSearch;
    }
    SearchResultBlock::SongSearch => {
      let next_index = app.search_results.selected_tracks_index.unwrap_or(0);

      app.search_results.selected_tracks_index = Some(next_index);
      app.search_results.selected_block = SearchResultBlock::SongSearch;
    }
    SearchResultBlock::ArtistSearch => {
      let next_index = app.search_results.selected_artists_index.unwrap_or(0);

      app.search_results.selected_artists_index = Some(next_index);
      app.search_results.selected_block = SearchResultBlock::ArtistSearch;
    }
    SearchResultBlock::PlaylistSearch => {
      let next_index = app.search_results.selected_playlists_index.unwrap_or(0);

      app.search_results.selected_playlists_index = Some(next_index);
      app.search_results.selected_block = SearchResultBlock::PlaylistSearch;
    }
    SearchResultBlock::ShowSearch => {
      let next_index = app.search_results.selected_shows_index.unwrap_or(0);

      app.search_results.selected_shows_index = Some(next_index);
      app.search_results.selected_block = SearchResultBlock::ShowSearch;
    }
    SearchResultBlock::EpisodeSearch => {
      let next_index = app.search_results.selected_episodes_index.unwrap_or(0);

      app.search_results.selected_episodes_index = Some(next_index);
      app.search_results.selected_block = SearchResultBlock::EpisodeSearch;
    }
    SearchResultBlock::Empty => {}
  };
}

fn handle_recommended_tracks(app: &mut App) {
  match app.search_results.selected_block {
    SearchResultBlock::AlbumSearch => {}
    SearchResultBlock::SongSearch => {
      if let Some(index) = &app.search_results.selected_tracks_index {
        if let Some(result) = app.search_results.tracks.clone() {
          if let Some(track) = result.items.get(index.to_owned()) {
            let track_id_list: Option<Vec<String>> =
              track.id.as_ref().map(|id| vec![id.to_string()]);

            app.recommendations_context = Some(RecommendationsContext::Song);
            app.recommendations_seed = track.name.clone();
            app.get_recommendations_for_seed(None, track_id_list, Some(track.clone()));
          };
        };
      };
    }
    SearchResultBlock::ArtistSearch => {
      if let Some(index) = &app.search_results.selected_artists_index {
        if let Some(result) = app.search_results.artists.clone() {
          if let Some(artist) = result.items.get(index.to_owned()) {
            let artist_id_list: Option<Vec<String>> = Some(vec![artist.id.id().to_string()]);
            app.recommendations_context = Some(RecommendationsContext::Artist);
            app.recommendations_seed = artist.name.clone();
            app.get_recommendations_for_seed(artist_id_list, None, None);
          };
        };
      };
    }
    SearchResultBlock::PlaylistSearch => {}
    SearchResultBlock::ShowSearch => {}
    SearchResultBlock::EpisodeSearch => {}
    SearchResultBlock::Empty => {}
  }
}

pub fn handler(key: Key, app: &mut App) {
  match key {
    Key::Esc => {
      app.search_results.selected_block = SearchResultBlock::Empty;
    }
    k if common_key_events::down_event(k) => {
      if app.search_results.selected_block != SearchResultBlock::Empty {
        handle_down_press_on_selected_block(app);
      } else {
        handle_down_press_on_hovered_block(app);
      }
    }
    k if common_key_events::up_event(k) => {
      if app.search_results.selected_block != SearchResultBlock::Empty {
        handle_up_press_on_selected_block(app);
      } else {
        handle_up_press_on_hovered_block(app);
      }
    }
    k if common_key_events::left_event(k) => {
      app.search_results.selected_block = SearchResultBlock::Empty;
      match app.search_results.hovered_block {
        SearchResultBlock::AlbumSearch => {
          common_key_events::handle_left_event(app);
        }
        SearchResultBlock::SongSearch => {
          common_key_events::handle_left_event(app);
        }
        SearchResultBlock::ArtistSearch => {
          app.search_results.hovered_block = SearchResultBlock::SongSearch;
        }
        SearchResultBlock::PlaylistSearch => {
          app.search_results.hovered_block = SearchResultBlock::AlbumSearch;
        }
        SearchResultBlock::ShowSearch => {
          common_key_events::handle_left_event(app);
        }
        SearchResultBlock::EpisodeSearch => {
          app.search_results.hovered_block = SearchResultBlock::ShowSearch;
        }
        SearchResultBlock::Empty => {}
      }
    }
    k if common_key_events::right_event(k) => {
      app.search_results.selected_block = SearchResultBlock::Empty;
      match app.search_results.hovered_block {
        SearchResultBlock::AlbumSearch => {
          app.search_results.hovered_block = SearchResultBlock::PlaylistSearch;
        }
        SearchResultBlock::SongSearch => {
          app.search_results.hovered_block = SearchResultBlock::ArtistSearch;
        }
        SearchResultBlock::ArtistSearch => {
          app.search_results.hovered_block = SearchResultBlock::SongSearch;
        }
        SearchResultBlock::PlaylistSearch => {
          app.search_results.hovered_block = SearchResultBlock::AlbumSearch;
        }
        SearchResultBlock::ShowSearch => {
          app.search_results.hovered_block = SearchResultBlock::EpisodeSearch;
        }
        SearchResultBlock::EpisodeSearch => {}
        SearchResultBlock::Empty => {}
      }
    }
    k if common_key_events::high_event(k) => {
      if app.search_results.selected_block != SearchResultBlock::Empty {
        handle_high_press_on_selected_block(app);
      }
    }
    k if common_key_events::middle_event(k) => {
      if app.search_results.selected_block != SearchResultBlock::Empty {
        handle_middle_press_on_selected_block(app);
      }
    }
    k if common_key_events::low_event(k) => {
      if app.search_results.selected_block != SearchResultBlock::Empty {
        handle_low_press_on_selected_block(app)
      }
    }
    // Handle pressing enter when block is selected to start playing track
    Key::Enter => match app.search_results.selected_block {
      SearchResultBlock::Empty => handle_enter_event_on_hovered_block(app),
      SearchResultBlock::PlaylistSearch => {
        app.playlist_offset = 0;
        handle_enter_event_on_selected_block(app);
      }
      _ => handle_enter_event_on_selected_block(app),
    },
    Key::Char('w') => match app.search_results.selected_block {
      SearchResultBlock::AlbumSearch => {
        app.current_user_saved_album_add(ActiveBlock::SearchResultBlock)
      }
      SearchResultBlock::SongSearch => {
        if let Some(track_id) = selected_search_track_id(app) {
          if !app.liked_song_ids_set.contains(&track_id) {
            app.dispatch(IoEvent::ToggleSaveTrack(track_id));
          }
        }
      }
      SearchResultBlock::EpisodeSearch => {}
      SearchResultBlock::ArtistSearch => app.user_follow_artists(ActiveBlock::SearchResultBlock),
      SearchResultBlock::PlaylistSearch => {
        app.user_follow_playlist();
      }
      SearchResultBlock::ShowSearch => app.user_follow_show(ActiveBlock::SearchResultBlock),
      SearchResultBlock::Empty => {}
    },
    Key::Char('D') => match app.search_results.selected_block {
      SearchResultBlock::AlbumSearch => {
        app.current_user_saved_album_delete(ActiveBlock::SearchResultBlock)
      }
      SearchResultBlock::SongSearch => {
        if let Some(track_id) = selected_search_track_id(app) {
          if app.liked_song_ids_set.contains(&track_id) {
            app.dispatch(IoEvent::ToggleSaveTrack(track_id));
          }
        }
      }
      SearchResultBlock::EpisodeSearch => {}
      SearchResultBlock::ArtistSearch => app.user_unfollow_artists(ActiveBlock::SearchResultBlock),
      SearchResultBlock::PlaylistSearch => {
        if let (Some(playlists), Some(selected_index)) = (
          &app.search_results.playlists,
          app.search_results.selected_playlists_index,
        ) {
          let selected_playlist = &playlists.items[selected_index].name;
          app.dialog = Some(selected_playlist.clone());
          app.confirm = false;

          app.push_navigation_stack(
            RouteId::Dialog,
            ActiveBlock::Dialog(DialogContext::PlaylistSearch),
          );
        }
      }
      SearchResultBlock::ShowSearch => app.user_unfollow_show(ActiveBlock::SearchResultBlock),
      SearchResultBlock::Empty => {}
    },
    Key::Char('r') => handle_recommended_tracks(app),
    Key::Char('s') => {
      if app.search_results.selected_block == SearchResultBlock::SongSearch {
        if let Some(track_id) = selected_search_track_id(app) {
          app.dispatch(IoEvent::ToggleSaveTrack(track_id));
        }
      }
    }
    _ if key == app.user_config.keys.add_to_playlist => {
      let uri = match app.search_results.selected_block {
        SearchResultBlock::SongSearch => app
          .search_results
          .selected_tracks_index
          .and_then(|i| {
            app
              .search_results
              .tracks
              .as_ref()
              .and_then(|p| p.items.get(i))
          })
          .and_then(|t| t.id.as_ref().map(|id| id.uri())),
        SearchResultBlock::EpisodeSearch => app
          .search_results
          .selected_episodes_index
          .and_then(|i| {
            app
              .search_results
              .episodes
              .as_ref()
              .and_then(|p| p.items.get(i))
          })
          .map(|e| e.id.uri()),
        _ => None,
      };
      if let Some(uri) = uri {
        app.open_playlist_picker(uri);
      }
    }
    _ if key == app.user_config.keys.add_item_to_queue => handle_add_item_to_queue(app),
    _ => {}
  }
}

/// Id of the track currently selected in the songs pane, if any.
fn selected_search_track_id(app: &App) -> Option<String> {
  let index = app.search_results.selected_tracks_index?;
  let tracks = app.search_results.tracks.as_ref()?;
  let track = tracks.items.get(index)?;
  track.id.as_ref().map(|id| id.id().to_string())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn hover_navigation_reaches_episode_pane() {
    let mut app = App::default();
    app.search_results.hovered_block = SearchResultBlock::PlaylistSearch;

    handler(Key::Char('j'), &mut app);
    assert_eq!(
      app.search_results.hovered_block,
      SearchResultBlock::EpisodeSearch
    );

    // and left goes back to the shows pane
    handler(Key::Char('h'), &mut app);
    assert_eq!(
      app.search_results.hovered_block,
      SearchResultBlock::ShowSearch
    );
  }

  #[test]
  fn enter_on_hovered_episode_pane_selects_it() {
    let mut app = App::default();
    app.search_results.hovered_block = SearchResultBlock::EpisodeSearch;
    app.search_results.selected_block = SearchResultBlock::Empty;

    handler(Key::Enter, &mut app);

    assert_eq!(
      app.search_results.selected_block,
      SearchResultBlock::EpisodeSearch
    );
  }
}
