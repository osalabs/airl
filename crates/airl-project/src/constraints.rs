//! Constraint system: enforce project-level invariants.
//!
//! Constraints define what must hold true about a project. They're checked
//! automatically before patches are committed, catching architectural
//! violations early.
//!
//! ## Supported constraints
//! - `MaxFunctionComplexity` — cyclomatic complexity limit per function
//! - `MaxModuleSize` — total node count limit per module
//! - `MaxFunctionCount` — number of functions per module
//! - `RequiredEffectPurity` — functions matching glob must be Pure
//! - `ForbiddenEffect` — functions matching glob must not have a given effect
//! - `ForbiddenTarget` — functions must not call a given target
//! - `MaxCallDepth` — limit on static call graph depth (no deep recursion)

use airl_ir::module::Module;
use airl_ir::node::Node;
use serde::{Deserialize, Serialize};

/// A single constraint definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Constraint {
    /// No function may exceed this cyclomatic complexity.
    MaxFunctionComplexity {
        /// Maximum allowed cyclomatic complexity.
        threshold: u32,
    },
    /// Module must not exceed this total node count.
    MaxModuleSize {
        /// Maximum allowed total node count across all functions.
        max_nodes: u32,
    },
    /// Module must not contain more than this many functions.
    MaxFunctionCount {
        /// Maximum allowed number of functions in the module.
        max: u32,
    },
    /// Functions matching `pattern` (substring match) must be pure.
    RequiredEffectPurity {
        /// Substring to match against function names.
        pattern: String,
    },
    /// Functions matching `pattern` must not have the given effect.
    ForbiddenEffect {
        /// Substring to match against function names.
        pattern: String,
        /// The effect name that is forbidden (e.g. `"IO"`, `"Allocate"`).
        effect: String,
    },
    /// No function may call `target`.
    ForbiddenTarget {
        /// The call target that is forbidden (e.g. `"std::process::exit"`).
        target: String,
    },
    /// Static call chain must not exceed this depth.
    MaxCallDepth {
        /// Maximum allowed static call depth.
        max_depth: u32,
    },
}

/// A single constraint violation, produced by [`check_constraint`] or [`check_all`].
#[derive(Clone, Debug, Serialize)]
pub struct ConstraintViolation {
    /// Display form of the constraint that was violated, e.g. `"MaxFunctionCount(5)"`.
    pub constraint: String,
    /// Human-readable description of the violation.
    pub message: String,
    /// Name of the offending function, if the violation is function-scoped.
    pub function: Option<String>,
}

/// Result of checking all constraints against a module.
///
/// Use [`ConstraintReport::is_ok`] to check if all constraints passed.
#[derive(Clone, Debug, Serialize)]
pub struct ConstraintReport {
    /// All violations found, in the order constraints were checked.
    pub violations: Vec<ConstraintViolation>,
}

impl ConstraintReport {
    /// Returns `true` if no constraints were violated.
    pub fn is_ok(&self) -> bool {
        self.violations.is_empty()
    }
}

/// Check a single constraint against a module.
pub fn check_constraint(constraint: &Constraint, module: &Module) -> Vec<ConstraintViolation> {
    let mut violations = Vec::new();

    match constraint {
        Constraint::MaxFunctionComplexity { threshold } => {
            for func in module.functions() {
                let complexity = cyclomatic_complexity(&func.body);
                if complexity > *threshold {
                    violations.push(ConstraintViolation {
                        constraint: format!("MaxFunctionComplexity({threshold})"),
                        message: format!(
                            "function '{}' has complexity {complexity}, exceeds {threshold}",
                            func.name
                        ),
                        function: Some(func.name.clone()),
                    });
                }
            }
        }
        Constraint::MaxModuleSize { max_nodes } => {
            let total: u32 = module
                .functions()
                .iter()
                .map(|f| count_nodes(&f.body))
                .sum();
            if total > *max_nodes {
                violations.push(ConstraintViolation {
                    constraint: format!("MaxModuleSize({max_nodes})"),
                    message: format!("module has {total} nodes, exceeds {max_nodes}"),
                    function: None,
                });
            }
        }
        Constraint::MaxFunctionCount { max } => {
            let count = module.functions().len() as u32;
            if count > *max {
                violations.push(ConstraintViolation {
                    constraint: format!("MaxFunctionCount({max})"),
                    message: format!("module has {count} functions, exceeds {max}"),
                    function: None,
                });
            }
        }
        Constraint::RequiredEffectPurity { pattern } => {
            for func in module.functions() {
                if func.name.contains(pattern) && !func.is_pure() {
                    let effects: Vec<String> =
                        func.effects.iter().map(|e| e.to_effect_str()).collect();
                    violations.push(ConstraintViolation {
                        constraint: format!("RequiredEffectPurity({pattern})"),
                        message: format!(
                            "function '{}' matches pattern but has effects [{}]",
                            func.name,
                            effects.join(", ")
                        ),
                        function: Some(func.name.clone()),
                    });
                }
            }
        }
        Constraint::ForbiddenEffect { pattern, effect } => {
            for func in module.functions() {
                if func.name.contains(pattern) {
                    let has = func.effects.iter().any(|e| e.to_effect_str() == *effect);
                    if has {
                        violations.push(ConstraintViolation {
                            constraint: format!("ForbiddenEffect({pattern},{effect})"),
                            message: format!(
                                "function '{}' matches pattern and has forbidden effect '{effect}'",
                                func.name
                            ),
                            function: Some(func.name.clone()),
                        });
                    }
                }
            }
        }
        Constraint::ForbiddenTarget { target } => {
            for func in module.functions() {
                if contains_call_to(&func.body, target) {
                    violations.push(ConstraintViolation {
                        constraint: format!("ForbiddenTarget({target})"),
                        message: format!(
                            "function '{}' calls forbidden target '{target}'",
                            func.name
                        ),
                        function: Some(func.name.clone()),
                    });
                }
            }
        }
        Constraint::MaxCallDepth { max_depth } => {
            // Compute max static call depth via call graph
            let depths = compute_call_depths(module);
            for (func_name, depth) in depths {
                if depth > *max_depth {
                    violations.push(ConstraintViolation {
                        constraint: format!("MaxCallDepth({max_depth})"),
                        message: format!(
                            "function '{func_name}' reaches call depth {depth}, exceeds {max_depth}"
                        ),
                        function: Some(func_name),
                    });
                }
            }
        }
    }

    violations
}

/// Check all constraints against a module.
pub fn check_all(constraints: &[Constraint], module: &Module) -> ConstraintReport {
    let mut violations = Vec::new();
    for c in constraints {
        violations.extend(check_constraint(c, module));
    }
    ConstraintReport { violations }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Cyclomatic complexity: 1 + number of decision points (if, match arms, loops).
fn cyclomatic_complexity(node: &Node) -> u32 {
    fn count(node: &Node) -> u32 {
        match node {
            Node::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => 1 + count(cond) + count(then_branch) + count(else_branch),
            Node::Match {
                scrutinee, arms, ..
            } => {
                // Each arm after the first adds a decision
                let arm_count = if arms.is_empty() {
                    0
                } else {
                    (arms.len() - 1) as u32
                };
                arm_count + count(scrutinee) + arms.iter().map(|a| count(&a.body)).sum::<u32>()
            }
            Node::Loop { body, .. } => 1 + count(body),
            Node::Let { value, body, .. } => count(value) + count(body),
            Node::Block {
                statements, result, ..
            } => statements.iter().map(count).sum::<u32>() + count(result),
            Node::Call { args, .. } => args.iter().map(count).sum(),
            Node::Return { value, .. } => count(value),
            Node::BinOp { lhs, rhs, op, .. } => {
                // Short-circuit operators add decision points
                let op_cost = if matches!(
                    op,
                    airl_ir::node::BinOpKind::And | airl_ir::node::BinOpKind::Or
                ) {
                    1
                } else {
                    0
                };
                op_cost + count(lhs) + count(rhs)
            }
            Node::UnaryOp { operand, .. } => count(operand),
            Node::ArrayLiteral { elements, .. } => elements.iter().map(count).sum(),
            Node::IndexAccess { array, index, .. } => count(array) + count(index),
            Node::StructLiteral { fields, .. } => fields.iter().map(|(_, n)| count(n)).sum(),
            Node::FieldAccess { object, .. } => count(object),
            _ => 0,
        }
    }
    1 + count(node)
}

/// Count total nodes in a subtree.
fn count_nodes(node: &Node) -> u32 {
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

/// Check if a node tree contains a call to the given target.
fn contains_call_to(node: &Node, target: &str) -> bool {
    match node {
        Node::Call {
            target: t, args, ..
        } => t == target || args.iter().any(|a| contains_call_to(a, target)),
        Node::Let { value, body, .. } => {
            contains_call_to(value, target) || contains_call_to(body, target)
        }
        Node::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            contains_call_to(cond, target)
                || contains_call_to(then_branch, target)
                || contains_call_to(else_branch, target)
        }
        Node::Block {
            statements, result, ..
        } => {
            statements.iter().any(|s| contains_call_to(s, target))
                || contains_call_to(result, target)
        }
        Node::BinOp { lhs, rhs, .. } => {
            contains_call_to(lhs, target) || contains_call_to(rhs, target)
        }
        Node::UnaryOp { operand, .. } => contains_call_to(operand, target),
        Node::Return { value, .. } => contains_call_to(value, target),
        Node::Loop { body, .. } => contains_call_to(body, target),
        Node::Match {
            scrutinee, arms, ..
        } => {
            contains_call_to(scrutinee, target)
                || arms.iter().any(|a| contains_call_to(&a.body, target))
        }
        _ => false,
    }
}

/// Compute the maximum static call depth for each user-defined function.
/// Depth 0 = leaf (no calls to user functions). Self-recursive functions
/// are reported as depth u32::MAX / 2 (capped to avoid overflow).
fn compute_call_depths(module: &Module) -> Vec<(String, u32)> {
    let user_funcs: std::collections::HashSet<String> =
        module.functions().iter().map(|f| f.name.clone()).collect();

    let mut result = Vec::new();
    for func in module.functions() {
        let mut visited = std::collections::HashSet::new();
        let depth = call_depth(&func.body, &user_funcs, module, &mut visited, 0);
        result.push((func.name.clone(), depth));
    }
    result
}

fn call_depth(
    node: &Node,
    user_funcs: &std::collections::HashSet<String>,
    module: &Module,
    visited: &mut std::collections::HashSet<String>,
    current: u32,
) -> u32 {
    if current > 100 {
        return current; // safety cap
    }
    let mut max = current;
    match node {
        Node::Call { target, args, .. } => {
            if user_funcs.contains(target) && !visited.contains(target) {
                if let Some(callee) = module.find_function(target) {
                    visited.insert(target.clone());
                    let sub = call_depth(&callee.body, user_funcs, module, visited, current + 1);
                    visited.remove(target);
                    max = max.max(sub);
                }
            }
            for arg in args {
                max = max.max(call_depth(arg, user_funcs, module, visited, current));
            }
        }
        Node::Let { value, body, .. } => {
            max = max.max(call_depth(value, user_funcs, module, visited, current));
            max = max.max(call_depth(body, user_funcs, module, visited, current));
        }
        Node::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            max = max.max(call_depth(cond, user_funcs, module, visited, current));
            max = max.max(call_depth(
                then_branch,
                user_funcs,
                module,
                visited,
                current,
            ));
            max = max.max(call_depth(
                else_branch,
                user_funcs,
                module,
                visited,
                current,
            ));
        }
        Node::Block {
            statements, result, ..
        } => {
            for s in statements {
                max = max.max(call_depth(s, user_funcs, module, visited, current));
            }
            max = max.max(call_depth(result, user_funcs, module, visited, current));
        }
        Node::BinOp { lhs, rhs, .. } => {
            max = max.max(call_depth(lhs, user_funcs, module, visited, current));
            max = max.max(call_depth(rhs, user_funcs, module, visited, current));
        }
        Node::UnaryOp { operand, .. } => {
            max = max.max(call_depth(operand, user_funcs, module, visited, current));
        }
        Node::Return { value, .. } => {
            max = max.max(call_depth(value, user_funcs, module, visited, current));
        }
        Node::Loop { body, .. } => {
            max = max.max(call_depth(body, user_funcs, module, visited, current));
        }
        Node::Match {
            scrutinee, arms, ..
        } => {
            max = max.max(call_depth(scrutinee, user_funcs, module, visited, current));
            for arm in arms {
                max = max.max(call_depth(&arm.body, user_funcs, module, visited, current));
            }
        }
        _ => {}
    }
    max
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(json: &str) -> Module {
        serde_json::from_str(json).unwrap()
    }

    fn hello_module() -> Module {
        load(
            r#"{
            "format_version":"0.1.0",
            "module":{"id":"m","name":"main",
                "metadata":{"version":"1","description":"","author":"","created_at":""},
                "imports":[],"exports":[],"types":[],
                "functions":[{
                    "id":"f","name":"main","params":[],"returns":"Unit","effects":["IO"],
                    "body":{"id":"n1","kind":"Call","type":"Unit","target":"std::io::println",
                        "args":[{"id":"n2","kind":"Literal","type":"String","value":"hi"}]}
                }]
            }
        }"#,
        )
    }

    #[test]
    fn test_max_function_count_ok() {
        let module = hello_module();
        let r = check_constraint(&Constraint::MaxFunctionCount { max: 10 }, &module);
        assert!(r.is_empty());
    }

    #[test]
    fn test_max_function_count_violation() {
        let module = hello_module();
        let r = check_constraint(&Constraint::MaxFunctionCount { max: 0 }, &module);
        assert_eq!(r.len(), 1);
        assert!(r[0].message.contains("1 functions"));
    }

    #[test]
    fn test_forbidden_target() {
        let module = hello_module();
        let r = check_constraint(
            &Constraint::ForbiddenTarget {
                target: "std::io::println".to_string(),
            },
            &module,
        );
        assert_eq!(r.len(), 1);
        assert!(r[0].message.contains("forbidden"));
    }

    #[test]
    fn test_required_purity_violation() {
        let module = hello_module();
        let r = check_constraint(
            &Constraint::RequiredEffectPurity {
                pattern: "main".to_string(),
            },
            &module,
        );
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn test_max_module_size() {
        let module = hello_module();
        // Module has only 2 nodes — pass with limit 10, fail with limit 1
        let ok = check_constraint(&Constraint::MaxModuleSize { max_nodes: 10 }, &module);
        assert!(ok.is_empty());
        let fail = check_constraint(&Constraint::MaxModuleSize { max_nodes: 1 }, &module);
        assert_eq!(fail.len(), 1);
    }

    #[test]
    fn test_cyclomatic_complexity_simple() {
        let module = hello_module();
        // Linear function has complexity 1
        let ok = check_constraint(
            &Constraint::MaxFunctionComplexity { threshold: 10 },
            &module,
        );
        assert!(ok.is_empty());
    }

    #[test]
    fn test_check_all_combines_violations() {
        let module = hello_module();
        let constraints = vec![
            Constraint::MaxFunctionCount { max: 0 },
            Constraint::ForbiddenTarget {
                target: "std::io::println".to_string(),
            },
        ];
        let report = check_all(&constraints, &module);
        assert_eq!(report.violations.len(), 2);
        assert!(!report.is_ok());
    }

    #[test]
    fn test_constraint_serde() {
        let c = Constraint::MaxFunctionComplexity { threshold: 15 };
        let json = serde_json::to_string(&c).unwrap();
        assert!(json.contains("MaxFunctionComplexity"));
        assert!(json.contains("15"));
        let parsed: Constraint = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            parsed,
            Constraint::MaxFunctionComplexity { threshold: 15 }
        ));
    }
}
