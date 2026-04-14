# AIRL: AI-native Intermediate Representation Language

## Implementation Plan

**Project codename:** AIRL (AI Intermediate Representation Language)
**Goal:** Build an AI-native programming representation and toolchain that AI coding agents use directly, without human-in-the-loop authoring.

---

## Executive Summary

This plan describes how to build AIRL — a graph-based intermediate representation (IR) designed as the primary authoring target for AI coding agents. Human-readable code (TypeScript, Python, etc.) becomes an *output projection*, not the source of truth.

AIRL is **not** a new syntax for humans. It is:
- A **typed program graph** (nodes + edges, not text)
- Manipulated via **semantic patch operations** (not full-file rewrites)
- With **multiple projections** for humans (pretty-printed code, diagrams, contract views)
- **Interpreted** for fast feedback, **compiled** (via LLVM/WASM) for production

The plan is split into **7 stages**. Each stage produces a working, testable artifact (MVP). An AI coding agent can execute each stage independently, using the previous stage's output as foundation.

---

## Design Principles

These principles guide every implementation decision:

1. **IR-first, text-second.** The canonical artifact is a typed graph. Text is a view.
2. **Small orthogonal core (RISC-like).** Few primitives, no near-equivalent ways to express the same thing. Reduces ambiguity for both generation and verification.
3. **Semantic patches as the edit primitive.** Changes are expressed as graph operations (add node, rewire edge, replace subtree), not text diffs.
4. **Explicit effects and boundaries.** Every function declares what it reads, writes, allocates, and can fail with. No hidden side effects.
5. **Separation of duties.** The IR supports independent authoring, reviewing, and testing by different agents.
6. **Evidence-backed acceptance.** Every change must carry evidence (tests, benchmarks, proofs) that can be independently verified.
7. **Constraints over procedures.** Specify what must hold true (invariants, budgets, red lines), not step-by-step instructions.
8. **Feedback latency is the bottleneck.** Interpreted execution for tight loops; compiled output for production.

---

## Technology Choices

| Component | Technology | Rationale |
|---|---|---|
| Core IR engine | **Rust** | Memory safety, performance, mature LLVM bindings (inkwell/llvm-sys), WASM compilation target |
| IR serialization | **FlatBuffers** (binary) + **JSON** (debug/interop) | Zero-copy binary for perf; JSON for agent API and debugging |
| Agent API | **gRPC + JSON-RPC over HTTP** | Language-agnostic; streaming support for incremental compilation |
| CLI tooling | **Rust** (clap) | Single binary distribution, fast startup |
| Human projections | **Tree-sitter grammars** + template engine | Generates readable TypeScript/Python/Go from IR |
| Compiler backend | **LLVM 18+** via Cranelift (initially), then LLVM | Cranelift for fast dev iteration; LLVM for production optimization |
| WASM target | **wasm32-wasi** | Server-side and browser execution |
| Test framework | Built-in: **property tests + golden tests + mutation tests** | Self-verifying from day one |

---

## Stage 0: Project Bootstrap (MVP: build system + empty pipeline)

**Goal:** Set up the project structure, build system, CI, and an empty end-to-end pipeline that compiles and runs "hello world" through every layer (even if most layers are stubs).

**Deliverables:**
- [ ] Rust workspace with crates: `airl-ir`, `airl-typecheck`, `airl-interp`, `airl-compile`, `airl-patch`, `airl-project`, `airl-cli`, `airl-api`
- [ ] `Cargo.toml` workspace config with shared dependencies
- [ ] CI pipeline (GitHub Actions): build, test, clippy, fmt
- [ ] A hardcoded "hello world" IR graph that flows through: parse → typecheck → interpret → print output
- [ ] Basic CLI: `airl run <file.airl>` (reads JSON IR, interprets, prints)
- [ ] `ARCHITECTURE.md` in repo root (see architecture.md)
- [ ] Property: `cargo test` passes, `airl run hello.airl.json` prints "hello world"

**Acceptance criteria:**
```
airl run examples/hello.airl.json
# Output: hello world
```

**Files to create:**
```
airl/
  Cargo.toml                    # workspace
  crates/
    airl-ir/                    # IR data structures
    airl-typecheck/             # type checker
    airl-interp/                # tree-walking interpreter
    airl-compile/               # compiler backend (stub)
    airl-patch/                 # semantic patch engine (stub)
    airl-project/               # project/module management (stub)
    airl-cli/                   # CLI binary
    airl-api/                   # gRPC/HTTP API server (stub)
  examples/
    hello.airl.json             # first IR program
  tests/
    golden/                     # golden test fixtures
  ARCHITECTURE.md
  README.md
```

---

## Stage 1: IR Core (MVP: typed graph with basic types and operations)

**Goal:** Define and implement the core IR data structures — the program graph that is AIRL's source of truth.

**Deliverables:**

### 1.1 Node Types (core primitives)

The IR is a directed graph where each node has a unique ID, a kind, a type, and edges to other nodes.

```
Primitives:
  - Literal(value: i64 | f64 | bool | string | bytes | unit)
  - Param(index: u32, name: Symbol)
  - Let(name: Symbol, value: NodeId, body: NodeId)
  - If(cond: NodeId, then: NodeId, else: NodeId)
  - Call(target: FuncId, args: Vec<NodeId>)
  - Return(value: NodeId)
  - BinOp(op: BinOpKind, lhs: NodeId, rhs: NodeId)
  - UnaryOp(op: UnaryOpKind, operand: NodeId)
  - StructLiteral(type: TypeId, fields: Vec<(Symbol, NodeId)>)
  - FieldAccess(object: NodeId, field: Symbol)
  - ArrayLiteral(elements: Vec<NodeId>)
  - IndexAccess(array: NodeId, index: NodeId)
  - Match(scrutinee: NodeId, arms: Vec<MatchArm>)
  - Loop(body: NodeId, break_value: Option<NodeId>)
  - Block(statements: Vec<NodeId>, result: NodeId)
  - Error(kind: ErrorKind, message: String)
```

### 1.2 Type System

```
Types:
  - Primitive: I32, I64, F32, F64, Bool, String, Bytes, Unit
  - Array(element: TypeId)
  - Tuple(elements: Vec<TypeId>)
  - Struct { name: Symbol, fields: Vec<(Symbol, TypeId)> }
  - Enum { name: Symbol, variants: Vec<Variant> }
  - Function { params: Vec<TypeId>, returns: TypeId, effects: EffectSet }
  - Reference(inner: TypeId, mutability: Mutability)
  - Optional(inner: TypeId)
  - Result(ok: TypeId, err: TypeId)
  - TypeParam(name: Symbol, bounds: Vec<TraitBound>)
  - Generic(base: TypeId, args: Vec<TypeId>)
```

### 1.3 Effect System

Every function declares its effects explicitly:

```
Effects:
  - Pure              # no side effects
  - Read(resource)    # reads from a named resource
  - Write(resource)   # writes to a named resource
  - Allocate          # heap allocation
  - IO                # general I/O (file, network, stdio)
  - Fail(error_type)  # can fail with a specific error type
  - Diverge           # may not terminate
```

### 1.4 Module System

```
Module:
  - id: ModuleId
  - name: Symbol
  - imports: Vec<Import>          # other modules
  - exports: Vec<Export>          # public items
  - types: Vec<TypeDef>          # type definitions
  - functions: Vec<FuncDef>      # function definitions
  - constants: Vec<ConstDef>     # compile-time constants
  - traits: Vec<TraitDef>        # interface definitions
  - impls: Vec<ImplDef>          # trait implementations
  - metadata: ModuleMetadata     # version, author, constraints
```

### 1.5 Serialization

- [ ] Binary format via FlatBuffers (schema file: `ir.fbs`)
- [ ] JSON format for debugging and agent API
- [ ] Round-trip property test: serialize → deserialize → serialize == original
- [ ] Versioned format with magic bytes and version number

**Acceptance criteria:**
- Construct any IR graph programmatically in Rust
- Serialize to JSON / binary, deserialize back, verify equality
- `cargo test` — all property tests pass
- At least 20 golden tests covering each node type

---

## Stage 2: Type Checker (MVP: catch type errors before execution)

**Goal:** Implement bidirectional type checking over the IR graph.

**Deliverables:**
- [ ] Type inference for let bindings (local Hindley-Milner, no global inference)
- [ ] Type checking for all node types from Stage 1
- [ ] Effect checking: verify declared effects match actual usage
- [ ] Error reporting with node IDs and human-readable messages
- [ ] Trait resolution (basic: single dispatch, no specialization yet)
- [ ] Generic instantiation (monomorphization tracking)

**Key rules:**
1. Every function must declare its return type and effect set
2. Effect sets compose: calling a function with `IO` effect makes the caller `IO`
3. `Pure` functions cannot call non-`Pure` functions
4. All pattern matches must be exhaustive
5. Struct field access is checked at compile time
6. No implicit coercions (all conversions explicit)

**Acceptance criteria:**
- Type check all Stage 1 golden test programs
- Reject 20+ intentionally ill-typed programs with clear error messages
- Effect checker catches undeclared effects
- `cargo test` — all tests pass

---

## Stage 3: Interpreter (MVP: execute IR programs, get output)

**Goal:** Tree-walking interpreter that executes IR graphs directly. This is the fast-feedback loop for agents.

**Deliverables:**
- [ ] Tree-walking interpreter for all node types
- [ ] Stack-based execution with explicit frames
- [ ] Built-in functions: print, string ops, math, array ops
- [ ] Basic I/O: stdin/stdout/stderr, file read/write
- [ ] Error handling: Result propagation, panic with stack traces
- [ ] Execution limits: step count, memory budget, time budget (for safety)
- [ ] Deterministic execution mode (for reproducible tests)
- [ ] REPL mode: agent sends IR nodes, gets results incrementally

**Built-in standard functions (initial set):**
```
io::print(s: String) -> Unit [IO]
io::println(s: String) -> Unit [IO]
io::read_line() -> String [IO]
io::read_file(path: String) -> Result<String, IOError> [IO]
io::write_file(path: String, content: String) -> Result<Unit, IOError> [IO]

string::len(s: String) -> I64 [Pure]
string::concat(a: String, b: String) -> String [Pure]
string::split(s: String, sep: String) -> Array<String> [Pure]
string::contains(s: String, sub: String) -> Bool [Pure]
string::to_i64(s: String) -> Result<I64, ParseError> [Pure]

array::len<T>(arr: Array<T>) -> I64 [Pure]
array::push<T>(arr: Array<T>, item: T) -> Array<T> [Pure]
array::map<T, U>(arr: Array<T>, f: Fn(T) -> U) -> Array<U> [Pure]
array::filter<T>(arr: Array<T>, f: Fn(T) -> Bool) -> Array<T> [Pure]
array::fold<T, U>(arr: Array<T>, init: U, f: Fn(U, T) -> U) -> U [Pure]

math::abs(n: I64) -> I64 [Pure]
math::max(a: I64, b: I64) -> I64 [Pure]
math::min(a: I64, b: I64) -> I64 [Pure]
```

**Acceptance criteria:**
```
# Create IR for: fn main() { println("hello world") }
airl run examples/hello.airl.json        # prints "hello world"
airl run examples/fibonacci.airl.json    # prints first 10 fibonacci numbers
airl run examples/fizzbuzz.airl.json     # prints fizzbuzz 1-100
```
- All golden tests execute and produce expected output
- Execution limits work (programs that loop forever get killed)
- Interpreter matches type checker: well-typed programs don't crash

---

## Stage 4: Semantic Patch Engine (MVP: agents edit IR via patches, not rewrites)

**Goal:** Implement the patch system that allows AI agents to make targeted changes to IR graphs without rewriting entire modules.

This is the **core differentiator** of AIRL. Instead of generating entire files, agents produce small semantic patches.

**Deliverables:**

### 4.1 Patch Operations

```
PatchOp:
  - InsertNode { parent: NodeId, position: InsertPos, node: Node }
  - RemoveNode { target: NodeId }
  - ReplaceNode { target: NodeId, replacement: Node }
  - RewireEdge { from: NodeId, old_target: NodeId, new_target: NodeId }
  - AddField { struct_type: TypeId, field: FieldDef }
  - RemoveField { struct_type: TypeId, field_name: Symbol }
  - AddFunction { module: ModuleId, func: FuncDef }
  - RemoveFunction { module: ModuleId, func_id: FuncId }
  - AddImport { module: ModuleId, import: Import }
  - RemoveImport { module: ModuleId, import: Import }
  - ChangeType { target: NodeId, new_type: TypeId }
  - AddEffect { func: FuncId, effect: Effect }
  - RemoveEffect { func: FuncId, effect: Effect }
  - RenameSymbol { old: Symbol, new: Symbol, scope: ScopeId }
  - MoveNode { target: NodeId, new_parent: NodeId, position: InsertPos }
  - AddVariant { enum_type: TypeId, variant: Variant }
  - RemoveVariant { enum_type: TypeId, variant_name: Symbol }
```

### 4.2 Patch Bundle

A **Patch** is an ordered list of `PatchOp` with metadata:

```
Patch:
  - id: PatchId (UUID)
  - parent_version: VersionId     # IR version this applies to
  - operations: Vec<PatchOp>
  - rationale: String             # why this change
  - evidence: Vec<EvidenceRef>    # test results, benchmarks
  - author: AgentId               # which agent authored this
  - timestamp: DateTime
  - constraints_checked: Vec<ConstraintId>  # which constraints were verified
```

### 4.3 Patch Application & Validation

- [ ] Apply patch to IR graph, producing new version
- [ ] Validate patch: all referenced nodes exist, types are consistent
- [ ] Conflict detection: two patches that touch the same nodes
- [ ] Patch composition: merge non-conflicting patches
- [ ] Patch inversion: generate undo patch for any patch
- [ ] Impact analysis: which functions/types are affected by a patch

### 4.4 Version Graph

- [ ] Each IR state has a `VersionId` (content hash)
- [ ] Versions form a DAG (branching + merging)
- [ ] Efficient diff between any two versions
- [ ] Garbage collection of unreachable versions

**Acceptance criteria:**
- Apply a patch that adds a function → type check → interpret → correct output
- Apply a patch that renames a symbol → all references updated
- Apply two non-conflicting patches → merged correctly
- Apply conflicting patches → conflict detected and reported
- Undo any patch → original IR restored exactly
- Property test: apply(patch) then apply(inverse(patch)) == identity

---

## Stage 5: Compiler Backend (MVP: compile IR to native binary via Cranelift, then LLVM)

**Goal:** Compile IR graphs to native executables and WASM modules.

### 5.1 Cranelift Backend (fast compilation, moderate optimization)

- [ ] Lower IR nodes to Cranelift IR (CLIF)
- [ ] Compile to native code for host platform
- [ ] Basic optimizations: constant folding, dead code elimination, inlining
- [ ] Link with system libc for I/O builtins
- [ ] Produce standalone executables

### 5.2 WASM Backend

- [ ] Lower IR to WASM via Cranelift's WASM codegen
- [ ] WASI support for I/O
- [ ] Browser-compatible WASM output (no WASI, JS glue)

### 5.3 LLVM Backend (high optimization, slower compilation)

- [ ] Lower IR to LLVM IR via inkwell
- [ ] Full optimization pipeline (-O2 equivalent)
- [ ] Platform-specific targets (x86_64, aarch64)
- [ ] Debug info (DWARF) for source-level debugging of IR

**Acceptance criteria:**
```
airl compile examples/fibonacci.airl.json -o fib
./fib   # prints fibonacci numbers, runs 10x+ faster than interpreter

airl compile examples/fibonacci.airl.json --target wasm -o fib.wasm
wasmtime fib.wasm   # same output
```
- Compiled output matches interpreter output for all golden tests
- Compilation of 1000-node IR completes in <2 seconds (Cranelift)
- Compiled binary runs at least 10x faster than interpreter for compute-heavy programs

---

## Stage 6: Agent API & Tooling (MVP: AI agent can create, edit, build, test programs via API)

**Goal:** Build the API layer that AI coding agents actually use to interact with AIRL.

### 6.1 gRPC/HTTP API

```
Service AIRL:
  # Project management
  CreateProject(name, config) -> ProjectId
  OpenProject(path) -> ProjectId

  # IR manipulation
  GetModule(project, module_id) -> Module (JSON IR)
  CreateModule(project, name, initial_ir) -> ModuleId

  # Patch operations (the primary edit interface)
  ApplyPatch(project, patch) -> PatchResult { new_version, diagnostics }
  PreviewPatch(project, patch) -> PatchPreview { affected_nodes, type_errors }
  GetDiff(project, version_a, version_b) -> Diff
  UndoPatch(project, patch_id) -> PatchResult

  # Build & run
  TypeCheck(project) -> Vec<Diagnostic>
  Interpret(project, entry_func, args) -> RunResult { output, metrics }
  Compile(project, target, opt_level) -> CompileResult { binary_path, metrics }

  # Queries (agents need to understand the codebase)
  FindFunction(project, name_pattern) -> Vec<FuncSummary>
  FindType(project, name_pattern) -> Vec<TypeSummary>
  GetCallGraph(project, func_id) -> CallGraph
  GetDependencyGraph(project) -> DepGraph
  GetEffectSummary(project, func_id) -> EffectSet

  # Projections (human-readable views)
  ProjectToText(project, module_id, language: "typescript"|"python"|"go"|"rust") -> String
  ProjectToContract(project, module_id) -> ContractView
  ProjectToDepDiagram(project) -> MermaidDiagram

  # Evidence & testing
  RunTests(project) -> TestResults
  RunBenchmarks(project) -> BenchmarkResults
  GetCoverage(project) -> CoverageReport

  # Constraints
  SetConstraint(project, constraint) -> ConstraintId
  CheckConstraints(project) -> Vec<ConstraintViolation>
```

### 6.2 Constraint System

Agents and humans define constraints that the system enforces automatically:

```
Constraint:
  - MaxFunctionComplexity(threshold: u32)        # cyclomatic complexity limit
  - MaxModuleSize(max_nodes: u32)                # prevent bloat
  - RequiredEffectPurity(func_pattern: Glob)     # these funcs must be Pure
  - ForbiddenDependency(from: ModuleGlob, to: ModuleGlob)  # architectural boundaries
  - PerformanceBudget(func: FuncId, max_ns: u64) # latency ceiling
  - MemoryBudget(func: FuncId, max_bytes: u64)   # memory ceiling
  - RequiredTests(func_pattern: Glob, min_coverage: f64)    # test coverage floor
  - ForbiddenEffect(func_pattern: Glob, effect: Effect)     # e.g., no IO in core logic
  - APIStability(module: ModuleId, level: SemVer)            # no breaking changes
  - CustomInvariant(predicate: IRPredicate)                  # arbitrary graph predicate
```

### 6.3 CLI Enhancements

```bash
# Agent workflow
airl init myproject                          # create project
airl patch apply patch.json                  # apply semantic patch
airl patch preview patch.json                # dry-run a patch
airl patch undo <patch-id>                   # revert a patch
airl check                                   # type check + constraint check
airl run                                     # interpret main
airl build                                   # compile to native
airl build --target wasm                     # compile to WASM
airl test                                    # run all tests
airl bench                                   # run benchmarks
airl project --text typescript               # project to TypeScript
airl project --text python                   # project to Python
airl project --contracts                     # show API contracts
airl project --deps                          # show dependency graph
airl query "functions that call io::*"       # search the IR
airl diff v1 v2                              # diff two versions
airl api serve --port 9090                   # start API server
```

**Acceptance criteria:**
- AI agent (Claude/Codex) can create a project, add modules, apply patches, run tests, and compile — entirely through the API
- Round-trip test: create project via API → apply patches → compile → run → verify output
- Constraint violations are caught before patches are committed
- Text projection produces valid, readable TypeScript/Python

---

## Stage 7: Standard Library & Ecosystem (MVP: useful programs can be built)

**Goal:** Build enough standard library that real CLI and web backend programs can be written.

### 7.1 Core Library (`std`)

```
std::io          # file I/O, stdio, path operations
std::net         # TCP/UDP sockets, HTTP client
std::json        # JSON parse/serialize
std::collections # HashMap, BTreeMap, HashSet, Queue, Stack
std::string      # string manipulation, regex, formatting
std::math        # math functions, random
std::time        # timestamps, durations, timers
std::concurrency # async/await, channels, spawn
std::error       # error types, error chains
std::fmt         # string formatting, display
std::env         # environment variables, command-line args
std::process     # spawn child processes
std::crypto      # hashing (SHA256, etc.), HMAC
std::testing     # test framework, assertions, property testing
std::bench       # benchmarking framework
```

### 7.2 Web Backend Library (`web`)

```
web::http        # HTTP server, request/response, routing
web::middleware   # logging, auth, CORS, rate limiting
web::json        # JSON API helpers
web::database    # database driver interface
web::template    # HTML templating
```

### 7.3 Example Programs

- [ ] CLI tool: file search utility (like a simple `grep`)
- [ ] HTTP API server: REST API with JSON, routing, middleware
- [ ] Data processing: CSV parser and transformer
- [ ] Build tool: simple task runner (like a minimal `make`)

**Acceptance criteria:**
- Build and run each example program
- HTTP server handles 1000 req/s on a single core
- All std library functions have tests and documentation
- An AI agent can build a new CLI tool using only the API + std library

---

## Cross-Cutting Concerns (apply to every stage)

### Testing Strategy

Every stage must include:
1. **Golden tests:** Input IR → expected output (checked into repo)
2. **Property tests:** Random IR generation → invariant checking (e.g., serialize/deserialize roundtrip)
3. **Mutation tests:** Mutate IR → verify type checker catches it OR interpreter produces different output
4. **Regression tests:** Every bug fix gets a test
5. **Benchmark tests:** Track compilation time, interpretation speed, binary size

### Evidence Bundle

Every patch/change must produce:
```
EvidenceBundle:
  - test_results: TestResults
  - type_check_result: TypeCheckResult
  - constraint_check_result: ConstraintCheckResult
  - benchmark_delta: Option<BenchmarkDelta>
  - coverage_delta: Option<CoverageDelta>
  - impact_analysis: ImpactAnalysis { affected_functions, affected_modules }
```

### Documentation

- Each crate has `//!` module docs
- Each public API has `///` doc comments with examples
- `ARCHITECTURE.md` updated at each stage
- `CHANGELOG.md` with every stage completion
- API documentation auto-generated

---

## Dependency Graph Between Stages

```
Stage 0 (Bootstrap)
    |
Stage 1 (IR Core)
    |
    ├── Stage 2 (Type Checker)
    |       |
    |       ├── Stage 3 (Interpreter)
    |       |
    |       └── Stage 5 (Compiler) ── needs Stage 3 for validation
    |
    └── Stage 4 (Patch Engine) ── needs Stage 2 for patch validation
            |
Stage 6 (Agent API) ── needs Stages 2, 3, 4, 5
    |
Stage 7 (Std Library) ── needs Stage 6
```

**Critical path:** 0 → 1 → 2 → 3 → 4 → 6 → 7 (with Stage 5 parallelizable after Stage 2)

---

## Agent Instructions

When feeding this plan to an AI coding agent, use this prompt pattern for each stage:

```
You are implementing Stage N of the AIRL project.

Read these files for context:
- plan.md (this file — overall plan)
- architecture.md (system architecture)
- ir-spec.md (IR specification)
- agent-api.md (API specification)

Previous stages completed: [list]

Your task: implement all deliverables listed under Stage N in plan.md.

Hard constraints:
1. All tests must pass before marking the stage complete
2. Do not change public APIs from previous stages (backward compatible)
3. Every public function must have at least one test
4. Use the project's existing patterns and conventions
5. Run `cargo test` and `cargo clippy` before committing

Deliver:
- Working code for all deliverables
- Tests (golden + property where applicable)
- Updated ARCHITECTURE.md if new crates/modules were added
- CHANGELOG.md entry for this stage
```

---

## Success Metrics

| Metric | Target | Stage |
|---|---|---|
| IR round-trip fidelity | 100% (serialize→deserialize→serialize == original) | 1 |
| Type checker coverage | Rejects all ill-typed golden tests | 2 |
| Interpreter correctness | Matches expected output for all golden tests | 3 |
| Patch inverse property | apply(inverse(apply(patch))) == identity, 100% | 4 |
| Compiled output correctness | Matches interpreter output for all tests | 5 |
| API round-trip | Create→Patch→TypeCheck→Run via API works | 6 |
| Compilation speed (Cranelift) | <2s for 1000-node IR | 5 |
| Interpretation speed | >100K nodes/sec | 3 |
| Binary performance | 10x+ faster than interpreter | 5 |
| Agent autonomy | Agent completes a task (create CLI tool) without human intervention | 7 |

---

## Risk Register

| Risk | Impact | Mitigation |
|---|---|---|
| Effect system too complex for agents to use | Agents produce invalid IR | Start with 3 effects (Pure, IO, Fail), expand later |
| Patch conflicts in concurrent agent workflows | Lost work, inconsistent state | Implement optimistic concurrency with version checks |
| LLVM compilation too slow for feedback loops | Agents wait too long | Use Cranelift for dev, LLVM only for release builds |
| IR format changes break existing programs | Migration burden | Version the format; write automatic migration tools |
| Standard library too thin for real programs | Agents can't build useful things | Prioritize by agent demand: I/O, JSON, HTTP first |
| Type inference too limited | Agents must annotate everything | Keep annotations required (explicit is better for agents) |

---

## File References

- [architecture.md](./architecture.md) — System architecture and crate structure
- [ir-spec.md](./ir-spec.md) — Detailed IR specification with examples
- [agent-api.md](./agent-api.md) — Agent API specification
- [evaluation-rubric.md](./evaluation-rubric.md) — Success criteria and evaluation rubric
