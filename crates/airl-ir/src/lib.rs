//! AIRL IR - Core intermediate representation data structures for the AIRL project.
//!
//! This crate defines the typed IR used throughout the AIRL system. The IR is
//! designed to be serialized to/from JSON, making it easy for AI agents to
//! generate and manipulate.

pub mod display;
pub mod effects;
pub mod graph;
pub mod ids;
pub mod module;
pub mod node;
pub mod symbol;
pub mod types;
pub mod version;

// Re-export key types for convenience.
pub use effects::Effect;
pub use graph::{IRGraph, IRGraphError};
pub use ids::{FuncId, ModuleId, NodeId, Symbol, TypeId};
pub use module::{Export, FuncDef, Import, Module, ModuleInner, ModuleMetadata, ParamDef, TypeDef};
pub use node::{BinOpKind, LiteralValue, MatchArm, Node, Pattern, UnaryOpKind};
pub use types::{Type, Variant};
pub use version::VersionId;
