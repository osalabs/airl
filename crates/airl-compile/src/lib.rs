//! AIRL Compiler - JIT compilation of AIRL IR via Cranelift, plus a WASM backend.
//!
//! Compiles IR modules to native machine code using Cranelift (JIT), or to
//! portable WebAssembly bytecode via [`wasm`].
//!
//! # Example (Cranelift JIT)
//!
//! ```no_run
//! use airl_compile::compile_and_run;
//! use airl_ir::Module;
//!
//! let module: Module = serde_json::from_str("...").unwrap();
//! let output = compile_and_run(&module).unwrap();
//! print!("{}", output.stdout);
//! ```
//!
//! # Example (WASM)
//!
//! ```no_run
//! use airl_compile::wasm::compile_to_wasm;
//! use airl_ir::Module;
//!
//! let module: Module = serde_json::from_str("...").unwrap();
//! let bytes = compile_to_wasm(&module).unwrap();
//! std::fs::write("out.wasm", &bytes).unwrap();
//! ```

#![warn(missing_docs)]

mod lower;
pub mod wasm;

use airl_ir::module::Module;
use std::time::Instant;
use thiserror::Error;

/// Errors returned by the compiler.
#[derive(Debug, Error)]
pub enum CompileError {
    /// The module has no `main` function to use as an entry point.
    #[error("no 'main' function found")]
    NoMainFunction,
    /// Encountered an IR node the compiler doesn't support.
    #[error("unsupported node type: {0}")]
    UnsupportedNode(String),
    /// Encountered a type the compiler doesn't support.
    #[error("unsupported type: {0}")]
    UnsupportedType(String),
    /// Cranelift codegen or WASM emission failed.
    #[error("codegen error: {0}")]
    CodegenError(String),
    /// Module-level error in Cranelift's module layer.
    #[error("module error: {0}")]
    ModuleError(String),
    /// A referenced function was not found in the module.
    #[error("function not found: {0}")]
    FunctionNotFound(String),
}

/// Output of compiling and running a module via Cranelift JIT.
#[derive(Debug, Clone)]
pub struct CompileOutput {
    /// Captured standard output.
    pub stdout: String,
    /// Process-style exit code.
    pub exit_code: i32,
    /// Compilation time in milliseconds (excludes execution time).
    pub compile_time_ms: u64,
}

/// Compile an AIRL module to native code via Cranelift JIT and execute it.
///
/// Currently supports integer-only programs (I64, Bool, Unit).
/// String arguments to println are handled by passing them as pre-registered
/// string table indices.
pub fn compile_and_run(module: &Module) -> Result<CompileOutput, CompileError> {
    let start = Instant::now();

    let stdout = lower::jit_compile_and_run(module)?;

    let compile_time_ms = start.elapsed().as_millis() as u64;

    Ok(CompileOutput {
        stdout,
        exit_code: 0,
        compile_time_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // JIT compilation uses global state for stdout capture,
    // so tests must run sequentially.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn load_module(json: &str) -> Module {
        serde_json::from_str(json).unwrap()
    }

    fn compile_and_assert(json: &str, expected: &str) {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let module = load_module(json);
        let output = compile_and_run(&module).unwrap();
        assert_eq!(output.stdout, expected);
    }

    fn wrap_main(body: &str) -> String {
        format!(
            r#"{{"format_version":"0.1.0","module":{{"id":"m","name":"main","metadata":{{"version":"1","description":"","author":"","created_at":""}},"imports":[],"exports":[],"types":[],"functions":[{{"id":"f_main","name":"main","params":[],"returns":"Unit","effects":["IO"],"body":{body}}}]}}}}"#
        )
    }

    fn wrap_with_functions(body: &str, extra: &str) -> String {
        format!(
            r#"{{"format_version":"0.1.0","module":{{"id":"m","name":"main","metadata":{{"version":"1","description":"","author":"","created_at":""}},"imports":[],"exports":[],"types":[],"functions":[{{"id":"f_main","name":"main","params":[],"returns":"Unit","effects":["IO"],"body":{body}}},{extra}]}}}}"#
        )
    }

    #[test]
    fn test_compile_integer_println() {
        compile_and_assert(
            &wrap_main(
                r#"{"id":"n1","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"n2","kind":"Literal","type":"I64","value":42}]}"#,
            ),
            "42\n",
        );
    }

    #[test]
    fn test_compile_arithmetic() {
        compile_and_assert(
            &wrap_main(
                r#"{"id":"n1","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"n2","kind":"BinOp","type":"I64","op":"Add","lhs":{"id":"n3","kind":"Literal","type":"I64","value":40},"rhs":{"id":"n4","kind":"Literal","type":"I64","value":2}}]}"#,
            ),
            "42\n",
        );
    }

    #[test]
    fn test_compile_if_else() {
        compile_and_assert(
            &wrap_main(
                r#"{"id":"n1","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"n2","kind":"If","type":"I64","cond":{"id":"n3","kind":"BinOp","type":"Bool","op":"Gt","lhs":{"id":"n4","kind":"Literal","type":"I64","value":10},"rhs":{"id":"n5","kind":"Literal","type":"I64","value":5}},"then_branch":{"id":"n6","kind":"Literal","type":"I64","value":1},"else_branch":{"id":"n7","kind":"Literal","type":"I64","value":0}}]}"#,
            ),
            "1\n",
        );
    }

    #[test]
    fn test_compile_let_binding() {
        compile_and_assert(
            &wrap_main(
                r#"{"id":"n1","kind":"Let","type":"Unit","name":"x","value":{"id":"n2","kind":"Literal","type":"I64","value":42},"body":{"id":"n3","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"n4","kind":"Param","type":"I64","name":"x","index":0}]}}"#,
            ),
            "42\n",
        );
    }

    #[test]
    fn test_compile_user_function() {
        let body = r#"{"id":"n1","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"n2","kind":"Call","type":"I64","target":"double","args":[{"id":"n3","kind":"Literal","type":"I64","value":21}]}]}"#;
        let double = r#"{"id":"f_double","name":"double","params":[{"name":"n","type":"I64","index":0}],"returns":"I64","effects":["Pure"],"body":{"id":"d1","kind":"BinOp","type":"I64","op":"Mul","lhs":{"id":"d2","kind":"Param","type":"I64","name":"n","index":0},"rhs":{"id":"d3","kind":"Literal","type":"I64","value":2}}}"#;
        compile_and_assert(&wrap_with_functions(body, double), "42\n");
    }

    #[test]
    fn test_compile_recursive_fibonacci() {
        let body = r#"{"id":"n1","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"n2","kind":"Call","type":"I64","target":"fib","args":[{"id":"n3","kind":"Literal","type":"I64","value":10}]}]}"#;
        let fib = r#"{"id":"f_fib","name":"fib","params":[{"name":"n","type":"I64","index":0}],"returns":"I64","effects":["Pure"],"body":{"id":"f1","kind":"If","type":"I64","cond":{"id":"f2","kind":"BinOp","type":"Bool","op":"Lte","lhs":{"id":"f3","kind":"Param","type":"I64","name":"n","index":0},"rhs":{"id":"f4","kind":"Literal","type":"I64","value":1}},"then_branch":{"id":"f5","kind":"Param","type":"I64","name":"n","index":0},"else_branch":{"id":"f6","kind":"BinOp","type":"I64","op":"Add","lhs":{"id":"f7","kind":"Call","type":"I64","target":"fib","args":[{"id":"f8","kind":"BinOp","type":"I64","op":"Sub","lhs":{"id":"f9","kind":"Param","type":"I64","name":"n","index":0},"rhs":{"id":"f10","kind":"Literal","type":"I64","value":1}}]},"rhs":{"id":"f11","kind":"Call","type":"I64","target":"fib","args":[{"id":"f12","kind":"BinOp","type":"I64","op":"Sub","lhs":{"id":"f13","kind":"Param","type":"I64","name":"n","index":0},"rhs":{"id":"f14","kind":"Literal","type":"I64","value":2}}]}}}}"#;
        compile_and_assert(&wrap_with_functions(body, fib), "55\n");
    }

    #[test]
    fn test_compile_block() {
        compile_and_assert(
            &wrap_main(
                r#"{"id":"n1","kind":"Block","type":"Unit","statements":[{"id":"s1","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"a1","kind":"Literal","type":"I64","value":1}]},{"id":"s2","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"a2","kind":"Literal","type":"I64","value":2}]}],"result":{"id":"n_end","kind":"Literal","type":"Unit","value":null}}"#,
            ),
            "1\n2\n",
        );
    }

    #[test]
    fn test_compile_unary_neg() {
        compile_and_assert(
            &wrap_main(
                r#"{"id":"n1","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"n2","kind":"UnaryOp","type":"I64","op":"Neg","operand":{"id":"n3","kind":"Literal","type":"I64","value":42}}]}"#,
            ),
            "-42\n",
        );
    }

    #[test]
    fn test_compile_string_println() {
        compile_and_assert(
            &wrap_main(
                r#"{"id":"n1","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"n2","kind":"Literal","type":"String","value":"hello world"}]}"#,
            ),
            "hello world\n",
        );
    }

    #[test]
    fn test_compile_multiple_comparisons() {
        compile_and_assert(
            &wrap_main(
                r#"{"id":"n1","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"n2","kind":"BinOp","type":"I64","op":"Mod","lhs":{"id":"n3","kind":"Literal","type":"I64","value":15},"rhs":{"id":"n4","kind":"Literal","type":"I64","value":4}}]}"#,
            ),
            "3\n",
        );
    }

    // --- New builtin tests ---

    #[test]
    fn test_compile_str_from_i64_println() {
        // println(string::from_i64(42)) should print "42\n"
        compile_and_assert(
            &wrap_main(
                r#"{"id":"n1","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"n2","kind":"Call","type":"String","target":"std::string::from_i64","args":[{"id":"n3","kind":"Literal","type":"I64","value":42}]}]}"#,
            ),
            "42\n",
        );
    }

    #[test]
    fn test_compile_str_concat() {
        // println(string::concat("hello", " world"))
        compile_and_assert(
            &wrap_main(
                r#"{"id":"n1","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"n2","kind":"Call","type":"String","target":"std::string::concat","args":[{"id":"n3","kind":"Literal","type":"String","value":"hello"},{"id":"n4","kind":"Literal","type":"String","value":" world"}]}]}"#,
            ),
            "hello world\n",
        );
    }

    #[test]
    fn test_compile_str_len() {
        // println(string::len("hello"))
        compile_and_assert(
            &wrap_main(
                r#"{"id":"n1","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"n2","kind":"Call","type":"I64","target":"std::string::len","args":[{"id":"n3","kind":"Literal","type":"String","value":"hello"}]}]}"#,
            ),
            "5\n",
        );
    }

    #[test]
    fn test_compile_math_abs() {
        compile_and_assert(
            &wrap_main(
                r#"{"id":"n1","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"n2","kind":"Call","type":"I64","target":"std::math::abs","args":[{"id":"n3","kind":"UnaryOp","type":"I64","op":"Neg","operand":{"id":"n4","kind":"Literal","type":"I64","value":42}}]}]}"#,
            ),
            "42\n",
        );
    }

    #[test]
    fn test_compile_math_max_min() {
        compile_and_assert(
            &wrap_main(
                r#"{"id":"b1","kind":"Block","type":"Unit","statements":[{"id":"n1","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"n2","kind":"Call","type":"I64","target":"std::math::max","args":[{"id":"n3","kind":"Literal","type":"I64","value":10},{"id":"n4","kind":"Literal","type":"I64","value":20}]}]},{"id":"n5","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"n6","kind":"Call","type":"I64","target":"std::math::min","args":[{"id":"n7","kind":"Literal","type":"I64","value":10},{"id":"n8","kind":"Literal","type":"I64","value":20}]}]}],"result":{"id":"e","kind":"Literal","type":"Unit","value":null}}"#,
            ),
            "20\n10\n",
        );
    }

    // --- WASM compilation tests ---

    #[test]
    fn test_wasm_hello_world() {
        let json = wrap_main(
            r#"{"id":"n1","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"n2","kind":"Literal","type":"String","value":"hello world"}]}"#,
        );
        let module = load_module(&json);
        let wasm_bytes = crate::wasm::compile_to_wasm(&module).unwrap();
        // Verify it's a valid WASM module
        assert!(
            wasm_bytes.starts_with(b"\0asm"),
            "should start with WASM magic bytes"
        );
        assert!(
            wasm_bytes.len() > 20,
            "WASM module should have substantial content"
        );
        // Validate with wasmparser
        wasmparser::validate(&wasm_bytes).expect("generated WASM should be valid");
    }

    #[test]
    fn test_wasm_arithmetic() {
        let body = r#"{"id":"n1","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"n2","kind":"BinOp","type":"I64","op":"Add","lhs":{"id":"n3","kind":"Literal","type":"I64","value":40},"rhs":{"id":"n4","kind":"Literal","type":"I64","value":2}}]}"#;
        let json = wrap_main(body);
        let module = load_module(&json);
        let wasm_bytes = crate::wasm::compile_to_wasm(&module).unwrap();
        wasmparser::validate(&wasm_bytes).expect("generated WASM should be valid");
    }

    #[test]
    fn test_wasm_user_function() {
        let body = r#"{"id":"n1","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"n2","kind":"Call","type":"I64","target":"double","args":[{"id":"n3","kind":"Literal","type":"I64","value":21}]}]}"#;
        let double = r#"{"id":"f_double","name":"double","params":[{"name":"n","type":"I64","index":0}],"returns":"I64","effects":["Pure"],"body":{"id":"d1","kind":"BinOp","type":"I64","op":"Mul","lhs":{"id":"d2","kind":"Param","type":"I64","name":"n","index":0},"rhs":{"id":"d3","kind":"Literal","type":"I64","value":2}}}"#;
        let json = wrap_with_functions(body, double);
        let module = load_module(&json);
        let wasm_bytes = crate::wasm::compile_to_wasm(&module).unwrap();
        wasmparser::validate(&wasm_bytes).expect("generated WASM should be valid");
    }

    #[test]
    fn test_wasm_if_else() {
        let body = r#"{"id":"n1","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"n2","kind":"If","type":"I64","cond":{"id":"n3","kind":"BinOp","type":"Bool","op":"Gt","lhs":{"id":"n4","kind":"Literal","type":"I64","value":10},"rhs":{"id":"n5","kind":"Literal","type":"I64","value":5}},"then_branch":{"id":"n6","kind":"Literal","type":"I64","value":1},"else_branch":{"id":"n7","kind":"Literal","type":"I64","value":0}}]}"#;
        let json = wrap_main(body);
        let module = load_module(&json);
        let wasm_bytes = crate::wasm::compile_to_wasm(&module).unwrap();
        wasmparser::validate(&wasm_bytes).expect("generated WASM should be valid");
    }

    #[test]
    fn test_wasm_recursive_fibonacci() {
        let body = r#"{"id":"n1","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"n2","kind":"Call","type":"I64","target":"fib","args":[{"id":"n3","kind":"Literal","type":"I64","value":10}]}]}"#;
        let fib = r#"{"id":"f_fib","name":"fib","params":[{"name":"n","type":"I64","index":0}],"returns":"I64","effects":["Pure"],"body":{"id":"f1","kind":"If","type":"I64","cond":{"id":"f2","kind":"BinOp","type":"Bool","op":"Lte","lhs":{"id":"f3","kind":"Param","type":"I64","name":"n","index":0},"rhs":{"id":"f4","kind":"Literal","type":"I64","value":1}},"then_branch":{"id":"f5","kind":"Param","type":"I64","name":"n","index":0},"else_branch":{"id":"f6","kind":"BinOp","type":"I64","op":"Add","lhs":{"id":"f7","kind":"Call","type":"I64","target":"fib","args":[{"id":"f8","kind":"BinOp","type":"I64","op":"Sub","lhs":{"id":"f9","kind":"Param","type":"I64","name":"n","index":0},"rhs":{"id":"f10","kind":"Literal","type":"I64","value":1}}]},"rhs":{"id":"f11","kind":"Call","type":"I64","target":"fib","args":[{"id":"f12","kind":"BinOp","type":"I64","op":"Sub","lhs":{"id":"f13","kind":"Param","type":"I64","name":"n","index":0},"rhs":{"id":"f14","kind":"Literal","type":"I64","value":2}}]}}}}"#;
        let json = wrap_with_functions(body, fib);
        let module = load_module(&json);
        let wasm_bytes = crate::wasm::compile_to_wasm(&module).unwrap();
        wasmparser::validate(&wasm_bytes).expect("generated WASM should be valid");
    }

    // --- JIT Match compilation test ---

    #[test]
    fn test_compile_match_literal() {
        // match 2 { 1 => 10, 2 => 20, _ => 0 } => should return 20
        let body = r#"{"id":"n1","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"n2","kind":"Match","type":"I64","scrutinee":{"id":"n3","kind":"Literal","type":"I64","value":2},"arms":[{"pattern":{"kind":"Literal","value":1},"body":{"id":"a1","kind":"Literal","type":"I64","value":10}},{"pattern":{"kind":"Literal","value":2},"body":{"id":"a2","kind":"Literal","type":"I64","value":20}},{"pattern":{"kind":"Wildcard"},"body":{"id":"a3","kind":"Literal","type":"I64","value":0}}]}]}"#;
        compile_and_assert(&wrap_main(body), "20\n");
    }

    // --- JIT fizzbuzz compilation test (uses string handles) ---

    #[test]
    fn test_compile_fizzbuzz_example() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let json = std::fs::read_to_string("../../../examples/fizzbuzz.airl.json")
            .unwrap_or_else(|_| include_str!("../../../examples/fizzbuzz.airl.json").to_string());
        let module = load_module(&json);
        let output = compile_and_run(&module).unwrap();
        // FizzBuzz 1-20: first lines should be 1, 2, Fizz, 4, Buzz, Fizz, ...
        let expected_start = "1\n2\nFizz\n4\nBuzz\nFizz\n";
        assert!(
            output.stdout.starts_with(expected_start),
            "unexpected fizzbuzz output (first 200 chars): {:?}",
            &output.stdout[..output.stdout.len().min(200)]
        );
        assert!(output.stdout.contains("FizzBuzz\n"));
    }

    // --- WASM Match test ---

    #[test]
    fn test_wasm_match() {
        let body = r#"{"id":"n1","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"n2","kind":"Match","type":"I64","scrutinee":{"id":"n3","kind":"Literal","type":"I64","value":2},"arms":[{"pattern":{"kind":"Literal","value":1},"body":{"id":"a1","kind":"Literal","type":"I64","value":10}},{"pattern":{"kind":"Literal","value":2},"body":{"id":"a2","kind":"Literal","type":"I64","value":20}},{"pattern":{"kind":"Wildcard"},"body":{"id":"a3","kind":"Literal","type":"I64","value":0}}]}]}"#;
        let json = wrap_main(body);
        let module = load_module(&json);
        let wasm_bytes = crate::wasm::compile_to_wasm(&module).unwrap();
        wasmparser::validate(&wasm_bytes).expect("generated WASM should be valid");
    }
}
