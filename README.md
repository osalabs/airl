# AIRL

**AI-native Intermediate Representation Language**

AIRL is a typed program graph designed as the primary authoring target for AI coding agents. Instead of generating text files, agents manipulate a structured IR through semantic patch operations. Human-readable code is an output projection, not the source of truth.

## What AIRL Is

- A **typed IR graph** (nodes + edges, not text) with explicit effects
- Manipulated via **semantic patches** (add function, replace node, rename symbol) instead of full-file rewrites
- **Interpreted** for fast feedback (tree-walking interpreter with 69 builtins)
- **Compiled** to native code via Cranelift JIT (23 natively-compiled builtins)
- **Compiled to WASM** for browser and edge deployment
- **Projected** to TypeScript or Python for human reading
- Served via **HTTP API** with token authentication for AI agent integration
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
|   Agent API      |---->|  Semantic   |---->|   Type    |
|  (axum HTTP)     |     |  Patch      |     |  Checker  |
|  15 endpoints    |     |  Engine     |     |  + Effects|
|  Bearer auth     |     |  invertible |     |  + hints  |
+------------------+     +-------------+     +-----------+
      |                                            |
      v                                            v
+------------------+     +-------------+     +-----------+
|  Interpreter     |     |  Cranelift  |     |    IR     |
|  (tree-walking)  |     |  JIT + WASM |     |   Core    |
|  69 builtins     |     |  Compiler   |     |  16 nodes |
+------------------+     +-------------+     +-----------+
      |                        |
      v                        v
+------------------+     +-------------+
|  TypeScript /    |     |  .wasm      |
|  Python output   |     |  binaries   |
+------------------+     +-------------+
```

## Quick Start

```bash
# Build
cargo build

# Run a program (interpreter)
cargo run -p airl-cli -- run examples/hello.airl.json

# Run a program (compiled via Cranelift JIT)
cargo run -p airl-cli -- run examples/fibonacci.airl.json --compiled

# Compile to WASM
cargo run -p airl-cli -- compile examples/fibonacci.airl.json --target wasm -o fib.wasm

# Type check
cargo run -p airl-cli -- check examples/fizzbuzz.airl.json

# Project to TypeScript
cargo run -p airl-cli -- project examples/fizzbuzz.airl.json --lang typescript

# Project to Python
cargo run -p airl-cli -- project examples/fibonacci.airl.json --lang python

# Interactive REPL
cargo run -p airl-cli -- repl

# Apply a semantic patch
cargo run -p airl-cli -- patch examples/hello.airl.json examples/change-greeting.patch.json -o /tmp/patched.airl.json

# Start the HTTP API server
cargo run -p airl-cli -- api serve --port 9090

# Start with authentication
cargo run -p airl-cli -- api serve --port 9090 --auth-tokens my-secret-token

# Run all tests (215 tests)
cargo test --workspace
```

## Examples (10 programs)

| Example | Description |
|---|---|
| `hello.airl.json` | Hello world |
| `fibonacci.airl.json` | Recursive Fibonacci (0-9) |
| `fizzbuzz.airl.json` | FizzBuzz 1-20 with string operations |
| `string_ops.airl.json` | String, array, math, formatting builtins |
| `file_search.airl.json` | List directory, check file existence |
| `json_processor.airl.json` | Parse JSON, transform, pretty-print |
| `kv_store.airl.json` | HashMap operations (insert, get, keys) |
| `http_client.airl.json` | HTTP GET request with response parsing |
| `self_test.airl.json` | Self-test using the testing framework |
| `change-greeting.patch.json` | Example semantic patch |

## CLI Commands

```
airl run <file>                          # Interpret
airl run <file> --compiled               # JIT compile + run
airl compile <file>                      # Cranelift JIT
airl compile <file> --target wasm -o f.wasm  # Compile to WASM
airl check <file>                        # Type check only
airl project <file> --lang ts|py         # Text projection
airl repl                                # Interactive REPL
airl patch <module> <patch> [-o out]     # Apply semantic patch
airl api serve [--port 9090] [--auth-tokens tok1,tok2]
```

## HTTP API (15 endpoints)

| Endpoint | Method | Description |
|---|---|---|
| `/project/create` | POST | Create project from IR JSON |
| `/project` | GET | Get project info |
| `/module` | GET | Get current module |
| `/patch/apply` | POST | Apply semantic patch |
| `/patch/preview` | POST | Preview patch (dry-run) |
| `/patch/undo` | POST | Undo last patch |
| `/typecheck` | POST | Run type checker |
| `/interpret` | POST | Run interpreter |
| `/compile` | POST | Cranelift JIT compile + run |
| `/compile/wasm` | POST | Compile to WASM binary |
| `/query/functions` | GET | Search functions |
| `/query/call-graph` | GET | Get call graph edges |
| `/query/effects` | GET | Get effect summary |
| `/project/text` | POST | Project to TypeScript/Python |

All endpoints support `Authorization: Bearer <token>` authentication.

## Built-in Functions (69)

| Module | Functions | Count |
|---|---|---|
| `std::io` | println, print, eprintln, read_line, read_file, write_file, read_dir, file_exists | 8 |
| `std::string` | len, concat, contains, split, starts_with, ends_with, trim, to_uppercase, to_lowercase, replace, from_i64, to_i64, index_of, substring, chars, repeat, parse_int | 17 |
| `std::math` | abs, max, min, pow, sqrt, floor, ceil | 7 |
| `std::array` | len, push, get, slice, contains, reverse, join, range | 8 |
| `std::fmt` | format | 1 |
| `std::env` | args | 1 |
| `std::json` | parse, serialize, serialize_pretty | 3 |
| `std::collections` | new_map, insert, get, remove, contains_key, keys, values, map_len | 8 |
| `std::error` | is_unit, unwrap_or, assert, panic | 4 |
| `std::process` | exit, exec, env_var, set_env_var | 4 |
| `std::time` | now_ms, now_secs, sleep_ms | 3 |
| `std::crypto` | sha256 | 1 |
| `std::testing` | assert_eq, assert_ne, assert_true | 3 |
| `std::net` | http_get, http_post | 2 |

## Project Structure

```
crates/
  airl-ir/          # Core IR: 16 node types, type system, effects, JSON serde
  airl-typecheck/   # Bidirectional type checker with effect checking + hints
  airl-interp/      # Tree-walking interpreter with 69 builtins
  airl-compile/     # Cranelift JIT + WASM compiler
  airl-patch/       # Semantic patch engine (apply, validate, invert)
  airl-project/     # Project management, history, queries, projections, workspace
  airl-api/         # HTTP API server (axum, 15 endpoints, Bearer auth)
  airl-cli/         # CLI binary (run, compile, check, project, repl, patch, api)
examples/           # 10 example IR programs and patches
docs/               # Design documents and specifications
```

## Tests (215)

```
airl-api:       22 tests  (endpoints, auth, WASM, projections)
airl-cli:       21 tests  (golden: compiled==interpreted, benchmarks, output checks)
airl-compile:   23 tests  (JIT, WASM validation, match, fizzbuzz)
airl-interp:    51 tests  (builtins, property tests, error handling)
airl-ir:        35 tests  (serde roundtrips, property tests, version hashing)
airl-patch:     19 tests  (operations, inversion, validation)
airl-project:   19 tests  (history, queries, projections, workspace)
airl-typecheck: 25 tests  (valid programs, error rejections, effect checking)
```

## Documentation

- [Implementation Plan](docs/plan.md) -- 7-stage build plan with acceptance criteria
- [Stage 8+ Roadmap](docs/roadmap-stage8.md) -- Post-v1 priorities and status
- [Architecture](docs/architecture.md) -- System design, crate structure, data flow
- [IR Specification](docs/ir-spec.md) -- Node types, type system, effects, JSON format
- [Agent API](docs/agent-api.md) -- Full HTTP API specification
- [Evaluation Rubric](docs/evaluation-rubric.md) -- Success criteria for each stage

## License

MIT
