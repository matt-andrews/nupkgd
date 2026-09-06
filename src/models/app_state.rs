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

    pub fn from_dir(dir: &PathBuf, base_url: Option<String>, recursive: bool) -> anyhow::Result<AppState> {
        if !dir.is_dir() {
            return Err(anyhow::anyhow!("{} is not a directory", dir.display()));
        }

        let max_depth = if recursive { usize::MAX } else { 1 };

        let contents: Vec<_> = WalkDir::new(dir)
            .max_depth(max_depth)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.file_name().to_str().unwrap_or_default().ends_with(".nupkg"))
            .filter_map(|e| Nuspec::unpack(e.path()).ok())
            .collect();

        Ok(AppState {
            base_url,
            contents: Arc::new(RwLock::new(contents)),
        })
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
        AppState { base_url: None, contents: Arc::new(RwLock::new(contents)) }
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
}
