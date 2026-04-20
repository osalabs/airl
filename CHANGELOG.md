# Changelog

All notable changes to AIRL are documented here.

## [0.1.0] - 2026-04

Initial release.

### Core
- IR with 16 node types, full type system with effect tracking
- Bidirectional type checker with effect verification
- Tree-walking interpreter with 74 builtins
- Cranelift JIT compiler (23 natively-compiled builtins)
- WASM backend with WASI integer printing, recursion, match
- Semantic patch engine (apply, validate, invert)
- Multi-module workspace with import resolution
- HTTP API server with Bearer token authentication
- TypeScript and Python text projections

### Standard Library (74 builtins across 15 modules)
- `std::io` — println, print, eprintln, read_line, read_file, write_file, read_dir, file_exists
- `std::string` — len, concat, contains, split, starts_with, ends_with, trim,
  to_uppercase, to_lowercase, replace, from_i64, to_i64, index_of, substring,
  chars, repeat, parse_int
- `std::math` — abs, max, min, pow, sqrt, floor, ceil
- `std::array` — len, push, get, slice, contains, reverse, join, range
- `std::fmt` — format
- `std::env` — args
- `std::json` — parse, serialize, serialize_pretty
- `std::collections` — new_map, insert, get, remove, contains_key, keys, values, map_len
- `std::error` — is_unit, unwrap_or, assert, panic
- `std::process` — exit, exec, env_var, set_env_var
- `std::time` — now_ms, now_secs, sleep_ms
- `std::crypto` — sha256
- `std::testing` — assert_eq, assert_ne, assert_true
- `std::net` — http_get, http_post, serve_once
- `std::concurrency` — spawn, await_result, sleep, thread_id

### Tools
- `airl run` — interpret programs
- `airl run --compiled` — Cranelift JIT execution
- `airl run --include <dir>` — multi-module workspace
- `airl compile --target wasm` — emit WASM binary
- `airl check` — type-check only
- `airl project --lang ts|py` — text projection
- `airl repl` — interactive REPL
- `airl patch` — apply semantic patch
- `airl api serve [--auth-tokens tok]` — HTTP API server

### Tests
- 217 tests across 8 crates, all passing
- Property-based tests for IR serialization, interpreter consistency
- Benchmarks for interpreter, compiler, type checker, WASM, JSON
- Golden tests verifying compiled == interpreted for all examples

### Examples (12 programs)
- hello, fibonacci, fizzbuzz, string_ops
- file_search, json_processor, kv_store
- http_client, self_test, concurrency
- multi/main + multi/mathlib (cross-module imports)
