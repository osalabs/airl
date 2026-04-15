//! Patch validation: verify a patch can be safely applied.

use airl_ir::module::Module;

use crate::ops::{Patch, PatchOp};
use crate::traverse;
use crate::PatchError;

/// Validate that a patch can be applied to the given module.
/// Returns Ok(()) if valid, Err with details if not.
pub fn validate_patch(module: &Module, patch: &Patch) -> Result<(), PatchError> {
    for (i, op) in patch.operations.iter().enumerate() {
        validate_op(module, op).map_err(|e| PatchError::ValidationFailed {
            op_index: i,
            message: e.to_string(),
        })?;
    }
    Ok(())
}

fn validate_op(module: &Module, op: &PatchOp) -> Result<(), PatchError> {
    match op {
        PatchOp::ReplaceNode { target, .. } => {
            // Target node must exist in some function
            if traverse::find_containing_function(module, target).is_none() {
                return Err(PatchError::NodeNotFound {
                    node_id: target.to_string(),
                });
            }
            Ok(())
        }
        PatchOp::RemoveFunction { func_id } => {
            if module.find_function_by_id(func_id).is_none() {
                return Err(PatchError::FunctionNotFound {
                    func_id: func_id.to_string(),
                });
            }
            Ok(())
        }
        PatchOp::AddFunction { func } => {
            // Check that function name doesn't already exist
            if module.find_function(&func.name).is_some() {
                return Err(PatchError::DuplicateFunction {
                    name: func.name.clone(),
                });
            }
            Ok(())
        }
        PatchOp::AddEffect { func_id, .. } | PatchOp::RemoveEffect { func_id, .. } => {
            if module.find_function_by_id(func_id).is_none() {
                return Err(PatchError::FunctionNotFound {
                    func_id: func_id.to_string(),
                });
            }
            Ok(())
        }
        // RenameSymbol, AddImport, RemoveImport are always structurally valid
        PatchOp::RenameSymbol { .. } | PatchOp::AddImport { .. } | PatchOp::RemoveImport { .. } => {
            Ok(())
        }
    }
}
