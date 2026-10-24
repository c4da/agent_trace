//! Dedicated standalone HTML viewer for artifacts (images, markdown, code, specs).
//! Optimized for mobile browsers (pinch-to-zoom, new-tab ergonomics, dark operator theme).

use super::scanner::scan_artifacts;
use super::types::{ArtifactItem, ArtifactKind};
use crate::zones::resolve_canonical_target;
use axum::{
    body::Body,
    extract::Path as AxumPath,
    http::{header, StatusCode},
    response::Response,
};
use std::fs;

/// `GET /api/v1/artifacts/:id/view` — Serves a mobile-optimized standalone viewer page in a new tab.
pub async fn view_artifact_handler(
    AxumPath(artifact_id): AxumPath<String>,
) -> Result<Response, StatusCode> {
    let items = scan_artifacts();
    let item = items
        .into_iter()
        .find(|a| a.id == artifact_id)
        .ok_or(StatusCode::NOT_FOUND)?;

    let path_norm = resolve_canonical_target(&item.file_path);
    let lower = path_norm.to_lowercase();
    if lower.contains(".secrets") || lower.contains(".ssh") || lower.ends_with(".env") {
        return Err(StatusCode::FORBIDDEN);
    }

    let html = render_viewer_html(&item)?;

    Response::builder()
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(html))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn render_viewer_html(item: &ArtifactItem) -> Result<String, StatusCode> {
    let title_escaped = escape_html(&item.title);
    let path_escaped = escape_html(&item.relative_path);
    let size_kb = (item.size_bytes as f64 / 1024.0).max(0.1);

    let body_content = match item.kind {
        ArtifactKind::Image => {
            format!(
                r#"
                <div class="media-container image-box">
                  <div class="hint-bar">
                    <span>💡 Tap image or pinch-to-zoom for full 1:1 resolution</span>
                    <a href="/api/v1/artifacts/{id}?raw=true" target="_blank" class="pill-link">🖼️ Open Raw Direct</a>
                  </div>
                  <a href="/api/v1/artifacts/{id}?raw=true" target="_blank" title="Tap for raw image in new tab">
                    <img src="/api/v1/artifacts/{id}?raw=true" alt="{title}" class="hero-image" />
                  </a>
                </div>
                "#,
                id = item.id,
                title = title_escaped
            )
        }
        _ => {
            let content = fs::read_to_string(&item.file_path)
                .unwrap_or_else(|_| "[Binary or unreadable content]".to_string());
            let content_escaped = escape_html(&content);
            format!(
                r#"
                <div class="media-container doc-box">
                  <div class="hint-bar">
                    <span>📄 {lines} lines · {chars} characters</span>
                    <button onclick="copyDocument()" id="btn-copy" class="pill-btn">📋 Copy All</button>
                    <a href="/api/v1/artifacts/{id}?raw=true" target="_blank" class="pill-link">⬇️ View Raw</a>
                  </div>
                  <pre id="doc-pre" class="code-view"><code>{content}</code></pre>
                </div>
                "#,
                lines = content.lines().count(),
                chars = content.len(),
                id = item.id,
                content = content_escaped
            )
        }
    };

    let full_html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=5.0, user-scalable=yes, viewport-fit=cover">
  <title>{title} // AgentTrace Artifact</title>
  <style>
    :root {{
      --bg: #090d13;
      --card-bg: #121822;
      --border: #1e2638;
      --accent: #58a6ff;
      --accent-glow: rgba(88, 166, 255, 0.15);
      --green: #3fb950;
      --text: #e6edf3;
      --text-muted: #8b949e;
      --font-mono: 'JetBrains Mono', 'SF Mono', Consolas, monospace;
      --font-sans: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    }}
    * {{ box-sizing: border-box; margin: 0; padding: 0; }}
    body {{
      background: var(--bg);
      color: var(--text);
      font-family: var(--font-sans);
      min-height: 100vh;
      display: flex;
      flex-direction: column;
      overflow-x: hidden;
      -webkit-font-smoothing: antialiased;
    }}
    header {{
      background: var(--card-bg);
      border-bottom: 1px solid var(--border);
      padding: 12px 16px;
      position: sticky;
      top: 0;
      z-index: 100;
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      justify-content: space-between;
      gap: 10px;
    }}
    .brand-meta {{
      display: flex;
      flex-direction: column;
      gap: 2px;
      min-width: 0;
      flex: 1;
    }}
    .title-row {{
      display: flex;
      align-items: center;
      gap: 8px;
    }}
    .title-text {{
      font-weight: 700;
      font-size: 15px;
      color: var(--text);
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
    }}
    .kind-badge {{
      font-family: var(--font-mono);
      font-size: 10px;
      text-transform: uppercase;
      padding: 2px 6px;
      border-radius: 4px;
      background: rgba(88, 166, 255, 0.15);
      color: var(--accent);
      border: 1px solid rgba(88, 166, 255, 0.3);
      flex-shrink: 0;
    }}
    .path-text {{
      font-family: var(--font-mono);
      font-size: 11px;
      color: var(--text-muted);
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
    }}
    .actions-row {{
      display: flex;
      align-items: center;
      gap: 8px;
      flex-shrink: 0;
    }}
    .btn {{
      display: inline-flex;
      align-items: center;
      gap: 4px;
      padding: 6px 12px;
      border-radius: 6px;
      font-size: 12px;
      font-weight: 600;
      text-decoration: none;
      cursor: pointer;
      border: 1px solid var(--border);
      background: #1e2638;
      color: var(--text);
      transition: all 0.15s ease;
    }}
    .btn:active {{ transform: scale(0.98); }}
    .btn-primary {{
      background: var(--accent);
      color: #05080e;
      border-color: var(--accent);
    }}
    main {{
      flex: 1;
      padding: 16px;
      display: flex;
      flex-direction: column;
      max-width: 1200px;
      width: 100%;
      margin: 0 auto;
    }}
    .media-container {{
      background: var(--card-bg);
      border: 1px solid var(--border);
      border-radius: 10px;
      overflow: hidden;
      display: flex;
      flex-direction: column;
    }}
    .hint-bar {{
      background: rgba(0, 0, 0, 0.35);
      border-bottom: 1px solid var(--border);
      padding: 8px 14px;
      display: flex;
      justify-content: space-between;
      align-items: center;
      font-size: 11px;
      color: var(--text-muted);
      font-family: var(--font-mono);
    }}
    .pill-link, .pill-btn {{
      background: rgba(88, 166, 255, 0.12);
      color: var(--accent);
      border: 1px solid rgba(88, 166, 255, 0.25);
      border-radius: 4px;
      padding: 3px 8px;
      font-size: 11px;
      text-decoration: none;
      font-family: var(--font-mono);
      cursor: pointer;
    }}
    .image-box {{
      padding-bottom: 24px;
      align-items: center;
      background: #06090e;
    }}
    .hero-image {{
      display: block;
      max-width: 100%;
      height: auto;
      margin: 16px auto;
      border-radius: 6px;
      border: 1px solid var(--border);
      box-shadow: 0 10px 40px rgba(0, 0, 0, 0.7);
      cursor: zoom-in;
    }}
    .code-view {{
      padding: 16px;
      margin: 0;
      font-family: var(--font-mono);
      font-size: 13px;
      line-height: 1.55;
      color: #e6edf3;
      white-space: pre-wrap;
      word-break: break-word;
      overflow-x: auto;
      background: #090d13;
    }}
  </style>
</head>
<body>
  <header>
    <div class="brand-meta">
      <div class="title-row">
        <span class="title-text">{title}</span>
        <span class="kind-badge">{kind:?}</span>
      </div>
      <div class="path-text">{path} · {size_kb:.1} KB</div>
    </div>
    <div class="actions-row">
      <a href="/api/v1/artifacts/{id}?raw=true" target="_blank" class="btn btn-primary">
        {action_label}
      </a>
      <a href="/mobile" class="btn">📱 Mobile App</a>
    </div>
  </header>

  <main>
    {body_content}
  </main>

  <script>
    function copyDocument() {{
      const pre = document.getElementById('doc-pre');
      if (!pre) return;
      navigator.clipboard.writeText(pre.innerText).then(() => {{
        const btn = document.getElementById('btn-copy');
        btn.textContent = '✅ Copied!';
        setTimeout(() => {{ btn.textContent = '📋 Copy All'; }}, 2000);
      }}).catch(err => {{
        alert('Copy failed: ' + err);
      }});
    }}
  </script>
</body>
</html>
"#,
        title = title_escaped,
        path = path_escaped,
        size_kb = size_kb,
        kind = item.kind,
        id = item.id,
        action_label = if item.kind == ArtifactKind::Image { "🖼️ Direct Raw" } else { "⬇️ Direct Raw" },
        body_content = body_content
    );

    Ok(full_html)
}

fn escape_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}
