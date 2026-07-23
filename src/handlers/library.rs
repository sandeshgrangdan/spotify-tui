use super::{
  super::app::{ActiveBlock, AlbumListContext, App, RouteId, TrackTableContext, LIBRARY_OPTIONS},
  common_key_events,
};
use crate::event::Key;
use crate::network::IoEvent;

pub fn handler(key: Key, app: &mut App) {
  match key {
    k if common_key_events::right_event(k) => common_key_events::handle_right_event(app),
    k if common_key_events::down_event(k) => {
      let next_index = common_key_events::on_down_press_handler(
        &LIBRARY_OPTIONS,
        Some(app.library.selected_index),
      );
      app.library.selected_index = next_index;
    }
    k if common_key_events::up_event(k) => {
      let next_index =
        common_key_events::on_up_press_handler(&LIBRARY_OPTIONS, Some(app.library.selected_index));
      app.library.selected_index = next_index;
    }
    k if common_key_events::high_event(k) => {
      let next_index = common_key_events::on_high_press_handler();
      app.library.selected_index = next_index;
    }
    k if common_key_events::middle_event(k) => {
      let next_index = common_key_events::on_middle_press_handler(&LIBRARY_OPTIONS);
      app.library.selected_index = next_index;
    }
    k if common_key_events::low_event(k) => {
      let next_index = common_key_events::on_low_press_handler(&LIBRARY_OPTIONS);
      app.library.selected_index = next_index
    }
    // `library` should probably be an array of structs with enums rather than just using indexes
    // like this
    Key::Enter => match app.library.selected_index {
      // Made For You,
      0 => {
        app.get_made_for_you();
        app.push_navigation_stack(RouteId::MadeForYou, ActiveBlock::MadeForYou);
      }
      // Recently Played,
      1 => {
        app.dispatch(IoEvent::GetRecentlyPlayed);
        app.push_navigation_stack(RouteId::RecentlyPlayed, ActiveBlock::RecentlyPlayed);
      }
      // Liked Songs,
      2 => {
        app.dispatch(IoEvent::GetCurrentSavedTracks(None));
        app.push_navigation_stack(RouteId::TrackTable, ActiveBlock::TrackTable);
      }
      // Albums,
      3 => {
        app.album_list_context = AlbumListContext::SavedAlbums;
        app.album_list_index = 0;
        app.dispatch(IoEvent::GetCurrentUserSavedAlbums(None));
        app.push_navigation_stack(RouteId::AlbumList, ActiveBlock::AlbumList);
      }
      //  Artists,
      4 => {
        app.dispatch(IoEvent::GetFollowedArtists(None));
        app.push_navigation_stack(RouteId::Artists, ActiveBlock::Artists);
      }
      // Podcasts,
      5 => {
        app.dispatch(IoEvent::GetCurrentUserSavedShows(None));
        app.push_navigation_stack(RouteId::Podcasts, ActiveBlock::Podcasts);
      }
      // Top Tracks,
      7 => {
        app.track_table.context = Some(TrackTableContext::TopTracks);
        app.dispatch(IoEvent::GetTopTracks);
        app.push_navigation_stack(RouteId::TrackTable, ActiveBlock::TrackTable);
      }
      // New Releases,
      6 => {
        app.album_list_context = AlbumListContext::NewReleases;
        app.album_list_index = 0;
        if app.library.new_releases.get_results(None).is_none() {
          app.dispatch(IoEvent::GetNewReleases(None));
        }
        app.push_navigation_stack(RouteId::AlbumList, ActiveBlock::AlbumList);
      }
      // This is required because Rust can't tell if this pattern in exhaustive
      _ => {}
    },
    _ => (),
  };
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn enter_on_new_releases_opens_album_list_in_new_releases_context() {
    let mut app = App::default();
    app.library.selected_index = 6; // "New Releases"

    handler(Key::Enter, &mut app);

    assert_eq!(app.album_list_context, AlbumListContext::NewReleases);
    let route = app.get_current_route();
    assert_eq!(route.id, RouteId::AlbumList);
    assert_eq!(route.active_block, ActiveBlock::AlbumList);
  }

  #[test]
  fn enter_on_top_tracks_opens_track_table_in_top_tracks_context() {
    let mut app = App::default();
    app.library.selected_index = 7; // "Top Tracks"

    handler(Key::Enter, &mut app);

    assert_eq!(app.track_table.context, Some(TrackTableContext::TopTracks));
    let route = app.get_current_route();
    assert_eq!(route.id, RouteId::TrackTable);
    assert_eq!(route.active_block, ActiveBlock::TrackTable);
  }

  #[test]
  fn enter_on_albums_resets_saved_albums_context() {
    let mut app = App::default();
    app.album_list_context = AlbumListContext::NewReleases;
    app.library.selected_index = 3; // "Albums"

    handler(Key::Enter, &mut app);

    assert_eq!(app.album_list_context, AlbumListContext::SavedAlbums);
  }
}
