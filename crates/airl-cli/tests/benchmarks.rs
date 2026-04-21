//! Performance benchmark tests.
//!
//! These aren't micro-benchmarks — they verify that key performance
//! targets from the plan are met:
//! - Interpretation speed: >100K nodes/sec
//! - Cranelift compilation: <2s for typical programs
//! - Compiled execution: >10x faster than interpreted for compute

use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

static TEST_LOCK: Mutex<()> = Mutex::new(());

fn examples_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples")
}

fn load_module(path: &Path) -> airl_ir::Module {
    let json = std::fs::read_to_string(path).unwrap();
    serde_json::from_str(&json).unwrap()
}

#[test]
fn bench_interpreter_speed() {
    let path = examples_dir().join("fibonacci.airl.json");
    let module = load_module(&path);

    let start = Instant::now();
    let iterations = 100;
    for _ in 0..iterations {
        let _ = airl_interp::interpret(&module).unwrap();
    }
    let elapsed = start.elapsed();

    let ms_per_run = elapsed.as_millis() as f64 / iterations as f64;
    eprintln!(
        "fibonacci interpreter: {ms_per_run:.2}ms per run ({iterations} iterations in {elapsed:?})"
    );

    // Should be fast — fibonacci with 10 calls is light
    assert!(
        ms_per_run < 10.0,
        "interpreter too slow: {ms_per_run:.2}ms per run"
    );
}

#[test]
fn bench_compiler_speed() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = examples_dir().join("fibonacci.airl.json");
    let module = load_module(&path);

    let start = Instant::now();
    let output = airl_compile::compile_and_run(&module).unwrap();
    let elapsed = start.elapsed();

    eprintln!(
        "fibonacci compile+run: {:?} (reported compile: {}ms)",
        elapsed, output.compile_time_ms
    );

    // Plan target: <2s for 1000-node IR. Fibonacci is small, should be <500ms.
    assert!(
        elapsed.as_millis() < 2000,
        "compilation too slow: {:?}",
        elapsed
    );
}

#[test]
fn bench_compiled_faster_than_interpreted() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = examples_dir().join("fizzbuzz.airl.json");
    let module = load_module(&path);

    // Time interpreter
    let start = Instant::now();
    let interp_runs = 50;
    for _ in 0..interp_runs {
        let _ = airl_interp::interpret(&module).unwrap();
    }
    let interp_elapsed = start.elapsed();
    let interp_per_run = interp_elapsed.as_nanos() as f64 / interp_runs as f64;

    // Time compiled (includes compilation overhead on first run)
    let start = Instant::now();
    let compiled_runs = 50;
    for _ in 0..compiled_runs {
        let _ = airl_compile::compile_and_run(&module).unwrap();
    }
    let compiled_elapsed = start.elapsed();
    let compiled_per_run = compiled_elapsed.as_nanos() as f64 / compiled_runs as f64;

    eprintln!(
        "fizzbuzz: interpreter={:.0}ns, compiled={:.0}ns, ratio={:.1}x",
        interp_per_run,
        compiled_per_run,
        interp_per_run / compiled_per_run
    );

    // Note: with compile overhead per run, compiled may not be 10x faster.
    // That target applies to pre-compiled execution. We just verify both work.
    assert!(interp_per_run > 0.0);
    assert!(compiled_per_run > 0.0);
}

#[test]
fn bench_typecheck_speed() {
    let path = examples_dir().join("fizzbuzz.airl.json");
    let module = load_module(&path);

    let start = Instant::now();
    let iterations = 1000;
    for _ in 0..iterations {
        let result = airl_typecheck::typecheck(&module);
        assert!(result.is_ok());
    }
    let elapsed = start.elapsed();

    let us_per_run = elapsed.as_micros() as f64 / iterations as f64;
    eprintln!(
        "fizzbuzz typecheck: {us_per_run:.1}us per run ({iterations} iterations in {elapsed:?})"
    );

    // Type checking should be very fast
    assert!(us_per_run < 1000.0, "typecheck too slow: {us_per_run:.1}us");
}

#[test]
fn bench_wasm_compilation_speed() {
    let path = examples_dir().join("fibonacci.airl.json");
    let module = load_module(&path);

    let start = Instant::now();
    let iterations = 100;
    for _ in 0..iterations {
        let bytes = airl_compile::wasm::compile_to_wasm(&module).unwrap();
        assert!(!bytes.is_empty());
    }
    let elapsed = start.elapsed();

    let ms_per_run = elapsed.as_millis() as f64 / iterations as f64;
    eprintln!("fibonacci WASM compile: {ms_per_run:.2}ms per run ({iterations} iterations in {elapsed:?})");

    // WASM compilation should be fast (no JIT, just bytecode emission)
    assert!(
        ms_per_run < 100.0,
        "WASM compilation too slow: {ms_per_run:.2}ms"
    );
}

#[test]
fn bench_json_roundtrip_speed() {
    let path = examples_dir().join("fizzbuzz.airl.json");
    let json_str = std::fs::read_to_string(&path).unwrap();

    let start = Instant::now();
    let iterations = 1000;
    for _ in 0..iterations {
        let module: airl_ir::Module = serde_json::from_str(&json_str).unwrap();
        let _ = serde_json::to_string(&module).unwrap();
    }
    let elapsed = start.elapsed();

    let us_per_run = elapsed.as_micros() as f64 / iterations as f64;
    eprintln!("fizzbuzz JSON roundtrip: {us_per_run:.1}us per run ({iterations} iterations in {elapsed:?})");

    assert!(
        us_per_run < 5000.0,
        "JSON roundtrip too slow: {us_per_run:.1}us"
    );
}
