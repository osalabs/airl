//! AIRL Type Checker - Validates IR type and effect invariants.
//!
//! Performs bidirectional type checking over the IR graph:
//! - Verifies declared types match actual usage
//! - Checks effect declarations cover actual effects
//! - Validates function call signatures
//! - Ensures control flow type consistency
//!
//! # Example
//!
//! ```
//! use airl_typecheck::typecheck;
//! use airl_ir::Module;
//!
//! let json = r#"{
//!     "format_version":"0.1.0",
//!     "module":{"id":"m","name":"main",
//!         "metadata":{"version":"1","description":"","author":"","created_at":""},
//!         "imports":[],"exports":[],"types":[],
//!         "functions":[{
//!             "id":"f","name":"main","params":[],"returns":"Unit","effects":["IO"],
//!             "body":{"id":"n1","kind":"Literal","type":"Unit","value":null}
//!         }]}
//! }"#;
//! let module: Module = serde_json::from_str(json).unwrap();
//! let result = typecheck(&module);
//! assert!(result.is_ok());
//! ```

#![deny(missing_docs)]

use airl_ir::effects::Effect;
use airl_ir::node::{BinOpKind, LiteralValue, Node, UnaryOpKind};
use airl_ir::types::Type;
use airl_ir::Module;
use std::collections::HashMap;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Severity of a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    /// A type or effect error that must be fixed.
    Error,
    /// A potential issue that doesn't prevent type checking from succeeding.
    Warning,
}

/// A type checking diagnostic.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// Error or warning.
    pub severity: Severity,
    /// The IR node where the issue was detected, if applicable.
    pub node_id: Option<String>,
    /// Human-readable description of the issue.
    pub message: String,
    /// Optional function context (which function contains the error).
    pub function_context: Option<String>,
    /// Optional hint for how to fix the issue.
    pub hint: Option<String>,
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let level = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        if let Some(ref func) = self.function_context {
            if let Some(ref id) = self.node_id {
                write!(f, "{level} in {func} at {id}: {}", self.message)?;
            } else {
                write!(f, "{level} in {func}: {}", self.message)?;
            }
        } else if let Some(ref id) = self.node_id {
            write!(f, "{level} at {id}: {}", self.message)?;
        } else {
            write!(f, "{level}: {}", self.message)?;
        }
        if let Some(ref hint) = self.hint {
            write!(f, "\n  hint: {hint}")?;
        }
        Ok(())
    }
}

/// Errors from the type checking pass.
#[derive(Debug, Error)]
pub struct TypeCheckError {
    /// All diagnostics produced (both errors and warnings).
    pub diagnostics: Vec<Diagnostic>,
}

impl std::fmt::Display for TypeCheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "type check failed with {} error(s)",
            self.diagnostics.len()
        )
    }
}

/// Result of type checking.
///
/// Use [`TypeCheckResult::is_ok`] to check if type checking succeeded.
#[derive(Debug)]
pub struct TypeCheckResult {
    /// Type or effect errors (severity [`Severity::Error`]).
    pub errors: Vec<Diagnostic>,
    /// Non-blocking warnings (severity [`Severity::Warning`]).
    pub warnings: Vec<Diagnostic>,
}

impl TypeCheckResult {
    /// Returns `true` if no type errors were found.
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Function signature for builtins and user-defined functions
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct FuncSig {
    params: Vec<Type>,
    returns: Type,
    effects: Vec<Effect>,
}

// ---------------------------------------------------------------------------
// Type checking context
// ---------------------------------------------------------------------------

struct TypeChecker {
    /// User-defined function signatures
    functions: HashMap<String, FuncSig>,
    /// Builtin function signatures
    builtins: HashMap<String, FuncSig>,
    /// Local variable bindings (stack-based, name → type)
    locals: Vec<(String, Type)>,
    /// The effects declared by the current function being checked
    current_func_effects: Vec<Effect>,
    /// Name of the function currently being checked (for diagnostic context)
    current_func_name: Option<String>,

    errors: Vec<Diagnostic>,
    warnings: Vec<Diagnostic>,
}

impl TypeChecker {
    fn new() -> Self {
        let mut tc = Self {
            functions: HashMap::new(),
            builtins: HashMap::new(),
            locals: Vec::new(),
            current_func_effects: Vec::new(),
            current_func_name: None,
            errors: Vec::new(),
            warnings: Vec::new(),
        };
        tc.register_builtins();
        tc
    }

    fn register_builtins(&mut self) {
        let b = &mut self.builtins;

        // I/O
        b.insert(
            "std::io::println".into(),
            FuncSig {
                params: vec![Type::String],
                returns: Type::Unit,
                effects: vec![Effect::IO],
            },
        );
        b.insert(
            "std::io::print".into(),
            FuncSig {
                params: vec![Type::String],
                returns: Type::Unit,
                effects: vec![Effect::IO],
            },
        );
        b.insert(
            "std::io::eprintln".into(),
            FuncSig {
                params: vec![Type::String],
                returns: Type::Unit,
                effects: vec![Effect::IO],
            },
        );
        b.insert(
            "std::io::read_line".into(),
            FuncSig {
                params: vec![],
                returns: Type::String,
                effects: vec![Effect::IO],
            },
        );
        b.insert(
            "std::io::read_file".into(),
            FuncSig {
                params: vec![Type::String],
                returns: Type::String,
                effects: vec![Effect::IO],
            },
        );
        b.insert(
            "std::io::write_file".into(),
            FuncSig {
                params: vec![Type::String, Type::String],
                returns: Type::Bool,
                effects: vec![Effect::IO],
            },
        );
        b.insert(
            "std::io::read_dir".into(),
            FuncSig {
                params: vec![Type::String],
                returns: Type::Array {
                    element: Box::new(Type::String),
                },
                effects: vec![Effect::IO],
            },
        );
        b.insert(
            "std::io::file_exists".into(),
            FuncSig {
                params: vec![Type::String],
                returns: Type::Bool,
                effects: vec![Effect::IO],
            },
        );

        // String
        b.insert(
            "std::string::len".into(),
            FuncSig {
                params: vec![Type::String],
                returns: Type::I64,
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::string::concat".into(),
            FuncSig {
                params: vec![Type::String, Type::String],
                returns: Type::String,
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::string::from_i64".into(),
            FuncSig {
                params: vec![Type::I64],
                returns: Type::String,
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::string::contains".into(),
            FuncSig {
                params: vec![Type::String, Type::String],
                returns: Type::Bool,
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::string::split".into(),
            FuncSig {
                params: vec![Type::String, Type::String],
                returns: Type::Array {
                    element: Box::new(Type::String),
                },
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::string::to_i64".into(),
            FuncSig {
                params: vec![Type::String],
                returns: Type::I64,
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::string::starts_with".into(),
            FuncSig {
                params: vec![Type::String, Type::String],
                returns: Type::Bool,
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::string::ends_with".into(),
            FuncSig {
                params: vec![Type::String, Type::String],
                returns: Type::Bool,
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::string::trim".into(),
            FuncSig {
                params: vec![Type::String],
                returns: Type::String,
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::string::to_uppercase".into(),
            FuncSig {
                params: vec![Type::String],
                returns: Type::String,
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::string::to_lowercase".into(),
            FuncSig {
                params: vec![Type::String],
                returns: Type::String,
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::string::replace".into(),
            FuncSig {
                params: vec![Type::String, Type::String, Type::String],
                returns: Type::String,
                effects: vec![Effect::Pure],
            },
        );

        // Math
        b.insert(
            "std::math::abs".into(),
            FuncSig {
                params: vec![Type::I64],
                returns: Type::I64,
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::math::max".into(),
            FuncSig {
                params: vec![Type::I64, Type::I64],
                returns: Type::I64,
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::math::min".into(),
            FuncSig {
                params: vec![Type::I64, Type::I64],
                returns: Type::I64,
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::math::pow".into(),
            FuncSig {
                params: vec![Type::I64, Type::I64],
                returns: Type::I64,
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::math::sqrt".into(),
            FuncSig {
                params: vec![Type::F64],
                returns: Type::F64,
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::math::floor".into(),
            FuncSig {
                params: vec![Type::F64],
                returns: Type::I64,
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::math::ceil".into(),
            FuncSig {
                params: vec![Type::F64],
                returns: Type::I64,
                effects: vec![Effect::Pure],
            },
        );

        // Array
        b.insert(
            "std::array::len".into(),
            FuncSig {
                params: vec![Type::Array {
                    element: Box::new(Type::Unit),
                }], // generic: accepts any array
                returns: Type::I64,
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::array::push".into(),
            FuncSig {
                params: vec![
                    Type::Array {
                        element: Box::new(Type::Unit),
                    },
                    Type::Unit, // accepts any element
                ],
                returns: Type::Array {
                    element: Box::new(Type::Unit),
                },
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::array::get".into(),
            FuncSig {
                params: vec![
                    Type::Array {
                        element: Box::new(Type::Unit),
                    },
                    Type::I64,
                ],
                returns: Type::Unit, // returns element of any type
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::array::slice".into(),
            FuncSig {
                params: vec![
                    Type::Array {
                        element: Box::new(Type::Unit),
                    },
                    Type::I64,
                    Type::I64,
                ],
                returns: Type::Array {
                    element: Box::new(Type::Unit),
                },
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::array::contains".into(),
            FuncSig {
                params: vec![
                    Type::Array {
                        element: Box::new(Type::Unit),
                    },
                    Type::Unit,
                ],
                returns: Type::Bool,
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::array::reverse".into(),
            FuncSig {
                params: vec![Type::Array {
                    element: Box::new(Type::Unit),
                }],
                returns: Type::Array {
                    element: Box::new(Type::Unit),
                },
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::array::join".into(),
            FuncSig {
                params: vec![
                    Type::Array {
                        element: Box::new(Type::Unit),
                    },
                    Type::String,
                ],
                returns: Type::String,
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::array::range".into(),
            FuncSig {
                params: vec![Type::I64, Type::I64],
                returns: Type::Array {
                    element: Box::new(Type::Unit),
                },
                effects: vec![Effect::Pure],
            },
        );

        // Formatting
        b.insert(
            "std::fmt::format".into(),
            FuncSig {
                params: vec![Type::String], // variadic args not typed
                returns: Type::String,
                effects: vec![Effect::Pure],
            },
        );

        // Environment
        b.insert(
            "std::env::args".into(),
            FuncSig {
                params: vec![],
                returns: Type::Array {
                    element: Box::new(Type::String),
                },
                effects: vec![Effect::IO],
            },
        );

        // JSON
        b.insert(
            "std::json::parse".into(),
            FuncSig {
                params: vec![Type::String],
                returns: Type::Unit, // returns dynamic value
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::json::serialize".into(),
            FuncSig {
                params: vec![Type::Unit], // accepts any value
                returns: Type::String,
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::json::serialize_pretty".into(),
            FuncSig {
                params: vec![Type::Unit],
                returns: Type::String,
                effects: vec![Effect::Pure],
            },
        );

        // Collections
        b.insert(
            "std::collections::new_map".into(),
            FuncSig {
                params: vec![],
                returns: Type::Unit, // returns Map (dynamic)
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::collections::insert".into(),
            FuncSig {
                params: vec![Type::Unit, Type::String, Type::Unit],
                returns: Type::Unit,
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::collections::get".into(),
            FuncSig {
                params: vec![Type::Unit, Type::String],
                returns: Type::Unit,
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::collections::remove".into(),
            FuncSig {
                params: vec![Type::Unit, Type::String],
                returns: Type::Unit,
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::collections::contains_key".into(),
            FuncSig {
                params: vec![Type::Unit, Type::String],
                returns: Type::Bool,
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::collections::keys".into(),
            FuncSig {
                params: vec![Type::Unit],
                returns: Type::Array {
                    element: Box::new(Type::String),
                },
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::collections::values".into(),
            FuncSig {
                params: vec![Type::Unit],
                returns: Type::Array {
                    element: Box::new(Type::Unit),
                },
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::collections::map_len".into(),
            FuncSig {
                params: vec![Type::Unit],
                returns: Type::I64,
                effects: vec![Effect::Pure],
            },
        );

        // Extended string operations
        b.insert(
            "std::string::index_of".into(),
            FuncSig {
                params: vec![Type::String, Type::String],
                returns: Type::I64,
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::string::substring".into(),
            FuncSig {
                params: vec![Type::String, Type::I64, Type::I64],
                returns: Type::String,
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::string::chars".into(),
            FuncSig {
                params: vec![Type::String],
                returns: Type::Array {
                    element: Box::new(Type::String),
                },
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::string::repeat".into(),
            FuncSig {
                params: vec![Type::String, Type::I64],
                returns: Type::String,
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::string::parse_int".into(),
            FuncSig {
                params: vec![Type::String],
                returns: Type::I64,
                effects: vec![Effect::Pure],
            },
        );

        // Time
        b.insert(
            "std::time::now_ms".into(),
            FuncSig {
                params: vec![],
                returns: Type::I64,
                effects: vec![Effect::IO],
            },
        );
        b.insert(
            "std::time::now_secs".into(),
            FuncSig {
                params: vec![],
                returns: Type::I64,
                effects: vec![Effect::IO],
            },
        );
        b.insert(
            "std::time::sleep_ms".into(),
            FuncSig {
                params: vec![Type::I64],
                returns: Type::Unit,
                effects: vec![Effect::IO],
            },
        );

        // Crypto
        b.insert(
            "std::crypto::sha256".into(),
            FuncSig {
                params: vec![Type::String],
                returns: Type::String,
                effects: vec![Effect::Pure],
            },
        );

        // Testing
        b.insert(
            "std::testing::assert_eq".into(),
            FuncSig {
                params: vec![Type::Unit, Type::Unit],
                returns: Type::Bool,
                effects: vec![Effect::IO],
            },
        );
        b.insert(
            "std::testing::assert_ne".into(),
            FuncSig {
                params: vec![Type::Unit, Type::Unit],
                returns: Type::Bool,
                effects: vec![Effect::IO],
            },
        );
        b.insert(
            "std::testing::assert_true".into(),
            FuncSig {
                params: vec![Type::Bool],
                returns: Type::Bool,
                effects: vec![Effect::IO],
            },
        );

        // Concurrency
        b.insert(
            "std::concurrency::spawn".into(),
            FuncSig {
                params: vec![Type::String], // func name + optional args
                returns: Type::I64,         // handle ID
                effects: vec![Effect::IO],
            },
        );
        b.insert(
            "std::concurrency::await_result".into(),
            FuncSig {
                params: vec![Type::I64],
                returns: Type::Unit, // returns whatever the spawned function returned
                effects: vec![Effect::IO],
            },
        );
        b.insert(
            "std::concurrency::sleep".into(),
            FuncSig {
                params: vec![Type::I64],
                returns: Type::Unit,
                effects: vec![Effect::IO],
            },
        );
        b.insert(
            "std::concurrency::thread_id".into(),
            FuncSig {
                params: vec![],
                returns: Type::I64,
                effects: vec![Effect::IO],
            },
        );

        // HTTP server
        b.insert(
            "std::net::serve_once".into(),
            FuncSig {
                params: vec![Type::I64, Type::String],
                returns: Type::Bool,
                effects: vec![Effect::IO],
            },
        );

        // HTTP client
        b.insert(
            "std::net::http_get".into(),
            FuncSig {
                params: vec![Type::String],
                returns: Type::Unit, // returns Map {status, body, error?}
                effects: vec![Effect::IO],
            },
        );
        b.insert(
            "std::net::http_post".into(),
            FuncSig {
                params: vec![Type::String, Type::String],
                returns: Type::Unit, // returns Map {status, body, error?}
                effects: vec![Effect::IO],
            },
        );

        // Process
        b.insert(
            "std::process::exit".into(),
            FuncSig {
                params: vec![Type::I64],
                returns: Type::Unit,
                effects: vec![Effect::IO],
            },
        );
        b.insert(
            "std::process::exec".into(),
            FuncSig {
                params: vec![Type::String],
                returns: Type::Unit, // returns Map
                effects: vec![Effect::IO],
            },
        );
        b.insert(
            "std::process::env_var".into(),
            FuncSig {
                params: vec![Type::String],
                returns: Type::String,
                effects: vec![Effect::IO],
            },
        );
        b.insert(
            "std::process::set_env_var".into(),
            FuncSig {
                params: vec![Type::String, Type::String],
                returns: Type::Unit,
                effects: vec![Effect::IO],
            },
        );

        // Error handling
        b.insert(
            "std::error::is_unit".into(),
            FuncSig {
                params: vec![Type::Unit],
                returns: Type::Bool,
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::error::unwrap_or".into(),
            FuncSig {
                params: vec![Type::Unit, Type::Unit],
                returns: Type::Unit,
                effects: vec![Effect::Pure],
            },
        );
        b.insert(
            "std::error::assert".into(),
            FuncSig {
                params: vec![Type::Bool],
                returns: Type::Unit,
                effects: vec![Effect::IO],
            },
        );
        b.insert(
            "std::error::panic".into(),
            FuncSig {
                params: vec![Type::String],
                returns: Type::Unit,
                effects: vec![Effect::Fail {
                    error_type: "Panic".to_string(),
                }],
            },
        );
    }

    fn error(&mut self, node_id: &str, message: impl Into<String>) {
        self.errors.push(Diagnostic {
            severity: Severity::Error,
            node_id: Some(node_id.to_string()),
            message: message.into(),
            function_context: self.current_func_name.clone(),
            hint: None,
        });
    }

    fn error_with_hint(
        &mut self,
        node_id: &str,
        message: impl Into<String>,
        hint: impl Into<String>,
    ) {
        self.errors.push(Diagnostic {
            severity: Severity::Error,
            node_id: Some(node_id.to_string()),
            message: message.into(),
            function_context: self.current_func_name.clone(),
            hint: Some(hint.into()),
        });
    }

    fn warning(&mut self, node_id: &str, message: impl Into<String>) {
        self.warnings.push(Diagnostic {
            severity: Severity::Warning,
            node_id: Some(node_id.to_string()),
            message: message.into(),
            function_context: self.current_func_name.clone(),
            hint: None,
        });
    }

    /// Check a node and return its inferred type.
    fn check(&mut self, node: &Node) -> Type {
        match node {
            Node::Literal {
                id,
                node_type,
                value,
            } => {
                let expected = self.literal_type(value);
                if !types_compatible(&expected, node_type) {
                    self.error(
                        id.as_str(),
                        format!(
                            "literal type mismatch: value is {}, declared as {}",
                            type_name(&expected),
                            type_name(node_type)
                        ),
                    );
                }
                node_type.clone()
            }

            Node::Param {
                id,
                name,
                node_type,
                ..
            } => {
                if let Some(bound_type) = self.lookup_local(name) {
                    if !types_compatible(&bound_type, node_type) {
                        self.error(
                            id.as_str(),
                            format!(
                                "variable '{name}' has type {}, but declared as {}",
                                type_name(&bound_type),
                                type_name(node_type)
                            ),
                        );
                    }
                    node_type.clone()
                } else {
                    self.error(id.as_str(), format!("undefined variable: {name}"));
                    node_type.clone()
                }
            }

            Node::Let {
                id: _,
                name,
                node_type,
                value,
                body,
            } => {
                let val_type = self.check(value);
                self.locals.push((name.clone(), val_type));
                let body_type = self.check(body);
                self.locals.pop();
                if !types_compatible(&body_type, node_type) && !is_unit(node_type) {
                    // Let's node_type usually reflects the body type; only warn if explicitly wrong
                }
                body_type
            }

            Node::If {
                id,
                node_type,
                cond,
                then_branch,
                else_branch,
            } => {
                let cond_type = self.check(cond);
                if !types_compatible(&cond_type, &Type::Bool) {
                    self.error(
                        id.as_str(),
                        format!("if condition must be Bool, got {}", type_name(&cond_type)),
                    );
                }
                let then_type = self.check(then_branch);
                let else_type = self.check(else_branch);
                if !types_compatible(&then_type, &else_type) {
                    self.error(
                        id.as_str(),
                        format!(
                            "if branches have different types: {} vs {}",
                            type_name(&then_type),
                            type_name(&else_type)
                        ),
                    );
                }
                if !types_compatible(&then_type, node_type) {
                    self.error(
                        id.as_str(),
                        format!(
                            "if declared type {} doesn't match branch type {}",
                            type_name(node_type),
                            type_name(&then_type)
                        ),
                    );
                }
                node_type.clone()
            }

            Node::Call {
                id,
                node_type,
                target,
                args,
            } => {
                // Evaluate argument types
                let arg_types: Vec<Type> = args.iter().map(|a| self.check(a)).collect();

                // Find function signature
                if let Some(sig) = self.lookup_function(target) {
                    // Check argument count (skip for variadic builtins)
                    if arg_types.len() != sig.params.len() && !is_variadic(target) {
                        self.error(
                            id.as_str(),
                            format!(
                                "function '{target}' expects {} argument(s), got {}",
                                sig.params.len(),
                                arg_types.len()
                            ),
                        );
                    } else {
                        // Check argument types (with flexibility for builtins that accept display-able values)
                        for (i, (arg_t, param_t)) in
                            arg_types.iter().zip(sig.params.iter()).enumerate()
                        {
                            if !types_compatible(arg_t, param_t)
                                && !is_printable_to_string(target, param_t)
                            {
                                self.error(
                                    id.as_str(),
                                    format!(
                                        "argument {} of '{target}': expected {}, got {}",
                                        i,
                                        type_name(param_t),
                                        type_name(arg_t)
                                    ),
                                );
                            }
                        }
                    }

                    // Check return type
                    if !types_compatible(&sig.returns, node_type) {
                        self.error(
                            id.as_str(),
                            format!(
                                "function '{target}' returns {}, but call declares {}",
                                type_name(&sig.returns),
                                type_name(node_type)
                            ),
                        );
                    }

                    // Check effects
                    self.check_effects(id.as_str(), target, &sig.effects);
                } else {
                    self.warning(id.as_str(), format!("unknown function: {target}"));
                }

                node_type.clone()
            }

            Node::Return {
                id: _,
                node_type,
                value,
            } => {
                self.check(value);
                node_type.clone()
            }

            Node::BinOp {
                id,
                op,
                node_type,
                lhs,
                rhs,
            } => {
                let lhs_type = self.check(lhs);
                let rhs_type = self.check(rhs);

                if !types_compatible(&lhs_type, &rhs_type) {
                    // Allow String + String
                    if !(matches!(op, BinOpKind::Add) && types_compatible(&lhs_type, &Type::String))
                    {
                        self.error(
                            id.as_str(),
                            format!(
                                "binary op {:?}: operands must have same type, got {} and {}",
                                op,
                                type_name(&lhs_type),
                                type_name(&rhs_type)
                            ),
                        );
                    }
                }

                // Check result type makes sense
                let expected_result = self.binop_result_type(op, &lhs_type);
                if !types_compatible(&expected_result, node_type) {
                    self.error(
                        id.as_str(),
                        format!(
                            "binary op {:?} on {} should produce {}, declared as {}",
                            op,
                            type_name(&lhs_type),
                            type_name(&expected_result),
                            type_name(node_type)
                        ),
                    );
                }

                node_type.clone()
            }

            Node::UnaryOp {
                id,
                op,
                node_type,
                operand,
            } => {
                let operand_type = self.check(operand);
                match op {
                    UnaryOpKind::Neg => {
                        if !is_numeric(&operand_type) {
                            self.error(
                                id.as_str(),
                                format!(
                                    "unary Neg requires numeric type, got {}",
                                    type_name(&operand_type)
                                ),
                            );
                        }
                    }
                    UnaryOpKind::Not => {
                        if !types_compatible(&operand_type, &Type::Bool) {
                            self.error(
                                id.as_str(),
                                format!(
                                    "unary Not requires Bool, got {}",
                                    type_name(&operand_type)
                                ),
                            );
                        }
                    }
                    UnaryOpKind::BitNot => {
                        if !is_integer(&operand_type) {
                            self.error(
                                id.as_str(),
                                format!(
                                    "unary BitNot requires integer type, got {}",
                                    type_name(&operand_type)
                                ),
                            );
                        }
                    }
                }
                node_type.clone()
            }

            Node::Block {
                statements, result, ..
            } => {
                for stmt in statements {
                    self.check(stmt);
                }
                self.check(result)
            }

            Node::Loop { body, .. } => {
                self.check(body);
                Type::Unit
            }

            Node::Match {
                id,
                node_type,
                scrutinee,
                arms,
            } => {
                self.check(scrutinee);
                for arm in arms {
                    let arm_type = self.check(&arm.body);
                    if !types_compatible(&arm_type, node_type) {
                        self.error(
                            id.as_str(),
                            format!(
                                "match arm type {} doesn't match declared type {}",
                                type_name(&arm_type),
                                type_name(node_type)
                            ),
                        );
                    }
                }
                node_type.clone()
            }

            Node::StructLiteral {
                node_type, fields, ..
            } => {
                for (_, node) in fields {
                    self.check(node);
                }
                node_type.clone()
            }

            Node::FieldAccess {
                id,
                node_type,
                object,
                field,
            } => {
                let obj_type = self.check(object);
                // We can only check struct field access if we have the struct definition
                // For Named types, we warn but don't error (struct definitions not yet tracked)
                if let Type::Struct { fields, .. } = &obj_type {
                    if !fields.iter().any(|(name, _)| name.as_str() == field) {
                        self.error(id.as_str(), format!("struct has no field '{field}'"));
                    }
                }
                node_type.clone()
            }

            Node::ArrayLiteral {
                id,
                node_type,
                elements,
            } => {
                if elements.len() >= 2 {
                    let first_type = self.check(&elements[0]);
                    for el in &elements[1..] {
                        let el_type = self.check(el);
                        if !types_compatible(&el_type, &first_type) {
                            self.error(
                                id.as_str(),
                                format!(
                                    "array element type mismatch: expected {}, got {}",
                                    type_name(&first_type),
                                    type_name(&el_type)
                                ),
                            );
                        }
                    }
                } else {
                    for el in elements {
                        self.check(el);
                    }
                }
                node_type.clone()
            }

            Node::IndexAccess {
                id,
                node_type,
                array,
                index,
            } => {
                let arr_type = self.check(array);
                let idx_type = self.check(index);
                if !is_array_type(&arr_type) {
                    self.error(
                        id.as_str(),
                        format!(
                            "index access requires array type, got {}",
                            type_name(&arr_type)
                        ),
                    );
                }
                if !is_integer(&idx_type) {
                    self.error(
                        id.as_str(),
                        format!("array index must be integer, got {}", type_name(&idx_type)),
                    );
                }
                node_type.clone()
            }

            Node::Error { id, message } => {
                self.error(id.as_str(), format!("IR error node: {message}"));
                Type::Unit
            }
        }
    }

    fn literal_type(&self, value: &LiteralValue) -> Type {
        match value {
            LiteralValue::Integer(_) => Type::I64,
            LiteralValue::Float(_) => Type::F64,
            LiteralValue::Boolean(_) => Type::Bool,
            LiteralValue::Str(_) => Type::String,
            LiteralValue::Unit => Type::Unit,
        }
    }

    fn lookup_local(&self, name: &str) -> Option<Type> {
        for (n, t) in self.locals.iter().rev() {
            if n == name {
                return Some(t.clone());
            }
        }
        None
    }

    fn lookup_function(&self, name: &str) -> Option<FuncSig> {
        if let Some(sig) = self.builtins.get(name) {
            return Some(sig.clone());
        }
        if let Some(sig) = self.functions.get(name) {
            return Some(sig.clone());
        }
        None
    }

    fn check_effects(&mut self, node_id: &str, target: &str, callee_effects: &[Effect]) {
        for effect in callee_effects {
            if *effect == Effect::Pure {
                continue; // Pure is always allowed
            }
            if !self.current_func_effects_cover(effect) {
                self.error_with_hint(
                    node_id,
                    format!(
                        "calling '{target}' requires effect {}, but current function doesn't declare it",
                        effect_name(effect)
                    ),
                    format!(
                        "add \"{}\" to the function's effects list",
                        effect_name(effect)
                    ),
                );
            }
        }
    }

    fn current_func_effects_cover(&self, effect: &Effect) -> bool {
        for declared in &self.current_func_effects {
            if *declared == *effect {
                return true;
            }
            // IO subsumes Read(*) and Write(*)
            if *declared == Effect::IO
                && matches!(effect, Effect::Read { .. } | Effect::Write { .. })
            {
                return true;
            }
        }
        false
    }

    fn binop_result_type(&self, op: &BinOpKind, operand_type: &Type) -> Type {
        match op {
            // Comparison operators always return Bool
            BinOpKind::Eq
            | BinOpKind::Neq
            | BinOpKind::Lt
            | BinOpKind::Lte
            | BinOpKind::Gt
            | BinOpKind::Gte => Type::Bool,
            // Logical operators return Bool
            BinOpKind::And | BinOpKind::Or => Type::Bool,
            // Arithmetic and bitwise return the operand type
            _ => operand_type.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Type helpers
// ---------------------------------------------------------------------------

/// Check if two types are compatible (equal, or one is a Named type that we accept loosely).
fn types_compatible(a: &Type, b: &Type) -> bool {
    if a == b {
        return true;
    }
    // Unit acts as a wildcard "any" type (used by dynamic builtins like collections/JSON)
    if matches!(a, Type::Unit) || matches!(b, Type::Unit) {
        return true;
    }
    // Named types are loosely compatible with anything (we don't have full struct defs yet)
    if matches!(a, Type::Named(_)) || matches!(b, Type::Named(_)) {
        return true;
    }
    // Array types: compare elements
    if let (Type::Array { element: ea }, Type::Array { element: eb }) = (a, b) {
        return types_compatible(ea, eb);
    }
    false
}

fn is_numeric(t: &Type) -> bool {
    matches!(
        t,
        Type::I8
            | Type::I16
            | Type::I32
            | Type::I64
            | Type::U8
            | Type::U16
            | Type::U32
            | Type::U64
            | Type::F32
            | Type::F64
    )
}

fn is_integer(t: &Type) -> bool {
    matches!(
        t,
        Type::I8 | Type::I16 | Type::I32 | Type::I64 | Type::U8 | Type::U16 | Type::U32 | Type::U64
    )
}

fn is_unit(t: &Type) -> bool {
    matches!(t, Type::Unit)
}

fn is_array_type(t: &Type) -> bool {
    matches!(t, Type::Array { .. } | Type::Named(_))
}

/// Functions that accept any type as arguments (auto-display / variadic).
fn is_printable_to_string(target: &str, _param_type: &Type) -> bool {
    matches!(
        target,
        "std::io::println"
            | "std::io::print"
            | "std::io::eprintln"
            | "std::fmt::format"
            | "std::array::join"
    )
}

/// Functions that are variadic (accept any number of arguments).
fn is_variadic(target: &str) -> bool {
    matches!(
        target,
        "std::io::println"
            | "std::io::print"
            | "std::io::eprintln"
            | "std::fmt::format"
            | "std::string::concat"
    )
}

fn type_name(t: &Type) -> String {
    t.to_type_str()
}

fn effect_name(e: &Effect) -> String {
    e.to_effect_str()
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Run type checking on a module.
pub fn typecheck(module: &Module) -> TypeCheckResult {
    let mut tc = TypeChecker::new();

    // Register all user-defined function signatures
    for func in module.functions() {
        let sig = FuncSig {
            params: func.params.iter().map(|p| p.param_type.clone()).collect(),
            returns: func.returns.clone(),
            effects: func.effects.clone(),
        };
        tc.functions.insert(func.name.clone(), sig);
    }

    // Check each function
    for func in module.functions() {
        tc.current_func_effects = func.effects.clone();
        tc.current_func_name = Some(func.name.clone());
        // Bind function parameters as locals
        tc.locals.clear();
        for param in &func.params {
            tc.locals
                .push((param.name.clone(), param.param_type.clone()));
        }
        tc.check(&func.body);
    }

    TypeCheckResult {
        errors: tc.errors,
        warnings: tc.warnings,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn load_module(json: &str) -> Module {
        serde_json::from_str(json).unwrap()
    }

    /// Helper: wrap a body + optional extra functions into a module
    fn module_json(func_json: &str) -> String {
        format!(
            r#"{{
            "format_version": "0.1.0",
            "module": {{
                "id": "mod_main", "name": "main",
                "metadata": {{"version": "1.0.0", "description": "", "author": "", "created_at": ""}},
                "imports": [], "exports": [], "types": [],
                "functions": [{func_json}]
            }}
        }}"#
        )
    }

    fn func_json(name: &str, params: &str, returns: &str, effects: &str, body: &str) -> String {
        format!(
            r#"{{
            "id": "f_{name}", "name": "{name}",
            "params": [{params}],
            "returns": "{returns}",
            "effects": [{effects}],
            "body": {body}
        }}"#
        )
    }

    fn check_ok(json: &str) {
        let module = load_module(json);
        let result = typecheck(&module);
        for e in &result.errors {
            eprintln!("  {e}");
        }
        assert!(
            result.is_ok(),
            "expected OK, got {} error(s)",
            result.errors.len()
        );
    }

    fn check_errors(json: &str) -> Vec<Diagnostic> {
        let module = load_module(json);
        let result = typecheck(&module);
        assert!(!result.is_ok(), "expected errors, got OK");
        result.errors
    }

    fn check_has_error(json: &str, substring: &str) {
        let errors = check_errors(json);
        assert!(
            errors.iter().any(|e| e.message.contains(substring)),
            "expected error containing '{substring}', got: {:?}",
            errors.iter().map(|e| &e.message).collect::<Vec<_>>()
        );
    }

    // ---- Valid programs ----

    #[test]
    fn test_valid_hello_world() {
        let json = module_json(&func_json(
            "main",
            "",
            "Unit",
            r#""IO""#,
            r#"{"id": "n_1", "kind": "Call", "type": "Unit", "target": "std::io::println",
                "args": [{"id": "n_2", "kind": "Literal", "type": "String", "value": "hello"}]}"#,
        ));
        check_ok(&json);
    }

    #[test]
    fn test_valid_arithmetic() {
        let json = module_json(&func_json(
            "main",
            "",
            "Unit",
            r#""IO""#,
            r#"{"id": "n_1", "kind": "Call", "type": "Unit", "target": "std::io::println",
                "args": [{"id": "n_2", "kind": "BinOp", "type": "I64", "op": "Add",
                    "lhs": {"id": "n_3", "kind": "Literal", "type": "I64", "value": 1},
                    "rhs": {"id": "n_4", "kind": "Literal", "type": "I64", "value": 2}}]}"#,
        ));
        check_ok(&json);
    }

    #[test]
    fn test_valid_let_binding() {
        let json = module_json(&func_json(
            "main",
            "",
            "Unit",
            r#""IO""#,
            r#"{"id": "n_1", "kind": "Let", "type": "Unit", "name": "x",
                "value": {"id": "n_2", "kind": "Literal", "type": "I64", "value": 42},
                "body": {"id": "n_3", "kind": "Call", "type": "Unit", "target": "std::io::println",
                    "args": [{"id": "n_4", "kind": "Param", "type": "I64", "name": "x", "index": 0}]}}"#,
        ));
        check_ok(&json);
    }

    #[test]
    fn test_valid_if() {
        let json = module_json(&func_json(
            "main",
            "",
            "Unit",
            r#""IO""#,
            r#"{"id": "n_1", "kind": "If", "type": "I64",
                "cond": {"id": "n_2", "kind": "Literal", "type": "Bool", "value": true},
                "then_branch": {"id": "n_3", "kind": "Literal", "type": "I64", "value": 1},
                "else_branch": {"id": "n_4", "kind": "Literal", "type": "I64", "value": 0}}"#,
        ));
        check_ok(&json);
    }

    #[test]
    fn test_valid_user_function_call() {
        let main_fn = func_json(
            "main",
            "",
            "Unit",
            r#""IO""#,
            r#"{"id": "n_1", "kind": "Call", "type": "Unit", "target": "std::io::println",
                "args": [{"id": "n_2", "kind": "Call", "type": "I64", "target": "double",
                    "args": [{"id": "n_3", "kind": "Literal", "type": "I64", "value": 21}]}]}"#,
        );
        let double_fn = func_json(
            "double",
            r#"{"name": "n", "type": "I64", "index": 0}"#,
            "I64",
            r#""Pure""#,
            r#"{"id": "n_10", "kind": "BinOp", "type": "I64", "op": "Mul",
                "lhs": {"id": "n_11", "kind": "Param", "type": "I64", "name": "n", "index": 0},
                "rhs": {"id": "n_12", "kind": "Literal", "type": "I64", "value": 2}}"#,
        );
        let json = format!(
            r#"{{"format_version": "0.1.0", "module": {{
                "id": "m", "name": "main",
                "metadata": {{"version": "1.0.0", "description": "", "author": "", "created_at": ""}},
                "imports": [], "exports": [], "types": [],
                "functions": [{main_fn}, {double_fn}]
            }}}}"#
        );
        check_ok(&json);
    }

    #[test]
    fn test_valid_empty_module() {
        let json = r#"{"format_version": "0.1.0", "module": {
            "id": "m", "name": "main",
            "metadata": {"version": "1.0.0", "description": "", "author": "", "created_at": ""},
            "imports": [], "exports": [], "types": [], "functions": []
        }}"#;
        check_ok(json);
    }

    // ---- Error cases ----

    #[test]
    fn test_error_if_condition_not_bool() {
        let json = module_json(&func_json(
            "main",
            "",
            "Unit",
            r#""IO""#,
            r#"{"id": "n_1", "kind": "If", "type": "I64",
                "cond": {"id": "n_2", "kind": "Literal", "type": "I64", "value": 42},
                "then_branch": {"id": "n_3", "kind": "Literal", "type": "I64", "value": 1},
                "else_branch": {"id": "n_4", "kind": "Literal", "type": "I64", "value": 0}}"#,
        ));
        check_has_error(&json, "if condition must be Bool");
    }

    #[test]
    fn test_error_if_branches_different_types() {
        let json = module_json(&func_json(
            "main",
            "",
            "Unit",
            r#""IO""#,
            r#"{"id": "n_1", "kind": "If", "type": "I64",
                "cond": {"id": "n_2", "kind": "Literal", "type": "Bool", "value": true},
                "then_branch": {"id": "n_3", "kind": "Literal", "type": "I64", "value": 1},
                "else_branch": {"id": "n_4", "kind": "Literal", "type": "String", "value": "no"}}"#,
        ));
        check_has_error(&json, "branches have different types");
    }

    #[test]
    fn test_error_binop_type_mismatch() {
        let json = module_json(&func_json(
            "main",
            "",
            "Unit",
            r#""IO""#,
            r#"{"id": "n_1", "kind": "BinOp", "type": "I64", "op": "Add",
                "lhs": {"id": "n_2", "kind": "Literal", "type": "I64", "value": 1},
                "rhs": {"id": "n_3", "kind": "Literal", "type": "String", "value": "two"}}"#,
        ));
        check_has_error(&json, "operands must have same type");
    }

    #[test]
    fn test_error_binop_wrong_result_type() {
        // Eq on I64 returns Bool, but declared as I64
        let json = module_json(&func_json(
            "main",
            "",
            "Unit",
            r#""IO""#,
            r#"{"id": "n_1", "kind": "BinOp", "type": "I64", "op": "Eq",
                "lhs": {"id": "n_2", "kind": "Literal", "type": "I64", "value": 1},
                "rhs": {"id": "n_3", "kind": "Literal", "type": "I64", "value": 2}}"#,
        ));
        check_has_error(&json, "should produce Bool");
    }

    #[test]
    fn test_error_wrong_arg_count() {
        let json = module_json(&func_json(
            "main",
            "",
            "Unit",
            r#""IO""#,
            r#"{"id": "n_1", "kind": "Call", "type": "I64", "target": "std::math::max",
                "args": [{"id": "n_2", "kind": "Literal", "type": "I64", "value": 1}]}"#,
        ));
        check_has_error(&json, "expects 2 argument(s), got 1");
    }

    #[test]
    fn test_error_undefined_variable() {
        let json = module_json(&func_json(
            "main",
            "",
            "Unit",
            r#""IO""#,
            r#"{"id": "n_1", "kind": "Param", "type": "I64", "name": "x", "index": 0}"#,
        ));
        check_has_error(&json, "undefined variable: x");
    }

    #[test]
    fn test_error_literal_type_mismatch() {
        let json = module_json(&func_json(
            "main",
            "",
            "Unit",
            r#""IO""#,
            r#"{"id": "n_1", "kind": "Literal", "type": "Bool", "value": 42}"#,
        ));
        check_has_error(&json, "literal type mismatch");
    }

    #[test]
    fn test_error_unary_neg_on_bool() {
        let json = module_json(&func_json(
            "main",
            "",
            "Unit",
            r#""IO""#,
            r#"{"id": "n_1", "kind": "UnaryOp", "type": "Bool", "op": "Neg",
                "operand": {"id": "n_2", "kind": "Literal", "type": "Bool", "value": true}}"#,
        ));
        check_has_error(&json, "unary Neg requires numeric");
    }

    #[test]
    fn test_error_unary_not_on_int() {
        let json = module_json(&func_json(
            "main",
            "",
            "Unit",
            r#""IO""#,
            r#"{"id": "n_1", "kind": "UnaryOp", "type": "I64", "op": "Not",
                "operand": {"id": "n_2", "kind": "Literal", "type": "I64", "value": 42}}"#,
        ));
        check_has_error(&json, "unary Not requires Bool");
    }

    #[test]
    fn test_error_index_non_integer() {
        let json = module_json(&func_json(
            "main",
            "",
            "Unit",
            r#""IO""#,
            r#"{"id": "n_1", "kind": "Let", "type": "Unit", "name": "arr",
                "value": {"id": "n_2", "kind": "ArrayLiteral", "type": "Array<I64>",
                    "elements": [{"id": "n_3", "kind": "Literal", "type": "I64", "value": 1}]},
                "body": {"id": "n_4", "kind": "IndexAccess", "type": "I64",
                    "array": {"id": "n_5", "kind": "Param", "type": "Array<I64>", "name": "arr", "index": 0},
                    "index": {"id": "n_6", "kind": "Literal", "type": "String", "value": "zero"}}}"#,
        ));
        check_has_error(&json, "array index must be integer");
    }

    #[test]
    fn test_error_array_element_type_mismatch() {
        let json = module_json(&func_json(
            "main",
            "",
            "Unit",
            r#""IO""#,
            r#"{"id": "n_1", "kind": "ArrayLiteral", "type": "Array<I64>",
                "elements": [
                    {"id": "n_2", "kind": "Literal", "type": "I64", "value": 1},
                    {"id": "n_3", "kind": "Literal", "type": "String", "value": "two"}
                ]}"#,
        ));
        check_has_error(&json, "array element type mismatch");
    }

    #[test]
    fn test_error_effect_violation_pure_calls_io() {
        // A Pure function calling println (which requires IO)
        let json = module_json(&func_json(
            "main",
            "",
            "Unit",
            r#""Pure""#,
            r#"{"id": "n_1", "kind": "Call", "type": "Unit", "target": "std::io::println",
                "args": [{"id": "n_2", "kind": "Literal", "type": "String", "value": "hi"}]}"#,
        ));
        check_has_error(&json, "requires effect IO");
    }

    #[test]
    fn test_error_ir_error_node() {
        let json = module_json(&func_json(
            "main",
            "",
            "Unit",
            r#""IO""#,
            r#"{"id": "n_1", "kind": "Error", "message": "broken"}"#,
        ));
        check_has_error(&json, "IR error node");
    }

    #[test]
    fn test_error_call_return_type_mismatch() {
        // std::string::len returns I64 but call declares String
        let json = module_json(&func_json(
            "main",
            "",
            "Unit",
            r#""Pure""#,
            r#"{"id": "n_1", "kind": "Call", "type": "String", "target": "std::string::len",
                "args": [{"id": "n_2", "kind": "Literal", "type": "String", "value": "hi"}]}"#,
        ));
        check_has_error(&json, "returns I64, but call declares String");
    }

    #[test]
    fn test_warning_unknown_function() {
        let json = module_json(&func_json(
            "main",
            "",
            "Unit",
            r#""IO""#,
            r#"{"id": "n_1", "kind": "Call", "type": "Unit", "target": "nonexistent",
                "args": []}"#,
        ));
        let module = load_module(&json);
        let result = typecheck(&module);
        assert!(result.is_ok()); // warnings don't fail
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].message.contains("nonexistent"));
    }

    #[test]
    fn test_error_user_func_wrong_arg_type() {
        let main_fn = func_json(
            "main",
            "",
            "Unit",
            r#""IO""#,
            r#"{"id": "n_1", "kind": "Call", "type": "I64", "target": "add_one",
                "args": [{"id": "n_2", "kind": "Literal", "type": "String", "value": "hello"}]}"#,
        );
        let add_one_fn = func_json(
            "add_one",
            r#"{"name": "n", "type": "I64", "index": 0}"#,
            "I64",
            r#""Pure""#,
            r#"{"id": "n_10", "kind": "BinOp", "type": "I64", "op": "Add",
                "lhs": {"id": "n_11", "kind": "Param", "type": "I64", "name": "n", "index": 0},
                "rhs": {"id": "n_12", "kind": "Literal", "type": "I64", "value": 1}}"#,
        );
        let json = format!(
            r#"{{"format_version": "0.1.0", "module": {{
                "id": "m", "name": "main",
                "metadata": {{"version": "1.0.0", "description": "", "author": "", "created_at": ""}},
                "imports": [], "exports": [], "types": [],
                "functions": [{main_fn}, {add_one_fn}]
            }}}}"#
        );
        check_has_error(&json, "expected I64, got String");
    }

    #[test]
    fn test_error_variable_type_mismatch() {
        // Bind x as I64, then reference it as String
        let json = module_json(&func_json(
            "main",
            "",
            "Unit",
            r#""IO""#,
            r#"{"id": "n_1", "kind": "Let", "type": "Unit", "name": "x",
                "value": {"id": "n_2", "kind": "Literal", "type": "I64", "value": 42},
                "body": {"id": "n_3", "kind": "Param", "type": "String", "name": "x", "index": 0}}"#,
        ));
        check_has_error(&json, "variable 'x' has type I64, but declared as String");
    }

    #[test]
    fn test_valid_match() {
        let json = module_json(&func_json(
            "main",
            "",
            "Unit",
            r#""IO""#,
            r#"{"id": "n_1", "kind": "Match", "type": "String",
                "scrutinee": {"id": "n_2", "kind": "Literal", "type": "I64", "value": 1},
                "arms": [
                    {"pattern": {"kind": "Literal", "value": 1},
                     "body": {"id": "n_3", "kind": "Literal", "type": "String", "value": "one"}},
                    {"pattern": {"kind": "Wildcard"},
                     "body": {"id": "n_4", "kind": "Literal", "type": "String", "value": "other"}}
                ]}"#,
        ));
        check_ok(&json);
    }

    #[test]
    fn test_error_match_arm_type_mismatch() {
        let json = module_json(&func_json(
            "main",
            "",
            "Unit",
            r#""IO""#,
            r#"{"id": "n_1", "kind": "Match", "type": "String",
                "scrutinee": {"id": "n_2", "kind": "Literal", "type": "I64", "value": 1},
                "arms": [
                    {"pattern": {"kind": "Literal", "value": 1},
                     "body": {"id": "n_3", "kind": "Literal", "type": "String", "value": "one"}},
                    {"pattern": {"kind": "Wildcard"},
                     "body": {"id": "n_4", "kind": "Literal", "type": "I64", "value": 0}}
                ]}"#,
        ));
        check_has_error(&json, "match arm type");
    }
}
