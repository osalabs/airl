//! AIRL SDK - Typed Rust client for the AIRL HTTP API.
//!
//! Provides a [`Client`] type wrapping every endpoint exposed by `airl-api`,
//! with optional Bearer token authentication and structured error types.
//!
//! # Example
//!
//! ```no_run
//! use airl_sdk::Client;
//!
//! let client = Client::new("http://127.0.0.1:9090");
//!
//! // Create a project from a JSON IR string
//! let info = client.create_project("my-app", "{...}").unwrap();
//! println!("project: {} version={}", info.name, info.version);
//!
//! // Type check
//! let tc = client.typecheck().unwrap();
//! assert!(tc.success);
//!
//! // Interpret
//! let output = client.interpret_default().unwrap();
//! print!("{}", output.stdout);
//! ```
//!
//! # Authentication
//!
//! If the server is started with `serve_with_auth`, provide a token:
//!
//! ```no_run
//! use airl_sdk::Client;
//!
//! let client = Client::new("http://127.0.0.1:9090")
//!     .with_auth_token("my-secret-token");
//! ```

#![deny(missing_docs)]

use airl_ir::Module;
use airl_patch::{Impact, Patch};
use airl_project::constraints::{Constraint, ConstraintViolation};
use airl_project::diff::ModuleDiff;
use airl_project::queries::{BuiltinUsage, DeadCodeReport, EffectSurface};
use airl_project::{CallEdge, EffectSummary, FuncSummary};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors returned by the AIRL SDK client.
#[derive(Debug, Error)]
pub enum SdkError {
    /// Network-level error (connection refused, DNS, timeout, etc.).
    #[error("HTTP transport error: {0}")]
    Transport(String),
    /// Server returned a non-success status with a structured error body.
    #[error("API error {status} ({code}): {message}")]
    Api {
        /// HTTP status code.
        status: u16,
        /// Short error code from the API.
        code: String,
        /// Human-readable error message.
        message: String,
    },
    /// Response body could not be parsed as the expected type.
    #[error("response parse error: {0}")]
    Parse(#[from] serde_json::Error),
}

/// Structured error body returned by the AIRL API on non-2xx responses.
#[derive(Debug, Deserialize)]
struct ApiErrorBody {
    error: String,
    code: String,
}

// ---------------------------------------------------------------------------
// Request/response types (mirror the wire format)
// ---------------------------------------------------------------------------

/// Summary information about a loaded project.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProjectInfo {
    /// Project name.
    pub name: String,
    /// Content-addressed version hash of the current module.
    pub version: String,
    /// Number of functions in the module.
    pub function_count: usize,
    /// Number of patches in the undo history.
    pub history_length: usize,
}

/// Result of [`Client::get_module`]: the current module plus its version.
#[derive(Debug, Clone, Deserialize)]
pub struct ModuleResponse {
    /// The current module state.
    pub module: Module,
    /// Content-addressed version hash.
    pub version: String,
}

/// One entry from the type checker output.
#[derive(Debug, Clone, Deserialize)]
pub struct DiagnosticResponse {
    /// Severity level: `"error"` or `"warning"`.
    pub severity: String,
    /// IR node ID where the diagnostic originated, if any.
    pub node_id: Option<String>,
    /// Human-readable message.
    pub message: String,
}

/// Response from [`Client::typecheck`].
#[derive(Debug, Clone, Deserialize)]
pub struct TypeCheckResponse {
    /// `true` if no errors were found.
    pub success: bool,
    /// Type errors (prevent execution).
    pub errors: Vec<DiagnosticResponse>,
    /// Non-fatal warnings.
    pub warnings: Vec<DiagnosticResponse>,
}

/// Response from [`Client::interpret`].
#[derive(Debug, Clone, Deserialize)]
pub struct InterpretResponse {
    /// `true` if interpretation succeeded.
    pub success: bool,
    /// Captured standard output.
    pub stdout: String,
    /// Exit code (0 = success).
    pub exit_code: i32,
    /// Runtime error message, if any.
    pub error: Option<String>,
}

/// Response from [`Client::compile`].
#[derive(Debug, Clone, Deserialize)]
pub struct CompileResponse {
    /// `true` if compilation and execution succeeded.
    pub success: bool,
    /// Captured standard output.
    pub stdout: String,
    /// Exit code (0 = success).
    pub exit_code: i32,
    /// Compilation time in milliseconds (excludes execution).
    pub compile_time_ms: u64,
    /// Compile error message, if any.
    pub error: Option<String>,
}

/// Response from [`Client::apply_patch`] or [`Client::undo_patch`].
#[derive(Debug, Clone, Deserialize)]
pub struct PatchResultResponse {
    /// `true` if the patch was applied.
    pub success: bool,
    /// Version hash after the patch.
    pub new_version: String,
    /// Analysis of which functions/types were affected.
    pub impact: Impact,
}

/// Response from [`Client::preview_patch`].
#[derive(Debug, Clone, Deserialize)]
pub struct PatchPreviewResponse {
    /// `true` if the patch would succeed if applied.
    pub would_succeed: bool,
    /// Structural validation error, if any.
    pub validation_error: Option<String>,
    /// Type errors that would arise after applying the patch.
    pub type_errors: Vec<String>,
    /// Analysis of which functions/types would be affected.
    pub impact: Impact,
}

/// Response from [`Client::check_constraints`].
#[derive(Debug, Clone, Deserialize)]
pub struct ConstraintsResponse {
    /// `true` if no constraints were violated.
    pub ok: bool,
    /// List of violations, one per constraint that failed.
    pub violations: Vec<ConstraintViolation>,
}

/// Execution limits for [`Client::interpret`].
#[derive(Debug, Clone, Copy, Serialize)]
pub struct InterpretLimits {
    /// Maximum number of evaluation steps before aborting.
    pub max_steps: u64,
    /// Maximum call-stack depth.
    pub max_call_depth: u32,
}

impl Default for InterpretLimits {
    fn default() -> Self {
        Self {
            max_steps: 1_000_000,
            max_call_depth: 1000,
        }
    }
}

/// Language for [`Client::project_to_text`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionLang {
    /// Project the module to TypeScript source.
    TypeScript,
    /// Project the module to Python source.
    Python,
    /// Raw JSON (pretty-printed IR).
    Json,
    /// Pseudocode function-signature summary (API fallback).
    Pseudocode,
}

impl ProjectionLang {
    fn as_str(self) -> &'static str {
        match self {
            ProjectionLang::TypeScript => "typescript",
            ProjectionLang::Python => "python",
            ProjectionLang::Json => "json",
            ProjectionLang::Pseudocode => "pseudocode",
        }
    }
}

/// Response from [`Client::project_to_text`].
#[derive(Debug, Clone, Deserialize)]
pub struct TextProjectionResponse {
    /// Language identifier echoed back from the request.
    pub language: String,
    /// Rendered source code.
    pub text: String,
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// Typed client for the AIRL HTTP API.
///
/// Construct with [`Client::new`] and chain optional configuration
/// methods like [`Client::with_auth_token`] and [`Client::with_timeout`].
pub struct Client {
    base_url: String,
    agent: ureq::Agent,
    auth_token: Option<String>,
}

impl Client {
    /// Create a new client pointing at the given API server base URL
    /// (e.g. `"http://127.0.0.1:9090"`).
    pub fn new(base_url: impl Into<String>) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(30))
            .build();
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            agent,
            auth_token: None,
        }
    }

    /// Set a Bearer token to send with every request.
    /// Required when the server was started via `serve_with_auth`.
    pub fn with_auth_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }

    /// Override the default 30-second request timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.agent = ureq::AgentBuilder::new().timeout(timeout).build();
        self
    }

    // -- Project management --

    /// Create a project from a JSON IR string. Equivalent to `POST /project/create`.
    pub fn create_project(
        &self,
        name: impl Into<String>,
        module_json: impl Into<String>,
    ) -> Result<ProjectInfo, SdkError> {
        self.post(
            "/project/create",
            &serde_json::json!({
                "name": name.into(),
                "module_json": module_json.into(),
            }),
        )
    }

    /// Get project metadata. Equivalent to `GET /project`.
    pub fn get_project(&self) -> Result<ProjectInfo, SdkError> {
        self.get("/project")
    }

    /// Fetch the current module IR. Equivalent to `GET /module`.
    pub fn get_module(&self) -> Result<ModuleResponse, SdkError> {
        self.get("/module")
    }

    // -- Patch operations --

    /// Apply a semantic patch. Equivalent to `POST /patch/apply`.
    ///
    /// The server expects the patch fields at the request body root
    /// (thanks to `#[serde(flatten)]` on the request struct), so we send
    /// the patch as-is.
    pub fn apply_patch(&self, patch: &Patch) -> Result<PatchResultResponse, SdkError> {
        self.post("/patch/apply", patch)
    }

    /// Preview a patch (dry-run). Equivalent to `POST /patch/preview`.
    pub fn preview_patch(&self, patch: &Patch) -> Result<PatchPreviewResponse, SdkError> {
        self.post("/patch/preview", patch)
    }

    /// Undo the most recent patch. Equivalent to `POST /patch/undo`.
    pub fn undo_patch(&self) -> Result<PatchResultResponse, SdkError> {
        self.post("/patch/undo", &serde_json::json!({}))
    }

    // -- Build & run --

    /// Type-check the current module. Equivalent to `POST /typecheck`.
    pub fn typecheck(&self) -> Result<TypeCheckResponse, SdkError> {
        self.post("/typecheck", &serde_json::json!({}))
    }

    /// Check the module against architectural constraints.
    /// Equivalent to `POST /constraints/check`.
    pub fn check_constraints(
        &self,
        constraints: &[Constraint],
    ) -> Result<ConstraintsResponse, SdkError> {
        self.post(
            "/constraints/check",
            &serde_json::json!({ "constraints": constraints }),
        )
    }

    /// Diff the current module against another module (passed as JSON).
    /// Equivalent to `POST /diff`.
    pub fn diff(&self, other_module_json: impl Into<String>) -> Result<ModuleDiff, SdkError> {
        self.post(
            "/diff",
            &serde_json::json!({ "other_module_json": other_module_json.into() }),
        )
    }

    /// Interpret the module with custom limits. Equivalent to `POST /interpret`.
    pub fn interpret(&self, limits: InterpretLimits) -> Result<InterpretResponse, SdkError> {
        self.post("/interpret", &limits)
    }

    /// Interpret the module with default limits (1M steps, 1000 call depth).
    pub fn interpret_default(&self) -> Result<InterpretResponse, SdkError> {
        self.interpret(InterpretLimits::default())
    }

    /// Cranelift JIT compile and run the module. Equivalent to `POST /compile`.
    pub fn compile(&self) -> Result<CompileResponse, SdkError> {
        self.post("/compile", &serde_json::json!({}))
    }

    /// Compile the module to a WASM binary. Equivalent to `POST /compile/wasm`.
    /// Returns raw bytes.
    pub fn compile_wasm(&self) -> Result<Vec<u8>, SdkError> {
        let url = format!("{}/compile/wasm", self.base_url);
        let mut req = self.agent.post(&url);
        if let Some(ref token) = self.auth_token {
            req = req.set("Authorization", &format!("Bearer {token}"));
        }
        match req.send_string("{}") {
            Ok(resp) => {
                let mut bytes = Vec::new();
                resp.into_reader()
                    .read_to_end(&mut bytes)
                    .map_err(|e| SdkError::Transport(e.to_string()))?;
                Ok(bytes)
            }
            Err(ureq::Error::Status(code, resp)) => Err(api_err(code, resp)),
            Err(e) => Err(SdkError::Transport(e.to_string())),
        }
    }

    // -- Queries --

    /// Find functions whose name contains `pattern` (substring match).
    /// Equivalent to `GET /query/functions?pattern=<p>`.
    pub fn find_functions(&self, pattern: &str) -> Result<Vec<FuncSummary>, SdkError> {
        #[derive(Deserialize)]
        struct Resp {
            functions: Vec<FuncSummary>,
        }
        let path = format!("/query/functions?pattern={}", url_encode(pattern));
        let resp: Resp = self.get(&path)?;
        Ok(resp.functions)
    }

    /// Get call-graph edges for a function.
    /// Equivalent to `GET /query/call-graph?func=<name>`.
    pub fn get_call_graph(&self, func: &str) -> Result<Vec<CallEdge>, SdkError> {
        #[derive(Deserialize)]
        struct Resp {
            edges: Vec<CallEdge>,
        }
        let path = format!("/query/call-graph?func={}", url_encode(func));
        let resp: Resp = self.get(&path)?;
        Ok(resp.edges)
    }

    /// Get the declared effect set for a function.
    /// Equivalent to `GET /query/effects?func=<name>`.
    pub fn get_effects(&self, func: &str) -> Result<EffectSummary, SdkError> {
        let path = format!("/query/effects?func={}", url_encode(func));
        self.get(&path)
    }

    /// Find functions unreachable from an entry point (default `"main"`).
    /// Equivalent to `GET /query/dead-code?entry=<name>`.
    pub fn find_dead_code(&self, entry: &str) -> Result<DeadCodeReport, SdkError> {
        let path = format!("/query/dead-code?entry={}", url_encode(entry));
        self.get(&path)
    }

    /// Count calls to each `std::...` builtin across the module.
    /// Equivalent to `GET /query/builtin-usage`.
    pub fn builtin_usage(&self) -> Result<BuiltinUsage, SdkError> {
        self.get("/query/builtin-usage")
    }

    /// Get the effect surface of the module.
    /// Equivalent to `GET /query/effect-surface`.
    pub fn effect_surface(&self) -> Result<EffectSurface, SdkError> {
        self.get("/query/effect-surface")
    }

    // -- Projections --

    /// Render the module in a target language.
    /// Equivalent to `POST /project/text { language }`.
    pub fn project_to_text(
        &self,
        lang: ProjectionLang,
    ) -> Result<TextProjectionResponse, SdkError> {
        self.post(
            "/project/text",
            &serde_json::json!({ "language": lang.as_str() }),
        )
    }

    // -- Low-level helpers --

    fn get<R: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<R, SdkError> {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self.agent.get(&url);
        if let Some(ref token) = self.auth_token {
            req = req.set("Authorization", &format!("Bearer {token}"));
        }
        match req.call() {
            Ok(resp) => {
                let text = resp
                    .into_string()
                    .map_err(|e| SdkError::Transport(e.to_string()))?;
                Ok(serde_json::from_str(&text)?)
            }
            Err(ureq::Error::Status(code, resp)) => Err(api_err(code, resp)),
            Err(e) => Err(SdkError::Transport(e.to_string())),
        }
    }

    fn post<B: Serialize, R: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<R, SdkError> {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self
            .agent
            .post(&url)
            .set("Content-Type", "application/json");
        if let Some(ref token) = self.auth_token {
            req = req.set("Authorization", &format!("Bearer {token}"));
        }
        let body_str = serde_json::to_string(body)?;
        match req.send_string(&body_str) {
            Ok(resp) => {
                let text = resp
                    .into_string()
                    .map_err(|e| SdkError::Transport(e.to_string()))?;
                Ok(serde_json::from_str(&text)?)
            }
            Err(ureq::Error::Status(code, resp)) => Err(api_err(code, resp)),
            Err(e) => Err(SdkError::Transport(e.to_string())),
        }
    }
}

/// Convert a ureq error response into an [`SdkError::Api`].
fn api_err(status: u16, resp: ureq::Response) -> SdkError {
    let body_text = resp.into_string().unwrap_or_default();
    match serde_json::from_str::<ApiErrorBody>(&body_text) {
        Ok(body) => SdkError::Api {
            status,
            code: body.code,
            message: body.error,
        },
        Err(_) => SdkError::Api {
            status,
            code: "UNKNOWN".to_string(),
            message: body_text,
        },
    }
}

/// Minimal URL-encoding for query string values (alphanumeric + `-_.~` pass through).
fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_builder() {
        let client = Client::new("http://localhost:9090/")
            .with_auth_token("secret")
            .with_timeout(Duration::from_secs(5));
        assert_eq!(client.base_url, "http://localhost:9090");
        assert_eq!(client.auth_token.as_deref(), Some("secret"));
    }

    #[test]
    fn test_url_encode() {
        assert_eq!(url_encode("hello"), "hello");
        assert_eq!(url_encode("hello world"), "hello%20world");
        assert_eq!(url_encode("a/b?c=d&e"), "a%2Fb%3Fc%3Dd%26e");
        assert_eq!(url_encode("abc-123_.~"), "abc-123_.~");
    }

    #[test]
    fn test_projection_lang_str() {
        assert_eq!(ProjectionLang::TypeScript.as_str(), "typescript");
        assert_eq!(ProjectionLang::Python.as_str(), "python");
        assert_eq!(ProjectionLang::Json.as_str(), "json");
        assert_eq!(ProjectionLang::Pseudocode.as_str(), "pseudocode");
    }

    #[test]
    fn test_interpret_limits_default() {
        let limits = InterpretLimits::default();
        assert_eq!(limits.max_steps, 1_000_000);
        assert_eq!(limits.max_call_depth, 1000);
    }

    #[test]
    fn test_unreachable_server_error() {
        // Use a port very unlikely to be bound. Should fail with Transport error
        // (not hang — the default timeout handles that).
        let client = Client::new("http://127.0.0.1:1").with_timeout(Duration::from_millis(200));
        let err = client.get_project().unwrap_err();
        assert!(matches!(err, SdkError::Transport(_)));
    }
}
