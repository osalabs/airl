//! AIRL Compiler - JIT compilation of AIRL IR via Cranelift.
//!
//! Compiles IR modules to native machine code using Cranelift as the backend.
//! Currently supports JIT execution (compile in memory and run immediately).
//!
//! Supported types: I64, Bool, Unit
//! Supported nodes: Literal, Param, Let, If, BinOp, UnaryOp, Call, Return, Block

mod lower;

use airl_ir::module::Module;
use std::time::Instant;
use thiserror::Error;

/// Errors during compilation.
#[derive(Debug, Error)]
pub enum CompileError {
    #[error("no 'main' function found")]
    NoMainFunction,
    #[error("unsupported node type: {0}")]
    UnsupportedNode(String),
    #[error("unsupported type: {0}")]
    UnsupportedType(String),
    #[error("codegen error: {0}")]
    CodegenError(String),
    #[error("module error: {0}")]
    ModuleError(String),
    #[error("function not found: {0}")]
    FunctionNotFound(String),
}

/// Output of compiling and running a module.
#[derive(Debug, Clone)]
pub struct CompileOutput {
    pub stdout: String,
    pub exit_code: i32,
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
        let _lock = TEST_LOCK.lock().unwrap();
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
            &wrap_main(r#"{"id":"n1","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"n2","kind":"Literal","type":"I64","value":42}]}"#),
            "42\n",
        );
    }

    #[test]
    fn test_compile_arithmetic() {
        compile_and_assert(
            &wrap_main(r#"{"id":"n1","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"n2","kind":"BinOp","type":"I64","op":"Add","lhs":{"id":"n3","kind":"Literal","type":"I64","value":40},"rhs":{"id":"n4","kind":"Literal","type":"I64","value":2}}]}"#),
            "42\n",
        );
    }

    #[test]
    fn test_compile_if_else() {
        compile_and_assert(
            &wrap_main(r#"{"id":"n1","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"n2","kind":"If","type":"I64","cond":{"id":"n3","kind":"BinOp","type":"Bool","op":"Gt","lhs":{"id":"n4","kind":"Literal","type":"I64","value":10},"rhs":{"id":"n5","kind":"Literal","type":"I64","value":5}},"then_branch":{"id":"n6","kind":"Literal","type":"I64","value":1},"else_branch":{"id":"n7","kind":"Literal","type":"I64","value":0}}]}"#),
            "1\n",
        );
    }

    #[test]
    fn test_compile_let_binding() {
        compile_and_assert(
            &wrap_main(r#"{"id":"n1","kind":"Let","type":"Unit","name":"x","value":{"id":"n2","kind":"Literal","type":"I64","value":42},"body":{"id":"n3","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"n4","kind":"Param","type":"I64","name":"x","index":0}]}}"#),
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
            &wrap_main(r#"{"id":"n1","kind":"Block","type":"Unit","statements":[{"id":"s1","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"a1","kind":"Literal","type":"I64","value":1}]},{"id":"s2","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"a2","kind":"Literal","type":"I64","value":2}]}],"result":{"id":"n_end","kind":"Literal","type":"Unit","value":null}}"#),
            "1\n2\n",
        );
    }

    #[test]
    fn test_compile_unary_neg() {
        compile_and_assert(
            &wrap_main(r#"{"id":"n1","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"n2","kind":"UnaryOp","type":"I64","op":"Neg","operand":{"id":"n3","kind":"Literal","type":"I64","value":42}}]}"#),
            "-42\n",
        );
    }

    #[test]
    fn test_compile_string_println() {
        compile_and_assert(
            &wrap_main(r#"{"id":"n1","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"n2","kind":"Literal","type":"String","value":"hello world"}]}"#),
            "hello world\n",
        );
    }

    #[test]
    fn test_compile_multiple_comparisons() {
        compile_and_assert(
            &wrap_main(r#"{"id":"n1","kind":"Call","type":"Unit","target":"std::io::println","args":[{"id":"n2","kind":"BinOp","type":"I64","op":"Mod","lhs":{"id":"n3","kind":"Literal","type":"I64","value":15},"rhs":{"id":"n4","kind":"Literal","type":"I64","value":4}}]}"#),
            "3\n",
        );
    }
}
