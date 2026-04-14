# AIRL Evaluation Rubric

## Purpose

This document defines how to evaluate whether each stage of the AIRL implementation is complete and correct. Use this rubric to verify stage completion before moving to the next stage.

---

## Stage 0: Project Bootstrap

### Pass Criteria

| # | Criterion | Test Command | Expected |
|---|---|---|---|
| 0.1 | Workspace builds | `cargo build` | Exit code 0, no errors |
| 0.2 | All tests pass | `cargo test` | All tests pass |
| 0.3 | Clippy clean | `cargo clippy -- -D warnings` | No warnings |
| 0.4 | Format clean | `cargo fmt -- --check` | No formatting issues |
| 0.5 | Hello world runs | `cargo run -p airl-cli -- run examples/hello.airl.json` | Prints "hello world" |
| 0.6 | All crates exist | Check Cargo.toml workspace members | 8 crates listed |
| 0.7 | CI passes | Push to repo, check Actions | Green build |

### Deliverable Checklist

- [ ] Cargo workspace with all 8+ crates
- [ ] At least one integration test
- [ ] ARCHITECTURE.md exists and is accurate
- [ ] README.md with build instructions
- [ ] examples/hello.airl.json works

---

## Stage 1: IR Core

### Pass Criteria

| # | Criterion | Test Command | Expected |
|---|---|---|---|
| 1.1 | All node types constructible | Unit tests in airl-ir | Create each node type, no panics |
| 1.2 | JSON round-trip | Property test | serialize(deserialize(serialize(ir))) == serialize(ir) |
| 1.3 | Binary round-trip | Property test | Same as JSON but with binary format |
| 1.4 | Version determinism | Property test | version_id(ir) == version_id(clone(ir)) |
| 1.5 | Type definitions | Unit tests | Create struct, enum, array, tuple, optional, result, function types |
| 1.6 | Effect definitions | Unit tests | Create all effect kinds |
| 1.7 | Module construction | Unit tests | Create module with imports, exports, functions, types |
| 1.8 | Symbol interning | Property test | intern(s) == intern(s) for all strings |
| 1.9 | Golden tests | 20+ fixtures | All node types covered |

### Quality Gates

- Line coverage >= 80% for airl-ir crate
- No `unwrap()` in library code (use Result/Option properly)
- All public types implement `Debug`, `Clone`, `PartialEq`, `Eq`
- All public types implement `Serialize`, `Deserialize`

---

## Stage 2: Type Checker

### Pass Criteria

| # | Criterion | Test Command | Expected |
|---|---|---|---|
| 2.1 | Well-typed programs accepted | Golden tests (positive) | Type check succeeds for all valid programs |
| 2.2 | Ill-typed programs rejected | Golden tests (negative) | Type check fails with correct error for 20+ invalid programs |
| 2.3 | Effect checking | Unit tests | Undeclared effects detected |
| 2.4 | Pure enforcement | Unit test | Pure function calling IO function → error |
| 2.5 | Exhaustive match | Unit test | Non-exhaustive match → error |
| 2.6 | Generic instantiation | Unit test | `Array<I64>` instantiates correctly |
| 2.7 | Trait resolution | Unit test | Method calls resolve to correct impl |
| 2.8 | Error quality | Manual review | Error messages include location, expected type, actual type |
| 2.9 | Fibonacci type checks | Integration test | fibonacci.airl.json passes type check |

### Specific Error Cases to Test

- [ ] Wrong argument type in function call
- [ ] Wrong number of arguments
- [ ] Access non-existent struct field
- [ ] Return type mismatch
- [ ] Assign incompatible types
- [ ] Use variable before declaration
- [ ] Use variable from wrong scope
- [ ] Non-exhaustive enum match
- [ ] Call Pure function from IO context (should work)
- [ ] Call IO function from Pure context (should fail)
- [ ] Missing Fail effect declaration
- [ ] Recursive type without indirection
- [ ] Duplicate field names in struct
- [ ] Duplicate variant names in enum
- [ ] Type parameter used without bounds when bounds needed
- [ ] Incompatible types in if/else branches
- [ ] Array element type mismatch
- [ ] Tuple index out of bounds
- [ ] Wrong generic argument count
- [ ] Trait method not implemented

---

## Stage 3: Interpreter

### Pass Criteria

| # | Criterion | Test Command | Expected |
|---|---|---|---|
| 3.1 | Hello world | Run hello.airl.json | stdout: "hello world\n" |
| 3.2 | Fibonacci | Run fibonacci.airl.json | Correct first 10 numbers |
| 3.3 | FizzBuzz | Run fizzbuzz.airl.json | Correct output 1-100 |
| 3.4 | Arithmetic | Golden tests | All binary/unary ops correct |
| 3.5 | String ops | Golden tests | concat, len, split, contains work |
| 3.6 | Array ops | Golden tests | push, map, filter, fold work |
| 3.7 | Control flow | Golden tests | if/else, match, loop, break |
| 3.8 | Struct/enum | Golden tests | Create, access fields, match variants |
| 3.9 | Error handling | Golden tests | Result propagation, try/catch |
| 3.10 | Step limit | Unit test | Infinite loop killed after N steps |
| 3.11 | Memory limit | Unit test | Allocation bomb killed |
| 3.12 | Time limit | Unit test | Slow program killed after timeout |
| 3.13 | Determinism | Property test | Same input → same output, always |

### Performance Targets

- Interpret 100K nodes/sec on a modern CPU
- Startup time < 50ms for small programs
- Memory overhead < 2x the program's data size

---

## Stage 4: Semantic Patch Engine

### Pass Criteria

| # | Criterion | Test Command | Expected |
|---|---|---|---|
| 4.1 | InsertNode | Unit test | Node added at correct position |
| 4.2 | RemoveNode | Unit test | Node removed, no dangling refs |
| 4.3 | ReplaceNode | Unit test | Node replaced, type still valid |
| 4.4 | RewireEdge | Unit test | Edge target changed correctly |
| 4.5 | AddFunction | Unit test | New function appears in module |
| 4.6 | RemoveFunction | Unit test | Function removed, callers flagged |
| 4.7 | RenameSymbol | Unit test | All references updated |
| 4.8 | AddField | Unit test | Struct field added |
| 4.9 | Patch inversion | Property test | apply(inverse(apply(p, ir))) == ir |
| 4.10 | Non-conflicting merge | Unit test | Two independent patches merge |
| 4.11 | Conflict detection | Unit test | Overlapping patches → conflict error |
| 4.12 | Version tracking | Unit test | Each patch creates new version |
| 4.13 | Impact analysis | Unit test | Affected functions correctly reported |
| 4.14 | Invalid patch rejection | Unit test | Patch referencing non-existent node → error |

### Property Tests

- **Inverse property:** For any valid patch P and IR I: `apply(inverse(P), apply(P, I)) == I`
- **Identity property:** Empty patch applied to any IR produces the same IR
- **Composition property:** `apply(P2, apply(P1, I)) == apply(compose(P1, P2), I)` for non-conflicting P1, P2
- **Type preservation:** If IR type-checks before patch, and patch passes validation, IR type-checks after patch

---

## Stage 5: Compiler Backend

### Pass Criteria

| # | Criterion | Test Command | Expected |
|---|---|---|---|
| 5.1 | Cranelift: hello | Compile + run hello | stdout: "hello world\n" |
| 5.2 | Cranelift: fibonacci | Compile + run fibonacci | Correct output |
| 5.3 | Cranelift: all golden | Compile + run all | Match interpreter output |
| 5.4 | WASM: hello | Compile to WASM + wasmtime | stdout: "hello world\n" |
| 5.5 | WASM: fibonacci | Compile to WASM + wasmtime | Correct output |
| 5.6 | Compile speed | Benchmark | < 2s for 1000-node program (Cranelift) |
| 5.7 | Runtime speed | Benchmark | 10x+ faster than interpreter |
| 5.8 | Binary size | Check | < 10MB for hello world (stripped) |
| 5.9 | Correctness | Property test | compiled(program, input) == interpreted(program, input) |

### LLVM Backend (if implemented)

| # | Criterion | Test Command | Expected |
|---|---|---|---|
| 5.10 | LLVM: all golden | Compile + run all | Match interpreter output |
| 5.11 | LLVM optimization | Benchmark | Faster than Cranelift output |
| 5.12 | Debug info | gdb/lldb | Source-level debugging works |

---

## Stage 6: Agent API

### Pass Criteria

| # | Criterion | Test Command | Expected |
|---|---|---|---|
| 6.1 | Create project via API | Integration test | Project created, version returned |
| 6.2 | Create module via API | Integration test | Module created, type-checked |
| 6.3 | Apply patch via API | Integration test | Patch applied, new version |
| 6.4 | Preview patch via API | Integration test | Impact analysis returned |
| 6.5 | Undo patch via API | Integration test | Previous version restored |
| 6.6 | Type check via API | Integration test | Errors/warnings returned |
| 6.7 | Interpret via API | Integration test | Output returned |
| 6.8 | Compile via API | Integration test | Binary produced |
| 6.9 | Find function via API | Integration test | Matching functions returned |
| 6.10 | Find type via API | Integration test | Matching types returned |
| 6.11 | Get call graph via API | Integration test | Graph structure correct |
| 6.12 | Project to text via API | Integration test | Valid TypeScript produced |
| 6.13 | Run tests via API | Integration test | Test results returned |
| 6.14 | Constraint check via API | Integration test | Violations detected |
| 6.15 | Full round-trip | End-to-end test | Create → Patch → Check → Run → Compile |
| 6.16 | CLI equivalence | Integration test | CLI produces same results as API |

### Agent Simulation Test

An automated test that simulates a real agent workflow:
1. Create project
2. Create module with initial IR
3. Apply 5 patches incrementally
4. Run tests after each patch
5. Compile final version
6. Execute compiled binary
7. Verify output

This must complete without human intervention.

---

## Stage 7: Standard Library

### Pass Criteria

| # | Criterion | Test Command | Expected |
|---|---|---|---|
| 7.1 | std::io functions | Unit tests | File read/write, stdio work |
| 7.2 | std::json functions | Unit tests | Parse/serialize round-trip |
| 7.3 | std::collections | Unit tests | HashMap, Vec operations correct |
| 7.4 | std::string functions | Unit tests | All string ops work |
| 7.5 | std::math functions | Unit tests | Math ops correct |
| 7.6 | Example: grep tool | Build + run | Searches files correctly |
| 7.7 | Example: HTTP server | Build + run | Responds to requests |
| 7.8 | Example: CSV processor | Build + run | Transforms CSV correctly |
| 7.9 | HTTP server perf | Benchmark | > 1000 req/s single core |
| 7.10 | All std tested | Coverage check | > 90% function coverage |

---

## Cross-Stage Quality Metrics

These must hold at all times after the relevant stage is complete:

| Metric | Minimum | Measured By |
|---|---|---|
| Build time (clean) | < 5 minutes | CI |
| Test suite time | < 2 minutes | CI |
| Test coverage (overall) | > 75% | cargo-llvm-cov |
| Clippy warnings | 0 | cargo clippy |
| Unsafe code blocks | 0 (except FFI to LLVM/Cranelift) | grep/audit |
| Public API doc coverage | 100% | cargo doc --no-deps |
| Binary size (CLI) | < 50MB | ls -la |

---

## Regression Prevention

After each stage, create a test suite that must continue to pass in all subsequent stages:

```
tests/
  regression/
    stage0/          # hello world runs
    stage1/          # IR round-trip
    stage2/          # type checker accepts/rejects
    stage3/          # interpreter produces correct output
    stage4/          # patch operations work
    stage5/          # compiler output matches interpreter
    stage6/          # API operations work
    stage7/          # std library works
```

Every CI run must execute all regression tests from all completed stages.
