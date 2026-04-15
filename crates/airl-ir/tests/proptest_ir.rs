//! Property-based tests for AIRL IR (no external deps).
//!
//! Uses a simple deterministic RNG to generate random IR and verify invariants.

use airl_ir::effects::Effect;
use airl_ir::ids::*;
use airl_ir::module::*;
use airl_ir::node::*;
use airl_ir::types::Type;
use airl_ir::version::VersionId;

/// Simple deterministic PRNG (xorshift64).
struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() as i64).abs() % (hi - lo + 1)
    }
    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        let idx = (self.next() as usize) % items.len();
        &items[idx]
    }
    fn string(&mut self, len: usize) -> String {
        (0..len)
            .map(|_| (b'a' + (self.next() % 26) as u8) as char)
            .collect()
    }
}

fn random_literal(rng: &mut Rng) -> (LiteralValue, Type) {
    match rng.next() % 4 {
        0 => {
            let v = rng.range(-1000, 1000);
            (LiteralValue::Integer(v), Type::I64)
        }
        1 => {
            let b = rng.next() % 2 == 0;
            (LiteralValue::Boolean(b), Type::Bool)
        }
        2 => {
            let len = rng.range(0, 15) as usize;
            let s = rng.string(len);
            (LiteralValue::Str(s), Type::String)
        }
        _ => (LiteralValue::Unit, Type::Unit),
    }
}

fn random_node(rng: &mut Rng, depth: usize) -> Node {
    if depth == 0 || rng.next() % 3 == 0 {
        let (val, ty) = random_literal(rng);
        return Node::Literal {
            id: NodeId::new(&format!("n{}", rng.next() % 100000)),
            node_type: ty,
            value: val,
        };
    }

    match rng.next() % 4 {
        0 => {
            // BinOp (integer)
            let ops = [BinOpKind::Add, BinOpKind::Sub, BinOpKind::Mul];
            Node::BinOp {
                id: NodeId::new(&format!("b{}", rng.next() % 100000)),
                op: ops[(rng.next() as usize) % ops.len()].clone(),
                node_type: Type::I64,
                lhs: Box::new(Node::Literal {
                    id: NodeId::new(&format!("l{}", rng.next() % 100000)),
                    node_type: Type::I64,
                    value: LiteralValue::Integer(rng.range(-100, 100)),
                }),
                rhs: Box::new(Node::Literal {
                    id: NodeId::new(&format!("r{}", rng.next() % 100000)),
                    node_type: Type::I64,
                    value: LiteralValue::Integer(rng.range(-100, 100)),
                }),
            }
        }
        1 => {
            // If
            Node::If {
                id: NodeId::new(&format!("if{}", rng.next() % 100000)),
                node_type: Type::I64,
                cond: Box::new(Node::Literal {
                    id: NodeId::new(&format!("c{}", rng.next() % 100000)),
                    node_type: Type::Bool,
                    value: LiteralValue::Boolean(rng.next() % 2 == 0),
                }),
                then_branch: Box::new(random_node(rng, depth - 1)),
                else_branch: Box::new(random_node(rng, depth - 1)),
            }
        }
        2 => {
            // Call println
            let (val, ty) = random_literal(rng);
            Node::Call {
                id: NodeId::new(&format!("call{}", rng.next() % 100000)),
                node_type: Type::Unit,
                target: "std::io::println".to_string(),
                args: vec![Node::Literal {
                    id: NodeId::new(&format!("a{}", rng.next() % 100000)),
                    node_type: ty,
                    value: val,
                }],
            }
        }
        _ => {
            let (val, ty) = random_literal(rng);
            Node::Literal {
                id: NodeId::new(&format!("lit{}", rng.next() % 100000)),
                node_type: ty,
                value: val,
            }
        }
    }
}

fn random_module(rng: &mut Rng) -> Module {
    let num_funcs = rng.range(1, 3) as usize;
    let mut functions = Vec::new();

    for i in 0..num_funcs {
        let name = if i == 0 {
            "main".to_string()
        } else {
            rng.string(5)
        };
        let body = random_node(rng, 2);
        let returns = match &body {
            Node::Call { node_type, .. } => node_type.clone(),
            Node::BinOp { node_type, .. } => node_type.clone(),
            Node::If { node_type, .. } => node_type.clone(),
            Node::Literal { node_type, .. } => node_type.clone(),
            _ => Type::Unit,
        };
        functions.push(FuncDef {
            id: FuncId::new(&format!("f_{name}")),
            name,
            params: vec![],
            returns,
            effects: vec![Effect::IO],
            body,
        });
    }

    Module {
        format_version: "0.1.0".to_string(),
        module: ModuleInner {
            id: ModuleId::new("mod_test"),
            name: "test".to_string(),
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
            functions,
        },
    }
}

#[test]
fn prop_serialize_roundtrip() {
    let mut rng = Rng::new(42);
    for _ in 0..100 {
        let module = random_module(&mut rng);
        let json = serde_json::to_string(&module).unwrap();
        let parsed: Module = serde_json::from_str(&json).unwrap();
        assert_eq!(module, parsed, "roundtrip failed for seed iteration");
    }
}

#[test]
fn prop_version_hash_deterministic() {
    let mut rng = Rng::new(123);
    for _ in 0..100 {
        let module = random_module(&mut rng);
        let v1 = VersionId::compute(&module).to_hex();
        let v2 = VersionId::compute(&module).to_hex();
        assert_eq!(v1, v2, "version hash not deterministic");
    }
}

#[test]
fn prop_version_hash_sensitive() {
    let mut rng = Rng::new(456);
    for _ in 0..50 {
        let m1 = random_module(&mut rng);
        let mut m2 = m1.clone();
        m2.module.name = format!("different_{}", rng.next());
        let v1 = VersionId::compute(&m1).to_hex();
        let v2 = VersionId::compute(&m2).to_hex();
        assert_ne!(v1, v2, "hash should change when content changes");
    }
}

#[test]
fn prop_type_string_roundtrip() {
    let types = [
        Type::Unit,
        Type::Bool,
        Type::I64,
        Type::F64,
        Type::String,
        Type::I32,
        Type::U8,
        Type::Bytes,
        Type::Array {
            element: Box::new(Type::I64),
        },
        Type::Array {
            element: Box::new(Type::String),
        },
        Type::Optional {
            inner: Box::new(Type::I64),
        },
        Type::Result {
            ok: Box::new(Type::I64),
            err: Box::new(Type::String),
        },
    ];
    for ty in &types {
        let s = ty.to_type_str();
        let parsed = Type::from_type_str(&s);
        assert_eq!(ty, &parsed, "type roundtrip failed for {s}");
    }
}

#[test]
fn prop_effect_string_roundtrip() {
    let effects = [
        Effect::Pure,
        Effect::IO,
        Effect::Allocate,
        Effect::Diverge,
        Effect::Read {
            resource: "fs".to_string(),
        },
        Effect::Write {
            resource: "net".to_string(),
        },
        Effect::Fail {
            error_type: "IOError".to_string(),
        },
    ];
    for eff in &effects {
        let s = eff.to_effect_str();
        let parsed = Effect::from_effect_str(&s);
        assert_eq!(eff, &parsed, "effect roundtrip failed for {s}");
    }
}
