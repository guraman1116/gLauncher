//! CurseForge API client
//!
//! Client for interacting with the CurseForge API.
//! Requires an API key from https://console.curseforge.com

use super::search::{
    DependencyType, ModDependency, ModSearchResult, ModSource, ModVersion, SearchFilter,
    SearchResults,
};
use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;

/// CurseForge API client
pub struct CurseForgeClient {
    client: Client,
    api_key: String,
}

impl CurseForgeClient {
    const BASE_URL: &'static str = "https://api.curseforge.com/v1";
    const MINECRAFT_GAME_ID: u32 = 432;
    const MOD_CLASS_ID: u32 = 6;

    /// Create a new CurseForge client with the given API key
    pub fn new(api_key: String) -> Self {
        let client = Client::builder()
            .build()
            .expect("Failed to create HTTP client");

        Self { client, api_key }
    }

    /// Check if the client has a valid API key
    pub fn has_api_key(&self) -> bool {
        !self.api_key.is_empty()
    }

    /// Update the API key
    pub fn set_api_key(&mut self, api_key: String) {
        self.api_key = api_key;
    }

    /// Search for mods
    pub async fn search(&self, filter: &SearchFilter) -> Result<SearchResults> {
        if !self.has_api_key() {
            anyhow::bail!("CurseForge API key not configured");
        }

        let url = format!("{}/mods/search", Self::BASE_URL);

        let mut query_params: Vec<(&str, String)> = vec![
            ("gameId", Self::MINECRAFT_GAME_ID.to_string()),
            ("classId", Self::MOD_CLASS_ID.to_string()),
            ("searchFilter", filter.query.clone()),
            ("pageSize", filter.limit.to_string()),
            ("index", filter.offset.to_string()),
            ("sortField", "2".to_string()), // Popularity
            ("sortOrder", "desc".to_string()),
        ];

        if let Some(ref version) = filter.game_version {
            query_params.push(("gameVersion", version.clone()));
        }

        if let Some(ref loader) = filter.loader {
            // CurseForge uses modLoaderType: 1=Forge, 4=Fabric, 5=Quilt, 6=NeoForge
            let loader_type = match loader.to_lowercase().as_str() {
                "forge" => "1",
                "fabric" => "4",
                "quilt" => "5",
                "neoforge" => "6",
                _ => "",
            };
            if !loader_type.is_empty() {
                query_params.push(("modLoaderType", loader_type.to_string()));
            }
        }

        let response = self
            .client
            .get(&url)
            .header("x-api-key", &self.api_key)
            .query(&query_params)
            .send()
            .await
            .context("Failed to send search request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("CurseForge API error: {} - {}", status, body);
        }

        let search_response: CurseForgeSearchResponse = response
            .json()
            .await
            .context("Failed to parse search response")?;

        let hits = search_response
            .data
            .into_iter()
            .map(|mod_data| {
                let author = mod_data
                    .authors
                    .first()
                    .map(|a| a.name.clone())
                    .unwrap_or_default();

                let loaders: Vec<String> = mod_data
                    .latest_files_indexes
                    .iter()
                    .filter_map(|f| f.mod_loader.clone())
                    .collect();

                ModSearchResult {
                    id: mod_data.id.to_string(),
                    source: ModSource::CurseForge,
                    name: mod_data.name,
                    slug: mod_data.slug.clone(),
                    description: mod_data.summary,
                    author,
                    downloads: mod_data.download_count as u64,
                    icon_url: mod_data.logo.map(|l| l.url),
                    page_url: format!(
                        "https://www.curseforge.com/minecraft/mc-mods/{}",
                        mod_data.slug
                    ),
                    game_versions: mod_data
                        .latest_files_indexes
                        .iter()
                        .map(|f| f.game_version.clone())
                        .collect(),
                    loaders,
                }
            })
            .collect();

        Ok(SearchResults {
            hits,
            total_hits: search_response.pagination.total_count,
            offset: search_response.pagination.index,
            limit: search_response.pagination.page_size,
        })
    }

    /// Get files for a mod
    pub async fn get_files(
        &self,
        mod_id: &str,
        game_version: Option<&str>,
        loader: Option<&str>,
    ) -> Result<Vec<ModVersion>> {
        if !self.has_api_key() {
            anyhow::bail!("CurseForge API key not configured");
        }

        let url = format!("{}/mods/{}/files", Self::BASE_URL, mod_id);

        let mut query_params: Vec<(&str, String)> = vec![("pageSize", "50".to_string())];

        if let Some(version) = game_version {
            query_params.push(("gameVersion", version.to_string()));
        }

        if let Some(l) = loader {
            let loader_type = match l.to_lowercase().as_str() {
                "forge" => "1",
                "fabric" => "4",
                "quilt" => "5",
                "neoforge" => "6",
                _ => "",
            };
            if !loader_type.is_empty() {
                query_params.push(("modLoaderType", loader_type.to_string()));
            }
        }

        let response = self
            .client
            .get(&url)
            .header("x-api-key", &self.api_key)
            .query(&query_params)
            .send()
            .await
            .context("Failed to get files")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("CurseForge API error: {} - {}", status, body);
        }

        let files_response: CurseForgeFilesResponse = response
            .json()
            .await
            .context("Failed to parse files response")?;

        let result = files_response
            .data
            .into_iter()
            .map(|file| {
                let download_url = file.download_url.unwrap_or_else(|| {
                    // Fallback URL construction if download_url is null
                    format!(
                        "https://edge.forgecdn.net/files/{}/{}/{}",
                        file.id / 1000,
                        file.id % 1000,
                        file.file_name
                    )
                });

                let loaders: Vec<String> = file
                    .game_versions
                    .iter()
                    .filter(|v| {
                        let lower = v.to_lowercase();
                        lower == "forge"
                            || lower == "fabric"
                            || lower == "quilt"
                            || lower == "neoforge"
                    })
                    .cloned()
                    .collect();

                let game_versions: Vec<String> = file
                    .game_versions
                    .iter()
                    .filter(|v| {
                        let lower = v.to_lowercase();
                        lower != "forge"
                            && lower != "fabric"
                            && lower != "quilt"
                            && lower != "neoforge"
                    })
                    .cloned()
                    .collect();

                ModVersion {
                    id: file.id.to_string(),
                    version_number: file.display_name,
                    mod_name: file.file_name.clone(),
                    game_versions,
                    loaders,
                    download_url,
                    filename: file.file_name,
                    file_size: file.file_length,
                    date_published: file.file_date,
                    dependencies: file
                        .dependencies
                        .into_iter()
                        .filter_map(|d| {
                            let dep_type = match d.relation_type {
                                1 => DependencyType::Embedded,
                                2 => DependencyType::Optional,
                                3 => DependencyType::Required,
                                4 => DependencyType::Incompatible,
                                _ => return None,
                            };
                            Some(ModDependency {
                                project_id: d.mod_id.to_string(),
                                dependency_type: dep_type,
                                name: None,
                            })
                        })
                        .collect(),
                }
            })
            .collect();

        Ok(result)
    }
}

// CurseForge API response types

#[derive(Debug, Deserialize)]
struct CurseForgeSearchResponse {
    data: Vec<CurseForgeModData>,
    pagination: CurseForgePagination,
}

#[derive(Debug, Deserialize)]
struct CurseForgeFilesResponse {
    data: Vec<CurseForgeFile>,
    #[allow(dead_code)]
    pagination: CurseForgePagination,
}

#[derive(Debug, Deserialize)]
struct CurseForgePagination {
    index: u32,
    #[serde(rename = "pageSize")]
    page_size: u32,
    #[serde(rename = "totalCount")]
    total_count: u64,
}

#[derive(Debug, Deserialize)]
struct CurseForgeModData {
    id: u64,
    name: String,
    slug: String,
    summary: String,
    #[serde(rename = "downloadCount")]
    download_count: f64,
    authors: Vec<CurseForgeAuthor>,
    logo: Option<CurseForgeLogo>,
    #[serde(rename = "latestFilesIndexes")]
    latest_files_indexes: Vec<CurseForgeFileIndex>,
}

#[derive(Debug, Deserialize)]
struct CurseForgeAuthor {
    name: String,
}

#[derive(Debug, Deserialize)]
struct CurseForgeLogo {
    url: String,
}

#[derive(Debug, Deserialize)]
struct CurseForgeFileIndex {
    #[serde(rename = "gameVersion")]
    game_version: String,
    #[serde(rename = "modLoader")]
    mod_loader: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CurseForgeFile {
    id: u64,
    #[serde(rename = "displayName")]
    display_name: String,
    #[serde(rename = "fileName")]
    file_name: String,
    #[serde(rename = "fileLength")]
    file_length: u64,
    #[serde(rename = "fileDate")]
    file_date: String,
    #[serde(rename = "downloadUrl")]
    download_url: Option<String>,
    #[serde(rename = "gameVersions")]
    game_versions: Vec<String>,
    dependencies: Vec<CurseForgeDependency>,
}

#[derive(Debug, Deserialize)]
struct CurseForgeDependency {
    #[serde(rename = "modId")]
    mod_id: u64,
    #[serde(rename = "relationType")]
    relation_type: u32,
}
