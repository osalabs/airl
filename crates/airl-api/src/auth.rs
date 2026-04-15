//! Token-based authentication middleware for the AIRL API.
//!
//! Agents authenticate via `Authorization: Bearer <token>` header.
//! Tokens are validated against a configured set of allowed tokens.
//!
//! Use `auth_layer()` to add auth to a router, or `no_auth_layer()` for testing.

use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::collections::HashSet;
use std::sync::Arc;

/// Auth configuration.
#[derive(Clone)]
pub struct AuthConfig {
    /// Set of valid API tokens. If empty, auth is disabled.
    pub tokens: Arc<HashSet<String>>,
}

impl AuthConfig {
    /// Create a new auth config with the given tokens.
    pub fn new(tokens: Vec<String>) -> Self {
        Self {
            tokens: Arc::new(tokens.into_iter().collect()),
        }
    }

    /// Create a config that allows all requests (no auth).
    pub fn allow_all() -> Self {
        Self {
            tokens: Arc::new(HashSet::new()),
        }
    }

    /// Check if auth is enabled.
    pub fn is_enabled(&self) -> bool {
        !self.tokens.is_empty()
    }
}

/// Auth middleware: validates Bearer token from the Authorization header.
pub async fn auth_middleware(
    axum::extract::State(config): axum::extract::State<AuthConfig>,
    request: Request,
    next: Next,
) -> Response {
    // If no tokens configured, auth is disabled
    if !config.is_enabled() {
        return next.run(request).await;
    }

    // Extract the Authorization header
    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    match auth_header {
        Some(header) if header.starts_with("Bearer ") => {
            let token = &header[7..];
            if config.tokens.contains(token) {
                next.run(request).await
            } else {
                (
                    StatusCode::UNAUTHORIZED,
                    axum::Json(serde_json::json!({
                        "error": "invalid API token",
                        "code": "INVALID_TOKEN"
                    })),
                )
                    .into_response()
            }
        }
        _ => (
            StatusCode::UNAUTHORIZED,
            axum::Json(serde_json::json!({
                "error": "missing Authorization: Bearer <token> header",
                "code": "MISSING_AUTH"
            })),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_config() {
        let config = AuthConfig::new(vec!["token1".to_string(), "token2".to_string()]);
        assert!(config.is_enabled());
        assert!(config.tokens.contains("token1"));
        assert!(!config.tokens.contains("token3"));
    }

    #[test]
    fn test_allow_all() {
        let config = AuthConfig::allow_all();
        assert!(!config.is_enabled());
    }
}
