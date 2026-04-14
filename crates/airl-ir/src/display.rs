//! Debug display implementations for key IR types.

use crate::module::{FuncDef, Module};
use crate::node::Node;
use std::fmt;

/// A helper for pretty-printing a Module summary.
pub struct ModuleDisplay<'a>(pub &'a Module);

impl<'a> fmt::Display for ModuleDisplay<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let m = &self.0.module;
        writeln!(f, "Module: {} ({})", m.name, m.id)?;
        writeln!(f, "  Format: {}", self.0.format_version)?;
        writeln!(f, "  Version: {}", m.metadata.version)?;
        writeln!(f, "  Description: {}", m.metadata.description)?;
        writeln!(f, "  Imports: {}", m.imports.len())?;
        writeln!(f, "  Exports: {}", m.exports.len())?;
        writeln!(f, "  Types: {}", m.types.len())?;
        writeln!(f, "  Functions: {}", m.functions.len())?;
        for func in &m.functions {
            writeln!(f, "    fn {} -> {}", func.name, func.returns)?;
        }
        Ok(())
    }
}

/// A helper for pretty-printing a FuncDef summary.
pub struct FuncDefDisplay<'a>(pub &'a FuncDef);

impl<'a> fmt::Display for FuncDefDisplay<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let func = self.0;
        write!(f, "fn {}(", func.name)?;
        for (i, param) in func.params.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}: {}", param.name, param.param_type)?;
        }
        write!(f, ") -> {}", func.returns)?;
        if !func.effects.is_empty() {
            write!(f, " [")?;
            for (i, effect) in func.effects.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{effect}")?;
            }
            write!(f, "]")?;
        }
        Ok(())
    }
}

/// A helper for pretty-printing a Node tree with indentation.
pub struct NodeTreeDisplay<'a> {
    pub node: &'a Node,
    pub indent: usize,
}

impl<'a> NodeTreeDisplay<'a> {
    pub fn new(node: &'a Node) -> Self {
        NodeTreeDisplay { node, indent: 0 }
    }

    fn write_indented(&self, f: &mut fmt::Formatter<'_>, depth: usize, node: &Node) -> fmt::Result {
        let prefix = "  ".repeat(depth);
        match node {
            Node::Literal { id, value, node_type, .. } => {
                writeln!(f, "{prefix}[{id}] Literal({value:?}) : {node_type}")
            }
            Node::Param { id, name, node_type, .. } => {
                writeln!(f, "{prefix}[{id}] Param({name}) : {node_type}")
            }
            Node::Let { id, name, value, body, node_type, .. } => {
                writeln!(f, "{prefix}[{id}] Let {name} : {node_type}")?;
                self.write_indented(f, depth + 1, value)?;
                self.write_indented(f, depth + 1, body)
            }
            Node::If { id, cond, then_branch, else_branch, node_type, .. } => {
                writeln!(f, "{prefix}[{id}] If : {node_type}")?;
                self.write_indented(f, depth + 1, cond)?;
                writeln!(f, "{prefix}  then:")?;
                self.write_indented(f, depth + 2, then_branch)?;
                writeln!(f, "{prefix}  else:")?;
                self.write_indented(f, depth + 2, else_branch)
            }
            Node::Call { id, target, args, node_type, .. } => {
                writeln!(f, "{prefix}[{id}] Call {target} : {node_type}")?;
                for arg in args {
                    self.write_indented(f, depth + 1, arg)?;
                }
                Ok(())
            }
            Node::Return { id, value, node_type, .. } => {
                writeln!(f, "{prefix}[{id}] Return : {node_type}")?;
                self.write_indented(f, depth + 1, value)
            }
            Node::BinOp { id, op, lhs, rhs, node_type, .. } => {
                writeln!(f, "{prefix}[{id}] BinOp({op:?}) : {node_type}")?;
                self.write_indented(f, depth + 1, lhs)?;
                self.write_indented(f, depth + 1, rhs)
            }
            Node::UnaryOp { id, op, operand, node_type, .. } => {
                writeln!(f, "{prefix}[{id}] UnaryOp({op:?}) : {node_type}")?;
                self.write_indented(f, depth + 1, operand)
            }
            Node::Block { id, statements, result, node_type, .. } => {
                writeln!(f, "{prefix}[{id}] Block : {node_type}")?;
                for stmt in statements {
                    self.write_indented(f, depth + 1, stmt)?;
                }
                self.write_indented(f, depth + 1, result)
            }
            Node::Loop { id, body, node_type, .. } => {
                writeln!(f, "{prefix}[{id}] Loop : {node_type}")?;
                self.write_indented(f, depth + 1, body)
            }
            Node::Match { id, scrutinee, arms, node_type, .. } => {
                writeln!(f, "{prefix}[{id}] Match : {node_type}")?;
                self.write_indented(f, depth + 1, scrutinee)?;
                for arm in arms {
                    writeln!(f, "{prefix}  arm {:?}:", arm.pattern)?;
                    self.write_indented(f, depth + 2, &arm.body)?;
                }
                Ok(())
            }
            Node::StructLiteral { id, fields, node_type, .. } => {
                writeln!(f, "{prefix}[{id}] StructLiteral : {node_type}")?;
                for (name, val) in fields {
                    writeln!(f, "{prefix}  {name}:")?;
                    self.write_indented(f, depth + 2, val)?;
                }
                Ok(())
            }
            Node::FieldAccess { id, object, field, node_type, .. } => {
                writeln!(f, "{prefix}[{id}] FieldAccess .{field} : {node_type}")?;
                self.write_indented(f, depth + 1, object)
            }
            Node::ArrayLiteral { id, elements, node_type, .. } => {
                writeln!(f, "{prefix}[{id}] ArrayLiteral : {node_type}")?;
                for elem in elements {
                    self.write_indented(f, depth + 1, elem)?;
                }
                Ok(())
            }
            Node::IndexAccess { id, array, index, node_type, .. } => {
                writeln!(f, "{prefix}[{id}] IndexAccess : {node_type}")?;
                self.write_indented(f, depth + 1, array)?;
                self.write_indented(f, depth + 1, index)
            }
            Node::Error { id, message, .. } => {
                writeln!(f, "{prefix}[{id}] Error: {message}")
            }
        }
    }
}

impl<'a> fmt::Display for NodeTreeDisplay<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_indented(f, self.indent, self.node)
    }
}
