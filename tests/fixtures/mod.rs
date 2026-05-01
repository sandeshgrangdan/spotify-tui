use std::collections::HashSet;
use std::path::PathBuf;

use rspotify::prelude::BaseClient;
use rspotify::{AuthCodeSpotify, Config, Credentials, OAuth, Token};
use wiremock::MockServer;

/// Construct an `AuthCodeSpotify` client whose `api_base_url` points at
/// the wiremock server and whose token is pre-populated so every request
/// carries a valid Bearer header without triggering the token-refresh path.
pub async fn build_client(mock_server: &MockServer) -> AuthCodeSpotify {
    let creds = Credentials::new("test_client_id", "test_client_secret");

    let oauth = OAuth {
        redirect_uri: "http://127.0.0.1:8888/callback".to_string(),
        scopes: HashSet::new(),
        ..Default::default()
    };

    // Point the client at the wiremock server. The URL must end with a `/` so
    // that rspotify's `api_url` concatenation works correctly.
    let api_base_url = format!("{}/", mock_server.uri());

    let config = Config {
        api_base_url,
        // Disable caching and auto-refreshing so the test token is used as-is.
        token_cached: false,
        token_refreshing: false,
        cache_path: PathBuf::from("/tmp/test_spotify_token_cache.json"),
        ..Default::default()
    };

    let client = AuthCodeSpotify::with_config(creds, oauth, config);

    // Pre-populate a token with a long `expires_in` so it appears non-expired.
    // We use Token::default() as a base and override the fields we care about.
    // `expires_at` in the default is `Some(Utc::now())` which means it's
    // technically expired, but because `token_refreshing: false` rspotify's
    // `auto_reauth` will skip the refresh check.
    let token = Token {
        access_token: "test_access_token".to_string(),
        refresh_token: Some("test_refresh_token".to_string()),
        ..Token::default()
    };

    *client.get_token().lock().await.unwrap() = Some(token);

    client
}

/// Load a fixture file from `tests/fixtures/responses/<name>`.
/// `name` should be a filename like `"me.json"`.
pub fn load_fixture(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("responses")
        .join(name);

    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {}", path.display(), e))
}
