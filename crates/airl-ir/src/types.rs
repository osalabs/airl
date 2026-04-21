use crate::effects::Effect;
use crate::ids::{Symbol, TypeId};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// A variant in an enum type definition.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Variant {
    /// Variant name (e.g. `Ok`, `Err`, `None`, `Some`).
    pub name: Symbol,
    /// Named fields of this variant, with their types.
    pub fields: Vec<(Symbol, Type)>,
}

/// The core type system for AIRL IR.
#[derive(Clone, Debug, PartialEq)]
#[allow(missing_docs)] // variant names are self-documenting
pub enum Type {
    /// The unit type `()`. Also used as a wildcard for generic builtins.
    Unit,
    /// Boolean type: `true` or `false`.
    Bool,
    I8,
    I16,
    I32,
    /// Signed 64-bit integer (the default integer type in AIRL).
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    /// 64-bit floating-point number.
    F64,
    /// UTF-8 string.
    String,
    /// Raw byte array.
    Bytes,

    // Composite
    Array {
        element: Box<Type>,
    },
    Tuple {
        elements: Vec<Type>,
    },
    Struct {
        name: Symbol,
        fields: Vec<(Symbol, Type)>,
    },
    Enum {
        name: Symbol,
        variants: Vec<Variant>,
    },
    Function {
        params: Vec<Type>,
        returns: Box<Type>,
        effects: Vec<Effect>,
    },
    Reference {
        inner: Box<Type>,
        mutable: bool,
    },
    Optional {
        inner: Box<Type>,
    },
    Result {
        ok: Box<Type>,
        err: Box<Type>,
    },
    TypeParam {
        name: Symbol,
        bounds: Vec<std::string::String>,
    },
    Generic {
        base: Box<Type>,
        args: Vec<Type>,
    },
    Named(TypeId),
}

impl Type {
    /// Parse a type string from the JSON IR format into a Type.
    ///
    /// Handles simple types like `I64`, `Bool`, `String`, `Unit`,
    /// composite types like `Array<I64>`, `Optional<String>`,
    /// `Result<I64,String>`, and falls back to Named for unknown types.
    pub fn from_type_str(s: &str) -> Type {
        let s = s.trim();
        match s {
            "Unit" | "()" => Type::Unit,
            "Bool" => Type::Bool,
            "I8" => Type::I8,
            "I16" => Type::I16,
            "I32" => Type::I32,
            "I64" => Type::I64,
            "U8" => Type::U8,
            "U16" => Type::U16,
            "U32" => Type::U32,
            "U64" => Type::U64,
            "F32" => Type::F32,
            "F64" => Type::F64,
            "String" => Type::String,
            "Bytes" => Type::Bytes,
            _ => {
                // Try to parse generic types like Array<I64>, Optional<String>, etc.
                if let Some(inner_str) = strip_generic(s, "Array") {
                    Type::Array {
                        element: Box::new(Type::from_type_str(inner_str)),
                    }
                } else if let Some(inner_str) = strip_generic(s, "Optional") {
                    Type::Optional {
                        inner: Box::new(Type::from_type_str(inner_str)),
                    }
                } else if let Some(inner_str) = strip_generic(s, "Result") {
                    // Result<Ok, Err>
                    let parts = split_type_args(inner_str);
                    if parts.len() == 2 {
                        Type::Result {
                            ok: Box::new(Type::from_type_str(&parts[0])),
                            err: Box::new(Type::from_type_str(&parts[1])),
                        }
                    } else {
                        Type::Named(TypeId::new(s))
                    }
                } else if let Some(inner_str) = strip_generic(s, "Tuple") {
                    let parts = split_type_args(inner_str);
                    Type::Tuple {
                        elements: parts.iter().map(|p| Type::from_type_str(p)).collect(),
                    }
                } else if let Some(inner_str) = strip_generic(s, "Ref") {
                    Type::Reference {
                        inner: Box::new(Type::from_type_str(inner_str)),
                        mutable: false,
                    }
                } else if let Some(inner_str) = strip_generic(s, "MutRef") {
                    Type::Reference {
                        inner: Box::new(Type::from_type_str(inner_str)),
                        mutable: true,
                    }
                } else {
                    Type::Named(TypeId::new(s))
                }
            }
        }
    }

    /// Convert a Type to its string representation for JSON serialization.
    pub fn to_type_str(&self) -> std::string::String {
        match self {
            Type::Unit => "Unit".into(),
            Type::Bool => "Bool".into(),
            Type::I8 => "I8".into(),
            Type::I16 => "I16".into(),
            Type::I32 => "I32".into(),
            Type::I64 => "I64".into(),
            Type::U8 => "U8".into(),
            Type::U16 => "U16".into(),
            Type::U32 => "U32".into(),
            Type::U64 => "U64".into(),
            Type::F32 => "F32".into(),
            Type::F64 => "F64".into(),
            Type::String => "String".into(),
            Type::Bytes => "Bytes".into(),
            Type::Array { element } => format!("Array<{}>", element.to_type_str()),
            Type::Tuple { elements } => {
                let inner: Vec<_> = elements.iter().map(|e| e.to_type_str()).collect();
                format!("Tuple<{}>", inner.join(", "))
            }
            Type::Optional { inner } => format!("Optional<{}>", inner.to_type_str()),
            Type::Result { ok, err } => {
                format!("Result<{}, {}>", ok.to_type_str(), err.to_type_str())
            }
            Type::Reference { inner, mutable } => {
                if *mutable {
                    format!("MutRef<{}>", inner.to_type_str())
                } else {
                    format!("Ref<{}>", inner.to_type_str())
                }
            }
            Type::Named(id) => id.0.clone(),
            Type::Struct { name, .. } => name.0.clone(),
            Type::Enum { name, .. } => name.0.clone(),
            Type::Function {
                params, returns, ..
            } => {
                let p: Vec<_> = params.iter().map(|t| t.to_type_str()).collect();
                format!("Fn({}) -> {}", p.join(", "), returns.to_type_str())
            }
            Type::TypeParam { name, .. } => name.0.clone(),
            Type::Generic { base, args } => {
                let a: Vec<_> = args.iter().map(|t| t.to_type_str()).collect();
                format!("{}<{}>", base.to_type_str(), a.join(", "))
            }
        }
    }
}

/// Strip a generic wrapper like "Array<I64>" -> "I64" given prefix "Array".
fn strip_generic<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix(prefix) {
        let rest = rest.trim_start();
        if rest.starts_with('<') && rest.ends_with('>') {
            Some(&rest[1..rest.len() - 1])
        } else {
            None
        }
    } else {
        None
    }
}

/// Split type arguments at the top level, respecting nested angle brackets.
/// "I64, String" -> ["I64", "String"]
/// "Array<I64>, String" -> ["Array<I64>", "String"]
fn split_type_args(s: &str) -> Vec<std::string::String> {
    let mut result = Vec::new();
    let mut depth = 0;
    let mut current = std::string::String::new();

    for ch in s.chars() {
        match ch {
            '<' => {
                depth += 1;
                current.push(ch);
            }
            '>' => {
                depth -= 1;
                current.push(ch);
            }
            ',' if depth == 0 => {
                result.push(current.trim().to_string());
                current = std::string::String::new();
            }
            _ => {
                current.push(ch);
            }
        }
    }

    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        result.push(trimmed);
    }

    result
}

impl Serialize for Type {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_type_str())
    }
}

impl<'de> Deserialize<'de> for Type {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = std::string::String::deserialize(deserializer)?;
        Ok(Type::from_type_str(&s))
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_type_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_primitive_roundtrip() {
        let types = vec![
            Type::Unit,
            Type::Bool,
            Type::I32,
            Type::I64,
            Type::F64,
            Type::String,
        ];
        for t in types {
            let s = t.to_type_str();
            let parsed = Type::from_type_str(&s);
            assert_eq!(t, parsed, "roundtrip failed for {s}");
        }
    }

    #[test]
    fn test_array_roundtrip() {
        let t = Type::Array {
            element: Box::new(Type::I64),
        };
        assert_eq!(t.to_type_str(), "Array<I64>");
        assert_eq!(Type::from_type_str("Array<I64>"), t);
    }

    #[test]
    fn test_result_roundtrip() {
        let t = Type::Result {
            ok: Box::new(Type::I64),
            err: Box::new(Type::String),
        };
        assert_eq!(t.to_type_str(), "Result<I64, String>");
        assert_eq!(Type::from_type_str("Result<I64, String>"), t);
    }

    #[test]
    fn test_named_fallback() {
        let t = Type::from_type_str("MyCustomType");
        assert_eq!(t, Type::Named(TypeId::new("MyCustomType")));
    }

    #[test]
    fn test_serde_roundtrip() {
        let t = Type::I64;
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(json, "\"I64\"");
        let parsed: Type = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, t);
    }
}
