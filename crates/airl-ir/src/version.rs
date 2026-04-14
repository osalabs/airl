use crate::module::Module;
use sha2::{Digest, Sha256};
use std::fmt;

/// A content-addressable version identifier computed from module contents.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct VersionId(pub [u8; 32]);

impl VersionId {
    /// Compute a version ID from a module by hashing its JSON representation.
    pub fn compute(module: &Module) -> Self {
        let json = serde_json::to_string(module).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(json.as_bytes());
        let result = hasher.finalize();
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&result);
        VersionId(bytes)
    }

    /// Get the version ID as a hex string.
    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }
}

impl fmt::Debug for VersionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VersionId({})", &self.to_hex()[..16])
    }
}

impl fmt::Display for VersionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_deterministic() {
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

        let module: Module = serde_json::from_str(json).unwrap();
        let v1 = VersionId::compute(&module);
        let v2 = VersionId::compute(&module);
        assert_eq!(v1, v2);
        assert_eq!(v1.to_hex().len(), 64);
    }
}
