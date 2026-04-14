use crate::effects::Effect;
use crate::ids::{FuncId, ModuleId, TypeId};
use crate::node::Node;
use crate::types::Type;
use serde::{Deserialize, Serialize};

/// Top-level module container matching the JSON IR format.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Module {
    pub format_version: String,
    pub module: ModuleInner,
}

/// The inner module definition containing all declarations.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleInner {
    pub id: ModuleId,
    pub name: String,
    pub metadata: ModuleMetadata,
    pub imports: Vec<Import>,
    pub exports: Vec<Export>,
    pub types: Vec<TypeDef>,
    #[serde(default)]
    pub traits: Vec<serde_json::Value>,
    #[serde(default)]
    pub impls: Vec<serde_json::Value>,
    #[serde(default)]
    pub constants: Vec<serde_json::Value>,
    pub functions: Vec<FuncDef>,
}

/// Module metadata.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleMetadata {
    pub version: String,
    pub description: String,
    pub author: String,
    pub created_at: String,
}

/// An import declaration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Import {
    pub module: String,
    pub items: Vec<String>,
}

/// An export declaration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Export {
    pub kind: String,
    pub name: String,
}

/// A type definition. For now uses serde_json::Value for the body,
/// but has a typed id field.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TypeDef {
    pub id: TypeId,
    #[serde(flatten)]
    pub data: serde_json::Value,
}

/// A function definition.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FuncDef {
    pub id: FuncId,
    pub name: String,
    #[serde(default)]
    pub params: Vec<ParamDef>,
    pub returns: Type,
    #[serde(default)]
    pub effects: Vec<Effect>,
    pub body: Node,
}

/// A function parameter definition.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParamDef {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: Type,
    #[serde(default)]
    pub index: u32,
}

impl Module {
    /// Get the module ID.
    pub fn id(&self) -> &ModuleId {
        &self.module.id
    }

    /// Get the module name.
    pub fn name(&self) -> &str {
        &self.module.name
    }

    /// Get all function definitions.
    pub fn functions(&self) -> &[FuncDef] {
        &self.module.functions
    }

    /// Find a function by name.
    pub fn find_function(&self, name: &str) -> Option<&FuncDef> {
        self.module.functions.iter().find(|f| f.name == name)
    }

    /// Find a function by ID.
    pub fn find_function_by_id(&self, id: &FuncId) -> Option<&FuncDef> {
        self.module.functions.iter().find(|f| &f.id == id)
    }
}

impl FuncDef {
    /// Check if this function has a specific effect.
    pub fn has_effect(&self, effect: &Effect) -> bool {
        self.effects.contains(effect)
    }

    /// Check if this function is pure (no side effects).
    pub fn is_pure(&self) -> bool {
        self.effects.is_empty() || (self.effects.len() == 1 && self.effects[0] == Effect::Pure)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hello_world_module_deserialize() {
        let json = r#"{
            "format_version": "0.1.0",
            "module": {
                "id": "mod_main",
                "name": "main",
                "metadata": {
                    "version": "1.0.0",
                    "description": "Hello world program",
                    "author": "agent-001",
                    "created_at": "2026-04-06T12:00:00Z"
                },
                "imports": [
                    { "module": "std::io", "items": ["println"] }
                ],
                "exports": [
                    { "kind": "Function", "name": "main" }
                ],
                "types": [],
                "traits": [],
                "impls": [],
                "constants": [],
                "functions": [
                    {
                        "id": "f_main",
                        "name": "main",
                        "params": [],
                        "returns": "Unit",
                        "effects": ["IO"],
                        "body": {
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
                        }
                    }
                ]
            }
        }"#;

        let module: Module = serde_json::from_str(json).unwrap();
        assert_eq!(module.format_version, "0.1.0");
        assert_eq!(module.module.id, ModuleId::new("mod_main"));
        assert_eq!(module.module.name, "main");
        assert_eq!(module.module.metadata.description, "Hello world program");
        assert_eq!(module.module.imports.len(), 1);
        assert_eq!(module.module.exports.len(), 1);
        assert_eq!(module.module.functions.len(), 1);

        let func = &module.module.functions[0];
        assert_eq!(func.name, "main");
        assert_eq!(func.returns, Type::Unit);
        assert_eq!(func.effects, vec![Effect::IO]);

        match &func.body {
            crate::node::Node::Call { target, args, .. } => {
                assert_eq!(target, "std::io::println");
                assert_eq!(args.len(), 1);
            }
            other => panic!("Expected Call node, got: {other:?}"),
        }
    }

    #[test]
    fn test_module_roundtrip() {
        let json = r#"{
            "format_version": "0.1.0",
            "module": {
                "id": "mod_main",
                "name": "main",
                "metadata": {
                    "version": "1.0.0",
                    "description": "Test",
                    "author": "test",
                    "created_at": "2026-01-01T00:00:00Z"
                },
                "imports": [],
                "exports": [],
                "types": [],
                "traits": [],
                "impls": [],
                "constants": [],
                "functions": [
                    {
                        "id": "f_main",
                        "name": "main",
                        "params": [],
                        "returns": "Unit",
                        "effects": ["Pure"],
                        "body": {
                            "id": "n_1",
                            "kind": "Literal",
                            "type": "Unit",
                            "value": null
                        }
                    }
                ]
            }
        }"#;

        let module: Module = serde_json::from_str(json).unwrap();
        let serialized = serde_json::to_string_pretty(&module).unwrap();
        let reparsed: Module = serde_json::from_str(&serialized).unwrap();
        assert_eq!(module, reparsed);
    }
}
