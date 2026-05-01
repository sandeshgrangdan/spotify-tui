use rspotify::AuthCodeSpotify;
use rspotify::prelude::OAuthClient;
use std::{
  io::prelude::*,
  net::{TcpListener, TcpStream},
};

/// Open a local TCP listener on `127.0.0.1:<port>`, get the authorization URL
/// from the client, open it in the default browser (or print it when the
/// browser can't be launched), and return the full redirect callback URL string
/// that Spotify sends back.
///
/// The returned URL still needs to be parsed with `client.parse_response_code`
/// and exchanged for a token with `client.request_token`.
pub fn redirect_uri_web_server(client: &AuthCodeSpotify, port: u16) -> Result<String, ()> {
  // Step 1: get the authorization URL from the client
  let auth_url = client.get_authorize_url(false).map_err(|e| {
    eprintln!("Failed to build authorization URL: {}", e);
  })?;

  // Step 2: try to open in a browser; fall back to printing the URL
  match webbrowser::open(&auth_url) {
    Ok(_) => println!("Opened Spotify authorization page in your browser."),
    Err(e) => {
      eprintln!(
        "Could not open browser ({}). Please navigate to:\n{}",
        e, auth_url
      );
    }
  }

  // Step 3: listen for the redirect callback
  let addr = format!("127.0.0.1:{}", port);
  let listener = TcpListener::bind(&addr).map_err(|e| {
    eprintln!("Failed to bind callback listener on {}: {}", addr, e);
  })?;

  println!("Waiting for Spotify to redirect to {}…", addr);

  for stream in listener.incoming() {
    match stream {
      Ok(stream) => {
        if let Some(url) = handle_connection(stream) {
          // The path captured from the TCP request is relative ("/callback?code=…&state=…"),
          // so we need to prepend the redirect_uri prefix to make it an absolute URL that
          // `parse_response_code` can parse.
          let base = client.get_oauth().redirect_uri.trim_end_matches('/');
          // The path already starts with '/', e.g. "/callback?code=...&state=..."
          let full_url = format!("{}{}", base, url);
          return Ok(full_url);
        }
      }
      Err(e) => {
        eprintln!("Connection error: {}", e);
      }
    }
  }

  Err(())
}

fn handle_connection(mut stream: TcpStream) -> Option<String> {
  // The request will be quite large (> 512) so just assign plenty just in case
  let mut buffer = [0; 1000];
  let _ = stream.read(&mut buffer).unwrap();

  // convert buffer into string and 'parse' the URL
  match String::from_utf8(buffer.to_vec()) {
    Ok(request) => {
      let split: Vec<&str> = request.split_whitespace().collect();

      if split.len() > 1 {
        respond_with_success(stream);
        return Some(split[1].to_string());
      }

      respond_with_error("Malformed request".to_string(), stream);
    }
    Err(e) => {
      respond_with_error(format!("Invalid UTF-8 sequence: {}", e), stream);
    }
  };

  None
}

fn respond_with_success(mut stream: TcpStream) {
  let contents = include_str!("redirect_uri.html");

  let response = format!("HTTP/1.1 200 OK\r\n\r\n{}", contents);

  stream.write_all(response.as_bytes()).unwrap();
  stream.flush().unwrap();
}

fn respond_with_error(error_message: String, mut stream: TcpStream) {
  println!("Error: {}", error_message);
  let response = format!(
    "HTTP/1.1 400 Bad Request\r\n\r\n400 - Bad Request - {}",
    error_message
  );

  stream.write_all(response.as_bytes()).unwrap();
  stream.flush().unwrap();
}
