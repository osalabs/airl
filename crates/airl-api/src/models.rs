//! JSON request/response models for the AIRL API.
//!
//! These types mirror the JSON wire format of each endpoint. Field names
//! correspond directly to JSON keys.

#![allow(missing_docs)] // JSON wire-format structs are self-documenting

use airl_ir::module::Module;
use airl_patch::{Impact, Patch};
use airl_project::{CallEdge, EffectSummary, FuncSummary};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Requests
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    pub module_json: String, // raw JSON of the Module
}

#[derive(Debug, Deserialize)]
pub struct ApplyPatchRequest {
    #[serde(flatten)]
    pub patch: Patch,
}

#[derive(Debug, Deserialize)]
pub struct InterpretRequest {
    #[serde(default = "default_entry")]
    pub entry_func: String,
    #[serde(default = "default_max_steps")]
    pub max_steps: u64,
    #[serde(default = "default_max_call_depth")]
    pub max_call_depth: u32,
}

fn default_entry() -> String {
    "main".to_string()
}
fn default_max_steps() -> u64 {
    1_000_000
}
fn default_max_call_depth() -> u32 {
    1000
}

#[derive(Debug, Deserialize)]
pub struct ProjectToTextRequest {
    #[serde(default = "default_language")]
    pub language: String,
}

fn default_language() -> String {
    "typescript".to_string()
}

// ---------------------------------------------------------------------------
// Responses
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct ProjectInfo {
    pub name: String,
    pub version: String,
    pub function_count: usize,
    pub history_length: usize,
}

#[derive(Debug, Serialize)]
pub struct ModuleResponse {
    pub module: Module,
    pub version: String,
}

#[derive(Debug, Serialize)]
pub struct PatchResultResponse {
    pub success: bool,
    pub new_version: String,
    pub impact: Impact,
}

#[derive(Debug, Serialize)]
pub struct PatchPreviewResponse {
    pub would_succeed: bool,
    pub validation_error: Option<String>,
    pub type_errors: Vec<String>,
    pub impact: Impact,
}

#[derive(Debug, Serialize)]
pub struct TypeCheckResponse {
    pub success: bool,
    pub errors: Vec<DiagnosticResponse>,
    pub warnings: Vec<DiagnosticResponse>,
}

#[derive(Debug, Serialize)]
pub struct DiagnosticResponse {
    pub severity: String,
    pub node_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct InterpretResponse {
    pub success: bool,
    pub stdout: String,
    pub exit_code: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CompileResponse {
    pub success: bool,
    pub stdout: String,
    pub exit_code: i32,
    pub compile_time_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FunctionsResponse {
    pub functions: Vec<FuncSummary>,
}

#[derive(Debug, Serialize)]
pub struct CallGraphResponse {
    pub edges: Vec<CallEdge>,
}

#[derive(Debug, Serialize)]
pub struct EffectsResponse {
    #[serde(flatten)]
    pub summary: EffectSummary,
}

#[derive(Debug, Serialize)]
pub struct TextProjectionResponse {
    pub language: String,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
}
