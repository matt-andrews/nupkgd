use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use crate::models::app_state::AppState;
use crate::models::nuget_version::NugetVersion;

#[derive(Deserialize, Serialize, ToSchema)]
pub struct PackageIndex {
    pub versions: Vec<String>
}

impl PackageIndex {
    pub async fn new(archive: &AppState, package_id: &str) -> Option<PackageIndex> {
        let package_id = package_id.to_lowercase();
        let mut package: Vec<NugetVersion> = archive
            .find_package(&package_id).await.unwrap_or_default()
            .iter()
            .map(|m| m.version.clone())
            .collect();
        package.sort();
        package.reverse();

        let result: Vec<String> = package.iter().map(|m|m.to_string()).collect();

        if !result.is_empty() {
            return Some(PackageIndex { versions: result })
        }

        None
    }
}