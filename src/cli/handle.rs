use crate::network::{IoEvent, Network};
use crate::user_config::UserConfig;

use super::{
  util::{Flag, JumpDirection, Type},
  CliApp,
};

use anyhow::{anyhow, Result};
use clap::parser::ValueSource;
use clap::ArgMatches;

/// The clap-4 migration lost per-flag format defaults; when the user didn't
/// pass --format explicitly, pick a default suited to the requested action.
fn effective_format(matches: &ArgMatches, per_flag_default: Option<&str>) -> String {
  let user_supplied = matches.value_source("format") != Some(ValueSource::DefaultValue);
  match (user_supplied, per_flag_default) {
    (false, Some(d)) => d.to_string(),
    _ => matches
      .get_one::<String>("format")
      .map(|s| s.to_string())
      .unwrap_or_default(),
  }
}

// Handle the different subcommands
pub async fn handle_matches(
  matches: &ArgMatches,
  cmd: String,
  net: Network,
  config: UserConfig,
) -> Result<String> {
  let mut cli = CliApp::new(net, config);

  cli.net.handle_network_event(IoEvent::GetDevices(false)).await;
  cli
    .net
    .handle_network_event(IoEvent::GetCurrentPlayback)
    .await;

  let devices_list = match &cli.net.app.lock().await.devices {
    Some(p) => p
      .devices
      .iter()
      .filter_map(|d| d.id.clone())
      .collect::<Vec<String>>(),
    None => Vec::new(),
  };

  // If the device_id is not specified, select the first available device
  let device_id = cli.net.client_config.device_id.clone();
  if device_id.is_none() || !devices_list.contains(&device_id.unwrap()) {
    // Select the first device available
    if let Some(d) = devices_list.get(0) {
      cli.net.client_config.set_device_id(d.clone())?;
    }
  }

  // `--device` is only defined on the playback/play subcommands. `get_one`
  // panics when an arg isn't part of the parsed command, which took `spt list`
  // and `spt search` down with it, so ask for it fallibly.
  if let Ok(Some(device)) = matches.try_get_one::<String>("device") {
    cli.set_device(device.clone()).await?;
  }

  // Evalute the subcommand
  let output = match cmd.as_str() {
    "playback" => {
      let per_flag_default = if matches.contains_id("transfer") {
        Some("%f %s %t - %a on %d")
      } else if matches.contains_id("volume") {
        Some("%v% %f %s %t - %a")
      } else if matches.contains_id("seek") {
        Some("%f %s %t - %a %r")
      } else {
        None
      };
      let format = effective_format(matches, per_flag_default);

      // Commands that are 'single'
      if matches.get_flag("share-track") {
        return cli.share_track_or_episode().await;
      } else if matches.get_flag("share-album") {
        return cli.share_album_or_show().await;
      }

      // Run the action, and print out the status
      // No 'else if's because multiple different commands are possible
      if matches.get_flag("toggle") {
        cli.toggle_playback().await;
      }
      if let Some(d) = matches.get_one::<String>("transfer").map(|s| s.as_str()) {
        cli.transfer_playback(d).await?;
      }
      // Multiple flags are possible
      if matches.contains_id("flags") {
        let flags = Flag::from_matches(matches);
        for f in flags {
          cli.mark(f).await?;
        }
      }
      if matches.contains_id("jumps") {
        let (direction, amount) = JumpDirection::from_matches(matches);
        for _ in 0..amount {
          cli.jump(&direction).await;
        }
      }
      if let Some(vol) = matches.get_one::<String>("volume").map(|s| s.as_str()) {
        cli.volume(vol.to_string()).await?;
      }
      if let Some(secs) = matches.get_one::<String>("seek").map(|s| s.as_str()) {
        cli.seek(secs.to_string()).await?;
      }

      // Print out the status if no errors were found
      cli.get_status(format).await
    }
    "play" => {
      let queue = matches.get_flag("queue");
      let random = matches.get_flag("random");
      let format = effective_format(matches, None);

      if let Some(uri) = matches.get_one::<String>("uri").map(|s| s.as_str()) {
        cli.play_uri(uri.to_string(), queue, random).await;
      } else if let Some(name) = matches.get_one::<String>("name").map(|s| s.as_str()) {
        let category = Type::play_from_matches(matches);
        cli.play(name.to_string(), category, queue, random).await?;
      }

      cli.get_status(format).await
    }
    "list" => {
      let per_flag_default = if matches.get_flag("devices") {
        Some("%v% %d")
      } else if matches.get_flag("playlists") {
        Some("%p (%u)")
      } else {
        None
      };
      let format = effective_format(matches, per_flag_default);

      // Update the limits for the list and search functions
      // I think the small and big search limits are very confusing
      // so I just set them both to max, is this okay?
      if let Some(max) = matches.get_one::<String>("limit").map(|s| s.as_str()) {
        cli.update_query_limits(max.to_string()).await?;
      }

      let category = Type::list_from_matches(matches);
      Ok(cli.list(category, &format).await)
    }
    "search" => {
      let per_flag_default = if matches.get_flag("albums") {
        Some("%b - %a (%u)")
      } else if matches.get_flag("artists") {
        Some("%a (%u)")
      } else if matches.get_flag("playlists") {
        Some("%p (%u)")
      } else if matches.get_flag("shows") {
        Some("%h (%u)")
      } else {
        None
      };
      let format = effective_format(matches, per_flag_default);

      // Update the limits for the list and search functions
      // I think the small and big search limits are very confusing
      // so I just set them both to max, is this okay?
      if let Some(max) = matches.get_one::<String>("limit").map(|s| s.as_str()) {
        cli.update_query_limits(max.to_string()).await?;
      }

      let category = Type::search_from_matches(matches);
      Ok(
        cli
          .query(
            matches.get_one::<String>("search").map(|s| s.as_str()).unwrap().to_string(),
            format,
            category,
          )
          .await,
      )
    }
    // Clap enforces that one of the things above is specified
    _ => unreachable!(),
  };

  // Check if there was an error
  let api_error = cli.net.app.lock().await.api_error.clone();
  if api_error.is_empty() {
    output
  } else {
    Err(anyhow!("{}", api_error))
  }
}
