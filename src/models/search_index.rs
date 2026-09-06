use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use crate::models::nuspec::Nuspec;

#[derive(Deserialize, Serialize, ToSchema, Default)]
pub struct SearchIndex {
    #[serde(rename = "totalHits")]
    pub total_hits: u32,
    pub data: Vec<SearchIndexItem>
}

#[derive(Deserialize, Serialize, ToSchema)]
pub struct SearchIndexItem {
    #[serde(rename = "@id")]
    pub at_id: String,
    #[serde(rename = "@type")]
    pub at_type: String,
    pub registration: String,
    pub id: String,
    pub version: String,
    pub description: String,
    pub authors: Vec<String>,
    pub tags: Vec<String>,
    #[serde(rename = "totalDownloads")]
    pub total_downloads: u32,
    pub verified: bool,
    pub versions: Vec<SearchVersion>
}

#[derive(Deserialize, Serialize, ToSchema)]
pub struct SearchVersion {
    #[serde(rename = "@id")]
    pub at_id: String,
    pub version: String,
    pub downloads: u32,
}

impl SearchIndex {
    pub fn new(base_url: &str, package: &[Nuspec], skip: Option<usize>, take: Option<usize>, prerelease: Option<bool>) -> SearchIndex {
        if package.is_empty() {
            return SearchIndex::default();
        }

        let mut package = package.to_vec();
        package.sort_by(|a, b| a.version.cmp(&b.version));

        let prerelease = prerelease.unwrap_or_else(|| false);

        let mut map: HashMap<String, Vec<Nuspec>> = HashMap::new();
        for item in package {
            if !prerelease && item.version.prerelease.is_some() {
                continue;//skip prerelease items when false
            }
            map.entry(item.id.to_lowercase().clone()).or_default().push(item.clone());
        }

        let skip = skip.unwrap_or_else(|| 0);
        let take = take.unwrap_or_else(|| 20);

        let data: Vec<SearchIndexItem> = map.iter()
            .map(|(k,v)| Self::create_index_item(base_url, k, v))
            .skip(skip)
            .take(take)
            .collect();

        SearchIndex {
            total_hits: map.len() as u32,
            data
        }
    }

    fn create_index_item(base_url: &str, package_id: &str, package: &[Nuspec]) -> SearchIndexItem {
        let mut package: Vec<Nuspec> = package.to_vec();
        package.sort_by(|a, b| a.version.cmp(&b.version));
        let latest = package.last().unwrap().clone();

        SearchIndexItem {
            at_id: format!("{}/v3/registration/{}/index.json", base_url, package_id),
            at_type: "Package".to_string(),
            registration: format!("{}/v3/registration/{}/index.json", base_url, package_id),
            id: package_id.to_string(),
            version: latest.version.to_string(),
            description: latest.description,
            authors: latest.authors.split(',').map(|m|m.to_string()).collect(),
            tags: latest.tags,
            total_downloads: 31415265,
            verified: true,
            versions: Self::create_search_versions(base_url, package_id, package)
        }
    }

    fn create_search_versions(base_url: &str, package_id: &str, package: Vec<Nuspec>) -> Vec<SearchVersion> {
        package.iter().map(|m| {
            SearchVersion {
                version: m.version.to_string(),
                downloads: 31415265,
                at_id: format!("{}/v3/registration/{}/{}.json", base_url, package_id, m.version.to_string()),
            }
        }).collect()
    }
}