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
        /// The ID of the node to replace.
        target: NodeId,
        /// The new subtree to splice in.
        replacement: Node,
    },

    /// Add a new function to the module.
    AddFunction {
        /// The new function definition.
        func: FuncDef,
    },

    /// Remove a function by its FuncId.
    RemoveFunction {
        /// The ID of the function to remove.
        func_id: FuncId,
    },

    /// Add an import to the module.
    AddImport {
        /// The import to add.
        import: Import,
    },

    /// Remove an import from the module.
    RemoveImport {
        /// The import to remove.
        import: Import,
    },

    /// Rename a symbol (variable, function name, call target) throughout the module
    /// or within a specific function scope.
    RenameSymbol {
        /// The existing name to replace.
        old_name: String,
        /// The new name.
        new_name: String,
        /// If Some, only rename within this function. If None, rename globally.
        scope: Option<FuncId>,
    },

    /// Add an effect to a function's declared effect list.
    AddEffect {
        /// The function to modify.
        func_id: FuncId,
        /// The effect to add.
        effect: Effect,
    },

    /// Remove an effect from a function's declared effect list.
    RemoveEffect {
        /// The function to modify.
        func_id: FuncId,
        /// The effect to remove.
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
