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
    assert!(count >= 3, "Expected at least 3 example files, found {count}");
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
    let _lock = TEST_LOCK.lock().unwrap();
    let path = examples_dir().join("hello.airl.json");
    let module = load_module(&path);

    let interp = airl_interp::interpret(&module).unwrap().stdout;
    let compiled = airl_compile::compile_and_run(&module).unwrap().stdout;

    assert_eq!(interp, compiled, "hello: compiled != interpreted");
}

/// Golden: fibonacci.airl.json compiled output == interpreted output.
#[test]
fn golden_fibonacci_compiled_matches_interpreted() {
    let _lock = TEST_LOCK.lock().unwrap();
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
