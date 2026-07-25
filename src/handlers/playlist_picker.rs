use super::{super::app::App, common_key_events};
use crate::event::Key;
use crate::network::IoEvent;
use rspotify::prelude::Id;

fn playlists_len(app: &App) -> usize {
  app.modifiable_playlists().len()
}

fn close_picker(app: &mut App) {
  app.playlist_picker_uri = None;
  app.pop_navigation_stack();
}

pub fn handler(key: Key, app: &mut App) {
  match key {
    Key::Esc => close_picker(app),
    k if common_key_events::down_event(k) => {
      let len = playlists_len(app);
      if len > 0 {
        app.playlist_picker_index = (app.playlist_picker_index + 1) % len;
      }
    }
    k if common_key_events::up_event(k) => {
      let len = playlists_len(app);
      if len > 0 {
        app.playlist_picker_index = (app.playlist_picker_index + len - 1) % len;
      }
    }
    Key::Enter => {
      let playlist_id = app
        .modifiable_playlists()
        .get(app.playlist_picker_index)
        .map(|p| p.id.id().to_string());
      if let (Some(pid), Some(uri)) = (playlist_id, app.playlist_picker_uri.clone()) {
        app.dispatch(IoEvent::AddItemToPlaylist(pid, uri));
      }
      close_picker(app);
    }
    _ => {}
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::{ActiveBlock, RouteId};

  #[test]
  fn esc_closes_picker_and_clears_uri() {
    let mut app = App::default();
    app.playlist_picker_uri = Some("spotify:track:abc".to_string());
    app.push_navigation_stack(RouteId::Dialog, ActiveBlock::PlaylistPicker);

    handler(Key::Esc, &mut app);

    assert_eq!(app.playlist_picker_uri, None);
    assert_ne!(
      app.get_current_route().active_block,
      ActiveBlock::PlaylistPicker
    );
  }

  #[test]
  fn navigation_with_no_playlists_does_not_panic() {
    let mut app = App::default();
    handler(Key::Char('j'), &mut app);
    handler(Key::Char('k'), &mut app);
    assert_eq!(app.playlist_picker_index, 0);
  }

  #[test]
  fn navigation_with_no_playlists_wraps_safely() {
    let mut app = App::default();
    for _ in 0..3 {
      handler(Key::Char('j'), &mut app);
    }
    assert_eq!(app.playlist_picker_index, 0);
  }

  #[test]
  fn enter_with_no_playlists_just_closes() {
    let mut app = App::default();
    app.playlist_picker_uri = Some("spotify:track:abc".to_string());
    app.push_navigation_stack(RouteId::Dialog, ActiveBlock::PlaylistPicker);

    handler(Key::Enter, &mut app);

    assert_eq!(app.playlist_picker_uri, None);
    assert_ne!(
      app.get_current_route().active_block,
      ActiveBlock::PlaylistPicker
    );
  }
}
