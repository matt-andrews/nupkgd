use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::{Json, Router};
use axum::routing::get;
use crate::models::app_state::AppState;
use crate::models::nuget_version::NugetVersion;
use crate::models::registration_index::RegistrationIndex;

pub fn router(router: Router<AppState>) -> Router<AppState> {
    router
        .route("/v3/registration/{id}/index.json", get(get_registration_index))
        .route("/v3/registration/{id}/{version}", get(get_registration_version))
}

#[utoipa::path(
    get,
    path = "/v3/registration/{id}/index.json",
)]
pub async fn get_registration_index(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap
) -> impl IntoResponse {
    let package = RegistrationIndex::new(
        &state.resolve_base_url(headers),
        &id,
        &state.find_package(&id).await.unwrap_or_default());

    let Some(package) = package else {
        return StatusCode::NOT_FOUND.into_response();
    };

    (StatusCode::OK, Json(package)).into_response()
}

#[utoipa::path(
    get,
    path = "/v3/registration/{id}/{version}",
)]
pub async fn get_registration_version(
    State(state): State<AppState>,
    Path((id, version)): Path<(String, String)>,
    headers: HeaderMap
) -> impl IntoResponse {
    let package = RegistrationIndex::new(
        &state.resolve_base_url(headers),
        &id,
        &state.find_package(&id).await.unwrap_or_default());

    let Some(package) = package else {
        return StatusCode::NOT_FOUND.into_response();
    };

    //return 404 if wrong ext
    let Ok(version) = NugetVersion::parse(version.trim_end_matches(".json")) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let version = version.to_string();

    let Some(package) = package.items
        .first().unwrap() //one page is hardcoded
        .items.iter()
        .find(|f| f.catalog_entry.version == version) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    (StatusCode::OK, Json(package)).into_response()
}