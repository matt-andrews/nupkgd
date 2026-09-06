use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::{Json, Router};
use axum::routing::get;
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use crate::models::app_state::AppState;
use crate::models::nuspec::Nuspec;
use crate::models::search_index::SearchIndex;

pub fn router(router: Router<AppState>) -> Router<AppState> {
    router
        .route("/v3/search", get(get_search))
}

#[utoipa::path(
    get,
    path = "/v3/search",
)]
pub async fn get_search(
    State(state): State<AppState>,
    Query(q): Query<SearchQuery>,
    headers: HeaderMap
) -> impl IntoResponse {

    let query = q.q.unwrap_or_default();

    let package = match query.is_empty() {
        true => SearchIndex::new(&state.resolve_base_url(headers), &state.get_contents().await, q.skip, q.take, q.prerelease),
        false => {
            let matcher = SkimMatcherV2::default().ignore_case();
            let filtered: Vec<Nuspec> = state.get_contents().await
                .iter()
                .filter(|item| matcher.fuzzy_match(&item.id, &query).is_some())
                .map(|m| m.clone())
                .collect();
            SearchIndex::new(&state.resolve_base_url(headers), &filtered, q.skip, q.take, q.prerelease)
        }
    };

    (StatusCode::OK, Json(package)).into_response()
}

#[derive(Deserialize, Serialize, ToSchema)]
pub struct SearchQuery {
    pub q: Option<String>,
    pub skip: Option<usize>,
    pub take: Option<usize>,
    pub prerelease: Option<bool>,
}