//! Impact analysis: determine which parts of a module are affected by a patch.

use airl_ir::ids::{FuncId, TypeId};
use airl_ir::module::Module;
use serde::{Deserialize, Serialize};

use crate::ops::PatchOp;
use crate::traverse;

/// The impact of a patch on a module.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Impact {
    pub affected_functions: Vec<FuncId>,
    pub affected_types: Vec<TypeId>,
}

/// Analyze the impact of a set of patch operations on a module.
pub fn analyze_impact(module: &Module, ops: &[PatchOp]) -> Impact {
    let mut affected_functions = Vec::new();
    let affected_types = Vec::new();

    for op in ops {
        match op {
            PatchOp::ReplaceNode { target, .. } => {
                let funcs = traverse::functions_containing_node(module, target);
                for f in funcs {
                    if !affected_functions.contains(&f) {
                        affected_functions.push(f);
                    }
                }
            }
            PatchOp::AddFunction { func } => {
                if !affected_functions.contains(&func.id) {
                    affected_functions.push(func.id.clone());
                }
            }
            PatchOp::RemoveFunction { func_id } => {
                if !affected_functions.contains(func_id) {
                    affected_functions.push(func_id.clone());
                }
            }
            PatchOp::RenameSymbol { scope, .. } => {
                if let Some(func_id) = scope {
                    if !affected_functions.contains(func_id) {
                        affected_functions.push(func_id.clone());
                    }
                } else {
                    // Global rename affects all functions
                    for func in module.functions() {
                        if !affected_functions.contains(&func.id) {
                            affected_functions.push(func.id.clone());
                        }
                    }
                }
            }
            PatchOp::AddEffect { func_id, .. } | PatchOp::RemoveEffect { func_id, .. } => {
                if !affected_functions.contains(func_id) {
                    affected_functions.push(func_id.clone());
                }
            }
            PatchOp::AddImport { .. } | PatchOp::RemoveImport { .. } => {
                // Imports don't directly affect functions or types,
                // but could be tracked as module-level changes
            }
        }
    }

    Impact {
        affected_functions,
        affected_types,
    }
}
