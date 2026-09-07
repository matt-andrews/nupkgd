use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Html;
use axum::routing::get;
use crate::models::app_state::AppState;
use crate::models::html_index;

pub fn router(router: Router<AppState>) -> Router<AppState> {
    router
        .route("/", get(get_html_index))
        .route("/index.html", get(get_html_index))
}

#[utoipa::path(
    get,
    path = "/",
)]
pub async fn get_html_index(State(state): State<AppState>, headers: HeaderMap) -> Html<String> {
    let base_url = state.resolve_base_url(headers);
    Html(html_index::render(&base_url, &state.get_contents().await))
}
