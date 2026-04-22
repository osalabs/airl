//! Criterion benchmarks for the AIRL compiler (WASM emission + Cranelift JIT).
//!
//! Run with:
//!   cargo bench --manifest-path crates/airl-benches/Cargo.toml --bench compile_bench
//!
//! Note: Cranelift JIT benches re-run full compile+execute per iteration
//! (our compile API doesn't separate compile from run), so results reflect
//! both phases. WASM emission is standalone and much faster.

use airl_benches::example_module;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_wasm_hello(c: &mut Criterion) {
    let module = example_module("hello.airl.json");
    c.bench_function("compile_wasm_hello", |b| {
        b.iter(|| {
            let _ = airl_compile::wasm::compile_to_wasm(black_box(&module)).unwrap();
        });
    });
}

fn bench_wasm_fibonacci(c: &mut Criterion) {
    let module = example_module("fibonacci.airl.json");
    c.bench_function("compile_wasm_fibonacci", |b| {
        b.iter(|| {
            let _ = airl_compile::wasm::compile_to_wasm(black_box(&module)).unwrap();
        });
    });
}

fn bench_wasm_fizzbuzz(c: &mut Criterion) {
    let module = example_module("fizzbuzz.airl.json");
    c.bench_function("compile_wasm_fizzbuzz", |b| {
        b.iter(|| {
            let _ = airl_compile::wasm::compile_to_wasm(black_box(&module)).unwrap();
        });
    });
}

fn bench_jit_hello(c: &mut Criterion) {
    let module = example_module("hello.airl.json");
    // JIT uses global state for stdout; reduce sample size to keep test time reasonable.
    let mut group = c.benchmark_group("compile_jit");
    group.sample_size(20);
    group.bench_function("jit_compile_and_run_hello", |b| {
        b.iter(|| {
            let _ = airl_compile::compile_and_run(black_box(&module)).unwrap();
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_wasm_hello,
    bench_wasm_fibonacci,
    bench_wasm_fizzbuzz,
    bench_jit_hello,
);
criterion_main!(benches);
