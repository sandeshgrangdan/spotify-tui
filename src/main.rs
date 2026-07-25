mod app;
mod banner;
mod cli;
mod config;
mod event;
mod handlers;
mod home_sections;
mod network;
mod redirect_uri;
mod ui;
mod user_config;

use crate::app::RouteId;
use crate::event::Key;
use anyhow::Result;
use app::{ActiveBlock, App};
use backtrace::Backtrace;
use banner::BANNER;
use clap::{Arg, Command};
use config::ClientConfig;
use crossterm::{
  cursor::MoveTo,
  event::{DisableMouseCapture, EnableMouseCapture},
  execute,
  style::Print,
  terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen, SetTitle,
  },
  ExecutableCommand,
};
use rspotify::prelude::{BaseClient, OAuthClient};
use network::{IoEvent, Network, get_spotify};
use std::{
  cmp::{max, min},
  io::{self, stdout},
  panic::{self, PanicHookInfo},
  path::PathBuf,
  sync::Arc,
  time::SystemTime,
};
use tokio::sync::Mutex;
use ratatui::{
  backend::{Backend, CrosstermBackend},
  Terminal,
};
use user_config::{UserConfig, UserConfigPaths};

fn close_application() -> Result<()> {
  disable_raw_mode()?;
  let mut stdout = io::stdout();
  execute!(stdout, LeaveAlternateScreen, DisableMouseCapture)?;
  Ok(())
}

fn panic_hook(info: &PanicHookInfo<'_>) {
  if cfg!(debug_assertions) {
    let location = info.location().unwrap();

    let msg = match info.payload().downcast_ref::<&'static str>() {
      Some(s) => *s,
      None => match info.payload().downcast_ref::<String>() {
        Some(s) => &s[..],
        None => "Box<Any>",
      },
    };

    let stacktrace: String = format!("{:?}", Backtrace::new()).replace('\n', "\n\r");

    disable_raw_mode().unwrap();
    execute!(
      io::stdout(),
      LeaveAlternateScreen,
      Print(format!(
        "thread '<unnamed>' panicked at '{}', {}\n\r{}",
        msg, location, stacktrace
      )),
      DisableMouseCapture
    )
    .unwrap();
  }
}

#[tokio::main]
async fn main() -> Result<()> {
  panic::set_hook(Box::new(|info| {
    panic_hook(info);
  }));

  let clap_app = Command::new(env!("CARGO_PKG_NAME"))
    .version(env!("CARGO_PKG_VERSION"))
    .author(env!("CARGO_PKG_AUTHORS"))
    .about(env!("CARGO_PKG_DESCRIPTION"))
    .override_usage("Press `?` while running the app to see keybindings")
    .before_help(BANNER)
    .after_help(
      "Your spotify Client ID and Client Secret are stored in $HOME/.config/spotify-tui/client.yml",
    )
    .arg(
      Arg::new("tick-rate")
        .short('t')
        .long("tick-rate")
        .help("Set the tick rate (milliseconds): the lower the number the higher the FPS.")
        .long_help(
          "Specify the tick rate in milliseconds: the lower the number the \
higher the FPS. It can be nicer to have a lower value when you want to use the audio analysis view \
of the app. Beware that this comes at a CPU cost!",
        )
        .num_args(1),
    )
    .arg(
      Arg::new("config")
        .short('c')
        .long("config")
        .help("Specify configuration file path.")
        .num_args(1),
    )
    // TODO: re-add shell completions via clap_complete in a follow-up.
    // Control spotify from the command line
    .subcommand(cli::playback_subcommand())
    .subcommand(cli::play_subcommand())
    .subcommand(cli::list_subcommand())
    .subcommand(cli::search_subcommand());

  let matches = clap_app.get_matches();

  let mut user_config = UserConfig::new();
  if let Some(config_file_path) = matches.get_one::<String>("config").map(|s| s.as_str()) {
    let config_file_path = PathBuf::from(config_file_path);
    let path = UserConfigPaths { config_file_path };
    user_config.path_to_config.replace(path);
  }
  user_config.load_config()?;

  if let Some(tick_rate) = matches
    .get_one::<String>("tick-rate")
    .and_then(|tick_rate| tick_rate.parse().ok())
  {
    if tick_rate >= 1000 {
      panic!("Tick rate must be below 1000");
    } else {
      user_config.behavior.tick_rate_milliseconds = tick_rate;
    }
  }

  let mut client_config = ClientConfig::new();
  client_config.load_config()?;

  let config_paths = client_config.get_or_build_paths()?;

  // --- OAuth bootstrap (rspotify 0.16) ---
  //
  // 1. Build the client from stored credentials.
  let mut spotify = get_spotify(&client_config, &config_paths);

  // 2. Try the token cache first.
  let token_loaded = match spotify.read_token_cache(true).await {
    Ok(Some(token)) => {
      *spotify.get_token().lock().await.unwrap() = Some(token);
      true
    }
    _ => false,
  };

  // 3. If the cache miss, run the full browser-based OAuth flow.
  if !token_loaded {
    let port = client_config.get_port();
    let redirect_url = redirect_uri::redirect_uri_web_server(&spotify, port)
      .map_err(|_| anyhow::anyhow!("OAuth redirect listener failed"))?;

    let code = spotify
      .parse_response_code(&redirect_url)
      .ok_or_else(|| anyhow::anyhow!("Failed to parse OAuth response code from redirect URL"))?;

    spotify.request_token(&code).await?;
  } else {
    // If the cached token is expired, try a refresh.
    let is_expired = spotify
      .get_token()
      .lock()
      .await
      .unwrap()
      .as_ref()
      .map_or(true, |t| t.is_expired());

    if is_expired {
      spotify.refresh_token().await?;
    }
  }

  // --- Application startup ---
  let (io_tx, io_rx) = std::sync::mpsc::channel::<IoEvent>();

  // Determine token expiry from the current token (used to schedule re-auth).
  // rspotify Token stores `expires_in` as a chrono::Duration; convert to
  // a std::time::SystemTime by adding the seconds to `now`.
  let spotify_token_expiry = {
    let token_arc = spotify.get_token();
    let guard = token_arc.lock().await.unwrap();
    guard
      .as_ref()
      .map(|t| {
        let secs = t.expires_in.num_seconds().max(0) as u64;
        SystemTime::now() + std::time::Duration::from_secs(secs)
      })
      .unwrap_or(SystemTime::now())
  };

  let app = Arc::new(Mutex::new(App::new(
    io_tx,
    user_config.clone(),
    spotify_token_expiry,
  )));

  let cloned_app = Arc::clone(&app);

  let mut network = Network::new(spotify, client_config, cloned_app);

  // CLI subcommands run headless against the network layer and exit before
  // the TUI starts.
  if let Some((cmd, sub_matches)) = matches.subcommand() {
    match cli::handle_matches(sub_matches, cmd.to_string(), network, user_config).await {
      Ok(output) => println!("{}", output),
      Err(e) => {
        eprintln!("{}", e);
        std::process::exit(1);
      }
    }
    return Ok(());
  }

  // Start the network event loop in a blocking thread so the async runtime
  // isn't blocked by the `mpsc::Receiver::recv` call.
  let _ = std::thread::spawn(move || {
    start_tokio(io_rx, &mut network);
  });

  start_ui(user_config, &app).await?;

  Ok(())
}

#[tokio::main]
async fn start_tokio(io_rx: std::sync::mpsc::Receiver<IoEvent>, network: &mut Network) {
  while let Ok(io_event) = io_rx.recv() {
    network.handle_network_event(io_event).await;
  }
}

async fn start_ui(user_config: UserConfig, app: &Arc<Mutex<App>>) -> Result<()> {
  // Terminal initialization
  let mut stdout = stdout();
  execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
  enable_raw_mode()?;

  let mut backend = CrosstermBackend::new(stdout);

  if user_config.behavior.set_window_title {
    backend.execute(SetTitle("spt - Spotify TUI"))?;
  }

  let mut terminal = Terminal::new(backend)?;
  terminal.hide_cursor()?;

  let events = event::Events::new(user_config.behavior.tick_rate_milliseconds);

  // play music on, if not send them to the device selection view

  let mut is_first_render = true;

  loop {
    let mut app = app.lock().await;
    // Get the size of the screen on each loop to account for resize event
    if let Ok(size) = terminal.backend().size() {
      let size = ratatui::layout::Rect::new(0, 0, size.width, size.height);
      // Reset the help menu is the terminal was resized
      if is_first_render || app.size != size {
        app.help_menu_max_lines = 0;
        app.help_menu_offset = 0;
        app.help_menu_page = 0;

        app.size = size;

        // Based on the size of the terminal, adjust the search limit.
        let potential_limit = max((app.size.height as i32) - 13, 0) as u32;
        let max_limit = min(potential_limit, 50);
        let large_search_limit = min((f32::from(size.height) / 1.4) as u32, max_limit);
        let small_search_limit = min((f32::from(size.height) / 2.85) as u32, max_limit / 2);

        app.dispatch(IoEvent::UpdateSearchLimits(
          large_search_limit,
          small_search_limit,
        ));

        // Based on the size of the terminal, adjust how many lines are
        // displayed in the help menu
        if app.size.height > 8 {
          app.help_menu_max_lines = (app.size.height as u32) - 8;
        } else {
          app.help_menu_max_lines = 0;
        }
      }
    };

    let current_route = app.get_current_route();
    terminal.draw(|f| {
      match current_route.active_block {
        ActiveBlock::HelpMenu => {
          ui::draw_help_menu(f, &app);
        }
        ActiveBlock::SelectDevice => {
          ui::draw_device_list(f, &app);
        }
        ActiveBlock::Analysis => {
          ui::audio_analysis::draw(f, &app);
        }
        ActiveBlock::BasicView => {
          ui::draw_basic_view(f, &app);
        }
        _ => {
          ui::draw_main_layout(f, &app);
        }
      }
      // Overlays every view, so an error is visible wherever the user is.
      ui::draw_toast(f, &app);
    })?;


    if current_route.active_block == ActiveBlock::Input {
      terminal.show_cursor()?;
    } else {
      terminal.hide_cursor()?;
    }

    // Put the cursor back inside the input box (accounts for the wide-layout
    // top bar, the outer margin, and the box border).
    let (cursor_col, cursor_row) = ui::util::search_cursor_position(
      app.size.width,
      app.size.height,
      app.user_config.behavior.enforce_wide_search_bar,
      app.input_cursor_position,
    );
    terminal
      .backend_mut()
      .execute(MoveTo(cursor_col, cursor_row))?;

    // Handle authentication refresh
    if SystemTime::now() > app.spotify_token_expiry {
      app.dispatch(IoEvent::RefreshAuthentication);
    }

    match events.next()? {
      event::Event::Input(key) => {
        if key == Key::Ctrl('c') {
          break;
        }

        let current_active_block = app.get_current_route().active_block;

        // To avoid swallowing the global key presses `q` and `-` make a special
        // case for the input handler
        if current_active_block == ActiveBlock::Input {
          handlers::input_handler(key, &mut app);
        } else if key == app.user_config.keys.back {
          if app.get_current_route().active_block != ActiveBlock::Input {
            // On the home screen, back steps out of the entered section the
            // way `Esc` does. Home is the root route, so popping the stack
            // there would do nothing at all.
            if !app.back_out_of_home() {
              // Go back through the navigation stack when not in search input
              // mode. Reaching the root route is a no-op — only Ctrl+C
              // terminates the app.
              if let Some(ref x) = app.pop_navigation_stack() {
                if x.id == RouteId::Search {
                  // Skip the intermediate Search route on the way back.
                  app.pop_navigation_stack();
                }
              }
            }
          }
        } else {
          handlers::handle_app(key, &mut app);
        }
      }
      event::Event::Tick => {
        app.update_on_tick();
      }
    }

    // Delay spotify request until first render, will have the effect of improving
    // startup speed
    if is_first_render {
      app.dispatch(IoEvent::GetPlaylists);
      app.dispatch(IoEvent::GetUser);
      app.dispatch(IoEvent::GetCurrentPlayback);
      app.dispatch(IoEvent::GetRecentlyPlayed);
      // Fills the sidebar's device list without hijacking the first screen.
      app.dispatch(IoEvent::GetDevices(false));
      app.dispatch(IoEvent::GetTopArtists);
      // Feeds the home screen's "On Repeat" card.
      app.dispatch(IoEvent::GetOnRepeatTracks);
      app.get_made_for_you();
      app.help_docs_size = ui::help::get_help_docs(&app.user_config.keys).len() as u32;

      is_first_render = false;
    }
  }

  terminal.show_cursor()?;
  close_application()?;

  Ok(())
}
