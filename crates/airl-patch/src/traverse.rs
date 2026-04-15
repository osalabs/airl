//! Tree traversal utilities for AIRL IR nodes.
//!
//! Since nodes are inline trees (not graph-referenced), these utilities
//! provide the building blocks for finding, replacing, and transforming
//! nodes by their NodeId.

use airl_ir::ids::{FuncId, NodeId};
use airl_ir::module::{FuncDef, Module};
use airl_ir::node::{MatchArm, Node};

/// Find which function contains a node with the given ID.
pub fn find_containing_function<'a>(module: &'a Module, target: &NodeId) -> Option<&'a FuncDef> {
    module
        .functions()
        .iter()
        .find(|func| node_contains_id(&func.body, target))
}

/// Check if a node tree contains a node with the given ID.
pub fn node_contains_id(node: &Node, target: &NodeId) -> bool {
    if node.id() == target {
        return true;
    }
    children(node)
        .iter()
        .any(|child| node_contains_id(child, target))
}

/// Find a node by ID in a tree, returning a reference to it.
pub fn find_node<'a>(node: &'a Node, target: &NodeId) -> Option<&'a Node> {
    if node.id() == target {
        return Some(node);
    }
    for child in children(node) {
        if let Some(found) = find_node(child, target) {
            return Some(found);
        }
    }
    None
}

/// Replace a node by ID in a tree, returning a new tree with the replacement.
/// Returns None if the target was not found.
pub fn replace_node_in_tree(root: &Node, target: &NodeId, replacement: &Node) -> Option<Node> {
    if root.id() == target {
        return Some(replacement.clone());
    }
    replace_in_node(root, target, replacement)
}

/// Collect all NodeIds in a tree.
pub fn collect_node_ids(node: &Node) -> Vec<NodeId> {
    let mut ids = vec![node.id().clone()];
    for child in children(node) {
        ids.extend(collect_node_ids(child));
    }
    ids
}

/// Rename all occurrences of a symbol in a node tree.
/// Renames: variable names in Param/Let, call targets in Call.
pub fn rename_in_tree(node: &Node, old_name: &str, new_name: &str) -> Node {
    match node {
        Node::Param {
            id,
            name,
            index,
            node_type,
        } => Node::Param {
            id: id.clone(),
            name: if name == old_name {
                new_name.to_string()
            } else {
                name.clone()
            },
            index: *index,
            node_type: node_type.clone(),
        },

        Node::Let {
            id,
            name,
            node_type,
            value,
            body,
        } => Node::Let {
            id: id.clone(),
            name: if name == old_name {
                new_name.to_string()
            } else {
                name.clone()
            },
            node_type: node_type.clone(),
            value: Box::new(rename_in_tree(value, old_name, new_name)),
            body: Box::new(rename_in_tree(body, old_name, new_name)),
        },

        Node::Call {
            id,
            node_type,
            target,
            args,
        } => Node::Call {
            id: id.clone(),
            node_type: node_type.clone(),
            target: if target == old_name {
                new_name.to_string()
            } else {
                target.clone()
            },
            args: args
                .iter()
                .map(|a| rename_in_tree(a, old_name, new_name))
                .collect(),
        },

        // For all other nodes, recursively rename in children
        other => map_children(other, &|child| rename_in_tree(child, old_name, new_name)),
    }
}

/// Collect all function IDs that contain a given node ID.
pub fn functions_containing_node(module: &Module, target: &NodeId) -> Vec<FuncId> {
    module
        .functions()
        .iter()
        .filter(|f| node_contains_id(&f.body, target))
        .map(|f| f.id.clone())
        .collect()
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Get the direct children of a node as references.
fn children(node: &Node) -> Vec<&Node> {
    match node {
        Node::Literal { .. } | Node::Param { .. } | Node::Error { .. } => vec![],

        Node::Let { value, body, .. } => vec![value.as_ref(), body.as_ref()],
        Node::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => vec![cond.as_ref(), then_branch.as_ref(), else_branch.as_ref()],
        Node::Call { args, .. } => args.iter().collect(),
        Node::Return { value, .. } => vec![value.as_ref()],
        Node::BinOp { lhs, rhs, .. } => vec![lhs.as_ref(), rhs.as_ref()],
        Node::UnaryOp { operand, .. } => vec![operand.as_ref()],
        Node::Block {
            statements, result, ..
        } => {
            let mut v: Vec<&Node> = statements.iter().collect();
            v.push(result.as_ref());
            v
        }
        Node::Loop { body, .. } => vec![body.as_ref()],
        Node::Match {
            scrutinee, arms, ..
        } => {
            let mut v = vec![scrutinee.as_ref()];
            for arm in arms {
                v.push(&arm.body);
            }
            v
        }
        Node::StructLiteral { fields, .. } => fields.iter().map(|(_, n)| n).collect(),
        Node::FieldAccess { object, .. } => vec![object.as_ref()],
        Node::ArrayLiteral { elements, .. } => elements.iter().collect(),
        Node::IndexAccess { array, index, .. } => vec![array.as_ref(), index.as_ref()],
    }
}

/// Try to replace a target node inside `root`'s children.
/// Returns None if target is not found in any child subtree.
fn replace_in_node(root: &Node, target: &NodeId, replacement: &Node) -> Option<Node> {
    match root {
        Node::Literal { .. } | Node::Param { .. } | Node::Error { .. } => None,

        Node::Let {
            id,
            name,
            node_type,
            value,
            body,
        } => {
            let new_value = replace_node_in_tree(value, target, replacement);
            let new_body = replace_node_in_tree(body, target, replacement);
            if new_value.is_some() || new_body.is_some() {
                Some(Node::Let {
                    id: id.clone(),
                    name: name.clone(),
                    node_type: node_type.clone(),
                    value: Box::new(new_value.unwrap_or_else(|| value.as_ref().clone())),
                    body: Box::new(new_body.unwrap_or_else(|| body.as_ref().clone())),
                })
            } else {
                None
            }
        }

        Node::If {
            id,
            node_type,
            cond,
            then_branch,
            else_branch,
        } => {
            let nc = replace_node_in_tree(cond, target, replacement);
            let nt = replace_node_in_tree(then_branch, target, replacement);
            let ne = replace_node_in_tree(else_branch, target, replacement);
            if nc.is_some() || nt.is_some() || ne.is_some() {
                Some(Node::If {
                    id: id.clone(),
                    node_type: node_type.clone(),
                    cond: Box::new(nc.unwrap_or_else(|| cond.as_ref().clone())),
                    then_branch: Box::new(nt.unwrap_or_else(|| then_branch.as_ref().clone())),
                    else_branch: Box::new(ne.unwrap_or_else(|| else_branch.as_ref().clone())),
                })
            } else {
                None
            }
        }

        Node::Call {
            id,
            node_type,
            target: call_target,
            args,
        } => {
            let mut changed = false;
            let new_args: Vec<Node> = args
                .iter()
                .map(|a| {
                    if let Some(replaced) = replace_node_in_tree(a, target, replacement) {
                        changed = true;
                        replaced
                    } else {
                        a.clone()
                    }
                })
                .collect();
            if changed {
                Some(Node::Call {
                    id: id.clone(),
                    node_type: node_type.clone(),
                    target: call_target.clone(),
                    args: new_args,
                })
            } else {
                None
            }
        }

        Node::Return {
            id,
            node_type,
            value,
        } => replace_node_in_tree(value, target, replacement).map(|nv| Node::Return {
            id: id.clone(),
            node_type: node_type.clone(),
            value: Box::new(nv),
        }),

        Node::BinOp {
            id,
            op,
            node_type,
            lhs,
            rhs,
        } => {
            let nl = replace_node_in_tree(lhs, target, replacement);
            let nr = replace_node_in_tree(rhs, target, replacement);
            if nl.is_some() || nr.is_some() {
                Some(Node::BinOp {
                    id: id.clone(),
                    op: op.clone(),
                    node_type: node_type.clone(),
                    lhs: Box::new(nl.unwrap_or_else(|| lhs.as_ref().clone())),
                    rhs: Box::new(nr.unwrap_or_else(|| rhs.as_ref().clone())),
                })
            } else {
                None
            }
        }

        Node::UnaryOp {
            id,
            op,
            node_type,
            operand,
        } => replace_node_in_tree(operand, target, replacement).map(|no| Node::UnaryOp {
            id: id.clone(),
            op: op.clone(),
            node_type: node_type.clone(),
            operand: Box::new(no),
        }),

        Node::Block {
            id,
            node_type,
            statements,
            result,
        } => {
            let mut changed = false;
            let new_stmts: Vec<Node> = statements
                .iter()
                .map(|s| {
                    if let Some(replaced) = replace_node_in_tree(s, target, replacement) {
                        changed = true;
                        replaced
                    } else {
                        s.clone()
                    }
                })
                .collect();
            let new_result = replace_node_in_tree(result, target, replacement);
            if changed || new_result.is_some() {
                Some(Node::Block {
                    id: id.clone(),
                    node_type: node_type.clone(),
                    statements: new_stmts,
                    result: Box::new(new_result.unwrap_or_else(|| result.as_ref().clone())),
                })
            } else {
                None
            }
        }

        Node::Loop {
            id,
            node_type,
            body,
        } => replace_node_in_tree(body, target, replacement).map(|nb| Node::Loop {
            id: id.clone(),
            node_type: node_type.clone(),
            body: Box::new(nb),
        }),

        Node::Match {
            id,
            node_type,
            scrutinee,
            arms,
        } => {
            let ns = replace_node_in_tree(scrutinee, target, replacement);
            let mut arms_changed = false;
            let new_arms: Vec<MatchArm> = arms
                .iter()
                .map(|arm| {
                    if let Some(nb) = replace_node_in_tree(&arm.body, target, replacement) {
                        arms_changed = true;
                        MatchArm {
                            pattern: arm.pattern.clone(),
                            body: nb,
                        }
                    } else {
                        arm.clone()
                    }
                })
                .collect();
            if ns.is_some() || arms_changed {
                Some(Node::Match {
                    id: id.clone(),
                    node_type: node_type.clone(),
                    scrutinee: Box::new(ns.unwrap_or_else(|| scrutinee.as_ref().clone())),
                    arms: new_arms,
                })
            } else {
                None
            }
        }

        Node::StructLiteral {
            id,
            node_type,
            fields,
        } => {
            let mut changed = false;
            let new_fields: Vec<(String, Node)> = fields
                .iter()
                .map(|(name, node)| {
                    if let Some(replaced) = replace_node_in_tree(node, target, replacement) {
                        changed = true;
                        (name.clone(), replaced)
                    } else {
                        (name.clone(), node.clone())
                    }
                })
                .collect();
            if changed {
                Some(Node::StructLiteral {
                    id: id.clone(),
                    node_type: node_type.clone(),
                    fields: new_fields,
                })
            } else {
                None
            }
        }

        Node::FieldAccess {
            id,
            node_type,
            object,
            field,
        } => replace_node_in_tree(object, target, replacement).map(|no| Node::FieldAccess {
            id: id.clone(),
            node_type: node_type.clone(),
            object: Box::new(no),
            field: field.clone(),
        }),

        Node::ArrayLiteral {
            id,
            node_type,
            elements,
        } => {
            let mut changed = false;
            let new_elements: Vec<Node> = elements
                .iter()
                .map(|e| {
                    if let Some(replaced) = replace_node_in_tree(e, target, replacement) {
                        changed = true;
                        replaced
                    } else {
                        e.clone()
                    }
                })
                .collect();
            if changed {
                Some(Node::ArrayLiteral {
                    id: id.clone(),
                    node_type: node_type.clone(),
                    elements: new_elements,
                })
            } else {
                None
            }
        }

        Node::IndexAccess {
            id,
            node_type,
            array,
            index,
        } => {
            let na = replace_node_in_tree(array, target, replacement);
            let ni = replace_node_in_tree(index, target, replacement);
            if na.is_some() || ni.is_some() {
                Some(Node::IndexAccess {
                    id: id.clone(),
                    node_type: node_type.clone(),
                    array: Box::new(na.unwrap_or_else(|| array.as_ref().clone())),
                    index: Box::new(ni.unwrap_or_else(|| index.as_ref().clone())),
                })
            } else {
                None
            }
        }
    }
}

/// Map a function over all children of a node, producing a new node.
/// Used for generic transformations (e.g., rename).
fn map_children(node: &Node, f: &dyn Fn(&Node) -> Node) -> Node {
    match node {
        Node::Literal { .. } | Node::Param { .. } | Node::Error { .. } => node.clone(),

        Node::Let {
            id,
            name,
            node_type,
            value,
            body,
        } => Node::Let {
            id: id.clone(),
            name: name.clone(),
            node_type: node_type.clone(),
            value: Box::new(f(value)),
            body: Box::new(f(body)),
        },

        Node::If {
            id,
            node_type,
            cond,
            then_branch,
            else_branch,
        } => Node::If {
            id: id.clone(),
            node_type: node_type.clone(),
            cond: Box::new(f(cond)),
            then_branch: Box::new(f(then_branch)),
            else_branch: Box::new(f(else_branch)),
        },

        Node::Call {
            id,
            node_type,
            target,
            args,
        } => Node::Call {
            id: id.clone(),
            node_type: node_type.clone(),
            target: target.clone(),
            args: args.iter().map(f).collect(),
        },

        Node::Return {
            id,
            node_type,
            value,
        } => Node::Return {
            id: id.clone(),
            node_type: node_type.clone(),
            value: Box::new(f(value)),
        },

        Node::BinOp {
            id,
            op,
            node_type,
            lhs,
            rhs,
        } => Node::BinOp {
            id: id.clone(),
            op: op.clone(),
            node_type: node_type.clone(),
            lhs: Box::new(f(lhs)),
            rhs: Box::new(f(rhs)),
        },

        Node::UnaryOp {
            id,
            op,
            node_type,
            operand,
        } => Node::UnaryOp {
            id: id.clone(),
            op: op.clone(),
            node_type: node_type.clone(),
            operand: Box::new(f(operand)),
        },

        Node::Block {
            id,
            node_type,
            statements,
            result,
        } => Node::Block {
            id: id.clone(),
            node_type: node_type.clone(),
            statements: statements.iter().map(f).collect(),
            result: Box::new(f(result)),
        },

        Node::Loop {
            id,
            node_type,
            body,
        } => Node::Loop {
            id: id.clone(),
            node_type: node_type.clone(),
            body: Box::new(f(body)),
        },

        Node::Match {
            id,
            node_type,
            scrutinee,
            arms,
        } => Node::Match {
            id: id.clone(),
            node_type: node_type.clone(),
            scrutinee: Box::new(f(scrutinee)),
            arms: arms
                .iter()
                .map(|arm| MatchArm {
                    pattern: arm.pattern.clone(),
                    body: f(&arm.body),
                })
                .collect(),
        },

        Node::StructLiteral {
            id,
            node_type,
            fields,
        } => Node::StructLiteral {
            id: id.clone(),
            node_type: node_type.clone(),
            fields: fields.iter().map(|(n, v)| (n.clone(), f(v))).collect(),
        },

        Node::FieldAccess {
            id,
            node_type,
            object,
            field,
        } => Node::FieldAccess {
            id: id.clone(),
            node_type: node_type.clone(),
            object: Box::new(f(object)),
            field: field.clone(),
        },

        Node::ArrayLiteral {
            id,
            node_type,
            elements,
        } => Node::ArrayLiteral {
            id: id.clone(),
            node_type: node_type.clone(),
            elements: elements.iter().map(f).collect(),
        },

        Node::IndexAccess {
            id,
            node_type,
            array,
            index,
        } => Node::IndexAccess {
            id: id.clone(),
            node_type: node_type.clone(),
            array: Box::new(f(array)),
            index: Box::new(f(index)),
        },
    }
}
