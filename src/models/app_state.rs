use crate::models::nuspec::Nuspec;
use std::path::PathBuf;
use std::sync::Arc;
use axum::http::HeaderMap;
use tokio::sync::RwLock;
use walkdir::WalkDir;
use crate::models::nuget_version::NugetVersion;

#[derive(Clone)]
pub struct AppState {
    pub base_url: Option<String>,
    pub contents: Arc<RwLock<Vec<Nuspec>>>,
    pub base_dir: PathBuf,
    pub recursive: bool,
}

impl AppState {
    pub async fn remove_file(&self, path: &PathBuf) -> anyhow::Result<()> {
        self.contents.write().await.retain(|f| !f.file.eq(path));
        Ok(())
    }
    pub async fn add_file(&self, path: &PathBuf) -> anyhow::Result<()> {
        // Parse before locking, then replace under a single write guard so concurrent
        // watcher events for the same file cannot interleave a remove and two pushes.
        let item = Nuspec::unpack(path)?;
        let mut write_guard = self.contents.write().await;
        write_guard.retain(|f| !f.file.eq(path));
        write_guard.push(item);
        Ok(())
    }

    pub async fn get_contents(&self) -> Vec<Nuspec> {
        self.contents
            .read().await
            .iter()
            .cloned()
            .collect()
    }

    pub fn resolve_base_url(&self, headers: HeaderMap) -> String {
        if let Some(base_url) = &self.base_url {
            return base_url.to_owned().trim_end_matches('/').to_string();
        }

        let scheme = headers.get("x-forwarded-proto")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("http");

        let host = headers.get("X-Forwarded-Host")
            .and_then(|v| v.to_str().ok())
            .unwrap_or(
                headers.get("Host")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("localhost:8080")
            );

        format!("{}://{}", scheme, host)
    }

    pub async fn scan_dir(&self) -> anyhow::Result<()>{
        if !self.base_dir.is_dir() {
            return Err(anyhow::anyhow!("{} is not a directory", self.base_dir.display()));
        }

        let max_depth = if self.recursive { usize::MAX } else { 1 };

        let contents: Vec<_> = WalkDir::new(&self.base_dir)
            .max_depth(max_depth)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.file_name().to_str().unwrap_or_default().ends_with(".nupkg"))
            .filter_map(|e| Nuspec::unpack(e.path()).ok())
            .collect();

        let mut write_guard = self.contents.write().await;
        write_guard.clear();

        for x in contents {
            write_guard.push(x);
        }

        Ok(())
    }

    pub async fn from_dir(dir: &PathBuf, base_url: Option<String>, recursive: bool) -> anyhow::Result<AppState> {
        let state = AppState {
            base_url,
            contents: Arc::new(RwLock::new(Vec::new())),
            base_dir: dir.clone(),
            recursive,
        };

        state.scan_dir().await?;

        Ok(state)
    }

    pub async fn find_package(&self, package_id: &str) -> Option<Vec<Nuspec>>{
        let package: Vec<Nuspec> = self.contents
            .read().await
            .iter()
            .filter(|f|f.match_id(package_id))
            .cloned()
            .collect();

        if !package.is_empty() {
            return Some(package);
        }

        None
    }

    pub async fn find_package_version(&self, package_id: &str, version: &str) -> Option<Nuspec>{
        let Ok(version) = NugetVersion::parse(version) else {
            return None;
        };
        
        self.contents
            .read().await
            .iter()
            .find(|f| f.match_id(package_id) && f.version == version)
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::timeout;

    fn state_with(files: &[&str]) -> AppState {
        let contents = files
            .iter()
            .map(|f| Nuspec { file: PathBuf::from(f), ..Default::default() })
            .collect();
        AppState { base_url: None, contents: Arc::new(RwLock::new(contents)), base_dir: PathBuf::default(), recursive: false }
    }

    #[tokio::test]
    async fn remove_file_does_not_deadlock() {
        let state = state_with(&["a.nupkg", "b.nupkg"]);

        timeout(Duration::from_secs(2), state.remove_file(&PathBuf::from("a.nupkg")))
            .await
            .expect("remove_file deadlocked on its own lock")
            .unwrap();

        let remaining = state.get_contents().await;
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].file, PathBuf::from("b.nupkg"));
    }

    #[tokio::test]
    async fn remove_file_ignores_unknown_paths() {
        let state = state_with(&["a.nupkg"]);

        timeout(Duration::from_secs(2), state.remove_file(&PathBuf::from("missing.nupkg")))
            .await
            .expect("remove_file deadlocked on its own lock")
            .unwrap();

        assert_eq!(state.get_contents().await.len(), 1);
    }

    #[tokio::test]
    async fn add_file_with_unreadable_package_releases_the_lock() {
        let state = state_with(&["a.nupkg"]);

        let result = timeout(Duration::from_secs(2), state.add_file(&PathBuf::from("does-not-exist.nupkg")))
            .await
            .expect("add_file deadlocked on its own lock");
        assert!(result.is_err());

        let contents = timeout(Duration::from_secs(2), state.get_contents())
            .await
            .expect("lock still held after a failed add_file");
        assert_eq!(contents.len(), 1);
    }

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tempest/fixtures").join(name)
    }

    #[tokio::test]
    async fn add_file_replaces_an_existing_entry_for_the_same_path() {
        let state = state_with(&[]);
        let path = fixture("Other.Widget.0.1.0.nupkg");

        state.add_file(&path).await.unwrap();
        state.add_file(&path).await.unwrap();

        let contents = state.get_contents().await;
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].id, "Other.Widget");
    }

    // multi_thread on purpose: on the current-thread runtime the old remove-then-push
    // add_file never interleaved and this test could not catch the duplication.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_add_file_calls_for_one_path_never_duplicate_it() {
        let state = state_with(&[]);
        let path = fixture("Other.Widget.0.1.0.nupkg");

        let tasks: Vec<_> = (0..32)
            .map(|_| {
                let state = state.clone();
                let path = path.clone();
                tokio::spawn(async move { state.add_file(&path).await })
            })
            .collect();
        for task in tasks {
            timeout(Duration::from_secs(5), task).await.expect("add_file hung").unwrap().unwrap();
        }

        assert_eq!(state.get_contents().await.len(), 1);
    }

    fn pkg(id: &str, version: &str) -> Nuspec {
        Nuspec {
            file: PathBuf::from(format!("{id}.{version}.nupkg")),
            id: id.to_string(),
            version: NugetVersion::parse(version).unwrap(),
            ..Default::default()
        }
    }

    fn state_with_packages(packages: Vec<Nuspec>) -> AppState {
        AppState { base_url: None, contents: Arc::new(RwLock::new(packages)), base_dir: PathBuf::default(), recursive: false }
    }

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.insert(
                axum::http::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                axum::http::HeaderValue::from_str(value).unwrap(),
            );
        }
        map
    }

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tempest/fixtures")
    }

    // --- get_contents ---

    #[tokio::test]
    async fn get_contents_returns_a_snapshot_not_a_live_view() {
        let state = state_with(&["a.nupkg"]);

        let mut snapshot = state.get_contents().await;
        snapshot.clear();

        assert_eq!(state.get_contents().await.len(), 1);
    }

    #[tokio::test]
    async fn cloned_state_shares_the_same_contents() {
        let state = state_with(&["a.nupkg"]);
        let clone = state.clone();

        clone.remove_file(&PathBuf::from("a.nupkg")).await.unwrap();

        assert!(state.get_contents().await.is_empty());
    }

    // --- resolve_base_url ---

    #[test]
    fn resolve_base_url_prefers_the_configured_value_over_headers() {
        let state = AppState {
            base_url: Some("https://feed.example.com/nuget".to_string()),
            contents: Arc::new(RwLock::new(vec![])),
            base_dir: PathBuf::default(), recursive: false
        };
        let headers = headers(&[
            ("host", "ignored:1234"),
            ("x-forwarded-host", "also-ignored"),
            ("x-forwarded-proto", "ftp"),
        ]);

        assert_eq!(state.resolve_base_url(headers), "https://feed.example.com/nuget");
    }

    #[test]
    fn resolve_base_url_strips_trailing_slashes_from_the_configured_value() {
        let state = AppState {
            base_url: Some("https://feed.example.com/nuget///".to_string()),
            contents: Arc::new(RwLock::new(vec![])),
            base_dir: PathBuf::default(), recursive: false
        };

        assert_eq!(state.resolve_base_url(HeaderMap::new()), "https://feed.example.com/nuget");
    }

    #[test]
    fn resolve_base_url_falls_back_to_localhost_when_no_headers_are_present() {
        let state = state_with(&[]);

        assert_eq!(state.resolve_base_url(HeaderMap::new()), "http://localhost:8080");
    }

    #[test]
    fn resolve_base_url_uses_the_host_header() {
        let state = state_with(&[]);

        let url = state.resolve_base_url(headers(&[("host", "nuget.internal:5555")]));

        assert_eq!(url, "http://nuget.internal:5555");
    }

    #[test]
    fn resolve_base_url_prefers_x_forwarded_host_over_host() {
        let state = state_with(&[]);
        let headers = headers(&[
            ("host", "127.0.0.1:5555"),
            ("x-forwarded-host", "nuget.example.com"),
        ]);

        assert_eq!(state.resolve_base_url(headers), "http://nuget.example.com");
    }

    #[test]
    fn resolve_base_url_uses_x_forwarded_proto_for_the_scheme() {
        let state = state_with(&[]);
        let headers = headers(&[
            ("host", "nuget.example.com"),
            ("x-forwarded-proto", "https"),
        ]);

        assert_eq!(state.resolve_base_url(headers), "https://nuget.example.com");
    }

    #[test]
    fn resolve_base_url_ignores_header_values_that_are_not_valid_strings() {
        let state = state_with(&[]);
        let mut headers = HeaderMap::new();
        headers.insert("host", axum::http::HeaderValue::from_bytes(b"bad\xffhost").unwrap());
        headers.insert("x-forwarded-proto", axum::http::HeaderValue::from_bytes(b"\xff").unwrap());

        assert_eq!(state.resolve_base_url(headers), "http://localhost:8080");
    }

    // --- from_dir ---

    #[tokio::test]
    async fn from_dir_rejects_a_path_that_does_not_exist() {
        let missing = fixtures_dir().join("does-not-exist");

        let err = AppState::from_dir(&missing, None, false).await.err().expect("expected from_dir to fail");

        assert!(err.to_string().contains("is not a directory"), "{err}");
    }

    #[tokio::test]
    async fn from_dir_rejects_a_file_path() {
        let file = fixture("Other.Widget.0.1.0.nupkg");

        let err = AppState::from_dir(&file, None, true).await.err().expect("expected from_dir to fail");

        assert!(err.to_string().contains("is not a directory"), "{err}");
    }

    #[tokio::test]
    async fn from_dir_non_recursive_loads_only_top_level_packages() {
        let state = AppState::from_dir(&fixtures_dir(), None, false).await.unwrap();

        let mut ids: Vec<_> = state
            .get_contents()
            .await
            .iter()
            .map(|p| format!("{}/{}", p.id, p.version))
            .collect();
        ids.sort();

        assert_eq!(ids, vec![
            "Other.Widget/0.1.0",
            "Solo.Prerelease/0.9.0-rc.1",
            "Test.Ex.Pkg/2.0.0-beta.1",
        ]);
    }

    #[tokio::test]
    async fn from_dir_recursive_loads_packages_from_subfolders() {
        let state = AppState::from_dir(&fixtures_dir(), None, true).await.unwrap();

        let contents = state.get_contents().await;
        let nested: Vec<_> = contents
            .iter()
            .filter(|p| p.file.starts_with(fixtures_dir().join("recursive")))
            .map(|p| p.version.to_string())
            .collect();

        assert_eq!(contents.len(), 7);
        assert_eq!(nested.len(), 4);
        for version in ["1.3.1", "1.3.2", "1.3.3", "1.3.4"] {
            assert!(nested.contains(&version.to_string()), "missing {version}");
        }
    }

    #[tokio::test]
    async fn from_dir_skips_unreadable_and_non_nupkg_files() {
        let state = AppState::from_dir(&fixtures_dir(), None, true).await.unwrap();

        let contents = state.get_contents().await;

        assert!(contents.iter().all(|p| p.file.extension().is_some_and(|e| e == "nupkg")));
        assert!(!contents.iter().any(|p| p.file.ends_with("Broken.Package.1.0.0.nupkg")));
        assert!(!contents.iter().any(|p| p.id == "Broken.Package"));
    }

    #[tokio::test]
    async fn from_dir_keeps_the_configured_base_url() {
        let state = AppState::from_dir(&fixtures_dir(), Some("https://x".to_string()), false).await.unwrap();

        assert_eq!(state.base_url.as_deref(), Some("https://x"));
    }

    // --- find_package ---

    #[tokio::test]
    async fn find_package_returns_none_when_no_versions_match() {
        let state = state_with_packages(vec![pkg("Other.Widget", "0.1.0")]);

        assert!(state.find_package("Missing").await.is_none());
    }

    #[tokio::test]
    async fn find_package_returns_every_version_of_the_package() {
        let state = state_with_packages(vec![
            pkg("Test.Ex.Pkg", "1.3.1"),
            pkg("Other.Widget", "0.1.0"),
            pkg("Test.Ex.Pkg", "2.0.0-beta.1"),
        ]);

        let found = state.find_package("Test.Ex.Pkg").await.unwrap();

        let mut versions: Vec<_> = found.iter().map(|p| p.version.to_string()).collect();
        versions.sort();
        assert_eq!(versions, vec!["1.3.1", "2.0.0-beta.1"]);
        assert!(found.iter().all(|p| p.id == "Test.Ex.Pkg"));
    }

    #[tokio::test]
    async fn find_package_matches_ids_case_insensitively() {
        let state = state_with_packages(vec![pkg("Test.Ex.Pkg", "1.3.1")]);

        assert_eq!(state.find_package("test.ex.pkg").await.unwrap().len(), 1);
        assert_eq!(state.find_package("TEST.EX.PKG").await.unwrap().len(), 1);
    }

    // --- find_package_version ---

    #[tokio::test]
    async fn find_package_version_returns_the_matching_package() {
        let state = state_with_packages(vec![
            pkg("Test.Ex.Pkg", "1.3.1"),
            pkg("Test.Ex.Pkg", "1.3.2"),
        ]);

        let found = state.find_package_version("Test.Ex.Pkg", "1.3.2").await.unwrap();

        assert_eq!(found.id, "Test.Ex.Pkg");
        assert_eq!(found.version.to_string(), "1.3.2");
    }

    #[tokio::test]
    async fn find_package_version_matches_ids_case_insensitively() {
        let state = state_with_packages(vec![pkg("Test.Ex.Pkg", "1.3.1")]);

        assert!(state.find_package_version("TEST.ex.PKG", "1.3.1").await.is_some());
    }

    #[tokio::test]
    async fn find_package_version_returns_none_for_an_unknown_version() {
        let state = state_with_packages(vec![pkg("Test.Ex.Pkg", "1.3.1")]);

        assert!(state.find_package_version("Test.Ex.Pkg", "9.9.9").await.is_none());
    }

    #[tokio::test]
    async fn find_package_version_does_not_match_the_version_of_a_different_package() {
        let state = state_with_packages(vec![
            pkg("Test.Ex.Pkg", "1.3.1"),
            pkg("Other.Widget", "0.1.0"),
        ]);

        assert!(state.find_package_version("Test.Ex.Pkg", "0.1.0").await.is_none());
    }

    #[tokio::test]
    async fn find_package_version_returns_none_for_an_unparseable_version() {
        let state = state_with_packages(vec![pkg("Test.Ex.Pkg", "1.3.1")]);

        for bad in ["", "not-a-version", "1.2.3.4.5", "1.0.0-"] {
            assert!(
                state.find_package_version("Test.Ex.Pkg", bad).await.is_none(),
                "expected {bad:?} to be rejected"
            );
        }
    }

    #[tokio::test]
    async fn find_package_version_normalises_equivalent_version_strings() {
        let state = state_with_packages(vec![pkg("Test.Ex.Pkg", "1.3.1")]);

        // Build metadata and a zero revision do not affect NuGet version equality.
        for equivalent in ["1.3.1.0", "1.3.1+build.7", "1.3.01"] {
            assert!(
                state.find_package_version("Test.Ex.Pkg", equivalent).await.is_some(),
                "expected {equivalent:?} to resolve to 1.3.1"
            );
        }
    }

    #[tokio::test]
    async fn find_package_version_distinguishes_prerelease_from_release() {
        let state = state_with_packages(vec![pkg("Test.Ex.Pkg", "2.0.0-beta.1")]);

        assert!(state.find_package_version("Test.Ex.Pkg", "2.0.0").await.is_none());
        assert!(state.find_package_version("Test.Ex.Pkg", "2.0.0-BETA.1").await.is_some());
    }
}
