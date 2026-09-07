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
    use std::io::Write;
    use std::path::PathBuf;
    use chrono::Utc;
    use crate::models::nuspec::Nuspec;

    const NS: &str = "http://schemas.microsoft.com/packaging/2013/05/nuspec.xsd";

    /// Wraps `metadata` in a full nuspec document so tests only spell out the tags they care about.
    fn nuspec_xml(metadata: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="{NS}">
  <metadata>
    {metadata}
  </metadata>
</package>"#
        )
    }

    fn parse(metadata: &str) -> Nuspec {
        Nuspec::deserialize(&nuspec_xml(metadata), PathBuf::from("/pkg.nupkg")).unwrap()
    }

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tempest/fixtures").join(name)
    }

    /// Writes a throwaway zip archive containing the given (name, bytes) entries and returns its path.
    fn temp_archive(tag: &str, entries: &[(&str, &[u8])]) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "nupkgd-{tag}-{}-{:?}.nupkg",
            std::process::id(),
            std::thread::current().id()
        ));
        let file = std::fs::File::create(&path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        for (name, bytes) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
        path
    }

    // ---- deserialize: full document ----

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
        assert_eq!(result.id, "Schism.Consumer");
        assert_eq!(result.version.to_string(), "0.3.9597.1896-debug");
        assert_eq!(result.description, "MSBuild support for consuming Schism-generated SDK clients.");
        assert_eq!(result.authors, "Matthew Andrews");
        assert_eq!(result.license, Some("MIT".to_string()));
        assert_eq!(result.license_url, Some("https://licenses.nuget.org/MIT".to_string()));
        assert_eq!(result.project_url, None);
        assert!(!result.require_license_acceptance);
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

    #[test]
    fn test_deserialize_keeps_source_path_and_raw_xml() {
        let xml = nuspec_xml("<id>Foo</id><version>1.0.0</version>");
        let path = PathBuf::from("/feed/Foo.1.0.0.nupkg");
        let result = Nuspec::deserialize(&xml, path.clone()).unwrap();
        assert_eq!(result.file, path);
        assert_eq!(result.xml, xml);
    }

    #[test]
    fn test_deserialize_sets_published_to_now() {
        let before = Utc::now();
        let result = parse("<id>Foo</id><version>1.0.0</version>");
        let after = Utc::now();
        assert!(result.published >= before && result.published <= after);
    }

    // ---- deserialize: optional and defaulted fields ----

    #[test]
    fn test_deserialize_minimal_document_uses_defaults() {
        let result = parse("<id>Foo</id>");
        assert_eq!(result.id, "Foo");
        assert_eq!(result.version.to_string(), "1.0.0");
        assert_eq!(result.description, "");
        assert_eq!(result.authors, "");
        assert!(result.tags.is_empty());
        assert_eq!(result.project_url, None);
        assert_eq!(result.license_url, None);
        assert_eq!(result.license, None);
        assert!(!result.require_license_acceptance);
        assert!(result.dependency_groups.is_empty());
    }

    #[test]
    fn test_deserialize_missing_id_is_empty_string() {
        let result = parse("<version>1.0.0</version>");
        assert_eq!(result.id, "");
    }

    #[test]
    fn test_deserialize_reads_project_url() {
        let result = parse("<id>Foo</id><projectUrl>https://example.com/foo</projectUrl>");
        assert_eq!(result.project_url.as_deref(), Some("https://example.com/foo"));
    }

    #[test]
    fn test_deserialize_trims_surrounding_whitespace_from_text() {
        let result = parse(
            "<id>\n      Foo.Bar   \n</id>\
             <authors>  Alice  </authors>\
             <description>\n\tdesc\n</description>\
             <version>  2.0.0  </version>",
        );
        assert_eq!(result.id, "Foo.Bar");
        assert_eq!(result.authors, "Alice");
        assert_eq!(result.description, "desc");
        assert_eq!(result.version.to_string(), "2.0.0");
    }

    #[test]
    fn test_deserialize_empty_element_yields_empty_string() {
        let result = parse("<id>Foo</id><description/><license></license>");
        assert_eq!(result.description, "");
        assert_eq!(result.license, Some(String::new()));
    }

    // ---- deserialize: tags ----

    #[test]
    fn test_deserialize_tags_split_on_any_whitespace() {
        let result = parse("<id>Foo</id><tags>  one\ttwo\n  three   four </tags>");
        assert_eq!(result.tags, vec!["one", "two", "three", "four"]);
    }

    #[test]
    fn test_deserialize_empty_tags_element_gives_no_tags() {
        let result = parse("<id>Foo</id><tags></tags>");
        assert!(result.tags.is_empty());
        let result = parse("<id>Foo</id><tags>   </tags>");
        assert!(result.tags.is_empty());
    }

    // ---- deserialize: version ----

    #[test]
    fn test_deserialize_version_with_prerelease_and_metadata() {
        let result = parse("<id>Foo</id><version>1.2.3.4-beta.2+build.7</version>");
        assert_eq!(result.version.major, 1);
        assert_eq!(result.version.minor, 2);
        assert_eq!(result.version.patch, 3);
        assert_eq!(result.version.revision, 4);
        assert_eq!(result.version.prerelease.as_deref(), Some("beta.2"));
        assert_eq!(result.version.metadata.as_deref(), Some("build.7"));
    }

    #[test]
    fn test_deserialize_invalid_version_is_an_error() {
        let xml = nuspec_xml("<id>Foo</id><version>not-a-version</version>");
        assert!(Nuspec::deserialize(&xml, PathBuf::from("/")).is_err());
    }

    #[test]
    fn test_deserialize_empty_version_is_an_error() {
        let xml = nuspec_xml("<id>Foo</id><version></version>");
        assert!(Nuspec::deserialize(&xml, PathBuf::from("/")).is_err());
    }

    // ---- deserialize: requireLicenseAcceptance ----

    #[test]
    fn test_deserialize_require_license_acceptance_true_variants() {
        for value in ["true", "TRUE", "True", "  true  "] {
            let result = parse(&format!(
                "<id>Foo</id><requireLicenseAcceptance>{value}</requireLicenseAcceptance>"
            ));
            assert!(result.require_license_acceptance, "value {value:?} should be true");
        }
    }

    #[test]
    fn test_deserialize_require_license_acceptance_false_variants() {
        for value in ["false", "FALSE", "", "yes", "1"] {
            let result = parse(&format!(
                "<id>Foo</id><requireLicenseAcceptance>{value}</requireLicenseAcceptance>"
            ));
            assert!(!result.require_license_acceptance, "value {value:?} should be false");
        }
    }

    // ---- deserialize: malformed input ----

    #[test]
    fn test_deserialize_malformed_xml_is_an_error() {
        assert!(Nuspec::deserialize("<package><metadata><id>Foo</id>", PathBuf::from("/")).is_err());
        assert!(Nuspec::deserialize("", PathBuf::from("/")).is_err());
        assert!(Nuspec::deserialize("not xml at all", PathBuf::from("/")).is_err());
    }

    #[test]
    fn test_deserialize_document_without_metadata_uses_defaults() {
        let result = Nuspec::deserialize("<package/>", PathBuf::from("/")).unwrap();
        assert_eq!(result.id, "");
        assert_eq!(result.version.to_string(), "1.0.0");
        assert!(result.dependency_groups.is_empty());
    }

    // ---- deserialize: dependency groups ----

    #[test]
    fn test_dependencies_absent_gives_no_groups() {
        let result = parse("<id>Foo</id>");
        assert!(result.dependency_groups.is_empty());
    }

    #[test]
    fn test_dependencies_empty_element_gives_no_groups() {
        let result = parse("<id>Foo</id><dependencies/>");
        assert!(result.dependency_groups.is_empty());
        let result = parse("<id>Foo</id><dependencies>  </dependencies>");
        assert!(result.dependency_groups.is_empty());
    }

    #[test]
    fn test_dependencies_multiple_groups_preserve_order() {
        let result = parse(
            r#"<id>Foo</id>
            <dependencies>
              <group targetFramework="net10.0">
                <dependency id="A" version="1.0.0" />
              </group>
              <group targetFramework="netstandard2.0">
                <dependency id="B" version="[2.0.0, 3.0.0)" />
                <dependency id="C" version="1.5.0" />
              </group>
            </dependencies>"#,
        );
        assert_eq!(result.dependency_groups.len(), 2);

        let net10 = &result.dependency_groups[0];
        assert_eq!(net10.target_framework.as_deref(), Some("net10.0"));
        assert_eq!(net10.dependencies.len(), 1);
        assert_eq!(net10.dependencies[0].id, "A");
        assert_eq!(net10.dependencies[0].range, "1.0.0");

        let netstandard = &result.dependency_groups[1];
        assert_eq!(netstandard.target_framework.as_deref(), Some("netstandard2.0"));
        assert_eq!(netstandard.dependencies.len(), 2);
        assert_eq!(netstandard.dependencies[0].id, "B");
        assert_eq!(netstandard.dependencies[0].range, "[2.0.0, 3.0.0)");
        assert_eq!(netstandard.dependencies[1].id, "C");
    }

    #[test]
    fn test_dependencies_empty_group_is_kept_with_no_dependencies() {
        let result = parse(
            r#"<id>Foo</id>
            <dependencies>
              <group targetFramework="net10.0" />
            </dependencies>"#,
        );
        assert_eq!(result.dependency_groups.len(), 1);
        assert_eq!(result.dependency_groups[0].target_framework.as_deref(), Some("net10.0"));
        assert!(result.dependency_groups[0].dependencies.is_empty());
    }

    #[test]
    fn test_dependencies_group_without_target_framework_is_none() {
        let result = parse(
            r#"<id>Foo</id>
            <dependencies>
              <group>
                <dependency id="A" version="1.0.0" />
              </group>
            </dependencies>"#,
        );
        assert_eq!(result.dependency_groups.len(), 1);
        assert_eq!(result.dependency_groups[0].target_framework, None);
        assert_eq!(result.dependency_groups[0].dependencies.len(), 1);
    }

    #[test]
    fn test_dependencies_group_with_empty_target_framework_is_none() {
        let result = parse(
            r#"<id>Foo</id>
            <dependencies>
              <group targetFramework="">
                <dependency id="A" version="1.0.0" />
              </group>
            </dependencies>"#,
        );
        assert_eq!(result.dependency_groups.len(), 1);
        assert_eq!(result.dependency_groups[0].target_framework, None);
    }

    #[test]
    fn test_dependencies_legacy_flat_form_becomes_single_untargeted_group() {
        let result = parse(
            r#"<id>Foo</id>
            <dependencies>
              <dependency id="A" version="1.0.0" />
              <dependency id="B" version="2.0.0" />
            </dependencies>"#,
        );
        assert_eq!(result.dependency_groups.len(), 1);
        let group = &result.dependency_groups[0];
        assert_eq!(group.target_framework, None);
        assert_eq!(group.dependencies.len(), 2);
        assert_eq!(group.dependencies[0].id, "A");
        assert_eq!(group.dependencies[0].range, "1.0.0");
        assert_eq!(group.dependencies[1].id, "B");
        assert_eq!(group.dependencies[1].range, "2.0.0");
    }

    #[test]
    fn test_dependencies_mixed_groups_and_flat_puts_flat_last() {
        let result = parse(
            r#"<id>Foo</id>
            <dependencies>
              <dependency id="Flat" version="1.0.0" />
              <group targetFramework="net10.0">
                <dependency id="Grouped" version="1.0.0" />
              </group>
            </dependencies>"#,
        );
        assert_eq!(result.dependency_groups.len(), 2);
        assert_eq!(result.dependency_groups[0].target_framework.as_deref(), Some("net10.0"));
        assert_eq!(result.dependency_groups[0].dependencies[0].id, "Grouped");
        assert_eq!(result.dependency_groups[1].target_framework, None);
        assert_eq!(result.dependency_groups[1].dependencies[0].id, "Flat");
    }

    #[test]
    fn test_dependency_without_version_has_empty_range() {
        let result = parse(
            r#"<id>Foo</id>
            <dependencies>
              <group targetFramework="net10.0">
                <dependency id="AnyVersion" />
              </group>
            </dependencies>"#,
        );
        let dep = &result.dependency_groups[0].dependencies[0];
        assert_eq!(dep.id, "AnyVersion");
        assert_eq!(dep.range, "");
    }

    #[test]
    fn test_dependency_without_id_is_skipped() {
        let result = parse(
            r#"<id>Foo</id>
            <dependencies>
              <group targetFramework="net10.0">
                <dependency version="1.0.0" />
                <dependency id="Kept" version="1.0.0" />
              </group>
            </dependencies>"#,
        );
        let deps = &result.dependency_groups[0].dependencies;
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].id, "Kept");
    }

    #[test]
    fn test_dependencies_ignore_unrelated_child_elements() {
        let result = parse(
            r#"<id>Foo</id>
            <dependencies>
              <group targetFramework="net10.0">
                <dependency id="A" version="1.0.0" />
                <somethingElse id="B" version="1.0.0" />
              </group>
              <notAGroup targetFramework="net8.0" />
            </dependencies>"#,
        );
        assert_eq!(result.dependency_groups.len(), 1);
        assert_eq!(result.dependency_groups[0].dependencies.len(), 1);
        assert_eq!(result.dependency_groups[0].dependencies[0].id, "A");
    }

    #[test]
    fn test_dependency_attributes_beyond_id_and_version_are_ignored() {
        let result = parse(
            r#"<id>Foo</id>
            <dependencies>
              <group targetFramework="net10.0">
                <dependency id="A" version="1.0.0" include="All" exclude="Build,Analyzers" />
              </group>
            </dependencies>"#,
        );
        let dep = &result.dependency_groups[0].dependencies[0];
        assert_eq!(dep.id, "A");
        assert_eq!(dep.range, "1.0.0");
    }

    // ---- match_id ----

    #[test]
    fn test_match_id_is_case_insensitive() {
        let nuspec = parse("<id>Foo.Bar</id>");
        assert!(nuspec.match_id("Foo.Bar"));
        assert!(nuspec.match_id("foo.bar"));
        assert!(nuspec.match_id("FOO.BAR"));
        assert!(nuspec.match_id("fOo.BaR"));
    }

    #[test]
    fn test_match_id_rejects_different_ids() {
        let nuspec = parse("<id>Foo.Bar</id>");
        assert!(!nuspec.match_id("Foo"));
        assert!(!nuspec.match_id("Foo.Bar.Baz"));
        assert!(!nuspec.match_id(" Foo.Bar"));
        assert!(!nuspec.match_id(""));
    }

    #[test]
    fn test_match_id_with_empty_id_only_matches_empty() {
        let nuspec = parse("<version>1.0.0</version>");
        assert!(nuspec.match_id(""));
        assert!(!nuspec.match_id("Foo"));
    }

    // ---- unpack ----

    #[test]
    fn test_unpack_reads_nuspec_from_fixture_package() {
        let path = fixture("Other.Widget.0.1.0.nupkg");
        let result = Nuspec::unpack(&path).unwrap();

        assert_eq!(result.file, path);
        assert_eq!(result.id, "Other.Widget");
        assert_eq!(result.version.to_string(), "0.1.0");
        assert_eq!(result.authors, "Alice, Bob");
        assert_eq!(result.description, "A widget with every metadata field the feed exposes.");
        assert_eq!(result.tags, vec!["widgets", "tools"]);
        assert_eq!(result.project_url.as_deref(), Some("https://example.com/widget"));
        assert_eq!(result.license.as_deref(), Some("MIT"));
        assert_eq!(result.license_url.as_deref(), Some("https://licenses.nuget.org/MIT"));
        assert!(result.require_license_acceptance);
        assert!(result.xml.contains("<id>Other.Widget</id>"));

        assert_eq!(result.dependency_groups.len(), 2);
        let net10 = &result.dependency_groups[0];
        assert_eq!(net10.target_framework.as_deref(), Some("net10.0"));
        assert_eq!(net10.dependencies.len(), 2);
        assert_eq!(net10.dependencies[0].id, "Newtonsoft.Json");
        assert_eq!(net10.dependencies[0].range, "13.0.3");
        assert_eq!(net10.dependencies[1].id, "Microsoft.Extensions.Logging.Abstractions");
        assert_eq!(net10.dependencies[1].range, "10.0.0");
        let netstandard = &result.dependency_groups[1];
        assert_eq!(netstandard.target_framework.as_deref(), Some("netstandard2.0"));
        assert!(netstandard.dependencies.is_empty());
    }

    #[test]
    fn test_unpack_reads_prerelease_fixture_package() {
        let result = Nuspec::unpack(fixture("Solo.Prerelease.0.9.0-rc.1.nupkg")).unwrap();
        assert_eq!(result.id, "Solo.Prerelease");
        assert_eq!(result.version.to_string(), "0.9.0-rc.1");
        assert_eq!(result.version.prerelease.as_deref(), Some("rc.1"));
    }

    #[test]
    fn test_unpack_finds_nuspec_among_other_package_entries() {
        // Built by `dotnet pack`: the nuspec (with a UTF-8 BOM) sits alongside
        // _rels/, lib/, [Content_Types].xml and the core-properties entry.
        let result = Nuspec::unpack(fixture("recursive/Test.Ex.Pkg.1.3.1.nupkg")).unwrap();
        assert_eq!(result.id, "Test.Ex.Pkg");
        assert_eq!(result.version.to_string(), "1.3.1");
        assert_eq!(result.authors, "test.ex.pkg");
        assert_eq!(result.tags, vec!["sdk"]);
        assert_eq!(result.dependency_groups.len(), 1);
        assert_eq!(result.dependency_groups[0].target_framework.as_deref(), Some("net10.0"));
        assert!(result.dependency_groups[0].dependencies.is_empty());
    }

    #[test]
    fn test_unpack_of_non_zip_file_is_an_error() {
        assert!(Nuspec::unpack(fixture("Broken.Package.1.0.0.nupkg")).is_err());
    }

    #[test]
    fn test_unpack_of_missing_file_is_an_error() {
        assert!(Nuspec::unpack(fixture("Does.Not.Exist.1.0.0.nupkg")).is_err());
    }

    #[test]
    fn test_unpack_of_archive_without_nuspec_is_an_error() {
        let path = temp_archive(
            "no-nuspec",
            &[
                ("lib/net10.0/foo.dll", b"not really a dll"),
                ("readme.txt", b"no nuspec here"),
            ],
        );

        let result = Nuspec::unpack(&path);
        let _ = std::fs::remove_file(&path);

        let err = result.expect_err("archive without a .nuspec should fail to unpack");
        assert!(err.to_string().contains("no .nuspec found"), "unexpected error: {err}");
    }

    #[test]
    fn test_unpack_of_archive_with_invalid_nuspec_is_an_error() {
        let path = temp_archive(
            "bad-nuspec",
            &[("Bad.nuspec", b"<package><metadata><id>Bad</id>")],
        );

        let result = Nuspec::unpack(&path);
        let _ = std::fs::remove_file(&path);

        assert!(result.is_err());
    }

    #[test]
    fn test_unpack_of_archive_with_non_utf8_nuspec_is_an_error() {
        let path = temp_archive("bad-utf8", &[("Bad.nuspec", &[0xff, 0xfe, 0xfd])]);

        let result = Nuspec::unpack(&path);
        let _ = std::fs::remove_file(&path);

        assert!(result.is_err());
    }

    #[test]
    fn test_unpack_uses_nuspec_nested_in_a_subdirectory() {
        let path = temp_archive(
            "nested-nuspec",
            &[
                ("readme.txt", b"hello"),
                ("nested/Deep.nuspec", nuspec_xml("<id>Deep</id><version>4.5.6</version>").as_bytes()),
            ],
        );

        let result = Nuspec::unpack(&path);
        let _ = std::fs::remove_file(&path);

        let result = result.unwrap();
        assert_eq!(result.file, path);
        assert_eq!(result.id, "Deep");
        assert_eq!(result.version.to_string(), "4.5.6");
    }
}
