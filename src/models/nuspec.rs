use crate::models::dependency_group::{Dependency, DependencyGroup};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use chrono::{DateTime, Utc};
use zip::{ZipArchive};
use crate::models::nuget_version::NugetVersion;

#[derive(Clone, Default, Debug)]
pub struct Nuspec {
    pub file: PathBuf,
    pub xml: String,
    pub id: String,
    pub description: String,
    pub authors: String,
    pub tags: Vec<String>,
    pub version: NugetVersion,
    pub project_url: Option<String>,
    pub license_url: Option<String>,
    pub license: Option<String>,
    pub require_license_acceptance: bool,
    pub dependency_groups: Vec<DependencyGroup>,
    pub published: DateTime<Utc>
}

impl Nuspec {
    pub fn match_id(&self, id: &str) -> bool {
        self.id.to_lowercase() == id.to_lowercase()
    }

    pub fn unpack(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let mut archive = ZipArchive::new(File::open(&path)?)?;

        let name = archive
            .file_names()
            .find(|n| n.ends_with(".nuspec"))
            .map(str::to_owned)
            .ok_or_else(|| anyhow::anyhow!("no .nuspec found in package"))?;

        let mut contents = String::new();
        archive.by_name(&name)?.read_to_string(&mut contents)?;

        Self::deserialize(&contents, path.as_ref().to_path_buf())

    }

    fn deserialize(s: &str, path: PathBuf) -> anyhow::Result<Self> {
        let doc = roxmltree::Document::parse(s)?;

        let id = Self::try_get_tag_text(&doc, "id").unwrap_or_default();
        let description = Self::try_get_tag_text(&doc, "description").unwrap_or_default();
        let authors = Self::try_get_tag_text(&doc, "authors").unwrap_or_default();
        let tags = Self::try_get_tag_text(&doc, "tags").unwrap_or_default()
            .split_whitespace().map(String::from).collect();

        let project_url = Self::try_get_tag_text(&doc, "projectUrl");
        let license_url = Self::try_get_tag_text(&doc, "licenseUrl");
        let license = Self::try_get_tag_text(&doc, "license");
        let version = Self::try_get_tag_text(&doc, "version").unwrap_or("1.0.0".to_string());

        Ok(Self{
            file: path,
            xml: s.to_string(),
            id,
            description,
            authors,
            tags,
            version: NugetVersion::parse(&version)?,
            project_url,
            license_url,
            license,
            dependency_groups: Self::parse_dependency_groups(&doc),
            require_license_acceptance: Self::try_get_tag_text(&doc, "requireLicenseAcceptance")
                .map(|s| s.trim().eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            published: Utc::now(),
        })
    }

    fn try_get_tag_text(doc: &roxmltree::Document, tag: &str) -> Option<String> {
        doc.descendants()
            .find(|n| n.has_tag_name(tag))
            .map(|n| n.text().unwrap_or_default().trim().to_owned())
    }

    fn parse_dependency_groups(doc: &roxmltree::Document) -> Vec<DependencyGroup> {
        let Some(deps) = doc.descendants().find(|n| n.has_tag_name("dependencies")) else {
            return Vec::new();
        };

        let mut groups: Vec<DependencyGroup> = deps
            .children()
            .filter(|n| n.has_tag_name("group"))
            .map(|g| DependencyGroup {
                target_framework: g
                    .attribute("targetFramework")
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned),
                dependencies: Self::parse_dependencies(g),
            })
            .collect();

        // Legacy flat form: <dependencies><dependency .../></dependencies>
        let flat = Self::parse_dependencies(deps);
        if !flat.is_empty() {
            groups.push(DependencyGroup { target_framework: None, dependencies: flat });
        }

        groups
    }

    fn parse_dependencies(parent: roxmltree::Node) -> Vec<Dependency> {
        parent
            .children()
            .filter(|n| n.has_tag_name("dependency"))
            .filter_map(|d| {
                Some(Dependency {
                    id: d.attribute("id")?.to_owned(),
                    // version is optional in the schema; absent means "any version"
                    range: d.attribute("version").unwrap_or_default().to_owned(),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::path::{PathBuf};
    use crate::models::nuspec::Nuspec;

    #[test]
    fn test_deserialize() {
        let test = r#"<?xml version="1.0" encoding="utf-8"?>
                            <package xmlns="http://schemas.microsoft.com/packaging/2013/05/nuspec.xsd">
                              <metadata>
                                <id>Schism.Consumer</id>
                                <version>0.3.9597.1896-debug</version>
                                <authors>Matthew Andrews</authors>
                                <license type="expression">MIT</license>
                                <licenseUrl>https://licenses.nuget.org/MIT</licenseUrl>
                                <description>MSBuild support for consuming Schism-generated SDK clients.</description>
                                <tags>sdk codegen aspnetcore http-client contract generator</tags>
                                <repository type="git" commit="5f30ac6a59b5714c4ba32d648935d6d02a705468" />
                                <dependencies>
                                  <group targetFramework="net10.0">
                                    <dependency id="Microsoft.Extensions.DependencyInjection.Abstractions" version="10.0.5" exclude="Build,Analyzers" />
                                    <dependency id="Microsoft.Extensions.Hosting.Abstractions" version="10.0.5" exclude="Build,Analyzers" />
                                    <dependency id="Microsoft.Extensions.Http" version="10.0.5" exclude="Build,Analyzers" />
                                  </group>
                                </dependencies>
                              </metadata>
                            </package>
        "#;
        let result = Nuspec::deserialize(test, PathBuf::from("/")).unwrap();
        assert_eq!(result.description, "MSBuild support for consuming Schism-generated SDK clients.");
        assert_eq!(result.authors, "Matthew Andrews");
        assert_eq!(result.license, Some("MIT".to_string()));
        assert_eq!(result.license_url, Some("https://licenses.nuget.org/MIT".to_string()));
        assert_eq!(result.project_url, None);
        let tags: Vec<&str> = vec!["sdk", "codegen", "aspnetcore", "http-client", "contract", "generator"];
        assert_eq!(result.tags, tags);
        assert_eq!(result.dependency_groups.len(), 1);
        let group = result.dependency_groups.first().unwrap();
        assert_eq!(group.target_framework.as_deref(), Some("net10.0"));
        assert_eq!(group.dependencies.len(), 3);
        assert_eq!(group.dependencies[0].id, "Microsoft.Extensions.DependencyInjection.Abstractions");
        assert_eq!(group.dependencies[1].id, "Microsoft.Extensions.Hosting.Abstractions");
        assert_eq!(group.dependencies[1].range, "10.0.5");
        assert_eq!(group.dependencies[2].id, "Microsoft.Extensions.Http");
        assert_eq!(group.dependencies[2].range, "10.0.5");
    }
}