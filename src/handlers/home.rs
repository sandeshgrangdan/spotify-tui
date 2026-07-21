use super::{
  super::app::{App, HomeBlock, HomeMode, TrackTableContext},
  common_key_events,
};
use crate::event::Key;
use crate::network::IoEvent;
use rspotify::prelude::Id;

fn made_for_you_len(app: &App) -> usize {
  app
    .library
    .made_for_you_playlists
    .get_results(None)
    .map(|p| p.items.len())
    .unwrap_or(0)
}

fn recommended_len(app: &App) -> usize {
  let mut seen: Vec<String> = Vec::new();
  if let Some(page) = app.recently_played.result.as_ref() {
    for item in &page.items {
      for artist in &item.track.artists {
        if !seen.iter().any(|n| n == &artist.name) {
          seen.push(artist.name.clone());
        }
        if seen.len() >= 12 {
          return 12;
        }
      }
    }
  }
  seen.len()
}

fn jump_back_len(app: &App) -> usize {
  app
    .recently_played
    .result
    .as_ref()
    .map(|p| p.items.len())
    .unwrap_or(0)
}

pub fn handler(key: Key, app: &mut App) {
  match app.home_mode {
    HomeMode::Music => {
      if app.home_section_entered {
        handle_music_row_level(key, app);
      } else {
        handle_music_section_level(key, app);
      }
    }
    HomeMode::Podcast => {
      if app.home_section_entered {
        handle_podcast_row_level(key, app);
      } else {
        handle_podcast_section_level(key, app);
      }
    }
  }
}

fn handle_music_section_level(key: Key, app: &mut App) {
  match key {
    k if common_key_events::left_event(k) => common_key_events::handle_left_event(app),
    k if common_key_events::down_event(k) => {
      app.home_selected_block = match app.home_selected_block {
        HomeBlock::MadeForYou => HomeBlock::RecommendedStations,
        HomeBlock::RecommendedStations => HomeBlock::JumpBackIn,
        HomeBlock::JumpBackIn => HomeBlock::MadeForYou,
        _ => app.home_selected_block,
      };
    }
    k if common_key_events::up_event(k) => {
      app.home_selected_block = match app.home_selected_block {
        HomeBlock::MadeForYou => HomeBlock::JumpBackIn,
        HomeBlock::RecommendedStations => HomeBlock::MadeForYou,
        HomeBlock::JumpBackIn => HomeBlock::RecommendedStations,
        _ => app.home_selected_block,
      };
    }
    Key::Enter => {
      app.home_section_entered = true;
    }
    _ => {}
  }
}

fn handle_music_row_level(key: Key, app: &mut App) {
  match key {
    k if common_key_events::left_event(k) => common_key_events::handle_left_event(app),
    k if common_key_events::down_event(k) => match app.home_selected_block {
      HomeBlock::MadeForYou => {
        let len = made_for_you_len(app);
        if len > 0 && app.home_made_for_you_index + 1 < len {
          app.home_made_for_you_index += 1;
        }
      }
      HomeBlock::RecommendedStations => {
        let len = recommended_len(app);
        if len > 0 && app.home_recommended_index + 1 < len {
          app.home_recommended_index += 1;
        }
      }
      HomeBlock::JumpBackIn => {
        let len = jump_back_len(app);
        if len > 0 && app.home_jump_back_index + 1 < len {
          app.home_jump_back_index += 1;
        }
      }
      _ => {}
    },
    k if common_key_events::up_event(k) => match app.home_selected_block {
      HomeBlock::MadeForYou => {
        if app.home_made_for_you_index > 0 {
          app.home_made_for_you_index -= 1;
        }
      }
      HomeBlock::RecommendedStations => {
        if app.home_recommended_index > 0 {
          app.home_recommended_index -= 1;
        }
      }
      HomeBlock::JumpBackIn => {
        if app.home_jump_back_index > 0 {
          app.home_jump_back_index -= 1;
        }
      }
      _ => {}
    },
    Key::Enter => match app.home_selected_block {
      HomeBlock::MadeForYou => {
        // Section now displays the user's top artists; Enter opens the
        // artist's page (top tracks + albums + related).
        if let Some(artist) = app.top_artists.get(app.home_made_for_you_index) {
          let artist_id = artist.id.id().to_string();
          let artist_name = artist.name.clone();
          app.get_artist(artist_id, artist_name);
        }
      }
      HomeBlock::RecommendedStations => {
        // Spotify's /recommendations endpoint is deprecated for newly-
        // registered apps (Nov 2024). Instead, open the seed artist's page —
        // the user can play a top track from there.
        if let Some(page) = app.recently_played.result.clone() {
          let mut seen: Vec<(String, String)> = Vec::new();
          for item in &page.items {
            for artist in &item.track.artists {
              if !seen.iter().any(|(_, n)| n == &artist.name) {
                let id = artist
                  .id
                  .as_ref()
                  .map(|i| i.id().to_string())
                  .unwrap_or_default();
                seen.push((id, artist.name.clone()));
              }
              if seen.len() >= 12 {
                break;
              }
            }
            if seen.len() >= 12 {
              break;
            }
          }
          if let Some((id, name)) = seen.get(app.home_recommended_index) {
            if !id.is_empty() {
              app.get_artist(id.clone(), name.clone());
            }
          }
        }
      }
      HomeBlock::JumpBackIn => {
        if let Some(page) = &app.recently_played.result.clone() {
          let track_uris: Vec<String> = page
            .items
            .iter()
            .map(|item| item.track.id.as_ref().map(|i| i.uri()).unwrap_or_default())
            .collect();
          app.dispatch(IoEvent::StartPlayback(
            None,
            Some(track_uris),
            Some(app.home_jump_back_index),
          ));
        }
      }
      _ => {}
    },
    _ => {}
  }
}

fn handle_podcast_section_level(key: Key, app: &mut App) {
  match key {
    k if common_key_events::left_event(k) => common_key_events::handle_left_event(app),
    k if common_key_events::down_event(k) => {
      app.home_selected_block = match app.home_selected_block {
        HomeBlock::YourShows => HomeBlock::ContinueListening,
        HomeBlock::ContinueListening => HomeBlock::EpisodesForYou,
        HomeBlock::EpisodesForYou => HomeBlock::YourShows,
        _ => HomeBlock::YourShows,
      };
    }
    k if common_key_events::up_event(k) => {
      app.home_selected_block = match app.home_selected_block {
        HomeBlock::YourShows => HomeBlock::EpisodesForYou,
        HomeBlock::ContinueListening => HomeBlock::YourShows,
        HomeBlock::EpisodesForYou => HomeBlock::ContinueListening,
        _ => HomeBlock::YourShows,
      };
    }
    Key::Enter => {
      app.home_section_entered = true;
    }
    _ => {}
  }
}

fn handle_podcast_row_level(key: Key, app: &mut App) {
  match key {
    k if common_key_events::left_event(k) => common_key_events::handle_left_event(app),
    k if common_key_events::down_event(k) => match app.home_selected_block {
      HomeBlock::YourShows => {
        let len = podcast_your_shows_len(app);
        if len > 0 && app.home_your_shows_index + 1 < len {
          app.home_your_shows_index += 1;
        }
      }
      HomeBlock::ContinueListening => {
        let len = podcast_continue_listening_len(app);
        if len > 0 && app.home_continue_listening_index + 1 < len {
          app.home_continue_listening_index += 1;
        }
      }
      HomeBlock::EpisodesForYou => {
        let len = podcast_episodes_for_you_len(app);
        if len > 0 && app.home_episodes_for_you_index + 1 < len {
          app.home_episodes_for_you_index += 1;
        }
      }
      _ => {}
    },
    k if common_key_events::up_event(k) => match app.home_selected_block {
      HomeBlock::YourShows => {
        if app.home_your_shows_index > 0 {
          app.home_your_shows_index -= 1;
        }
      }
      HomeBlock::ContinueListening => {
        if app.home_continue_listening_index > 0 {
          app.home_continue_listening_index -= 1;
        }
      }
      HomeBlock::EpisodesForYou => {
        if app.home_episodes_for_you_index > 0 {
          app.home_episodes_for_you_index -= 1;
        }
      }
      _ => {}
    },
    Key::Enter => match app.home_selected_block {
      HomeBlock::YourShows => {
        if let Some(page) = app.library.saved_shows.get_results(None) {
          if let Some(saved) = page.items.get(app.home_your_shows_index) {
            app.dispatch(IoEvent::GetShowEpisodes(Box::new(saved.show.clone())));
          }
        }
      }
      HomeBlock::ContinueListening => {
        if let Some(uri) =
          podcast_continue_listening_uri_at(app, app.home_continue_listening_index)
        {
          app.dispatch(IoEvent::StartPlayback(None, Some(vec![uri]), None));
        }
      }
      HomeBlock::EpisodesForYou => {
        if let Some(uri) =
          podcast_episodes_for_you_uri_at(app, app.home_episodes_for_you_index)
        {
          app.dispatch(IoEvent::StartPlayback(None, Some(vec![uri]), None));
        }
      }
      _ => {}
    },
    _ => {}
  }
}

fn podcast_your_shows_len(app: &App) -> usize {
  app
    .library
    .saved_shows
    .get_results(None)
    .map(|p| p.items.len())
    .unwrap_or(0)
}

fn podcast_continue_listening_len(app: &App) -> usize {
  podcast_continue_listening_episodes(app).len()
}

fn podcast_episodes_for_you_len(app: &App) -> usize {
  podcast_episodes_for_you_episodes(app).len()
}

fn podcast_continue_listening_uri_at(app: &App, index: usize) -> Option<String> {
  podcast_continue_listening_episodes(app)
    .get(index)
    .map(|(_show_name, episode)| rspotify::prelude::Id::uri(&episode.id))
}

fn podcast_episodes_for_you_uri_at(app: &App, index: usize) -> Option<String> {
  podcast_episodes_for_you_episodes(app)
    .get(index)
    .map(|(_show_name, episode)| rspotify::prelude::Id::uri(&episode.id))
}

fn podcast_continue_listening_episodes(
  app: &App,
) -> Vec<(String, rspotify::model::SimplifiedEpisode)> {
  let mut out: Vec<(String, rspotify::model::SimplifiedEpisode)> = Vec::new();
  if let Some(saved_page) = app.library.saved_shows.get_results(None) {
    for saved in &saved_page.items {
      let show_id = saved.show.id.id().to_string();
      if let Some(episodes) = app.podcast_episodes_per_show.get(&show_id) {
        for episode in episodes {
          if let Some(rp) = &episode.resume_point {
            if rp.resume_position.num_milliseconds() > 0 && !rp.fully_played {
              out.push((saved.show.name.clone(), episode.clone()));
            }
          }
        }
      }
    }
  }
  out.sort_by(|a, b| b.1.release_date.cmp(&a.1.release_date));
  out.truncate(10);
  out
}

fn podcast_episodes_for_you_episodes(
  app: &App,
) -> Vec<(String, rspotify::model::SimplifiedEpisode)> {
  let mut out: Vec<(String, rspotify::model::SimplifiedEpisode)> = Vec::new();
  if let Some(saved_page) = app.library.saved_shows.get_results(None) {
    for saved in &saved_page.items {
      let show_id = saved.show.id.id().to_string();
      if let Some(episodes) = app.podcast_episodes_per_show.get(&show_id) {
        if let Some(first) = episodes.first() {
          out.push((saved.show.name.clone(), first.clone()));
        }
      }
    }
  }
  out.sort_by(|a, b| b.1.release_date.cmp(&a.1.release_date));
  out.truncate(10);
  out
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn left_escapes_to_library_at_section_level() {
    use super::super::super::app::ActiveBlock;
    let mut app = App::default();
    handler(Key::Char('h'), &mut app);
    let route = app.get_current_route();
    assert_eq!(route.active_block, ActiveBlock::Empty);
    assert_eq!(route.hovered_block, ActiveBlock::Library);
  }

  #[test]
  fn down_at_section_level_cycles_sections() {
    let mut app = App::default();
    assert_eq!(app.home_selected_block, HomeBlock::MadeForYou);
    assert!(!app.home_section_entered);
    handler(Key::Char('j'), &mut app);
    assert_eq!(app.home_selected_block, HomeBlock::RecommendedStations);
    handler(Key::Char('j'), &mut app);
    assert_eq!(app.home_selected_block, HomeBlock::JumpBackIn);
    handler(Key::Char('j'), &mut app);
    assert_eq!(app.home_selected_block, HomeBlock::MadeForYou);
  }

  #[test]
  fn enter_at_section_level_enters_section() {
    let mut app = App::default();
    assert!(!app.home_section_entered);
    handler(Key::Enter, &mut app);
    assert!(app.home_section_entered);
  }

  #[test]
  fn down_at_row_level_does_not_cross_sections() {
    let mut app = App::default();
    app.home_section_entered = true;
    handler(Key::Char('j'), &mut app);
    assert_eq!(app.home_selected_block, HomeBlock::MadeForYou);
    assert_eq!(app.home_made_for_you_index, 0);
  }
}
