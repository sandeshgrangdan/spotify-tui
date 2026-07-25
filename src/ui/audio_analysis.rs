use super::util;
use crate::app::App;
use ratatui::{
  layout::{Constraint, Direction, Layout, Rect},
  style::Style,
  text::{Line, Span},
  widgets::{BarChart, Block, Borders, Paragraph},
  Frame,
};
use rspotify::model::PlayableItem;
const PITCHES: [&str; 12] = [
  "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// Why the analysis panes are empty, worded for whatever is playing.
///
/// The usual answer is the last one: Spotify revoked `/audio-analysis` for
/// third-party apps in November 2024 and the request now returns 403, so no
/// beat, key or pitch data exists for any track. The other cases get their own
/// wording so an empty screen never reads as a broken key.
fn unavailable_reason(item: Option<&PlayableItem>) -> Vec<Line<'static>> {
  match item {
    None => vec![
      Line::from("Nothing is playing."),
      Line::from("Start a track, then press v again."),
    ],
    Some(PlayableItem::Episode(episode)) => vec![
      Line::from(format!("Now playing: {}", episode.name)),
      Line::from("Podcast episodes have no audio analysis."),
    ],
    Some(PlayableItem::Unknown(_)) => vec![
      Line::from("The current item isn't a track — a video or a local file."),
      Line::from("Only tracks can be analysed."),
    ],
    Some(PlayableItem::Track(track)) => vec![
      Line::from(format!(
        "Now playing: {} — {}",
        track.name,
        util::create_artist_string(&track.artists)
      )),
      Line::from(""),
      Line::from("Spotify removed the audio-analysis API for third-party apps in November 2024;"),
      Line::from("the request is refused with 403, so there is no beat, key or pitch data."),
      Line::from("No version of the app can bring it back — the endpoint is gone."),
    ],
  }
}

/// Nothing to plot, so the explanation gets the whole screen. Squeezing it into
/// a 5-row box beside an empty bar chart is what made an unavailable API look
/// like a broken key.
fn draw_unavailable(f: &mut Frame, app: &App, area: Rect, margin: u16) {
  let chunk = Layout::default()
    .constraints([Constraint::Min(0)].as_ref())
    .margin(margin)
    .split(area)[0];

  let block = Block::default()
    .title(Span::styled(
      "Analysis",
      Style::default().fg(app.user_config.theme.inactive),
    ))
    .borders(Borders::ALL)
    .border_style(Style::default().fg(app.user_config.theme.inactive));

  f.render_widget(
    Paragraph::new(unavailable_reason(
      app
        .current_playback_context
        .as_ref()
        .and_then(|context| context.item.as_ref()),
    ))
    .block(block)
    .style(Style::default().fg(app.user_config.theme.text)),
    chunk,
  );
}

pub fn draw(f: &mut Frame, app: &App)
{
  let margin = util::get_main_layout_margin(app);
  let area = f.area();

  let analysis = match &app.audio_analysis {
    Some(analysis) => analysis,
    None => return draw_unavailable(f, app, area, margin),
  };

  let progress_seconds = (app.song_progress_ms as f32) / 1000.0;
  let beat = analysis
    .beats
    .iter()
    .find(|beat| beat.start >= progress_seconds);
  let beat_offset = beat
    .map(|beat| beat.start - progress_seconds)
    .unwrap_or(0.0);
  let segment = analysis
    .segments
    .iter()
    .find(|segment| segment.time_interval.start >= progress_seconds);
  let section = analysis
    .sections
    .iter()
    .find(|section| section.time_interval.start >= progress_seconds);
  let (segment, section) = match (segment, section) {
    (Some(segment), Some(section)) => (segment, section),
    // Played past the last analysed segment.
    _ => return draw_unavailable(f, app, area, margin),
  };

  let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([Constraint::Min(5), Constraint::Length(95)].as_ref())
    .margin(margin)
    .split(area);

  let analysis_block = Block::default()
    .title(Span::styled(
      "Analysis",
      Style::default().fg(app.user_config.theme.inactive),
    ))
    .borders(Borders::ALL)
    .border_style(Style::default().fg(app.user_config.theme.inactive));

  let white = Style::default().fg(app.user_config.theme.text);
  let gray = Style::default().fg(app.user_config.theme.inactive);
  let width = (chunks[1].width) as f32 / (1 + PITCHES.len()) as f32;
  let tick_rate = app.user_config.behavior.tick_rate_milliseconds;
  let bar_chart_title = &format!("Pitches | Tick Rate {} {}FPS", tick_rate, 1000 / tick_rate);

  let bar_chart_block = Block::default()
    .borders(Borders::ALL)
    .style(white)
    .title(Span::styled(bar_chart_title, gray))
    .border_style(gray);

  let texts = vec![
    Line::from(format!(
      "Tempo: {} (confidence {:.0}%)",
      section.tempo,
      section.tempo_confidence * 100.0
    )),
    Line::from(format!(
      "Key: {} (confidence {:.0}%)",
      PITCHES.get(section.key as usize).unwrap_or(&PITCHES[0]),
      section.key_confidence * 100.0
    )),
    Line::from(format!(
      "Time Signature: {}/4 (confidence {:.0}%)",
      section.time_signature,
      section.time_signature_confidence * 100.0
    )),
  ];
  let p = Paragraph::new(texts)
    .block(analysis_block)
    .style(Style::default().fg(app.user_config.theme.text));
  f.render_widget(p, chunks[0]);

  let data: Vec<(&str, u64)> = segment
    .clone()
    .pitches
    .iter()
    .enumerate()
    .map(|(index, pitch)| {
      let display_pitch = *PITCHES.get(index).unwrap_or(&PITCHES[0]);
      let bar_value = ((pitch * 1000.0) as u64)
        // Add a beat offset to make the bar animate between beats
        .checked_add((beat_offset * 3000.0) as u64)
        .unwrap_or(0);

      (display_pitch, bar_value)
    })
    .collect();

  let analysis_bar = BarChart::default()
    .block(bar_chart_block)
    .data(&data)
    .bar_width(width as u16)
    .bar_style(Style::default().fg(app.user_config.theme.analysis_bar))
    .value_style(
      Style::default()
        .fg(app.user_config.theme.analysis_bar_text)
        .bg(app.user_config.theme.analysis_bar),
    );
  f.render_widget(analysis_bar, chunks[1]);
}

#[cfg(test)]
mod tests {
  use super::*;

  fn track(name: &str) -> PlayableItem {
    use rspotify::model::{FullTrack, SimplifiedAlbum, SimplifiedArtist};
    PlayableItem::Track(FullTrack {
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
      artists: vec![SimplifiedArtist {
        external_urls: Default::default(),
        href: None,
        id: None,
        name: "Linkin Park".to_owned(),
      }],
      available_markets: vec![],
      disc_number: 1,
      duration: chrono::TimeDelta::seconds(0),
      explicit: false,
      external_ids: Default::default(),
      external_urls: Default::default(),
      href: None,
      id: None,
      is_local: false,
      is_playable: None,
      linked_from: None,
      name: name.to_owned(),
      popularity: 0,
      preview_url: None,
      restrictions: None,
      track_number: 0,
      r#type: rspotify::model::Type::Track,
    })
  }

  fn text(item: Option<&PlayableItem>) -> String {
    unavailable_reason(item)
      .iter()
      .map(|line| line.to_string())
      .collect::<Vec<String>>()
      .join("\n")
  }

  #[test]
  fn nothing_playing_says_so_instead_of_looking_broken() {
    let reason = text(None);
    assert!(reason.contains("Nothing is playing"), "{}", reason);
    assert!(reason.contains("press v again"), "{}", reason);
  }

  #[test]
  fn a_track_explains_the_removed_endpoint() {
    let item = track("The Emptiness Machine");
    let reason = text(Some(&item));
    // Names what is playing, so the screen is clearly live...
    assert!(reason.contains("The Emptiness Machine — Linkin Park"), "{}", reason);
    // ...and why it has no data to show.
    assert!(reason.contains("403"), "{}", reason);
    assert!(reason.contains("November 2024"), "{}", reason);
  }

  #[test]
  fn a_non_track_item_is_called_out_separately() {
    // A music video or local file lands in `Unknown`, which has no analysis.
    let item = PlayableItem::Unknown(serde_json::json!({"type": "video"}));
    let reason = text(Some(&item));
    assert!(reason.contains("isn't a track"), "{}", reason);
  }
}
