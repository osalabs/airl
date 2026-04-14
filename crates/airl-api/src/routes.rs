//! Route definitions for the AIRL API.

use axum::routing::{get, post};
use axum::Router;

use crate::handlers::{self, AppState};

/// Build the axum Router with all AIRL API routes.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        // Project management
        .route("/project/create", post(handlers::create_project))
        .route("/project", get(handlers::get_project))
        .route("/module", get(handlers::get_module))
        // Patch operations
        .route("/patch/apply", post(handlers::apply_patch))
        .route("/patch/preview", post(handlers::preview_patch))
        .route("/patch/undo", post(handlers::undo_patch))
        // Build & run
        .route("/typecheck", post(handlers::typecheck))
        .route("/interpret", post(handlers::interpret))
        .route("/compile", post(handlers::compile))
        // Queries
        .route("/query/functions", get(handlers::find_functions))
        .route("/query/call-graph", get(handlers::get_call_graph))
        .route("/query/effects", get(handlers::get_effects))
        // Projections
        .route("/project/text", post(handlers::project_to_text))
        .with_state(state)
}
