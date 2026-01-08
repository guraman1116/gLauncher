//! Modrinth API client
//!
//! Client for interacting with the Modrinth API v2.
//! No authentication required, but User-Agent header is recommended.

use super::search::{
    DependencyType, ModDependency, ModSearchResult, ModSource, ModVersion, SearchFilter,
    SearchResults,
};
use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;

/// Modrinth API client
pub struct ModrinthClient {
    client: Client,
}

impl ModrinthClient {
    const BASE_URL: &'static str = "https://api.modrinth.com/v2";
    const USER_AGENT: &'static str = "gLauncher/0.1.1 (https://github.com/guraman1116/gLauncher)";

    /// Create a new Modrinth client
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent(Self::USER_AGENT)
            .build()
            .expect("Failed to create HTTP client");

        Self { client }
    }

    /// Search for mods
    pub async fn search(&self, filter: &SearchFilter) -> Result<SearchResults> {
        let mut facets = Vec::new();

        // Add project_type facet for mods only
        facets.push(r#"["project_type:mod"]"#.to_string());

        if let Some(ref version) = filter.game_version {
            facets.push(format!(r#"["versions:{}"]"#, version));
        }
        if let Some(ref loader) = filter.loader {
            facets.push(format!(r#"["categories:{}"]"#, loader));
        }

        let facets_str = format!("[{}]", facets.join(","));

        let url = format!("{}/search", Self::BASE_URL);
        let response = self
            .client
            .get(&url)
            .query(&[
                ("query", filter.query.as_str()),
                ("facets", &facets_str),
                ("limit", &filter.limit.to_string()),
                ("offset", &filter.offset.to_string()),
            ])
            .send()
            .await
            .context("Failed to send search request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Modrinth API error: {} - {}", status, body);
        }

        let search_response: ModrinthSearchResponse = response
            .json()
            .await
            .context("Failed to parse search response")?;

        let hits = search_response
            .hits
            .into_iter()
            .map(|hit| {
                let page_url = format!("https://modrinth.com/mod/{}", &hit.slug);
                ModSearchResult {
                    id: hit.project_id,
                    source: ModSource::Modrinth,
                    name: hit.title,
                    slug: hit.slug,
                    description: hit.description,
                    author: hit.author,
                    downloads: hit.downloads,
                    icon_url: hit.icon_url,
                    page_url,
                    game_versions: hit.versions,
                    loaders: hit.categories,
                }
            })
            .collect();

        Ok(SearchResults {
            hits,
            total_hits: search_response.total_hits,
            offset: search_response.offset,
            limit: search_response.limit,
        })
    }

    /// Get versions for a project
    pub async fn get_versions(
        &self,
        project_id: &str,
        game_version: Option<&str>,
        loader: Option<&str>,
    ) -> Result<Vec<ModVersion>> {
        let url = format!("{}/project/{}/version", Self::BASE_URL, project_id);

        let mut query_params: Vec<(&str, String)> = Vec::new();

        if let Some(version) = game_version {
            query_params.push(("game_versions", format!(r#"["{}"]"#, version)));
        }
        if let Some(l) = loader {
            query_params.push(("loaders", format!(r#"["{}"]"#, l)));
        }

        let response = self
            .client
            .get(&url)
            .query(&query_params)
            .send()
            .await
            .context("Failed to get versions")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Modrinth API error: {} - {}", status, body);
        }

        let versions: Vec<ModrinthVersion> = response
            .json()
            .await
            .context("Failed to parse versions response")?;

        let result = versions
            .into_iter()
            .filter_map(|v| {
                // Get the primary file
                let primary_file = v.files.into_iter().find(|f| f.primary)?;

                Some(ModVersion {
                    id: v.id,
                    version_number: v.version_number,
                    mod_name: v.name,
                    game_versions: v.game_versions,
                    loaders: v.loaders,
                    download_url: primary_file.url,
                    filename: primary_file.filename,
                    file_size: primary_file.size,
                    date_published: v.date_published,
                    dependencies: v
                        .dependencies
                        .into_iter()
                        .filter_map(|d| {
                            let project_id = d.project_id?;
                            let dep_type = match d.dependency_type.as_str() {
                                "required" => DependencyType::Required,
                                "optional" => DependencyType::Optional,
                                "incompatible" => DependencyType::Incompatible,
                                "embedded" => DependencyType::Embedded,
                                _ => return None,
                            };
                            Some(ModDependency {
                                project_id,
                                dependency_type: dep_type,
                                name: None,
                            })
                        })
                        .collect(),
                })
            })
            .collect();

        Ok(result)
    }

    /// Get project details
    pub async fn get_project(&self, id_or_slug: &str) -> Result<ModSearchResult> {
        let url = format!("{}/project/{}", Self::BASE_URL, id_or_slug);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to get project")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Modrinth API error: {} - {}", status, body);
        }

        let project: ModrinthProject = response
            .json()
            .await
            .context("Failed to parse project response")?;

        Ok(ModSearchResult {
            id: project.id,
            source: ModSource::Modrinth,
            name: project.title,
            slug: project.slug.clone(),
            description: project.description,
            author: String::new(), // Would need separate API call
            downloads: project.downloads,
            icon_url: project.icon_url,
            page_url: format!("https://modrinth.com/mod/{}", project.slug),
            game_versions: project.game_versions,
            loaders: project.loaders,
        })
    }
}

impl Default for ModrinthClient {
    fn default() -> Self {
        Self::new()
    }
}

// Modrinth API response types

#[derive(Debug, Deserialize)]
struct ModrinthSearchResponse {
    hits: Vec<ModrinthSearchHit>,
    total_hits: u64,
    offset: u32,
    limit: u32,
}

#[derive(Debug, Deserialize)]
struct ModrinthSearchHit {
    project_id: String,
    title: String,
    slug: String,
    description: String,
    author: String,
    downloads: u64,
    icon_url: Option<String>,
    versions: Vec<String>,
    categories: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ModrinthProject {
    id: String,
    title: String,
    slug: String,
    description: String,
    downloads: u64,
    icon_url: Option<String>,
    game_versions: Vec<String>,
    loaders: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ModrinthVersion {
    id: String,
    name: String,
    version_number: String,
    game_versions: Vec<String>,
    loaders: Vec<String>,
    files: Vec<ModrinthFile>,
    dependencies: Vec<ModrinthDependency>,
    date_published: String,
}

#[derive(Debug, Deserialize)]
struct ModrinthFile {
    url: String,
    filename: String,
    primary: bool,
    size: u64,
}

#[derive(Debug, Deserialize)]
struct ModrinthDependency {
    project_id: Option<String>,
    dependency_type: String,
}
