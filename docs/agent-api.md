# AIRL Agent API Specification

## Overview

This document specifies the API that AI coding agents use to interact with AIRL. The API is designed so that agents never touch text files — they work exclusively with structured IR through typed operations.

The API is available as:
1. **gRPC** (for high-performance, streaming use cases)
2. **JSON-RPC over HTTP** (for simpler integrations)
3. **CLI** (for shell-based agents)

All three interfaces expose the same operations.

---

## 1. Project Management

### CreateProject

Create a new AIRL project.

```json
// Request
{
  "method": "CreateProject",
  "params": {
    "name": "my-api-server",
    "config": {
      "default_target": "native",
      "opt_level": "fast",
      "effects_allowed": {
        "main": ["IO", "Fail"],
        "lib": ["Pure", "Fail"]
      }
    }
  }
}

// Response
{
  "project_id": "proj_abc123",
  "path": "/home/agent/projects/my-api-server",
  "initial_version": "sha256:000...000"
}
```

### OpenProject

Open an existing project from disk.

```json
// Request
{ "method": "OpenProject", "params": { "path": "/path/to/project" } }

// Response
{
  "project_id": "proj_abc123",
  "name": "my-api-server",
  "modules": ["main", "lib", "utils"],
  "current_version": "sha256:aabb..."
}
```

---

## 2. Module Operations

### GetModule

Retrieve a module's full IR.

```json
// Request
{
  "method": "GetModule",
  "params": {
    "project": "proj_abc123",
    "module_id": "mod_main"
  }
}

// Response
{
  "module": { /* full Module JSON as defined in ir-spec.md */ },
  "version": "sha256:aabb..."
}
```

### CreateModule

Create a new module with initial IR content.

```json
// Request
{
  "method": "CreateModule",
  "params": {
    "project": "proj_abc123",
    "name": "handlers",
    "initial_ir": { /* Module JSON */ }
  }
}

// Response
{
  "module_id": "mod_handlers",
  "version": "sha256:ccdd...",
  "diagnostics": []
}
```

### ListModules

```json
// Request
{ "method": "ListModules", "params": { "project": "proj_abc123" } }

// Response
{
  "modules": [
    { "id": "mod_main", "name": "main", "function_count": 3, "type_count": 2 },
    { "id": "mod_lib", "name": "lib", "function_count": 12, "type_count": 5 }
  ]
}
```

---

## 3. Patch Operations (Primary Edit Interface)

### ApplyPatch

Apply a semantic patch to the project. This is the **primary way agents make changes**.

```json
// Request
{
  "method": "ApplyPatch",
  "params": {
    "project": "proj_abc123",
    "patch": {
      "rationale": "Add input validation to create_user handler",
      "operations": [
        {
          "kind": "ReplaceNode",
          "target": "n_500",
          "replacement": { /* new node tree */ }
        },
        {
          "kind": "AddEffect",
          "func": "f_create_user",
          "effect": { "kind": "Fail", "error_type": "t_ValidationError" }
        }
      ]
    }
  }
}

// Response
{
  "success": true,
  "patch_id": "p_xyz789",
  "new_version": "sha256:eeff...",
  "diagnostics": [],
  "impact": {
    "affected_functions": ["f_create_user", "f_handle_request"],
    "affected_types": [],
    "affected_modules": ["mod_handlers"]
  }
}
```

### PreviewPatch

Dry-run a patch without committing. Returns what would change and any errors.

```json
// Request
{
  "method": "PreviewPatch",
  "params": {
    "project": "proj_abc123",
    "patch": { /* same as ApplyPatch */ }
  }
}

// Response
{
  "would_succeed": true,
  "type_errors": [],
  "constraint_violations": [],
  "impact": {
    "affected_functions": ["f_create_user"],
    "affected_types": [],
    "breaking_changes": false
  }
}
```

### UndoPatch

Revert a previously applied patch.

```json
// Request
{
  "method": "UndoPatch",
  "params": {
    "project": "proj_abc123",
    "patch_id": "p_xyz789"
  }
}

// Response
{
  "success": true,
  "new_version": "sha256:aabb...",
  "diagnostics": []
}
```

### GetDiff

Get structured diff between two versions.

```json
// Request
{
  "method": "GetDiff",
  "params": {
    "project": "proj_abc123",
    "from_version": "sha256:aabb...",
    "to_version": "sha256:eeff..."
  }
}

// Response
{
  "diff": {
    "added_functions": ["f_validate_input"],
    "removed_functions": [],
    "modified_functions": [
      {
        "func_id": "f_create_user",
        "changes": [
          { "kind": "NodeReplaced", "node": "n_500", "description": "Added validation before insert" },
          { "kind": "EffectAdded", "effect": "Fail(ValidationError)" }
        ]
      }
    ],
    "added_types": ["t_ValidationError"],
    "removed_types": [],
    "modified_types": []
  }
}
```

---

## 4. Build & Run

### TypeCheck

Run the type checker on the entire project.

```json
// Request
{ "method": "TypeCheck", "params": { "project": "proj_abc123" } }

// Response
{
  "success": true,
  "errors": [],
  "warnings": [
    {
      "kind": "UnusedVariable",
      "location": { "module": "mod_main", "func": "f_main", "node": "n_105" },
      "message": "Variable 'temp' is declared but never used"
    }
  ]
}
```

### Interpret

Execute a function using the interpreter (fast feedback).

```json
// Request
{
  "method": "Interpret",
  "params": {
    "project": "proj_abc123",
    "entry_func": "f_main",
    "args": [],
    "limits": {
      "max_steps": 1000000,
      "max_memory_bytes": 67108864,
      "timeout_ms": 5000
    }
  }
}

// Response
{
  "success": true,
  "exit_code": 0,
  "stdout": "Server started on :8080\n",
  "stderr": "",
  "metrics": {
    "steps_executed": 1523,
    "memory_peak_bytes": 4096,
    "wall_time_ms": 12
  }
}
```

### Compile

Compile the project to a binary.

```json
// Request
{
  "method": "Compile",
  "params": {
    "project": "proj_abc123",
    "target": "native",
    "opt_level": "fast",
    "output_path": "/tmp/my-api-server"
  }
}

// Response
{
  "success": true,
  "binary_path": "/tmp/my-api-server",
  "binary_size_bytes": 2048576,
  "compile_time_ms": 450,
  "warnings": []
}
```

---

## 5. Query Operations

Agents need to understand the codebase before making changes.

### FindFunction

```json
// Request
{
  "method": "FindFunction",
  "params": {
    "project": "proj_abc123",
    "pattern": "handle_*"
  }
}

// Response
{
  "functions": [
    {
      "id": "f_handle_request",
      "name": "handle_request",
      "module": "mod_handlers",
      "params": [{ "name": "req", "type": "HttpRequest" }],
      "returns": "HttpResponse",
      "effects": ["IO", "Fail(HttpError)"],
      "node_count": 45
    },
    {
      "id": "f_handle_error",
      "name": "handle_error",
      "module": "mod_handlers",
      "params": [{ "name": "err", "type": "HttpError" }],
      "returns": "HttpResponse",
      "effects": ["Pure"],
      "node_count": 12
    }
  ]
}
```

### FindType

```json
// Request
{ "method": "FindType", "params": { "project": "proj_abc123", "pattern": "Http*" } }

// Response
{
  "types": [
    { "id": "t_HttpRequest", "name": "HttpRequest", "kind": "Struct", "field_count": 5 },
    { "id": "t_HttpResponse", "name": "HttpResponse", "kind": "Struct", "field_count": 3 },
    { "id": "t_HttpError", "name": "HttpError", "kind": "Enum", "variant_count": 4 }
  ]
}
```

### GetCallGraph

```json
// Request
{
  "method": "GetCallGraph",
  "params": {
    "project": "proj_abc123",
    "func_id": "f_handle_request",
    "depth": 3
  }
}

// Response
{
  "root": "f_handle_request",
  "edges": [
    { "from": "f_handle_request", "to": "f_parse_body", "call_count": 1 },
    { "from": "f_handle_request", "to": "f_validate_input", "call_count": 1 },
    { "from": "f_handle_request", "to": "f_create_user", "call_count": 1 },
    { "from": "f_create_user", "to": "std::json::serialize", "call_count": 1 },
    { "from": "f_validate_input", "to": "std::string::len", "call_count": 2 }
  ]
}
```

### GetEffectSummary

```json
// Request
{
  "method": "GetEffectSummary",
  "params": {
    "project": "proj_abc123",
    "func_id": "f_handle_request"
  }
}

// Response
{
  "func_id": "f_handle_request",
  "declared_effects": ["IO", "Fail(HttpError)"],
  "actual_effects": ["IO", "Fail(HttpError)", "Allocate"],
  "effect_sources": [
    { "effect": "IO", "source": "f_create_user -> std::io::write_file" },
    { "effect": "Fail(HttpError)", "source": "f_validate_input" },
    { "effect": "Allocate", "source": "std::json::serialize" }
  ]
}
```

### GetDependencyGraph

```json
// Request
{ "method": "GetDependencyGraph", "params": { "project": "proj_abc123" } }

// Response
{
  "modules": [
    { "id": "mod_main", "depends_on": ["mod_handlers", "mod_config", "std::io"] },
    { "id": "mod_handlers", "depends_on": ["mod_models", "std::json", "std::io"] },
    { "id": "mod_models", "depends_on": ["std::string"] },
    { "id": "mod_config", "depends_on": ["std::io", "std::json"] }
  ]
}
```

---

## 6. Projection Operations

### ProjectToText

Generate human-readable code from IR.

```json
// Request
{
  "method": "ProjectToText",
  "params": {
    "project": "proj_abc123",
    "module_id": "mod_handlers",
    "language": "typescript"
  }
}

// Response
{
  "language": "typescript",
  "code": "export function handleRequest(req: HttpRequest): HttpResponse {\n  const body = parseBody(req);\n  const validated = validateInput(body);\n  return createUser(validated);\n}\n...",
  "line_map": {
    "f_handle_request": { "start": 1, "end": 5 },
    "f_validate_input": { "start": 7, "end": 15 }
  }
}
```

### ProjectToContract

Show only the public interfaces (types, function signatures, effects).

```json
// Request
{
  "method": "ProjectToContract",
  "params": {
    "project": "proj_abc123",
    "module_id": "mod_handlers"
  }
}

// Response
{
  "module": "handlers",
  "public_types": [
    { "name": "HttpRequest", "fields": [...] },
    { "name": "HttpResponse", "fields": [...] }
  ],
  "public_functions": [
    {
      "name": "handle_request",
      "signature": "(req: HttpRequest) -> HttpResponse [IO, Fail(HttpError)]"
    }
  ],
  "invariants": [
    "handle_request always returns a valid HTTP status code",
    "response body is valid JSON when content_type is application/json"
  ]
}
```

### ProjectToDiagram

Generate a Mermaid diagram of the project structure.

```json
// Request
{
  "method": "ProjectToDiagram",
  "params": {
    "project": "proj_abc123",
    "kind": "module_deps"
  }
}

// Response
{
  "format": "mermaid",
  "diagram": "graph TD\n  main --> handlers\n  main --> config\n  handlers --> models\n  handlers --> std_json\n  handlers --> std_io\n  models --> std_string\n  config --> std_io\n  config --> std_json"
}
```

---

## 7. Evidence & Testing

### RunTests

```json
// Request
{
  "method": "RunTests",
  "params": {
    "project": "proj_abc123",
    "filter": "test_validate_*",
    "limits": { "timeout_ms": 30000 }
  }
}

// Response
{
  "total": 8,
  "passed": 7,
  "failed": 1,
  "skipped": 0,
  "results": [
    { "name": "test_validate_empty_name", "status": "passed", "duration_ms": 2 },
    { "name": "test_validate_long_email", "status": "failed",
      "duration_ms": 3,
      "error": "assertion failed: expected Err(TooLong), got Ok(\"a@b.c...\")" },
    ...
  ],
  "coverage": {
    "line_coverage": 0.85,
    "function_coverage": 0.92,
    "uncovered_functions": ["f_handle_timeout"]
  }
}
```

### RunBenchmarks

```json
// Request
{
  "method": "RunBenchmarks",
  "params": {
    "project": "proj_abc123",
    "filter": "bench_*"
  }
}

// Response
{
  "benchmarks": [
    {
      "name": "bench_handle_request",
      "iterations": 10000,
      "mean_ns": 4523,
      "std_dev_ns": 312,
      "min_ns": 3890,
      "max_ns": 5201,
      "allocations": 3
    }
  ],
  "comparison_to_previous": {
    "bench_handle_request": { "delta_percent": -2.3, "regression": false }
  }
}
```

---

## 8. Constraint Management

### SetConstraint

```json
// Request
{
  "method": "SetConstraint",
  "params": {
    "project": "proj_abc123",
    "constraint": {
      "kind": "PerformanceBudget",
      "func": "f_handle_request",
      "max_ns": 10000,
      "enforcement": "block_patch"
    }
  }
}

// Response
{
  "constraint_id": "c_001",
  "status": "active"
}
```

### CheckConstraints

```json
// Request
{ "method": "CheckConstraints", "params": { "project": "proj_abc123" } }

// Response
{
  "all_pass": false,
  "results": [
    { "constraint_id": "c_001", "kind": "PerformanceBudget", "status": "pass" },
    { "constraint_id": "c_002", "kind": "RequiredTests", "status": "fail",
      "message": "Function f_handle_timeout has 0% test coverage (minimum: 80%)" },
    { "constraint_id": "c_003", "kind": "MaxFunctionComplexity", "status": "pass" }
  ]
}
```

---

## 9. Typical Agent Workflow

### Create a new feature end-to-end:

```
1. Agent receives task: "Add user registration endpoint"

2. Agent queries existing codebase:
   - FindFunction("handle_*")           → understand existing handlers
   - FindType("User*")                  → understand data model
   - GetCallGraph("f_handle_request")   → understand request flow
   - GetDependencyGraph()               → understand module structure

3. Agent plans changes:
   - New type: UserRegistration (input DTO)
   - New function: validate_registration
   - New function: handle_registration
   - Modify: router to add new route

4. Agent applies patches (one per logical change):
   Patch 1: AddType(UserRegistration) → PreviewPatch → ApplyPatch
   Patch 2: AddFunction(validate_registration) → PreviewPatch → ApplyPatch
   Patch 3: AddFunction(handle_registration) → PreviewPatch → ApplyPatch
   Patch 4: ReplaceNode(router config, add route) → PreviewPatch → ApplyPatch

5. Agent verifies:
   - TypeCheck()                        → no type errors
   - RunTests()                         → all pass
   - CheckConstraints()                 → all pass
   - Interpret(f_main, test_args)       → correct behavior

6. Agent generates evidence bundle:
   - Test results
   - Constraint check results
   - Impact analysis (which functions changed)
   - Projection to TypeScript for human review
```

### Fix a bug:

```
1. Agent receives: "handle_request crashes when body is empty"

2. Agent investigates:
   - FindFunction("handle_request")
   - GetModule("mod_handlers")          → read full IR
   - GetEffectSummary("f_handle_request") → check error handling

3. Agent identifies the issue:
   - Node n_500 (Call to parse_body) has no error handling
   - parse_body has effect Fail(ParseError) but caller doesn't handle it

4. Agent applies fix:
   Patch: ReplaceNode(n_500, TryCall with error handling)
        + AddEffect(f_handle_request, Fail(ParseError))

5. Agent adds test:
   Patch: AddFunction(test_empty_body) in test module

6. Agent verifies:
   - TypeCheck() → pass
   - RunTests("test_empty_body") → pass
   - RunTests() → all pass (no regressions)
```

---

## 10. Error Codes

| Code | Meaning |
|---|---|
| `PATCH_CONFLICT` | Patch references nodes that have been modified since parent_version |
| `TYPE_ERROR` | Patch would introduce a type error |
| `EFFECT_VIOLATION` | Patch introduces undeclared effects |
| `CONSTRAINT_VIOLATION` | Patch violates a project constraint |
| `NODE_NOT_FOUND` | Referenced node ID doesn't exist |
| `MODULE_NOT_FOUND` | Referenced module doesn't exist |
| `VERSION_MISMATCH` | Parent version doesn't match current version |
| `EXECUTION_LIMIT` | Interpretation exceeded time/memory/step budget |
| `COMPILATION_ERROR` | Backend compilation failed |
| `SERIALIZATION_ERROR` | IR cannot be serialized/deserialized |

---

## 11. Streaming Operations

For long-running operations, the API supports streaming:

### InterpretStream

Stream stdout/stderr as the program runs (useful for long-running programs).

### CompileStream

Stream compilation progress (useful for large projects).

### WatchProject

Subscribe to project changes (useful for multi-agent coordination):
- New patch applied
- Type check completed
- Constraint violation detected
- New version created

---

## 12. Authentication & Authorization

For multi-agent setups:

```json
{
  "agent_id": "agent-author-001",
  "roles": ["author"],              // can apply patches
  "effect_budget": ["Pure", "IO"],   // allowed effects in authored code
  "module_scope": ["mod_handlers"],  // can only modify these modules
  "constraints": {
    "max_patches_per_minute": 10,
    "require_evidence": true
  }
}
```

Separation of duties: author agent and review agent must have different `agent_id`.
