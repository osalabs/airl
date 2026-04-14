use crate::module::Module;
use thiserror::Error;

/// Errors that can occur when working with the IR graph.
#[derive(Debug, Error)]
pub enum IRGraphError {
    #[error("JSON serialization error: {0}")]
    SerializeError(#[from] serde_json::Error),
    #[error("Invalid IR: {0}")]
    InvalidIR(String),
}

/// A container for the AIRL IR module graph.
///
/// Provides convenience methods for loading and saving IR modules.
#[derive(Clone, Debug)]
pub struct IRGraph {
    pub module: Module,
}

impl IRGraph {
    /// Parse an IR graph from a JSON string.
    pub fn from_json(json: &str) -> Result<Self, IRGraphError> {
        let module: Module = serde_json::from_str(json)?;
        Ok(IRGraph { module })
    }

    /// Serialize the IR graph to a pretty-printed JSON string.
    pub fn to_json(&self) -> Result<String, IRGraphError> {
        let json = serde_json::to_string_pretty(&self.module)?;
        Ok(json)
    }

    /// Serialize the IR graph to a compact JSON string.
    pub fn to_json_compact(&self) -> Result<String, IRGraphError> {
        let json = serde_json::to_string(&self.module)?;
        Ok(json)
    }

    /// Get a reference to the underlying module.
    pub fn module(&self) -> &Module {
        &self.module
    }

    /// Get a mutable reference to the underlying module.
    pub fn module_mut(&mut self) -> &mut Module {
        &mut self.module
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_from_json() {
        let json = r#"{
            "format_version": "0.1.0",
            "module": {
                "id": "mod_test",
                "name": "test",
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
                "functions": []
            }
        }"#;

        let graph = IRGraph::from_json(json).unwrap();
        assert_eq!(graph.module().name(), "test");

        // Roundtrip
        let json_out = graph.to_json().unwrap();
        let graph2 = IRGraph::from_json(&json_out).unwrap();
        assert_eq!(graph.module, graph2.module);
    }
}
