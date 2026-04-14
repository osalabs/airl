//! Patch operation types and patch bundle structure.

use airl_ir::effects::Effect;
use airl_ir::ids::{FuncId, NodeId};
use airl_ir::module::{FuncDef, Import};
use airl_ir::node::Node;
use serde::{Deserialize, Serialize};

/// A single patch operation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum PatchOp {
    /// Replace a node (identified by NodeId) with a new node.
    /// The replacement's root may have a different NodeId.
    ReplaceNode {
        target: NodeId,
        replacement: Node,
    },

    /// Add a new function to the module.
    AddFunction {
        func: FuncDef,
    },

    /// Remove a function by its FuncId.
    RemoveFunction {
        func_id: FuncId,
    },

    /// Add an import to the module.
    AddImport {
        import: Import,
    },

    /// Remove an import from the module.
    RemoveImport {
        import: Import,
    },

    /// Rename a symbol (variable, function name, call target) throughout the module
    /// or within a specific function scope.
    RenameSymbol {
        old_name: String,
        new_name: String,
        /// If Some, only rename within this function. If None, rename globally.
        scope: Option<FuncId>,
    },

    /// Add an effect to a function's declared effect list.
    AddEffect {
        func_id: FuncId,
        effect: Effect,
    },

    /// Remove an effect from a function's declared effect list.
    RemoveEffect {
        func_id: FuncId,
        effect: Effect,
    },
}

/// A patch is an ordered list of operations with metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Patch {
    /// Unique patch identifier.
    pub id: String,
    /// The VersionId (hex) of the module state this patch applies to.
    pub parent_version: String,
    /// Ordered list of operations to apply.
    pub operations: Vec<PatchOp>,
    /// Human/agent-readable rationale for the change.
    pub rationale: String,
    /// Author identifier (agent ID or human).
    pub author: String,
}
