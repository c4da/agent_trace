//! Data types for the Workspace & Brain Artifacts subsystem.

use serde::{Deserialize, Serialize};

/// Type classification for artifacts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Markdown,
    Image,
    Specification,
    Diff,
    Code,
    Unknown,
}

impl ArtifactKind {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "md" | "markdown" => Self::Markdown,
            "jpg" | "jpeg" | "png" | "gif" | "svg" | "webp" => Self::Image,
            "json" | "yaml" | "yml" | "toml" => Self::Specification,
            "diff" | "patch" => Self::Diff,
            "rs" | "py" | "js" | "ts" | "sh" | "ps1" => Self::Code,
            _ => Self::Unknown,
        }
    }

    pub fn mime_type(&self, ext: &str) -> &'static str {
        match ext.to_lowercase().as_str() {
            "md" | "markdown" => "text/markdown; charset=utf-8",
            "jpg" | "jpeg" => "image/jpeg",
            "png" => "image/png",
            "gif" => "image/gif",
            "svg" => "image/svg+xml",
            "webp" => "image/webp",
            "json" => "application/json",
            "yaml" | "yml" => "text/yaml; charset=utf-8",
            "toml" => "text/plain; charset=utf-8",
            "rs" | "py" | "js" | "ts" => "text/plain; charset=utf-8",
            _ => "application/octet-stream",
        }
    }
}

/// Metadata item describing an available artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactItem {
    pub id: String,
    pub title: String,
    pub kind: ArtifactKind,
    pub file_path: String,
    pub relative_path: String,
    pub size_bytes: u64,
    pub modified_at: String,
    pub preview: String,
    pub mime_type: String,
}

/// Detailed payload of an artifact for reading/viewing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactDetail {
    pub item: ArtifactItem,
    pub is_binary: bool,
    pub content: String,
}
