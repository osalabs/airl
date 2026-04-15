//! Patch inversion: generate an undo patch for any applied patch.
//!
//! Key property: apply(inverse(p), apply(p, module)) == module

use airl_ir::module::Module;

use crate::ops::{Patch, PatchOp};
use crate::traverse;
use crate::PatchError;

/// Generate an inverse patch that undoes the given patch.
///
/// Must be called BEFORE the patch is applied, so we can capture the
/// original state of modified nodes/functions.
pub fn invert_patch(module: &Module, patch: &Patch) -> Result<Patch, PatchError> {
    let mut inverse_ops = Vec::new();

    for op in &patch.operations {
        let inv = invert_op(module, op)?;
        inverse_ops.push(inv);
    }

    // Reverse the order: if patch applies [A, B, C], inverse applies [C⁻¹, B⁻¹, A⁻¹]
    inverse_ops.reverse();

    Ok(Patch {
        id: format!("inv_{}", patch.id),
        parent_version: String::new(), // Will be the version after applying the original patch
        operations: inverse_ops,
        rationale: format!("Inverse of: {}", patch.rationale),
        author: patch.author.clone(),
    })
}

fn invert_op(module: &Module, op: &PatchOp) -> Result<PatchOp, PatchError> {
    match op {
        PatchOp::ReplaceNode { target, .. } => {
            // Capture the original node before replacement
            let func = traverse::find_containing_function(module, target).ok_or(
                PatchError::NodeNotFound {
                    node_id: target.to_string(),
                },
            )?;
            let original_node =
                traverse::find_node(&func.body, target).ok_or(PatchError::NodeNotFound {
                    node_id: target.to_string(),
                })?;

            Ok(PatchOp::ReplaceNode {
                // The inverse replaces the NEW node (which will have replacement's root ID)
                // back with the original node. We use the original target ID since after
                // applying the forward patch, the replacement's root sits where target was.
                target: target.clone(),
                replacement: original_node.clone(),
            })
        }

        PatchOp::AddFunction { func } => Ok(PatchOp::RemoveFunction {
            func_id: func.id.clone(),
        }),

        PatchOp::RemoveFunction { func_id } => {
            // Capture the function being removed
            let func = module
                .find_function_by_id(func_id)
                .ok_or(PatchError::FunctionNotFound {
                    func_id: func_id.to_string(),
                })?;
            Ok(PatchOp::AddFunction { func: func.clone() })
        }

        PatchOp::AddImport { import } => Ok(PatchOp::RemoveImport {
            import: import.clone(),
        }),

        PatchOp::RemoveImport { import } => Ok(PatchOp::AddImport {
            import: import.clone(),
        }),

        PatchOp::RenameSymbol {
            old_name,
            new_name,
            scope,
        } => Ok(PatchOp::RenameSymbol {
            old_name: new_name.clone(),
            new_name: old_name.clone(),
            scope: scope.clone(),
        }),

        PatchOp::AddEffect { func_id, effect } => Ok(PatchOp::RemoveEffect {
            func_id: func_id.clone(),
            effect: effect.clone(),
        }),

        PatchOp::RemoveEffect { func_id, effect } => Ok(PatchOp::AddEffect {
            func_id: func_id.clone(),
            effect: effect.clone(),
        }),
    }
}
