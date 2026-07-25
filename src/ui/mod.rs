pub mod audio_analysis;
pub mod help;
pub mod util;
use super::app::{
  ActiveBlock, AlbumListContext, AlbumTableContext, App, ArtistBlock, EpisodeTableContext,
  HomeMode, InputMode, RecommendationsContext, RouteId, SearchResultBlock, TrackTableContext,
  LIBRARY_OPTIONS,
};
use crate::home_sections;
use help::get_help_docs;
use rspotify::model::ResumePoint;
use rspotify::model::PlayableItem;
use rspotify::model::RepeatState;
use rspotify::prelude::Id as SpotifyId;
use ratatui::{
  layout::{Alignment, Constraint, Direction, Layout, Rect},
  style::{Modifier, Style},
  text::{Line, Span, Text},
  widgets::{
    Block, BorderType, Borders, Clear, Gauge, List, ListItem, ListState, Paragraph, Row, Table,
    Wrap,
  },
  Frame,
};
use util::{
  create_artist_string, display_track_progress, get_artist_highlight_state, get_border_type,
  get_color, get_home_highlight_state, get_percentage_width, get_row_highlight_style,
  get_search_results_highlight_state, get_track_progress_percentage, millis_to_minutes,
  BASIC_VIEW_HEIGHT, SMALL_TERMINAL_WIDTH,
};

pub enum TableId {
  Album,
  AlbumList,
  Artist,
  Podcast,
  Song,
  RecentlyPlayed,
  MadeForYou,
  PodcastEpisodes,
}

#[derive(PartialEq)]
pub enum ColumnId {
  None,
  Title,
  Liked,
}

impl Default for ColumnId {
  fn default() -> Self {
    ColumnId::None
  }
}

pub struct TableHeader<'a> {
  id: TableId,
  items: Vec<TableHeaderItem<'a>>,
}

impl TableHeader<'_> {
  pub fn get_index(&self, id: ColumnId) -> Option<usize> {
    self.items.iter().position(|item| item.id == id)
  }
}

#[derive(Default)]
pub struct TableHeaderItem<'a> {
  id: ColumnId,
  text: &'a str,
  width: u16,
}

pub struct TableItem {
  id: String,
  format: Vec<String>,
}

pub fn draw_help_menu(f: &mut Frame, app: &App)
{
  let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([Constraint::Percentage(100)].as_ref())
    .margin(2)
    .split(f.area());

  // Create a one-column table to avoid flickering due to non-determinism when
  // resolving constraints on widths of table columns.
  let format_row =
    |r: Vec<String>| -> Vec<String> { vec![format!("{:50}{:40}{:20}", r[0], r[1], r[2])] };

  let help_menu_style = Style::default().fg(app.user_config.theme.text);
  let header = ["Description", "Event", "Context"];
  let header = format_row(header.iter().map(|s| s.to_string()).collect());

  let help_docs = get_help_docs(&app.user_config.keys);
  let help_docs = help_docs
    .into_iter()
    .map(format_row)
    .collect::<Vec<Vec<String>>>();
  let help_docs = &help_docs[app.help_menu_offset as usize..];

  let rows = help_docs
    .iter()
    .map(|item| Row::new(item.clone()).style(help_menu_style));

  let help_menu = Table::new(rows, [Constraint::Percentage(100)])
    .header(Row::new(header))
    .block(
      Block::default()
        .borders(Borders::ALL)
        .style(help_menu_style)
        .title(Span::styled(
          "Help (press <Esc> to go back)",
          help_menu_style,
        ))
        .border_style(help_menu_style),
    )
    .style(help_menu_style);
  f.render_widget(help_menu, chunks[0]);
}

pub fn draw_input_and_help_box(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  // Check for the width and change the contraints accordingly
  let chunks = Layout::default()
    .direction(Direction::Horizontal)
    .constraints(
      if app.size.width >= SMALL_TERMINAL_WIDTH && !app.user_config.behavior.enforce_wide_search_bar
      {
        [Constraint::Percentage(65), Constraint::Percentage(35)].as_ref()
      } else {
        [Constraint::Percentage(90), Constraint::Percentage(10)].as_ref()
      },
    )
    .split(layout_chunk);

  let current_route = app.get_current_route();

  let highlight_state = (
    current_route.active_block == ActiveBlock::Input,
    current_route.hovered_block == ActiveBlock::Input,
  );

  let input_string: String = app.input.iter().collect();
  let lines = Text::from((&input_string).as_str());
  let input = Paragraph::new(lines).block(
    Block::default()
      .borders(Borders::ALL)
      .border_type(get_border_type(highlight_state))
      .title(Span::styled(
        match app.input_mode {
          InputMode::NewPlaylist => "New playlist name",
          InputMode::Search => "Search",
        },
        get_color(highlight_state, app.user_config.theme),
      ))
      .border_style(get_color(highlight_state, app.user_config.theme)),
  );
  f.render_widget(input, chunks[0]);

  let show_loading = app.is_loading && app.user_config.behavior.show_loading_indicator;
  let help_block_text = if show_loading {
    (app.user_config.theme.hint, "Loading...")
  } else {
    (app.user_config.theme.inactive, "Type ?")
  };

  let block = Block::default()
    .title(Span::styled("Help", Style::default().fg(help_block_text.0)))
    .borders(Borders::ALL)
    .border_style(Style::default().fg(help_block_text.0));

  let lines = Text::from(help_block_text.1);
  let help = Paragraph::new(lines)
    .block(block)
    .style(Style::default().fg(help_block_text.0));
  f.render_widget(help, chunks[1]);
}

pub fn draw_top_bar(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let theme = app.user_config.theme;

  // Two stacked rows: content row + a thin horizontal rule.
  let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([Constraint::Length(1), Constraint::Length(1)].as_ref())
    .split(layout_chunk);

  // Row 1: tabs left, greeting right.
  let row1 = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
    .split(chunks[0]);

  let podcast_active = matches!(app.home_mode, HomeMode::Podcast);
  let active_tab_style = Style::default()
    .fg(theme.banner)
    .add_modifier(Modifier::BOLD);
  let inactive_tab_style = Style::default().fg(theme.inactive);

  let music_style = if podcast_active {
    inactive_tab_style
  } else {
    active_tab_style
  };
  let podcasts_style = if podcast_active {
    active_tab_style
  } else {
    inactive_tab_style
  };

  // Lyrics ON/OFF status pill — sits between the tabs and the greeting so
  // the user can see the key actually fired.
  let (lyrics_status_text, lyrics_status_style) = if app.lyrics_visible {
    (
      "  Lyrics: ON ",
      Style::default()
        .fg(theme.banner)
        .add_modifier(Modifier::BOLD),
    )
  } else {
    ("  Lyrics: OFF ", Style::default().fg(theme.inactive))
  };

  let tabs = Paragraph::new(Line::from(vec![
    Span::styled("  Music", music_style),
    Span::raw("   "),
    Span::styled("Podcasts", podcasts_style),
    Span::raw("   "),
    Span::styled(lyrics_status_text, lyrics_status_style),
  ]));
  f.render_widget(tabs, row1[0]);

  let display_name = app
    .user
    .as_ref()
    .and_then(|u| u.display_name.clone())
    .unwrap_or_default();

  let greeting_text = if display_name.is_empty() {
    String::new()
  } else {
    format!("Hi, {}  ", display_name)
  };

  let greeting = Paragraph::new(Line::from(Span::styled(
    greeting_text,
    Style::default().fg(theme.text),
  )))
  .alignment(Alignment::Right);
  f.render_widget(greeting, row1[1]);

  // Row 2: a single horizontal rule across the full width.
  let rule = Block::default()
    .borders(Borders::TOP)
    .border_style(Style::default().fg(theme.inactive));
  f.render_widget(rule, chunks[1]);
}

pub fn draw_main_layout(f: &mut Frame, app: &App)
{
  let margin = util::get_main_layout_margin(app);
  // Responsive layout: new one kicks in at width 150 or higher
  if app.size.width >= SMALL_TERMINAL_WIDTH && !app.user_config.behavior.enforce_wide_search_bar {
    let parent_layout = Layout::default()
      .direction(Direction::Vertical)
      .constraints(
        [
          Constraint::Length(2),
          Constraint::Min(1),
          Constraint::Length(6),
        ]
        .as_ref(),
      )
      .margin(margin)
      .split(f.area());

    // Top bar (Music | Podcasts + greeting)
    draw_top_bar(f, app, parent_layout[0]);

    // Nested main block with potential routes
    draw_routes(f, app, parent_layout[1]);

    // Currently playing
    draw_playbar(f, app, parent_layout[2]);
  } else {
    let parent_layout = Layout::default()
      .direction(Direction::Vertical)
      .constraints(
        [
          Constraint::Length(3),
          Constraint::Min(1),
          Constraint::Length(6),
        ]
        .as_ref(),
      )
      .margin(margin)
      .split(f.area());

    // Search input and help
    draw_input_and_help_box(f, app, parent_layout[0]);

    // Nested main block with potential routes
    draw_routes(f, app, parent_layout[1]);

    // Currently playing
    draw_playbar(f, app, parent_layout[2]);
  }

  // Possibly draw confirm dialog
  draw_dialog(f, app);
  draw_playlist_picker(f, app);
}

pub fn draw_routes(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let chunks = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([Constraint::Percentage(20), Constraint::Percentage(80)].as_ref())
    .split(layout_chunk);

  draw_user_block(f, app, chunks[0]);

  let current_route = app.get_current_route();

  let (content_area, lyrics_area) = if app.lyrics_visible {
    let split = Layout::default()
      .direction(Direction::Horizontal)
      .constraints([Constraint::Percentage(65), Constraint::Percentage(35)].as_ref())
      .split(chunks[1]);
    (split[0], Some(split[1]))
  } else {
    (chunks[1], None)
  };

  match current_route.id {
    RouteId::Search => {
      draw_search_results(f, app, content_area);
    }
    RouteId::TrackTable => {
      draw_song_table(f, app, content_area);
    }
    RouteId::AlbumTracks => {
      draw_album_table(f, app, content_area);
    }
    RouteId::RecentlyPlayed => {
      draw_recently_played_table(f, app, content_area);
    }
    RouteId::Artist => {
      draw_artist_albums(f, app, content_area);
    }
    RouteId::AlbumList => {
      draw_album_list(f, app, content_area);
    }
    RouteId::PodcastEpisodes => {
      draw_show_episodes(f, app, content_area);
    }
    RouteId::Home => {
      draw_home(f, app, content_area);
    }
    RouteId::MadeForYou => {
      draw_made_for_you(f, app, content_area);
    }
    RouteId::Artists => {
      draw_artist_table(f, app, content_area);
    }
    RouteId::Podcasts => {
      draw_podcast_table(f, app, content_area);
    }
    RouteId::Queue => {
      draw_queue(f, app, content_area);
    }
    RouteId::Recommendations => {
      draw_recommendations_table(f, app, content_area);
    }
    RouteId::SelectedDevice => {} // This is handled as a "full screen" route in main.rs
    RouteId::Analysis => {} // This is handled as a "full screen" route in main.rs
    RouteId::BasicView => {} // This is handled as a "full screen" route in main.rs
    RouteId::Dialog => {} // This is handled in the draw_dialog function in mod.rs
  };

  if let Some(area) = lyrics_area {
    draw_lyrics_panel(f, app, area);
  }
}

pub fn draw_library_block(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let current_route = app.get_current_route();
  let highlight_state = (
    current_route.active_block == ActiveBlock::Library,
    current_route.hovered_block == ActiveBlock::Library,
  );
  draw_selectable_list(
    f,
    app,
    layout_chunk,
    "Library",
    &LIBRARY_OPTIONS,
    highlight_state,
    Some(app.library.selected_index),
  );
}

pub fn draw_playlist_block(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let current_route = app.get_current_route();
  let highlight_state = (
    current_route.active_block == ActiveBlock::MyPlaylists,
    current_route.hovered_block == ActiveBlock::MyPlaylists,
  );

  let playlist_items: Vec<String> = match &app.playlists {
    Some(p) => p.items.iter().map(|item| item.name.to_owned()).collect(),
    None => vec![],
  };

  draw_selectable_list(
    f,
    app,
    layout_chunk,
    "Playlists",
    &playlist_items,
    highlight_state,
    app.selected_playlist_index,
  );
}

/// Rows for the sidebar's Devices pane.
///
/// `●` marks the device playback is *actually* on, taken from the polled
/// playback context rather than the `is_active` flags baked into the cached
/// device list: that list is only re-fetched on `d`, so it used to keep the dot
/// on the previous device after switching, while the playbar showed the new one.
///
/// The marker leads the row rather than trailing it — the sidebar is ~26 columns
/// and "Sandesh Grangdan MacBook Pro" alone overflows that, so a trailing dot
/// was clipped away on exactly the devices whose names are long.
fn device_rows(app: &App) -> Vec<String> {
  let active_id = app
    .current_playback_context
    .as_ref()
    .and_then(|context| context.device.id.as_deref());

  match app.devices.as_ref() {
    None => vec!["Press d to load…".to_owned()],
    Some(payload) if payload.devices.is_empty() => vec!["No devices found".to_owned()],
    Some(payload) => payload
      .devices
      .iter()
      .map(|device| {
        let is_active = match active_id {
          Some(active) => device.id.as_deref() == Some(active),
          // Nothing is playing, so there is no context to trust — fall back to
          // whatever the device list itself claimed.
          None => device.is_active,
        };
        format!("{} {}", if is_active { "●" } else { " " }, device.name)
      })
      .collect(),
  }
}

pub fn draw_devices_block(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let current_route = app.get_current_route();
  let highlight_state = (
    current_route.active_block == ActiveBlock::Devices,
    current_route.hovered_block == ActiveBlock::Devices,
  );

  let device_items = device_rows(app);

  draw_selectable_list(
    f,
    app,
    layout_chunk,
    "Devices",
    &device_items,
    highlight_state,
    app.selected_device_index.or(Some(0)),
  );
}

pub fn draw_user_block(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  // Check for width to make a responsive layout
  if app.size.width >= SMALL_TERMINAL_WIDTH && !app.user_config.behavior.enforce_wide_search_bar {
    let chunks = Layout::default()
      .direction(Direction::Vertical)
      .constraints(
        [
          Constraint::Length(3),
          Constraint::Percentage(28),
          Constraint::Percentage(45),
          Constraint::Percentage(27),
        ]
        .as_ref(),
      )
      .split(layout_chunk);

    draw_input_and_help_box(f, app, chunks[0]);
    draw_library_block(f, app, chunks[1]);
    draw_playlist_block(f, app, chunks[2]);
    draw_devices_block(f, app, chunks[3]);
  } else {
    let chunks = Layout::default()
      .direction(Direction::Vertical)
      .constraints(
        [
          Constraint::Percentage(28),
          Constraint::Percentage(45),
          Constraint::Percentage(27),
        ]
        .as_ref(),
      )
      .split(layout_chunk);

    draw_library_block(f, app, chunks[0]);
    draw_playlist_block(f, app, chunks[1]);
    draw_devices_block(f, app, chunks[2]);
  }
}

pub fn draw_search_results(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints(
      [
        Constraint::Percentage(35),
        Constraint::Percentage(35),
        Constraint::Percentage(25),
      ]
      .as_ref(),
    )
    .split(layout_chunk);

  {
    let song_artist_block = Layout::default()
      .direction(Direction::Horizontal)
      .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
      .split(chunks[0]);

    let currently_playing_id = app
      .current_playback_context
      .clone()
      .and_then(|context| {
        context.item.and_then(|item| match item {
          PlayableItem::Track(track) => track.id.as_ref().map(|i| i.id().to_string()),
          PlayableItem::Episode(episode) => Some(episode.id.id().to_string()),
          PlayableItem::Unknown(_) => None,
        })
      })
      .unwrap_or_default();

    let songs = match &app.search_results.tracks {
      Some(tracks) => tracks
        .items
        .iter()
        .map(|item| {
          let mut song_name = "".to_string();
          let id = item.id.as_ref().map(|i| i.id().to_string()).unwrap_or_default();
          if currently_playing_id == id {
            song_name += "▶ "
          }
          if app.liked_song_ids_set.contains(id.as_str()) {
            song_name += &app.user_config.padded_liked_icon();
          }

          song_name += &item.name;
          song_name += &format!(" - {}", &create_artist_string(&item.artists));
          song_name
        })
        .collect(),
      None => vec![],
    };

    draw_selectable_list(
      f,
      app,
      song_artist_block[0],
      "Songs",
      &songs,
      get_search_results_highlight_state(app, SearchResultBlock::SongSearch),
      app.search_results.selected_tracks_index,
    );

    let artists = match &app.search_results.artists {
      Some(artists) => artists
        .items
        .iter()
        .map(|item| {
          let mut artist = String::new();
          if app.followed_artist_ids_set.contains(item.id.id()) {
            artist.push_str(&app.user_config.padded_liked_icon());
          }
          artist.push_str(&item.name.to_owned());
          artist
        })
        .collect(),
      None => vec![],
    };

    draw_selectable_list(
      f,
      app,
      song_artist_block[1],
      "Artists",
      &artists,
      get_search_results_highlight_state(app, SearchResultBlock::ArtistSearch),
      app.search_results.selected_artists_index,
    );
  }

  {
    let albums_playlist_block = Layout::default()
      .direction(Direction::Horizontal)
      .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
      .split(chunks[1]);

    let albums = match &app.search_results.albums {
      Some(albums) => albums
        .items
        .iter()
        .map(|item| {
          let mut album_artist = String::new();
          if let Some(ref album_id) = item.id {
            if app.saved_album_ids_set.contains(album_id.id()) {
              album_artist.push_str(&app.user_config.padded_liked_icon());
            }
          }
          album_artist.push_str(&format!(
            "{} - {} ({})",
            item.name.to_owned(),
            create_artist_string(&item.artists),
            item.album_type.as_deref().unwrap_or("unknown")
          ));
          album_artist
        })
        .collect(),
      None => vec![],
    };

    draw_selectable_list(
      f,
      app,
      albums_playlist_block[0],
      "Albums",
      &albums,
      get_search_results_highlight_state(app, SearchResultBlock::AlbumSearch),
      app.search_results.selected_album_index,
    );

    let playlists = match &app.search_results.playlists {
      Some(playlists) => playlists
        .items
        .iter()
        .map(|item| item.name.to_owned())
        .collect(),
      None => vec![],
    };
    draw_selectable_list(
      f,
      app,
      albums_playlist_block[1],
      "Playlists",
      &playlists,
      get_search_results_highlight_state(app, SearchResultBlock::PlaylistSearch),
      app.search_results.selected_playlists_index,
    );
  }

  {
    let podcasts_block = Layout::default()
      .direction(Direction::Horizontal)
      .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
      .split(chunks[2]);

    let podcasts = match &app.search_results.shows {
      Some(podcasts) => podcasts
        .items
        .iter()
        .map(|item| {
          let mut show_name = String::new();
          if app.saved_show_ids_set.contains(item.id.id()) {
            show_name.push_str(&app.user_config.padded_liked_icon());
          }
          show_name.push_str(&format!("{:} - {}", item.name, item.publisher));
          show_name
        })
        .collect(),
      None => vec![],
    };
    draw_selectable_list(
      f,
      app,
      podcasts_block[0],
      "Podcasts",
      &podcasts,
      get_search_results_highlight_state(app, SearchResultBlock::ShowSearch),
      app.search_results.selected_shows_index,
    );

    let episodes = match &app.search_results.episodes {
      Some(episodes) => episodes
        .items
        .iter()
        .map(|item| format!("{} ({})", item.name, item.release_date))
        .collect(),
      None => vec![],
    };
    draw_selectable_list(
      f,
      app,
      podcasts_block[1],
      "Episodes",
      &episodes,
      get_search_results_highlight_state(app, SearchResultBlock::EpisodeSearch),
      app.search_results.selected_episodes_index,
    );
  }
}

struct AlbumUi {
  selected_index: usize,
  items: Vec<TableItem>,
  title: String,
}

pub fn draw_artist_table(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let header = TableHeader {
    id: TableId::Artist,
    items: vec![TableHeaderItem {
      text: "Artist",
      width: get_percentage_width(layout_chunk.width, 1.0),
      ..Default::default()
    }],
  };

  let current_route = app.get_current_route();
  let highlight_state = (
    current_route.active_block == ActiveBlock::Artists,
    current_route.hovered_block == ActiveBlock::Artists,
  );
  let items = app
    .artists
    .iter()
    .map(|item| TableItem {
      id: item.id.id().to_string(),
      format: vec![item.name.to_owned()],
    })
    .collect::<Vec<TableItem>>();

  draw_table(
    f,
    app,
    layout_chunk,
    ("Artists", &header),
    &items,
    app.artists_list_index,
    highlight_state,
  )
}

pub fn draw_podcast_table(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let header = TableHeader {
    id: TableId::Podcast,
    items: vec![
      TableHeaderItem {
        text: "Name",
        width: get_percentage_width(layout_chunk.width, 2.0 / 5.0),
        ..Default::default()
      },
      TableHeaderItem {
        text: "Publisher(s)",
        width: get_percentage_width(layout_chunk.width, 2.0 / 5.0),
        ..Default::default()
      },
    ],
  };

  let current_route = app.get_current_route();

  let highlight_state = (
    current_route.active_block == ActiveBlock::Podcasts,
    current_route.hovered_block == ActiveBlock::Podcasts,
  );

  if let Some(saved_shows) = app.library.saved_shows.get_results(None) {
    let items = saved_shows
      .items
      .iter()
      .map(|show_page| TableItem {
        id: show_page.show.id.id().to_string(),
        format: vec![
          show_page.show.name.to_owned(),
          show_page.show.publisher.to_owned(),
        ],
      })
      .collect::<Vec<TableItem>>();

    draw_table(
      f,
      app,
      layout_chunk,
      ("Podcasts", &header),
      &items,
      app.shows_list_index,
      highlight_state,
    )
  };
}

pub fn draw_queue(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let theme = app.user_config.theme;
  let current_route = app.get_current_route();
  let highlight_state = (
    current_route.active_block == ActiveBlock::Queue,
    current_route.hovered_block == ActiveBlock::Queue,
  );

  // Vertical: list (Min) + 1-row hint footer at the bottom.
  let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([Constraint::Min(1), Constraint::Length(1)].as_ref())
    .split(layout_chunk);

  // Build the row strings from app.queue.
  let rows: Vec<String> = match &app.queue {
    None => vec!["Loading queue…".to_owned()],
    Some(payload) if payload.queue.is_empty() => {
      vec!["Queue is empty — add tracks with z.".to_owned()]
    }
    Some(payload) => {
      let mut out: Vec<String> = Vec::with_capacity(payload.queue.len() + 1);
      if let Some(playing) = &payload.currently_playing {
        out.push(format!("▶ {}", queue_row_text(playing)));
      }
      for item in &payload.queue {
        out.push(format!("  {}", queue_row_text(item)));
      }
      out
    }
  };

  // Clamp the selection one more time at render (defence in depth).
  let len = rows.len();
  let selected_index = if len == 0 {
    0
  } else {
    app.queue_selected_index.min(len.saturating_sub(1))
  };

  let mut state = ListState::default();
  state.select(Some(selected_index));

  let lst_items: Vec<ListItem> = rows
    .iter()
    .map(|s| ListItem::new(Span::raw(s.as_str())))
    .collect();

  let list = List::new(lst_items)
    .block(
      Block::default()
        .title(Span::styled(
          "Queue",
          get_color(highlight_state, theme),
        ))
        .borders(Borders::ALL)
        .border_type(get_border_type(highlight_state))
        .border_style(get_color(highlight_state, theme)),
    )
    .style(Style::default().fg(theme.text))
    .highlight_style(get_row_highlight_style(highlight_state, theme));
  f.render_stateful_widget(list, chunks[0], &mut state);

  let hint = Paragraph::new(Line::from(Span::styled(
    "j/k move · x pop next · Enter skip-to-here · q back",
    Style::default().fg(theme.inactive),
  )));
  f.render_widget(hint, chunks[1]);
}

fn queue_row_text(item: &PlayableItem) -> String {
  match item {
    PlayableItem::Track(track) => {
      format!("{} — {}", track.name, create_artist_string(&track.artists))
    }
    PlayableItem::Episode(episode) => {
      format!("{} — {}", episode.name, episode.show.name)
    }
    PlayableItem::Unknown(_) => "(unknown item)".to_owned(),
  }
}

pub fn draw_lyrics_panel(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let theme = app.user_config.theme;
  let block = Block::default()
    .title(Span::styled(
      "Lyrics",
      Style::default().fg(theme.banner).add_modifier(Modifier::BOLD),
    ))
    .borders(Borders::ALL)
    .border_type(BorderType::Plain)
    .border_style(Style::default().fg(theme.inactive));
  let inner = block.inner(layout_chunk);
  f.render_widget(block, layout_chunk);

  let placeholder = lyrics_placeholder(app);
  if let Some(text) = placeholder {
    let p = Paragraph::new(Line::from(Span::styled(
      text,
      Style::default().fg(theme.inactive),
    )));
    f.render_widget(p, inner);
    return;
  }

  let lyrics = match &app.lyrics {
    Some(l) => l,
    None => return,
  };

  if !lyrics.synced.is_empty() {
    let progress_ms = app.song_progress_ms as u32;
    let current = lyrics
      .synced
      .iter()
      .rposition(|(t, _)| *t <= progress_ms)
      .unwrap_or(0);

    let visible_rows = inner.height as usize;
    if visible_rows == 0 {
      return;
    }
    let total = lyrics.synced.len();
    let half = visible_rows / 2;
    let max_offset = total.saturating_sub(visible_rows);
    let offset = current.saturating_sub(half).min(max_offset);

    let items: Vec<ListItem> = lyrics
      .synced
      .iter()
      .enumerate()
      .map(|(i, (_, line))| {
        let style = if i == current {
          Style::default()
            .fg(theme.selected)
            .add_modifier(Modifier::BOLD)
        } else {
          Style::default().fg(theme.playbar_text)
        };
        ListItem::new(Span::styled(line.clone(), style))
      })
      .collect();

    let mut state = ListState::default();
    *state.offset_mut() = offset;
    state.select(Some(current));
    let list = List::new(items);
    f.render_stateful_widget(list, inner, &mut state);
    return;
  }

  if let Some(plain) = &lyrics.plain {
    let lines: Vec<Line> = plain
      .lines()
      .map(|s| Line::from(Span::styled(s.to_string(), Style::default().fg(theme.playbar_text))))
      .collect();
    let p = Paragraph::new(lines);
    f.render_widget(p, inner);
  }
}

fn lyrics_placeholder(app: &App) -> Option<&'static str> {
  if app.lyrics_loading && app.lyrics.is_none() {
    return Some("✓ Lyrics panel ON — fetching from lrclib.net…");
  }
  let context = match &app.current_playback_context {
    Some(c) => c,
    None => return Some("✓ Lyrics panel ON — start a track to see lyrics"),
  };
  let item = match &context.item {
    Some(i) => i,
    None => return Some("✓ Lyrics panel ON — start a track to see lyrics"),
  };
  match item {
    PlayableItem::Episode(_) => {
      return Some("✓ Lyrics panel ON — podcasts don't have lyrics")
    }
    PlayableItem::Unknown(_) => {
      return Some("✓ Lyrics panel ON — track type unknown, no lyrics")
    }
    PlayableItem::Track(_) => {}
  }
  match &app.lyrics {
    None => Some("✓ Lyrics panel ON — lrclib has no match for this track"),
    Some(l) if l.synced.is_empty() && l.plain.is_none() => {
      Some("✓ Lyrics panel ON — lrclib has no match for this track")
    }
    Some(_) => None,
  }
}

pub fn draw_album_table(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let header = TableHeader {
    id: TableId::Album,
    items: vec![
      TableHeaderItem {
        id: ColumnId::Liked,
        text: "",
        width: 2,
      },
      TableHeaderItem {
        text: "#",
        width: 3,
        ..Default::default()
      },
      TableHeaderItem {
        id: ColumnId::Title,
        text: "Title",
        width: get_percentage_width(layout_chunk.width, 2.0 / 5.0) - 5,
      },
      TableHeaderItem {
        text: "Artist",
        width: get_percentage_width(layout_chunk.width, 2.0 / 5.0),
        ..Default::default()
      },
      TableHeaderItem {
        text: "Length",
        width: get_percentage_width(layout_chunk.width, 1.0 / 5.0),
        ..Default::default()
      },
    ],
  };

  let current_route = app.get_current_route();
  let highlight_state = (
    current_route.active_block == ActiveBlock::AlbumTracks,
    current_route.hovered_block == ActiveBlock::AlbumTracks,
  );

  let album_ui = match &app.album_table_context {
    AlbumTableContext::Simplified => {
      app
        .selected_album_simplified
        .as_ref()
        .map(|selected_album_simplified| AlbumUi {
          items: selected_album_simplified
            .tracks
            .items
            .iter()
            .map(|item| TableItem {
              id: item.id.as_ref().map(|i| i.id().to_string()).unwrap_or_default(),
              format: vec![
                "".to_string(),
                item.track_number.to_string(),
                item.name.to_owned(),
                create_artist_string(&item.artists),
                millis_to_minutes(item.duration.num_milliseconds() as u128),
              ],
            })
            .collect::<Vec<TableItem>>(),
          title: format!(
            "{} by {}",
            selected_album_simplified.album.name,
            create_artist_string(&selected_album_simplified.album.artists)
          ),
          selected_index: selected_album_simplified.selected_index,
        })
    }
    AlbumTableContext::Full => match app.selected_album_full.clone() {
      Some(selected_album) => Some(AlbumUi {
        items: selected_album
          .album
          .tracks
          .items
          .iter()
          .map(|item| TableItem {
            id: item.id.as_ref().map(|i| i.id().to_string()).unwrap_or_default(),
            format: vec![
              "".to_string(),
              item.track_number.to_string(),
              item.name.to_owned(),
              create_artist_string(&item.artists),
              millis_to_minutes(item.duration.num_milliseconds() as u128),
            ],
          })
          .collect::<Vec<TableItem>>(),
        title: format!(
          "{} by {}",
          selected_album.album.name,
          create_artist_string(&selected_album.album.artists)
        ),
        selected_index: app.saved_album_tracks_index,
      }),
      None => None,
    },
  };

  if let Some(album_ui) = album_ui {
    draw_table(
      f,
      app,
      layout_chunk,
      (&album_ui.title, &header),
      &album_ui.items,
      album_ui.selected_index,
      highlight_state,
    );
  };
}

pub fn draw_recommendations_table(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let header = TableHeader {
    id: TableId::Song,
    items: vec![
      TableHeaderItem {
        id: ColumnId::Liked,
        text: "",
        width: 2,
      },
      TableHeaderItem {
        id: ColumnId::Title,
        text: "Title",
        width: get_percentage_width(layout_chunk.width, 0.3),
      },
      TableHeaderItem {
        text: "Artist",
        width: get_percentage_width(layout_chunk.width, 0.3),
        ..Default::default()
      },
      TableHeaderItem {
        text: "Album",
        width: get_percentage_width(layout_chunk.width, 0.3),
        ..Default::default()
      },
      TableHeaderItem {
        text: "Length",
        width: get_percentage_width(layout_chunk.width, 0.1),
        ..Default::default()
      },
    ],
  };

  let current_route = app.get_current_route();
  let highlight_state = (
    current_route.active_block == ActiveBlock::TrackTable,
    current_route.hovered_block == ActiveBlock::TrackTable,
  );

  let items = app
    .track_table
    .tracks
    .iter()
    .map(|item| TableItem {
      id: item.id.as_ref().map(|i| i.id().to_string()).unwrap_or_default(),
      format: vec![
        "".to_string(),
        item.name.to_owned(),
        create_artist_string(&item.artists),
        item.album.name.to_owned(),
        millis_to_minutes(item.duration.num_milliseconds() as u128),
      ],
    })
    .collect::<Vec<TableItem>>();
  // match RecommendedContext
  let recommendations_ui = match &app.recommendations_context {
    Some(RecommendationsContext::Song) => format!(
      "Recommendations based on Song \'{}\'",
      &app.recommendations_seed
    ),
    Some(RecommendationsContext::Artist) => format!(
      "Recommendations based on Artist \'{}\'",
      &app.recommendations_seed
    ),
    // A mix's name already says what it is.
    Some(RecommendationsContext::Mix) => app.recommendations_seed.clone(),
    None => "Recommendations".to_string(),
  };
  draw_table(
    f,
    app,
    layout_chunk,
    (&recommendations_ui[..], &header),
    &items,
    app.track_table.selected_index,
    highlight_state,
  )
}

pub fn draw_song_table(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let header = TableHeader {
    id: TableId::Song,
    items: vec![
      TableHeaderItem {
        id: ColumnId::Liked,
        text: "",
        width: 2,
      },
      TableHeaderItem {
        id: ColumnId::Title,
        text: "Title",
        width: get_percentage_width(layout_chunk.width, 0.3),
      },
      TableHeaderItem {
        text: "Artist",
        width: get_percentage_width(layout_chunk.width, 0.3),
        ..Default::default()
      },
      TableHeaderItem {
        text: "Album",
        width: get_percentage_width(layout_chunk.width, 0.3),
        ..Default::default()
      },
      TableHeaderItem {
        text: "Length",
        width: get_percentage_width(layout_chunk.width, 0.1),
        ..Default::default()
      },
    ],
  };

  let current_route = app.get_current_route();
  let highlight_state = (
    current_route.active_block == ActiveBlock::TrackTable,
    current_route.hovered_block == ActiveBlock::TrackTable,
  );

  let items = app
    .track_table
    .tracks
    .iter()
    .map(|item| TableItem {
      id: item.id.as_ref().map(|i| i.id().to_string()).unwrap_or_default(),
      format: vec![
        "".to_string(),
        item.name.to_owned(),
        create_artist_string(&item.artists),
        item.album.name.to_owned(),
        millis_to_minutes(item.duration.num_milliseconds() as u128),
      ],
    })
    .collect::<Vec<TableItem>>();

  let title = match app.track_table.context {
    Some(TrackTableContext::TopTracks) => "Your Top Tracks",
    Some(TrackTableContext::SavedTracks) => "Liked Songs",
    _ => "Songs",
  };

  draw_table(
    f,
    app,
    layout_chunk,
    (title, &header),
    &items,
    app.track_table.selected_index,
    highlight_state,
  )
}

pub fn draw_basic_view(f: &mut Frame, app: &App)
{
  // If space is negative, do nothing because the widget would not fit
  if let Some(s) = app.size.height.checked_sub(BASIC_VIEW_HEIGHT) {
    let space = s / 2;
    let chunks = Layout::default()
      .direction(Direction::Vertical)
      .constraints(
        [
          Constraint::Length(space),
          Constraint::Length(BASIC_VIEW_HEIGHT),
          Constraint::Length(space),
        ]
        .as_ref(),
      )
      .split(f.area());

    draw_playbar(f, app, chunks[1]);
  }
}

pub fn draw_playbar(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let theme = app.user_config.theme;
  let behavior = &app.user_config.behavior;

  // Early return if there's nothing to render — matches today's behaviour.
  let context = match &app.current_playback_context {
    Some(c) => c,
    None => return,
  };
  let track_item = match &context.item {
    Some(i) => i,
    None => return,
  };
  if matches!(track_item, PlayableItem::Unknown(_)) {
    return;
  }

  // Outer border around the playbar (subtle, inactive color so it frames
  // without competing with focused-panel highlights elsewhere).
  let outer_block = Block::default()
    .borders(Borders::ALL)
    .border_type(BorderType::Plain)
    .border_style(Style::default().fg(theme.inactive));
  let inner_area = outer_block.inner(layout_chunk);
  f.render_widget(outer_block, layout_chunk);

  // Inside the border (inner_area is 4 rows tall after the border consumes
  // 1 row top + 1 row bottom from the 6-row playbar budget):
  //   info row (2) + spacer (1) + progress row (1) = 4
  let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints(
      [
        Constraint::Length(2),
        Constraint::Length(1),
        Constraint::Length(1),
      ]
      .as_ref(),
    )
    .horizontal_margin(1)
    .split(inner_area);

  // Info row split horizontally 35 / 35 / 30.
  let info = Layout::default()
    .direction(Direction::Horizontal)
    .constraints(
      [
        Constraint::Percentage(35),
        Constraint::Percentage(35),
        Constraint::Percentage(30),
      ]
      .as_ref(),
    )
    .split(chunks[0]);

  // ── Left: track name + artist ──
  let (item_id, name, duration_ms) = match track_item {
    PlayableItem::Track(track) => (
      track.id.as_ref().map(|i| i.id().to_string()).unwrap_or_default(),
      track.name.to_owned(),
      track.duration.num_milliseconds() as u32,
    ),
    PlayableItem::Episode(episode) => (
      episode.id.id().to_string(),
      episode.name.to_owned(),
      episode.duration.num_milliseconds() as u32,
    ),
    PlayableItem::Unknown(_) => return,
  };

  let liked_prefix = if app.liked_song_ids_set.contains(&item_id) {
    app.user_config.padded_liked_icon()
  } else {
    String::new()
  };

  let artist_text = match track_item {
    PlayableItem::Track(track) => create_artist_string(&track.artists),
    PlayableItem::Episode(episode) => format!("{} - {}", episode.name, episode.show.name),
    PlayableItem::Unknown(_) => String::new(),
  };

  let left = Paragraph::new(Text::from(vec![
    Line::from(Span::styled(
      name,
      Style::default().fg(theme.selected).add_modifier(Modifier::BOLD),
    )),
    Line::from(Span::styled(
      format!("{}{}", liked_prefix, artist_text),
      Style::default().fg(theme.playbar_text),
    )),
  ]));
  f.render_widget(left, info[0]);

  // ── Center: compact text status (Playing/Paused · Shuffle · Repeat) ──
  // No fake-button glyphs; controls happen via keybindings (space, n, p,
  // Ctrl-S, Ctrl-R — see `?` help).
  let active_text_style = Style::default().fg(theme.playbar_text);
  let inactive_text_style = Style::default().fg(theme.inactive);

  let play_state_text = if context.is_playing { "Playing" } else { "Paused" };

  let (shuffle_label, shuffle_style) = if context.shuffle_state {
    ("Shuffle: On", active_text_style)
  } else {
    ("Shuffle: Off", inactive_text_style)
  };

  let (repeat_label, repeat_style) = match context.repeat_state {
    RepeatState::Track => ("Repeat: Track", active_text_style),
    RepeatState::Context => ("Repeat: All", active_text_style),
    RepeatState::Off => ("Repeat: Off", inactive_text_style),
  };

  let center = Paragraph::new(Line::from(vec![
    Span::styled(
      play_state_text,
      active_text_style.add_modifier(Modifier::BOLD),
    ),
    Span::styled("  ·  ", inactive_text_style),
    Span::styled(shuffle_label, shuffle_style),
    Span::styled("  ·  ", inactive_text_style),
    Span::styled(repeat_label, repeat_style),
  ]))
  .alignment(Alignment::Center);
  f.render_widget(center, info[1]);

  // ── Right: device · volume ──
  let volume_text = match context.device.volume_percent {
    Some(v) => format!("{}%", v),
    None => "--%".to_owned(),
  };
  let right = Paragraph::new(Line::from(Span::styled(
    format!("{} · {}", context.device.name, volume_text),
    Style::default().fg(theme.playbar_text),
  )))
  .alignment(Alignment::Right);
  f.render_widget(right, info[2]);

  // ── Original full-width Gauge (solid filled bar with centered label) ──
  let progress_ms = match app.seek_ms {
    Some(seek_ms) => seek_ms,
    None => app.song_progress_ms,
  };
  let perc = get_track_progress_percentage(progress_ms, duration_ms);
  let song_progress_label = display_track_progress(progress_ms, duration_ms);
  let modifier = if behavior.enable_text_emphasis {
    Modifier::ITALIC | Modifier::BOLD
  } else {
    Modifier::empty()
  };
  let song_progress = Gauge::default()
    .gauge_style(
      Style::default()
        .fg(theme.playbar_progress)
        .bg(theme.playbar_background)
        .add_modifier(modifier),
    )
    .percent(perc)
    .label(Span::styled(
      song_progress_label,
      Style::default()
        .fg(theme.playbar_text)
        .add_modifier(Modifier::BOLD),
    ));
  f.render_widget(song_progress, chunks[2]);
}

/// Transient error notification in the top-right corner.
///
/// Errors used to push a full-screen route that sat there until dismissed; a
/// 403 from a command that no longer applies doesn't deserve that. The toast
/// overlays whatever is on screen and clears itself after a few seconds
/// (`Toast::LIFETIME`), so nothing has to be acknowledged.
pub fn draw_toast(f: &mut Frame, app: &App) {
  let toast = match &app.toast {
    Some(toast) => toast,
    None => return,
  };
  let area = f.area();
  let text_width = 60.min(area.width.saturating_sub(6) as usize);
  if text_width < 12 || area.height < 6 {
    return;
  }

  let mut lines: Vec<Line> = util::wrap_text(&toast.message, text_width, 3)
    .into_iter()
    .map(|line| {
      Line::from(Span::styled(
        format!(" {}", line),
        Style::default().fg(app.user_config.theme.error_text),
      ))
    })
    .collect();
  if lines.is_empty() {
    return;
  }
  if let Some(hint) = toast.hint {
    lines.extend(
      util::wrap_text(hint, text_width, 2)
        .into_iter()
        .map(|line| {
          Line::from(Span::styled(
            format!(" {}", line),
            Style::default().fg(app.user_config.theme.inactive),
          ))
        }),
    );
  }

  let width = lines
    .iter()
    .map(|line| line.width())
    .max()
    .unwrap_or(0)
    .saturating_add(3) as u16;
  let height = lines.len() as u16 + 2;
  let rect = Rect {
    x: area.width.saturating_sub(width + 1),
    // One row down from the top edge, so it reads as floating above the view.
    y: if area.height > height + 1 { 1 } else { 0 },
    width: width.min(area.width),
    height: height.min(area.height),
  };

  f.render_widget(Clear, rect);
  f.render_widget(
    Paragraph::new(Text::from(lines)).block(
      Block::default()
        .title(Span::styled(
          " Error ",
          Style::default()
            .fg(app.user_config.theme.error_border)
            .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.user_config.theme.error_border)),
    ),
    rect,
  );
}

fn draw_home(f: &mut Frame, app: &App, layout_chunk: Rect) {
  let sections = home_sections::sections(app);
  let selected_section = sections
    .iter()
    .position(|section| section.block == app.home_selected_block)
    .unwrap_or(0);

  // 4 rows: a border plus the greeting's two lines.
  let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([Constraint::Length(4), Constraint::Min(0)].as_ref())
    .split(layout_chunk);

  draw_home_greeting(f, app, chunks[0], selected_section, sections.len());

  if sections.is_empty() || chunks[1].height == 0 {
    return;
  }
  let rows = Layout::default()
    .direction(Direction::Vertical)
    .constraints(
      sections
        .iter()
        .map(|_| Constraint::Ratio(1, sections.len() as u32))
        .collect::<Vec<Constraint>>(),
    )
    .split(chunks[1]);

  for (index, section) in sections.iter().enumerate() {
    let items: Vec<String> = section
      .items
      .iter()
      .map(|item| {
        if item.subtitle.is_empty() {
          item.title.clone()
        } else {
          format!("{}  ·  {}", item.title, item.subtitle)
        }
      })
      .collect();
    let selected = home_sections::item_index(app, section.block).min(items.len().saturating_sub(1));
    draw_selectable_list(
      f,
      app,
      rows[index],
      &section.title,
      &items,
      get_home_highlight_state(app, section.block),
      (!items.is_empty()).then_some(selected),
    );
  }
}

fn draw_home_greeting(
  f: &mut Frame,
  app: &App,
  layout_chunk: Rect,
  selected_section: usize,
  section_count: usize,
) {
  let display_name = app
    .user
    .as_ref()
    .and_then(|u| u.display_name.clone())
    .unwrap_or_else(|| "there".to_owned());
  let other_mode = match app.home_mode {
    HomeMode::Music => "podcasts",
    HomeMode::Podcast => "music",
  };
  let hint = if app.home_section_entered {
    format!(
      "j/k move · Enter plays · q/Esc back to sections · {} for {} · ? help",
      app.user_config.keys.toggle_home_mode, other_mode
    )
  } else {
    format!(
      "j/k pick a section ({}/{}) · Enter opens it · {} for {} · ? help",
      selected_section + 1,
      section_count,
      app.user_config.keys.toggle_home_mode,
      other_mode
    )
  };

  let greeting = Paragraph::new(Text::from(vec![
    Line::from(Span::styled(
      format!(
        "Hello, {} — what do you want to listen to today?",
        display_name
      ),
      Style::default()
        .fg(app.user_config.theme.banner)
        .add_modifier(Modifier::BOLD),
    )),
    Line::from(Span::styled(
      hint,
      Style::default().fg(app.user_config.theme.inactive),
    )),
  ]))
  .style(Style::default().fg(app.user_config.theme.text))
  .block(
    Block::default()
      .borders(Borders::ALL)
      .border_type(BorderType::Rounded)
      .border_style(Style::default().fg(app.user_config.theme.banner)),
  );
  f.render_widget(greeting, layout_chunk);
}

fn draw_artist_albums(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let chunks = Layout::default()
    .direction(Direction::Horizontal)
    .constraints(
      [
        Constraint::Percentage(33),
        Constraint::Percentage(33),
        Constraint::Percentage(33),
      ]
      .as_ref(),
    )
    .split(layout_chunk);

  if let Some(artist) = &app.artist {
    let top_tracks = artist
      .top_tracks
      .iter()
      .map(|top_track| {
        let mut name = String::new();
        if let Some(context) = &app.current_playback_context {
          let playing_id: Option<String> = match &context.item {
            Some(PlayableItem::Track(track)) => track.id.as_ref().map(|i| i.id().to_string()),
            _ => None,
          };
          let top_track_id = top_track.id.as_ref().map(|i| i.id().to_string());

          if playing_id.is_some() && playing_id == top_track_id {
            name.push_str("▶ ");
          }
        };
        name.push_str(&top_track.name);
        name
      })
      .collect::<Vec<String>>();

    draw_selectable_list(
      f,
      app,
      chunks[0],
      &format!("{} - Top Tracks", &artist.artist_name),
      &top_tracks,
      get_artist_highlight_state(app, ArtistBlock::TopTracks),
      Some(artist.selected_top_track_index),
    );

    let albums = &artist
      .albums
      .items
      .iter()
      .map(|item| {
        let mut album_artist = String::new();
        if let Some(ref album_id) = item.id {
          if app.saved_album_ids_set.contains(album_id.id()) {
            album_artist.push_str(&app.user_config.padded_liked_icon());
          }
        }
        album_artist.push_str(&format!(
          "{} - {} ({})",
          item.name.to_owned(),
          create_artist_string(&item.artists),
          item.album_type.as_deref().unwrap_or("unknown")
        ));
        album_artist
      })
      .collect::<Vec<String>>();

    draw_selectable_list(
      f,
      app,
      chunks[1],
      "Albums",
      albums,
      get_artist_highlight_state(app, ArtistBlock::Albums),
      Some(artist.selected_album_index),
    );

    let related_artists = artist
      .related_artists
      .iter()
      .map(|item| {
        let mut artist = String::new();
        if app.followed_artist_ids_set.contains(item.id.id()) {
          artist.push_str(&app.user_config.padded_liked_icon());
        }
        artist.push_str(&item.name.to_owned());
        artist
      })
      .collect::<Vec<String>>();

    draw_selectable_list(
      f,
      app,
      chunks[2],
      "Related artists",
      &related_artists,
      get_artist_highlight_state(app, ArtistBlock::RelatedArtists),
      Some(artist.selected_related_artist_index),
    );
  };
}

pub fn draw_device_list(f: &mut Frame, app: &App)
{
  let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([Constraint::Percentage(20), Constraint::Percentage(80)].as_ref())
    .margin(5)
    .split(f.area());

  let device_instructions: Vec<Line> = vec![
        "To play tracks, please select a device. ",
        "Use `j/k` or up/down arrow keys to move up and down and <Enter> to select. ",
        "Your choice here will be cached so you can jump straight back in when you next open `spotify-tui`. ",
        "You can change the playback device at any time by pressing `d`.",
    ].into_iter().map(|instruction| Line::from(Span::raw(instruction))).collect();

  let instructions = Paragraph::new(device_instructions)
    .style(Style::default().fg(app.user_config.theme.text))
    .wrap(Wrap { trim: true })
    .block(
      Block::default().borders(Borders::NONE).title(Span::styled(
        "Welcome to spotify-tui!",
        Style::default()
          .fg(app.user_config.theme.active)
          .add_modifier(Modifier::BOLD),
      )),
    );
  f.render_widget(instructions, chunks[0]);

  let no_device_message = Span::raw("No devices found: Make sure a device is active");

  let items = match &app.devices {
    Some(items) => {
      if items.devices.is_empty() {
        vec![ListItem::new(no_device_message)]
      } else {
        items
          .devices
          .iter()
          .map(|device| ListItem::new(Span::raw(&device.name)))
          .collect()
      }
    }
    None => vec![ListItem::new(no_device_message)],
  };

  let mut state = ListState::default();
  state.select(app.selected_device_index);
  let list = List::new(items)
    .block(
      Block::default()
        .title(Span::styled(
          "Devices",
          Style::default()
            .fg(app.user_config.theme.active)
            .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.user_config.theme.active)),
    )
    .style(Style::default().fg(app.user_config.theme.text))
    .highlight_style(
      Style::default()
        .bg(app.user_config.theme.selected)
        .fg(app.user_config.theme.playbar_background)
        .add_modifier(Modifier::BOLD),
    );
  f.render_stateful_widget(list, chunks[1], &mut state);
}

pub fn draw_album_list(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let header = TableHeader {
    id: TableId::AlbumList,
    items: vec![
      TableHeaderItem {
        text: "Name",
        width: get_percentage_width(layout_chunk.width, 2.0 / 5.0),
        ..Default::default()
      },
      TableHeaderItem {
        text: "Artists",
        width: get_percentage_width(layout_chunk.width, 2.0 / 5.0),
        ..Default::default()
      },
      TableHeaderItem {
        text: "Release Date",
        width: get_percentage_width(layout_chunk.width, 1.0 / 5.0),
        ..Default::default()
      },
    ],
  };

  let current_route = app.get_current_route();

  let highlight_state = (
    current_route.active_block == ActiveBlock::AlbumList,
    current_route.hovered_block == ActiveBlock::AlbumList,
  );

  let selected_song_index = app.album_list_index;

  match app.album_list_context {
    AlbumListContext::SavedAlbums => {
      if let Some(saved_albums) = app.library.saved_albums.get_results(None) {
        let items = saved_albums
          .items
          .iter()
          .map(|album_page| TableItem {
            id: album_page.album.id.id().to_string(),
            format: vec![
              format!(
                "{}{}",
                app.user_config.padded_liked_icon(),
                &album_page.album.name
              ),
              create_artist_string(&album_page.album.artists),
              album_page.album.release_date.to_owned(),
            ],
          })
          .collect::<Vec<TableItem>>();

        draw_table(
          f,
          app,
          layout_chunk,
          ("Saved Albums", &header),
          &items,
          selected_song_index,
          highlight_state,
        )
      };
    }
    AlbumListContext::NewReleases => {
      if let Some(page) = app.library.new_releases.get_results(None) {
        let items = page
          .items
          .iter()
          .map(|album| TableItem {
            id: album
              .id
              .as_ref()
              .map(|id| id.id().to_string())
              .unwrap_or_default(),
            format: vec![
              format!(
                "{}{}",
                if album
                  .id
                  .as_ref()
                  .map(|id| app.saved_album_ids_set.contains(&id.id().to_string()))
                  .unwrap_or(false)
                {
                  app.user_config.padded_liked_icon()
                } else {
                  String::new()
                },
                &album.name
              ),
              create_artist_string(&album.artists),
              album.release_date.clone().unwrap_or_default(),
            ],
          })
          .collect::<Vec<TableItem>>();

        draw_table(
          f,
          app,
          layout_chunk,
          ("New Releases (press w to save an album)", &header),
          &items,
          selected_song_index,
          highlight_state,
        )
      };
    }
  }
}

pub fn draw_show_episodes(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let header = TableHeader {
    id: TableId::PodcastEpisodes,
    items: vec![
      TableHeaderItem {
        // Column to mark an episode as fully played
        text: "",
        width: 2,
        ..Default::default()
      },
      TableHeaderItem {
        text: "Date",
        width: get_percentage_width(layout_chunk.width, 0.5 / 5.0) - 2,
        ..Default::default()
      },
      TableHeaderItem {
        text: "Name",
        width: get_percentage_width(layout_chunk.width, 3.5 / 5.0),
        id: ColumnId::Title,
      },
      TableHeaderItem {
        text: "Duration",
        width: get_percentage_width(layout_chunk.width, 1.0 / 5.0),
        ..Default::default()
      },
    ],
  };

  let current_route = app.get_current_route();

  let highlight_state = (
    current_route.active_block == ActiveBlock::EpisodeTable,
    current_route.hovered_block == ActiveBlock::EpisodeTable,
  );

  if let Some(episodes) = app.library.show_episodes.get_results(None) {
    let items = episodes
      .items
      .iter()
      .map(|episode| {
        let (played_str, time_str) = match episode.resume_point {
          Some(ResumePoint {
            fully_played,
            resume_position,
          }) => (
            if fully_played {
              " ✔".to_owned()
            } else {
              "".to_owned()
            },
            format!(
              "{} / {}",
              millis_to_minutes(resume_position.num_milliseconds() as u128),
              millis_to_minutes(episode.duration.num_milliseconds() as u128)
            ),
          ),
          None => (
            "".to_owned(),
            millis_to_minutes(episode.duration.num_milliseconds() as u128),
          ),
        };
        TableItem {
          id: episode.id.id().to_string(),
          format: vec![
            played_str,
            episode.release_date.to_owned(),
            episode.name.to_owned(),
            time_str,
          ],
        }
      })
      .collect::<Vec<TableItem>>();

    let title = match &app.episode_table_context {
      EpisodeTableContext::Simplified => match &app.selected_show_simplified {
        Some(selected_show) => {
          format!(
            "{} by {}",
            selected_show.show.name.to_owned(),
            selected_show.show.publisher
          )
        }
        None => "Episodes".to_owned(),
      },
      EpisodeTableContext::Full => match &app.selected_show_full {
        Some(selected_show) => {
          format!(
            "{} by {}",
            selected_show.show.name.to_owned(),
            selected_show.show.publisher
          )
        }
        None => "Episodes".to_owned(),
      },
    };

    draw_table(
      f,
      app,
      layout_chunk,
      (&title, &header),
      &items,
      app.episode_list_index,
      highlight_state,
    );
  };
}

pub fn draw_made_for_you(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let header = TableHeader {
    id: TableId::MadeForYou,
    items: vec![TableHeaderItem {
      text: "Name",
      width: get_percentage_width(layout_chunk.width, 2.0 / 5.0),
      ..Default::default()
    }],
  };

  if let Some(playlists) = &app.library.made_for_you_playlists.get_results(None) {
    let items = playlists
      .items
      .iter()
      .map(|playlist| TableItem {
        id: playlist.id.id().to_string(),
        format: vec![playlist.name.to_owned()],
      })
      .collect::<Vec<TableItem>>();

    let current_route = app.get_current_route();
    let highlight_state = (
      current_route.active_block == ActiveBlock::MadeForYou,
      current_route.hovered_block == ActiveBlock::MadeForYou,
    );

    draw_table(
      f,
      app,
      layout_chunk,
      ("Made For You", &header),
      &items,
      app.made_for_you_index,
      highlight_state,
    );
  }
}

pub fn draw_recently_played_table(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let header = TableHeader {
    id: TableId::RecentlyPlayed,
    items: vec![
      TableHeaderItem {
        id: ColumnId::Liked,
        text: "",
        width: 2,
      },
      TableHeaderItem {
        id: ColumnId::Title,
        text: "Title",
        // We need to subtract the fixed value of the previous column
        width: get_percentage_width(layout_chunk.width, 2.0 / 5.0) - 2,
      },
      TableHeaderItem {
        text: "Artist",
        width: get_percentage_width(layout_chunk.width, 2.0 / 5.0),
        ..Default::default()
      },
      TableHeaderItem {
        text: "Length",
        width: get_percentage_width(layout_chunk.width, 1.0 / 5.0),
        ..Default::default()
      },
    ],
  };

  if let Some(recently_played) = &app.recently_played.result {
    let current_route = app.get_current_route();

    let highlight_state = (
      current_route.active_block == ActiveBlock::RecentlyPlayed,
      current_route.hovered_block == ActiveBlock::RecentlyPlayed,
    );

    let selected_song_index = app.recently_played.index;

    let items = recently_played
      .items
      .iter()
      .map(|item| TableItem {
        id: item.track.id.as_ref().map(|i| i.id().to_string()).unwrap_or_default(),
        format: vec![
          "".to_string(),
          item.track.name.to_owned(),
          create_artist_string(&item.track.artists),
          millis_to_minutes(item.track.duration.num_milliseconds() as u128),
        ],
      })
      .collect::<Vec<TableItem>>();

    draw_table(
      f,
      app,
      layout_chunk,
      ("Recently Played Tracks", &header),
      &items,
      selected_song_index,
      highlight_state,
    )
  };
}

fn draw_selectable_list<S>(
  f: &mut Frame,
  app: &App,
  layout_chunk: Rect,
  title: &str,
  items: &[S],
  highlight_state: (bool, bool),
  selected_index: Option<usize>,
) where
  S: std::convert::AsRef<str>,
{
  let mut state = ListState::default();
  state.select(selected_index);

  let lst_items: Vec<ListItem> = items
    .iter()
    .map(|i| ListItem::new(Span::raw(i.as_ref())))
    .collect();

  let list = List::new(lst_items)
    .block(
      Block::default()
        .title(Span::styled(
          title,
          get_color(highlight_state, app.user_config.theme),
        ))
        .borders(Borders::ALL)
        .border_type(get_border_type(highlight_state))
        .border_style(get_color(highlight_state, app.user_config.theme)),
    )
    .style(Style::default().fg(app.user_config.theme.text))
    .highlight_style(get_row_highlight_style(
      highlight_state,
      app.user_config.theme,
    ));
  f.render_stateful_widget(list, layout_chunk, &mut state);
}

fn draw_playlist_picker(f: &mut Frame, app: &App)
{
  if app.get_current_route().active_block != ActiveBlock::PlaylistPicker {
    return;
  }
  let bounds = f.area();
  let width = std::cmp::min(bounds.width.saturating_sub(2), 50);
  let height = std::cmp::min(bounds.height.saturating_sub(2), 16);
  let left = bounds.width.saturating_sub(width) / 2;
  let top = bounds.height / 5;
  // Clamp to the frame — an unclamped rect panics ratatui's Clear on short
  // terminals.
  let rect = Rect::new(left, top, width, height).intersection(bounds);

  f.render_widget(Clear, rect);

  let names: Vec<String> = app
    .modifiable_playlists()
    .iter()
    .map(|pl| pl.name.clone())
    .collect();

  let items: Vec<ListItem> = names
    .iter()
    .map(|n| ListItem::new(Span::raw(n.as_str())))
    .collect();

  let mut state = ListState::default();
  if !names.is_empty() {
    state.select(Some(app.playlist_picker_index.min(names.len() - 1)));
  }

  let list = List::new(items)
    .block(
      Block::default()
        .title(Span::styled(
          "Add to playlist (Enter: add, Esc: cancel)",
          Style::default().fg(app.user_config.theme.active),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.user_config.theme.hovered)),
    )
    .style(Style::default().fg(app.user_config.theme.text))
    .highlight_style(
      Style::default()
        .fg(app.user_config.theme.selected)
        .add_modifier(Modifier::BOLD),
    );
  f.render_stateful_widget(list, rect, &mut state);
}

fn draw_dialog(f: &mut Frame, app: &App)
{
  if let ActiveBlock::Dialog(_) = app.get_current_route().active_block {
    if let Some(playlist) = app.dialog.as_ref() {
      let bounds = f.area();
      // maybe do this better
      let width = std::cmp::min(bounds.width - 2, 45);
      let height = 8;
      let left = (bounds.width - width) / 2;
      let top = bounds.height / 4;

      let rect = Rect::new(left, top, width, height).intersection(bounds);

      f.render_widget(Clear, rect);

      let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.user_config.theme.hovered));

      f.render_widget(block, rect);

      let vchunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([Constraint::Min(3), Constraint::Length(3)].as_ref())
        .split(rect);

      // suggestion: possibly put this as part of
      // app.dialog, but would have to introduce lifetime
      let text = vec![
        Line::from(Span::raw("Are you sure you want to delete the playlist: ")),
        Line::from(Span::styled(
          playlist.as_str(),
          Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::raw("?")),
      ];

      let text = Paragraph::new(text)
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Center);

      f.render_widget(text, vchunks[0]);

      let hchunks = Layout::default()
        .direction(Direction::Horizontal)
        .horizontal_margin(3)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)].as_ref())
        .split(vchunks[1]);

      let ok_text = Span::raw("Ok");
      let ok = Paragraph::new(ok_text)
        .style(Style::default().fg(if app.confirm {
          app.user_config.theme.hovered
        } else {
          app.user_config.theme.inactive
        }))
        .alignment(Alignment::Center);

      f.render_widget(ok, hchunks[0]);

      let cancel_text = Span::raw("Cancel");
      let cancel = Paragraph::new(cancel_text)
        .style(Style::default().fg(if app.confirm {
          app.user_config.theme.inactive
        } else {
          app.user_config.theme.hovered
        }))
        .alignment(Alignment::Center);

      f.render_widget(cancel, hchunks[1]);
    }
  }
}

fn draw_table(
  f: &mut Frame,
  app: &App,
  layout_chunk: Rect,
  table_layout: (&str, &TableHeader), // (title, header colums)
  items: &[TableItem], // The nested vector must have the same length as the `header_columns`
  selected_index: usize,
  highlight_state: (bool, bool),
)
{
  let selected_style = get_row_highlight_style(highlight_state, app.user_config.theme);

  let track_playing_index = app.current_playback_context.to_owned().and_then(|ctx| {
    ctx.item.and_then(|item| match item {
      PlayableItem::Track(track) => items.iter().position(|table_item| {
        track
          .id
          .as_ref()
          .map(|id| id.id() == table_item.id.as_str())
          .unwrap_or(false)
      }),
      PlayableItem::Episode(episode) => items
        .iter()
        .position(|table_item| episode.id.id() == table_item.id.as_str()),
      PlayableItem::Unknown(_) => None,
    })
  });

  let (title, header) = table_layout;

  // Make sure that the selected item is visible on the page. Need to add some rows of padding
  // to chunk height for header and header space to get a true table height
  let padding = 5;
  let offset = layout_chunk
    .height
    .checked_sub(padding)
    .and_then(|height| selected_index.checked_sub(height as usize))
    .unwrap_or(0);

  let rows = items.iter().skip(offset).enumerate().map(|(i, item)| {
    let mut formatted_row = item.format.clone();
    let mut style = Style::default().fg(app.user_config.theme.text); // default styling

    // if table displays songs
    match header.id {
      TableId::Song | TableId::RecentlyPlayed | TableId::Album => {
        // First check if the song should be highlighted because it is currently playing
        if let Some(title_idx) = header.get_index(ColumnId::Title) {
          if let Some(track_playing_offset_index) =
            track_playing_index.and_then(|idx| idx.checked_sub(offset))
          {
            if i == track_playing_offset_index {
              formatted_row[title_idx] = format!("▶ {}", &formatted_row[title_idx]);
              style = Style::default()
                .fg(app.user_config.theme.active)
                .add_modifier(Modifier::BOLD);
            }
          }
        }

        // Show this the liked icon if the song is liked
        if let Some(liked_idx) = header.get_index(ColumnId::Liked) {
          if app.liked_song_ids_set.contains(item.id.as_str()) {
            formatted_row[liked_idx] = app.user_config.padded_liked_icon();
          }
        }
      }
      TableId::PodcastEpisodes => {
        if let Some(name_idx) = header.get_index(ColumnId::Title) {
          if let Some(track_playing_offset_index) =
            track_playing_index.and_then(|idx| idx.checked_sub(offset))
          {
            if i == track_playing_offset_index {
              formatted_row[name_idx] = format!("▶ {}", &formatted_row[name_idx]);
              style = Style::default()
                .fg(app.user_config.theme.active)
                .add_modifier(Modifier::BOLD);
            }
          }
        }
      }
      _ => {}
    }

    // Next check if the item is under selection.
    if Some(i) == selected_index.checked_sub(offset) {
      style = selected_style;
    }

    // Return row styled data
    Row::new(formatted_row).style(style)
  });

  let widths = header
    .items
    .iter()
    .map(|h| Constraint::Length(h.width))
    .collect::<Vec<ratatui::layout::Constraint>>();

  let table = Table::new(rows, widths)
    .header(
      Row::new(header.items.iter().map(|h| h.text))
        .style(Style::default().fg(app.user_config.theme.header)),
    )
    .block(
      Block::default()
        .borders(Borders::ALL)
        .border_type(get_border_type(highlight_state))
        .style(Style::default().fg(app.user_config.theme.text))
        .title(Span::styled(
          title,
          get_color(highlight_state, app.user_config.theme),
        ))
        .border_style(get_color(highlight_state, app.user_config.theme)),
    )
    .style(Style::default().fg(app.user_config.theme.text));
  f.render_widget(table, layout_chunk);
}

#[cfg(test)]
mod home_tests {
  use super::*;
  use ratatui::{backend::TestBackend, Terminal};

  fn render(width: u16, height: u16, app: &App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
      .draw(|f| draw_home(f, app, Rect::new(0, 0, width, height)))
      .unwrap();
    let buffer = terminal.backend().buffer().clone();
    (0..height)
      .map(|y| {
        (0..width)
          .map(|x| buffer[(x, y)].symbol())
          .collect::<String>()
      })
      .collect::<Vec<String>>()
      .join("\n")
  }

  #[test]
  fn every_section_is_titled_and_the_hint_line_is_visible() {
    let app = App::default();
    let screen = render(110, 40, &app);
    for title in [
      "Made For You",
      "Recommended Stations",
      "Jump Back In",
      "Your Top Artists",
    ] {
      assert!(screen.contains(title), "{} missing from:\n{}", title, screen);
    }
    // The greeting block is 4 rows so its second line isn't clipped.
    assert!(screen.contains("Hello, there"), "{}", screen);
    assert!(screen.contains("pick a section (1/4)"), "{}", screen);
  }

  #[test]
  fn rendering_never_draws_outside_the_home_pane() {
    // ratatui panics on out-of-bounds writes, so this covers the section
    // layout maths at every size the pane can be given.
    let mut app = App::default();
    for height in 1u16..46 {
      for width in [1u16, 20, 60, 110, 200] {
        render(width, height, &app);
      }
    }
    app.home_mode = crate::app::HomeMode::Podcast;
    for height in [1u16, 8, 20, 40] {
      render(80, height, &app);
    }
  }
}

#[cfg(test)]
mod toast_tests {
  use super::*;
  use ratatui::{backend::TestBackend, Terminal};

  fn render(width: u16, height: u16, app: &App) -> Vec<String> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
      .draw(|f| {
        // Something underneath, to prove the toast overlays rather than replaces.
        f.render_widget(
          Paragraph::new("UNDERNEATH").block(Block::default().borders(Borders::ALL)),
          Rect::new(0, 0, width, height),
        );
        draw_toast(f, app);
      })
      .unwrap();
    let buffer = terminal.backend().buffer().clone();
    (0..height)
      .map(|y| {
        (0..width)
          .map(|x| buffer[(x, y)].symbol())
          .collect::<String>()
      })
      .collect()
  }

  fn app_with_error(message: &str) -> App {
    let mut app = App::default();
    app.handle_error(anyhow::anyhow!("{}", message));
    app
  }

  #[test]
  fn a_toast_sits_in_the_top_right_over_the_screen_behind_it() {
    let app = app_with_error("http error: status code 403 Forbidden");
    let screen = render(100, 20, &app);
    let joined = screen.join("\n");

    assert!(joined.contains("403 Forbidden"), "{}", joined);
    assert!(joined.contains("Error"), "{}", joined);
    // The 403 hint from the old error screen survives.
    assert!(joined.contains("Premium"), "{}", joined);
    // The view behind it is still drawn.
    assert!(joined.contains("UNDERNEATH"), "{}", joined);
    // Top-right: the message is near the top, right of centre.
    let row = screen
      .iter()
      .position(|line| line.contains("403 Forbidden"))
      .unwrap();
    assert!(row < 5, "toast should hug the top, was row {}", row);
    let column = screen[row].find("403").unwrap();
    assert!(column > 50, "toast should hug the right, was column {}", column);
  }

  #[test]
  fn no_toast_draws_nothing() {
    let app = App::default();
    let screen = render(100, 20, &app).join("\n");
    assert!(!screen.contains("Error"), "{}", screen);
  }

  #[test]
  fn a_toast_never_draws_outside_the_terminal() {
    let app = app_with_error("http error: status code 403 Forbidden — a fairly long message here");
    for width in 1u16..120 {
      for height in 1u16..14 {
        render(width, height, &app);
      }
    }
  }
}



#[cfg(test)]
mod device_rows_tests {
  use super::*;
  use rspotify::model::{
    context::{Actions, CurrentPlaybackContext},
    device::{Device, DevicePayload},
    CurrentlyPlayingType, DeviceType, RepeatState,
  };

  fn device(id: &str, name: &str, is_active: bool) -> Device {
    Device {
      id: Some(id.to_owned()),
      is_active,
      is_private_session: false,
      is_restricted: false,
      name: name.to_owned(),
      _type: DeviceType::Computer,
      volume_percent: Some(100),
    }
  }

  fn app_with(devices: Vec<Device>, playing_on: Option<Device>) -> App {
    let mut app = App::default();
    app.devices = Some(DevicePayload { devices });
    app.current_playback_context = playing_on.map(|device| CurrentPlaybackContext {
      device,
      repeat_state: RepeatState::Off,
      shuffle_state: false,
      context: None,
      timestamp: chrono::DateTime::from_timestamp(0, 0).unwrap(),
      progress: None,
      is_playing: true,
      item: None,
      currently_playing_type: CurrentlyPlayingType::Track,
      actions: Actions { disallows: vec![] },
    });
    app
  }

  #[test]
  fn the_dot_follows_playback_not_the_stale_device_list() {
    // The cached list still claims the Mac is active — it was fetched before
    // the user switched to the phone.
    let app = app_with(
      vec![
        device("mac", "MacBook Pro", true),
        device("phone", "Pixel", false),
      ],
      Some(device("phone", "Pixel", true)),
    );
    assert_eq!(
      device_rows(&app),
      vec!["  MacBook Pro".to_owned(), "● Pixel".to_owned()]
    );
  }

  #[test]
  fn with_nothing_playing_the_cached_flags_are_all_there_is() {
    let app = app_with(
      vec![
        device("mac", "MacBook Pro", true),
        device("phone", "Pixel", false),
      ],
      None,
    );
    assert_eq!(
      device_rows(&app),
      vec!["● MacBook Pro".to_owned(), "  Pixel".to_owned()]
    );
  }

  #[test]
  fn playing_on_a_device_that_is_not_in_the_list_marks_nothing() {
    let app = app_with(
      vec![device("mac", "MacBook Pro", true)],
      Some(device("web", "Web Player", true)),
    );
    assert_eq!(device_rows(&app), vec!["  MacBook Pro".to_owned()]);
  }

  #[test]
  fn empty_and_unloaded_lists_explain_themselves() {
    let mut app = App::default();
    assert_eq!(device_rows(&app), vec!["Press d to load…".to_owned()]);
    app.devices = Some(DevicePayload { devices: vec![] });
    assert_eq!(device_rows(&app), vec!["No devices found".to_owned()]);
  }
}
