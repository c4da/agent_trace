//! Antigravity Desktop Client Web Workspace (`GET /client` and `GET /app`).

use axum::response::Html;

pub const CLIENT_HTML: &str = include_str!("../client.html");

/// Serves the full Antigravity desktop web client.
pub async fn serve_client() -> Html<&'static str> {
    Html(CLIENT_HTML)
}
