pub mod crawlers;
pub mod models;
pub mod processing;
pub mod repository;

/// Shared cosine-similarity threshold for automatic matching workflows.
pub const SIMILARITY_THRESHOLD: f32 = 0.8;
