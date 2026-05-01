mod fixtures;

use rspotify::prelude::{BaseClient, OAuthClient};
use rspotify::model::idtypes::Id;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Smoke test: verifies that the test harness wires up correctly and can
/// construct an `AuthCodeSpotify` client pointed at a wiremock server.
#[tokio::test]
async fn client_constructs_against_mock() {
    let mock_server = MockServer::start().await;
    let client = fixtures::build_client(&mock_server).await;

    // The token was populated — just check it's not None.
    let has_token = client.get_token().lock().await.unwrap().is_some();
    assert!(has_token, "expected a pre-populated token on the test client");
}

/// TDD template for `Network::get_user` / `current_user()`.
/// Mocks `GET /v1/me`, calls `current_user()`, and asserts the parsed user id.
#[tokio::test]
async fn current_user_parses_response() {
    let mock_server = MockServer::start().await;
    let client = fixtures::build_client(&mock_server).await;

    let body = fixtures::load_fixture("me.json");

    // Register the mock for GET /me/
    // Note: rspotify appends the endpoint path ("me/") directly onto the
    // api_base_url ("<mockserver>/"), so the wiremock path is just "/me/" —
    // there is no "/v1/" prefix when using a custom base URL.
    Mock::given(method("GET"))
        .and(path("/me/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&mock_server)
        .await;

    let user = client
        .current_user()
        .await
        .expect("current_user() should succeed against mock");

    assert_eq!(user.id.id(), "testuser");
}
