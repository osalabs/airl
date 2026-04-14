//! Patch application: transform a Module by applying a Patch.

use airl_ir::module::{FuncDef, Module};
use airl_ir::version::VersionId;

use crate::impact::{self, Impact};
use crate::ops::{Patch, PatchOp};
use crate::traverse;
use crate::validate;
use crate::PatchError;

/// Result of applying a patch.
#[derive(Clone, Debug)]
pub struct PatchResult {
    /// The new module after patch application.
    pub new_module: Module,
    /// Content hash of the new module state.
    pub new_version: String,
    /// Impact analysis: which functions/types were affected.
    pub impact: Impact,
}

/// Apply a patch to a module, producing a new module.
///
/// Validates the patch first, then applies operations sequentially.
/// Returns the new module, its version hash, and impact analysis.
pub fn apply_patch(module: &Module, patch: &Patch) -> Result<PatchResult, PatchError> {
    // Validate before applying
    validate::validate_patch(module, patch)?;

    // Compute impact before mutation
    let impact_result = impact::analyze_impact(module, &patch.operations);

    // Clone module and apply operations sequentially
    let mut new_module = module.clone();

    for op in &patch.operations {
        apply_op(&mut new_module, op)?;
    }

    // Compute new version hash
    let version = VersionId::compute(&new_module);
    let version_hex = version.to_hex();

    Ok(PatchResult {
        new_module,
        new_version: version_hex,
        impact: impact_result,
    })
}

/// Apply a single patch operation to a module (mutating it in place).
fn apply_op(module: &mut Module, op: &PatchOp) -> Result<(), PatchError> {
    match op {
        PatchOp::ReplaceNode {
            target,
            replacement,
        } => {
            // Find which function contains this node and replace it
            let mut found = false;
            let new_functions: Vec<FuncDef> = module
                .module
                .functions
                .iter()
                .map(|func| {
                    if let Some(new_body) =
                        traverse::replace_node_in_tree(&func.body, target, replacement)
                    {
                        found = true;
                        FuncDef {
                            body: new_body,
                            ..func.clone()
                        }
                    } else {
                        func.clone()
                    }
                })
                .collect();

            if !found {
                return Err(PatchError::NodeNotFound {
                    node_id: target.to_string(),
                });
            }
            module.module.functions = new_functions;
            Ok(())
        }

        PatchOp::AddFunction { func } => {
            module.module.functions.push(func.clone());
            Ok(())
        }

        PatchOp::RemoveFunction { func_id } => {
            module
                .module
                .functions
                .retain(|f| &f.id != func_id);
            Ok(())
        }

        PatchOp::AddImport { import } => {
            module.module.imports.push(import.clone());
            Ok(())
        }

        PatchOp::RemoveImport { import } => {
            module
                .module
                .imports
                .retain(|i| i.module != import.module || i.items != import.items);
            Ok(())
        }

        PatchOp::RenameSymbol {
            old_name,
            new_name,
            scope,
        } => {
            let new_functions: Vec<FuncDef> = module
                .module
                .functions
                .iter()
                .map(|func| {
                    let in_scope = scope.as_ref().is_none_or(|s| &func.id == s);
                    if in_scope {
                        let new_body =
                            traverse::rename_in_tree(&func.body, old_name, new_name);
                        let new_name_field = if func.name == *old_name {
                            new_name.clone()
                        } else {
                            func.name.clone()
                        };
                        FuncDef {
                            name: new_name_field,
                            body: new_body,
                            ..func.clone()
                        }
                    } else {
                        func.clone()
                    }
                })
                .collect();
            module.module.functions = new_functions;
            Ok(())
        }

        PatchOp::AddEffect { func_id, effect } => {
            for func in &mut module.module.functions {
                if &func.id == func_id {
                    if !func.effects.contains(effect) {
                        func.effects.push(effect.clone());
                    }
                    return Ok(());
                }
            }
            Err(PatchError::FunctionNotFound {
                func_id: func_id.to_string(),
            })
        }

        PatchOp::RemoveEffect { func_id, effect } => {
            for func in &mut module.module.functions {
                if &func.id == func_id {
                    func.effects.retain(|e| e != effect);
                    return Ok(());
                }
            }
            Err(PatchError::FunctionNotFound {
                func_id: func_id.to_string(),
            })
        }
    }
}
