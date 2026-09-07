pub mod handlers;
pub mod html;
pub mod package;
pub mod registration;
pub mod search;

use axum::Router;
use crate::models::app_state::AppState;

pub fn router(state: AppState) -> Router {
    let mut router = Router::new();
    router = handlers::router(router);
    router = html::router(router);
    router = package::router(router);
    router = registration::router(router);
    router = search::router(router);

    router.with_state(state)
}