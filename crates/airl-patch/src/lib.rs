//! AIRL Patch Engine - Semantic patch operations on IR graphs.
//!
//! Provides the core editing interface for AI agents: instead of rewriting
//! entire modules, agents produce small semantic patches that are validated,
//! applied, and can be inverted.
//!
//! Key property: `apply(inverse(p), apply(p, module)) == module`

pub mod apply;
pub mod impact;
pub mod inverse;
pub mod ops;
pub mod traverse;
pub mod validate;

use thiserror::Error;

// Re-export key types
pub use apply::{apply_patch, PatchResult};
pub use impact::Impact;
pub use inverse::invert_patch;
pub use ops::{Patch, PatchOp};
pub use validate::validate_patch;

/// Errors that can occur during patch operations.
#[derive(Debug, Error)]
pub enum PatchError {
    #[error("node not found: {node_id}")]
    NodeNotFound { node_id: String },

    #[error("function not found: {func_id}")]
    FunctionNotFound { func_id: String },

    #[error("duplicate function name: {name}")]
    DuplicateFunction { name: String },

    #[error("validation failed at operation {op_index}: {message}")]
    ValidationFailed { op_index: usize, message: String },

    #[error("type check failed after patch: {message}")]
    TypeCheckFailed { message: String },

    #[error("version mismatch: expected {expected}, got {actual}")]
    VersionMismatch { expected: String, actual: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use airl_ir::effects::Effect;
    use airl_ir::ids::{FuncId, NodeId};
    use airl_ir::module::{FuncDef, Import, Module};
    use airl_ir::node::{LiteralValue, Node};
    use airl_ir::types::Type;

    fn load_module(json: &str) -> Module {
        serde_json::from_str(json).unwrap()
    }

    fn hello_module() -> Module {
        load_module(
            r#"{
            "format_version": "0.1.0",
            "module": {
                "id": "mod_main", "name": "main",
                "metadata": {"version": "1.0.0", "description": "", "author": "", "created_at": ""},
                "imports": [{"module": "std::io", "items": ["println"]}],
                "exports": [],
                "types": [],
                "functions": [{
                    "id": "f_main", "name": "main", "params": [], "returns": "Unit",
                    "effects": ["IO"],
                    "body": {"id": "n_1", "kind": "Call", "type": "Unit",
                        "target": "std::io::println",
                        "args": [{"id": "n_2", "kind": "Literal", "type": "String", "value": "hello world"}]
                    }
                }]
            }
        }"#,
        )
    }

    fn make_patch(ops: Vec<PatchOp>) -> Patch {
        Patch {
            id: "test_patch".to_string(),
            parent_version: String::new(),
            operations: ops,
            rationale: "test".to_string(),
            author: "test-agent".to_string(),
        }
    }

    // ---- ReplaceNode tests ----

    #[test]
    fn test_replace_leaf_node() {
        let module = hello_module();
        let patch = make_patch(vec![PatchOp::ReplaceNode {
            target: NodeId::new("n_2"),
            replacement: Node::Literal {
                id: NodeId::new("n_2"),
                node_type: Type::String,
                value: LiteralValue::Str("goodbye world".to_string()),
            },
        }]);

        let result = apply_patch(&module, &patch).unwrap();

        // Verify the replacement took effect
        let func = result.new_module.find_function("main").unwrap();
        match &func.body {
            Node::Call { args, .. } => match &args[0] {
                Node::Literal { value, .. } => {
                    assert_eq!(*value, LiteralValue::Str("goodbye world".to_string()));
                }
                other => panic!("Expected Literal, got: {other:?}"),
            },
            other => panic!("Expected Call, got: {other:?}"),
        }
    }

    #[test]
    fn test_replace_node_runs_correctly() {
        let module = hello_module();
        let patch = make_patch(vec![PatchOp::ReplaceNode {
            target: NodeId::new("n_2"),
            replacement: Node::Literal {
                id: NodeId::new("n_2"),
                node_type: Type::String,
                value: LiteralValue::Str("patched!".to_string()),
            },
        }]);

        let result = apply_patch(&module, &patch).unwrap();

        // Run the patched module through the interpreter
        let output = airl_interp::interpret(&result.new_module).unwrap();
        assert_eq!(output.stdout, "patched!\n");
    }

    #[test]
    fn test_replace_nonexistent_node_fails() {
        let module = hello_module();
        let patch = make_patch(vec![PatchOp::ReplaceNode {
            target: NodeId::new("n_999"),
            replacement: Node::Literal {
                id: NodeId::new("n_999"),
                node_type: Type::Unit,
                value: LiteralValue::Unit,
            },
        }]);

        let result = apply_patch(&module, &patch);
        assert!(result.is_err());
    }

    // ---- AddFunction / RemoveFunction tests ----

    #[test]
    fn test_add_function() {
        let module = hello_module();
        let new_func = FuncDef {
            id: FuncId::new("f_greet"),
            name: "greet".to_string(),
            params: vec![],
            returns: Type::String,
            effects: vec![Effect::Pure],
            body: Node::Literal {
                id: NodeId::new("g_1"),
                node_type: Type::String,
                value: LiteralValue::Str("hi".to_string()),
            },
        };

        let patch = make_patch(vec![PatchOp::AddFunction { func: new_func }]);
        let result = apply_patch(&module, &patch).unwrap();

        assert_eq!(result.new_module.functions().len(), 2);
        assert!(result.new_module.find_function("greet").is_some());
    }

    #[test]
    fn test_remove_function() {
        let module = hello_module();
        let patch = make_patch(vec![PatchOp::RemoveFunction {
            func_id: FuncId::new("f_main"),
        }]);

        let result = apply_patch(&module, &patch).unwrap();
        assert_eq!(result.new_module.functions().len(), 0);
    }

    #[test]
    fn test_add_duplicate_function_fails() {
        let module = hello_module();
        let dup_func = FuncDef {
            id: FuncId::new("f_main2"),
            name: "main".to_string(), // same name!
            params: vec![],
            returns: Type::Unit,
            effects: vec![],
            body: Node::Literal {
                id: NodeId::new("d_1"),
                node_type: Type::Unit,
                value: LiteralValue::Unit,
            },
        };

        let patch = make_patch(vec![PatchOp::AddFunction { func: dup_func }]);
        let result = apply_patch(&module, &patch);
        assert!(result.is_err());
    }

    // ---- RenameSymbol tests ----

    #[test]
    fn test_rename_call_target() {
        let module = hello_module();
        let patch = make_patch(vec![PatchOp::RenameSymbol {
            old_name: "std::io::println".to_string(),
            new_name: "std::io::print".to_string(),
            scope: None,
        }]);

        let result = apply_patch(&module, &patch).unwrap();
        let func = result.new_module.find_function("main").unwrap();
        match &func.body {
            Node::Call { target, .. } => {
                assert_eq!(target, "std::io::print");
            }
            other => panic!("Expected Call, got: {other:?}"),
        }
    }

    // ---- AddEffect / RemoveEffect tests ----

    #[test]
    fn test_add_effect() {
        let module = hello_module();
        let patch = make_patch(vec![PatchOp::AddEffect {
            func_id: FuncId::new("f_main"),
            effect: Effect::Fail {
                error_type: "IOError".to_string(),
            },
        }]);

        let result = apply_patch(&module, &patch).unwrap();
        let func = result.new_module.find_function("main").unwrap();
        assert!(func.effects.contains(&Effect::Fail {
            error_type: "IOError".to_string()
        }));
        assert!(func.effects.contains(&Effect::IO));
    }

    #[test]
    fn test_remove_effect() {
        let module = hello_module();
        let patch = make_patch(vec![PatchOp::RemoveEffect {
            func_id: FuncId::new("f_main"),
            effect: Effect::IO,
        }]);

        let result = apply_patch(&module, &patch).unwrap();
        let func = result.new_module.find_function("main").unwrap();
        assert!(!func.effects.contains(&Effect::IO));
    }

    // ---- AddImport / RemoveImport tests ----

    #[test]
    fn test_add_import() {
        let module = hello_module();
        let patch = make_patch(vec![PatchOp::AddImport {
            import: Import {
                module: "std::math".to_string(),
                items: vec!["abs".to_string()],
            },
        }]);

        let result = apply_patch(&module, &patch).unwrap();
        assert_eq!(result.new_module.module.imports.len(), 2);
    }

    #[test]
    fn test_remove_import() {
        let module = hello_module();
        let patch = make_patch(vec![PatchOp::RemoveImport {
            import: Import {
                module: "std::io".to_string(),
                items: vec!["println".to_string()],
            },
        }]);

        let result = apply_patch(&module, &patch).unwrap();
        assert_eq!(result.new_module.module.imports.len(), 0);
    }

    // ---- Patch inversion tests ----

    #[test]
    fn test_inversion_replace_node() {
        let module = hello_module();
        let patch = make_patch(vec![PatchOp::ReplaceNode {
            target: NodeId::new("n_2"),
            replacement: Node::Literal {
                id: NodeId::new("n_2"),
                node_type: Type::String,
                value: LiteralValue::Str("changed".to_string()),
            },
        }]);

        // Generate inverse BEFORE applying
        let inverse = invert_patch(&module, &patch).unwrap();

        // Apply forward patch
        let patched = apply_patch(&module, &patch).unwrap();

        // Apply inverse patch
        let restored = apply_patch(&patched.new_module, &inverse).unwrap();

        // Original and restored should be equal
        assert_eq!(
            serde_json::to_string(&module).unwrap(),
            serde_json::to_string(&restored.new_module).unwrap(),
        );
    }

    #[test]
    fn test_inversion_add_remove_function() {
        let module = hello_module();
        let new_func = FuncDef {
            id: FuncId::new("f_helper"),
            name: "helper".to_string(),
            params: vec![],
            returns: Type::Unit,
            effects: vec![Effect::Pure],
            body: Node::Literal {
                id: NodeId::new("h_1"),
                node_type: Type::Unit,
                value: LiteralValue::Unit,
            },
        };

        let patch = make_patch(vec![PatchOp::AddFunction {
            func: new_func.clone(),
        }]);

        let inverse = invert_patch(&module, &patch).unwrap();
        let patched = apply_patch(&module, &patch).unwrap();
        assert_eq!(patched.new_module.functions().len(), 2);

        let restored = apply_patch(&patched.new_module, &inverse).unwrap();
        assert_eq!(restored.new_module.functions().len(), 1);
    }

    #[test]
    fn test_inversion_rename_symbol() {
        let module = hello_module();
        let patch = make_patch(vec![PatchOp::RenameSymbol {
            old_name: "std::io::println".to_string(),
            new_name: "std::io::print".to_string(),
            scope: None,
        }]);

        let inverse = invert_patch(&module, &patch).unwrap();
        let patched = apply_patch(&module, &patch).unwrap();
        let restored = apply_patch(&patched.new_module, &inverse).unwrap();

        assert_eq!(
            serde_json::to_string(&module).unwrap(),
            serde_json::to_string(&restored.new_module).unwrap(),
        );
    }

    // ---- Impact analysis tests ----

    #[test]
    fn test_impact_replace_node() {
        let module = hello_module();
        let patch = make_patch(vec![PatchOp::ReplaceNode {
            target: NodeId::new("n_2"),
            replacement: Node::Literal {
                id: NodeId::new("n_2"),
                node_type: Type::String,
                value: LiteralValue::Str("x".to_string()),
            },
        }]);

        let result = apply_patch(&module, &patch).unwrap();
        assert!(result
            .impact
            .affected_functions
            .contains(&FuncId::new("f_main")));
    }

    // ---- Version tracking tests ----

    #[test]
    fn test_version_changes_after_patch() {
        let module = hello_module();
        let v1 = airl_ir::version::VersionId::compute(&module).to_hex();

        let patch = make_patch(vec![PatchOp::ReplaceNode {
            target: NodeId::new("n_2"),
            replacement: Node::Literal {
                id: NodeId::new("n_2"),
                node_type: Type::String,
                value: LiteralValue::Str("different".to_string()),
            },
        }]);

        let result = apply_patch(&module, &patch).unwrap();
        assert_ne!(v1, result.new_version);
    }

    // ---- Multi-op patch tests ----

    #[test]
    fn test_multiple_ops_in_one_patch() {
        let module = hello_module();
        let patch = make_patch(vec![
            // Change the message
            PatchOp::ReplaceNode {
                target: NodeId::new("n_2"),
                replacement: Node::Literal {
                    id: NodeId::new("n_2"),
                    node_type: Type::String,
                    value: LiteralValue::Str("multi-patched".to_string()),
                },
            },
            // Add Fail effect
            PatchOp::AddEffect {
                func_id: FuncId::new("f_main"),
                effect: Effect::Fail {
                    error_type: "E".to_string(),
                },
            },
        ]);

        let result = apply_patch(&module, &patch).unwrap();
        let func = result.new_module.find_function("main").unwrap();
        assert!(func.has_effect(&Effect::Fail {
            error_type: "E".to_string()
        }));

        let output = airl_interp::interpret(&result.new_module).unwrap();
        assert_eq!(output.stdout, "multi-patched\n");
    }

    // ---- Traverse utility tests ----

    #[test]
    fn test_collect_node_ids() {
        let module = hello_module();
        let func = module.find_function("main").unwrap();
        let ids = traverse::collect_node_ids(&func.body);
        assert!(ids.contains(&NodeId::new("n_1")));
        assert!(ids.contains(&NodeId::new("n_2")));
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn test_find_node() {
        let module = hello_module();
        let func = module.find_function("main").unwrap();
        let node = traverse::find_node(&func.body, &NodeId::new("n_2"));
        assert!(node.is_some());
        match node.unwrap() {
            Node::Literal { value, .. } => {
                assert_eq!(*value, LiteralValue::Str("hello world".to_string()));
            }
            other => panic!("Expected Literal, got: {other:?}"),
        }
    }
}
