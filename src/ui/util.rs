use super::super::app::{ActiveBlock, App, ArtistBlock, HomeBlock, SearchResultBlock};
use crate::user_config::Theme;
use rspotify::model::SimplifiedArtist;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::BorderType;

pub const BASIC_VIEW_HEIGHT: u16 = 6;
pub const SMALL_TERMINAL_WIDTH: u16 = 150;
pub const SMALL_TERMINAL_HEIGHT: u16 = 45;

pub fn get_search_results_highlight_state(
  app: &App,
  block_to_match: SearchResultBlock,
) -> (bool, bool) {
  let current_route = app.get_current_route();
  (
    app.search_results.selected_block == block_to_match,
    current_route.hovered_block == ActiveBlock::SearchResultBlock
      && app.search_results.hovered_block == block_to_match,
  )
}

pub fn get_home_highlight_state(app: &App, block_to_match: HomeBlock) -> (bool, bool) {
  let current_route = app.get_current_route();
  let on_home = current_route.active_block == ActiveBlock::Home
    || current_route.hovered_block == ActiveBlock::Home;
  let column_match = app.home_selected_block == block_to_match;
  let is_active = current_route.active_block == ActiveBlock::Home
    && column_match
    && app.home_section_entered;
  let is_hovered = on_home && column_match && !is_active;
  (is_active, is_hovered)
}

pub fn get_artist_highlight_state(app: &App, block_to_match: ArtistBlock) -> (bool, bool) {
  let current_route = app.get_current_route();
  if let Some(artist) = &app.artist {
    let is_hovered = artist.artist_selected_block == block_to_match;
    let is_selected = current_route.hovered_block == ActiveBlock::ArtistBlock
      && artist.artist_hovered_block == block_to_match;
    (is_hovered, is_selected)
  } else {
    (false, false)
  }
}

pub fn get_color((is_active, is_hovered): (bool, bool), theme: Theme) -> Style {
  match (is_active, is_hovered) {
    (true, _) => Style::default()
      .fg(theme.selected)
      .add_modifier(Modifier::BOLD),
    (false, true) => Style::default().fg(theme.hovered),
    _ => Style::default().fg(theme.inactive),
  }
}

pub fn get_border_type((is_active, _is_hovered): (bool, bool)) -> BorderType {
  if is_active {
    BorderType::Thick
  } else {
    BorderType::Plain
  }
}

pub fn get_row_highlight_style(
  (is_active, is_hovered): (bool, bool),
  theme: Theme,
) -> Style {
  if is_active {
    Style::default()
      .bg(theme.selected)
      .fg(theme.playbar_background)
      .add_modifier(Modifier::BOLD)
  } else if is_hovered {
    Style::default()
      .fg(theme.hovered)
      .add_modifier(Modifier::BOLD)
  } else {
    Style::default()
      .fg(theme.text)
      .add_modifier(Modifier::BOLD)
  }
}

pub fn create_artist_string(artists: &[SimplifiedArtist]) -> String {
  artists
    .iter()
    .map(|artist| artist.name.to_string())
    .collect::<Vec<String>>()
    .join(", ")
}

pub fn millis_to_minutes(millis: u128) -> String {
  let minutes = millis / 60000;
  let seconds = (millis % 60000) / 1000;
  let seconds_display = if seconds < 10 {
    format!("0{}", seconds)
  } else {
    format!("{}", seconds)
  };

  if seconds == 60 {
    format!("{}:00", minutes + 1)
  } else {
    format!("{}:{}", minutes, seconds_display)
  }
}

pub fn display_track_progress(progress: u128, track_duration: u32) -> String {
  let duration = millis_to_minutes(u128::from(track_duration));
  let progress_display = millis_to_minutes(progress);
  let remaining = millis_to_minutes(u128::from(track_duration).saturating_sub(progress));

  format!("{}/{} (-{})", progress_display, duration, remaining,)
}

// `percentage` param needs to be between 0 and 1
pub fn get_percentage_width(width: u16, percentage: f32) -> u16 {
  let padding = 3;
  let width = width.saturating_sub(padding);
  (f32::from(width) * percentage) as u16
}

// Ensure track progress percentage is between 0 and 100 inclusive
pub fn get_track_progress_percentage(song_progress_ms: u128, track_duration_ms: u32) -> u16 {
  let min_perc = 0_f64;
  let track_progress = std::cmp::min(song_progress_ms, track_duration_ms.into());
  let track_perc = (track_progress as f64 / f64::from(track_duration_ms)) * 100_f64;
  min_perc.max(track_perc) as u16
}

// Make better use of space on small terminals
pub fn get_main_layout_margin(app: &App) -> u16 {
  if app.size.height > SMALL_TERMINAL_HEIGHT {
    1
  } else {
    0
  }
}

/// Screen `(column, row)` for the text cursor inside the search input box.
///
/// The box's position tracks the responsive layout in `draw_main_layout`:
/// in wide mode the search box lives in the sidebar *below* the 2-row top bar,
/// while in narrow mode it is the top row. Both add the outer layout margin
/// and the box's own border.
pub fn search_cursor_position(
  width: u16,
  height: u16,
  enforce_wide_search_bar: bool,
  input_cursor_position: u16,
) -> (u16, u16) {
  let margin = if height > SMALL_TERMINAL_HEIGHT { 1 } else { 0 };
  let border = 1;
  let wide = width >= SMALL_TERMINAL_WIDTH && !enforce_wide_search_bar;
  let top_bar = if wide { 2 } else { 0 };
  let column = margin + border + input_cursor_position;
  let row = margin + top_bar + border;
  (column, row)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn search_cursor_position_wide_clears_top_bar() {
    // width >= 150, height > 45: margin 1 + top bar 2 + border 1 = row 4.
    assert_eq!(search_cursor_position(180, 50, false, 0), (2, 4));
    // Cursor column tracks the text offset.
    assert_eq!(search_cursor_position(180, 50, false, 5), (7, 4));
  }

  #[test]
  fn search_cursor_position_narrow_has_no_top_bar() {
    // width < 150: no top bar. margin 1 + border 1 = row 2.
    assert_eq!(search_cursor_position(120, 50, false, 0), (2, 2));
  }

  #[test]
  fn search_cursor_position_enforce_wide_search_bar_uses_narrow_layout() {
    // enforce_wide_search_bar forces the narrow layout even on a wide terminal.
    assert_eq!(search_cursor_position(180, 50, true, 0), (2, 2));
  }

  #[test]
  fn search_cursor_position_small_terminal_drops_margin() {
    // height <= 45: no outer margin. narrow: border only -> row 1.
    assert_eq!(search_cursor_position(120, 40, false, 0), (1, 1));
    // wide small terminal: top bar 2 + border 1 = row 3, column border only.
    assert_eq!(search_cursor_position(180, 40, false, 0), (1, 3));
  }

  #[test]
  fn millis_to_minutes_test() {
    assert_eq!(millis_to_minutes(0), "0:00");
    assert_eq!(millis_to_minutes(1000), "0:01");
    assert_eq!(millis_to_minutes(1500), "0:01");
    assert_eq!(millis_to_minutes(1900), "0:01");
    assert_eq!(millis_to_minutes(60 * 1000), "1:00");
    assert_eq!(millis_to_minutes(60 * 1500), "1:30");
  }

  #[test]
  fn get_percentage_width_narrow_chunk_does_not_underflow() {
    assert_eq!(get_percentage_width(2, 0.5), 0);
  }

  #[test]
  fn display_track_progress_test() {
    assert_eq!(
      display_track_progress(0, 2 * 60 * 1000),
      "0:00/2:00 (-2:00)"
    );

    assert_eq!(
      display_track_progress(60 * 1000, 2 * 60 * 1000),
      "1:00/2:00 (-1:00)"
    );
  }

  #[test]
  fn get_track_progress_percentage_test() {
    let track_length = 60 * 1000;
    assert_eq!(get_track_progress_percentage(0, track_length), 0);
    assert_eq!(
      get_track_progress_percentage((60 * 1000) / 2, track_length),
      50
    );

    // If progress is somehow higher than total duration, 100 should be max
    assert_eq!(
      get_track_progress_percentage(60 * 1000 * 2, track_length),
      100
    );
  }
}
