# AIRL Architecture

## System Overview

```
┌─────────────────────────────────────────────────────────┐
│                    AI Coding Agent                       │
│              (Claude, Codex, etc.)                       │
└──────────────────────┬──────────────────────────────────┘
                       │ gRPC / JSON-RPC / CLI
                       ▼
┌─────────────────────────────────────────────────────────┐
│                   Agent API Layer                        │
│  ┌──────────┐ ┌──────────┐ ┌───────────┐ ┌──────────┐  │
│  │ Project  │ │  Patch   │ │  Query    │ │Projection│  │
│  │ Mgmt     │ │  Engine  │ │  Engine   │ │  Engine  │  │
│  └──────────┘ └──────────┘ └───────────┘ └──────────┘  │
└──────────────────────┬──────────────────────────────────┘
                       │
┌──────────────────────┼──────────────────────────────────┐
│                 Core Pipeline                            │
│                      │                                   │
│  ┌───────┐    ┌──────┴──────┐    ┌───────────────────┐  │
│  │  IR   │───▶│ Type Checker │───▶│   Interpreter     │  │
│  │ Store │    └─────────────┘    │   (fast feedback)  │  │
│  │       │           │           └───────────────────┘  │
│  │       │           ▼                                   │
│  │       │    ┌─────────────┐    ┌───────────────────┐  │
│  │       │    │ Constraint  │    │    Compiler        │  │
│  │       │    │  Checker    │    │  ┌──────────────┐  │  │
│  │       │    └─────────────┘    │  │  Cranelift   │  │  │
│  │       │                       │  │  (fast dev)  │  │  │
│  │       │                       │  ├──────────────┤  │  │
│  │       │                       │  │    LLVM      │  │  │
│  │       │                       │  │ (optimized)  │  │  │
│  │       │                       │  ├──────────────┤  │  │
│  │       │                       │  │    WASM      │  │  │
│  │       │                       │  │  (portable)  │  │  │
│  │       │                       │  └──────────────┘  │  │
│  └───────┘                       └───────────────────┘  │
└─────────────────────────────────────────────────────────┘
                       │
                       ▼
              ┌─────────────────┐
              │  Native Binary  │
              │  WASM Module    │
              │  Text Projection│
              └─────────────────┘
```

---

## Crate Structure

```
airl/
├── Cargo.toml                  # Workspace root
│
├── crates/
│   ├── airl-ir/                # IR data structures & serialization
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── node.rs         # Node types (the graph nodes)
│   │   │   ├── types.rs        # Type system
│   │   │   ├── effects.rs      # Effect system
│   │   │   ├── module.rs       # Module, imports, exports
│   │   │   ├── graph.rs        # IRGraph container + traversal
│   │   │   ├── ids.rs          # NodeId, TypeId, FuncId, etc.
│   │   │   ├── symbol.rs       # Interned symbols/strings
│   │   │   ├── serialize.rs    # JSON + binary serialization
│   │   │   ├── version.rs      # Content-addressable versioning
│   │   │   └── display.rs      # Debug display for IR nodes
│   │   └── Cargo.toml
│   │
│   ├── airl-typecheck/         # Type checker
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── checker.rs      # Main type checking pass
│   │   │   ├── inference.rs    # Local type inference
│   │   │   ├── effects.rs      # Effect checking
│   │   │   ├── traits.rs       # Trait resolution
│   │   │   ├── generics.rs     # Generic instantiation
│   │   │   └── diagnostic.rs   # Error messages
│   │   └── Cargo.toml
│   │
│   ├── airl-interp/            # Tree-walking interpreter
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── interpreter.rs  # Main eval loop
│   │   │   ├── value.rs        # Runtime values
│   │   │   ├── frame.rs        # Call stack frames
│   │   │   ├── builtins.rs     # Built-in functions
│   │   │   ├── limits.rs       # Execution budgets
│   │   │   └── repl.rs         # Interactive REPL mode
│   │   └── Cargo.toml
│   │
│   ├── airl-compile/           # Compiler backends
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── lower.rs        # IR → backend-neutral lowered form
│   │   │   ├── cranelift.rs    # Cranelift backend
│   │   │   ├── llvm.rs         # LLVM backend
│   │   │   ├── wasm.rs         # WASM backend
│   │   │   ├── linker.rs       # Linking and binary output
│   │   │   └── optimize.rs     # IR-level optimizations
│   │   └── Cargo.toml
│   │
│   ├── airl-patch/             # Semantic patch engine
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── patch.rs        # Patch data structures
│   │   │   ├── apply.rs        # Patch application
│   │   │   ├── validate.rs     # Patch validation
│   │   │   ├── inverse.rs      # Patch inversion
│   │   │   ├── merge.rs        # Patch merging/conflict detection
│   │   │   ├── diff.rs         # Version diffing
│   │   │   └── impact.rs       # Impact analysis
│   │   └── Cargo.toml
│   │
│   ├── airl-project/           # Project & module management
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── project.rs      # Project container
│   │   │   ├── config.rs       # Project configuration
│   │   │   ├── constraint.rs   # Constraint definitions & checking
│   │   │   ├── evidence.rs     # Evidence bundle management
│   │   │   ├── history.rs      # Version history (DAG)
│   │   │   └── storage.rs      # On-disk persistence
│   │   └── Cargo.toml
│   │
│   ├── airl-project-text/      # Human-readable projections
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── typescript.rs   # IR → TypeScript
│   │   │   ├── python.rs       # IR → Python
│   │   │   ├── go.rs           # IR → Go
│   │   │   ├── rust_proj.rs    # IR → Rust
│   │   │   ├── contracts.rs    # IR → contract/interface view
│   │   │   └── diagram.rs      # IR → Mermaid diagrams
│   │   └── Cargo.toml
│   │
│   ├── airl-cli/               # CLI binary
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   └── commands/
│   │   │       ├── mod.rs
│   │   │       ├── run.rs      # airl run
│   │   │       ├── build.rs    # airl build / compile
│   │   │       ├── check.rs    # airl check
│   │   │       ├── patch.rs    # airl patch apply/preview/undo
│   │   │       ├── project.rs  # airl project (projections)
│   │   │       ├── test.rs     # airl test
│   │   │       ├── bench.rs    # airl bench
│   │   │       ├── query.rs    # airl query
│   │   │       ├── diff.rs     # airl diff
│   │   │       ├── init.rs     # airl init
│   │   │       └── api.rs      # airl api serve
│   │   └── Cargo.toml
│   │
│   ├── airl-api/               # gRPC/HTTP API server
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── server.rs       # Server setup
│   │   │   ├── routes.rs       # HTTP routes
│   │   │   ├── grpc.rs         # gRPC service impl
│   │   │   └── handlers/
│   │   │       ├── mod.rs
│   │   │       ├── project.rs  # Project management handlers
│   │   │       ├── patch.rs    # Patch operation handlers
│   │   │       ├── build.rs    # Build/run handlers
│   │   │       ├── query.rs    # Query handlers
│   │   │       └── evidence.rs # Test/bench handlers
│   │   └── Cargo.toml
│   │
│   └── airl-std/               # Standard library (IR definitions)
│       ├── src/
│       │   ├── lib.rs
│       │   ├── io.rs           # I/O functions
│       │   ├── net.rs          # Networking
│       │   ├── json.rs         # JSON
│       │   ├── collections.rs  # Data structures
│       │   ├── string.rs       # String operations
│       │   ├── math.rs         # Math functions
│       │   ├── time.rs         # Time/date
│       │   ├── concurrency.rs  # Async/channels
│       │   └── testing.rs      # Test framework
│       └── Cargo.toml
│
├── proto/                      # Protobuf/FlatBuffers schemas
│   ├── airl_ir.fbs             # FlatBuffers schema for IR
│   └── airl_api.proto          # gRPC service definition
│
├── examples/                   # Example AIRL programs (JSON IR)
│   ├── hello.airl.json
│   ├── fibonacci.airl.json
│   ├── fizzbuzz.airl.json
│   ├── http_server.airl.json
│   └── file_search.airl.json
│
├── tests/
│   ├── golden/                 # Golden test fixtures
│   │   ├── typecheck/          # Expected type-check results
│   │   ├── interpret/          # Expected execution results
│   │   ├── compile/            # Expected compilation results
│   │   └── patch/              # Expected patch results
│   ├── property/               # Property test generators
│   └── integration/            # End-to-end tests
│
├── ARCHITECTURE.md             # This file
├── CHANGELOG.md
└── README.md
```

---

## Data Flow

### Agent creates a new program:

```
Agent                   API                    Core
  │                      │                      │
  ├─ CreateProject ─────▶│                      │
  │                      ├─ init project ──────▶│
  │◀── ProjectId ────────│                      │
  │                      │                      │
  ├─ CreateModule ──────▶│                      │
  │   (initial IR)       ├─ store IR ──────────▶│
  │                      ├─ typecheck ─────────▶│
  │◀── ModuleId + diags ─│◀── TypeCheckResult ──│
  │                      │                      │
  ├─ Interpret ─────────▶│                      │
  │                      ├─ interpret ─────────▶│
  │◀── output ───────────│◀── RunResult ────────│
```

### Agent edits an existing program:

```
Agent                   API                    Core
  │                      │                      │
  ├─ ApplyPatch ────────▶│                      │
  │   (patch ops)        ├─ validate patch ────▶│
  │                      ├─ apply patch ───────▶│
  │                      ├─ typecheck ─────────▶│
  │                      ├─ check constraints ─▶│
  │◀── PatchResult ──────│◀── new version ──────│
  │   (version, diags)   │                      │
  │                      │                      │
  ├─ RunTests ──────────▶│                      │
  │                      ├─ interpret tests ───▶│
  │◀── TestResults ──────│◀── results ──────────│
  │                      │                      │
  ├─ Compile ───────────▶│                      │
  │                      ├─ lower + codegen ───▶│
  │◀── binary path ──────│◀── binary ───────────│
```

---

## Key Design Decisions

### 1. Why a graph, not text?

Text-based code requires:
- Parsing (error-prone, ambiguous)
- Pretty-printing (layout decisions)
- Text-based diffing (line-oriented, loses semantics)
- Re-parsing after every edit

A graph IR eliminates all of these. Agents manipulate structure directly. Humans see projections.

### 2. Why explicit effects?

Without explicit effects, an agent cannot know if adding a function call will introduce I/O, allocation, or failure modes. Explicit effects make the impact of every change visible and checkable.

### 3. Why semantic patches instead of full rewrites?

Full rewrites are:
- Expensive (regenerate entire module for a one-line change)
- Hard to review (diff is the whole file)
- Hard to merge (no structural awareness)

Semantic patches are:
- Minimal (only describe the change)
- Reviewable (each operation has clear semantics)
- Mergeable (non-overlapping patches compose automatically)
- Invertible (every patch has an exact undo)

### 4. Why Cranelift AND LLVM?

- **Cranelift:** Fast compilation (~10ms for small programs), good for development/feedback loops
- **LLVM:** Slow compilation (~1s+), but highly optimized output for production
- Agents use Cranelift during development, LLVM for final builds

### 5. Why require type annotations (no global inference)?

Global type inference (like Haskell's) creates non-local effects: changing one function can change the inferred type of a distant function. This makes semantic patches unpredictable.

Local inference (infer within function bodies, require signatures) gives agents a stable contract: the function signature is always explicit, and patching the body doesn't change the interface.

### 6. Why version as content hash?

Content-addressable versioning means:
- Two agents that make the same change produce the same version
- No mutable state, no race conditions
- Efficient storage (deduplicate identical subgraphs)
- Deterministic builds

---

## Concurrency Model

The system supports multiple agents working on the same project concurrently:

1. Each agent works on a **branch** (fork of the version DAG)
2. Patches are applied optimistically to the branch
3. Merging branches uses structural patch merging:
   - Non-overlapping patches: automatic merge
   - Overlapping patches: conflict reported, agent must resolve
4. The **main** branch requires all constraints to pass before accepting a merge

This is analogous to git branching, but at the IR-graph level with structural (not textual) merge.

---

## Security Model

Since agents operate autonomously, the system includes:

1. **Execution limits:** Interpreter and compiler have configurable time/memory/step budgets
2. **Effect restrictions:** Agents can be restricted to specific effect sets (e.g., no IO, no network)
3. **Constraint enforcement:** Constraints are checked automatically; violations block patches
4. **Audit log:** Every patch records author, timestamp, rationale, and evidence
5. **Separation of duties:** Author agent and review agent must be different identities
