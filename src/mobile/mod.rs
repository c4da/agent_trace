//! Mobile Touch-First Web Interface (`GET /mobile`).

use axum::response::Html;

pub const MOBILE_HTML: &str = include_str!("../mobile.html");

/// `GET /mobile` — Serves the mobile-optimized operator interface.
pub async fn serve_mobile() -> Html<&'static str> {
    Html(MOBILE_HTML)
}
