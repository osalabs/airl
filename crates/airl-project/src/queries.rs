//! Richer IR queries for agent navigation.
//!
//! Beyond simple name search, these queries answer structural questions
//! about the codebase:
//! - Dead code: functions not reachable from `main`
//! - Type usage: which functions use a given type
//! - Builtin usage: which standard library functions are called
//! - Effect surface: what effects does the module actually use

use airl_ir::module::Module;
use airl_ir::node::Node;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// A dead-code report: functions unreachable from an entry point.
#[derive(Clone, Debug, Serialize)]
pub struct DeadCodeReport {
    /// The function name used as the reachability root.
    pub entry_point: String,
    /// Functions reachable from `entry_point` via the static call graph, sorted.
    pub reachable: Vec<String>,
    /// Functions declared in the module but not reachable from `entry_point`, sorted.
    pub dead: Vec<String>,
}

/// Builtin usage statistics across a module.
#[derive(Clone, Debug, Serialize)]
pub struct BuiltinUsage {
    /// Map from builtin name to how many times it's called across the module.
    pub counts: HashMap<String, u32>,
    /// All builtins called, sorted alphabetically.
    pub unique_builtins: Vec<String>,
}

/// Effect surface: the set of effects actually used across all functions.
#[derive(Clone, Debug, Serialize)]
pub struct EffectSurface {
    /// All effect names declared anywhere in the module, sorted.
    pub effects: Vec<String>,
    /// Names of functions declaring the `IO` effect, sorted.
    pub io_functions: Vec<String>,
    /// Names of functions that are pure (no effects or only `Pure`), sorted.
    pub pure_functions: Vec<String>,
}

/// Find dead functions: those not reachable from the entry point.
pub fn find_dead_code(module: &Module, entry: &str) -> DeadCodeReport {
    let mut reachable: HashSet<String> = HashSet::new();
    let mut work: Vec<String> = vec![entry.to_string()];

    while let Some(name) = work.pop() {
        if !reachable.insert(name.clone()) {
            continue;
        }
        if let Some(func) = module.find_function(&name) {
            let mut targets = Vec::new();
            collect_call_targets(&func.body, &mut targets);
            for target in targets {
                if module.find_function(&target).is_some() && !reachable.contains(&target) {
                    work.push(target);
                }
            }
        }
    }

    let mut reachable_sorted: Vec<String> = reachable.iter().cloned().collect();
    reachable_sorted.sort();

    let all_names: HashSet<String> = module.functions().iter().map(|f| f.name.clone()).collect();
    let mut dead: Vec<String> = all_names.difference(&reachable).cloned().collect();
    dead.sort();

    DeadCodeReport {
        entry_point: entry.to_string(),
        reachable: reachable_sorted,
        dead,
    }
}

/// Count calls to each builtin (`std::...`) target.
pub fn builtin_usage(module: &Module) -> BuiltinUsage {
    let mut counts: HashMap<String, u32> = HashMap::new();
    for func in module.functions() {
        let mut targets = Vec::new();
        collect_call_targets(&func.body, &mut targets);
        for t in targets {
            if t.starts_with("std::") {
                *counts.entry(t).or_insert(0) += 1;
            }
        }
    }

    let mut unique_builtins: Vec<String> = counts.keys().cloned().collect();
    unique_builtins.sort();

    BuiltinUsage {
        counts,
        unique_builtins,
    }
}

/// Compute the effect surface: which effects are declared across functions.
pub fn effect_surface(module: &Module) -> EffectSurface {
    let mut effects: HashSet<String> = HashSet::new();
    let mut io_functions = Vec::new();
    let mut pure_functions = Vec::new();

    for func in module.functions() {
        let has_io = func
            .effects
            .iter()
            .any(|e| matches!(e, airl_ir::effects::Effect::IO));
        let is_pure = func.is_pure();

        for e in &func.effects {
            effects.insert(e.to_effect_str());
        }

        if has_io {
            io_functions.push(func.name.clone());
        }
        if is_pure {
            pure_functions.push(func.name.clone());
        }
    }

    let mut effects_sorted: Vec<String> = effects.into_iter().collect();
    effects_sorted.sort();
    io_functions.sort();
    pure_functions.sort();

    EffectSurface {
        effects: effects_sorted,
        io_functions,
        pure_functions,
    }
}

/// Find functions whose signature mentions a given type (as string representation).
pub fn find_functions_using_type(module: &Module, type_str: &str) -> Vec<String> {
    let mut result = Vec::new();
    for func in module.functions() {
        let ret_matches = func.returns.to_type_str().contains(type_str);
        let param_matches = func
            .params
            .iter()
            .any(|p| p.param_type.to_type_str().contains(type_str));
        if ret_matches || param_matches {
            result.push(func.name.clone());
        }
    }
    result.sort();
    result
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn collect_call_targets(node: &Node, out: &mut Vec<String>) {
    match node {
        Node::Call { target, args, .. } => {
            out.push(target.clone());
            for a in args {
                collect_call_targets(a, out);
            }
        }
        Node::Let { value, body, .. } => {
            collect_call_targets(value, out);
            collect_call_targets(body, out);
        }
        Node::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            collect_call_targets(cond, out);
            collect_call_targets(then_branch, out);
            collect_call_targets(else_branch, out);
        }
        Node::Block {
            statements, result, ..
        } => {
            for s in statements {
                collect_call_targets(s, out);
            }
            collect_call_targets(result, out);
        }
        Node::BinOp { lhs, rhs, .. } => {
            collect_call_targets(lhs, out);
            collect_call_targets(rhs, out);
        }
        Node::UnaryOp { operand, .. } => collect_call_targets(operand, out),
        Node::Return { value, .. } => collect_call_targets(value, out),
        Node::Loop { body, .. } => collect_call_targets(body, out),
        Node::Match {
            scrutinee, arms, ..
        } => {
            collect_call_targets(scrutinee, out);
            for arm in arms {
                collect_call_targets(&arm.body, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(json: &str) -> Module {
        serde_json::from_str(json).unwrap()
    }

    fn module_with_dead_fn() -> Module {
        load(
            r#"{
            "format_version":"0.1.0",
            "module":{"id":"m","name":"main",
                "metadata":{"version":"1","description":"","author":"","created_at":""},
                "imports":[],"exports":[],"types":[],
                "functions":[
                    {"id":"fm","name":"main","params":[],"returns":"Unit","effects":["IO"],
                        "body":{"id":"n1","kind":"Call","type":"Unit","target":"std::io::println",
                            "args":[{"id":"n2","kind":"Literal","type":"String","value":"hi"}]}},
                    {"id":"fu","name":"used_helper","params":[],"returns":"I64","effects":["Pure"],
                        "body":{"id":"n3","kind":"Literal","type":"I64","value":1}},
                    {"id":"fd","name":"dead_function","params":[],"returns":"I64","effects":["Pure"],
                        "body":{"id":"n4","kind":"Literal","type":"I64","value":42}}
                ]
            }
        }"#,
        )
    }

    #[test]
    fn test_find_dead_code() {
        let module = module_with_dead_fn();
        let report = find_dead_code(&module, "main");
        assert!(report.reachable.contains(&"main".to_string()));
        assert!(report.dead.contains(&"dead_function".to_string()));
        assert!(report.dead.contains(&"used_helper".to_string()));
    }

    #[test]
    fn test_builtin_usage() {
        let module = module_with_dead_fn();
        let usage = builtin_usage(&module);
        assert!(usage
            .unique_builtins
            .contains(&"std::io::println".to_string()));
        assert_eq!(usage.counts.get("std::io::println"), Some(&1));
    }

    #[test]
    fn test_effect_surface() {
        let module = module_with_dead_fn();
        let surface = effect_surface(&module);
        assert!(surface.effects.contains(&"IO".to_string()));
        assert!(surface.effects.contains(&"Pure".to_string()));
        assert!(surface.io_functions.contains(&"main".to_string()));
        assert!(surface.pure_functions.contains(&"used_helper".to_string()));
    }

    #[test]
    fn test_find_functions_using_type() {
        let module = module_with_dead_fn();
        let i64_funcs = find_functions_using_type(&module, "I64");
        assert!(i64_funcs.contains(&"used_helper".to_string()));
        assert!(i64_funcs.contains(&"dead_function".to_string()));
    }
}
