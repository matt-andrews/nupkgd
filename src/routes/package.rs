use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::{Json, Router};
use axum::response::IntoResponse;
use axum::routing::get;
use tokio_util::io::ReaderStream;
use crate::models::app_state::AppState;
use crate::models::package_index::PackageIndex;

pub fn router(router: Router<AppState>) -> Router<AppState> {
    router
        .route("/v3/package/{id}/index.json", get(get_package_index))
        .route("/v3/package/{id}/{version}/{file}", get(get_package))
}

#[utoipa::path(
    get,
    path = "/v3/package/{id}/index.json",
)]
pub async fn get_package_index(
    State(state): State<AppState>,
    Path(id): Path<String>
) -> impl IntoResponse {
    let package = PackageIndex::new(&state, &id).await;

    let Some(package) = package else {
        return StatusCode::NOT_FOUND.into_response();
    };

    (StatusCode::OK, Json(package)).into_response()
}

#[utoipa::path(
    get,
    path = "/v3/package/{id}/{version}/{file}",
)]
pub async fn get_package(
    State(state): State<AppState>,
    Path((id, version, file)): Path<(String, String, String)>
) -> impl IntoResponse {
    let Some(package) = state.find_package_version(&id, &version).await else {
        return StatusCode::NOT_FOUND.into_response();
    };

    return if file.ends_with(".nuspec"){
        let body = Body::from(package.xml.to_string());

        (
            [(header::CONTENT_TYPE, "application/xml".to_string())],
            body,
        ).into_response()
    } else if file.ends_with(".nupkg") {
        let Ok(file) = tokio::fs::File::open(&package.file).await else {
            return StatusCode::NOT_FOUND.into_response();
        };

        let mime = "application/octet-stream";
        let name = format!("{}.{}.nupkg", &id, &version);
        let body = Body::from_stream(ReaderStream::new(file));

        (
            [
                (header::CONTENT_TYPE, mime.to_string()),
                (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{name}\"")),
            ],
            body,
        ).into_response()
    } else {
        StatusCode::BAD_REQUEST.into_response()
    }
}