# AIRL Stage 8+ Roadmap

All 7 original stages are complete. This roadmap defines the next priorities.

**Current test count: 177 tests passing** (up from 146 at stage 7 completion).

**All 7 priorities complete.**

---

## Priority List

| Priority | Feature | Why it matters | Status |
|---|---|---|---|
| **1** | **Text projections (IR → TypeScript/Python)** | `/project/text` now outputs real TypeScript and Python code. | **Done** |
| **2** | **File I/O builtins** | `read_file`, `write_file`, `read_dir`, `file_exists` in interpreter + type checker. | **Done** |
| **3** | **Compiler builtin expansion** | Cranelift JIT now handles `string::concat`, `string::len`, `string::from_i64`, `string::contains`, `math::abs/max/min/pow`, `io::print`. | **Done** |
| **4** | **WASM compilation target** | `airl compile --target wasm` emits valid WASM modules via `wasm-encoder`. API endpoint `POST /compile/wasm`. | **Done** |
| **5** | **JSON parse/serialize builtins** | `json::parse`, `json::serialize`, `json::serialize_pretty` in interpreter. | **Done** |
| **6** | **HashMap/collections** | `Value::Map` type, `new_map/insert/get/remove/contains_key/keys/values/map_len` builtins. | **Done** |
| **7** | **API auth + WASM endpoint** | Token-based `Authorization: Bearer` middleware, `POST /compile/wasm` endpoint, `serve_with_auth()`. | **Done** |

---

## Priority 1: Text Projections

### Goal
Replace the stub pseudocode projection in `/project/text` with real code generation for TypeScript and Python.

### Design
- New crate-internal module `airl-project/src/projections.rs` (or embedded in API handlers)
- Recursive IR node → source code translation
- Language-specific formatters for TypeScript and Python
- Handles: functions, let bindings, if/else, loops, calls, binops, arrays, structs, match

### Acceptance Criteria
- `POST /project/text {language: "typescript"}` returns valid TypeScript-like code
- `POST /project/text {language: "python"}` returns valid Python-like code
- All example programs (hello, fibonacci, fizzbuzz, string_ops) produce readable output

---

## Priority 2: File I/O Builtins

### Goal
Add `std::io::read_file`, `std::io::write_file`, `std::io::read_dir` to the interpreter.

### Design
- Interpreter builtins that perform real filesystem operations
- Returns `Result`-like values (value or Unit on error)
- Type checker recognizes the new builtin signatures
- Sandboxed: restricted to working directory by default

### Acceptance Criteria
- Programs can read/write files via interpreter
- New golden test: file I/O program
- Type checker validates effect requirements (IO)

---

## Priority 3: Compiler Builtin Expansion

### Goal
Extend the Cranelift JIT to handle builtins beyond just `println`.

### Design
- Register runtime helper functions for string ops (concat, len, etc.)
- Register runtime helpers for math ops (abs, pow, etc.)
- String values in JIT use ptr+len representation
- Array values use heap-allocated buffers with length tracking

### Acceptance Criteria
- `std::string::concat`, `std::string::len` work in compiled code
- `std::math::abs`, `std::math::max`, `std::math::min` work in compiled code
- `std::string::from_i64` works (needed for fizzbuzz compilation)
- Golden tests: compiled output matches interpreter output for string_ops

---

## Priority 4: WASM Compilation Target

### Goal
Add `--target wasm` to the CLI compiler using Cranelift's WASM backend.

### Design
- Use `cranelift-wasm` or target `wasm32` ISA
- WASI support for I/O
- Output `.wasm` file
- CLI: `airl compile <file> --target wasm -o output.wasm`

---

## Priority 5: JSON Builtins

### Goal
Add `std::json::parse` and `std::json::serialize` to the interpreter.

### Design
- `parse(s: String) -> Value` — returns AIRL Struct/Array/String/Integer values
- `serialize(v: Value) -> String` — produces JSON string
- Maps JSON objects → AIRL Struct, arrays → Array, numbers → Integer/Float

---

## Priority 6: HashMap/Collections

### Goal
Add `HashMap` as a new Value type and collection builtins.

### Design
- New `Value::Map(BTreeMap<String, Value>)` variant
- `std::collections::new_map`, `insert`, `get`, `remove`, `keys`, `values`, `contains_key`
- Type system: `Map<K, V>` type

---

## Priority 7: gRPC API + Auth

### Goal
Replace/augment HTTP API with gRPC for production agent interfaces.

### Design
- Tonic-based gRPC service
- Token-based authentication
- Streaming patch application
- Multi-agent session support
