use super::common_key_events;
use crate::{
  app::{ActiveBlock, AlbumListContext, AlbumTableContext, App, RouteId, SelectedFullAlbum},
  event::Key,
  network::IoEvent,
};
use rspotify::prelude::Id;

fn albums_len(app: &App) -> usize {
  match app.album_list_context {
    AlbumListContext::SavedAlbums => app
      .library
      .saved_albums
      .get_results(None)
      .map(|p| p.items.len())
      .unwrap_or(0),
    AlbumListContext::NewReleases => app
      .library
      .new_releases
      .get_results(None)
      .map(|p| p.items.len())
      .unwrap_or(0),
  }
}

pub fn handler(key: Key, app: &mut App) {
  match key {
    k if common_key_events::left_event(k) => common_key_events::handle_left_event(app),
    k if common_key_events::down_event(k) => {
      let len = albums_len(app);
      if len > 0 {
        app.album_list_index = (app.album_list_index + 1) % len;
      }
    }
    k if common_key_events::up_event(k) => {
      let len = albums_len(app);
      if len > 0 {
        app.album_list_index = (app.album_list_index + len - 1) % len;
      }
    }
    k if common_key_events::high_event(k) => {
      if albums_len(app) > 0 {
        app.album_list_index = 0;
      }
    }
    k if common_key_events::middle_event(k) => {
      let len = albums_len(app);
      if len > 0 {
        app.album_list_index = len / 2;
      }
    }
    k if common_key_events::low_event(k) => {
      let len = albums_len(app);
      if len > 0 {
        app.album_list_index = len - 1;
      }
    }
    Key::Enter => match app.album_list_context {
      AlbumListContext::SavedAlbums => {
        if let Some(albums) = app.library.saved_albums.get_results(None) {
          if let Some(selected_album) = albums.items.get(app.album_list_index) {
            app.selected_album_full = Some(SelectedFullAlbum {
              album: selected_album.album.clone(),
              selected_index: 0,
            });
            app.album_table_context = AlbumTableContext::Full;
            app.push_navigation_stack(RouteId::AlbumTracks, ActiveBlock::AlbumTracks);
          };
        }
      }
      AlbumListContext::NewReleases => {
        if let Some(page) = app.library.new_releases.get_results(None) {
          if let Some(album) = page.items.get(app.album_list_index) {
            app.dispatch(IoEvent::GetAlbumTracks(Box::new(album.clone())));
          }
        }
      }
    },
    k if k == app.user_config.keys.next_page => match app.album_list_context {
      AlbumListContext::SavedAlbums => app.get_current_user_saved_albums_next(),
      AlbumListContext::NewReleases => app.get_new_releases_next(),
    },
    k if k == app.user_config.keys.previous_page => match app.album_list_context {
      AlbumListContext::SavedAlbums => app.get_current_user_saved_albums_previous(),
      AlbumListContext::NewReleases => app.get_new_releases_previous(),
    },
    Key::Char('w') => {
      if let AlbumListContext::NewReleases = app.album_list_context {
        if let Some(page) = app.library.new_releases.get_results(None) {
          if let Some(album) = page.items.get(app.album_list_index) {
            if let Some(id) = &album.id {
              app.dispatch(IoEvent::CurrentUserSavedAlbumAdd(id.id().to_string()));
            }
          }
        }
      }
    }
    Key::Char('D') => {
      if let AlbumListContext::SavedAlbums = app.album_list_context {
        app.current_user_saved_album_delete(ActiveBlock::AlbumList);
      }
    }
    _ => {}
  };
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn on_left_press() {
    let mut app = App::default();
    app.set_current_route_state(
      Some(ActiveBlock::AlbumTracks),
      Some(ActiveBlock::AlbumTracks),
    );

    handler(Key::Left, &mut app);
    let current_route = app.get_current_route();
    assert_eq!(current_route.active_block, ActiveBlock::Empty);
    assert_eq!(current_route.hovered_block, ActiveBlock::Library);
  }

  #[test]
  fn on_esc() {
    let mut app = App::default();

    handler(Key::Esc, &mut app);

    let current_route = app.get_current_route();
    assert_eq!(current_route.active_block, ActiveBlock::Empty);
  }
}
