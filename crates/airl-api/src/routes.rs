//! Route definitions for the AIRL API.

use axum::middleware;
use axum::routing::{get, post};
use axum::Router;

use crate::auth::{self, AuthConfig};
use crate::handlers::{self, AppState};

/// Build the axum Router with all AIRL API routes and optional auth.
pub fn build_router(state: AppState, auth_config: AuthConfig) -> Router {
    let api_routes = Router::new()
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
        .route("/constraints/check", post(handlers::check_constraints))
        .route("/diff", post(handlers::diff_module))
        .route("/query/dead-code", get(handlers::find_dead_code))
        .route("/query/builtin-usage", get(handlers::builtin_usage))
        .route("/query/effect-surface", get(handlers::effect_surface))
        .route("/interpret", post(handlers::interpret))
        .route("/compile", post(handlers::compile))
        .route("/compile/wasm", post(handlers::compile_wasm))
        // Queries
        .route("/query/functions", get(handlers::find_functions))
        .route("/query/call-graph", get(handlers::get_call_graph))
        .route("/query/effects", get(handlers::get_effects))
        // Projections
        .route("/project/text", post(handlers::project_to_text))
        .with_state(state);

    // Apply auth middleware if tokens are configured
    if auth_config.is_enabled() {
        api_routes.route_layer(middleware::from_fn_with_state(
            auth_config,
            auth::auth_middleware,
        ))
    } else {
        api_routes
    }
}
