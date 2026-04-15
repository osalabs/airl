//! Golden tests: verify compiled output matches interpreted output,
//! and all examples pass type checking.

use std::path::Path;
use std::sync::Mutex;

// JIT compilation uses global state, so these tests must be serialized.
static TEST_LOCK: Mutex<()> = Mutex::new(());

fn examples_dir() -> std::path::PathBuf {
    // Navigate from crates/airl-cli to repo root
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent() // crates/
        .unwrap()
        .parent() // repo root
        .unwrap()
        .join("examples")
}

fn load_module(path: &Path) -> airl_ir::Module {
    let json = std::fs::read_to_string(path).unwrap();
    serde_json::from_str(&json).unwrap()
}

/// Verify all examples pass type checking.
#[test]
fn all_examples_typecheck() {
    let dir = examples_dir();
    let mut count = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.to_string_lossy().ends_with(".airl.json") {
            let module = load_module(&path);
            let result = airl_typecheck::typecheck(&module);
            assert!(
                result.is_ok(),
                "Type check failed for {}: {:?}",
                path.display(),
                result.errors.iter().map(|e| &e.message).collect::<Vec<_>>()
            );
            count += 1;
        }
    }
    assert!(
        count >= 3,
        "Expected at least 3 example files, found {count}"
    );
}

/// Verify all examples run through the interpreter.
#[test]
fn all_examples_interpret() {
    let dir = examples_dir();
    for entry in std::fs::read_dir(&dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.to_string_lossy().ends_with(".airl.json") {
            let module = load_module(&path);
            let output = airl_interp::interpret(&module).unwrap();
            assert!(
                !output.stdout.is_empty(),
                "Expected output from {}, got empty",
                path.display()
            );
        }
    }
}

/// Golden: hello.airl.json compiled output == interpreted output.
#[test]
fn golden_hello_compiled_matches_interpreted() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = examples_dir().join("hello.airl.json");
    let module = load_module(&path);

    let interp = airl_interp::interpret(&module).unwrap().stdout;
    let compiled = airl_compile::compile_and_run(&module).unwrap().stdout;

    assert_eq!(interp, compiled, "hello: compiled != interpreted");
}

/// Golden: fibonacci.airl.json compiled output == interpreted output.
#[test]
fn golden_fibonacci_compiled_matches_interpreted() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = examples_dir().join("fibonacci.airl.json");
    let module = load_module(&path);

    let interp = airl_interp::interpret(&module).unwrap().stdout;
    let compiled = airl_compile::compile_and_run(&module).unwrap().stdout;

    assert_eq!(interp, compiled, "fibonacci: compiled != interpreted");
}

/// Verify the interpreter output matches expected values.
#[test]
fn hello_output_correct() {
    let path = examples_dir().join("hello.airl.json");
    let module = load_module(&path);
    let output = airl_interp::interpret(&module).unwrap();
    assert_eq!(output.stdout, "hello world\n");
}

#[test]
fn fibonacci_output_correct() {
    let path = examples_dir().join("fibonacci.airl.json");
    let module = load_module(&path);
    let output = airl_interp::interpret(&module).unwrap();
    assert_eq!(output.stdout, "0\n1\n1\n2\n3\n5\n8\n13\n21\n34\n");
}

#[test]
fn fizzbuzz_output_correct() {
    let path = examples_dir().join("fizzbuzz.airl.json");
    let module = load_module(&path);
    let output = airl_interp::interpret(&module).unwrap();
    assert!(output.stdout.starts_with("1\n2\nFizz\n4\nBuzz\n"));
    assert!(output.stdout.contains("FizzBuzz\n"));
    // FizzBuzz for 1-20: 20 lines
    assert_eq!(output.stdout.lines().count(), 20);
}

// --- Golden: compiled == interpreted for all compilable examples ---

#[test]
fn golden_fizzbuzz_compiled_matches_interpreted() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = examples_dir().join("fizzbuzz.airl.json");
    let module = load_module(&path);

    let interp = airl_interp::interpret(&module).unwrap().stdout;
    let compiled = airl_compile::compile_and_run(&module).unwrap().stdout;

    assert_eq!(interp, compiled, "fizzbuzz: compiled != interpreted");
}

#[test]
fn golden_string_ops_compiled_matches_interpreted() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = examples_dir().join("string_ops.airl.json");
    let module = load_module(&path);

    let interp = airl_interp::interpret(&module).unwrap().stdout;
    let compiled = airl_compile::compile_and_run(&module).unwrap().stdout;

    assert_eq!(interp, compiled, "string_ops: compiled != interpreted");
}

// --- New example output correctness tests ---

#[test]
fn json_processor_output_correct() {
    let path = examples_dir().join("json_processor.airl.json");
    let module = load_module(&path);
    let output = airl_interp::interpret(&module).unwrap();
    assert!(output.stdout.contains("=== JSON Processor ==="));
    assert!(output.stdout.contains("\"name\""));
    assert!(output.stdout.contains("AIRL"));
    assert!(output.stdout.contains("--- Pretty ---"));
}

#[test]
fn kv_store_output_correct() {
    let path = examples_dir().join("kv_store.airl.json");
    let module = load_module(&path);
    let output = airl_interp::interpret(&module).unwrap();
    assert!(output.stdout.contains("=== Key-Value Store ==="));
    assert!(output.stdout.contains("entries: 3"));
    assert!(output.stdout.contains("language = AIRL"));
    assert!(output.stdout.contains("has backend? true"));
    assert!(output.stdout.contains("keys:"));
}

#[test]
fn file_search_output_correct() {
    // This test must run from the repo root where `examples/` exists.
    // Set the working directory to the repo root for this test.
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let _prev = std::env::current_dir().unwrap();
    std::env::set_current_dir(repo_root).unwrap();

    let path = examples_dir().join("file_search.airl.json");
    let module = load_module(&path);
    let output = airl_interp::interpret(&module).unwrap();
    assert!(output.stdout.contains("=== File Search Tool ==="));
    assert!(output.stdout.contains("hello.airl.json exists? true"));
    assert!(output.stdout.contains("nonexistent.txt exists? false"));

    // Restore
    std::env::set_current_dir(_prev).unwrap();
}

// --- WASM compilation validation for all examples ---

#[test]
fn all_examples_produce_valid_wasm() {
    let dir = examples_dir();
    let mut count = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.to_string_lossy().ends_with(".airl.json") {
            let module = load_module(&path);
            let wasm_bytes = airl_compile::wasm::compile_to_wasm(&module).unwrap();
            assert!(
                wasm_bytes.starts_with(b"\0asm"),
                "WASM magic bytes missing for {}",
                path.display()
            );
            assert!(
                wasm_bytes.len() > 20,
                "WASM too small for {}",
                path.display()
            );
            count += 1;
        }
    }
    assert!(
        count >= 5,
        "Expected at least 5 WASM files, compiled {count}"
    );
}

// --- Text projection tests for all examples ---

#[test]
fn all_examples_project_to_typescript() {
    use airl_project::projection::{project_module, Language};
    let dir = examples_dir();
    for entry in std::fs::read_dir(&dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.to_string_lossy().ends_with(".airl.json") {
            let module = load_module(&path);
            let ts = project_module(&module, Language::TypeScript);
            assert!(
                ts.contains("function "),
                "TypeScript projection for {} should contain 'function'",
                path.display()
            );
        }
    }
}

#[test]
fn all_examples_project_to_python() {
    use airl_project::projection::{project_module, Language};
    let dir = examples_dir();
    for entry in std::fs::read_dir(&dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.to_string_lossy().ends_with(".airl.json") {
            let module = load_module(&path);
            let py = project_module(&module, Language::Python);
            assert!(
                py.contains("def "),
                "Python projection for {} should contain 'def'",
                path.display()
            );
        }
    }
}
