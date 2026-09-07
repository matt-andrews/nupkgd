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

        let prerelease = prerelease.unwrap_or(false);

        let mut map: HashMap<String, Vec<Nuspec>> = HashMap::new();
        for item in package {
            if !prerelease && item.version.prerelease.is_some() {
                continue;//skip prerelease items when false
            }
            map.entry(item.id.to_lowercase().clone()).or_default().push(item.clone());
        }

        let skip = skip.unwrap_or(0);
        let take = take.unwrap_or(20);

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
                at_id: format!("{}/v3/registration/{}/{}.json", base_url, package_id, m.version),
            }
        }).collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::nuget_version::NugetVersion;

    const BASE_URL: &str = "http://localhost:5555";

    fn nuspec(id: &str, version: &str) -> Nuspec {
        Nuspec {
            id: id.to_string(),
            version: NugetVersion::parse(version).unwrap(),
            description: format!("{id} {version}"),
            authors: "Matthew Andrews".to_string(),
            tags: vec!["tag".to_string()],
            ..Nuspec::default()
        }
    }

    fn find<'a>(index: &'a SearchIndex, id: &str) -> &'a SearchIndexItem {
        index.data.iter().find(|m| m.id == id).unwrap_or_else(|| panic!("no item with id {id}"))
    }

    #[test]
    fn test_new_with_empty_returns_default() {
        let index = SearchIndex::new(BASE_URL, &[], None, None, None);
        assert_eq!(index.total_hits, 0);
        assert!(index.data.is_empty());
    }

    #[test]
    fn test_new_groups_by_id_case_insensitively() {
        let pkg = vec![
            nuspec("Foo.Bar", "1.0.0"),
            nuspec("foo.bar", "1.1.0"),
            nuspec("FOO.BAR", "2.0.0"),
            nuspec("Baz", "0.1.0"),
        ];
        let index = SearchIndex::new(BASE_URL, &pkg, None, None, None);
        assert_eq!(index.total_hits, 2);
        assert_eq!(index.data.len(), 2);

        let foo = find(&index, "foo.bar");
        assert_eq!(foo.versions.len(), 3);
        let baz = find(&index, "baz");
        assert_eq!(baz.versions.len(), 1);
    }

    #[test]
    fn test_new_uses_latest_version_for_item_metadata() {
        let pkg = vec![
            nuspec("Foo", "0.1.0"),
            nuspec("Foo", "2.0.0"),
            nuspec("Foo", "1.5.0"),
        ];
        let index = SearchIndex::new(BASE_URL, &pkg, None, None, None);
        let foo = find(&index, "foo");
        assert_eq!(foo.version, "2.0.0");
        assert_eq!(foo.description, "Foo 2.0.0");
    }

    #[test]
    fn test_new_versions_sorted_ascending() {
        let pkg = vec![
            nuspec("Foo", "1.1.0"),
            nuspec("Foo", "0.1.0"),
            nuspec("Foo", "0.1.1"),
        ];
        let index = SearchIndex::new(BASE_URL, &pkg, None, None, None);
        let versions: Vec<&str> = find(&index, "foo").versions.iter().map(|v| v.version.as_str()).collect();
        assert_eq!(versions, vec!["0.1.0", "0.1.1", "1.1.0"]);
    }

    #[test]
    fn test_new_excludes_prerelease_by_default() {
        let pkg = vec![
            nuspec("Foo", "1.0.0"),
            nuspec("Foo", "2.0.0-beta"),
        ];
        let index = SearchIndex::new(BASE_URL, &pkg, None, None, None);
        let foo = find(&index, "foo");
        assert_eq!(foo.version, "1.0.0");
        assert_eq!(foo.versions.len(), 1);
        assert_eq!(foo.versions[0].version, "1.0.0");
    }

    #[test]
    fn test_new_excludes_prerelease_when_false() {
        let pkg = vec![
            nuspec("Foo", "1.0.0"),
            nuspec("Foo", "2.0.0-beta"),
        ];
        let index = SearchIndex::new(BASE_URL, &pkg, None, None, Some(false));
        let foo = find(&index, "foo");
        assert_eq!(foo.version, "1.0.0");
        assert_eq!(foo.versions.len(), 1);
    }

    #[test]
    fn test_new_includes_prerelease_when_true() {
        let pkg = vec![
            nuspec("Foo", "1.0.0"),
            nuspec("Foo", "2.0.0-beta"),
        ];
        let index = SearchIndex::new(BASE_URL, &pkg, None, None, Some(true));
        let foo = find(&index, "foo");
        assert_eq!(foo.version, "2.0.0-beta");
        let versions: Vec<&str> = foo.versions.iter().map(|v| v.version.as_str()).collect();
        assert_eq!(versions, vec!["1.0.0", "2.0.0-beta"]);
    }

    #[test]
    fn test_new_prerelease_only_package_is_hidden_without_prerelease() {
        let pkg = vec![
            nuspec("Stable", "1.0.0"),
            nuspec("PreOnly", "1.0.0-alpha"),
        ];
        let index = SearchIndex::new(BASE_URL, &pkg, None, None, None);
        assert_eq!(index.total_hits, 1);
        assert_eq!(index.data.len(), 1);
        assert_eq!(index.data[0].id, "stable");
    }

    #[test]
    fn test_new_skip_and_take_page_results() {
        let pkg: Vec<Nuspec> = (0..5).map(|i| nuspec(&format!("Pkg{i}"), "1.0.0")).collect();

        let page = SearchIndex::new(BASE_URL, &pkg, Some(1), Some(2), None);
        assert_eq!(page.total_hits, 5, "total_hits should reflect all packages, not the page size");
        assert_eq!(page.data.len(), 2);

        let all = SearchIndex::new(BASE_URL, &pkg, None, None, None);
        assert_eq!(all.data.len(), 5);

        let skipped_past_end = SearchIndex::new(BASE_URL, &pkg, Some(10), None, None);
        assert_eq!(skipped_past_end.total_hits, 5);
        assert!(skipped_past_end.data.is_empty());
    }

    #[test]
    fn test_new_default_take_is_20() {
        let pkg: Vec<Nuspec> = (0..25).map(|i| nuspec(&format!("Pkg{i}"), "1.0.0")).collect();
        let index = SearchIndex::new(BASE_URL, &pkg, None, None, None);
        assert_eq!(index.total_hits, 25);
        assert_eq!(index.data.len(), 20);
    }

    #[test]
    fn test_new_item_fields() {
        let pkg = vec![Nuspec {
            authors: "Alice,Bob".to_string(),
            tags: vec!["one".to_string(), "two".to_string()],
            ..nuspec("Foo.Bar", "1.2.3")
        }];
        let index = SearchIndex::new(BASE_URL, &pkg, None, None, None);
        let item = find(&index, "foo.bar");

        assert_eq!(item.at_id, "http://localhost:5555/v3/registration/foo.bar/index.json");
        assert_eq!(item.registration, "http://localhost:5555/v3/registration/foo.bar/index.json");
        assert_eq!(item.at_type, "Package");
        assert_eq!(item.id, "foo.bar");
        assert_eq!(item.version, "1.2.3");
        assert_eq!(item.authors, vec!["Alice", "Bob"]);
        assert_eq!(item.tags, vec!["one", "two"]);
        assert!(item.verified);

        assert_eq!(item.versions.len(), 1);
        assert_eq!(item.versions[0].version, "1.2.3");
        assert_eq!(item.versions[0].at_id, "http://localhost:5555/v3/registration/foo.bar/1.2.3.json");
    }

    #[test]
    fn test_serialize_uses_nuget_field_names() {
        let pkg = vec![nuspec("Foo", "1.0.0")];
        let index = SearchIndex::new(BASE_URL, &pkg, None, None, None);
        let json: serde_json::Value = serde_json::to_value(&index).unwrap();

        assert_eq!(json["totalHits"], 1);
        let item = &json["data"][0];
        assert_eq!(item["@id"], "http://localhost:5555/v3/registration/foo/index.json");
        assert_eq!(item["@type"], "Package");
        assert!(item["totalDownloads"].is_number());
        assert_eq!(item["versions"][0]["@id"], "http://localhost:5555/v3/registration/foo/1.0.0.json");
        assert!(item.get("total_hits").is_none());
        assert!(item.get("at_id").is_none());
    }
}
