//! HTTP request handlers for the AIRL API.
//!
//! Each `pub async fn` corresponds to a single API route; the request
//! and response types are mostly thin JSON wrappers over the core
//! `airl-project`/`airl-interp`/etc. operations.

#![allow(missing_docs)] // handlers and wire-format structs are self-documenting

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use std::sync::{Arc, Mutex};

use airl_project::Project;

use crate::models::*;

pub type AppState = Arc<Mutex<Option<Project>>>;

fn with_project<F, R>(state: &AppState, f: F) -> Result<R, (StatusCode, Json<ErrorResponse>)>
where
    F: FnOnce(&Project) -> R,
{
    let lock = state.lock().unwrap();
    let project = lock.as_ref().ok_or((
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: "no project loaded — POST /project/create first".to_string(),
            code: "NO_PROJECT".to_string(),
        }),
    ))?;
    Ok(f(project))
}

fn with_project_mut<F, R>(state: &AppState, f: F) -> Result<R, (StatusCode, Json<ErrorResponse>)>
where
    F: FnOnce(&mut Project) -> R,
{
    let mut lock = state.lock().unwrap();
    let project = lock.as_mut().ok_or((
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: "no project loaded — POST /project/create first".to_string(),
            code: "NO_PROJECT".to_string(),
        }),
    ))?;
    Ok(f(project))
}

// ---------------------------------------------------------------------------
// Project management
// ---------------------------------------------------------------------------

pub async fn create_project(
    State(state): State<AppState>,
    Json(req): Json<CreateProjectRequest>,
) -> impl IntoResponse {
    match Project::from_json(&req.name, &req.module_json) {
        Ok(project) => {
            let info = ProjectInfo {
                name: project.name.clone(),
                version: project.version.clone(),
                function_count: project.module.functions().len(),
                history_length: 0,
            };
            let mut lock = state.lock().unwrap();
            *lock = Some(project);
            (StatusCode::CREATED, Json(info)).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
                code: "INVALID_IR".to_string(),
            }),
        )
            .into_response(),
    }
}

pub async fn get_project(State(state): State<AppState>) -> impl IntoResponse {
    match with_project(&state, |p| ProjectInfo {
        name: p.name.clone(),
        version: p.version.clone(),
        function_count: p.module.functions().len(),
        history_length: p.history.len(),
    }) {
        Ok(info) => (StatusCode::OK, Json(info)).into_response(),
        Err(e) => e.into_response(),
    }
}

pub async fn get_module(State(state): State<AppState>) -> impl IntoResponse {
    match with_project(&state, |p| ModuleResponse {
        module: p.module.clone(),
        version: p.version.clone(),
    }) {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(e) => e.into_response(),
    }
}

// ---------------------------------------------------------------------------
// Patch operations
// ---------------------------------------------------------------------------

pub async fn apply_patch(
    State(state): State<AppState>,
    Json(req): Json<ApplyPatchRequest>,
) -> impl IntoResponse {
    match with_project_mut(&state, |p| p.apply_patch(&req.patch)) {
        Ok(Ok(result)) => (
            StatusCode::OK,
            Json(PatchResultResponse {
                success: true,
                new_version: result.new_version,
                impact: result.impact,
            }),
        )
            .into_response(),
        Ok(Err(e)) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
                code: "PATCH_ERROR".to_string(),
            }),
        )
            .into_response(),
        Err(e) => e.into_response(),
    }
}

pub async fn preview_patch(
    State(state): State<AppState>,
    Json(req): Json<ApplyPatchRequest>,
) -> impl IntoResponse {
    match with_project(&state, |p| p.preview_patch(&req.patch)) {
        Ok(Ok(preview)) => (
            StatusCode::OK,
            Json(PatchPreviewResponse {
                would_succeed: preview.would_succeed,
                validation_error: preview.validation_error,
                type_errors: preview.type_errors,
                impact: preview.impact,
            }),
        )
            .into_response(),
        Ok(Err(e)) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
                code: "PATCH_ERROR".to_string(),
            }),
        )
            .into_response(),
        Err(e) => e.into_response(),
    }
}

pub async fn undo_patch(State(state): State<AppState>) -> impl IntoResponse {
    match with_project_mut(&state, |p| p.undo_last()) {
        Ok(Ok(result)) => (
            StatusCode::OK,
            Json(PatchResultResponse {
                success: true,
                new_version: result.new_version,
                impact: result.impact,
            }),
        )
            .into_response(),
        Ok(Err(e)) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
                code: "UNDO_ERROR".to_string(),
            }),
        )
            .into_response(),
        Err(e) => e.into_response(),
    }
}

// ---------------------------------------------------------------------------
// Build & Run
// ---------------------------------------------------------------------------

pub async fn typecheck(State(state): State<AppState>) -> impl IntoResponse {
    match with_project(&state, |p| p.typecheck()) {
        Ok(result) => {
            let resp = TypeCheckResponse {
                success: result.is_ok(),
                errors: result
                    .errors
                    .iter()
                    .map(|d| DiagnosticResponse {
                        severity: "error".to_string(),
                        node_id: d.node_id.clone(),
                        message: d.message.clone(),
                    })
                    .collect(),
                warnings: result
                    .warnings
                    .iter()
                    .map(|d| DiagnosticResponse {
                        severity: "warning".to_string(),
                        node_id: d.node_id.clone(),
                        message: d.message.clone(),
                    })
                    .collect(),
            };
            (StatusCode::OK, Json(resp)).into_response()
        }
        Err(e) => e.into_response(),
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct ConstraintsRequest {
    pub constraints: Vec<airl_project::constraints::Constraint>,
}

#[derive(Debug, serde::Serialize)]
pub struct ConstraintsResponse {
    pub ok: bool,
    pub violations: Vec<airl_project::constraints::ConstraintViolation>,
}

pub async fn check_constraints(
    State(state): State<AppState>,
    Json(req): Json<ConstraintsRequest>,
) -> impl IntoResponse {
    match with_project(&state, |p| p.check_constraints(&req.constraints)) {
        Ok(report) => (
            StatusCode::OK,
            Json(ConstraintsResponse {
                ok: report.is_ok(),
                violations: report.violations,
            }),
        )
            .into_response(),
        Err(e) => e.into_response(),
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct DiffRequest {
    pub other_module_json: String,
}

pub async fn diff_module(
    State(state): State<AppState>,
    Json(req): Json<DiffRequest>,
) -> impl IntoResponse {
    let current = match with_project(&state, |p| p.module.clone()) {
        Ok(m) => m,
        Err(e) => return e.into_response(),
    };
    let other: airl_ir::Module = match serde_json::from_str(&req.other_module_json) {
        Ok(m) => m,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: e.to_string(),
                    code: "INVALID_IR".to_string(),
                }),
            )
                .into_response();
        }
    };
    let diff = airl_project::diff::diff(&current, &other);
    (StatusCode::OK, Json(diff)).into_response()
}

#[derive(Debug, serde::Deserialize)]
pub struct DeadCodeQuery {
    #[serde(default = "default_entry")]
    pub entry: String,
}
fn default_entry() -> String {
    "main".to_string()
}

pub async fn find_dead_code(
    State(state): State<AppState>,
    Query(q): Query<DeadCodeQuery>,
) -> impl IntoResponse {
    match with_project(&state, |p| {
        airl_project::queries::find_dead_code(&p.module, &q.entry)
    }) {
        Ok(report) => (StatusCode::OK, Json(report)).into_response(),
        Err(e) => e.into_response(),
    }
}

pub async fn builtin_usage(State(state): State<AppState>) -> impl IntoResponse {
    match with_project(&state, |p| airl_project::queries::builtin_usage(&p.module)) {
        Ok(usage) => (StatusCode::OK, Json(usage)).into_response(),
        Err(e) => e.into_response(),
    }
}

pub async fn effect_surface(State(state): State<AppState>) -> impl IntoResponse {
    match with_project(&state, |p| airl_project::queries::effect_surface(&p.module)) {
        Ok(surface) => (StatusCode::OK, Json(surface)).into_response(),
        Err(e) => e.into_response(),
    }
}

pub async fn interpret(
    State(state): State<AppState>,
    Json(req): Json<InterpretRequest>,
) -> impl IntoResponse {
    let module = match with_project(&state, |p| p.module.clone()) {
        Ok(m) => m,
        Err(e) => return e.into_response(),
    };

    let limits = airl_interp::ExecutionLimits {
        max_steps: req.max_steps,
        max_call_depth: req.max_call_depth,
    };

    match airl_interp::interpret_with_limits(&module, limits) {
        Ok(output) => (
            StatusCode::OK,
            Json(InterpretResponse {
                success: true,
                stdout: output.stdout,
                exit_code: output.exit_code,
                error: None,
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::OK,
            Json(InterpretResponse {
                success: false,
                stdout: String::new(),
                exit_code: 1,
                error: Some(e.to_string()),
            }),
        )
            .into_response(),
    }
}

pub async fn compile(State(state): State<AppState>) -> impl IntoResponse {
    let module = match with_project(&state, |p| p.module.clone()) {
        Ok(m) => m,
        Err(e) => return e.into_response(),
    };

    match airl_compile::compile_and_run(&module) {
        Ok(output) => (
            StatusCode::OK,
            Json(CompileResponse {
                success: true,
                stdout: output.stdout,
                exit_code: output.exit_code,
                compile_time_ms: output.compile_time_ms,
                error: None,
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::OK,
            Json(CompileResponse {
                success: false,
                stdout: String::new(),
                exit_code: 1,
                compile_time_ms: 0,
                error: Some(e.to_string()),
            }),
        )
            .into_response(),
    }
}

// ---------------------------------------------------------------------------
// Queries
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize)]
pub struct PatternQuery {
    #[serde(default)]
    pub pattern: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct FuncQuery {
    pub func: String,
}

pub async fn find_functions(
    State(state): State<AppState>,
    Query(q): Query<PatternQuery>,
) -> impl IntoResponse {
    match with_project(&state, |p| p.find_functions(&q.pattern)) {
        Ok(funcs) => (StatusCode::OK, Json(FunctionsResponse { functions: funcs })).into_response(),
        Err(e) => e.into_response(),
    }
}

pub async fn get_call_graph(
    State(state): State<AppState>,
    Query(q): Query<FuncQuery>,
) -> impl IntoResponse {
    match with_project(&state, |p| p.get_call_graph(&q.func)) {
        Ok(edges) => (StatusCode::OK, Json(CallGraphResponse { edges })).into_response(),
        Err(e) => e.into_response(),
    }
}

pub async fn get_effects(
    State(state): State<AppState>,
    Query(q): Query<FuncQuery>,
) -> impl IntoResponse {
    match with_project(&state, |p| p.get_effect_summary(&q.func)) {
        Ok(Some(summary)) => (StatusCode::OK, Json(EffectsResponse { summary })).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("function '{}' not found", q.func),
                code: "FUNCTION_NOT_FOUND".to_string(),
            }),
        )
            .into_response(),
        Err(e) => e.into_response(),
    }
}

// ---------------------------------------------------------------------------
// WASM compilation
// ---------------------------------------------------------------------------

pub async fn compile_wasm(State(state): State<AppState>) -> impl IntoResponse {
    let module = match with_project(&state, |p| p.module.clone()) {
        Ok(m) => m,
        Err(e) => return e.into_response(),
    };

    match airl_compile::wasm::compile_to_wasm(&module) {
        Ok(wasm_bytes) => {
            use axum::http::header;
            let headers = [
                (header::CONTENT_TYPE, "application/wasm"),
                (
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=\"output.wasm\"",
                ),
            ];
            (StatusCode::OK, headers, wasm_bytes).into_response()
        }
        Err(e) => (
            StatusCode::OK,
            Json(CompileResponse {
                success: false,
                stdout: String::new(),
                exit_code: 1,
                compile_time_ms: 0,
                error: Some(e.to_string()),
            }),
        )
            .into_response(),
    }
}

// ---------------------------------------------------------------------------
// Projections
// ---------------------------------------------------------------------------

pub async fn project_to_text(
    State(state): State<AppState>,
    Json(req): Json<ProjectToTextRequest>,
) -> impl IntoResponse {
    match with_project(&state, |p| {
        let text = match req.language.as_str() {
            "json" => serde_json::to_string_pretty(&p.module).unwrap_or_default(),
            lang => {
                // Try real language projection (TypeScript, Python)
                if let Some(language) = airl_project::projection::Language::parse(lang) {
                    airl_project::projection::project_module(&p.module, language)
                } else {
                    // Fallback: pseudo-code signatures
                    let mut out = String::new();
                    for func in p.module.functions() {
                        let params: Vec<String> = func
                            .params
                            .iter()
                            .map(|p| format!("{}: {}", p.name, p.param_type.to_type_str()))
                            .collect();
                        let effects: Vec<String> =
                            func.effects.iter().map(|e| e.to_effect_str()).collect();
                        out.push_str(&format!(
                            "fn {}({}) -> {} [{}]\n",
                            func.name,
                            params.join(", "),
                            func.returns.to_type_str(),
                            effects.join(", ")
                        ));
                    }
                    out
                }
            }
        };
        TextProjectionResponse {
            language: req.language.clone(),
            text,
        }
    }) {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(e) => e.into_response(),
    }
}
