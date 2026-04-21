//! Semantic diff between two AIRL modules.
//!
//! Unlike a text diff, this operates on the IR graph structure:
//! - Added / removed / modified functions
//! - Changed effects
//! - Signature changes (parameters, return type)
//! - Body structural changes (node count delta)

use airl_ir::module::{FuncDef, Module};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// A semantic diff between two modules.
///
/// Produced by [`diff`]. Use [`ModuleDiff::is_empty`] to check for any changes
/// and [`ModuleDiff::summary`] for a short human-readable overview.
#[derive(Clone, Debug, Serialize)]
pub struct ModuleDiff {
    /// Names of functions present in `new` but not `old`.
    pub added_functions: Vec<String>,
    /// Names of functions present in `old` but not `new`.
    pub removed_functions: Vec<String>,
    /// Functions present in both with any change (signature, effects, body).
    pub modified_functions: Vec<FunctionDiff>,
    /// Imports added in `new` (formatted as `"module::item1,item2"`).
    pub added_imports: Vec<String>,
    /// Imports removed in `new` (formatted as `"module::item1,item2"`).
    pub removed_imports: Vec<String>,
}

/// A function-level diff showing what changed between two versions.
#[derive(Clone, Debug, Serialize)]
pub struct FunctionDiff {
    /// Function name (same in both versions).
    pub name: String,
    /// Whether parameters or return type changed.
    pub signature_changed: bool,
    /// Signature rendering from the old version, e.g. `"fn foo(x: I64) -> Unit"`.
    pub old_signature: String,
    /// Signature rendering from the new version.
    pub new_signature: String,
    /// Whether declared effects changed.
    pub effects_changed: bool,
    /// Effects declared in the old version, as strings.
    pub old_effects: Vec<String>,
    /// Effects declared in the new version, as strings.
    pub new_effects: Vec<String>,
    /// Difference in body node count: `new - old`. Negative values indicate shrinkage.
    pub body_node_count_delta: i64,
    /// Total body node count in the old version.
    pub old_node_count: u32,
    /// Total body node count in the new version.
    pub new_node_count: u32,
}

impl ModuleDiff {
    /// True if the two modules are semantically identical (no changes).
    pub fn is_empty(&self) -> bool {
        self.added_functions.is_empty()
            && self.removed_functions.is_empty()
            && self.modified_functions.is_empty()
            && self.added_imports.is_empty()
            && self.removed_imports.is_empty()
    }

    /// Short human-readable summary.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if !self.added_functions.is_empty() {
            parts.push(format!("+{} fn", self.added_functions.len()));
        }
        if !self.removed_functions.is_empty() {
            parts.push(format!("-{} fn", self.removed_functions.len()));
        }
        if !self.modified_functions.is_empty() {
            parts.push(format!("~{} fn", self.modified_functions.len()));
        }
        if !self.added_imports.is_empty() {
            parts.push(format!("+{} import", self.added_imports.len()));
        }
        if !self.removed_imports.is_empty() {
            parts.push(format!("-{} import", self.removed_imports.len()));
        }
        if parts.is_empty() {
            "no changes".to_string()
        } else {
            parts.join(", ")
        }
    }
}

/// Compute a semantic diff from `old` to `new`.
pub fn diff(old: &Module, new: &Module) -> ModuleDiff {
    let old_funcs: HashMap<&str, &FuncDef> = old
        .functions()
        .iter()
        .map(|f| (f.name.as_str(), f))
        .collect();
    let new_funcs: HashMap<&str, &FuncDef> = new
        .functions()
        .iter()
        .map(|f| (f.name.as_str(), f))
        .collect();

    let old_names: HashSet<&str> = old_funcs.keys().copied().collect();
    let new_names: HashSet<&str> = new_funcs.keys().copied().collect();

    let added_functions: Vec<String> = new_names
        .difference(&old_names)
        .map(|s| s.to_string())
        .collect();
    let removed_functions: Vec<String> = old_names
        .difference(&new_names)
        .map(|s| s.to_string())
        .collect();

    let mut modified_functions = Vec::new();
    for name in old_names.intersection(&new_names) {
        let old_f = old_funcs[name];
        let new_f = new_funcs[name];
        if let Some(fd) = diff_function(old_f, new_f) {
            modified_functions.push(fd);
        }
    }

    // Imports diff
    let old_imports: HashSet<String> = old
        .module
        .imports
        .iter()
        .map(|i| format!("{}::{}", i.module, i.items.join(",")))
        .collect();
    let new_imports: HashSet<String> = new
        .module
        .imports
        .iter()
        .map(|i| format!("{}::{}", i.module, i.items.join(",")))
        .collect();

    let added_imports: Vec<String> = new_imports.difference(&old_imports).cloned().collect();
    let removed_imports: Vec<String> = old_imports.difference(&new_imports).cloned().collect();

    // Sort for deterministic output
    let mut added_functions = added_functions;
    added_functions.sort();
    let mut removed_functions = removed_functions;
    removed_functions.sort();
    modified_functions.sort_by(|a, b| a.name.cmp(&b.name));
    let mut added_imports = added_imports;
    added_imports.sort();
    let mut removed_imports = removed_imports;
    removed_imports.sort();

    ModuleDiff {
        added_functions,
        removed_functions,
        modified_functions,
        added_imports,
        removed_imports,
    }
}

fn diff_function(old: &FuncDef, new: &FuncDef) -> Option<FunctionDiff> {
    let old_sig = function_signature(old);
    let new_sig = function_signature(new);
    let signature_changed = old_sig != new_sig;

    let old_effects: Vec<String> = old.effects.iter().map(|e| e.to_effect_str()).collect();
    let new_effects: Vec<String> = new.effects.iter().map(|e| e.to_effect_str()).collect();
    let effects_changed = old_effects != new_effects;

    let old_nodes = count_nodes(&old.body);
    let new_nodes = count_nodes(&new.body);
    let body_node_count_delta = new_nodes as i64 - old_nodes as i64;

    let body_structurally_same = old.body == new.body;

    if !signature_changed && !effects_changed && body_structurally_same {
        return None;
    }

    Some(FunctionDiff {
        name: old.name.clone(),
        signature_changed,
        old_signature: old_sig,
        new_signature: new_sig,
        effects_changed,
        old_effects,
        new_effects,
        body_node_count_delta,
        old_node_count: old_nodes,
        new_node_count: new_nodes,
    })
}

fn function_signature(func: &FuncDef) -> String {
    let params: Vec<String> = func
        .params
        .iter()
        .map(|p| format!("{}: {}", p.name, p.param_type.to_type_str()))
        .collect();
    format!(
        "fn {}({}) -> {}",
        func.name,
        params.join(", "),
        func.returns.to_type_str()
    )
}

fn count_nodes(node: &airl_ir::node::Node) -> u32 {
    use airl_ir::node::Node;
    1 + match node {
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
        } => statements.iter().map(count_nodes).sum::<u32>() + count_nodes(result),
        Node::Loop { body, .. } => count_nodes(body),
        Node::Match {
            scrutinee, arms, ..
        } => count_nodes(scrutinee) + arms.iter().map(|a| count_nodes(&a.body)).sum::<u32>(),
        Node::ArrayLiteral { elements, .. } => elements.iter().map(count_nodes).sum(),
        Node::IndexAccess { array, index, .. } => count_nodes(array) + count_nodes(index),
        Node::StructLiteral { fields, .. } => fields.iter().map(|(_, n)| count_nodes(n)).sum(),
        Node::FieldAccess { object, .. } => count_nodes(object),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(json: &str) -> Module {
        serde_json::from_str(json).unwrap()
    }

    fn hello_v1() -> Module {
        load(
            r#"{
            "format_version":"0.1.0",
            "module":{"id":"m","name":"main",
                "metadata":{"version":"1","description":"","author":"","created_at":""},
                "imports":[],"exports":[],"types":[],
                "functions":[{
                    "id":"f","name":"main","params":[],"returns":"Unit","effects":["IO"],
                    "body":{"id":"n1","kind":"Call","type":"Unit","target":"std::io::println",
                        "args":[{"id":"n2","kind":"Literal","type":"String","value":"hello"}]}
                }]
            }
        }"#,
        )
    }

    fn hello_v2_changed_body() -> Module {
        load(
            r#"{
            "format_version":"0.1.0",
            "module":{"id":"m","name":"main",
                "metadata":{"version":"1","description":"","author":"","created_at":""},
                "imports":[],"exports":[],"types":[],
                "functions":[{
                    "id":"f","name":"main","params":[],"returns":"Unit","effects":["IO"],
                    "body":{"id":"n1","kind":"Call","type":"Unit","target":"std::io::println",
                        "args":[{"id":"n2","kind":"Literal","type":"String","value":"world"}]}
                }]
            }
        }"#,
        )
    }

    #[test]
    fn test_diff_empty_when_identical() {
        let m = hello_v1();
        let d = diff(&m, &m);
        assert!(d.is_empty());
        assert_eq!(d.summary(), "no changes");
    }

    #[test]
    fn test_diff_modified_body() {
        let v1 = hello_v1();
        let v2 = hello_v2_changed_body();
        let d = diff(&v1, &v2);
        assert!(!d.is_empty());
        assert_eq!(d.modified_functions.len(), 1);
        assert_eq!(d.modified_functions[0].name, "main");
        // Signature unchanged
        assert!(!d.modified_functions[0].signature_changed);
    }

    #[test]
    fn test_diff_added_function() {
        let v1 = hello_v1();
        let v2 = load(
            r#"{
            "format_version":"0.1.0",
            "module":{"id":"m","name":"main",
                "metadata":{"version":"1","description":"","author":"","created_at":""},
                "imports":[],"exports":[],"types":[],
                "functions":[
                    {"id":"f","name":"main","params":[],"returns":"Unit","effects":["IO"],
                        "body":{"id":"n1","kind":"Literal","type":"Unit","value":null}},
                    {"id":"g","name":"helper","params":[],"returns":"I64","effects":["Pure"],
                        "body":{"id":"n2","kind":"Literal","type":"I64","value":42}}
                ]
            }
        }"#,
        );
        let d = diff(&v1, &v2);
        assert_eq!(d.added_functions, vec!["helper".to_string()]);
        assert!(d.removed_functions.is_empty());
    }

    #[test]
    fn test_diff_summary() {
        let v1 = hello_v1();
        let v2 = hello_v2_changed_body();
        let d = diff(&v1, &v2);
        assert_eq!(d.summary(), "~1 fn");
    }
}
