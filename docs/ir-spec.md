# AIRL IR Specification

## Overview

The AIRL IR is a **typed, directed acyclic graph** (DAG) where:
- **Nodes** represent computations, values, and declarations
- **Edges** represent data flow and structural relationships
- **Types** are explicit on every node
- **Effects** are declared on every function boundary

The IR is the **source of truth**. Text code is a projection.

---

## 1. Identifiers

All entities in the IR use typed identifiers:

```rust
// All IDs are 64-bit, content-addressable where possible
struct NodeId(u64);      // computation graph node
struct TypeId(u64);      // type definition
struct FuncId(u64);      // function definition
struct ModuleId(u64);    // module
struct TraitId(u64);     // trait/interface
struct ImplId(u64);      // trait implementation
struct ConstId(u64);     // compile-time constant
struct VersionId([u8; 32]);  // SHA-256 content hash
struct PatchId(Uuid);    // patch identifier
struct Symbol(u32);      // interned string (index into symbol table)
```

All IDs are **stable across serialization** — the same IR graph always produces the same IDs.

---

## 2. Node Types

### 2.1 Literals

```json
{
  "id": "n_001",
  "kind": "Literal",
  "type": "I64",
  "value": 42
}

{
  "id": "n_002",
  "kind": "Literal",
  "type": "String",
  "value": "hello world"
}

{
  "id": "n_003",
  "kind": "Literal",
  "type": "Bool",
  "value": true
}

{
  "id": "n_004",
  "kind": "Literal",
  "type": "F64",
  "value": 3.14159
}

{
  "id": "n_005",
  "kind": "Literal",
  "type": "Unit",
  "value": null
}
```

### 2.2 Variables and Bindings

```json
{
  "id": "n_010",
  "kind": "Param",
  "name": "x",
  "index": 0,
  "type": "I64"
}

{
  "id": "n_011",
  "kind": "Let",
  "name": "result",
  "type": "I64",
  "value": "n_012",     // NodeId of the value expression
  "body": "n_013"       // NodeId of the continuation
}
```

### 2.3 Operations

```json
{
  "id": "n_020",
  "kind": "BinOp",
  "op": "Add",
  "type": "I64",
  "lhs": "n_010",
  "rhs": "n_001"
}

{
  "id": "n_021",
  "kind": "BinOp",
  "op": "Eq",
  "type": "Bool",
  "lhs": "n_010",
  "rhs": "n_001"
}

{
  "id": "n_022",
  "kind": "UnaryOp",
  "op": "Neg",
  "type": "I64",
  "operand": "n_010"
}
```

**BinOp kinds:** `Add`, `Sub`, `Mul`, `Div`, `Mod`, `Eq`, `Neq`, `Lt`, `Lte`, `Gt`, `Gte`, `And`, `Or`, `BitAnd`, `BitOr`, `BitXor`, `Shl`, `Shr`

**UnaryOp kinds:** `Neg`, `Not`, `BitNot`

### 2.4 Control Flow

```json
{
  "id": "n_030",
  "kind": "If",
  "type": "I64",
  "cond": "n_021",
  "then_branch": "n_031",
  "else_branch": "n_032"
}

{
  "id": "n_033",
  "kind": "Match",
  "type": "String",
  "scrutinee": "n_040",
  "arms": [
    { "pattern": { "kind": "Literal", "value": 1 }, "body": "n_041" },
    { "pattern": { "kind": "Literal", "value": 2 }, "body": "n_042" },
    { "pattern": { "kind": "Wildcard" }, "body": "n_043" }
  ]
}

{
  "id": "n_034",
  "kind": "Loop",
  "type": "Unit",
  "body": "n_050",
  "break_value": null
}

{
  "id": "n_035",
  "kind": "Block",
  "type": "I64",
  "statements": ["n_060", "n_061", "n_062"],
  "result": "n_063"
}
```

### 2.5 Functions and Calls

```json
{
  "id": "n_070",
  "kind": "Call",
  "type": "Unit",
  "target": "f_println",
  "args": ["n_002"]
}

{
  "id": "n_071",
  "kind": "Return",
  "type": "I64",
  "value": "n_020"
}
```

### 2.6 Data Structures

```json
{
  "id": "n_080",
  "kind": "StructLiteral",
  "type": "t_Point",
  "fields": {
    "x": "n_001",
    "y": "n_002"
  }
}

{
  "id": "n_081",
  "kind": "FieldAccess",
  "type": "I64",
  "object": "n_080",
  "field": "x"
}

{
  "id": "n_082",
  "kind": "ArrayLiteral",
  "type": "Array<I64>",
  "elements": ["n_001", "n_002", "n_003"]
}

{
  "id": "n_083",
  "kind": "IndexAccess",
  "type": "I64",
  "array": "n_082",
  "index": "n_001"
}
```

### 2.7 Error Handling

```json
{
  "id": "n_090",
  "kind": "TryCall",
  "type": "Result<String, IOError>",
  "target": "f_read_file",
  "args": ["n_path"],
  "on_error": "n_091"
}

{
  "id": "n_092",
  "kind": "PropagateError",
  "type": "String",
  "inner": "n_090"
}
```

---

## 3. Type System

### 3.1 Primitive Types

| Type | Size | Description |
|---|---|---|
| `Unit` | 0 | No value (like void) |
| `Bool` | 1 byte | true/false |
| `I8` | 1 byte | Signed 8-bit integer |
| `I16` | 2 bytes | Signed 16-bit integer |
| `I32` | 4 bytes | Signed 32-bit integer |
| `I64` | 8 bytes | Signed 64-bit integer |
| `U8` | 1 byte | Unsigned 8-bit integer |
| `U16` | 2 bytes | Unsigned 16-bit integer |
| `U32` | 4 bytes | Unsigned 32-bit integer |
| `U64` | 8 bytes | Unsigned 64-bit integer |
| `F32` | 4 bytes | 32-bit float (IEEE 754) |
| `F64` | 8 bytes | 64-bit float (IEEE 754) |
| `String` | ptr+len | UTF-8 string |
| `Bytes` | ptr+len | Raw byte array |

### 3.2 Composite Types

```json
// Struct
{
  "id": "t_Point",
  "kind": "Struct",
  "name": "Point",
  "fields": [
    { "name": "x", "type": "F64" },
    { "name": "y", "type": "F64" }
  ]
}

// Enum (tagged union)
{
  "id": "t_Shape",
  "kind": "Enum",
  "name": "Shape",
  "variants": [
    { "name": "Circle", "fields": [{ "name": "radius", "type": "F64" }] },
    { "name": "Rect", "fields": [{ "name": "w", "type": "F64" }, { "name": "h", "type": "F64" }] },
    { "name": "Point", "fields": [] }
  ]
}

// Array
{ "id": "t_arr_i64", "kind": "Array", "element": "I64" }

// Tuple
{ "id": "t_pair", "kind": "Tuple", "elements": ["I64", "String"] }

// Optional
{ "id": "t_opt_i64", "kind": "Optional", "inner": "I64" }

// Result
{ "id": "t_res", "kind": "Result", "ok": "String", "err": "t_IOError" }

// Function type
{
  "id": "t_fn_add",
  "kind": "Function",
  "params": ["I64", "I64"],
  "returns": "I64",
  "effects": ["Pure"]
}

// Reference
{ "id": "t_ref_i64", "kind": "Reference", "inner": "I64", "mutable": false }

// Generic
{
  "id": "t_vec_t",
  "kind": "Generic",
  "base": "t_Array",
  "args": [{ "kind": "TypeParam", "name": "T", "bounds": [] }]
}
```

### 3.3 Traits (Interfaces)

```json
{
  "id": "tr_Display",
  "kind": "Trait",
  "name": "Display",
  "type_params": [],
  "methods": [
    {
      "name": "to_string",
      "params": [{ "name": "self", "type": "Self" }],
      "returns": "String",
      "effects": ["Pure"],
      "default_body": null
    }
  ]
}

// Implementation
{
  "id": "impl_001",
  "kind": "Impl",
  "trait": "tr_Display",
  "for_type": "t_Point",
  "methods": {
    "to_string": "f_point_to_string"
  }
}
```

---

## 4. Effect System

### 4.1 Effect Definitions

```json
// Pure: no side effects
{ "kind": "Pure" }

// IO: reads from or writes to external world
{ "kind": "IO" }

// Read: reads from a named resource
{ "kind": "Read", "resource": "filesystem" }

// Write: writes to a named resource
{ "kind": "Write", "resource": "filesystem" }

// Allocate: performs heap allocation
{ "kind": "Allocate" }

// Fail: can produce an error
{ "kind": "Fail", "error_type": "t_IOError" }

// Diverge: may not terminate
{ "kind": "Diverge" }
```

### 4.2 Effect Rules

1. A function's declared effect set must be a **superset** of all effects actually used in its body
2. Calling a function with effects `E` within a function with effects `F` requires `E ⊆ F`
3. A `Pure` function cannot call any non-`Pure` function
4. `IO` subsumes `Read(*)` and `Write(*)` (it's the top of the I/O lattice)
5. Effects compose via union: calling `f: [IO]` and `g: [Fail(E)]` requires `[IO, Fail(E)]`
6. Effect polymorphism: higher-order functions inherit effects from their callback arguments

### 4.3 Effect Lattice

```
        Diverge
           |
          IO
         / \
    Read    Write
        \ /
      Allocate
         |
     Fail(E)
         |
        Pure
```

---

## 5. Module Structure

A complete module in JSON:

```json
{
  "format_version": "0.1.0",
  "module": {
    "id": "mod_main",
    "name": "main",
    "metadata": {
      "version": "1.0.0",
      "description": "Entry point module",
      "author": "agent-001",
      "created_at": "2026-04-06T12:00:00Z"
    },
    "imports": [
      { "module": "std::io", "items": ["println"] },
      { "module": "std::string", "items": ["*"] }
    ],
    "exports": [
      { "kind": "Function", "name": "main" }
    ],
    "types": [],
    "traits": [],
    "impls": [],
    "constants": [],
    "functions": [
      {
        "id": "f_main",
        "name": "main",
        "params": [],
        "returns": "Unit",
        "effects": ["IO"],
        "body": {
          "id": "n_100",
          "kind": "Call",
          "type": "Unit",
          "target": "std::io::println",
          "args": [
            {
              "id": "n_101",
              "kind": "Literal",
              "type": "String",
              "value": "hello world"
            }
          ]
        }
      }
    ]
  }
}
```

---

## 6. Function Definition

```json
{
  "id": "f_fibonacci",
  "name": "fibonacci",
  "visibility": "public",
  "type_params": [],
  "params": [
    { "name": "n", "type": "I64", "index": 0 }
  ],
  "returns": "I64",
  "effects": ["Pure"],
  "constraints": [],
  "body": {
    "id": "n_200",
    "kind": "If",
    "type": "I64",
    "cond": {
      "id": "n_201",
      "kind": "BinOp",
      "op": "Lte",
      "type": "Bool",
      "lhs": { "id": "n_202", "kind": "Param", "name": "n", "index": 0, "type": "I64" },
      "rhs": { "id": "n_203", "kind": "Literal", "type": "I64", "value": 1 }
    },
    "then_branch": {
      "id": "n_204", "kind": "Param", "name": "n", "index": 0, "type": "I64"
    },
    "else_branch": {
      "id": "n_210",
      "kind": "BinOp",
      "op": "Add",
      "type": "I64",
      "lhs": {
        "id": "n_211",
        "kind": "Call",
        "type": "I64",
        "target": "f_fibonacci",
        "args": [{
          "id": "n_212",
          "kind": "BinOp",
          "op": "Sub",
          "type": "I64",
          "lhs": { "id": "n_213", "kind": "Param", "name": "n", "index": 0, "type": "I64" },
          "rhs": { "id": "n_214", "kind": "Literal", "type": "I64", "value": 1 }
        }]
      },
      "rhs": {
        "id": "n_215",
        "kind": "Call",
        "type": "I64",
        "target": "f_fibonacci",
        "args": [{
          "id": "n_216",
          "kind": "BinOp",
          "op": "Sub",
          "type": "I64",
          "lhs": { "id": "n_217", "kind": "Param", "name": "n", "index": 0, "type": "I64" },
          "rhs": { "id": "n_218", "kind": "Literal", "type": "I64", "value": 2 }
        }]
      }
    }
  }
}
```

---

## 7. Semantic Patch Format

### 7.1 Patch Structure

```json
{
  "patch": {
    "id": "p_abc123",
    "parent_version": "sha256:aabbccdd...",
    "author": "agent-review-001",
    "timestamp": "2026-04-06T12:30:00Z",
    "rationale": "Add bounds checking to array access in process_data",
    "operations": [
      {
        "kind": "ReplaceNode",
        "target": "n_083",
        "replacement": {
          "id": "n_083_v2",
          "kind": "If",
          "type": "I64",
          "cond": {
            "id": "n_083_check",
            "kind": "BinOp",
            "op": "Lt",
            "type": "Bool",
            "lhs": { "id": "n_083_idx", "kind": "Param", "name": "index", "index": 1, "type": "I64" },
            "rhs": {
              "id": "n_083_len",
              "kind": "Call",
              "type": "I64",
              "target": "std::array::len",
              "args": ["n_082"]
            }
          },
          "then_branch": {
            "id": "n_083_ok",
            "kind": "IndexAccess",
            "type": "I64",
            "array": "n_082",
            "index": "n_083_idx"
          },
          "else_branch": {
            "id": "n_083_err",
            "kind": "Call",
            "type": "I64",
            "target": "std::panic",
            "args": [{
              "id": "n_083_msg",
              "kind": "Literal",
              "type": "String",
              "value": "index out of bounds"
            }]
          }
        }
      }
    ],
    "evidence": [
      {
        "kind": "TestResult",
        "test_name": "test_bounds_check",
        "passed": true,
        "output": "OK"
      }
    ],
    "constraints_checked": ["max_function_complexity", "required_tests"]
  }
}
```

### 7.2 Common Patch Patterns

**Add a new function:**
```json
{
  "kind": "AddFunction",
  "module": "mod_main",
  "func": { /* full FuncDef */ }
}
```

**Rename a symbol across scope:**
```json
{
  "kind": "RenameSymbol",
  "old": "processData",
  "new": "process_data",
  "scope": "mod_main"
}
```

**Add a field to a struct:**
```json
{
  "kind": "AddField",
  "struct_type": "t_User",
  "field": { "name": "email", "type": "Optional<String>" }
}
```

**Change function effects:**
```json
{
  "kind": "AddEffect",
  "func": "f_process",
  "effect": { "kind": "Fail", "error_type": "t_ValidationError" }
}
```

---

## 8. Versioning

### 8.1 Content Addressing

Every IR module version is identified by a SHA-256 hash of its canonical serialized form:

```
VersionId = SHA-256(canonical_serialize(module))
```

Canonical serialization:
- Nodes sorted by ID
- Fields sorted alphabetically within each node
- No whitespace
- Deterministic floating-point representation

### 8.2 Version DAG

```
v1 ── v2 ── v3 ── v5 (main)
              \          /
               v4 ──────  (branch merged)
```

Each version records:
- `parent_versions: Vec<VersionId>` (one for linear, two for merge)
- `patch: PatchId` that produced this version from parent
- `metadata: VersionMetadata`

---

## 9. Project Structure (on disk)

```
myproject/
├── airl.toml                    # project config
├── modules/
│   ├── main.airl                # binary serialized IR (canonical)
│   ├── main.airl.json           # JSON debug view (optional)
│   ├── lib.airl
│   └── utils.airl
├── patches/
│   ├── p_abc123.patch.json      # applied patches (history)
│   └── p_def456.patch.json
├── evidence/
│   ├── tests/                   # test results
│   ├── benchmarks/              # benchmark results
│   └── coverage/                # coverage reports
├── constraints/
│   └── project.constraints.json # constraint definitions
└── .airl/
    ├── versions/                # version store (content-addressable)
    └── index                    # version DAG index
```

### 9.1 Project Config (airl.toml)

```toml
[project]
name = "myproject"
version = "0.1.0"
entry = "main"

[build]
default_target = "native"        # native | wasm | wasm-browser
opt_level = "fast"               # fast (Cranelift) | optimized (LLVM)

[constraints]
max_function_complexity = 20
max_module_nodes = 5000
required_test_coverage = 0.8

[dependencies]
std = "0.1.0"

[effects.allowed]
main = ["IO", "Fail"]            # entry point can do IO
lib = ["Pure", "Fail"]           # library must be Pure (except errors)
```

---

## 10. IR Invariants

These invariants must hold for any valid IR graph:

1. **Acyclicity:** The node graph (excluding recursive calls) is a DAG
2. **Type consistency:** Every node's type matches its operation's type rules
3. **Scope correctness:** Every variable reference resolves to a binding in scope
4. **Effect coverage:** Every function's declared effects cover all effects in its body
5. **Exhaustive matching:** Every `Match` node covers all variants of its scrutinee type
6. **Unique IDs:** No two nodes in a module share an ID
7. **Referential integrity:** Every edge target exists in the same module or is a valid import
8. **Version determinism:** The same IR content always produces the same VersionId
