use crate::ids::NodeId;
use crate::types::Type;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

// ---------------------------------------------------------------------------
// Supporting enums
// ---------------------------------------------------------------------------

/// Literal values in the IR.
#[derive(Clone, Debug, PartialEq)]
pub enum LiteralValue {
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Str(String),
    Unit,
}

impl Serialize for LiteralValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            LiteralValue::Integer(v) => serializer.serialize_i64(*v),
            LiteralValue::Float(v) => serializer.serialize_f64(*v),
            LiteralValue::Boolean(v) => serializer.serialize_bool(*v),
            LiteralValue::Str(v) => serializer.serialize_str(v),
            LiteralValue::Unit => serializer.serialize_none(),
        }
    }
}

impl<'de> Deserialize<'de> for LiteralValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val = serde_json::Value::deserialize(deserializer)?;
        Ok(literal_from_json_value(&val))
    }
}

fn literal_from_json_value(val: &serde_json::Value) -> LiteralValue {
    match val {
        serde_json::Value::Bool(b) => LiteralValue::Boolean(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                LiteralValue::Integer(i)
            } else if let Some(f) = n.as_f64() {
                LiteralValue::Float(f)
            } else {
                LiteralValue::Integer(0)
            }
        }
        serde_json::Value::String(s) => LiteralValue::Str(s.clone()),
        serde_json::Value::Null => LiteralValue::Unit,
        _ => LiteralValue::Unit,
    }
}

/// Binary operators.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BinOpKind {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Neq,
    Lt,
    Lte,
    Gt,
    Gte,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

/// Unary operators.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UnaryOpKind {
    Neg,
    Not,
    BitNot,
}

/// A pattern in a match arm.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Pattern {
    Literal { value: LiteralValue },
    Wildcard,
    Variable { name: String },
}

/// A match arm: pattern -> body.
#[derive(Clone, Debug, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Node,
}

// ---------------------------------------------------------------------------
// Node - the core IR node type
// ---------------------------------------------------------------------------

/// The core IR node enum. Each variant represents a different kind of
/// computation in the AIRL intermediate representation.
///
/// Nodes are serialized to/from JSON with a "kind" discriminator field.
#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Literal {
        id: NodeId,
        node_type: Type,
        value: LiteralValue,
    },
    Param {
        id: NodeId,
        name: String,
        index: u32,
        node_type: Type,
    },
    Let {
        id: NodeId,
        name: String,
        node_type: Type,
        value: Box<Node>,
        body: Box<Node>,
    },
    If {
        id: NodeId,
        node_type: Type,
        cond: Box<Node>,
        then_branch: Box<Node>,
        else_branch: Box<Node>,
    },
    Call {
        id: NodeId,
        node_type: Type,
        target: String,
        args: Vec<Node>,
    },
    Return {
        id: NodeId,
        node_type: Type,
        value: Box<Node>,
    },
    BinOp {
        id: NodeId,
        op: BinOpKind,
        node_type: Type,
        lhs: Box<Node>,
        rhs: Box<Node>,
    },
    UnaryOp {
        id: NodeId,
        op: UnaryOpKind,
        node_type: Type,
        operand: Box<Node>,
    },
    Block {
        id: NodeId,
        node_type: Type,
        statements: Vec<Node>,
        result: Box<Node>,
    },
    Loop {
        id: NodeId,
        node_type: Type,
        body: Box<Node>,
    },
    Match {
        id: NodeId,
        node_type: Type,
        scrutinee: Box<Node>,
        arms: Vec<MatchArm>,
    },
    StructLiteral {
        id: NodeId,
        node_type: Type,
        fields: Vec<(String, Node)>,
    },
    FieldAccess {
        id: NodeId,
        node_type: Type,
        object: Box<Node>,
        field: String,
    },
    ArrayLiteral {
        id: NodeId,
        node_type: Type,
        elements: Vec<Node>,
    },
    IndexAccess {
        id: NodeId,
        node_type: Type,
        array: Box<Node>,
        index: Box<Node>,
    },
    Error {
        id: NodeId,
        message: String,
    },
}

impl Node {
    /// Get the NodeId of this node.
    pub fn id(&self) -> &NodeId {
        match self {
            Node::Literal { id, .. }
            | Node::Param { id, .. }
            | Node::Let { id, .. }
            | Node::If { id, .. }
            | Node::Call { id, .. }
            | Node::Return { id, .. }
            | Node::BinOp { id, .. }
            | Node::UnaryOp { id, .. }
            | Node::Block { id, .. }
            | Node::Loop { id, .. }
            | Node::Match { id, .. }
            | Node::StructLiteral { id, .. }
            | Node::FieldAccess { id, .. }
            | Node::ArrayLiteral { id, .. }
            | Node::IndexAccess { id, .. }
            | Node::Error { id, .. } => id,
        }
    }

    /// Get the type of this node (if it has one).
    pub fn node_type(&self) -> Option<&Type> {
        match self {
            Node::Literal { node_type, .. }
            | Node::Param { node_type, .. }
            | Node::Let { node_type, .. }
            | Node::If { node_type, .. }
            | Node::Call { node_type, .. }
            | Node::Return { node_type, .. }
            | Node::BinOp { node_type, .. }
            | Node::UnaryOp { node_type, .. }
            | Node::Block { node_type, .. }
            | Node::Loop { node_type, .. }
            | Node::Match { node_type, .. }
            | Node::StructLiteral { node_type, .. }
            | Node::FieldAccess { node_type, .. }
            | Node::ArrayLiteral { node_type, .. }
            | Node::IndexAccess { node_type, .. } => Some(node_type),
            Node::Error { .. } => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Custom Serde for Node
// ---------------------------------------------------------------------------

// We use a flat JSON representation with a "kind" field as discriminator.
// The "type" field is a string that gets parsed via Type::from_type_str.

impl Serialize for Node {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeMap;

        match self {
            Node::Literal {
                id,
                node_type,
                value,
            } => {
                let mut map = serializer.serialize_map(None)?;
                map.serialize_entry("id", &id)?;
                map.serialize_entry("kind", "Literal")?;
                map.serialize_entry("type", &node_type.to_type_str())?;
                map.serialize_entry("value", value)?;
                map.end()
            }
            Node::Param {
                id,
                name,
                index,
                node_type,
            } => {
                let mut map = serializer.serialize_map(None)?;
                map.serialize_entry("id", &id)?;
                map.serialize_entry("kind", "Param")?;
                map.serialize_entry("type", &node_type.to_type_str())?;
                map.serialize_entry("name", name)?;
                map.serialize_entry("index", index)?;
                map.end()
            }
            Node::Let {
                id,
                name,
                node_type,
                value,
                body,
            } => {
                let mut map = serializer.serialize_map(None)?;
                map.serialize_entry("id", &id)?;
                map.serialize_entry("kind", "Let")?;
                map.serialize_entry("type", &node_type.to_type_str())?;
                map.serialize_entry("name", name)?;
                map.serialize_entry("value", &**value)?;
                map.serialize_entry("body", &**body)?;
                map.end()
            }
            Node::If {
                id,
                node_type,
                cond,
                then_branch,
                else_branch,
            } => {
                let mut map = serializer.serialize_map(None)?;
                map.serialize_entry("id", &id)?;
                map.serialize_entry("kind", "If")?;
                map.serialize_entry("type", &node_type.to_type_str())?;
                map.serialize_entry("cond", &**cond)?;
                map.serialize_entry("then_branch", &**then_branch)?;
                map.serialize_entry("else_branch", &**else_branch)?;
                map.end()
            }
            Node::Call {
                id,
                node_type,
                target,
                args,
            } => {
                let mut map = serializer.serialize_map(None)?;
                map.serialize_entry("id", &id)?;
                map.serialize_entry("kind", "Call")?;
                map.serialize_entry("type", &node_type.to_type_str())?;
                map.serialize_entry("target", target)?;
                map.serialize_entry("args", args)?;
                map.end()
            }
            Node::Return {
                id,
                node_type,
                value,
            } => {
                let mut map = serializer.serialize_map(None)?;
                map.serialize_entry("id", &id)?;
                map.serialize_entry("kind", "Return")?;
                map.serialize_entry("type", &node_type.to_type_str())?;
                map.serialize_entry("value", &**value)?;
                map.end()
            }
            Node::BinOp {
                id,
                op,
                node_type,
                lhs,
                rhs,
            } => {
                let mut map = serializer.serialize_map(None)?;
                map.serialize_entry("id", &id)?;
                map.serialize_entry("kind", "BinOp")?;
                map.serialize_entry("type", &node_type.to_type_str())?;
                map.serialize_entry("op", op)?;
                map.serialize_entry("lhs", &**lhs)?;
                map.serialize_entry("rhs", &**rhs)?;
                map.end()
            }
            Node::UnaryOp {
                id,
                op,
                node_type,
                operand,
            } => {
                let mut map = serializer.serialize_map(None)?;
                map.serialize_entry("id", &id)?;
                map.serialize_entry("kind", "UnaryOp")?;
                map.serialize_entry("type", &node_type.to_type_str())?;
                map.serialize_entry("op", op)?;
                map.serialize_entry("operand", &**operand)?;
                map.end()
            }
            Node::Block {
                id,
                node_type,
                statements,
                result,
            } => {
                let mut map = serializer.serialize_map(None)?;
                map.serialize_entry("id", &id)?;
                map.serialize_entry("kind", "Block")?;
                map.serialize_entry("type", &node_type.to_type_str())?;
                map.serialize_entry("statements", statements)?;
                map.serialize_entry("result", &**result)?;
                map.end()
            }
            Node::Loop {
                id,
                node_type,
                body,
            } => {
                let mut map = serializer.serialize_map(None)?;
                map.serialize_entry("id", &id)?;
                map.serialize_entry("kind", "Loop")?;
                map.serialize_entry("type", &node_type.to_type_str())?;
                map.serialize_entry("body", &**body)?;
                map.end()
            }
            Node::Match {
                id,
                node_type,
                scrutinee,
                arms,
            } => {
                let mut map = serializer.serialize_map(None)?;
                map.serialize_entry("id", &id)?;
                map.serialize_entry("kind", "Match")?;
                map.serialize_entry("type", &node_type.to_type_str())?;
                map.serialize_entry("scrutinee", &**scrutinee)?;
                // Serialize arms as array of objects
                let arm_values: Vec<serde_json::Value> = arms
                    .iter()
                    .map(|arm| {
                        let body_val = serde_json::to_value(&arm.body).unwrap_or_default();
                        let pattern_val = serde_json::to_value(&arm.pattern).unwrap_or_default();
                        serde_json::json!({
                            "pattern": pattern_val,
                            "body": body_val,
                        })
                    })
                    .collect();
                map.serialize_entry("arms", &arm_values)?;
                map.end()
            }
            Node::StructLiteral {
                id,
                node_type,
                fields,
            } => {
                let mut map = serializer.serialize_map(None)?;
                map.serialize_entry("id", &id)?;
                map.serialize_entry("kind", "StructLiteral")?;
                map.serialize_entry("type", &node_type.to_type_str())?;
                // Serialize fields as array of {name, value} objects
                let field_values: Vec<serde_json::Value> = fields
                    .iter()
                    .map(|(name, node)| {
                        let node_val = serde_json::to_value(node).unwrap_or_default();
                        serde_json::json!({
                            "name": name,
                            "value": node_val,
                        })
                    })
                    .collect();
                map.serialize_entry("fields", &field_values)?;
                map.end()
            }
            Node::FieldAccess {
                id,
                node_type,
                object,
                field,
            } => {
                let mut map = serializer.serialize_map(None)?;
                map.serialize_entry("id", &id)?;
                map.serialize_entry("kind", "FieldAccess")?;
                map.serialize_entry("type", &node_type.to_type_str())?;
                map.serialize_entry("object", &**object)?;
                map.serialize_entry("field", field)?;
                map.end()
            }
            Node::ArrayLiteral {
                id,
                node_type,
                elements,
            } => {
                let mut map = serializer.serialize_map(None)?;
                map.serialize_entry("id", &id)?;
                map.serialize_entry("kind", "ArrayLiteral")?;
                map.serialize_entry("type", &node_type.to_type_str())?;
                map.serialize_entry("elements", elements)?;
                map.end()
            }
            Node::IndexAccess {
                id,
                node_type,
                array,
                index,
            } => {
                let mut map = serializer.serialize_map(None)?;
                map.serialize_entry("id", &id)?;
                map.serialize_entry("kind", "IndexAccess")?;
                map.serialize_entry("type", &node_type.to_type_str())?;
                map.serialize_entry("array", &**array)?;
                map.serialize_entry("index", &**index)?;
                map.end()
            }
            Node::Error { id, message } => {
                let mut map = serializer.serialize_map(None)?;
                map.serialize_entry("id", &id)?;
                map.serialize_entry("kind", "Error")?;
                map.serialize_entry("message", message)?;
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for Node {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val = serde_json::Value::deserialize(deserializer)?;
        node_from_value(&val).map_err(serde::de::Error::custom)
    }
}

/// Deserialize a Node from a serde_json::Value.
fn node_from_value(val: &serde_json::Value) -> Result<Node, String> {
    let obj = val.as_object().ok_or("Node must be a JSON object")?;

    let id = obj
        .get("id")
        .and_then(|v| v.as_str())
        .map(NodeId::new)
        .ok_or("Node missing 'id' field")?;

    let kind = obj
        .get("kind")
        .and_then(|v| v.as_str())
        .ok_or("Node missing 'kind' field")?;

    let node_type = obj
        .get("type")
        .and_then(|v| v.as_str())
        .map(Type::from_type_str)
        .unwrap_or(Type::Unit);

    match kind {
        "Literal" => {
            let value = obj
                .get("value")
                .map(literal_from_json_value)
                .unwrap_or(LiteralValue::Unit);
            Ok(Node::Literal {
                id,
                node_type,
                value,
            })
        }
        "Param" => {
            let name = obj
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let index = obj.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            Ok(Node::Param {
                id,
                name,
                index,
                node_type,
            })
        }
        "Let" => {
            let name = obj
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let value = obj.get("value").ok_or("Let missing 'value'")?;
            let body = obj.get("body").ok_or("Let missing 'body'")?;
            Ok(Node::Let {
                id,
                name,
                node_type,
                value: Box::new(node_from_value(value)?),
                body: Box::new(node_from_value(body)?),
            })
        }
        "If" => {
            let cond = obj.get("cond").ok_or("If missing 'cond'")?;
            let then_branch = obj.get("then_branch").ok_or("If missing 'then_branch'")?;
            let else_branch = obj.get("else_branch").ok_or("If missing 'else_branch'")?;
            Ok(Node::If {
                id,
                node_type,
                cond: Box::new(node_from_value(cond)?),
                then_branch: Box::new(node_from_value(then_branch)?),
                else_branch: Box::new(node_from_value(else_branch)?),
            })
        }
        "Call" => {
            let target = obj
                .get("target")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let args = obj
                .get("args")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .map(node_from_value)
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?
                .unwrap_or_default();
            Ok(Node::Call {
                id,
                node_type,
                target,
                args,
            })
        }
        "Return" => {
            let value = obj.get("value").ok_or("Return missing 'value'")?;
            Ok(Node::Return {
                id,
                node_type,
                value: Box::new(node_from_value(value)?),
            })
        }
        "BinOp" => {
            let op_str = obj
                .get("op")
                .and_then(|v| v.as_str())
                .ok_or("BinOp missing 'op'")?;
            let op: BinOpKind = serde_json::from_value(serde_json::Value::String(op_str.into()))
                .map_err(|e| format!("Invalid BinOp op: {e}"))?;
            let lhs = obj.get("lhs").ok_or("BinOp missing 'lhs'")?;
            let rhs = obj.get("rhs").ok_or("BinOp missing 'rhs'")?;
            Ok(Node::BinOp {
                id,
                op,
                node_type,
                lhs: Box::new(node_from_value(lhs)?),
                rhs: Box::new(node_from_value(rhs)?),
            })
        }
        "UnaryOp" => {
            let op_str = obj
                .get("op")
                .and_then(|v| v.as_str())
                .ok_or("UnaryOp missing 'op'")?;
            let op: UnaryOpKind = serde_json::from_value(serde_json::Value::String(op_str.into()))
                .map_err(|e| format!("Invalid UnaryOp op: {e}"))?;
            let operand = obj.get("operand").ok_or("UnaryOp missing 'operand'")?;
            Ok(Node::UnaryOp {
                id,
                op,
                node_type,
                operand: Box::new(node_from_value(operand)?),
            })
        }
        "Block" => {
            let statements = obj
                .get("statements")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .map(node_from_value)
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?
                .unwrap_or_default();
            let result = obj.get("result").ok_or("Block missing 'result'")?;
            Ok(Node::Block {
                id,
                node_type,
                statements,
                result: Box::new(node_from_value(result)?),
            })
        }
        "Loop" => {
            let body = obj.get("body").ok_or("Loop missing 'body'")?;
            Ok(Node::Loop {
                id,
                node_type,
                body: Box::new(node_from_value(body)?),
            })
        }
        "Match" => {
            let scrutinee = obj.get("scrutinee").ok_or("Match missing 'scrutinee'")?;
            let arms = obj
                .get("arms")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .map(|arm_val| {
                            let arm_obj = arm_val.as_object().ok_or("Match arm must be object")?;
                            let pattern: Pattern = arm_obj
                                .get("pattern")
                                .map(|v| {
                                    serde_json::from_value(v.clone())
                                        .map_err(|e| format!("Invalid pattern: {e}"))
                                })
                                .transpose()?
                                .unwrap_or(Pattern::Wildcard);
                            let body = arm_obj.get("body").ok_or("Match arm missing 'body'")?;
                            Ok(MatchArm {
                                pattern,
                                body: node_from_value(body)?,
                            })
                        })
                        .collect::<Result<Vec<_>, String>>()
                })
                .transpose()?
                .unwrap_or_default();
            Ok(Node::Match {
                id,
                node_type,
                scrutinee: Box::new(node_from_value(scrutinee)?),
                arms,
            })
        }
        "StructLiteral" => {
            let fields = obj
                .get("fields")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .map(|field_val| {
                            let field_obj = field_val
                                .as_object()
                                .ok_or("StructLiteral field must be object")?;
                            let name = field_obj
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let value = field_obj
                                .get("value")
                                .ok_or("StructLiteral field missing 'value'")?;
                            Ok((name, node_from_value(value)?))
                        })
                        .collect::<Result<Vec<_>, String>>()
                })
                .transpose()?
                .unwrap_or_default();
            Ok(Node::StructLiteral {
                id,
                node_type,
                fields,
            })
        }
        "FieldAccess" => {
            let object = obj.get("object").ok_or("FieldAccess missing 'object'")?;
            let field = obj
                .get("field")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Ok(Node::FieldAccess {
                id,
                node_type,
                object: Box::new(node_from_value(object)?),
                field,
            })
        }
        "ArrayLiteral" => {
            let elements = obj
                .get("elements")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .map(node_from_value)
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?
                .unwrap_or_default();
            Ok(Node::ArrayLiteral {
                id,
                node_type,
                elements,
            })
        }
        "IndexAccess" => {
            let array = obj.get("array").ok_or("IndexAccess missing 'array'")?;
            let index = obj.get("index").ok_or("IndexAccess missing 'index'")?;
            Ok(Node::IndexAccess {
                id,
                node_type,
                array: Box::new(node_from_value(array)?),
                index: Box::new(node_from_value(index)?),
            })
        }
        "Error" => {
            let message = obj
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Ok(Node::Error { id, message })
        }
        other => Err(format!("Unknown node kind: {other}")),
    }
}

impl Serialize for MatchArm {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("pattern", &self.pattern)?;
        map.serialize_entry("body", &self.body)?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for MatchArm {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val = serde_json::Value::deserialize(deserializer)?;
        let obj = val
            .as_object()
            .ok_or_else(|| serde::de::Error::custom("MatchArm must be a JSON object"))?;
        let pattern: Pattern = obj
            .get("pattern")
            .map(|v| serde_json::from_value(v.clone()).map_err(serde::de::Error::custom))
            .transpose()?
            .unwrap_or(Pattern::Wildcard);
        let body = obj
            .get("body")
            .ok_or_else(|| serde::de::Error::custom("MatchArm missing 'body'"))?;
        let body = node_from_value(body).map_err(serde::de::Error::custom)?;
        Ok(MatchArm { pattern, body })
    }
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Node::Literal { value, .. } => write!(f, "{value:?}"),
            Node::Param { name, .. } => write!(f, "param:{name}"),
            Node::Let { name, .. } => write!(f, "let {name}"),
            Node::If { .. } => write!(f, "if"),
            Node::Call { target, .. } => write!(f, "call {target}"),
            Node::Return { .. } => write!(f, "return"),
            Node::BinOp { op, .. } => write!(f, "binop {op:?}"),
            Node::UnaryOp { op, .. } => write!(f, "unaryop {op:?}"),
            Node::Block { .. } => write!(f, "block"),
            Node::Loop { .. } => write!(f, "loop"),
            Node::Match { .. } => write!(f, "match"),
            Node::StructLiteral { .. } => write!(f, "struct literal"),
            Node::FieldAccess { field, .. } => write!(f, ".{field}"),
            Node::ArrayLiteral { .. } => write!(f, "array literal"),
            Node::IndexAccess { .. } => write!(f, "index access"),
            Node::Error { message, .. } => write!(f, "error: {message}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::TypeId;

    #[test]
    fn test_literal_node_roundtrip() {
        let node = Node::Literal {
            id: NodeId::new("n_1"),
            node_type: Type::I64,
            value: LiteralValue::Integer(42),
        };
        let json = serde_json::to_string(&node).unwrap();
        let parsed: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(node, parsed);
    }

    #[test]
    fn test_call_node_roundtrip() {
        let node = Node::Call {
            id: NodeId::new("n_100"),
            node_type: Type::Unit,
            target: "std::io::println".to_string(),
            args: vec![Node::Literal {
                id: NodeId::new("n_101"),
                node_type: Type::String,
                value: LiteralValue::Str("hello world".to_string()),
            }],
        };
        let json = serde_json::to_string_pretty(&node).unwrap();
        let parsed: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(node, parsed);
    }

    #[test]
    fn test_binop_node_roundtrip() {
        let node = Node::BinOp {
            id: NodeId::new("n_5"),
            op: BinOpKind::Add,
            node_type: Type::I64,
            lhs: Box::new(Node::Literal {
                id: NodeId::new("n_6"),
                node_type: Type::I64,
                value: LiteralValue::Integer(1),
            }),
            rhs: Box::new(Node::Literal {
                id: NodeId::new("n_7"),
                node_type: Type::I64,
                value: LiteralValue::Integer(2),
            }),
        };
        let json = serde_json::to_string(&node).unwrap();
        let parsed: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(node, parsed);
    }

    #[test]
    fn test_param_roundtrip() {
        let node = Node::Param {
            id: NodeId::new("n_1"),
            name: "x".to_string(),
            index: 0,
            node_type: Type::I64,
        };
        let json = serde_json::to_string(&node).unwrap();
        let parsed: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(node, parsed);
    }

    #[test]
    fn test_let_roundtrip() {
        let node = Node::Let {
            id: NodeId::new("n_1"),
            name: "x".to_string(),
            node_type: Type::I64,
            value: Box::new(Node::Literal {
                id: NodeId::new("n_2"),
                node_type: Type::I64,
                value: LiteralValue::Integer(42),
            }),
            body: Box::new(Node::Param {
                id: NodeId::new("n_3"),
                name: "x".to_string(),
                index: 0,
                node_type: Type::I64,
            }),
        };
        let json = serde_json::to_string(&node).unwrap();
        let parsed: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(node, parsed);
    }

    #[test]
    fn test_if_roundtrip() {
        let node = Node::If {
            id: NodeId::new("n_1"),
            node_type: Type::I64,
            cond: Box::new(Node::Literal {
                id: NodeId::new("n_2"),
                node_type: Type::Bool,
                value: LiteralValue::Boolean(true),
            }),
            then_branch: Box::new(Node::Literal {
                id: NodeId::new("n_3"),
                node_type: Type::I64,
                value: LiteralValue::Integer(1),
            }),
            else_branch: Box::new(Node::Literal {
                id: NodeId::new("n_4"),
                node_type: Type::I64,
                value: LiteralValue::Integer(0),
            }),
        };
        let json = serde_json::to_string(&node).unwrap();
        let parsed: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(node, parsed);
    }

    #[test]
    fn test_return_roundtrip() {
        let node = Node::Return {
            id: NodeId::new("n_1"),
            node_type: Type::I64,
            value: Box::new(Node::Literal {
                id: NodeId::new("n_2"),
                node_type: Type::I64,
                value: LiteralValue::Integer(42),
            }),
        };
        let json = serde_json::to_string(&node).unwrap();
        let parsed: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(node, parsed);
    }

    #[test]
    fn test_unaryop_roundtrip() {
        let node = Node::UnaryOp {
            id: NodeId::new("n_1"),
            op: UnaryOpKind::Neg,
            node_type: Type::I64,
            operand: Box::new(Node::Literal {
                id: NodeId::new("n_2"),
                node_type: Type::I64,
                value: LiteralValue::Integer(5),
            }),
        };
        let json = serde_json::to_string(&node).unwrap();
        let parsed: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(node, parsed);
    }

    #[test]
    fn test_block_roundtrip() {
        let node = Node::Block {
            id: NodeId::new("n_1"),
            node_type: Type::I64,
            statements: vec![Node::Literal {
                id: NodeId::new("n_2"),
                node_type: Type::Unit,
                value: LiteralValue::Unit,
            }],
            result: Box::new(Node::Literal {
                id: NodeId::new("n_3"),
                node_type: Type::I64,
                value: LiteralValue::Integer(42),
            }),
        };
        let json = serde_json::to_string(&node).unwrap();
        let parsed: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(node, parsed);
    }

    #[test]
    fn test_loop_roundtrip() {
        let node = Node::Loop {
            id: NodeId::new("n_1"),
            node_type: Type::Unit,
            body: Box::new(Node::Literal {
                id: NodeId::new("n_2"),
                node_type: Type::Unit,
                value: LiteralValue::Unit,
            }),
        };
        let json = serde_json::to_string(&node).unwrap();
        let parsed: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(node, parsed);
    }

    #[test]
    fn test_match_roundtrip() {
        let node = Node::Match {
            id: NodeId::new("n_1"),
            node_type: Type::String,
            scrutinee: Box::new(Node::Literal {
                id: NodeId::new("n_2"),
                node_type: Type::I64,
                value: LiteralValue::Integer(1),
            }),
            arms: vec![
                MatchArm {
                    pattern: Pattern::Literal {
                        value: LiteralValue::Integer(1),
                    },
                    body: Node::Literal {
                        id: NodeId::new("n_3"),
                        node_type: Type::String,
                        value: LiteralValue::Str("one".to_string()),
                    },
                },
                MatchArm {
                    pattern: Pattern::Wildcard,
                    body: Node::Literal {
                        id: NodeId::new("n_4"),
                        node_type: Type::String,
                        value: LiteralValue::Str("other".to_string()),
                    },
                },
            ],
        };
        let json = serde_json::to_string(&node).unwrap();
        let parsed: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(node, parsed);
    }

    #[test]
    fn test_struct_literal_roundtrip() {
        let node = Node::StructLiteral {
            id: NodeId::new("n_1"),
            node_type: Type::Named(TypeId::new("Point")),
            fields: vec![
                (
                    "x".to_string(),
                    Node::Literal {
                        id: NodeId::new("n_2"),
                        node_type: Type::F64,
                        value: LiteralValue::Float(1.0),
                    },
                ),
                (
                    "y".to_string(),
                    Node::Literal {
                        id: NodeId::new("n_3"),
                        node_type: Type::F64,
                        value: LiteralValue::Float(2.0),
                    },
                ),
            ],
        };
        let json = serde_json::to_string(&node).unwrap();
        let parsed: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(node, parsed);
    }

    #[test]
    fn test_field_access_roundtrip() {
        let node = Node::FieldAccess {
            id: NodeId::new("n_1"),
            node_type: Type::F64,
            object: Box::new(Node::Param {
                id: NodeId::new("n_2"),
                name: "point".to_string(),
                index: 0,
                node_type: Type::Named(TypeId::new("Point")),
            }),
            field: "x".to_string(),
        };
        let json = serde_json::to_string(&node).unwrap();
        let parsed: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(node, parsed);
    }

    #[test]
    fn test_array_literal_roundtrip() {
        let node = Node::ArrayLiteral {
            id: NodeId::new("n_1"),
            node_type: Type::Array {
                element: Box::new(Type::I64),
            },
            elements: vec![
                Node::Literal {
                    id: NodeId::new("n_2"),
                    node_type: Type::I64,
                    value: LiteralValue::Integer(1),
                },
                Node::Literal {
                    id: NodeId::new("n_3"),
                    node_type: Type::I64,
                    value: LiteralValue::Integer(2),
                },
            ],
        };
        let json = serde_json::to_string(&node).unwrap();
        let parsed: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(node, parsed);
    }

    #[test]
    fn test_index_access_roundtrip() {
        let node = Node::IndexAccess {
            id: NodeId::new("n_1"),
            node_type: Type::I64,
            array: Box::new(Node::Param {
                id: NodeId::new("n_2"),
                name: "arr".to_string(),
                index: 0,
                node_type: Type::Array {
                    element: Box::new(Type::I64),
                },
            }),
            index: Box::new(Node::Literal {
                id: NodeId::new("n_3"),
                node_type: Type::I64,
                value: LiteralValue::Integer(0),
            }),
        };
        let json = serde_json::to_string(&node).unwrap();
        let parsed: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(node, parsed);
    }

    #[test]
    fn test_nested_let_if_binop_roundtrip() {
        // Let x = 10 in (if x > 5 then x + 1 else x - 1)
        let node = Node::Let {
            id: NodeId::new("n_1"),
            name: "x".to_string(),
            node_type: Type::I64,
            value: Box::new(Node::Literal {
                id: NodeId::new("n_2"),
                node_type: Type::I64,
                value: LiteralValue::Integer(10),
            }),
            body: Box::new(Node::If {
                id: NodeId::new("n_3"),
                node_type: Type::I64,
                cond: Box::new(Node::BinOp {
                    id: NodeId::new("n_4"),
                    op: BinOpKind::Gt,
                    node_type: Type::Bool,
                    lhs: Box::new(Node::Param {
                        id: NodeId::new("n_5"),
                        name: "x".to_string(),
                        index: 0,
                        node_type: Type::I64,
                    }),
                    rhs: Box::new(Node::Literal {
                        id: NodeId::new("n_6"),
                        node_type: Type::I64,
                        value: LiteralValue::Integer(5),
                    }),
                }),
                then_branch: Box::new(Node::BinOp {
                    id: NodeId::new("n_7"),
                    op: BinOpKind::Add,
                    node_type: Type::I64,
                    lhs: Box::new(Node::Param {
                        id: NodeId::new("n_8"),
                        name: "x".to_string(),
                        index: 0,
                        node_type: Type::I64,
                    }),
                    rhs: Box::new(Node::Literal {
                        id: NodeId::new("n_9"),
                        node_type: Type::I64,
                        value: LiteralValue::Integer(1),
                    }),
                }),
                else_branch: Box::new(Node::BinOp {
                    id: NodeId::new("n_10"),
                    op: BinOpKind::Sub,
                    node_type: Type::I64,
                    lhs: Box::new(Node::Param {
                        id: NodeId::new("n_11"),
                        name: "x".to_string(),
                        index: 0,
                        node_type: Type::I64,
                    }),
                    rhs: Box::new(Node::Literal {
                        id: NodeId::new("n_12"),
                        node_type: Type::I64,
                        value: LiteralValue::Integer(1),
                    }),
                }),
            }),
        };
        let json = serde_json::to_string(&node).unwrap();
        let parsed: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(node, parsed);
    }

    #[test]
    fn test_error_node_roundtrip() {
        let node = Node::Error {
            id: NodeId::new("n_1"),
            message: "something went wrong".to_string(),
        };
        let json = serde_json::to_string(&node).unwrap();
        let parsed: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(node, parsed);
    }

    #[test]
    fn test_deserialize_from_spec_json() {
        let json = r#"{
            "id": "n_100",
            "kind": "Call",
            "type": "Unit",
            "target": "std::io::println",
            "args": [
                {
                    "id": "n_101",
                    "kind": "Literal",
                    "type": "String",
                    "value": "hello world"
                }
            ]
        }"#;
        let node: Node = serde_json::from_str(json).unwrap();
        match &node {
            Node::Call {
                id,
                target,
                args,
                node_type,
            } => {
                assert_eq!(id.as_str(), "n_100");
                assert_eq!(target, "std::io::println");
                assert_eq!(*node_type, Type::Unit);
                assert_eq!(args.len(), 1);
            }
            _ => panic!("Expected Call node"),
        }
    }
}
