//! Criterion benchmarks for AIRL IR operations.
//!
//! Run with:
//!   cargo bench --manifest-path crates/airl-benches/Cargo.toml --bench ir_bench

use airl_benches::example_json;
use airl_ir::version::VersionId;
use airl_ir::Module;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_json_parse_fizzbuzz(c: &mut Criterion) {
    let json = example_json("fizzbuzz.airl.json");
    c.bench_function("ir_json_parse_fizzbuzz", |b| {
        b.iter(|| {
            let _: Module = serde_json::from_str(black_box(&json)).unwrap();
        });
    });
}

fn bench_json_serialize_fizzbuzz(c: &mut Criterion) {
    let module: Module = serde_json::from_str(&example_json("fizzbuzz.airl.json")).unwrap();
    c.bench_function("ir_json_serialize_fizzbuzz", |b| {
        b.iter(|| {
            let _ = serde_json::to_string(black_box(&module)).unwrap();
        });
    });
}

fn bench_json_roundtrip_fizzbuzz(c: &mut Criterion) {
    let json = example_json("fizzbuzz.airl.json");
    c.bench_function("ir_json_roundtrip_fizzbuzz", |b| {
        b.iter(|| {
            let module: Module = serde_json::from_str(black_box(&json)).unwrap();
            let _ = serde_json::to_string(&module).unwrap();
        });
    });
}

fn bench_version_hash_small(c: &mut Criterion) {
    let module: Module = serde_json::from_str(&example_json("hello.airl.json")).unwrap();
    c.bench_function("ir_version_hash_hello", |b| {
        b.iter(|| {
            let _ = VersionId::compute(black_box(&module));
        });
    });
}

fn bench_version_hash_large(c: &mut Criterion) {
    let module: Module = serde_json::from_str(&example_json("fizzbuzz.airl.json")).unwrap();
    c.bench_function("ir_version_hash_fizzbuzz", |b| {
        b.iter(|| {
            let _ = VersionId::compute(black_box(&module));
        });
    });
}

criterion_group!(
    benches,
    bench_json_parse_fizzbuzz,
    bench_json_serialize_fizzbuzz,
    bench_json_roundtrip_fizzbuzz,
    bench_version_hash_small,
    bench_version_hash_large,
);
criterion_main!(benches);
