//! Criterion benchmarks for the AIRL type checker.
//!
//! Run with:
//!   cargo bench --manifest-path crates/airl-benches/Cargo.toml --bench typecheck_bench

use airl_benches::example_module;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_typecheck_hello(c: &mut Criterion) {
    let module = example_module("hello.airl.json");
    c.bench_function("typecheck_hello", |b| {
        b.iter(|| {
            let result = airl_typecheck::typecheck(black_box(&module));
            assert!(result.is_ok());
        });
    });
}

fn bench_typecheck_fibonacci(c: &mut Criterion) {
    let module = example_module("fibonacci.airl.json");
    c.bench_function("typecheck_fibonacci", |b| {
        b.iter(|| {
            let result = airl_typecheck::typecheck(black_box(&module));
            assert!(result.is_ok());
        });
    });
}

fn bench_typecheck_fizzbuzz(c: &mut Criterion) {
    let module = example_module("fizzbuzz.airl.json");
    c.bench_function("typecheck_fizzbuzz", |b| {
        b.iter(|| {
            let result = airl_typecheck::typecheck(black_box(&module));
            assert!(result.is_ok());
        });
    });
}

fn bench_typecheck_string_ops(c: &mut Criterion) {
    let module = example_module("string_ops.airl.json");
    c.bench_function("typecheck_string_ops", |b| {
        b.iter(|| {
            let result = airl_typecheck::typecheck(black_box(&module));
            assert!(result.is_ok());
        });
    });
}

criterion_group!(
    benches,
    bench_typecheck_hello,
    bench_typecheck_fibonacci,
    bench_typecheck_fizzbuzz,
    bench_typecheck_string_ops,
);
criterion_main!(benches);
