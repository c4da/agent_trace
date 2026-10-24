//! Artifacts subsystem coordinator module.

pub mod handlers;
pub mod scanner;
pub mod types;
pub mod viewer;

#[cfg(test)]
mod tests;

pub use handlers::{get_artifact_handler, list_artifacts_handler};
pub use scanner::scan_artifacts;
pub use types::{ArtifactDetail, ArtifactItem, ArtifactKind};
pub use viewer::view_artifact_handler;

