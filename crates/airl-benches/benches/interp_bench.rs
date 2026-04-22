//! Criterion benchmarks for the AIRL interpreter.
//!
//! Run with:
//!   cargo bench --manifest-path crates/airl-benches/Cargo.toml --bench interp_bench
//!
//! Save / compare baselines:
//!   ... -- --save-baseline main
//!   ... -- --baseline main

use airl_benches::example_module;
use airl_ir::effects::Effect;
use airl_ir::ids::{FuncId, ModuleId, NodeId};
use airl_ir::module::{FuncDef, Module, ModuleInner, ModuleMetadata};
use airl_ir::node::{BinOpKind, LiteralValue, Node};
use airl_ir::types::Type;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

/// Tiny baseline: a single `println("hi")` with no recursion.
fn hello_world() -> Module {
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
                id: FuncId::new("f"),
                name: "main".to_string(),
                params: vec![],
                returns: Type::Unit,
                effects: vec![Effect::IO],
                body: Node::Call {
                    id: NodeId::new("c"),
                    node_type: Type::Unit,
                    target: "std::io::println".to_string(),
                    args: vec![Node::Literal {
                        id: NodeId::new("v"),
                        node_type: Type::String,
                        value: LiteralValue::Str("hi".to_string()),
                    }],
                },
            }],
        },
    }
}

/// 1000 nested integer additions: stresses arithmetic + tree walking.
fn arith_1000() -> Module {
    let mut body = Node::Literal {
        id: NodeId::new("n0"),
        node_type: Type::I64,
        value: LiteralValue::Integer(0),
    };
    for i in 1..=1000i64 {
        body = Node::BinOp {
            id: NodeId::new(&format!("a{i}")),
            op: BinOpKind::Add,
            node_type: Type::I64,
            lhs: Box::new(body),
            rhs: Box::new(Node::Literal {
                id: NodeId::new(&format!("l{i}")),
                node_type: Type::I64,
                value: LiteralValue::Integer(i),
            }),
        };
    }
    let wrapped = Node::Call {
        id: NodeId::new("print"),
        node_type: Type::Unit,
        target: "std::io::println".to_string(),
        args: vec![body],
    };
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
                id: FuncId::new("f"),
                name: "main".to_string(),
                params: vec![],
                returns: Type::Unit,
                effects: vec![Effect::IO],
                body: wrapped,
            }],
        },
    }
}

fn bench_hello_world(c: &mut Criterion) {
    let module = hello_world();
    c.bench_function("interp_hello_world", |b| {
        b.iter(|| {
            let _ = airl_interp::interpret(black_box(&module)).unwrap();
        });
    });
}

fn bench_fibonacci(c: &mut Criterion) {
    let module = example_module("fibonacci.airl.json");
    c.bench_function("interp_fibonacci_10", |b| {
        b.iter(|| {
            let _ = airl_interp::interpret(black_box(&module)).unwrap();
        });
    });
}

fn bench_fizzbuzz(c: &mut Criterion) {
    let module = example_module("fizzbuzz.airl.json");
    c.bench_function("interp_fizzbuzz_1_20", |b| {
        b.iter(|| {
            let _ = airl_interp::interpret(black_box(&module)).unwrap();
        });
    });
}

fn bench_arith_1000(c: &mut Criterion) {
    let module = arith_1000();
    c.bench_function("interp_arith_1000_additions", |b| {
        b.iter(|| {
            let _ = airl_interp::interpret(black_box(&module)).unwrap();
        });
    });
}

criterion_group!(
    benches,
    bench_hello_world,
    bench_fibonacci,
    bench_fizzbuzz,
    bench_arith_1000,
);
criterion_main!(benches);
