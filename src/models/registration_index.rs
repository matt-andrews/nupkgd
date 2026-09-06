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