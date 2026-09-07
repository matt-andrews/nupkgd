use std::collections::BTreeMap;
use std::fmt::Write;
use crate::models::nuspec::Nuspec;

/// Renders the package index as a deliberately plain HTML page:
/// no CSS, no JS, just the tags a 1998 browser would understand.
pub fn render(base_url: &str, packages: &[Nuspec]) -> String {
    let mut groups: BTreeMap<String, Vec<Nuspec>> = BTreeMap::new();
    for item in packages {
        groups.entry(item.id.to_lowercase()).or_default().push(item.clone());
    }

    let mut html = String::new();
    html.push_str("<html><head><title>nupkgd</title></head><body>\n");
    html.push_str("<h1>Index of nupkgd</h1>\n<hr>\n");

    if groups.is_empty() {
        html.push_str("<p><i>No packages found.</i></p>\n");
    }

    let package_count = groups.len();

    for (id_lower, mut versions) in groups {
        versions.sort_by(|a, b| b.version.cmp(&a.version));

        let latest = versions
            .iter()
            .find(|v| v.version.prerelease.is_none())
            .unwrap_or(&versions[0])
            .clone();

        let _ = write!(html, "<h2>{}</h2>\n<p>", escape(&latest.id));
        let _ = write!(html, "{}<br>", escape(&latest.description));
        let _ = write!(html, "<i>{}</i>", escape(&latest.authors));
        if !latest.tags.is_empty() {
            let _ = write!(html, "<br>Tags: {}", escape(&latest.tags.join(" ")));
        }
        html.push_str("</p>\n<ul>\n");

        for item in &versions {
            let version = item.version.to_string();
            let _ = write!(
                html,
                "<li><a href=\"{base_url}/v3/package/{id}/{ver}/{id}.{ver}.nupkg\">{ver}</a>",
                id = escape(&id_lower),
                ver = escape(&version),
            );
            if item.version == latest.version {
                html.push_str(" (latest)");
            }
            if item.version.prerelease.is_some() {
                html.push_str(" (prerelease)");
            }
            html.push_str("</li>\n");
        }

        html.push_str("</ul>\n");
    }

    let _ = write!(
        html,
        "<hr>\n<address>nupkgd/{} &mdash; {} {}, {} {}</address>\n</body></html>\n",
        env!("CARGO_PKG_VERSION"),
        package_count,
        plural(package_count, "package"),
        packages.len(),
        plural(packages.len(), "file"),
    );

    html
}

fn plural(n: usize, word: &str) -> String {
    if n == 1 { word.to_string() } else { format!("{word}s") }
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use crate::models::nuget_version::NugetVersion;

    fn fixture(name: &str) -> Nuspec {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tempest/fixtures").join(name);
        Nuspec::unpack(path).unwrap()
    }

    fn fixtures() -> Vec<Nuspec> {
        [
            "Other.Widget.0.1.0.nupkg",
            "Solo.Prerelease.0.9.0-rc.1.nupkg",
            "Test.Ex.Pkg.2.0.0-beta.1.nupkg",
            "recursive/Test.Ex.Pkg.1.3.1.nupkg",
            "recursive/Test.Ex.Pkg.1.3.2.nupkg",
            "recursive/Test.Ex.Pkg.1.3.3.nupkg",
            "recursive/Test.Ex.Pkg.1.3.4.nupkg",
        ].iter().map(|n| fixture(n)).collect()
    }

    const BASE: &str = "http://feed.example.com";

    #[test]
    fn escape_replaces_html_special_characters() {
        assert_eq!(escape("a & b < c > d \" e ' f"), "a &amp; b &lt; c &gt; d &quot; e &#39; f");
        assert_eq!(escape("plain"), "plain");
    }

    #[test]
    fn empty_feed_renders_a_placeholder() {
        let html = render(BASE, &[]);
        assert!(html.contains("<p><i>No packages found.</i></p>"));
        assert!(!html.contains("<h2>"));
        assert!(html.contains("0 packages, 0 files"));
    }

    #[test]
    fn renders_as_html_document_with_title() {
        let html = render(BASE, &fixtures());
        assert!(html.starts_with("<html><head><title>nupkgd</title></head><body>"));
        assert!(html.trim_end().ends_with("</body></html>"));
        assert!(html.contains("<h1>Index of nupkgd</h1>"));
    }

    #[test]
    fn packages_are_listed_alphabetically_by_id() {
        let html = render(BASE, &fixtures());
        let other = html.find("<h2>Other.Widget</h2>").unwrap();
        let solo = html.find("<h2>Solo.Prerelease</h2>").unwrap();
        let test = html.find("<h2>Test.Ex.Pkg</h2>").unwrap();
        assert!(other < solo && solo < test);
        assert_eq!(html.matches("<h2>").count(), 3);
    }

    #[test]
    fn versions_are_listed_newest_first() {
        let html = render(BASE, &fixtures());
        let pos = |v: &str| html.find(&format!(">{v}</a>")).unwrap();
        assert!(pos("2.0.0-beta.1") < pos("1.3.4"));
        assert!(pos("1.3.4") < pos("1.3.3"));
        assert!(pos("1.3.3") < pos("1.3.2"));
        assert!(pos("1.3.2") < pos("1.3.1"));
    }

    #[test]
    fn latest_is_highest_stable_version_not_prerelease() {
        let html = render(BASE, &fixtures());
        assert!(html.contains(">1.3.4</a> (latest)</li>"));
        assert!(html.contains(">2.0.0-beta.1</a> (prerelease)</li>"));
        assert!(!html.contains(">2.0.0-beta.1</a> (latest)"));
    }

    #[test]
    fn prerelease_only_package_marks_its_prerelease_as_latest() {
        let html = render(BASE, &fixtures());
        assert!(html.contains(">0.9.0-rc.1</a> (latest) (prerelease)</li>"));
    }

    #[test]
    fn version_links_point_at_nupkg_download_with_lowercase_id() {
        let html = render(BASE, &fixtures());
        assert!(html.contains(
            "<a href=\"http://feed.example.com/v3/package/test.ex.pkg/1.3.4/test.ex.pkg.1.3.4.nupkg\">1.3.4</a>"
        ));
        assert!(html.contains(
            "<a href=\"http://feed.example.com/v3/package/other.widget/0.1.0/other.widget.0.1.0.nupkg\">0.1.0</a>"
        ));
    }

    #[test]
    fn shows_description_authors_and_tags() {
        let html = render(BASE, &fixtures());
        assert!(html.contains("A widget with every metadata field the feed exposes.<br><i>Alice, Bob</i><br>Tags: widgets tools</p>"));
    }

    #[test]
    fn footer_counts_packages_and_files() {
        let html = render(BASE, &fixtures());
        assert!(html.contains(&format!("<address>nupkgd/{} &mdash; 3 packages, 7 files</address>", env!("CARGO_PKG_VERSION"))));
    }

    #[test]
    fn footer_uses_singular_for_one() {
        let html = render(BASE, &[fixture("Other.Widget.0.1.0.nupkg")]);
        assert!(html.contains("1 package, 1 file</address>"));
    }

    #[test]
    fn omits_tags_line_when_there_are_none() {
        let mut pkg = fixture("Other.Widget.0.1.0.nupkg");
        pkg.tags.clear();
        let html = render(BASE, &[pkg]);
        assert!(!html.contains("Tags:"));
    }

    #[test]
    fn user_supplied_text_is_escaped() {
        let pkg = Nuspec {
            id: "Evil<Pkg>".to_string(),
            description: "<script>alert(1)</script>".to_string(),
            authors: "Mallory & Co".to_string(),
            tags: vec!["\"quoted\"".to_string()],
            version: NugetVersion::parse("1.0.0").unwrap(),
            ..Default::default()
        };
        let html = render(BASE, &[pkg]);
        assert!(!html.contains("<script>"));
        assert!(html.contains("<h2>Evil&lt;Pkg&gt;</h2>"));
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(html.contains("Mallory &amp; Co"));
        assert!(html.contains("Tags: &quot;quoted&quot;"));
        assert!(html.contains("/v3/package/evil&lt;pkg&gt;/1.0.0/"));
    }
}
