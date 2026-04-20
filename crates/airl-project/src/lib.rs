//! AIRL Project - Project state management, patch history, queries.
//!
//! Manages the lifecycle of an AIRL project: creating from IR,
//! applying patches with undo history, querying functions/types,
//! constraint checking, and text projections.

pub mod constraints;
pub mod diff;
pub mod projection;
pub mod queries;
pub mod workspace;

use airl_ir::module::Module;
use airl_ir::node::Node;
use airl_ir::version::VersionId;
use airl_patch::{self, Impact, Patch};
use airl_typecheck::{self, TypeCheckResult};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Project-level errors.
#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("no module loaded")]
    NoModule,
    #[error("patch error: {0}")]
    PatchError(#[from] airl_patch::PatchError),
    #[error("no patches to undo")]
    NothingToUndo,
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}

/// A history entry tracking an applied patch and its inverse.
#[derive(Clone, Debug)]
pub struct HistoryEntry {
    pub previous_version: String,
    pub patch: Patch,
    pub inverse: Patch,
}

/// Summary of a function in the module.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FuncSummary {
    pub id: String,
    pub name: String,
    pub params: Vec<ParamSummary>,
    pub returns: String,
    pub effects: Vec<String>,
    pub node_count: usize,
}

/// Summary of a function parameter.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParamSummary {
    pub name: String,
    pub param_type: String,
}

/// An edge in a call graph.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CallEdge {
    pub from: String,
    pub to: String,
}

/// Effect summary for a function.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EffectSummary {
    pub func_name: String,
    pub declared_effects: Vec<String>,
}

/// An AIRL project holding module state, history, and metadata.
pub struct Project {
    pub name: String,
    pub module: Module,
    pub version: String,
    pub history: Vec<HistoryEntry>,
}

impl Project {
    /// Create a new project from a module.
    pub fn new(name: impl Into<String>, module: Module) -> Self {
        let version = VersionId::compute(&module).to_hex();
        Self {
            name: name.into(),
            module,
            version,
            history: Vec::new(),
        }
    }

    /// Create a project from a JSON IR string.
    pub fn from_json(name: impl Into<String>, json: &str) -> Result<Self, ProjectError> {
        let module: Module = serde_json::from_str(json)?;
        Ok(Self::new(name, module))
    }

    /// Apply a patch to the project. Records history for undo.
    pub fn apply_patch(&mut self, patch: &Patch) -> Result<PatchApplyResult, ProjectError> {
        // Generate inverse BEFORE applying
        let inverse = airl_patch::invert_patch(&self.module, patch)?;
        let old_version = self.version.clone();

        // Apply
        let result = airl_patch::apply_patch(&self.module, patch)?;

        // Record history
        self.history.push(HistoryEntry {
            previous_version: old_version,
            patch: patch.clone(),
            inverse,
        });

        // Update project state
        self.module = result.new_module;
        self.version = result.new_version.clone();

        Ok(PatchApplyResult {
            new_version: result.new_version,
            impact: result.impact,
        })
    }

    /// Preview a patch without applying it.
    pub fn preview_patch(&self, patch: &Patch) -> Result<PatchPreviewResult, ProjectError> {
        // Validate
        let validation = airl_patch::validate_patch(&self.module, patch);
        let valid = validation.is_ok();
        let validation_error = validation.err().map(|e| e.to_string());

        // Try to apply to a clone to check types
        let mut type_errors = Vec::new();
        let mut impact = Impact::default();

        if valid {
            if let Ok(result) = airl_patch::apply_patch(&self.module, patch) {
                impact = result.impact;
                let tc = airl_typecheck::typecheck(&result.new_module);
                type_errors = tc.errors.iter().map(|e| e.message.clone()).collect();
            }
        }

        Ok(PatchPreviewResult {
            would_succeed: valid && type_errors.is_empty(),
            validation_error,
            type_errors,
            impact,
        })
    }

    /// Undo the last patch.
    pub fn undo_last(&mut self) -> Result<PatchApplyResult, ProjectError> {
        let entry = self.history.pop().ok_or(ProjectError::NothingToUndo)?;

        let result = airl_patch::apply_patch(&self.module, &entry.inverse)?;
        self.module = result.new_module;
        self.version = result.new_version.clone();

        Ok(PatchApplyResult {
            new_version: result.new_version,
            impact: result.impact,
        })
    }

    /// Run type checker on the current module.
    pub fn typecheck(&self) -> TypeCheckResult {
        airl_typecheck::typecheck(&self.module)
    }

    /// Check the module against a set of constraints.
    pub fn check_constraints(
        &self,
        constraints: &[constraints::Constraint],
    ) -> constraints::ConstraintReport {
        constraints::check_all(constraints, &self.module)
    }

    /// Find functions matching a name pattern (simple substring match).
    pub fn find_functions(&self, pattern: &str) -> Vec<FuncSummary> {
        self.module
            .functions()
            .iter()
            .filter(|f| pattern.is_empty() || f.name.contains(pattern))
            .map(|f| FuncSummary {
                id: f.id.to_string(),
                name: f.name.clone(),
                params: f
                    .params
                    .iter()
                    .map(|p| ParamSummary {
                        name: p.name.clone(),
                        param_type: p.param_type.to_type_str(),
                    })
                    .collect(),
                returns: f.returns.to_type_str(),
                effects: f.effects.iter().map(|e| e.to_effect_str()).collect(),
                node_count: count_nodes(&f.body),
            })
            .collect()
    }

    /// Get call graph edges for a function.
    pub fn get_call_graph(&self, func_name: &str) -> Vec<CallEdge> {
        let mut edges = Vec::new();
        if let Some(func) = self.module.find_function(func_name) {
            collect_calls(&func.body, &func.name, &mut edges);
        }
        edges
    }

    /// Get effect summary for a function.
    pub fn get_effect_summary(&self, func_name: &str) -> Option<EffectSummary> {
        self.module.find_function(func_name).map(|f| EffectSummary {
            func_name: f.name.clone(),
            declared_effects: f.effects.iter().map(|e| e.to_effect_str()).collect(),
        })
    }
}

/// Result of applying a patch.
#[derive(Clone, Debug, Serialize)]
pub struct PatchApplyResult {
    pub new_version: String,
    pub impact: Impact,
}

/// Result of previewing a patch.
#[derive(Clone, Debug, Serialize)]
pub struct PatchPreviewResult {
    pub would_succeed: bool,
    pub validation_error: Option<String>,
    pub type_errors: Vec<String>,
    pub impact: Impact,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn count_nodes(node: &Node) -> usize {
    1 + match node {
        Node::Literal { .. } | Node::Param { .. } | Node::Error { .. } => 0,
        Node::Let { value, body, .. } => count_nodes(value) + count_nodes(body),
        Node::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => count_nodes(cond) + count_nodes(then_branch) + count_nodes(else_branch),
        Node::Call { args, .. } => args.iter().map(count_nodes).sum(),
        Node::Return { value, .. } => count_nodes(value),
        Node::BinOp { lhs, rhs, .. } => count_nodes(lhs) + count_nodes(rhs),
        Node::UnaryOp { operand, .. } => count_nodes(operand),
        Node::Block {
            statements, result, ..
        } => statements.iter().map(count_nodes).sum::<usize>() + count_nodes(result),
        Node::Loop { body, .. } => count_nodes(body),
        Node::Match {
            scrutinee, arms, ..
        } => count_nodes(scrutinee) + arms.iter().map(|a| count_nodes(&a.body)).sum::<usize>(),
        Node::StructLiteral { fields, .. } => fields.iter().map(|(_, n)| count_nodes(n)).sum(),
        Node::FieldAccess { object, .. } => count_nodes(object),
        Node::ArrayLiteral { elements, .. } => elements.iter().map(count_nodes).sum(),
        Node::IndexAccess { array, index, .. } => count_nodes(array) + count_nodes(index),
    }
}

fn collect_calls(node: &Node, current_func: &str, edges: &mut Vec<CallEdge>) {
    match node {
        Node::Call { target, args, .. } => {
            edges.push(CallEdge {
                from: current_func.to_string(),
                to: target.clone(),
            });
            for arg in args {
                collect_calls(arg, current_func, edges);
            }
        }
        Node::Let { value, body, .. } => {
            collect_calls(value, current_func, edges);
            collect_calls(body, current_func, edges);
        }
        Node::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            collect_calls(cond, current_func, edges);
            collect_calls(then_branch, current_func, edges);
            collect_calls(else_branch, current_func, edges);
        }
        Node::BinOp { lhs, rhs, .. } => {
            collect_calls(lhs, current_func, edges);
            collect_calls(rhs, current_func, edges);
        }
        Node::UnaryOp { operand, .. } => collect_calls(operand, current_func, edges),
        Node::Return { value, .. } => collect_calls(value, current_func, edges),
        Node::Block {
            statements, result, ..
        } => {
            for s in statements {
                collect_calls(s, current_func, edges);
            }
            collect_calls(result, current_func, edges);
        }
        Node::Match {
            scrutinee, arms, ..
        } => {
            collect_calls(scrutinee, current_func, edges);
            for arm in arms {
                collect_calls(&arm.body, current_func, edges);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hello_json() -> &'static str {
        r#"{
            "format_version": "0.1.0",
            "module": {
                "id": "mod_main", "name": "main",
                "metadata": {"version": "1.0.0", "description": "", "author": "", "created_at": ""},
                "imports": [{"module": "std::io", "items": ["println"]}],
                "exports": [], "types": [],
                "functions": [{
                    "id": "f_main", "name": "main", "params": [], "returns": "Unit",
                    "effects": ["IO"],
                    "body": {"id": "n_1", "kind": "Call", "type": "Unit",
                        "target": "std::io::println",
                        "args": [{"id": "n_2", "kind": "Literal", "type": "String", "value": "hello"}]
                    }
                }]
            }
        }"#
    }

    #[test]
    fn test_create_project() {
        let project = Project::from_json("test", hello_json()).unwrap();
        assert_eq!(project.name, "test");
        assert!(!project.version.is_empty());
        assert_eq!(project.module.functions().len(), 1);
    }

    #[test]
    fn test_apply_and_undo_patch() {
        let mut project = Project::from_json("test", hello_json()).unwrap();
        let v1 = project.version.clone();

        let patch = Patch {
            id: "p1".to_string(),
            parent_version: v1.clone(),
            operations: vec![airl_patch::PatchOp::ReplaceNode {
                target: airl_ir::NodeId::new("n_2"),
                replacement: airl_ir::node::Node::Literal {
                    id: airl_ir::NodeId::new("n_2"),
                    node_type: airl_ir::types::Type::String,
                    value: airl_ir::node::LiteralValue::Str("changed".to_string()),
                },
            }],
            rationale: "test".to_string(),
            author: "agent".to_string(),
        };

        let result = project.apply_patch(&patch).unwrap();
        assert_ne!(result.new_version, v1);
        assert_eq!(project.history.len(), 1);

        // Undo
        project.undo_last().unwrap();
        assert_eq!(project.version, v1);
        assert_eq!(project.history.len(), 0);
    }

    #[test]
    fn test_preview_patch() {
        let project = Project::from_json("test", hello_json()).unwrap();
        let patch = Patch {
            id: "p1".to_string(),
            parent_version: String::new(),
            operations: vec![airl_patch::PatchOp::ReplaceNode {
                target: airl_ir::NodeId::new("n_2"),
                replacement: airl_ir::node::Node::Literal {
                    id: airl_ir::NodeId::new("n_2"),
                    node_type: airl_ir::types::Type::String,
                    value: airl_ir::node::LiteralValue::Str("preview".to_string()),
                },
            }],
            rationale: "test".to_string(),
            author: "agent".to_string(),
        };

        let preview = project.preview_patch(&patch).unwrap();
        assert!(preview.would_succeed);
    }

    #[test]
    fn test_find_functions() {
        let project = Project::from_json("test", hello_json()).unwrap();
        let funcs = project.find_functions("main");
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].name, "main");
    }

    #[test]
    fn test_get_call_graph() {
        let project = Project::from_json("test", hello_json()).unwrap();
        let edges = project.get_call_graph("main");
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].to, "std::io::println");
    }

    #[test]
    fn test_get_effect_summary() {
        let project = Project::from_json("test", hello_json()).unwrap();
        let summary = project.get_effect_summary("main").unwrap();
        assert!(summary.declared_effects.contains(&"IO".to_string()));
    }

    #[test]
    fn test_typecheck() {
        let project = Project::from_json("test", hello_json()).unwrap();
        let result = project.typecheck();
        assert!(result.is_ok());
    }

    #[test]
    fn test_undo_empty_fails() {
        let mut project = Project::from_json("test", hello_json()).unwrap();
        assert!(project.undo_last().is_err());
    }
}
