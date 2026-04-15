//! Property-based tests for the AIRL interpreter (no external deps).
//!
//! Generates random well-formed programs and verifies:
//! - Interpreter doesn't panic on valid programs
//! - Arithmetic is consistent
//! - String builtins produce valid output
//! - Division by zero returns error, not panic

use airl_ir::effects::Effect;
use airl_ir::ids::*;
use airl_ir::module::*;
use airl_ir::node::*;
use airl_ir::types::Type;

fn make_module(body: Node) -> Module {
    Module {
        format_version: "0.1.0".to_string(),
        module: ModuleInner {
            id: ModuleId::new("m"),
            name: "main".to_string(),
            metadata: ModuleMetadata {
                version: "1".to_string(),
                description: String::new(),
                author: String::new(),
                created_at: String::new(),
            },
            imports: vec![],
            exports: vec![],
            types: vec![],
            traits: vec![],
            impls: vec![],
            constants: vec![],
            functions: vec![FuncDef {
                id: FuncId::new("f_main"),
                name: "main".to_string(),
                params: vec![],
                returns: Type::Unit,
                effects: vec![Effect::IO],
                body,
            }],
        },
    }
}

fn println_node(arg: Node) -> Node {
    Node::Call {
        id: NodeId::new("c"),
        node_type: Type::Unit,
        target: "std::io::println".to_string(),
        args: vec![arg],
    }
}

fn int_lit(val: i64) -> Node {
    Node::Literal {
        id: NodeId::new("v"),
        node_type: Type::I64,
        value: LiteralValue::Integer(val),
    }
}

fn str_lit(val: &str) -> Node {
    Node::Literal {
        id: NodeId::new("s"),
        node_type: Type::String,
        value: LiteralValue::Str(val.to_string()),
    }
}

// --- Tests ---

#[test]
fn prop_print_integers() {
    let test_values: Vec<i64> = vec![
        0,
        1,
        -1,
        42,
        -42,
        100,
        -100,
        1000000,
        -1000000,
        i64::MAX,
        i64::MIN,
        i64::MAX - 1,
        i64::MIN + 1,
    ];
    for val in test_values {
        let body = println_node(int_lit(val));
        let module = make_module(body);
        let output = airl_interp::interpret(&module).unwrap();
        assert_eq!(output.stdout, format!("{val}\n"), "failed for {val}");
    }
}

#[test]
fn prop_print_strings() {
    let test_strings = ["", "hello", "hello world", "a b c", "!@#$%", "\t\n"];
    for s in &test_strings {
        let body = println_node(str_lit(s));
        let module = make_module(body);
        let output = airl_interp::interpret(&module).unwrap();
        assert_eq!(output.stdout, format!("{s}\n"), "failed for {s:?}");
    }
}

#[test]
fn prop_addition_consistency() {
    let pairs: Vec<(i64, i64)> = vec![
        (0, 0),
        (1, 1),
        (-1, 1),
        (100, -100),
        (i64::MAX - 1, 1),
        (1000, 2000),
        (-500, -300),
        (0, i64::MAX),
    ];
    for (a, b) in pairs {
        let body = println_node(Node::BinOp {
            id: NodeId::new("add"),
            op: BinOpKind::Add,
            node_type: Type::I64,
            lhs: Box::new(int_lit(a)),
            rhs: Box::new(int_lit(b)),
        });
        let module = make_module(body);
        let output = airl_interp::interpret(&module).unwrap();
        assert_eq!(
            output.stdout,
            format!("{}\n", a.wrapping_add(b)),
            "add({a}, {b})"
        );
    }
}

#[test]
fn prop_multiplication_consistency() {
    let pairs: Vec<(i64, i64)> = vec![(0, 100), (1, 42), (-1, 42), (7, 6), (-3, -5), (100, 100)];
    for (a, b) in pairs {
        let body = println_node(Node::BinOp {
            id: NodeId::new("mul"),
            op: BinOpKind::Mul,
            node_type: Type::I64,
            lhs: Box::new(int_lit(a)),
            rhs: Box::new(int_lit(b)),
        });
        let module = make_module(body);
        let output = airl_interp::interpret(&module).unwrap();
        assert_eq!(
            output.stdout,
            format!("{}\n", a.wrapping_mul(b)),
            "mul({a}, {b})"
        );
    }
}

#[test]
fn prop_division_by_zero_errors() {
    for a in [-100i64, -1, 0, 1, 42, 100, i64::MAX] {
        let body = println_node(Node::BinOp {
            id: NodeId::new("div"),
            op: BinOpKind::Div,
            node_type: Type::I64,
            lhs: Box::new(int_lit(a)),
            rhs: Box::new(int_lit(0)),
        });
        let module = make_module(body);
        let result = airl_interp::interpret(&module);
        assert!(result.is_err(), "div({a}, 0) should error");
    }
}

#[test]
fn prop_mod_by_zero_errors() {
    for a in [-100i64, 0, 1, 100] {
        let body = println_node(Node::BinOp {
            id: NodeId::new("mod"),
            op: BinOpKind::Mod,
            node_type: Type::I64,
            lhs: Box::new(int_lit(a)),
            rhs: Box::new(int_lit(0)),
        });
        let module = make_module(body);
        let result = airl_interp::interpret(&module);
        assert!(result.is_err(), "mod({a}, 0) should error");
    }
}

#[test]
fn prop_if_else_selects_correctly() {
    for (cond, then_v, else_v) in [
        (true, 10, 20),
        (false, 10, 20),
        (true, -1, 0),
        (false, 0, -1),
    ] {
        let body = println_node(Node::If {
            id: NodeId::new("if"),
            node_type: Type::I64,
            cond: Box::new(Node::Literal {
                id: NodeId::new("c"),
                node_type: Type::Bool,
                value: LiteralValue::Boolean(cond),
            }),
            then_branch: Box::new(int_lit(then_v)),
            else_branch: Box::new(int_lit(else_v)),
        });
        let module = make_module(body);
        let output = airl_interp::interpret(&module).unwrap();
        let expected = if cond { then_v } else { else_v };
        assert_eq!(output.stdout, format!("{expected}\n"));
    }
}

#[test]
fn prop_string_len() {
    for s in ["", "a", "hello", "hello world", "abcdefghij"] {
        let body = println_node(Node::Call {
            id: NodeId::new("len"),
            node_type: Type::I64,
            target: "std::string::len".to_string(),
            args: vec![str_lit(s)],
        });
        let module = make_module(body);
        let output = airl_interp::interpret(&module).unwrap();
        assert_eq!(output.stdout, format!("{}\n", s.len()), "len({s:?})");
    }
}

#[test]
fn prop_string_concat() {
    for (a, b) in [("", ""), ("a", "b"), ("hello", " world"), ("", "x")] {
        let body = println_node(Node::Call {
            id: NodeId::new("cat"),
            node_type: Type::String,
            target: "std::string::concat".to_string(),
            args: vec![str_lit(a), str_lit(b)],
        });
        let module = make_module(body);
        let output = airl_interp::interpret(&module).unwrap();
        assert_eq!(output.stdout, format!("{a}{b}\n"));
    }
}

#[test]
fn prop_math_abs() {
    for val in [-100i64, -1, 0, 1, 42, 100] {
        let body = println_node(Node::Call {
            id: NodeId::new("abs"),
            node_type: Type::I64,
            target: "std::math::abs".to_string(),
            args: vec![int_lit(val)],
        });
        let module = make_module(body);
        let output = airl_interp::interpret(&module).unwrap();
        assert_eq!(output.stdout, format!("{}\n", val.abs()), "abs({val})");
    }
}

#[test]
fn prop_random_modules_dont_panic() {
    // Generate 100 random modules and verify they don't panic the interpreter
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    let mut rng = Rng(12345);
    for _ in 0..100 {
        let val = (rng.next() as i64) % 10000;
        let body = match rng.next() % 3 {
            0 => println_node(int_lit(val)),
            1 => println_node(Node::BinOp {
                id: NodeId::new("op"),
                op: BinOpKind::Add,
                node_type: Type::I64,
                lhs: Box::new(int_lit(val)),
                rhs: Box::new(int_lit((rng.next() as i64) % 100)),
            }),
            _ => println_node(str_lit(&format!("test_{val}"))),
        };
        let module = make_module(body);
        // Should not panic — errors are fine, panics are not
        let _ = airl_interp::interpret(&module);
    }
}
