use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use crate::models::nuget_version::NugetVersion;
use crate::models::nuspec::Nuspec;

#[derive(Deserialize, Serialize, ToSchema)]
pub struct PackageIndex {
    pub versions: Vec<String>
}

impl PackageIndex {
    pub fn new(package: &[Nuspec]) -> Option<PackageIndex> {
        let mut package: Vec<NugetVersion> = package
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_new_correct_order() {
        let pkg = vec![
            Nuspec{ version: NugetVersion::parse("0.1.0").unwrap(), ..Nuspec::default() },
            Nuspec{ version: NugetVersion::parse("1.1.0").unwrap(), ..Nuspec::default() },
            Nuspec{ version: NugetVersion::parse("0.1.1").unwrap(), ..Nuspec::default() },
        ];
        let index = PackageIndex::new(&pkg);
        assert!(index.is_some());
        assert_eq!(index.unwrap().versions, vec!["1.1.0", "0.1.1", "0.1.0"]);
    }

    #[test]
    fn test_new_with_none_returns_none() {
        let index = PackageIndex::new(&Vec::new());
        assert!(index.is_none());
    }
}