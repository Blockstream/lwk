//! OAuth2 token fetching shared by the authenticated clients.

use crate::Error;
use reqwest::Response;

/// Builds an [`Error`] describing an unsuccessful HTTP response, including the
/// status code and (a bounded prefix of) the response body so failures like an
/// authenticated backend returning `402 Insufficient credits` are reported
/// clearly rather than as a downstream JSON parsing error.
pub(crate) async fn error_for_status(url: &str, response: Response) -> Error {
    let status = response.status().as_u16();
    let body = response.text().await.unwrap_or_default();
    let snippet: String = body.trim().chars().take(500).collect();
    Error::EsploraHttpError {
        url: url.to_string(),
        status,
        body: (!snippet.is_empty()).then_some(snippet),
    }
}

/// Fetches an OAuth2 access token using client credentials flow
pub(crate) async fn fetch_oauth_token(
    client: &reqwest::Client,
    url: &str,
    client_id: &str,
    client_secret: &str,
) -> Result<String, Error> {
    let response = client
        .post(url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("grant_type", "client_credentials"),
            ("scope", "openid"),
        ])
        .send()
        .await?;

    // Surface OAuth/HTTP errors from the token endpoint (e.g. a 401 with
    // `{"error":"invalid_client"}` for bad credentials) instead of falling
    // through to a generic "missing access_token" message below.
    if !response.status().is_success() {
        return Err(error_for_status(url, response).await);
    }

    let token_response: serde_json::Value = response.json().await?;

    let token = token_response["access_token"]
        .as_str()
        .ok_or_else(|| Error::Generic("Missing access_token in response".to_string()))?
        .to_string();

    Ok(token)
}
