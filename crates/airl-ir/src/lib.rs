//! AIRL IR - Core intermediate representation data structures.
//!
//! This crate defines the typed IR used throughout the AIRL system. The IR is
//! designed to be serialized to/from JSON, making it easy for AI agents to
//! generate and manipulate.
//!
//! # Example
//!
//! ```
//! use airl_ir::Module;
//!
//! let json = r#"{
//!     "format_version":"0.1.0",
//!     "module":{"id":"m","name":"main",
//!         "metadata":{"version":"1","description":"","author":"","created_at":""},
//!         "imports":[],"exports":[],"types":[],
//!         "functions":[]}
//! }"#;
//! let module: Module = serde_json::from_str(json).unwrap();
//! assert_eq!(module.name(), "main");
//! ```
//!
//! # Module organization
//!
//! - [`node`] — expression and statement nodes (16 variants)
//! - [`types`] — the type system
//! - [`effects`] — the effect system
//! - [`module`] — top-level module, function, and parameter definitions
//! - [`ids`] — strongly-typed identifiers
//! - [`version`] — content-addressable module versions
//! - [`graph`] — high-level graph container with validation
//! - [`display`] — pretty-printing for debugging
//! - [`symbol`] — symbol interning helpers

#![deny(missing_docs)]

/// Pretty-printers and `Display` impls for IR nodes.
pub mod display;
/// Effect system: `Pure`, `IO`, `Fail`, `Read`, `Write`, `Allocate`, `Diverge`.
pub mod effects;
/// High-level IR graph container with JSON (de)serialization.
pub mod graph;
/// Strongly-typed identifiers: [`NodeId`], [`FuncId`], [`ModuleId`], [`TypeId`], [`Symbol`].
pub mod ids;
/// Top-level module structure: [`Module`], [`FuncDef`], [`ParamDef`], imports, exports.
pub mod module;
/// Core IR node types (expressions, statements, control flow, patterns).
pub mod node;
/// Lightweight symbol (string) wrapper used for names and identifiers.
pub mod symbol;
/// The AIRL type system: primitives, arrays, tuples, structs, enums, generics.
pub mod types;
/// Content-addressable versioning for modules (SHA-256 based).
pub mod version;

// Re-export key types for convenience.
pub use effects::Effect;
pub use graph::{IRGraph, IRGraphError};
pub use ids::{FuncId, ModuleId, NodeId, Symbol, TypeId};
pub use module::{Export, FuncDef, Import, Module, ModuleInner, ModuleMetadata, ParamDef, TypeDef};
pub use node::{BinOpKind, LiteralValue, MatchArm, Node, Pattern, UnaryOpKind};
pub use types::{Type, Variant};
pub use version::VersionId;
