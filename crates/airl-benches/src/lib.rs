//! Shared helpers for AIRL criterion benchmarks.
//!
//! The benches themselves live in `benches/`. This library exposes
//! an `examples_dir()` helper and a few module fixtures so each
//! bench file doesn't duplicate boilerplate.

use airl_ir::Module;
use std::path::{Path, PathBuf};

/// Path to the `examples/` directory at the repository root.
pub fn examples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .parent()
        .expect("repo root")
        .join("examples")
}

/// Load one of the example modules by filename (e.g. `"fibonacci.airl.json"`).
pub fn example_module(name: &str) -> Module {
    let path = examples_dir().join(name);
    let json = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    serde_json::from_str(&json).expect("valid AIRL JSON")
}

/// Load an example module as raw JSON (no deserialization).
pub fn example_json(name: &str) -> String {
    let path = examples_dir().join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
}
