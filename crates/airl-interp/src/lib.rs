//! AIRL Interpreter - Tree-walking interpreter for AIRL IR programs.
//!
//! Evaluates IR graphs directly for fast feedback during development.
//! Supports all node types, user-defined function calls with recursion,
//! and configurable execution limits.

use airl_ir::node::{BinOpKind, LiteralValue, Node, Pattern, UnaryOpKind};
use airl_ir::Module;
use std::collections::BTreeMap;
use thiserror::Error;

/// Errors that can occur during interpretation.
#[derive(Debug, Error)]
pub enum InterpreterError {
    #[error("no 'main' function found")]
    NoMainFunction,
    #[error("unknown function: {0}")]
    UnknownFunction(String),
    #[error("type error at node {node_id}: {message}")]
    TypeError { node_id: String, message: String },
    #[error("unsupported node at {node_id}: {kind}")]
    Unsupported { node_id: String, kind: String },
    #[error("division by zero at node {0}")]
    DivisionByZero(String),
    #[error("step limit exceeded ({0} steps)")]
    StepLimitExceeded(u64),
    #[error("call depth limit exceeded ({0} frames)")]
    CallDepthExceeded(u32),
    #[error("no matching arm in match at node {0}")]
    NoMatchingArm(String),
    #[error("index out of bounds at node {node_id}: index {index}, length {length}")]
    IndexOutOfBounds {
        node_id: String,
        index: i64,
        length: usize,
    },
    #[error("field not found at node {node_id}: {field}")]
    FieldNotFound { node_id: String, field: String },
    #[error("break from loop")]
    LoopBreak(Value),
}

/// The result of interpreting a program.
#[derive(Debug, Clone)]
pub struct InterpreterOutput {
    pub stdout: String,
    pub exit_code: i32,
}

/// Configurable execution limits.
#[derive(Debug, Clone)]
pub struct ExecutionLimits {
    pub max_steps: u64,
    pub max_call_depth: u32,
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        Self {
            max_steps: 1_000_000,
            max_call_depth: 1000,
        }
    }
}

/// Runtime values during interpretation.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Str(String),
    Unit,
    Array(Vec<Value>),
    Struct(BTreeMap<String, Value>),
    Map(BTreeMap<String, Value>),
}

impl Value {
    fn as_integer(&self) -> Option<i64> {
        if let Value::Integer(i) = self {
            Some(*i)
        } else {
            None
        }
    }

    fn as_float(&self) -> Option<f64> {
        if let Value::Float(f) = self {
            Some(*f)
        } else {
            None
        }
    }

    fn as_boolean(&self) -> Option<bool> {
        if let Value::Boolean(b) = self {
            Some(*b)
        } else {
            None
        }
    }

    fn to_display_string(&self) -> String {
        match self {
            Value::Integer(i) => i.to_string(),
            Value::Float(f) => {
                // Format without trailing zeros for clean output
                if *f == (*f as i64) as f64 && f.is_finite() {
                    format!("{f:.1}")
                } else {
                    f.to_string()
                }
            }
            Value::Boolean(b) => b.to_string(),
            Value::Str(s) => s.clone(),
            Value::Unit => "()".to_string(),
            Value::Array(items) => {
                let inner: Vec<String> = items.iter().map(|v| v.to_display_string()).collect();
                format!("[{}]", inner.join(", "))
            }
            Value::Struct(fields) => {
                let inner: Vec<String> = fields
                    .iter()
                    .map(|(k, v)| format!("{k}: {}", v.to_display_string()))
                    .collect();
                format!("{{{}}}", inner.join(", "))
            }
            Value::Map(entries) => {
                let inner: Vec<String> = entries
                    .iter()
                    .map(|(k, v)| format!("{k}: {}", v.to_display_string()))
                    .collect();
                format!("{{{}}}", inner.join(", "))
            }
        }
    }
}

/// Interpreter state.
struct Interpreter<'a> {
    module: &'a Module,
    stdout: String,
    locals: Vec<(String, Value)>,
    steps: u64,
    call_depth: u32,
    limits: ExecutionLimits,
}

impl<'a> Interpreter<'a> {
    fn new(module: &'a Module, limits: ExecutionLimits) -> Self {
        Self {
            module,
            stdout: String::new(),
            locals: Vec::new(),
            steps: 0,
            call_depth: 0,
            limits,
        }
    }

    fn eval(&mut self, node: &Node) -> Result<Value, InterpreterError> {
        self.steps += 1;
        if self.steps > self.limits.max_steps {
            return Err(InterpreterError::StepLimitExceeded(self.limits.max_steps));
        }

        match node {
            Node::Literal { value, .. } => Ok(Self::literal_to_value(value)),

            Node::Param { name, id, .. } => self.lookup_var(name, id),

            Node::Let {
                name, value, body, ..
            } => {
                let val = self.eval(value)?;
                self.locals.push((name.clone(), val));
                let result = self.eval(body)?;
                self.locals.pop();
                Ok(result)
            }

            Node::If {
                cond,
                then_branch,
                else_branch,
                id,
                ..
            } => {
                let cond_val = self.eval(cond)?;
                match cond_val.as_boolean() {
                    Some(true) => self.eval(then_branch),
                    Some(false) => self.eval(else_branch),
                    None => Err(InterpreterError::TypeError {
                        node_id: id.to_string(),
                        message: "if condition must be boolean".to_string(),
                    }),
                }
            }

            Node::Call {
                target, args, id, ..
            } => {
                let arg_values: Vec<Value> = args
                    .iter()
                    .map(|a| self.eval(a))
                    .collect::<Result<_, _>>()?;
                self.call_function(target, &arg_values, id)
            }

            Node::Return { value, .. } => self.eval(value),

            Node::BinOp {
                op, lhs, rhs, id, ..
            } => {
                let l = self.eval(lhs)?;
                let r = self.eval(rhs)?;
                self.eval_binop(op, &l, &r, id)
            }

            Node::UnaryOp {
                op, operand, id, ..
            } => {
                let val = self.eval(operand)?;
                self.eval_unaryop(op, &val, id)
            }

            Node::Block {
                statements, result, ..
            } => {
                for stmt in statements {
                    self.eval(stmt)?;
                }
                self.eval(result)
            }

            Node::Loop { body, id, .. } => {
                loop {
                    match self.eval(body) {
                        Ok(_) => {} // continue looping
                        Err(InterpreterError::LoopBreak(val)) => return Ok(val),
                        Err(InterpreterError::StepLimitExceeded(n)) => {
                            return Err(InterpreterError::StepLimitExceeded(n));
                        }
                        Err(e) => return Err(e),
                    }
                    // Safety: check step limit in case body doesn't have enough nodes
                    self.steps += 1;
                    if self.steps > self.limits.max_steps {
                        return Err(InterpreterError::StepLimitExceeded(self.limits.max_steps));
                    }
                    let _ = id; // suppress unused warning
                }
            }

            Node::Match {
                scrutinee,
                arms,
                id,
                ..
            } => {
                let scrutinee_val = self.eval(scrutinee)?;
                for arm in arms {
                    if self.pattern_matches(&arm.pattern, &scrutinee_val) {
                        // For variable patterns, bind the value
                        if let Pattern::Variable { name } = &arm.pattern {
                            self.locals.push((name.clone(), scrutinee_val));
                            let result = self.eval(&arm.body)?;
                            self.locals.pop();
                            return Ok(result);
                        }
                        return self.eval(&arm.body);
                    }
                }
                Err(InterpreterError::NoMatchingArm(id.to_string()))
            }

            Node::StructLiteral { fields, .. } => {
                let mut map = BTreeMap::new();
                for (name, node) in fields {
                    let val = self.eval(node)?;
                    map.insert(name.clone(), val);
                }
                Ok(Value::Struct(map))
            }

            Node::FieldAccess {
                object, field, id, ..
            } => {
                let obj = self.eval(object)?;
                match obj {
                    Value::Struct(fields) => fields
                        .get(field)
                        .cloned()
                        .ok_or(InterpreterError::FieldNotFound {
                            node_id: id.to_string(),
                            field: field.clone(),
                        }),
                    _ => Err(InterpreterError::TypeError {
                        node_id: id.to_string(),
                        message: "field access on non-struct value".to_string(),
                    }),
                }
            }

            Node::ArrayLiteral { elements, .. } => {
                let mut items = Vec::with_capacity(elements.len());
                for el in elements {
                    items.push(self.eval(el)?);
                }
                Ok(Value::Array(items))
            }

            Node::IndexAccess {
                array, index, id, ..
            } => {
                let arr = self.eval(array)?;
                let idx = self.eval(index)?;
                match (&arr, idx.as_integer()) {
                    (Value::Array(items), Some(i)) => {
                        if i >= 0 && (i as usize) < items.len() {
                            Ok(items[i as usize].clone())
                        } else {
                            Err(InterpreterError::IndexOutOfBounds {
                                node_id: id.to_string(),
                                index: i,
                                length: items.len(),
                            })
                        }
                    }
                    _ => Err(InterpreterError::TypeError {
                        node_id: id.to_string(),
                        message: "index access requires array and integer index".to_string(),
                    }),
                }
            }

            Node::Error { id, message } => Err(InterpreterError::TypeError {
                node_id: id.to_string(),
                message: format!("IR error node: {message}"),
            }),
        }
    }

    fn pattern_matches(&self, pattern: &Pattern, value: &Value) -> bool {
        match pattern {
            Pattern::Wildcard => true,
            Pattern::Variable { .. } => true,
            Pattern::Literal {
                value: pat_value, ..
            } => {
                let pat_val = Self::literal_to_value(pat_value);
                pat_val == *value
            }
        }
    }

    fn literal_to_value(lit: &LiteralValue) -> Value {
        match lit {
            LiteralValue::Integer(i) => Value::Integer(*i),
            LiteralValue::Float(f) => Value::Float(*f),
            LiteralValue::Boolean(b) => Value::Boolean(*b),
            LiteralValue::Str(s) => Value::Str(s.clone()),
            LiteralValue::Unit => Value::Unit,
        }
    }

    fn lookup_var(
        &self,
        name: &str,
        id: &airl_ir::NodeId,
    ) -> Result<Value, InterpreterError> {
        for (n, v) in self.locals.iter().rev() {
            if n == name {
                return Ok(v.clone());
            }
        }
        Err(InterpreterError::TypeError {
            node_id: id.to_string(),
            message: format!("undefined variable: {name}"),
        })
    }

    fn call_function(
        &mut self,
        target: &str,
        args: &[Value],
        id: &airl_ir::NodeId,
    ) -> Result<Value, InterpreterError> {
        // Try builtin first
        if let Some(result) = self.try_builtin(target, args, id)? {
            return Ok(result);
        }

        // Look up user-defined function in the module
        let func = self
            .module
            .find_function(target)
            .ok_or_else(|| InterpreterError::UnknownFunction(target.to_string()))?;

        // Check call depth
        self.call_depth += 1;
        if self.call_depth > self.limits.max_call_depth {
            return Err(InterpreterError::CallDepthExceeded(
                self.limits.max_call_depth,
            ));
        }

        // Save current locals, bind parameters
        let saved_locals = std::mem::take(&mut self.locals);
        for (i, param) in func.params.iter().enumerate() {
            let val = args.get(i).cloned().unwrap_or(Value::Unit);
            self.locals.push((param.name.clone(), val));
        }

        // Evaluate function body
        let result = self.eval(&func.body);

        // Restore locals and call depth
        self.locals = saved_locals;
        self.call_depth -= 1;

        result
    }

    /// Try to execute a builtin function. Returns None if not a builtin.
    fn try_builtin(
        &mut self,
        target: &str,
        args: &[Value],
        _id: &airl_ir::NodeId,
    ) -> Result<Option<Value>, InterpreterError> {
        let result = match target {
            // I/O
            "std::io::println" => {
                let text = args
                    .iter()
                    .map(|a| a.to_display_string())
                    .collect::<Vec<_>>()
                    .join(" ");
                self.stdout.push_str(&text);
                self.stdout.push('\n');
                Value::Unit
            }
            "std::io::print" => {
                let text = args
                    .iter()
                    .map(|a| a.to_display_string())
                    .collect::<Vec<_>>()
                    .join(" ");
                self.stdout.push_str(&text);
                Value::Unit
            }
            "std::io::eprintln" => {
                // Write to stderr buffer (captured as stdout for now)
                let text = args
                    .iter()
                    .map(|a| a.to_display_string())
                    .collect::<Vec<_>>()
                    .join(" ");
                self.stdout.push_str(&text);
                self.stdout.push('\n');
                Value::Unit
            }
            "std::io::read_line" => {
                // Stub: returns empty string (real stdin not available in interpreter)
                Value::Str(String::new())
            }

            // String operations
            "std::string::len" => match args.first() {
                Some(Value::Str(s)) => Value::Integer(s.len() as i64),
                _ => Value::Integer(0),
            },
            "std::string::concat" => {
                let result: String = args.iter().map(|a| a.to_display_string()).collect();
                Value::Str(result)
            }
            "std::string::contains" => match (args.first(), args.get(1)) {
                (Some(Value::Str(s)), Some(Value::Str(sub))) => Value::Boolean(s.contains(sub.as_str())),
                _ => Value::Boolean(false),
            },
            "std::string::split" => match (args.first(), args.get(1)) {
                (Some(Value::Str(s)), Some(Value::Str(sep))) => {
                    let parts: Vec<Value> = s.split(sep.as_str()).map(|p| Value::Str(p.to_string())).collect();
                    Value::Array(parts)
                }
                _ => Value::Array(vec![]),
            },
            "std::string::from_i64" => match args.first() {
                Some(Value::Integer(i)) => Value::Str(i.to_string()),
                Some(v) => Value::Str(v.to_display_string()),
                _ => Value::Str(String::new()),
            },
            "std::string::to_i64" => match args.first() {
                Some(Value::Str(s)) => match s.parse::<i64>() {
                    Ok(n) => Value::Integer(n),
                    Err(_) => Value::Unit,
                },
                _ => Value::Unit,
            },
            "std::string::starts_with" => match (args.first(), args.get(1)) {
                (Some(Value::Str(s)), Some(Value::Str(prefix))) => {
                    Value::Boolean(s.starts_with(prefix.as_str()))
                }
                _ => Value::Boolean(false),
            },
            "std::string::ends_with" => match (args.first(), args.get(1)) {
                (Some(Value::Str(s)), Some(Value::Str(suffix))) => {
                    Value::Boolean(s.ends_with(suffix.as_str()))
                }
                _ => Value::Boolean(false),
            },
            "std::string::trim" => match args.first() {
                Some(Value::Str(s)) => Value::Str(s.trim().to_string()),
                _ => Value::Str(String::new()),
            },
            "std::string::to_uppercase" => match args.first() {
                Some(Value::Str(s)) => Value::Str(s.to_uppercase()),
                _ => Value::Str(String::new()),
            },
            "std::string::to_lowercase" => match args.first() {
                Some(Value::Str(s)) => Value::Str(s.to_lowercase()),
                _ => Value::Str(String::new()),
            },
            "std::string::replace" => match (args.first(), args.get(1), args.get(2)) {
                (Some(Value::Str(s)), Some(Value::Str(from)), Some(Value::Str(to))) => {
                    Value::Str(s.replace(from.as_str(), to.as_str()))
                }
                _ => Value::Str(String::new()),
            },

            // Math
            "std::math::abs" => match args.first() {
                Some(Value::Integer(i)) => Value::Integer(i.abs()),
                Some(Value::Float(f)) => Value::Float(f.abs()),
                _ => Value::Integer(0),
            },
            "std::math::max" => match (args.first(), args.get(1)) {
                (Some(Value::Integer(a)), Some(Value::Integer(b))) => {
                    Value::Integer((*a).max(*b))
                }
                _ => Value::Integer(0),
            },
            "std::math::min" => match (args.first(), args.get(1)) {
                (Some(Value::Integer(a)), Some(Value::Integer(b))) => {
                    Value::Integer((*a).min(*b))
                }
                _ => Value::Integer(0),
            },
            "std::math::pow" => match (args.first(), args.get(1)) {
                (Some(Value::Integer(base)), Some(Value::Integer(exp))) => {
                    Value::Integer(base.wrapping_pow(*exp as u32))
                }
                _ => Value::Integer(0),
            },
            "std::math::sqrt" => match args.first() {
                Some(Value::Float(f)) => Value::Float(f.sqrt()),
                Some(Value::Integer(i)) => Value::Float((*i as f64).sqrt()),
                _ => Value::Float(0.0),
            },
            "std::math::floor" => match args.first() {
                Some(Value::Float(f)) => Value::Integer(f.floor() as i64),
                Some(Value::Integer(i)) => Value::Integer(*i),
                _ => Value::Integer(0),
            },
            "std::math::ceil" => match args.first() {
                Some(Value::Float(f)) => Value::Integer(f.ceil() as i64),
                Some(Value::Integer(i)) => Value::Integer(*i),
                _ => Value::Integer(0),
            },

            // Array operations
            "std::array::len" => match args.first() {
                Some(Value::Array(items)) => Value::Integer(items.len() as i64),
                _ => Value::Integer(0),
            },
            "std::array::push" => match (args.first(), args.get(1)) {
                (Some(Value::Array(items)), Some(val)) => {
                    let mut new_items = items.clone();
                    new_items.push(val.clone());
                    Value::Array(new_items)
                }
                _ => Value::Array(vec![]),
            },
            "std::array::get" => match (args.first(), args.get(1)) {
                (Some(Value::Array(items)), Some(Value::Integer(i))) => {
                    if *i >= 0 && (*i as usize) < items.len() {
                        items[*i as usize].clone()
                    } else {
                        Value::Unit
                    }
                }
                _ => Value::Unit,
            },
            "std::array::slice" => match (args.first(), args.get(1), args.get(2)) {
                (Some(Value::Array(items)), Some(Value::Integer(start)), Some(Value::Integer(end))) => {
                    let s = (*start).max(0) as usize;
                    let e = (*end).min(items.len() as i64) as usize;
                    if s <= e && e <= items.len() {
                        Value::Array(items[s..e].to_vec())
                    } else {
                        Value::Array(vec![])
                    }
                }
                _ => Value::Array(vec![]),
            },
            "std::array::contains" => match (args.first(), args.get(1)) {
                (Some(Value::Array(items)), Some(val)) => {
                    Value::Boolean(items.contains(val))
                }
                _ => Value::Boolean(false),
            },
            "std::array::reverse" => match args.first() {
                Some(Value::Array(items)) => {
                    let mut rev = items.clone();
                    rev.reverse();
                    Value::Array(rev)
                }
                _ => Value::Array(vec![]),
            },
            "std::array::join" => match (args.first(), args.get(1)) {
                (Some(Value::Array(items)), Some(Value::Str(sep))) => {
                    let strings: Vec<String> = items.iter().map(|v| v.to_display_string()).collect();
                    Value::Str(strings.join(sep.as_str()))
                }
                _ => Value::Str(String::new()),
            },
            "std::array::range" => match (args.first(), args.get(1)) {
                (Some(Value::Integer(start)), Some(Value::Integer(end))) => {
                    let items: Vec<Value> = (*start..*end).map(Value::Integer).collect();
                    Value::Array(items)
                }
                _ => Value::Array(vec![]),
            },

            // Formatting
            "std::fmt::format" => {
                if let Some(Value::Str(template)) = args.first() {
                    let mut result = template.clone();
                    for arg in &args[1..] {
                        if let Some(pos) = result.find("{}") {
                            result.replace_range(pos..pos + 2, &arg.to_display_string());
                        }
                    }
                    Value::Str(result)
                } else {
                    Value::Str(String::new())
                }
            }

            // Environment
            "std::env::args" => {
                // Stub: returns empty array
                Value::Array(vec![])
            }

            // JSON
            "std::json::parse" => match args.first() {
                Some(Value::Str(s)) => match serde_json::from_str::<serde_json::Value>(s) {
                    Ok(val) => json_to_value(&val),
                    Err(_) => Value::Unit,
                },
                _ => Value::Unit,
            },
            "std::json::serialize" => {
                let json_val = value_to_json(args.first().unwrap_or(&Value::Unit));
                Value::Str(json_val.to_string())
            }
            "std::json::serialize_pretty" => {
                let json_val = value_to_json(args.first().unwrap_or(&Value::Unit));
                Value::Str(serde_json::to_string_pretty(&json_val).unwrap_or_default())
            }

            // Collections (HashMap)
            "std::collections::new_map" => Value::Map(BTreeMap::new()),
            "std::collections::insert" => match (args.first(), args.get(1), args.get(2)) {
                (Some(Value::Map(map)), Some(Value::Str(key)), Some(val)) => {
                    let mut new_map = map.clone();
                    new_map.insert(key.clone(), val.clone());
                    Value::Map(new_map)
                }
                _ => Value::Unit,
            },
            "std::collections::get" => match (args.first(), args.get(1)) {
                (Some(Value::Map(map)), Some(Value::Str(key))) => {
                    map.get(key).cloned().unwrap_or(Value::Unit)
                }
                _ => Value::Unit,
            },
            "std::collections::remove" => match (args.first(), args.get(1)) {
                (Some(Value::Map(map)), Some(Value::Str(key))) => {
                    let mut new_map = map.clone();
                    new_map.remove(key);
                    Value::Map(new_map)
                }
                _ => Value::Unit,
            },
            "std::collections::contains_key" => match (args.first(), args.get(1)) {
                (Some(Value::Map(map)), Some(Value::Str(key))) => {
                    Value::Boolean(map.contains_key(key))
                }
                _ => Value::Boolean(false),
            },
            "std::collections::keys" => match args.first() {
                Some(Value::Map(map)) => {
                    Value::Array(map.keys().map(|k| Value::Str(k.clone())).collect())
                }
                _ => Value::Array(vec![]),
            },
            "std::collections::values" => match args.first() {
                Some(Value::Map(map)) => {
                    Value::Array(map.values().cloned().collect())
                }
                _ => Value::Array(vec![]),
            },
            "std::collections::map_len" => match args.first() {
                Some(Value::Map(map)) => Value::Integer(map.len() as i64),
                _ => Value::Integer(0),
            },

            // Error handling
            "std::error::is_unit" => match args.first() {
                Some(Value::Unit) => Value::Boolean(true),
                _ => Value::Boolean(false),
            },
            "std::error::unwrap_or" => match (args.first(), args.get(1)) {
                (Some(Value::Unit), Some(default)) => default.clone(),
                (Some(val), _) => val.clone(),
                _ => Value::Unit,
            },
            "std::error::assert" => match args.first() {
                Some(Value::Boolean(true)) => Value::Unit,
                Some(Value::Boolean(false)) => {
                    let msg = args.get(1)
                        .map(|v| v.to_display_string())
                        .unwrap_or_else(|| "assertion failed".to_string());
                    self.stdout.push_str(&format!("ASSERTION FAILED: {msg}\n"));
                    Value::Unit
                }
                _ => Value::Unit,
            },
            "std::error::panic" => {
                let msg = args.first()
                    .map(|v| v.to_display_string())
                    .unwrap_or_else(|| "panic".to_string());
                return Err(InterpreterError::TypeError {
                    node_id: _id.to_string(),
                    message: format!("panic: {msg}"),
                });
            }

            // Process
            "std::process::exit" => {
                let code = args.first()
                    .and_then(|v| v.as_integer())
                    .unwrap_or(0);
                std::process::exit(code as i32);
            }
            "std::process::exec" => match args.first() {
                Some(Value::Str(cmd)) => {
                    let output = std::process::Command::new(if cfg!(windows) { "cmd" } else { "sh" })
                        .args(if cfg!(windows) { vec!["/C", cmd] } else { vec!["-c", cmd] })
                        .output();
                    match output {
                        Ok(o) => {
                            let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                            let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                            let mut result = BTreeMap::new();
                            result.insert("stdout".to_string(), Value::Str(stdout));
                            result.insert("stderr".to_string(), Value::Str(stderr));
                            result.insert("code".to_string(), Value::Integer(o.status.code().unwrap_or(-1) as i64));
                            Value::Map(result)
                        }
                        Err(_) => Value::Unit,
                    }
                }
                _ => Value::Unit,
            },
            "std::process::env_var" => match args.first() {
                Some(Value::Str(name)) => match std::env::var(name) {
                    Ok(val) => Value::Str(val),
                    Err(_) => Value::Unit,
                },
                _ => Value::Unit,
            },
            "std::process::set_env_var" => match (args.first(), args.get(1)) {
                (Some(Value::Str(name)), Some(Value::Str(val))) => {
                    std::env::set_var(name, val);
                    Value::Unit
                }
                _ => Value::Unit,
            },

            // Time
            "std::time::now_ms" => {
                let ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0);
                Value::Integer(ms)
            }
            "std::time::now_secs" => {
                let secs = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                Value::Integer(secs)
            }
            "std::time::sleep_ms" => {
                if let Some(Value::Integer(ms)) = args.first() {
                    std::thread::sleep(std::time::Duration::from_millis(*ms as u64));
                }
                Value::Unit
            }

            // Crypto
            "std::crypto::sha256" => match args.first() {
                Some(Value::Str(s)) => {
                    let hash = simple_hash(s.as_bytes());
                    Value::Str(hash)
                }
                _ => Value::Str(String::new()),
            },

            // Testing framework
            "std::testing::assert_eq" => {
                let lhs = args.first().cloned().unwrap_or(Value::Unit);
                let rhs = args.get(1).cloned().unwrap_or(Value::Unit);
                if lhs == rhs {
                    Value::Boolean(true)
                } else {
                    let msg = format!(
                        "assertion failed: {} != {}",
                        lhs.to_display_string(),
                        rhs.to_display_string()
                    );
                    self.stdout.push_str(&format!("FAIL: {msg}\n"));
                    Value::Boolean(false)
                }
            }
            "std::testing::assert_ne" => {
                let lhs = args.first().cloned().unwrap_or(Value::Unit);
                let rhs = args.get(1).cloned().unwrap_or(Value::Unit);
                if lhs != rhs {
                    Value::Boolean(true)
                } else {
                    let msg = format!(
                        "assertion failed: values are equal: {}",
                        lhs.to_display_string()
                    );
                    self.stdout.push_str(&format!("FAIL: {msg}\n"));
                    Value::Boolean(false)
                }
            }
            "std::testing::assert_true" => match args.first() {
                Some(Value::Boolean(true)) => Value::Boolean(true),
                _ => {
                    self.stdout.push_str("FAIL: expected true\n");
                    Value::Boolean(false)
                }
            },

            // File I/O
            "std::io::read_file" => match args.first() {
                Some(Value::Str(path)) => match std::fs::read_to_string(path) {
                    Ok(contents) => Value::Str(contents),
                    Err(_) => Value::Unit,
                },
                _ => Value::Unit,
            },
            "std::io::write_file" => match (args.first(), args.get(1)) {
                (Some(Value::Str(path)), Some(Value::Str(contents))) => {
                    match std::fs::write(path, contents) {
                        Ok(()) => Value::Boolean(true),
                        Err(_) => Value::Boolean(false),
                    }
                }
                _ => Value::Boolean(false),
            },
            "std::io::read_dir" => match args.first() {
                Some(Value::Str(path)) => match std::fs::read_dir(path) {
                    Ok(entries) => {
                        let mut items = Vec::new();
                        for entry in entries.flatten() {
                            if let Some(name) = entry.file_name().to_str() {
                                items.push(Value::Str(name.to_string()));
                            }
                        }
                        Value::Array(items)
                    }
                    Err(_) => Value::Array(vec![]),
                },
                _ => Value::Array(vec![]),
            },
            "std::io::file_exists" => match args.first() {
                Some(Value::Str(path)) => Value::Boolean(std::path::Path::new(path).exists()),
                _ => Value::Boolean(false),
            },

            // Not a builtin
            _ => return Ok(None),
        };
        Ok(Some(result))
    }

    fn eval_binop(
        &self,
        op: &BinOpKind,
        lhs: &Value,
        rhs: &Value,
        id: &airl_ir::NodeId,
    ) -> Result<Value, InterpreterError> {
        // String concatenation: String + String
        if let (BinOpKind::Add, Value::Str(l), Value::Str(r)) = (op, lhs, rhs) {
            return Ok(Value::Str(format!("{l}{r}")));
        }

        // Integer operations
        if let (Some(l), Some(r)) = (lhs.as_integer(), rhs.as_integer()) {
            return match op {
                BinOpKind::Add => Ok(Value::Integer(l.wrapping_add(r))),
                BinOpKind::Sub => Ok(Value::Integer(l.wrapping_sub(r))),
                BinOpKind::Mul => Ok(Value::Integer(l.wrapping_mul(r))),
                BinOpKind::Div => {
                    if r == 0 {
                        Err(InterpreterError::DivisionByZero(id.to_string()))
                    } else {
                        Ok(Value::Integer(l / r))
                    }
                }
                BinOpKind::Mod => {
                    if r == 0 {
                        Err(InterpreterError::DivisionByZero(id.to_string()))
                    } else {
                        Ok(Value::Integer(l % r))
                    }
                }
                BinOpKind::Eq => Ok(Value::Boolean(l == r)),
                BinOpKind::Neq => Ok(Value::Boolean(l != r)),
                BinOpKind::Lt => Ok(Value::Boolean(l < r)),
                BinOpKind::Lte => Ok(Value::Boolean(l <= r)),
                BinOpKind::Gt => Ok(Value::Boolean(l > r)),
                BinOpKind::Gte => Ok(Value::Boolean(l >= r)),
                BinOpKind::BitAnd => Ok(Value::Integer(l & r)),
                BinOpKind::BitOr => Ok(Value::Integer(l | r)),
                BinOpKind::BitXor => Ok(Value::Integer(l ^ r)),
                BinOpKind::Shl => Ok(Value::Integer(l << r)),
                BinOpKind::Shr => Ok(Value::Integer(l >> r)),
                BinOpKind::And | BinOpKind::Or => Err(InterpreterError::TypeError {
                    node_id: id.to_string(),
                    message: "logical operators require boolean operands".to_string(),
                }),
            };
        }

        // Float operations
        if let (Some(l), Some(r)) = (lhs.as_float(), rhs.as_float()) {
            return match op {
                BinOpKind::Add => Ok(Value::Float(l + r)),
                BinOpKind::Sub => Ok(Value::Float(l - r)),
                BinOpKind::Mul => Ok(Value::Float(l * r)),
                BinOpKind::Div => Ok(Value::Float(l / r)),
                BinOpKind::Eq => Ok(Value::Boolean(l == r)),
                BinOpKind::Neq => Ok(Value::Boolean(l != r)),
                BinOpKind::Lt => Ok(Value::Boolean(l < r)),
                BinOpKind::Lte => Ok(Value::Boolean(l <= r)),
                BinOpKind::Gt => Ok(Value::Boolean(l > r)),
                BinOpKind::Gte => Ok(Value::Boolean(l >= r)),
                _ => Err(InterpreterError::TypeError {
                    node_id: id.to_string(),
                    message: format!("unsupported float operation: {op:?}"),
                }),
            };
        }

        // Boolean operations
        if let (Some(l), Some(r)) = (lhs.as_boolean(), rhs.as_boolean()) {
            return match op {
                BinOpKind::And => Ok(Value::Boolean(l && r)),
                BinOpKind::Or => Ok(Value::Boolean(l || r)),
                BinOpKind::Eq => Ok(Value::Boolean(l == r)),
                BinOpKind::Neq => Ok(Value::Boolean(l != r)),
                _ => Err(InterpreterError::TypeError {
                    node_id: id.to_string(),
                    message: format!("unsupported boolean operation: {op:?}"),
                }),
            };
        }

        Err(InterpreterError::TypeError {
            node_id: id.to_string(),
            message: format!("type mismatch in {op:?}: {} vs {}", lhs.to_display_string(), rhs.to_display_string()),
        })
    }

    fn eval_unaryop(
        &self,
        op: &UnaryOpKind,
        val: &Value,
        id: &airl_ir::NodeId,
    ) -> Result<Value, InterpreterError> {
        match (op, val) {
            (UnaryOpKind::Neg, Value::Integer(i)) => Ok(Value::Integer(-i)),
            (UnaryOpKind::Neg, Value::Float(f)) => Ok(Value::Float(-f)),
            (UnaryOpKind::Not, Value::Boolean(b)) => Ok(Value::Boolean(!b)),
            (UnaryOpKind::BitNot, Value::Integer(i)) => Ok(Value::Integer(!i)),
            _ => Err(InterpreterError::TypeError {
                node_id: id.to_string(),
                message: format!("unsupported unary operation: {op:?}"),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Crypto helper
// ---------------------------------------------------------------------------

/// Simple deterministic hash (FNV-1a-based hex digest, not cryptographic).
/// Used as a placeholder for std::crypto::sha256 without adding external deps.
fn simple_hash(data: &[u8]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    // Produce a 64-char hex string (pad with repeated hash)
    let h2 = hash.wrapping_mul(0x9e3779b97f4a7c15).wrapping_add(hash >> 3);
    let h3 = h2.wrapping_mul(0x517cc1b727220a95).wrapping_add(h2 >> 5);
    let h4 = h3.wrapping_mul(0x6c62272e07bb0142).wrapping_add(h3 >> 7);
    format!("{hash:016x}{h2:016x}{h3:016x}{h4:016x}")
}

// ---------------------------------------------------------------------------
// JSON conversion helpers
// ---------------------------------------------------------------------------

fn json_to_value(val: &serde_json::Value) -> Value {
    match val {
        serde_json::Value::Null => Value::Unit,
        serde_json::Value::Bool(b) => Value::Boolean(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                Value::Integer(0)
            }
        }
        serde_json::Value::String(s) => Value::Str(s.clone()),
        serde_json::Value::Array(arr) => {
            Value::Array(arr.iter().map(json_to_value).collect())
        }
        serde_json::Value::Object(obj) => {
            let map: BTreeMap<String, Value> = obj
                .iter()
                .map(|(k, v)| (k.clone(), json_to_value(v)))
                .collect();
            Value::Map(map)
        }
    }
}

fn value_to_json(val: &Value) -> serde_json::Value {
    match val {
        Value::Integer(i) => serde_json::Value::Number((*i).into()),
        Value::Float(f) => {
            serde_json::Number::from_f64(*f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null)
        }
        Value::Boolean(b) => serde_json::Value::Bool(*b),
        Value::Str(s) => serde_json::Value::String(s.clone()),
        Value::Unit => serde_json::Value::Null,
        Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(value_to_json).collect())
        }
        Value::Struct(fields) | Value::Map(fields) => {
            let map: serde_json::Map<String, serde_json::Value> = fields
                .iter()
                .map(|(k, v)| (k.clone(), value_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
    }
}

/// Interpret an AIRL module by finding and executing its `main` function.
pub fn interpret(module: &Module) -> Result<InterpreterOutput, InterpreterError> {
    interpret_with_limits(module, ExecutionLimits::default())
}

/// Interpret with custom execution limits.
pub fn interpret_with_limits(
    module: &Module,
    limits: ExecutionLimits,
) -> Result<InterpreterOutput, InterpreterError> {
    let main_fn = module
        .find_function("main")
        .ok_or(InterpreterError::NoMainFunction)?;

    let mut interp = Interpreter::new(module, limits);
    interp.eval(&main_fn.body)?;

    Ok(InterpreterOutput {
        stdout: interp.stdout,
        exit_code: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_module(json: &str) -> Module {
        serde_json::from_str(json).unwrap()
    }

    fn run(json: &str) -> String {
        let module = load_module(json);
        let output = interpret(&module).unwrap();
        output.stdout
    }

    fn run_err(json: &str) -> InterpreterError {
        let module = load_module(json);
        interpret(&module).unwrap_err()
    }

    /// Helper: wrap a body node in a main function module
    fn wrap_main(body_json: &str) -> String {
        format!(
            r#"{{
            "format_version": "0.1.0",
            "module": {{
                "id": "mod_main",
                "name": "main",
                "metadata": {{
                    "version": "1.0.0",
                    "description": "test",
                    "author": "test",
                    "created_at": "2026-01-01T00:00:00Z"
                }},
                "imports": [],
                "exports": [],
                "types": [],
                "functions": [
                    {{
                        "id": "f_main",
                        "name": "main",
                        "params": [],
                        "returns": "Unit",
                        "effects": ["IO"],
                        "body": {body_json}
                    }}
                ]
            }}
        }}"#
        )
    }

    /// Helper: wrap body + extra functions
    fn wrap_with_functions(body_json: &str, extra_funcs: &str) -> String {
        format!(
            r#"{{
            "format_version": "0.1.0",
            "module": {{
                "id": "mod_main",
                "name": "main",
                "metadata": {{
                    "version": "1.0.0",
                    "description": "test",
                    "author": "test",
                    "created_at": "2026-01-01T00:00:00Z"
                }},
                "imports": [],
                "exports": [],
                "types": [],
                "functions": [
                    {{
                        "id": "f_main",
                        "name": "main",
                        "params": [],
                        "returns": "Unit",
                        "effects": ["IO"],
                        "body": {body_json}
                    }},
                    {extra_funcs}
                ]
            }}
        }}"#
        )
    }

    #[test]
    fn test_hello_world() {
        let json = wrap_main(r#"{
            "id": "n_1", "kind": "Call", "type": "Unit",
            "target": "std::io::println",
            "args": [{"id": "n_2", "kind": "Literal", "type": "String", "value": "hello world"}]
        }"#);
        assert_eq!(run(&json), "hello world\n");
    }

    #[test]
    fn test_arithmetic() {
        let json = wrap_main(r#"{
            "id": "n_1", "kind": "Call", "type": "Unit",
            "target": "std::io::println",
            "args": [{"id": "n_2", "kind": "BinOp", "type": "I64", "op": "Add",
                "lhs": {"id": "n_3", "kind": "Literal", "type": "I64", "value": 40},
                "rhs": {"id": "n_4", "kind": "Literal", "type": "I64", "value": 2}
            }]
        }"#);
        assert_eq!(run(&json), "42\n");
    }

    #[test]
    fn test_if_then_else() {
        let json = wrap_main(r#"{
            "id": "n_1", "kind": "Call", "type": "Unit",
            "target": "std::io::println",
            "args": [{"id": "n_2", "kind": "If", "type": "String",
                "cond": {"id": "n_3", "kind": "BinOp", "type": "Bool", "op": "Gt",
                    "lhs": {"id": "n_4", "kind": "Literal", "type": "I64", "value": 10},
                    "rhs": {"id": "n_5", "kind": "Literal", "type": "I64", "value": 5}
                },
                "then_branch": {"id": "n_6", "kind": "Literal", "type": "String", "value": "yes"},
                "else_branch": {"id": "n_7", "kind": "Literal", "type": "String", "value": "no"}
            }]
        }"#);
        assert_eq!(run(&json), "yes\n");
    }

    #[test]
    fn test_let_binding() {
        let json = wrap_main(r#"{
            "id": "n_1", "kind": "Let", "type": "Unit", "name": "x",
            "value": {"id": "n_2", "kind": "Literal", "type": "I64", "value": 42},
            "body": {"id": "n_3", "kind": "Call", "type": "Unit",
                "target": "std::io::println",
                "args": [{"id": "n_4", "kind": "Param", "type": "I64", "name": "x", "index": 0}]
            }
        }"#);
        assert_eq!(run(&json), "42\n");
    }

    #[test]
    fn test_no_main_function() {
        let json = r#"{
            "format_version": "0.1.0",
            "module": {"id": "m", "name": "main",
                "metadata": {"version": "1.0.0", "description": "", "author": "", "created_at": ""},
                "imports": [], "exports": [], "types": [], "functions": []
            }
        }"#;
        assert!(matches!(run_err(json), InterpreterError::NoMainFunction));
    }

    #[test]
    fn test_user_defined_function_call() {
        let body = r#"{
            "id": "n_1", "kind": "Call", "type": "Unit",
            "target": "std::io::println",
            "args": [{"id": "n_2", "kind": "Call", "type": "I64",
                "target": "double",
                "args": [{"id": "n_3", "kind": "Literal", "type": "I64", "value": 21}]
            }]
        }"#;
        let double_fn = r#"{
            "id": "f_double",
            "name": "double",
            "params": [{"name": "n", "type": "I64", "index": 0}],
            "returns": "I64",
            "effects": ["Pure"],
            "body": {"id": "n_10", "kind": "BinOp", "type": "I64", "op": "Mul",
                "lhs": {"id": "n_11", "kind": "Param", "type": "I64", "name": "n", "index": 0},
                "rhs": {"id": "n_12", "kind": "Literal", "type": "I64", "value": 2}
            }
        }"#;
        let json = wrap_with_functions(body, double_fn);
        assert_eq!(run(&json), "42\n");
    }

    #[test]
    fn test_recursive_fibonacci() {
        // fib(n) = if n <= 1 then n else fib(n-1) + fib(n-2)
        let body = r#"{
            "id": "n_1", "kind": "Block", "type": "Unit",
            "statements": [
                {"id": "s1", "kind": "Call", "type": "Unit", "target": "std::io::println",
                    "args": [{"id": "a1", "kind": "Call", "type": "I64", "target": "fib",
                        "args": [{"id": "a2", "kind": "Literal", "type": "I64", "value": 10}]}]
                }
            ],
            "result": {"id": "n_end", "kind": "Literal", "type": "Unit", "value": null}
        }"#;
        let fib_fn = r#"{
            "id": "f_fib", "name": "fib",
            "params": [{"name": "n", "type": "I64", "index": 0}],
            "returns": "I64", "effects": ["Pure"],
            "body": {"id": "f1", "kind": "If", "type": "I64",
                "cond": {"id": "f2", "kind": "BinOp", "type": "Bool", "op": "Lte",
                    "lhs": {"id": "f3", "kind": "Param", "type": "I64", "name": "n", "index": 0},
                    "rhs": {"id": "f4", "kind": "Literal", "type": "I64", "value": 1}
                },
                "then_branch": {"id": "f5", "kind": "Param", "type": "I64", "name": "n", "index": 0},
                "else_branch": {"id": "f6", "kind": "BinOp", "type": "I64", "op": "Add",
                    "lhs": {"id": "f7", "kind": "Call", "type": "I64", "target": "fib",
                        "args": [{"id": "f8", "kind": "BinOp", "type": "I64", "op": "Sub",
                            "lhs": {"id": "f9", "kind": "Param", "type": "I64", "name": "n", "index": 0},
                            "rhs": {"id": "f10", "kind": "Literal", "type": "I64", "value": 1}}]
                    },
                    "rhs": {"id": "f11", "kind": "Call", "type": "I64", "target": "fib",
                        "args": [{"id": "f12", "kind": "BinOp", "type": "I64", "op": "Sub",
                            "lhs": {"id": "f13", "kind": "Param", "type": "I64", "name": "n", "index": 0},
                            "rhs": {"id": "f14", "kind": "Literal", "type": "I64", "value": 2}}]
                    }
                }
            }
        }"#;
        let json = wrap_with_functions(body, fib_fn);
        assert_eq!(run(&json), "55\n"); // fib(10) = 55
    }

    #[test]
    fn test_struct_create_and_access() {
        let json = wrap_main(r#"{
            "id": "n_1", "kind": "Let", "type": "Unit", "name": "p",
            "value": {"id": "n_2", "kind": "StructLiteral", "type": "Point",
                "fields": [
                    {"name": "x", "value": {"id": "n_3", "kind": "Literal", "type": "I64", "value": 10}},
                    {"name": "y", "value": {"id": "n_4", "kind": "Literal", "type": "I64", "value": 20}}
                ]
            },
            "body": {"id": "n_5", "kind": "Call", "type": "Unit",
                "target": "std::io::println",
                "args": [{"id": "n_6", "kind": "FieldAccess", "type": "I64",
                    "object": {"id": "n_7", "kind": "Param", "type": "Point", "name": "p", "index": 0},
                    "field": "x"
                }]
            }
        }"#);
        assert_eq!(run(&json), "10\n");
    }

    #[test]
    fn test_array_create_and_index() {
        let json = wrap_main(r#"{
            "id": "n_1", "kind": "Let", "type": "Unit", "name": "arr",
            "value": {"id": "n_2", "kind": "ArrayLiteral", "type": "Array<I64>",
                "elements": [
                    {"id": "n_3", "kind": "Literal", "type": "I64", "value": 10},
                    {"id": "n_4", "kind": "Literal", "type": "I64", "value": 20},
                    {"id": "n_5", "kind": "Literal", "type": "I64", "value": 30}
                ]
            },
            "body": {"id": "n_6", "kind": "Call", "type": "Unit",
                "target": "std::io::println",
                "args": [{"id": "n_7", "kind": "IndexAccess", "type": "I64",
                    "array": {"id": "n_8", "kind": "Param", "type": "Array<I64>", "name": "arr", "index": 0},
                    "index": {"id": "n_9", "kind": "Literal", "type": "I64", "value": 1}
                }]
            }
        }"#);
        assert_eq!(run(&json), "20\n");
    }

    #[test]
    fn test_match_literal() {
        let json = wrap_main(r#"{
            "id": "n_1", "kind": "Call", "type": "Unit",
            "target": "std::io::println",
            "args": [{"id": "n_2", "kind": "Match", "type": "String",
                "scrutinee": {"id": "n_3", "kind": "Literal", "type": "I64", "value": 2},
                "arms": [
                    {"pattern": {"kind": "Literal", "value": 1},
                     "body": {"id": "n_4", "kind": "Literal", "type": "String", "value": "one"}},
                    {"pattern": {"kind": "Literal", "value": 2},
                     "body": {"id": "n_5", "kind": "Literal", "type": "String", "value": "two"}},
                    {"pattern": {"kind": "Wildcard"},
                     "body": {"id": "n_6", "kind": "Literal", "type": "String", "value": "other"}}
                ]
            }]
        }"#);
        assert_eq!(run(&json), "two\n");
    }

    #[test]
    fn test_match_wildcard() {
        let json = wrap_main(r#"{
            "id": "n_1", "kind": "Call", "type": "Unit",
            "target": "std::io::println",
            "args": [{"id": "n_2", "kind": "Match", "type": "String",
                "scrutinee": {"id": "n_3", "kind": "Literal", "type": "I64", "value": 99},
                "arms": [
                    {"pattern": {"kind": "Literal", "value": 1},
                     "body": {"id": "n_4", "kind": "Literal", "type": "String", "value": "one"}},
                    {"pattern": {"kind": "Wildcard"},
                     "body": {"id": "n_5", "kind": "Literal", "type": "String", "value": "other"}}
                ]
            }]
        }"#);
        assert_eq!(run(&json), "other\n");
    }

    #[test]
    fn test_block_multiple_statements() {
        let json = wrap_main(r#"{
            "id": "n_1", "kind": "Block", "type": "Unit",
            "statements": [
                {"id": "s1", "kind": "Call", "type": "Unit",
                    "target": "std::io::println",
                    "args": [{"id": "a1", "kind": "Literal", "type": "String", "value": "first"}]},
                {"id": "s2", "kind": "Call", "type": "Unit",
                    "target": "std::io::println",
                    "args": [{"id": "a2", "kind": "Literal", "type": "String", "value": "second"}]}
            ],
            "result": {"id": "n_end", "kind": "Literal", "type": "Unit", "value": null}
        }"#);
        assert_eq!(run(&json), "first\nsecond\n");
    }

    #[test]
    fn test_nested_let_bindings() {
        let json = wrap_main(r#"{
            "id": "n_1", "kind": "Let", "type": "Unit", "name": "x",
            "value": {"id": "n_2", "kind": "Literal", "type": "I64", "value": 10},
            "body": {"id": "n_3", "kind": "Let", "type": "Unit", "name": "y",
                "value": {"id": "n_4", "kind": "BinOp", "type": "I64", "op": "Mul",
                    "lhs": {"id": "n_5", "kind": "Param", "type": "I64", "name": "x", "index": 0},
                    "rhs": {"id": "n_6", "kind": "Literal", "type": "I64", "value": 2}
                },
                "body": {"id": "n_7", "kind": "Call", "type": "Unit",
                    "target": "std::io::println",
                    "args": [{"id": "n_8", "kind": "Param", "type": "I64", "name": "y", "index": 0}]
                }
            }
        }"#);
        assert_eq!(run(&json), "20\n");
    }

    #[test]
    fn test_division_by_zero() {
        let json = wrap_main(r#"{
            "id": "n_1", "kind": "BinOp", "type": "I64", "op": "Div",
            "lhs": {"id": "n_2", "kind": "Literal", "type": "I64", "value": 10},
            "rhs": {"id": "n_3", "kind": "Literal", "type": "I64", "value": 0}
        }"#);
        assert!(matches!(run_err(&json), InterpreterError::DivisionByZero(_)));
    }

    #[test]
    fn test_step_limit() {
        // An infinite-like computation that should be killed
        let body = r#"{
            "id": "n_1", "kind": "Call", "type": "Unit",
            "target": "std::io::println",
            "args": [{"id": "n_2", "kind": "Call", "type": "I64", "target": "spin",
                "args": [{"id": "n_3", "kind": "Literal", "type": "I64", "value": 0}]}]
        }"#;
        // spin(n) = spin(n + 1) — infinite recursion
        let spin_fn = r#"{
            "id": "f_spin", "name": "spin",
            "params": [{"name": "n", "type": "I64", "index": 0}],
            "returns": "I64", "effects": ["Pure"],
            "body": {"id": "s1", "kind": "Call", "type": "I64", "target": "spin",
                "args": [{"id": "s2", "kind": "BinOp", "type": "I64", "op": "Add",
                    "lhs": {"id": "s3", "kind": "Param", "type": "I64", "name": "n", "index": 0},
                    "rhs": {"id": "s4", "kind": "Literal", "type": "I64", "value": 1}
                }]
            }
        }"#;
        let json = wrap_with_functions(body, spin_fn);
        let module = load_module(&json);
        let limits = ExecutionLimits {
            max_steps: 100,
            max_call_depth: 1000,
        };
        let err = interpret_with_limits(&module, limits).unwrap_err();
        assert!(matches!(err, InterpreterError::StepLimitExceeded(_)));
    }

    #[test]
    fn test_call_depth_limit() {
        let body = r#"{
            "id": "n_1", "kind": "Call", "type": "I64", "target": "deep",
            "args": [{"id": "n_2", "kind": "Literal", "type": "I64", "value": 0}]
        }"#;
        let deep_fn = r#"{
            "id": "f_deep", "name": "deep",
            "params": [{"name": "n", "type": "I64", "index": 0}],
            "returns": "I64", "effects": ["Pure"],
            "body": {"id": "d1", "kind": "Call", "type": "I64", "target": "deep",
                "args": [{"id": "d2", "kind": "BinOp", "type": "I64", "op": "Add",
                    "lhs": {"id": "d3", "kind": "Param", "type": "I64", "name": "n", "index": 0},
                    "rhs": {"id": "d4", "kind": "Literal", "type": "I64", "value": 1}
                }]
            }
        }"#;
        let json = wrap_with_functions(body, deep_fn);
        let module = load_module(&json);
        let limits = ExecutionLimits {
            max_steps: 1_000_000,
            max_call_depth: 50,
        };
        let err = interpret_with_limits(&module, limits).unwrap_err();
        assert!(matches!(err, InterpreterError::CallDepthExceeded(_)));
    }

    #[test]
    fn test_index_out_of_bounds() {
        let json = wrap_main(r#"{
            "id": "n_1", "kind": "Let", "type": "Unit", "name": "arr",
            "value": {"id": "n_2", "kind": "ArrayLiteral", "type": "Array<I64>",
                "elements": [{"id": "n_3", "kind": "Literal", "type": "I64", "value": 1}]
            },
            "body": {"id": "n_4", "kind": "IndexAccess", "type": "I64",
                "array": {"id": "n_5", "kind": "Param", "type": "Array<I64>", "name": "arr", "index": 0},
                "index": {"id": "n_6", "kind": "Literal", "type": "I64", "value": 5}
            }
        }"#);
        assert!(matches!(
            run_err(&json),
            InterpreterError::IndexOutOfBounds { .. }
        ));
    }

    #[test]
    fn test_string_builtins() {
        let json = wrap_main(r#"{
            "id": "n_1", "kind": "Block", "type": "Unit",
            "statements": [
                {"id": "s1", "kind": "Call", "type": "Unit", "target": "std::io::println",
                    "args": [{"id": "a1", "kind": "Call", "type": "I64", "target": "std::string::len",
                        "args": [{"id": "a2", "kind": "Literal", "type": "String", "value": "hello"}]}]
                },
                {"id": "s2", "kind": "Call", "type": "Unit", "target": "std::io::println",
                    "args": [{"id": "a3", "kind": "Call", "type": "Bool", "target": "std::string::contains",
                        "args": [
                            {"id": "a4", "kind": "Literal", "type": "String", "value": "hello world"},
                            {"id": "a5", "kind": "Literal", "type": "String", "value": "world"}
                        ]}]
                }
            ],
            "result": {"id": "n_end", "kind": "Literal", "type": "Unit", "value": null}
        }"#);
        assert_eq!(run(&json), "5\ntrue\n");
    }

    #[test]
    fn test_string_concat_via_add() {
        let json = wrap_main(r#"{
            "id": "n_1", "kind": "Call", "type": "Unit", "target": "std::io::println",
            "args": [{"id": "n_2", "kind": "BinOp", "type": "String", "op": "Add",
                "lhs": {"id": "n_3", "kind": "Literal", "type": "String", "value": "hello "},
                "rhs": {"id": "n_4", "kind": "Literal", "type": "String", "value": "world"}
            }]
        }"#);
        assert_eq!(run(&json), "hello world\n");
    }

    #[test]
    fn test_array_builtins() {
        let json = wrap_main(r#"{
            "id": "n_1", "kind": "Let", "type": "Unit", "name": "arr",
            "value": {"id": "n_2", "kind": "ArrayLiteral", "type": "Array<I64>",
                "elements": [
                    {"id": "n_3", "kind": "Literal", "type": "I64", "value": 10},
                    {"id": "n_4", "kind": "Literal", "type": "I64", "value": 20}
                ]
            },
            "body": {"id": "n_5", "kind": "Call", "type": "Unit", "target": "std::io::println",
                "args": [{"id": "n_6", "kind": "Call", "type": "I64", "target": "std::array::len",
                    "args": [{"id": "n_7", "kind": "Param", "type": "Array<I64>", "name": "arr", "index": 0}]
                }]
            }
        }"#);
        assert_eq!(run(&json), "2\n");
    }

    // ---- New Stage 7 builtin tests ----

    #[test]
    fn test_string_starts_with() {
        let json = wrap_main(r#"{
            "id":"n1","kind":"Call","type":"Unit","target":"std::io::println",
            "args":[{"id":"n2","kind":"Call","type":"Bool","target":"std::string::starts_with",
                "args":[
                    {"id":"n3","kind":"Literal","type":"String","value":"hello world"},
                    {"id":"n4","kind":"Literal","type":"String","value":"hello"}
                ]}]
        }"#);
        assert_eq!(run(&json), "true\n");
    }

    #[test]
    fn test_string_ends_with() {
        let json = wrap_main(r#"{
            "id":"n1","kind":"Call","type":"Unit","target":"std::io::println",
            "args":[{"id":"n2","kind":"Call","type":"Bool","target":"std::string::ends_with",
                "args":[
                    {"id":"n3","kind":"Literal","type":"String","value":"hello world"},
                    {"id":"n4","kind":"Literal","type":"String","value":"world"}
                ]}]
        }"#);
        assert_eq!(run(&json), "true\n");
    }

    #[test]
    fn test_string_trim() {
        let json = wrap_main(r#"{
            "id":"n1","kind":"Call","type":"Unit","target":"std::io::println",
            "args":[{"id":"n2","kind":"Call","type":"String","target":"std::string::trim",
                "args":[{"id":"n3","kind":"Literal","type":"String","value":"  hello  "}]}]
        }"#);
        assert_eq!(run(&json), "hello\n");
    }

    #[test]
    fn test_string_to_uppercase() {
        let json = wrap_main(r#"{
            "id":"n1","kind":"Call","type":"Unit","target":"std::io::println",
            "args":[{"id":"n2","kind":"Call","type":"String","target":"std::string::to_uppercase",
                "args":[{"id":"n3","kind":"Literal","type":"String","value":"hello"}]}]
        }"#);
        assert_eq!(run(&json), "HELLO\n");
    }

    #[test]
    fn test_string_to_lowercase() {
        let json = wrap_main(r#"{
            "id":"n1","kind":"Call","type":"Unit","target":"std::io::println",
            "args":[{"id":"n2","kind":"Call","type":"String","target":"std::string::to_lowercase",
                "args":[{"id":"n3","kind":"Literal","type":"String","value":"HELLO"}]}]
        }"#);
        assert_eq!(run(&json), "hello\n");
    }

    #[test]
    fn test_string_replace() {
        let json = wrap_main(r#"{
            "id":"n1","kind":"Call","type":"Unit","target":"std::io::println",
            "args":[{"id":"n2","kind":"Call","type":"String","target":"std::string::replace",
                "args":[
                    {"id":"n3","kind":"Literal","type":"String","value":"hello world"},
                    {"id":"n4","kind":"Literal","type":"String","value":"world"},
                    {"id":"n5","kind":"Literal","type":"String","value":"AIRL"}
                ]}]
        }"#);
        assert_eq!(run(&json), "hello AIRL\n");
    }

    #[test]
    fn test_math_pow() {
        let json = wrap_main(r#"{
            "id":"n1","kind":"Call","type":"Unit","target":"std::io::println",
            "args":[{"id":"n2","kind":"Call","type":"I64","target":"std::math::pow",
                "args":[
                    {"id":"n3","kind":"Literal","type":"I64","value":2},
                    {"id":"n4","kind":"Literal","type":"I64","value":10}
                ]}]
        }"#);
        assert_eq!(run(&json), "1024\n");
    }

    #[test]
    fn test_array_range() {
        let json = wrap_main(r#"{
            "id":"n1","kind":"Call","type":"Unit","target":"std::io::println",
            "args":[{"id":"n2","kind":"Call","type":"String","target":"std::array::join",
                "args":[
                    {"id":"n3","kind":"Call","type":"Array<I64>","target":"std::array::range",
                        "args":[
                            {"id":"n4","kind":"Literal","type":"I64","value":1},
                            {"id":"n5","kind":"Literal","type":"I64","value":5}
                        ]},
                    {"id":"n6","kind":"Literal","type":"String","value":", "}
                ]}]
        }"#);
        assert_eq!(run(&json), "1, 2, 3, 4\n");
    }

    #[test]
    fn test_array_contains() {
        let json = wrap_main(r#"{
            "id":"n1","kind":"Call","type":"Unit","target":"std::io::println",
            "args":[{"id":"n2","kind":"Call","type":"Bool","target":"std::array::contains",
                "args":[
                    {"id":"n3","kind":"ArrayLiteral","type":"Array<I64>","elements":[
                        {"id":"e1","kind":"Literal","type":"I64","value":10},
                        {"id":"e2","kind":"Literal","type":"I64","value":20},
                        {"id":"e3","kind":"Literal","type":"I64","value":30}
                    ]},
                    {"id":"n4","kind":"Literal","type":"I64","value":20}
                ]}]
        }"#);
        assert_eq!(run(&json), "true\n");
    }

    #[test]
    fn test_array_reverse() {
        let json = wrap_main(r#"{
            "id":"n1","kind":"Call","type":"Unit","target":"std::io::println",
            "args":[{"id":"n2","kind":"Call","type":"String","target":"std::array::join",
                "args":[
                    {"id":"n3","kind":"Call","type":"Array<I64>","target":"std::array::reverse",
                        "args":[{"id":"n4","kind":"ArrayLiteral","type":"Array<I64>","elements":[
                            {"id":"e1","kind":"Literal","type":"I64","value":1},
                            {"id":"e2","kind":"Literal","type":"I64","value":2},
                            {"id":"e3","kind":"Literal","type":"I64","value":3}
                        ]}]},
                    {"id":"n5","kind":"Literal","type":"String","value":", "}
                ]}]
        }"#);
        assert_eq!(run(&json), "3, 2, 1\n");
    }

    #[test]
    fn test_array_slice() {
        let json = wrap_main(r#"{
            "id":"n1","kind":"Call","type":"Unit","target":"std::io::println",
            "args":[{"id":"n2","kind":"Call","type":"I64","target":"std::array::len",
                "args":[{"id":"n3","kind":"Call","type":"Array<I64>","target":"std::array::slice",
                    "args":[
                        {"id":"n4","kind":"ArrayLiteral","type":"Array<I64>","elements":[
                            {"id":"e1","kind":"Literal","type":"I64","value":10},
                            {"id":"e2","kind":"Literal","type":"I64","value":20},
                            {"id":"e3","kind":"Literal","type":"I64","value":30},
                            {"id":"e4","kind":"Literal","type":"I64","value":40}
                        ]},
                        {"id":"n5","kind":"Literal","type":"I64","value":1},
                        {"id":"n6","kind":"Literal","type":"I64","value":3}
                    ]}]
            }]
        }"#);
        assert_eq!(run(&json), "2\n");
    }

    #[test]
    fn test_fmt_format() {
        let json = wrap_main(r#"{
            "id":"n1","kind":"Call","type":"Unit","target":"std::io::println",
            "args":[{"id":"n2","kind":"Call","type":"String","target":"std::fmt::format",
                "args":[
                    {"id":"n3","kind":"Literal","type":"String","value":"Hello, {}! You are {} years old."},
                    {"id":"n4","kind":"Literal","type":"String","value":"Alice"},
                    {"id":"n5","kind":"Literal","type":"I64","value":30}
                ]}]
        }"#);
        assert_eq!(run(&json), "Hello, Alice! You are 30 years old.\n");
    }

    #[test]
    fn test_array_get() {
        let json = wrap_main(r#"{
            "id":"n1","kind":"Call","type":"Unit","target":"std::io::println",
            "args":[{"id":"n2","kind":"Call","type":"I64","target":"std::array::get",
                "args":[
                    {"id":"n3","kind":"ArrayLiteral","type":"Array<I64>","elements":[
                        {"id":"e1","kind":"Literal","type":"I64","value":10},
                        {"id":"e2","kind":"Literal","type":"I64","value":20},
                        {"id":"e3","kind":"Literal","type":"I64","value":30}
                    ]},
                    {"id":"n4","kind":"Literal","type":"I64","value":1}
                ]}]
        }"#);
        assert_eq!(run(&json), "20\n");
    }

    #[test]
    fn test_file_io_read_write() {
        // Write a file, then read it back
        let tmp = std::env::temp_dir().join("airl_test_file_io.txt");
        let tmp_str = tmp.to_string_lossy().replace('\\', "\\\\");

        // write_file
        let write_json = wrap_main(&format!(r#"{{
            "id":"n1","kind":"Call","type":"Unit","target":"std::io::println",
            "args":[{{"id":"n2","kind":"Call","type":"Bool","target":"std::io::write_file",
                "args":[
                    {{"id":"n3","kind":"Literal","type":"String","value":"{tmp_str}"}},
                    {{"id":"n4","kind":"Literal","type":"String","value":"hello from airl"}}
                ]}}]
        }}"#));
        assert_eq!(run(&write_json), "true\n");

        // read_file
        let read_json = wrap_main(&format!(r#"{{
            "id":"n1","kind":"Call","type":"Unit","target":"std::io::println",
            "args":[{{"id":"n2","kind":"Call","type":"String","target":"std::io::read_file",
                "args":[
                    {{"id":"n3","kind":"Literal","type":"String","value":"{tmp_str}"}}
                ]}}]
        }}"#));
        assert_eq!(run(&read_json), "hello from airl\n");

        // file_exists
        let exists_json = wrap_main(&format!(r#"{{
            "id":"n1","kind":"Call","type":"Unit","target":"std::io::println",
            "args":[{{"id":"n2","kind":"Call","type":"Bool","target":"std::io::file_exists",
                "args":[
                    {{"id":"n3","kind":"Literal","type":"String","value":"{tmp_str}"}}
                ]}}]
        }}"#));
        assert_eq!(run(&exists_json), "true\n");

        // Clean up
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_file_io_read_dir() {
        let tmp_dir = std::env::temp_dir().join("airl_test_read_dir");
        let _ = std::fs::create_dir_all(&tmp_dir);
        std::fs::write(tmp_dir.join("a.txt"), "a").unwrap();
        std::fs::write(tmp_dir.join("b.txt"), "b").unwrap();

        let dir_str = tmp_dir.to_string_lossy().replace('\\', "\\\\");
        let json = wrap_main(&format!(r#"{{
            "id":"n1","kind":"Call","type":"Unit","target":"std::io::println",
            "args":[{{"id":"n2","kind":"Call","type":"Array<String>","target":"std::io::read_dir",
                "args":[
                    {{"id":"n3","kind":"Literal","type":"String","value":"{dir_str}"}}
                ]}}]
        }}"#));

        let output = run(&json);
        assert!(output.contains("a.txt"));
        assert!(output.contains("b.txt"));

        // Clean up
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_json_parse_and_serialize() {
        // Parse a JSON string, then serialize it back
        let json = wrap_main(r#"{
            "id":"n1","kind":"Call","type":"Unit","target":"std::io::println",
            "args":[{"id":"n2","kind":"Call","type":"String","target":"std::json::serialize",
                "args":[{"id":"n3","kind":"Call","type":"Unit","target":"std::json::parse",
                    "args":[{"id":"n4","kind":"Literal","type":"String","value":"{\"x\":42}"}]
                }]
            }]
        }"#);
        let output = run(&json);
        assert!(output.contains("\"x\":42") || output.contains("\"x\": 42"));
    }

    #[test]
    fn test_collections_map() {
        // Create map, insert, get
        let json = wrap_main(r#"{
            "id":"b","kind":"Block","type":"Unit",
            "statements":[],
            "result": {"id":"n1","kind":"Let","type":"Unit","name":"m",
                "value":{"id":"n2","kind":"Call","type":"Unit","target":"std::collections::new_map","args":[]},
                "body":{"id":"n3","kind":"Let","type":"Unit","name":"m2",
                    "value":{"id":"n4","kind":"Call","type":"Unit","target":"std::collections::insert",
                        "args":[
                            {"id":"n5","kind":"Param","type":"Unit","name":"m","index":0},
                            {"id":"n6","kind":"Literal","type":"String","value":"key"},
                            {"id":"n7","kind":"Literal","type":"I64","value":99}
                        ]},
                    "body":{"id":"n8","kind":"Call","type":"Unit","target":"std::io::println",
                        "args":[{"id":"n9","kind":"Call","type":"Unit","target":"std::collections::get",
                            "args":[
                                {"id":"n10","kind":"Param","type":"Unit","name":"m2","index":0},
                                {"id":"n11","kind":"Literal","type":"String","value":"key"}
                            ]}]
                    }
                }
            }
        }"#);
        assert_eq!(run(&json), "99\n");
    }

    #[test]
    fn test_collections_contains_key() {
        let json = wrap_main(r#"{
            "id":"b","kind":"Block","type":"Unit",
            "statements":[],
            "result": {"id":"n1","kind":"Let","type":"Unit","name":"m",
                "value":{"id":"n2","kind":"Call","type":"Unit","target":"std::collections::insert",
                    "args":[
                        {"id":"n3","kind":"Call","type":"Unit","target":"std::collections::new_map","args":[]},
                        {"id":"n4","kind":"Literal","type":"String","value":"foo"},
                        {"id":"n5","kind":"Literal","type":"I64","value":1}
                    ]},
                "body":{"id":"n6","kind":"Call","type":"Unit","target":"std::io::println",
                    "args":[{"id":"n7","kind":"Call","type":"Bool","target":"std::collections::contains_key",
                        "args":[
                            {"id":"n8","kind":"Param","type":"Unit","name":"m","index":0},
                            {"id":"n9","kind":"Literal","type":"String","value":"foo"}
                        ]}]
                }
            }
        }"#);
        assert_eq!(run(&json), "true\n");
    }

    #[test]
    fn test_process_env_var() {
        // Set an env var, then read it back
        let json = wrap_main(r#"{
            "id":"b","kind":"Block","type":"Unit",
            "statements":[
                {"id":"s1","kind":"Call","type":"Unit","target":"std::process::set_env_var",
                    "args":[
                        {"id":"k","kind":"Literal","type":"String","value":"AIRL_TEST_VAR"},
                        {"id":"v","kind":"Literal","type":"String","value":"hello_from_airl"}
                    ]}
            ],
            "result":{"id":"s2","kind":"Call","type":"Unit","target":"std::io::println",
                "args":[{"id":"g","kind":"Call","type":"String","target":"std::process::env_var",
                    "args":[{"id":"k2","kind":"Literal","type":"String","value":"AIRL_TEST_VAR"}]}]}
        }"#);
        assert_eq!(run(&json), "hello_from_airl\n");
    }

    #[test]
    fn test_error_assert_and_unwrap() {
        // unwrap_or on non-unit returns the value
        let json = wrap_main(r#"{
            "id":"n1","kind":"Call","type":"Unit","target":"std::io::println",
            "args":[{"id":"n2","kind":"Call","type":"Unit","target":"std::error::unwrap_or",
                "args":[
                    {"id":"n3","kind":"Literal","type":"I64","value":42},
                    {"id":"n4","kind":"Literal","type":"I64","value":0}
                ]}]
        }"#);
        assert_eq!(run(&json), "42\n");

        // unwrap_or on Unit returns the default
        let json2 = wrap_main(r#"{
            "id":"n1","kind":"Call","type":"Unit","target":"std::io::println",
            "args":[{"id":"n2","kind":"Call","type":"Unit","target":"std::error::unwrap_or",
                "args":[
                    {"id":"n3","kind":"Literal","type":"Unit","value":null},
                    {"id":"n4","kind":"Literal","type":"I64","value":99}
                ]}]
        }"#);
        assert_eq!(run(&json2), "99\n");
    }
}
