//! AIRL API - HTTP API server for AI coding agents.
//!
//! Provides a JSON HTTP API wrapping all AIRL toolchain operations:
//! project management, semantic patches, type checking, interpretation,
//! compilation, queries, and text projections.
//!
//! Start the server with `serve()` or use `build_router()` for testing.

pub mod handlers;
pub mod models;
pub mod routes;

use handlers::AppState;
use std::sync::{Arc, Mutex};

/// Start the API server on the given port.
pub async fn serve(port: u16) {
    let state: AppState = Arc::new(Mutex::new(None));
    let app = routes::build_router(state);

    let addr = format!("0.0.0.0:{port}");
    eprintln!("AIRL API server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// Build the router for testing (no server startup).
pub fn build_test_router() -> axum::Router {
    let state: AppState = Arc::new(Mutex::new(None));
    routes::build_router(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::util::ServiceExt;

    fn hello_module_json() -> &'static str {
        r#"{"format_version":"0.1.0","module":{"id":"mod_main","name":"main","metadata":{"version":"1.0.0","description":"","author":"","created_at":""},"imports":[{"module":"std::io","items":["println"]}],"exports":[],"types":[],"functions":[{"id":"f_main","name":"main","params":[],"returns":"Unit","effects":["IO"],"body":{"id":"n_1","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"n_2","kind":"Literal","type":"String","value":"hello"}]}}]}}"#
    }

    async fn body_string(body: Body) -> String {
        let bytes = body.collect().await.unwrap().to_bytes();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    fn json_request(method: &str, uri: &str, body: &str) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    // Helper: create a project and return the app for further requests
    async fn setup_project() -> axum::Router {
        let app = build_test_router();
        let create_body = format!(
            r#"{{"name":"test","module_json":{}}}"#,
            serde_json::to_string(hello_module_json()).unwrap()
        );
        let resp = app
            .clone()
            .oneshot(json_request("POST", "/project/create", &create_body))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        app
    }

    #[tokio::test]
    async fn test_create_project() {
        let app = build_test_router();
        let create_body = format!(
            r#"{{"name":"test","module_json":{}}}"#,
            serde_json::to_string(hello_module_json()).unwrap()
        );
        let resp = app
            .oneshot(json_request("POST", "/project/create", &create_body))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let body = body_string(resp.into_body()).await;
        assert!(body.contains("\"name\":\"test\""));
        assert!(body.contains("\"function_count\":1"));
    }

    #[tokio::test]
    async fn test_get_project_no_project() {
        let app = build_test_router();
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/project")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_get_project() {
        let app = setup_project().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/project")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_string(resp.into_body()).await;
        assert!(body.contains("\"name\":\"test\""));
    }

    #[tokio::test]
    async fn test_get_module() {
        let app = setup_project().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/module")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_string(resp.into_body()).await;
        assert!(body.contains("\"format_version\""));
    }

    #[tokio::test]
    async fn test_typecheck() {
        let app = setup_project().await;
        let resp = app
            .oneshot(json_request("POST", "/typecheck", "{}"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_string(resp.into_body()).await;
        assert!(body.contains("\"success\":true"));
    }

    #[tokio::test]
    async fn test_interpret() {
        let app = setup_project().await;
        let resp = app
            .oneshot(json_request("POST", "/interpret", "{}"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_string(resp.into_body()).await;
        assert!(body.contains("\"success\":true"));
        assert!(body.contains("hello"));
    }

    #[tokio::test]
    async fn test_compile() {
        let app = setup_project().await;
        let resp = app
            .oneshot(json_request("POST", "/compile", "{}"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_string(resp.into_body()).await;
        assert!(body.contains("\"success\":true"));
        assert!(body.contains("hello"));
        assert!(body.contains("compile_time_ms"));
    }

    #[tokio::test]
    async fn test_find_functions() {
        let app = setup_project().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/query/functions?pattern=main")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_string(resp.into_body()).await;
        assert!(body.contains("\"name\":\"main\""));
    }

    #[tokio::test]
    async fn test_get_call_graph() {
        let app = setup_project().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/query/call-graph?func=main")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_string(resp.into_body()).await;
        assert!(body.contains("std::io::println"));
    }

    #[tokio::test]
    async fn test_get_effects() {
        let app = setup_project().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/query/effects?func=main")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_string(resp.into_body()).await;
        assert!(body.contains("IO"));
    }

    #[tokio::test]
    async fn test_project_to_text() {
        let app = setup_project().await;
        let resp = app
            .oneshot(json_request(
                "POST",
                "/project/text",
                r#"{"language":"pseudocode"}"#,
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_string(resp.into_body()).await;
        assert!(body.contains("fn main"));
    }

    #[tokio::test]
    async fn test_apply_patch() {
        let app = setup_project().await;
        let patch_body = r#"{
            "id": "p1",
            "parent_version": "",
            "operations": [{
                "kind": "ReplaceNode",
                "target": "n_2",
                "replacement": {"id":"n_2","kind":"Literal","type":"String","value":"patched"}
            }],
            "rationale": "test",
            "author": "agent"
        }"#;
        let resp = app
            .oneshot(json_request("POST", "/patch/apply", patch_body))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_string(resp.into_body()).await;
        assert!(body.contains("\"success\":true"));
        assert!(body.contains("new_version"));
    }

    #[tokio::test]
    async fn test_preview_patch() {
        let app = setup_project().await;
        let patch_body = r#"{
            "id": "p1",
            "parent_version": "",
            "operations": [{
                "kind": "ReplaceNode",
                "target": "n_2",
                "replacement": {"id":"n_2","kind":"Literal","type":"String","value":"preview"}
            }],
            "rationale": "test",
            "author": "agent"
        }"#;
        let resp = app
            .oneshot(json_request("POST", "/patch/preview", patch_body))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_string(resp.into_body()).await;
        assert!(body.contains("\"would_succeed\":true"));
    }

    #[tokio::test]
    async fn test_end_to_end_create_patch_interpret() {
        let app = setup_project().await;

        // Apply patch
        let patch_body = r#"{
            "id": "p1",
            "parent_version": "",
            "operations": [{
                "kind": "ReplaceNode",
                "target": "n_2",
                "replacement": {"id":"n_2","kind":"Literal","type":"String","value":"patched via API"}
            }],
            "rationale": "e2e test",
            "author": "agent"
        }"#;
        let resp = app
            .clone()
            .oneshot(json_request("POST", "/patch/apply", patch_body))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Interpret the patched module
        let resp = app
            .oneshot(json_request("POST", "/interpret", "{}"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_string(resp.into_body()).await;
        assert!(body.contains("patched via API"));
    }
}
