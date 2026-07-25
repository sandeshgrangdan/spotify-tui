use super::{
  super::app::{ActiveBlock, App, RecommendationsContext, RouteId, TrackTableContext},
  common_key_events,
};
use crate::event::Key;
use crate::home_sections::{self, HomeItemAction, HomeSection};
use crate::network::IoEvent;
use rspotify::prelude::Id;

/// Two levels: `j`/`k` picks a section, `Enter` steps into it, then `j`/`k`
/// moves through that section's list and `Enter` opens the selected row. `Esc`
/// steps back out to section level; `h` leaves for the library sidebar.
pub fn handler(key: Key, app: &mut App) {
  let sections = home_sections::sections(app);
  if sections.is_empty() {
    return;
  }
  let section = sections
    .iter()
    .position(|section| section.block == app.home_selected_block)
    .unwrap_or(0);

  if app.home_section_entered {
    handle_row_level(key, app, &sections, section);
  } else {
    handle_section_level(key, app, &sections, section);
  }
}

fn handle_section_level(key: Key, app: &mut App, sections: &[HomeSection], section: usize) {
  match key {
    k if common_key_events::left_event(k) => common_key_events::handle_left_event(app),
    k if common_key_events::down_event(k) => {
      let next = common_key_events::on_down_press_handler(sections, Some(section));
      app.home_selected_block = sections[next].block;
    }
    k if common_key_events::up_event(k) => {
      let next = common_key_events::on_up_press_handler(sections, Some(section));
      app.home_selected_block = sections[next].block;
    }
    Key::Enter => {
      app.home_section_entered = true;
    }
    _ => {}
  }
}

fn handle_row_level(key: Key, app: &mut App, sections: &[HomeSection], section: usize) {
  let block = sections[section].block;
  let items = &sections[section].items;
  let last = items.len().saturating_sub(1);
  // A section's cursor can outlive its list (a shelf shrinks when data
  // reloads), so clamp before using it.
  let index = home_sections::item_index(app, block).min(last);

  match key {
    k if common_key_events::left_event(k) => common_key_events::handle_left_event(app),
    k if common_key_events::down_event(k) => {
      home_sections::set_item_index(app, block, (index + 1).min(last));
    }
    k if common_key_events::up_event(k) => {
      home_sections::set_item_index(app, block, index.saturating_sub(1));
    }
    k if common_key_events::high_event(k) => {
      home_sections::set_item_index(app, block, common_key_events::on_high_press_handler());
    }
    k if common_key_events::middle_event(k) => {
      home_sections::set_item_index(
        app,
        block,
        common_key_events::on_middle_press_handler(items),
      );
    }
    k if common_key_events::low_event(k) => {
      home_sections::set_item_index(app, block, common_key_events::on_low_press_handler(items));
    }
    Key::Enter => {
      if let Some(item) = items.get(index) {
        activate(app, item.action.clone());
      }
    }
    _ => {}
  }
}

fn activate(app: &mut App, action: HomeItemAction) {
  match action {
    HomeItemAction::OpenMix { index, id } => {
      // Reuses the Library → Made For You plumbing: the network handler pushes
      // the track table, whose paging reads `made_for_you_index`/`_offset`.
      app.track_table.context = Some(TrackTableContext::MadeForYou);
      app.made_for_you_index = index;
      app.playlist_offset = 0;
      app.made_for_you_offset = 0;
      app.dispatch(IoEvent::GetMadeForYouPlaylistTracks(id, 0));
    }
    HomeItemAction::PlayStation { seeds, name } => {
      app.recommendations_context = Some(RecommendationsContext::Artist);
      app.recommendations_seed = name;
      app.get_recommendations_for_seed(Some(seeds), None, None);
    }
    HomeItemAction::PlayMix { seeds, name } => {
      // Same builder as a station; only the table's heading differs, since
      // "Rock Mix" already reads as a name.
      app.recommendations_context = Some(RecommendationsContext::Mix);
      app.recommendations_seed = name;
      app.get_recommendations_for_seed(Some(seeds), None, None);
    }
    HomeItemAction::OpenOnRepeat => {
      if !app.on_repeat_tracks.is_empty() {
        app.track_table.context = Some(TrackTableContext::TopTracks);
        app.track_table.selected_index = 0;
        app.dispatch(IoEvent::SetTracksToTable(app.on_repeat_tracks.clone()));
        app.push_navigation_stack(RouteId::TrackTable, ActiveBlock::TrackTable);
      }
    }
    HomeItemAction::PlayRecent { index } => {
      // Play the whole history from this track on, so the section behaves like
      // the app's "jump back in" rather than stopping after one song.
      let uris: Option<Vec<String>> = app.recently_played.result.as_ref().map(|page| {
        page
          .items
          .iter()
          .map(|item| {
            item
              .track
              .id
              .as_ref()
              .map(|id| id.uri())
              .unwrap_or_default()
          })
          .collect()
      });
      if let Some(uris) = uris {
        app.dispatch(IoEvent::StartPlayback(None, Some(uris), Some(index)));
      }
    }
    HomeItemAction::OpenArtist { id, name } => app.get_artist(id, name),
    HomeItemAction::OpenShow { index } => {
      let show = app
        .library
        .saved_shows
        .get_results(None)
        .and_then(|page| page.items.get(index))
        .map(|saved| saved.show.clone());
      if let Some(show) = show {
        app.dispatch(IoEvent::GetShowEpisodes(Box::new(show)));
      }
    }
    HomeItemAction::PlayEpisode { uri } => {
      app.dispatch(IoEvent::StartPlayback(None, Some(vec![uri]), None));
    }
    HomeItemAction::Inert => {}
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::{HomeBlock, HomeMode};

  #[allow(deprecated)]
  fn make_artist(name: &str, id: &str) -> rspotify::model::FullArtist {
    use rspotify::model::{ArtistId, Followers, FullArtist};
    FullArtist {
      external_urls: Default::default(),
      followers: Followers { total: 0 },
      genres: vec![],
      href: String::new(),
      id: ArtistId::from_id(id.to_string()).unwrap(),
      images: vec![],
      name: name.to_string(),
      popularity: 0,
    }
  }

  fn app_with_artists() -> App {
    let mut app = App::default();
    app.top_artists = vec![
      make_artist("A", "2CIMQHirSU0MQqyYHq0eOx"),
      make_artist("B", "57dN52uHvrHOxijzpIgu3E"),
      make_artist("C", "1vCWHaC5f2uS3yhpwWbIA6"),
    ];
    app
  }

  #[test]
  fn left_at_section_level_escapes_to_the_library() {
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
    for expected in [
      HomeBlock::RecommendedStations,
      HomeBlock::JumpBackIn,
      HomeBlock::TopArtists,
      // Wraps back round to the first section.
      HomeBlock::MadeForYou,
    ] {
      handler(Key::Char('j'), &mut app);
      assert_eq!(app.home_selected_block, expected);
    }
    handler(Key::Char('k'), &mut app);
    assert_eq!(app.home_selected_block, HomeBlock::TopArtists);
  }

  #[test]
  fn podcast_mode_cycles_its_own_sections() {
    let mut app = App::default();
    app.home_mode = HomeMode::Podcast;
    app.home_selected_block = HomeBlock::YourShows;
    handler(Key::Char('j'), &mut app);
    assert_eq!(app.home_selected_block, HomeBlock::LatestEpisodes);
    handler(Key::Char('j'), &mut app);
    assert_eq!(app.home_selected_block, HomeBlock::ContinueListening);
    handler(Key::Char('j'), &mut app);
    assert_eq!(app.home_selected_block, HomeBlock::YourShows);
  }

  #[test]
  fn enter_at_section_level_enters_the_section() {
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

  #[test]
  fn down_and_up_at_row_level_move_and_clamp() {
    let mut app = app_with_artists();
    app.home_selected_block = HomeBlock::TopArtists;
    app.home_section_entered = true;

    handler(Key::Char('j'), &mut app);
    assert_eq!(app.home_top_artists_index, 1);
    handler(Key::Char('j'), &mut app);
    assert_eq!(app.home_top_artists_index, 2);
    // Clamped at the last row rather than wrapping.
    handler(Key::Char('j'), &mut app);
    assert_eq!(app.home_top_artists_index, 2);

    handler(Key::Char('k'), &mut app);
    assert_eq!(app.home_top_artists_index, 1);
    // Another section's cursor is untouched.
    assert_eq!(app.home_made_for_you_index, 0);
  }

  #[test]
  fn high_middle_low_jump_within_a_section() {
    let mut app = app_with_artists();
    app.home_selected_block = HomeBlock::TopArtists;
    app.home_section_entered = true;

    handler(Key::Char('L'), &mut app);
    assert_eq!(app.home_top_artists_index, 2);
    handler(Key::Char('M'), &mut app);
    assert_eq!(app.home_top_artists_index, 1);
    handler(Key::Char('H'), &mut app);
    assert_eq!(app.home_top_artists_index, 0);
  }

  #[test]
  fn enter_on_a_station_seeds_the_recommendations_view() {
    let mut app = app_with_artists();
    app.home_selected_block = HomeBlock::RecommendedStations;
    app.home_section_entered = true;
    handler(Key::Char('j'), &mut app);
    handler(Key::Enter, &mut app);
    assert_eq!(app.recommendations_seed, "B");
    assert_eq!(
      app.recommendations_context,
      Some(RecommendationsContext::Artist)
    );
  }

  #[test]
  fn enter_on_a_placeholder_row_does_nothing() {
    let mut app = App::default();
    // Nothing has loaded, so the section holds only its placeholder row.
    app.home_section_entered = true;
    handler(Key::Enter, &mut app);
    assert!(app.track_table.context.is_none());
    assert!(!app.is_loading);
  }

  #[test]
  fn opening_a_mix_points_the_track_table_at_it() {
    let mut app = App::default();
    activate(
      &mut app,
      HomeItemAction::OpenMix {
        index: 3,
        id: "37i9dQZF1E35Ly1BdOZlbY".to_owned(),
      },
    );
    assert_eq!(app.track_table.context, Some(TrackTableContext::MadeForYou));
    // Track paging in `track_table.rs` looks the playlist up by this index.
    assert_eq!(app.made_for_you_index, 3);
    assert_eq!(app.made_for_you_offset, 0);
    assert_eq!(app.playlist_offset, 0);
  }

  #[test]
  fn back_steps_out_of_a_section_then_out_to_hover_mode() {
    // The back key (`q`) routes through `App::back_out_of_home` in main.rs.
    let mut app = App::default();
    app.set_current_route_state(Some(ActiveBlock::Home), Some(ActiveBlock::Home));
    app.home_section_entered = true;

    assert!(app.back_out_of_home());
    assert!(!app.home_section_entered);
    // Still on the home pane, just back at section level.
    assert_eq!(app.get_current_route().active_block, ActiveBlock::Home);

    // A second press leaves the pane entirely.
    assert!(app.back_out_of_home());
    assert_eq!(app.get_current_route().active_block, ActiveBlock::Empty);
  }

  #[test]
  fn back_outside_the_home_pane_is_left_to_the_navigation_stack() {
    let mut app = App::default();
    app.set_current_route_state(Some(ActiveBlock::Empty), Some(ActiveBlock::Library));
    assert!(!app.back_out_of_home());

    app.push_navigation_stack(RouteId::TrackTable, ActiveBlock::TrackTable);
    app.home_section_entered = true;
    assert!(!app.back_out_of_home());
    // Leaves the home state alone so returning to it keeps the cursor.
    assert!(app.home_section_entered);
  }

  #[test]
  fn escape_and_back_leave_a_section_the_same_way() {
    let mut escaped = App::default();
    escaped.set_current_route_state(Some(ActiveBlock::Home), Some(ActiveBlock::Home));
    escaped.home_section_entered = true;
    super::super::handle_app(Key::Esc, &mut escaped);

    let mut backed = App::default();
    backed.set_current_route_state(Some(ActiveBlock::Home), Some(ActiveBlock::Home));
    backed.home_section_entered = true;
    backed.back_out_of_home();

    assert_eq!(escaped.home_section_entered, backed.home_section_entered);
    assert_eq!(
      escaped.get_current_route().active_block,
      backed.get_current_route().active_block
    );
  }

  #[test]
  fn a_stale_cursor_past_the_end_does_not_panic() {
    let mut app = app_with_artists();
    app.home_selected_block = HomeBlock::TopArtists;
    app.home_section_entered = true;
    app.home_top_artists_index = 99;
    handler(Key::Enter, &mut app);
    handler(Key::Char('j'), &mut app);
    handler(Key::Char('k'), &mut app);
    assert_eq!(app.home_top_artists_index, 1);
  }
}
