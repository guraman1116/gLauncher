//! Mod search types and unified interface
//!
//! Common types for mod search results across different platforms.

use serde::{Deserialize, Serialize};

/// Source platform for mods
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModSource {
    Modrinth,
    CurseForge,
}

impl std::fmt::Display for ModSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModSource::Modrinth => write!(f, "Modrinth"),
            ModSource::CurseForge => write!(f, "CurseForge"),
        }
    }
}

/// Unified mod search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModSearchResult {
    /// Platform-specific ID
    pub id: String,
    /// Source platform
    pub source: ModSource,
    /// Mod name
    pub name: String,
    /// URL-friendly slug
    pub slug: String,
    /// Short description
    pub description: String,
    /// Author name
    pub author: String,
    /// Total download count
    pub downloads: u64,
    /// Icon/logo URL
    pub icon_url: Option<String>,
    /// Web page URL
    pub page_url: String,
    /// Supported game versions
    pub game_versions: Vec<String>,
    /// Supported mod loaders
    pub loaders: Vec<String>,
}

/// Mod version/file information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModVersion {
    /// Version ID
    pub id: String,
    /// Version number string
    pub version_number: String,
    /// Mod name (for display)
    pub mod_name: String,
    /// Supported game versions
    pub game_versions: Vec<String>,
    /// Supported loaders (fabric, forge, etc.)
    pub loaders: Vec<String>,
    /// Direct download URL
    pub download_url: String,
    /// Filename of the JAR
    pub filename: String,
    /// File size in bytes
    pub file_size: u64,
    /// Release date
    pub date_published: String,
    /// Required dependencies
    pub dependencies: Vec<ModDependency>,
}

/// Mod dependency information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModDependency {
    /// Dependency project ID
    pub project_id: String,
    /// Dependency type
    pub dependency_type: DependencyType,
    /// Dependency name (if available)
    pub name: Option<String>,
}

/// Type of dependency
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencyType {
    Required,
    Optional,
    Incompatible,
    Embedded,
}

/// Search filter parameters
#[derive(Debug, Clone, Default)]
pub struct SearchFilter {
    /// Search query
    pub query: String,
    /// Filter by game version (e.g., "1.20.1")
    pub game_version: Option<String>,
    /// Filter by mod loader (e.g., "fabric", "forge")
    pub loader: Option<String>,
    /// Maximum results to return
    pub limit: u32,
    /// Offset for pagination
    pub offset: u32,
}

impl SearchFilter {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            limit: 20,
            offset: 0,
            ..Default::default()
        }
    }

    pub fn with_game_version(mut self, version: impl Into<String>) -> Self {
        self.game_version = Some(version.into());
        self
    }

    pub fn with_loader(mut self, loader: impl Into<String>) -> Self {
        self.loader = Some(loader.into());
        self
    }

    pub fn with_limit(mut self, limit: u32) -> Self {
        self.limit = limit;
        self
    }

    pub fn with_offset(mut self, offset: u32) -> Self {
        self.offset = offset;
        self
    }
}

/// Search results with pagination info
#[derive(Debug, Clone)]
pub struct SearchResults {
    /// List of matching mods
    pub hits: Vec<ModSearchResult>,
    /// Total number of results
    pub total_hits: u64,
    /// Current offset
    pub offset: u32,
    /// Results per page
    pub limit: u32,
}
