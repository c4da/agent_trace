//! Unit tests for the artifacts scanner and types.

use super::scanner::scan_artifacts;
use super::types::ArtifactKind;

#[test]
fn test_artifact_kind_from_extension() {
    assert_eq!(ArtifactKind::from_extension("md"), ArtifactKind::Markdown);
    assert_eq!(ArtifactKind::from_extension("jpg"), ArtifactKind::Image);
    assert_eq!(ArtifactKind::from_extension("png"), ArtifactKind::Image);
    assert_eq!(ArtifactKind::from_extension("json"), ArtifactKind::Specification);
    assert_eq!(ArtifactKind::from_extension("rs"), ArtifactKind::Code);
    assert_eq!(ArtifactKind::from_extension("unknown_xyz"), ArtifactKind::Unknown);
}

#[test]
fn test_scan_artifacts_discovers_docs() {
    let items = scan_artifacts();
    // In our repository, docs/ exists and has markdown files
    assert!(!items.is_empty(), "Artifact scanner should discover markdown docs");
    for item in &items {
        assert!(!item.id.is_empty());
        assert!(!item.file_path.is_empty());
        assert!(item.size_bytes > 0);
    }
}

#[tokio::test]
async fn test_view_artifact_handler_renders_html() {
    let items = scan_artifacts();
    let first = items.first().expect("At least one artifact expected");
    let resp = super::viewer::view_artifact_handler(axum::extract::Path(first.id.clone()))
        .await
        .expect("Viewer handler should succeed");

    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    let content_type = resp
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(content_type.contains("text/html"));
}
