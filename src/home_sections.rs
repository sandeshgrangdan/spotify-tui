//! Derived data for the Home screen's sections.
//!
//! Both the renderer (`ui::draw_home`) and the key handler (`handlers::home`)
//! build the screen from here, so a row's text and what its Enter key does can
//! never drift apart.

use crate::app::{App, HomeBlock, HomeMode};
use rspotify::model::SimplifiedEpisode;
use rspotify::prelude::Id;

/// How many artist stations the home screen offers.
const MAX_STATIONS: usize = 12;
/// Radio seed cap — matches `RADIO_MAX_SEED_ARTISTS` in `network.rs`.
const MAX_STATION_SEEDS: usize = 3;
/// Rows per section — more than this and the user is scrolling a list that
/// would be better served by the dedicated library screens.
const MAX_ROW_ITEMS: usize = 20;

/// What activating a row does. Carried by the row so the handler never has to
/// re-derive the section to find out what the user picked.
#[derive(Debug, Clone, PartialEq)]
pub enum HomeItemAction {
  /// Open a Spotify-curated mix from the user's library in the track table.
  /// `index` positions `app.made_for_you_index`, which drives track paging.
  OpenMix {
    index: usize,
    id: String,
  },
  /// Build a client-side radio station from up to `MAX_STATION_SEEDS` artists.
  PlayStation {
    seeds: Vec<String>,
    name: String,
  },
  /// Same builder as a station, but the row names a mix rather than an artist.
  PlayMix {
    seeds: Vec<String>,
    name: String,
  },
  /// Open the user's short-term top tracks in the track table.
  OpenOnRepeat,
  /// Start the recently-played queue at `index`.
  PlayRecent {
    index: usize,
  },
  OpenArtist {
    id: String,
    name: String,
  },
  /// Open a saved show's episode list.
  OpenShow {
    index: usize,
  },
  PlayEpisode {
    uri: String,
  },
  /// Loading / empty placeholder — does nothing.
  Inert,
}

pub struct HomeItem {
  pub title: String,
  pub subtitle: String,
  pub action: HomeItemAction,
}

pub struct HomeSection {
  pub block: HomeBlock,
  pub title: String,
  pub items: Vec<HomeItem>,
}

/// The rows of the current home mode, top to bottom.
pub fn sections(app: &App) -> Vec<HomeSection> {
  match app.home_mode {
    HomeMode::Music => vec![
      made_for_you(app),
      recommended_stations(app),
      jump_back_in(app),
      top_artists(app),
    ],
    HomeMode::Podcast => vec![
      your_shows(app),
      // The feed sits above Continue Listening: picking something new to play
      // is the common case, and resuming is one row further down.
      latest_episodes_section(app),
      continue_listening(app),
    ],
  }
}

/// Selected row in `block`'s section.
pub fn item_index(app: &App, block: HomeBlock) -> usize {
  match block {
    HomeBlock::MadeForYou => app.home_made_for_you_index,
    HomeBlock::RecommendedStations => app.home_recommended_index,
    HomeBlock::JumpBackIn => app.home_jump_back_index,
    HomeBlock::TopArtists => app.home_top_artists_index,
    HomeBlock::YourShows => app.home_your_shows_index,
    HomeBlock::ContinueListening => app.home_continue_listening_index,
    HomeBlock::LatestEpisodes => app.home_latest_episodes_index,
  }
}

pub fn set_item_index(app: &mut App, block: HomeBlock, index: usize) {
  match block {
    HomeBlock::MadeForYou => app.home_made_for_you_index = index,
    HomeBlock::RecommendedStations => app.home_recommended_index = index,
    HomeBlock::JumpBackIn => app.home_jump_back_index = index,
    HomeBlock::TopArtists => app.home_top_artists_index = index,
    HomeBlock::YourShows => app.home_your_shows_index = index,
    HomeBlock::ContinueListening => app.home_continue_listening_index = index,
    HomeBlock::LatestEpisodes => app.home_latest_episodes_index = index,
  }
}

fn placeholder(title: &str, subtitle: &str) -> Vec<HomeItem> {
  vec![HomeItem {
    title: title.to_owned(),
    subtitle: subtitle.to_owned(),
    action: HomeItemAction::Inert,
  }]
}

// ── Made For <user> ─────────────────────────────────────────────────────────

/// The user's personal mixes.
///
/// Spotify's own Daily Mixes are *not* reachable from the Web API — they live
/// outside `current_user_playlists`, so a third-party app simply cannot list
/// them (only Discover Weekly / Release Radar appear, and only for users who
/// followed them). So this section builds the same idea from data the API does
/// give us: genre clusters of the user's top artists, plus "On Repeat", plus
/// any genuinely Spotify-owned playlist that *is* in their library.
fn made_for_you(app: &App) -> HomeSection {
  let display_name = app
    .user
    .as_ref()
    .and_then(|u| u.display_name.clone())
    .unwrap_or_else(|| "You".to_owned());

  // Order mirrors the app's own shelf: mixes, then On Repeat, then the
  // curated weekly playlists.
  let mut items = genre_mixes(app);
  items.extend(on_repeat_item(app));
  items.extend(library_mixes(app));
  items.truncate(MAX_ROW_ITEMS);

  if items.is_empty() {
    items = placeholder(
      "Building your mixes…",
      "Mixes come from the artists and songs you play most",
    );
  }

  HomeSection {
    block: HomeBlock::MadeForYou,
    title: format!("Made For {}", display_name),
    items,
  }
}

/// How many genre mixes to offer — Spotify tops out at six Daily Mixes too.
const MAX_GENRE_MIXES: usize = 6;

/// A mix per cluster of same-genre artists, which is what a Daily Mix is.
/// Playing one interleaves those artists' top tracks (see `merge_radio_tracks`).
///
/// `genres` is deprecated upstream; if it comes back empty there are no
/// clusters to build and the section falls back to the other row types.
#[allow(deprecated)]
fn genre_mixes(app: &App) -> Vec<HomeItem> {
  // Rank genres by how many of the user's artists share them, so the biggest
  // clusters become the first mixes. Ties break by name to keep it stable.
  let mut counts: Vec<(String, usize)> = Vec::new();
  for artist in &app.top_artists {
    for genre in &artist.genres {
      match counts.iter_mut().find(|(name, _)| name == genre) {
        Some((_, count)) => *count += 1,
        None => counts.push((genre.clone(), 1)),
      }
    }
  }
  counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

  let mut mixes: Vec<HomeItem> = Vec::new();
  let mut seeded: Vec<String> = Vec::new();
  for (genre, _) in counts {
    // An artist seeds only one mix, so six mixes don't all open with the same
    // three artists.
    let members: Vec<&rspotify::model::FullArtist> = app
      .top_artists
      .iter()
      .filter(|artist| artist.genres.contains(&genre))
      .filter(|artist| !seeded.contains(&artist.id.id().to_string()))
      .collect();
    if members.len() < 2 {
      continue;
    }

    let seeds: Vec<String> = members
      .iter()
      .take(MAX_STATION_SEEDS)
      .map(|artist| artist.id.id().to_string())
      .collect();
    let names: Vec<&str> = members
      .iter()
      .take(MAX_STATION_SEEDS)
      .map(|artist| artist.name.as_str())
      .collect();
    let subtitle = if members.len() > names.len() {
      format!("{} and more", names.join(", "))
    } else {
      names.join(", ")
    };
    seeded.extend(seeds.iter().cloned());

    let name = format!("{} Mix", title_case(&genre));
    mixes.push(HomeItem {
      title: name.clone(),
      subtitle,
      action: HomeItemAction::PlayMix { seeds, name },
    });
    if mixes.len() == MAX_GENRE_MIXES {
      break;
    }
  }
  mixes
}

/// "On Repeat" — what the user has played most in the last four weeks.
fn on_repeat_item(app: &App) -> Option<HomeItem> {
  if app.on_repeat_tracks.is_empty() {
    return None;
  }
  let mut names: Vec<&str> = Vec::new();
  for track in &app.on_repeat_tracks {
    for artist in &track.artists {
      if names.len() < 3 && !names.contains(&artist.name.as_str()) {
        names.push(&artist.name);
      }
    }
  }
  Some(HomeItem {
    title: "On Repeat".to_owned(),
    subtitle: if names.is_empty() {
      format!(
        "{} songs you keep coming back to",
        app.on_repeat_tracks.len()
      )
    } else {
      format!("{} and more", names.join(", "))
    },
    action: HomeItemAction::OpenOnRepeat,
  })
}

/// Spotify-owned playlists that really are in the user's library — Discover
/// Weekly and Release Radar for users who followed them, plus artist mixes.
/// Populated by `App::populate_made_for_you_from_library`.
fn library_mixes(app: &App) -> Vec<HomeItem> {
  match app.library.made_for_you_playlists.get_results(Some(0)) {
    Some(page) => page
      .items
      .iter()
      .enumerate()
      .map(|(index, playlist)| {
        let id = playlist.id.id().to_string();
        HomeItem {
          title: playlist.name.clone(),
          // Filled in asynchronously by `IoEvent::FetchMadeForYouPreview`.
          subtitle: app
            .made_for_you_previews
            .get(&id)
            .cloned()
            .unwrap_or_else(|| format!("{} tracks", playlist.items.total)),
          action: HomeItemAction::OpenMix { index, id },
        }
      })
      .collect(),
    None => Vec::new(),
  }
}

/// "nu metal" -> "Nu Metal", for card titles.
fn title_case(text: &str) -> String {
  text
    .split_whitespace()
    .map(|word| {
      let mut chars = word.chars();
      match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
      }
    })
    .collect::<Vec<_>>()
    .join(" ")
}

// ── Recommended Stations ────────────────────────────────────────────────────

struct Seed {
  id: String,
  name: String,
  genres: Vec<String>,
}

/// One radio card per artist, blended with the artists closest to it.
fn recommended_stations(app: &App) -> HomeSection {
  let pool = station_pool(app);
  let items = if pool.is_empty() {
    placeholder(
      "No stations yet",
      "Play a few tracks — stations are built from the artists you listen to",
    )
  } else {
    pool
      .iter()
      .enumerate()
      .map(|(index, seed)| {
        let blend = companions(&pool, index);
        let subtitle = if blend.is_empty() {
          format!("Songs based on {}", seed.name)
        } else {
          format!(
            "With {} and more",
            blend
              .iter()
              .map(|s| s.name.as_str())
              .collect::<Vec<_>>()
              .join(", ")
          )
        };
        let mut seeds = vec![seed.id.clone()];
        seeds.extend(blend.iter().map(|s| s.id.clone()));
        HomeItem {
          title: format!("{} Radio", seed.name),
          subtitle,
          action: HomeItemAction::PlayStation {
            seeds,
            name: seed.name.clone(),
          },
        }
      })
      .collect()
  };

  HomeSection {
    block: HomeBlock::RecommendedStations,
    title: "Recommended Stations".to_owned(),
    items,
  }
}

/// Artists worth a station, best-known first: top artists, then followed
/// artists (only if that page has already been fetched), then whoever the user
/// played recently.
///
/// `genres` is deprecated upstream and may come back empty; `companions` treats
/// that as "no blend" rather than assuming it's there.
#[allow(deprecated)]
fn station_pool(app: &App) -> Vec<Seed> {
  let mut pool: Vec<Seed> = Vec::new();
  for artist in app.top_artists.iter().chain(app.artists.iter()) {
    add_seed(
      &mut pool,
      artist.id.id().to_string(),
      artist.name.clone(),
      artist.genres.clone(),
    );
  }
  if let Some(page) = app.recently_played.result.as_ref() {
    for item in &page.items {
      for artist in &item.track.artists {
        if let Some(id) = artist.id.as_ref() {
          add_seed(
            &mut pool,
            id.id().to_string(),
            artist.name.clone(),
            Vec::new(),
          );
        }
      }
    }
  }
  pool.truncate(MAX_STATIONS);
  pool
}

fn add_seed(pool: &mut Vec<Seed>, id: String, name: String, genres: Vec<String>) {
  if id.is_empty() || pool.iter().any(|seed| seed.id == id) {
    return;
  }
  pool.push(Seed { id, name, genres });
}

/// Artists to blend into a station, ranked by shared genres.
///
/// `/v1/artists/{id}/related-artists` is gone for third-party apps, so the
/// user's own artists stand in for it: the closest ones by genre are added as
/// extra seeds, which is also what makes the "With …" subtitle truthful — the
/// station really is built from these artists' top tracks.
fn companions(pool: &[Seed], index: usize) -> Vec<&Seed> {
  let seed = match pool.get(index) {
    Some(seed) if !seed.genres.is_empty() => seed,
    _ => return Vec::new(),
  };
  let mut ranked: Vec<(usize, &Seed)> = pool
    .iter()
    .enumerate()
    .filter(|(other_index, _)| *other_index != index)
    .map(|(_, other)| (shared_genres(seed, other), other))
    .filter(|(score, _)| *score > 0)
    .collect();
  // Stable sort over a deterministic pool keeps a station's blend fixed
  // between renders.
  ranked.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
  ranked
    .into_iter()
    .take(MAX_STATION_SEEDS - 1)
    .map(|(_, seed)| seed)
    .collect()
}

fn shared_genres(a: &Seed, b: &Seed) -> usize {
  a.genres.iter().filter(|g| b.genres.contains(g)).count()
}

// ── Jump Back In ────────────────────────────────────────────────────────────

fn jump_back_in(app: &App) -> HomeSection {
  let items = match app.recently_played.result.as_ref() {
    None => placeholder("Loading recently played…", ""),
    Some(page) if page.items.is_empty() => {
      placeholder("Nothing played yet", "Your listening history shows up here")
    }
    Some(page) => {
      let mut seen: Vec<String> = Vec::new();
      let mut rows: Vec<HomeItem> = Vec::new();
      for (index, item) in page.items.iter().enumerate() {
        // The history repeats tracks; a list of duplicates is useless.
        // `index` stays the position in the *full* page so playback resumes at
        // the right offset.
        let key = item
          .track
          .id
          .as_ref()
          .map(|id| id.id().to_string())
          .unwrap_or_else(|| item.track.name.clone());
        if seen.contains(&key) {
          continue;
        }
        seen.push(key);
        rows.push(HomeItem {
          title: item.track.name.clone(),
          subtitle: crate::ui::util::create_artist_string(&item.track.artists),
          action: HomeItemAction::PlayRecent { index },
        });
        if rows.len() >= MAX_ROW_ITEMS {
          break;
        }
      }
      rows
    }
  };

  HomeSection {
    block: HomeBlock::JumpBackIn,
    title: "Jump Back In".to_owned(),
    items,
  }
}

// ── Your Top Artists ────────────────────────────────────────────────────────

#[allow(deprecated)]
fn top_artists(app: &App) -> HomeSection {
  let items = if app.top_artists.is_empty() {
    placeholder("Loading your top artists…", "")
  } else {
    app
      .top_artists
      .iter()
      .take(MAX_ROW_ITEMS)
      .map(|artist| HomeItem {
        title: artist.name.clone(),
        subtitle: if artist.genres.is_empty() {
          "Top tracks, albums & more".to_owned()
        } else {
          artist
            .genres
            .iter()
            .take(3)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
        },
        action: HomeItemAction::OpenArtist {
          id: artist.id.id().to_string(),
          name: artist.name.clone(),
        },
      })
      .collect()
  };

  HomeSection {
    block: HomeBlock::TopArtists,
    title: "Your Top Artists".to_owned(),
    items,
  }
}

// ── Podcast mode ────────────────────────────────────────────────────────────

/// Floor for how many recent episodes each show may contribute to the feed, so
/// a daily show can't crowd out a weekly one. Raised when there are few shows —
/// with a single podcast saved, capping at four would just hide episodes.
const MIN_EPISODES_PER_SHOW: usize = 4;

#[allow(deprecated)]
fn your_shows(app: &App) -> HomeSection {
  let items = match app.library.saved_shows.get_results(None) {
    Some(page) if !page.items.is_empty() => page
      .items
      .iter()
      .enumerate()
      .take(MAX_ROW_ITEMS)
      .map(|(index, saved)| {
        let unheard = app
          .podcast_episodes_per_show
          .get(saved.show.id.id())
          .map(|episodes| episodes.iter().filter(|e| is_unplayed(e)).count())
          .unwrap_or(0);
        HomeItem {
          title: saved.show.name.clone(),
          subtitle: if unheard > 0 {
            format!("{} · {} unplayed", saved.show.publisher, unheard)
          } else {
            saved.show.publisher.clone()
          },
          action: HomeItemAction::OpenShow { index },
        }
      })
      .collect(),
    _ => placeholder(
      "No saved podcasts",
      "Open Library → Podcasts and save a show",
    ),
  };

  HomeSection {
    block: HomeBlock::YourShows,
    title: "Your Shows".to_owned(),
    items,
  }
}

/// The browsable feed: recent episodes from every saved show, newest first, so
/// there is something to pick from without opening each show in turn.
fn latest_episodes_section(app: &App) -> HomeSection {
  let today = chrono::Utc::now().date_naive();
  let episodes = latest_episodes(app);
  let items = if episodes.is_empty() {
    placeholder(
      "No episodes yet",
      "Save a show in Library → Podcasts and its episodes appear here",
    )
  } else {
    episodes
      .iter()
      .map(|(show_name, episode)| HomeItem {
        // A leading dot marks what hasn't been played, the way the app badges
        // unheard episodes. It leads the row so a narrow pane can't clip it.
        title: format!(
          "{} {}",
          if is_unplayed(episode) { "●" } else { " " },
          episode.name
        ),
        subtitle: episode_details(show_name, episode, today),
        action: HomeItemAction::PlayEpisode {
          uri: episode.id.uri(),
        },
      })
      .collect()
  };

  HomeSection {
    block: HomeBlock::LatestEpisodes,
    title: "Latest Episodes".to_owned(),
    items,
  }
}

fn continue_listening(app: &App) -> HomeSection {
  let today = chrono::Utc::now().date_naive();
  let episodes = resumable_episodes(app);
  let items = if episodes.is_empty() {
    placeholder(
      "Nothing to resume",
      "Part-played episodes show up here with the time left",
    )
  } else {
    episodes
      .iter()
      .map(|(show_name, episode)| HomeItem {
        title: episode.name.clone(),
        subtitle: episode_details(show_name, episode, today),
        action: HomeItemAction::PlayEpisode {
          uri: episode.id.uri(),
        },
      })
      .collect()
  };

  HomeSection {
    block: HomeBlock::ContinueListening,
    title: "Continue Listening".to_owned(),
    items,
  }
}

/// `Show name · 3d ago · 42m` — plus how much is left once part-played.
fn episode_details(
  show_name: &str,
  episode: &SimplifiedEpisode,
  today: chrono::NaiveDate,
) -> String {
  let mut parts = vec![
    show_name.to_owned(),
    relative_date(&episode.release_date, today),
  ];
  match remaining_minutes(episode) {
    Some(minutes) => parts.push(format!("{} left", format_minutes(minutes))),
    None => parts.push(format_minutes(episode.duration.num_minutes())),
  }
  parts.join(" · ")
}

/// Never started, so worth a marker. A missing resume point means the API
/// didn't say — treat that as unplayed rather than inventing progress.
fn is_unplayed(episode: &SimplifiedEpisode) -> bool {
  match &episode.resume_point {
    Some(point) => !point.fully_played && point.resume_position.num_milliseconds() == 0,
    None => true,
  }
}

/// Minutes left for a part-played episode; `None` if it was never started or is
/// already finished.
fn remaining_minutes(episode: &SimplifiedEpisode) -> Option<i64> {
  let point = episode.resume_point.as_ref()?;
  let played = point.resume_position.num_milliseconds();
  if point.fully_played || played == 0 {
    return None;
  }
  Some(
    episode
      .duration
      .num_milliseconds()
      .saturating_sub(played)
      .max(0)
      / 60_000,
  )
}

fn format_minutes(minutes: i64) -> String {
  if minutes >= 60 {
    format!("{}h {}m", minutes / 60, minutes % 60)
  } else {
    format!("{}m", minutes.max(1))
  }
}

/// Release dates read better as an age. Falls back to the raw string for
/// anything that isn't a full date (Spotify also returns year-only precision).
fn relative_date(release_date: &str, today: chrono::NaiveDate) -> String {
  let released = match chrono::NaiveDate::parse_from_str(release_date, "%Y-%m-%d") {
    Ok(date) => date,
    Err(_) => return release_date.to_owned(),
  };
  let days = (today - released).num_days();
  match days {
    d if d < 0 => release_date.to_owned(),
    0 => "today".to_owned(),
    1 => "yesterday".to_owned(),
    2..=6 => format!("{}d ago", days),
    7..=27 => format!("{}w ago", days / 7),
    28..=364 => format!("{}mo ago", days / 30),
    _ => release_date.to_owned(),
  }
}

/// Part-played episodes across every saved show, newest first.
fn resumable_episodes(app: &App) -> Vec<(String, SimplifiedEpisode)> {
  let mut out = Vec::new();
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

/// Recent episodes from every saved show, newest first.
///
/// Each show contributes a share of the row budget rather than everything it
/// has cached: with many shows that keeps a daily podcast from filling the feed,
/// and with one or two shows the share is large enough to list them properly.
fn latest_episodes(app: &App) -> Vec<(String, SimplifiedEpisode)> {
  let mut out = Vec::new();
  if let Some(saved_page) = app.library.saved_shows.get_results(None) {
    let per_show = (MAX_ROW_ITEMS / saved_page.items.len().max(1)).max(MIN_EPISODES_PER_SHOW);
    for saved in &saved_page.items {
      let show_id = saved.show.id.id().to_string();
      if let Some(episodes) = app.podcast_episodes_per_show.get(&show_id) {
        for episode in episodes.iter().take(per_show) {
          out.push((saved.show.name.clone(), episode.clone()));
        }
      }
    }
  }
  out.sort_by(|a, b| b.1.release_date.cmp(&a.1.release_date));
  out.truncate(MAX_ROW_ITEMS);
  out
}

#[cfg(test)]
mod tests {
  use super::*;

  fn seed(name: &str, genres: &[&str]) -> Seed {
    Seed {
      id: name.to_lowercase(),
      name: name.to_owned(),
      genres: genres.iter().map(|g| g.to_string()).collect(),
    }
  }

  /// 22-char base62 ids, unique per artist name.
  #[allow(deprecated)]
  fn full_artist(name: &str, index: usize, genres: &[&str]) -> rspotify::model::FullArtist {
    use rspotify::model::{ArtistId, Followers, FullArtist};
    FullArtist {
      external_urls: Default::default(),
      followers: Followers { total: 0 },
      genres: genres.iter().map(|g| g.to_string()).collect(),
      href: String::new(),
      id: ArtistId::from_id(format!("{:022}", index)).unwrap(),
      images: vec![],
      name: name.to_owned(),
      popularity: 0,
    }
  }

  fn app_with_top_artists(artists: &[(&str, &[&str])]) -> App {
    let mut app = App::default();
    app.top_artists = artists
      .iter()
      .enumerate()
      .map(|(index, (name, genres))| full_artist(name, index, genres))
      .collect();
    app
  }

  #[test]
  fn title_case_capitalises_each_word() {
    assert_eq!(title_case("nu metal"), "Nu Metal");
    assert_eq!(title_case("rock"), "Rock");
    assert_eq!(title_case(""), "");
  }

  #[test]
  fn genre_mixes_cluster_artists_by_shared_genre() {
    let app = app_with_top_artists(&[
      ("Linkin Park", &["nu metal", "rock"]),
      ("Staind", &["nu metal", "rock"]),
      ("Audioslave", &["nu metal"]),
      ("Purna Rai", &["nepali pop"]),
      ("Swar", &["nepali pop"]),
    ]);
    let mixes = genre_mixes(&app);
    let titles: Vec<&str> = mixes.iter().map(|c| c.title.as_str()).collect();
    // Biggest cluster first.
    assert_eq!(titles, vec!["Nu Metal Mix", "Nepali Pop Mix"]);
    assert_eq!(mixes[0].subtitle, "Linkin Park, Staind, Audioslave");
    // "rock" would re-seed the same three artists, so it doesn't become a mix.
    assert!(!titles.contains(&"Rock Mix"));
  }

  #[test]
  fn a_genre_mix_seeds_the_radio_builder_with_its_artists() {
    let app = app_with_top_artists(&[
      ("A", &["rock"]),
      ("B", &["rock"]),
      ("C", &["rock"]),
      ("D", &["rock"]),
    ]);
    let mixes = genre_mixes(&app);
    match &mixes[0].action {
      HomeItemAction::PlayMix { seeds, name } => {
        // Capped at the radio builder's seed limit.
        assert_eq!(seeds.len(), MAX_STATION_SEEDS);
        assert_eq!(name, "Rock Mix");
      }
      other => panic!("expected a mix action, got {:?}", other),
    }
    assert_eq!(mixes[0].subtitle, "A, B, C and more");
  }

  #[test]
  fn a_single_artist_genre_is_not_a_mix() {
    let app = app_with_top_artists(&[("A", &["rock"]), ("B", &["jazz"])]);
    assert!(genre_mixes(&app).is_empty());
  }

  #[test]
  fn no_genres_means_no_mixes_rather_than_a_panic() {
    let app = app_with_top_artists(&[("A", &[]), ("B", &[])]);
    assert!(genre_mixes(&app).is_empty());
  }

  #[test]
  fn made_for_you_falls_back_to_a_placeholder_before_data_arrives() {
    let app = App::default();
    let section = made_for_you(&app);
    assert_eq!(section.title, "Made For You");
    assert_eq!(section.items[0].action, HomeItemAction::Inert);
  }

  #[test]
  fn made_for_you_shows_the_users_name_and_mixes_once_loaded() {
    let app = app_with_top_artists(&[("A", &["rock"]), ("B", &["rock"])]);
    let section = made_for_you(&app);
    assert_eq!(section.items.len(), 1);
    assert_eq!(section.items[0].title, "Rock Mix");
  }

  #[test]
  fn companions_rank_by_shared_genres() {
    let pool = vec![
      seed("Bryan Adams", &["rock", "soft rock", "canadian"]),
      seed("Purna Rai", &["nepali pop"]),
      seed("Chicago", &["rock", "soft rock"]),
      seed("TOTO", &["rock"]),
    ];
    let blend: Vec<&str> = companions(&pool, 0)
      .iter()
      .map(|s| s.name.as_str())
      .collect();
    // Two shared genres beats one, and only 2 companions fit the 3-seed cap.
    assert_eq!(blend, vec!["Chicago", "TOTO"]);
  }

  #[test]
  fn companions_are_empty_without_genre_overlap() {
    let pool = vec![seed("A", &["rock"]), seed("B", &["jazz"])];
    assert!(companions(&pool, 0).is_empty());
    // An artist with no genres at all (e.g. from recently-played) gets none.
    let pool = vec![seed("A", &[]), seed("B", &["jazz"])];
    assert!(companions(&pool, 0).is_empty());
  }

  #[test]
  fn add_seed_skips_duplicates_and_missing_ids() {
    let mut pool = Vec::new();
    add_seed(&mut pool, "1".into(), "A".into(), vec![]);
    add_seed(&mut pool, "1".into(), "A again".into(), vec![]);
    add_seed(&mut pool, String::new(), "No id".into(), vec![]);
    assert_eq!(pool.len(), 1);
  }

  #[test]
  fn music_mode_has_four_sections() {
    let app = App::default();
    let blocks: Vec<HomeBlock> = sections(&app).iter().map(|s| s.block).collect();
    assert_eq!(
      blocks,
      vec![
        HomeBlock::MadeForYou,
        HomeBlock::RecommendedStations,
        HomeBlock::JumpBackIn,
        HomeBlock::TopArtists,
      ]
    );
  }

  #[test]
  fn empty_sections_still_render_one_row_so_nothing_is_ever_blank() {
    let app = App::default();
    for section in sections(&app) {
      assert!(!section.items.is_empty(), "{} had no rows", section.title);
      assert_eq!(section.items[0].action, HomeItemAction::Inert);
    }
  }

  #[test]
  fn item_index_round_trips_per_block() {
    let mut app = App::default();
    for (i, block) in [
      HomeBlock::MadeForYou,
      HomeBlock::RecommendedStations,
      HomeBlock::JumpBackIn,
      HomeBlock::TopArtists,
      HomeBlock::YourShows,
      HomeBlock::ContinueListening,
      HomeBlock::LatestEpisodes,
    ]
    .iter()
    .enumerate()
    {
      set_item_index(&mut app, *block, i);
      assert_eq!(item_index(&app, *block), i);
    }
  }

  // ── Podcast fixtures ──────────────────────────────────────────────────────

  #[allow(deprecated)]
  fn saved_show(id: &str, name: &str) -> rspotify::model::Show {
    use rspotify::model::{ShowId, SimplifiedShow};
    rspotify::model::Show {
      added_at: String::new(),
      show: SimplifiedShow {
        available_markets: vec![],
        copyrights: vec![],
        description: String::new(),
        explicit: false,
        external_urls: Default::default(),
        href: String::new(),
        id: ShowId::from_id(format!("{:0>22}", id)).unwrap(),
        images: vec![],
        is_externally_hosted: Some(false),
        languages: vec![],
        media_type: "audio".to_owned(),
        name: name.to_owned(),
        publisher: "A Publisher".to_owned(),
      },
    }
  }

  #[allow(deprecated)]
  fn episode(
    id: &str,
    name: &str,
    release_date: &str,
    minutes: i64,
    played_minutes: Option<i64>,
  ) -> SimplifiedEpisode {
    use rspotify::model::{DatePrecision, EpisodeId, ResumePoint};
    SimplifiedEpisode {
      audio_preview_url: None,
      description: String::new(),
      duration: chrono::TimeDelta::minutes(minutes),
      explicit: false,
      external_urls: Default::default(),
      href: String::new(),
      id: EpisodeId::from_id(format!("{:0>22}", id)).unwrap(),
      images: vec![],
      is_externally_hosted: false,
      is_playable: true,
      language: String::new(),
      languages: vec![],
      name: name.to_owned(),
      release_date: release_date.to_owned(),
      release_date_precision: DatePrecision::Day,
      resume_point: played_minutes.map(|played| ResumePoint {
        fully_played: false,
        resume_position: chrono::TimeDelta::minutes(played),
      }),
    }
  }

  fn podcast_app(shows: Vec<(rspotify::model::Show, Vec<SimplifiedEpisode>)>) -> App {
    use rspotify::model::Page;
    let mut app = App::default();
    app.home_mode = HomeMode::Podcast;
    let total = shows.len() as u32;
    let mut pages = Vec::new();
    for (show, episodes) in shows {
      app
        .podcast_episodes_per_show
        .insert(show.show.id.id().to_string(), episodes);
      pages.push(show);
    }
    app.library.saved_shows.pages.push(Page {
      items: pages,
      href: String::new(),
      limit: total.max(1),
      next: None,
      offset: 0,
      previous: None,
      total,
    });
    app
  }

  fn day(date: &str) -> chrono::NaiveDate {
    chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap()
  }

  #[test]
  fn release_dates_read_as_an_age() {
    let today = day("2026-07-25");
    assert_eq!(relative_date("2026-07-25", today), "today");
    assert_eq!(relative_date("2026-07-24", today), "yesterday");
    assert_eq!(relative_date("2026-07-21", today), "4d ago");
    assert_eq!(relative_date("2026-07-11", today), "2w ago");
    assert_eq!(relative_date("2026-05-25", today), "2mo ago");
    // Older than a year, and anything not a full date, is shown verbatim.
    assert_eq!(relative_date("2024-01-01", today), "2024-01-01");
    assert_eq!(relative_date("2026", today), "2026");
    // A future date isn't dressed up as "in -2 days".
    assert_eq!(relative_date("2026-07-27", today), "2026-07-27");
  }

  #[test]
  fn unplayed_means_no_progress_recorded() {
    assert!(is_unplayed(&episode("1", "e", "2026-07-25", 40, None)));
    assert!(is_unplayed(&episode("1", "e", "2026-07-25", 40, Some(0))));
    assert!(!is_unplayed(&episode("1", "e", "2026-07-25", 40, Some(5))));
  }

  #[test]
  fn remaining_minutes_only_apply_to_part_played_episodes() {
    assert_eq!(
      remaining_minutes(&episode("1", "e", "2026-07-25", 40, Some(15))),
      Some(25)
    );
    assert_eq!(
      remaining_minutes(&episode("1", "e", "2026-07-25", 40, None)),
      None
    );
    assert_eq!(
      remaining_minutes(&episode("1", "e", "2026-07-25", 40, Some(0))),
      None
    );
  }

  #[test]
  fn durations_read_in_hours_once_they_are_long() {
    assert_eq!(format_minutes(42), "42m");
    assert_eq!(format_minutes(72), "1h 12m");
    assert_eq!(format_minutes(60), "1h 0m");
    // Sub-minute episodes still say something.
    assert_eq!(format_minutes(0), "1m");
  }

  #[test]
  fn a_row_names_the_show_its_age_and_its_length() {
    let today = day("2026-07-25");
    let fresh = episode("1", "e", "2026-07-23", 42, None);
    assert_eq!(
      episode_details("The Show", &fresh, today),
      "The Show · 2d ago · 42m"
    );
    let started = episode("2", "e", "2026-07-23", 42, Some(30));
    assert_eq!(
      episode_details("The Show", &started, today),
      "The Show · 2d ago · 12m left"
    );
  }

  #[test]
  fn the_feed_merges_shows_newest_first() {
    let app = podcast_app(vec![
      (
        saved_show("1", "Daily Show"),
        vec![
          episode("11", "D-1", "2026-07-24", 30, None),
          episode("12", "D-2", "2026-07-23", 30, None),
        ],
      ),
      (
        saved_show("2", "Weekly Show"),
        vec![episode("21", "W-1", "2026-07-25", 60, None)],
      ),
    ]);

    let feed: Vec<String> = latest_episodes(&app)
      .into_iter()
      .map(|(_, episode)| episode.name)
      .collect();
    assert_eq!(feed, vec!["W-1", "D-1", "D-2"]);
  }

  #[test]
  fn one_saved_show_is_listed_properly_rather_than_capped_at_four() {
    let episodes: Vec<SimplifiedEpisode> = (1..=8)
      .map(|n| {
        episode(
          &format!("1{}", n),
          &format!("E-{}", n),
          &format!("2026-07-{:02}", 25 - n),
          30,
          None,
        )
      })
      .collect();
    let app = podcast_app(vec![(saved_show("1", "The Only Show"), episodes)]);
    assert_eq!(latest_episodes(&app).len(), 8);
  }

  #[test]
  fn with_many_shows_no_single_one_fills_the_feed() {
    let shows: Vec<_> = (1..=6)
      .map(|show| {
        let episodes: Vec<SimplifiedEpisode> = (1..=8)
          .map(|n| {
            episode(
              &format!("{}{:02}", show, n),
              &format!("S{}-E{}", show, n),
              &format!("2026-07-{:02}", 25 - n),
              30,
              None,
            )
          })
          .collect();
        (
          saved_show(&show.to_string(), &format!("Show {}", show)),
          episodes,
        )
      })
      .collect();
    let app = podcast_app(shows);

    let feed = latest_episodes(&app);
    assert_eq!(feed.len(), MAX_ROW_ITEMS);
    let from_first_show = feed
      .iter()
      .filter(|(show_name, _)| show_name == "Show 1")
      .count();
    assert!(
      from_first_show <= MIN_EPISODES_PER_SHOW,
      "one show took {} of {} rows",
      from_first_show,
      MAX_ROW_ITEMS
    );
  }

  #[test]
  fn the_feed_marks_what_has_not_been_played() {
    let app = podcast_app(vec![(
      saved_show("1", "The Show"),
      vec![
        episode("11", "Unheard", "2026-07-24", 30, None),
        episode("12", "Started", "2026-07-23", 30, Some(10)),
      ],
    )]);
    let section = latest_episodes_section(&app);
    assert!(
      section.items[0].title.starts_with("● "),
      "{:?}",
      section.items[0].title
    );
    assert!(
      section.items[1].title.starts_with("  "),
      "{:?}",
      section.items[1].title
    );
  }

  #[test]
  fn a_shows_row_counts_its_unplayed_episodes() {
    let app = podcast_app(vec![(
      saved_show("1", "The Show"),
      vec![
        episode("11", "a", "2026-07-24", 30, None),
        episode("12", "b", "2026-07-23", 30, None),
        episode("13", "c", "2026-07-22", 30, Some(10)),
      ],
    )]);
    assert_eq!(
      your_shows(&app).items[0].subtitle,
      "A Publisher · 2 unplayed"
    );
  }

  #[test]
  fn podcast_sections_are_shows_then_the_feed_then_resume() {
    let app = podcast_app(vec![(saved_show("1", "The Show"), vec![])]);
    let blocks: Vec<HomeBlock> = sections(&app).iter().map(|s| s.block).collect();
    assert_eq!(
      blocks,
      vec![
        HomeBlock::YourShows,
        HomeBlock::LatestEpisodes,
        HomeBlock::ContinueListening,
      ]
    );
  }
}
