use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use crate::models::dependency_group::DependencyGroup;
use crate::models::nuspec::Nuspec;
use crate::models::nuget_version::NugetVersion;

#[derive(Deserialize, Serialize, ToSchema)]
pub struct RegistrationIndex {
    #[serde(rename = "@id")]
    pub at_id: String,
    pub count: u16,
    pub items: Vec<RegistrationPage>
}

#[derive(Deserialize, Serialize, ToSchema)]
pub struct RegistrationPage {
    #[serde(rename = "@id")]
    pub at_id: String,
    pub count: u16,
    pub lower: String,
    pub upper: String,
    pub items: Vec<RegistrationVersion>,
}

#[derive(Deserialize, Serialize, ToSchema)]
pub struct RegistrationVersion{
    #[serde(rename = "@id")]
    pub at_id: String,
    #[serde(rename = "packageContent")]
    pub package_content: String,
    #[serde(rename = "catalogEntry")]
    pub catalog_entry: RegistrationCatalogEntry,

}

#[derive(Deserialize, Serialize, ToSchema)]
pub struct RegistrationCatalogEntry{
    #[serde(rename = "@id")]
    pub at_id: String,
    pub id: String,
    pub version: String,
    pub description: String,
    pub authors: String,
    pub listed: bool,
    pub published: String,
    #[serde(rename = "packageContent")]
    pub package_content: String,
    #[serde(rename = "projectUrl")]
    pub project_url: String,
    #[serde(rename = "licenseUrl")]
    pub license_url: String,
    pub tags: Vec<String>,
    #[serde(rename = "requireLicenseAcceptance")]
    pub require_license_acceptance: bool,
    #[serde(rename = "dependencyGroups")]
    pub dependency_groups: Vec<DependencyGroup>,
}

impl RegistrationIndex {
    pub fn new(base_url: &str, package_id: &str, package: &[Nuspec]) -> Option<RegistrationIndex> {
        if package.is_empty() {
            return None;
        }

        Some(RegistrationIndex {
            at_id: format!("{}/v3/registration/{}/index.json", base_url, package_id),
            count: 1,
            items: vec![Self::create_page(base_url, package_id, package)]
        })
    }

    fn create_page(base_url: &str, package_id: &str, package: &[Nuspec]) -> RegistrationPage {
        let mut versions: Vec<NugetVersion> = package.iter().map(|m|m.version.clone()).collect();
        versions.sort();

        let default_version = &NugetVersion::default();
        let lower_version = versions.first().unwrap_or(default_version);
        let upper_version = versions.last().unwrap_or(default_version);

        let items: Vec<RegistrationVersion> = package
            .iter()
            .map(|m|Self::create_version(base_url, package_id, m))
            .collect();

        RegistrationPage {
            at_id: format!("{}/v3/registration/{}/index.json#page/{}/{}", base_url, package_id, lower_version, upper_version),
            lower: lower_version.to_string(),
            upper: upper_version.to_string(),
            count: items.len() as u16,
            items
        }
    }

    fn create_version(base_url: &str, package_id: &str, package: &Nuspec) -> RegistrationVersion {
        RegistrationVersion {
            at_id: format!("{}/v3/registration/{}/{}.json", base_url, package_id, package.version),
            package_content: format!("{}/v3/package/{}/{}/{}.{}.nupkg", base_url, package_id, package.version, package_id, package.version),
            catalog_entry: Self::create_catalog(base_url, package_id, package)
        }
    }

    fn create_catalog(base_url: &str, package_id: &str, package: &Nuspec) -> RegistrationCatalogEntry {
        let package = package.clone();
        RegistrationCatalogEntry {
            at_id: format!("{}/v3/registration/{}/{}.json", base_url, package_id, package.version),
            id: package.id,//original id casing
            version: package.version.to_string(),
            description: package.description,
            authors: package.authors,
            listed: true,
            published: package.published.timestamp().to_string(),
            package_content: format!("{}/v3/package/{}/{}/{}.{}.nupkg", base_url, package_id, package.version, package_id, package.version),
            project_url: package.project_url.unwrap_or_default(),
            license_url: package.license_url.unwrap_or_default(),
            tags: package.tags,
            require_license_acceptance: package.require_license_acceptance,
            dependency_groups: package.dependency_groups,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::dependency_group::Dependency;
    use chrono::{DateTime, Utc};

    const BASE_URL: &str = "http://localhost:5555";

    fn nuspec(id: &str, version: &str) -> Nuspec {
        Nuspec {
            id: id.to_string(),
            version: NugetVersion::parse(version).unwrap(),
            description: format!("{id} {version}"),
            authors: "Matthew Andrews".to_string(),
            tags: vec!["tag".to_string()],
            published: DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap(),
            ..Nuspec::default()
        }
    }

    #[test]
    fn test_new_with_empty_returns_none() {
        assert!(RegistrationIndex::new(BASE_URL, "foo", &[]).is_none());
    }

    #[test]
    fn test_new_builds_single_page_index() {
        let pkg = vec![nuspec("Foo", "1.0.0")];
        let index = RegistrationIndex::new(BASE_URL, "foo", &pkg).unwrap();
        assert_eq!(index.at_id, "http://localhost:5555/v3/registration/foo/index.json");
        assert_eq!(index.count, 1);
        assert_eq!(index.items.len(), 1);
    }

    #[test]
    fn test_page_bounds_use_sorted_versions() {
        let pkg = vec![
            nuspec("Foo", "1.1.0"),
            nuspec("Foo", "0.1.0"),
            nuspec("Foo", "2.0.0-beta"),
            nuspec("Foo", "0.1.1"),
        ];
        let index = RegistrationIndex::new(BASE_URL, "foo", &pkg).unwrap();
        let page = &index.items[0];
        assert_eq!(page.lower, "0.1.0");
        assert_eq!(page.upper, "2.0.0-beta");
        assert_eq!(page.count, 4);
        assert_eq!(page.items.len(), 4);
        assert_eq!(page.at_id, "http://localhost:5555/v3/registration/foo/index.json#page/0.1.0/2.0.0-beta");
    }

    #[test]
    fn test_page_bounds_with_single_version_are_equal() {
        let pkg = vec![nuspec("Foo", "3.2.1")];
        let index = RegistrationIndex::new(BASE_URL, "foo", &pkg).unwrap();
        let page = &index.items[0];
        assert_eq!(page.lower, "3.2.1");
        assert_eq!(page.upper, "3.2.1");
        assert_eq!(page.at_id, "http://localhost:5555/v3/registration/foo/index.json#page/3.2.1/3.2.1");
    }

    #[test]
    fn test_version_urls_use_requested_package_id() {
        let pkg = vec![nuspec("Foo.Bar", "1.2.3")];
        let index = RegistrationIndex::new(BASE_URL, "foo.bar", &pkg).unwrap();
        let version = &index.items[0].items[0];
        assert_eq!(version.at_id, "http://localhost:5555/v3/registration/foo.bar/1.2.3.json");
        assert_eq!(version.package_content, "http://localhost:5555/v3/package/foo.bar/1.2.3/foo.bar.1.2.3.nupkg");
        assert_eq!(version.catalog_entry.at_id, version.at_id);
        assert_eq!(version.catalog_entry.package_content, version.package_content);
    }

    #[test]
    fn test_catalog_entry_copies_nuspec_metadata() {
        let mut n = nuspec("Foo.Bar", "1.2.3");
        n.project_url = Some("https://example.com/project".to_string());
        n.license_url = Some("https://licenses.nuget.org/MIT".to_string());
        n.require_license_acceptance = true;
        n.dependency_groups = vec![DependencyGroup {
            target_framework: Some("net10.0".to_string()),
            dependencies: vec![Dependency { id: "Dep".to_string(), range: "2.0.0".to_string() }],
        }];

        let index = RegistrationIndex::new(BASE_URL, "foo.bar", &[n]).unwrap();
        let entry = &index.items[0].items[0].catalog_entry;
        assert_eq!(entry.id, "Foo.Bar", "catalog id should keep original nuspec casing");
        assert_eq!(entry.version, "1.2.3");
        assert_eq!(entry.description, "Foo.Bar 1.2.3");
        assert_eq!(entry.authors, "Matthew Andrews");
        assert_eq!(entry.tags, vec!["tag"]);
        assert!(entry.listed);
        assert_eq!(entry.published, "1700000000");
        assert_eq!(entry.project_url, "https://example.com/project");
        assert_eq!(entry.license_url, "https://licenses.nuget.org/MIT");
        assert!(entry.require_license_acceptance);
        assert_eq!(entry.dependency_groups.len(), 1);
        assert_eq!(entry.dependency_groups[0].target_framework.as_deref(), Some("net10.0"));
        assert_eq!(entry.dependency_groups[0].dependencies[0].id, "Dep");
        assert_eq!(entry.dependency_groups[0].dependencies[0].range, "2.0.0");
    }

    #[test]
    fn test_catalog_entry_defaults_missing_urls_to_empty() {
        let pkg = vec![nuspec("Foo", "1.0.0")];
        let index = RegistrationIndex::new(BASE_URL, "foo", &pkg).unwrap();
        let entry = &index.items[0].items[0].catalog_entry;
        assert_eq!(entry.project_url, "");
        assert_eq!(entry.license_url, "");
        assert!(!entry.require_license_acceptance);
        assert!(entry.dependency_groups.is_empty());
    }

    #[test]
    fn test_serialize_uses_nuget_field_names() {
        let pkg = vec![nuspec("Foo", "1.0.0")];
        let index = RegistrationIndex::new(BASE_URL, "foo", &pkg).unwrap();
        let json: serde_json::Value = serde_json::from_str(&serde_json::to_string(&index).unwrap()).unwrap();

        assert_eq!(json["@id"], "http://localhost:5555/v3/registration/foo/index.json");
        assert_eq!(json["count"], 1);

        let page = &json["items"][0];
        assert_eq!(page["@id"], "http://localhost:5555/v3/registration/foo/index.json#page/1.0.0/1.0.0");
        assert_eq!(page["lower"], "1.0.0");
        assert_eq!(page["upper"], "1.0.0");

        let version = &page["items"][0];
        assert_eq!(version["@id"], "http://localhost:5555/v3/registration/foo/1.0.0.json");
        assert_eq!(version["packageContent"], "http://localhost:5555/v3/package/foo/1.0.0/foo.1.0.0.nupkg");

        let entry = &version["catalogEntry"];
        assert_eq!(entry["@id"], version["@id"]);
        assert_eq!(entry["packageContent"], version["packageContent"]);
        assert_eq!(entry["projectUrl"], "");
        assert_eq!(entry["licenseUrl"], "");
        assert_eq!(entry["requireLicenseAcceptance"], false);
        assert!(entry["dependencyGroups"].as_array().unwrap().is_empty());
        assert!(entry.get("package_content").is_none());
        assert!(entry.get("catalog_entry").is_none());
    }
}
