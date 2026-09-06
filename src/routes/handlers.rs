use axum::http::{HeaderMap, StatusCode};
use axum::{Json, Router};
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use crate::models::app_state::AppState;
use crate::models::index;

pub fn router(router: Router<AppState>) -> Router<AppState> {
    router
        .route("/v3/index.json", get(get_index))
        .route("/healthz", get(health))
        .route("/versionz", get(version))
}

#[utoipa::path(
    get,
    path = "/v3/index.json",
)]
pub async fn get_index(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    Json(index::Index::new(&state.resolve_base_url(headers)))
}

#[utoipa::path(
    get,
    path = "/healthz",
)]
pub async fn health() -> StatusCode {
    StatusCode::OK
}

#[utoipa::path(
    get,
    path = "/versionz",
)]
pub async fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}


