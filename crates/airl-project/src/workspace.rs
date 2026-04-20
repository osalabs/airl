//! Multi-module workspace: load and link multiple AIRL modules.
//!
//! A workspace holds multiple modules and resolves cross-module imports
//! by merging function definitions into a single flat namespace.

use airl_ir::module::Module;
use std::collections::HashMap;
use std::path::Path;

/// A workspace containing multiple AIRL modules.
#[derive(Debug)]
pub struct Workspace {
    /// All loaded modules, keyed by module name.
    pub modules: HashMap<String, Module>,
}

/// Errors when building or querying a workspace.
#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parse error in {file}: {error}")]
    Parse { file: String, error: String },
    #[error("duplicate module name: {0}")]
    DuplicateModule(String),
    #[error(
        "unresolved import: module '{module}' imports '{item}' from '{from_module}' which was not found"
    )]
    UnresolvedImport {
        module: String,
        from_module: String,
        item: String,
    },
}

impl Workspace {
    /// Create an empty workspace.
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
        }
    }

    /// Load a single module from a JSON file and add it to the workspace.
    pub fn load_file(&mut self, path: &Path) -> Result<String, WorkspaceError> {
        let json = std::fs::read_to_string(path)?;
        let module: Module = serde_json::from_str(&json).map_err(|e| WorkspaceError::Parse {
            file: path.display().to_string(),
            error: e.to_string(),
        })?;
        let name = module.module.name.clone();
        if self.modules.contains_key(&name) {
            return Err(WorkspaceError::DuplicateModule(name));
        }
        self.modules.insert(name.clone(), module);
        Ok(name)
    }

    /// Add a module directly.
    pub fn add_module(&mut self, module: Module) -> Result<String, WorkspaceError> {
        let name = module.module.name.clone();
        if self.modules.contains_key(&name) {
            return Err(WorkspaceError::DuplicateModule(name));
        }
        self.modules.insert(name.clone(), module);
        Ok(name)
    }

    /// Load all `.airl.json` files from a directory.
    /// Silently skips modules with names that are already loaded.
    pub fn load_dir(&mut self, dir: &Path) -> Result<Vec<String>, WorkspaceError> {
        let mut loaded = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.to_string_lossy().ends_with(".airl.json") {
                // Peek at the module name before adding
                let json = std::fs::read_to_string(&path)?;
                let module: Module =
                    serde_json::from_str(&json).map_err(|e| WorkspaceError::Parse {
                        file: path.display().to_string(),
                        error: e.to_string(),
                    })?;
                if !self.modules.contains_key(&module.module.name) {
                    let name = module.module.name.clone();
                    self.modules.insert(name.clone(), module);
                    loaded.push(name);
                }
            }
        }
        Ok(loaded)
    }

    /// Resolve imports and produce a merged module containing all functions.
    /// Functions are namespaced as `module_name::func_name` for non-main modules.
    pub fn resolve(&self) -> Result<Module, WorkspaceError> {
        // Verify all imports can be resolved
        for (mod_name, module) in &self.modules {
            for import in &module.module.imports {
                // std:: imports are builtins, skip
                if import.module.starts_with("std::") {
                    continue;
                }
                // Check if the imported module exists
                if !self.modules.contains_key(&import.module) {
                    // Not a fatal error — might be a builtin module
                    continue;
                }
                // Check if imported items exist in the source module
                let source = &self.modules[&import.module];
                for item in &import.items {
                    if source.find_function(item).is_none() {
                        return Err(WorkspaceError::UnresolvedImport {
                            module: mod_name.clone(),
                            from_module: import.module.clone(),
                            item: item.clone(),
                        });
                    }
                }
            }
        }

        // Find the "main" module (the one with a main function)
        let main_module = self
            .modules
            .values()
            .find(|m| m.find_function("main").is_some())
            .or_else(|| self.modules.values().next());

        let base = match main_module {
            Some(m) => m.clone(),
            None => {
                return Ok(Module {
                    format_version: "0.1.0".to_string(),
                    module: airl_ir::module::ModuleInner {
                        id: airl_ir::ids::ModuleId::new("mod_workspace"),
                        name: "workspace".to_string(),
                        metadata: airl_ir::module::ModuleMetadata {
                            version: "1.0.0".to_string(),
                            description: "Empty workspace".to_string(),
                            author: String::new(),
                            created_at: String::new(),
                        },
                        imports: vec![],
                        exports: vec![],
                        types: vec![],
                        traits: vec![],
                        impls: vec![],
                        constants: vec![],
                        functions: vec![],
                    },
                });
            }
        };

        let mut merged = base;

        // Add functions from other modules (skip duplicates)
        let existing_names: std::collections::HashSet<String> =
            merged.functions().iter().map(|f| f.name.clone()).collect();

        for (mod_name, module) in &self.modules {
            if module.module.name == merged.module.name {
                continue; // Skip the base module
            }
            for func in module.functions() {
                let qualified = if existing_names.contains(&func.name) {
                    format!("{mod_name}::{}", func.name)
                } else {
                    func.name.clone()
                };
                if !existing_names.contains(&qualified) {
                    let mut f = func.clone();
                    f.name = qualified;
                    merged.module.functions.push(f);
                }
            }
        }

        Ok(merged)
    }

    /// Get a list of all function names across all modules.
    pub fn all_functions(&self) -> Vec<(String, String)> {
        let mut result = Vec::new();
        for (mod_name, module) in &self.modules {
            for func in module.functions() {
                result.push((mod_name.clone(), func.name.clone()));
            }
        }
        result
    }
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hello_module() -> Module {
        serde_json::from_str(
            r#"{
            "format_version":"0.1.0",
            "module":{"id":"m1","name":"main",
                "metadata":{"version":"1","description":"","author":"","created_at":""},
                "imports":[],"exports":[],"types":[],
                "functions":[{
                    "id":"f1","name":"main","params":[],"returns":"Unit","effects":["IO"],
                    "body":{"id":"n1","kind":"Call","type":"Unit","target":"std::io::println",
                        "args":[{"id":"n2","kind":"Literal","type":"String","value":"hello"}]}
                }]
            }
        }"#,
        )
        .unwrap()
    }

    fn math_module() -> Module {
        serde_json::from_str(
            r#"{
            "format_version":"0.1.0",
            "module":{"id":"m2","name":"mathlib",
                "metadata":{"version":"1","description":"","author":"","created_at":""},
                "imports":[],"exports":[],"types":[],
                "functions":[{
                    "id":"f2","name":"double","params":[{"name":"n","type":"I64","index":0}],
                    "returns":"I64","effects":["Pure"],
                    "body":{"id":"n3","kind":"BinOp","type":"I64","op":"Mul",
                        "lhs":{"id":"n4","kind":"Param","type":"I64","name":"n","index":0},
                        "rhs":{"id":"n5","kind":"Literal","type":"I64","value":2}}
                }]
            }
        }"#,
        )
        .unwrap()
    }

    #[test]
    fn test_workspace_add_and_resolve() {
        let mut ws = Workspace::new();
        ws.add_module(hello_module()).unwrap();
        ws.add_module(math_module()).unwrap();

        let merged = ws.resolve().unwrap();
        assert!(merged.find_function("main").is_some());
        assert!(merged.find_function("double").is_some());
        assert_eq!(merged.functions().len(), 2);
    }

    #[test]
    fn test_workspace_duplicate_module_error() {
        let mut ws = Workspace::new();
        ws.add_module(hello_module()).unwrap();
        assert!(ws.add_module(hello_module()).is_err());
    }

    #[test]
    fn test_workspace_all_functions() {
        let mut ws = Workspace::new();
        ws.add_module(hello_module()).unwrap();
        ws.add_module(math_module()).unwrap();

        let funcs = ws.all_functions();
        assert_eq!(funcs.len(), 2);
    }
}
