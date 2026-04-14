# AIRL

**AI-native Intermediate Representation Language**

AIRL is a typed program graph designed as the primary authoring target for AI coding agents. Instead of generating text files, agents manipulate a structured IR through semantic patch operations. Human-readable code is an output projection, not the source of truth.

## What AIRL Is

- A **typed IR graph** (nodes + edges, not text) with explicit effects
- Manipulated via **semantic patches** (add function, replace node, rename symbol) instead of full-file rewrites
- **Interpreted** for fast feedback (tree-walking interpreter)
- **Compiled** to native code via Cranelift JIT for production speed
- Served via **HTTP API** for AI agent integration
- **Type-checked** with effect tracking (Pure, IO, Fail, etc.)

## What AIRL Is Not

- Not a new programming language syntax for humans
- Not a text format that gets parsed (the IR *is* the source of truth)
- Not a wrapper around an existing language

## Architecture

```
AI Coding Agent
      |  JSON / HTTP
      v
+------------------+     +-------------+     +-----------+
|   Agent API      |---->|  Semantic    |---->|   Type    |
|  (axum HTTP)     |     |  Patch      |     |  Checker  |
|  13 endpoints    |     |  Engine     |     |  + Effects|
+------------------+     +-------------+     +-----------+
      |                                            |
      v                                            v
+------------------+     +-------------+     +-----------+
|  Interpreter     |     |  Cranelift  |     |    IR     |
|  (tree-walking)  |     |  JIT        |     |   Core    |
|  33 builtins     |     |  Compiler   |     |  16 nodes |
+------------------+     +-------------+     +-----------+
```

## Quick Start

```bash
# Build
cargo build

# Run a program (interpreter)
cargo run -p airl-cli -- run examples/hello.airl.json

# Run a program (compiled via Cranelift JIT)
cargo run -p airl-cli -- compile examples/fibonacci.airl.json

# Type check
cargo run -p airl-cli -- check examples/fizzbuzz.airl.json

# Apply a semantic patch
cargo run -p airl-cli -- patch examples/hello.airl.json examples/change-greeting.patch.json -o /tmp/patched.airl.json

# Start the HTTP API server
cargo run -p airl-cli -- api serve --port 9090

# Run all tests
cargo test
```

## Examples

### Hello World (`examples/hello.airl.json`)

The IR is a JSON graph. Here's the structure (simplified):

```json
{
  "functions": [{
    "name": "main",
    "effects": ["IO"],
    "body": {
      "kind": "Call",
      "target": "std::io::println",
      "args": [{ "kind": "Literal", "type": "String", "value": "hello world" }]
    }
  }]
}
```

```
$ cargo run -p airl-cli -- run examples/hello.airl.json
hello world
```

### Semantic Patch

Instead of rewriting the entire file, agents produce targeted patches:

```json
{
  "operations": [{
    "kind": "ReplaceNode",
    "target": "n_101",
    "replacement": { "kind": "Literal", "type": "String", "value": "hello from a patch!" }
  }],
  "rationale": "Change the greeting message"
}
```

Patches are validated, invertible (every patch has an exact undo), and produce a new content-addressed version.

### HTTP API

```bash
# Create a project
curl -X POST http://localhost:9090/project/create \
  -H 'Content-Type: application/json' \
  -d '{"name": "my-project", "module_json": "..."}'

# Apply a patch
curl -X POST http://localhost:9090/patch/apply \
  -H 'Content-Type: application/json' \
  -d '{"id": "p1", "operations": [...], "rationale": "add feature"}'

# Run the program
curl -X POST http://localhost:9090/interpret \
  -H 'Content-Type: application/json' -d '{}'
```

## Project Structure

```
crates/
  airl-ir/          # Core IR: 16 node types, type system, effects, JSON serde
  airl-typecheck/   # Bidirectional type checker with effect checking
  airl-interp/      # Tree-walking interpreter with 33 builtins
  airl-compile/     # Cranelift JIT compiler (IR -> native code)
  airl-patch/       # Semantic patch engine (apply, validate, invert)
  airl-project/     # Project state management, history, queries
  airl-api/         # HTTP API server (axum, 13 endpoints)
  airl-cli/         # CLI binary (run, check, compile, patch, api serve)
examples/           # Example IR programs and patches
docs/               # Design documents and specifications
```

## Features

| Feature | Status | Details |
|---|---|---|
| IR Core | Complete | 16 node types, algebraic type system, 7 effect types |
| Type Checker | Complete | Bidirectional checking, effect verification, 33 builtin signatures |
| Interpreter | Complete | All node types, recursion, execution limits, 33 builtins |
| Cranelift Compiler | Complete | JIT compilation, integer/string programs, recursive functions |
| Patch Engine | Complete | 8 operations, validation, inversion, impact analysis |
| Project Management | Complete | Patch history, undo, queries, version tracking |
| HTTP API | Complete | 13 endpoints, JSON request/response |
| Standard Library | Complete (MVP) | I/O, strings, math, arrays, formatting |

## Built-in Functions (33)

| Module | Functions |
|---|---|
| `std::io` | println, print, eprintln, read_line |
| `std::string` | len, concat, contains, split, starts_with, ends_with, trim, to_uppercase, to_lowercase, replace, from_i64, to_i64 |
| `std::math` | abs, max, min, pow, sqrt, floor, ceil |
| `std::array` | len, push, get, slice, contains, reverse, join, range |
| `std::fmt` | format |
| `std::env` | args |

## Tests

146 tests across 8 crates, all passing:

```
airl-ir:        30 tests  (node/type/effect serialization, round-trips)
airl-typecheck: 25 tests  (valid programs, 18 error rejections)
airl-interp:    33 tests  (all builtins, recursion, limits, data structures)
airl-compile:   10 tests  (JIT: arithmetic, if/else, recursion, strings)
airl-patch:     19 tests  (replace, add/remove, rename, inversion property)
airl-project:    8 tests  (create, patch, undo, queries)
airl-api:       14 tests  (all endpoints, end-to-end workflow)
golden:          7 tests  (compiled == interpreted, all examples)
```

## Documentation

- [Implementation Plan](docs/plan.md) — 7-stage build plan with acceptance criteria
- [Architecture](docs/architecture.md) — System design, crate structure, data flow
- [IR Specification](docs/ir-spec.md) — Node types, type system, effects, JSON format
- [Agent API](docs/agent-api.md) — Full HTTP API specification with request/response examples
- [Evaluation Rubric](docs/evaluation-rubric.md) — Success criteria for each stage
- [Design Discussion](docs/discussion.md) — Initial design exploration for AI-native languages

## License

MIT
