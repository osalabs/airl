#!/usr/bin/env python3
"""
AIRL agent harness demo.

Demonstrates an end-to-end agent workflow using the AIRL HTTP API:
  1. Start an AIRL server (user runs: `airl api serve --port 9090`)
  2. Create a project from a "hello world" IR
  3. Apply a semantic patch to change the greeting
  4. Type-check and interpret the patched module
  5. Project the module to TypeScript
  6. Query the call graph and builtin usage
  7. Check architectural constraints

Run:
  # Terminal 1:
  cargo run -p airl-cli -- api serve --port 9090

  # Terminal 2:
  python3 examples/agent_harness.py
"""

import json
import sys
import urllib.request
import urllib.error

API = "http://127.0.0.1:9090"


def api_call(method: str, path: str, body=None):
    """POST/GET JSON to the AIRL API; return parsed JSON response."""
    url = f"{API}{path}"
    data = None
    headers = {}
    if body is not None:
        data = json.dumps(body).encode("utf-8")
        headers["Content-Type"] = "application/json"

    req = urllib.request.Request(url, data=data, method=method, headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=5) as resp:
            return resp.status, json.loads(resp.read().decode("utf-8") or "{}")
    except urllib.error.HTTPError as e:
        return e.code, json.loads(e.read().decode("utf-8") or "{}")
    except urllib.error.URLError as e:
        print(f"ERROR: cannot reach {API} — is the server running?")
        print(f"  Start with: cargo run -p airl-cli -- api serve --port 9090")
        print(f"  ({e.reason})")
        sys.exit(1)


def section(title: str):
    print()
    print("=" * 60)
    print(f"  {title}")
    print("=" * 60)


# Step 1: The IR we want to load
HELLO_MODULE = {
    "format_version": "0.1.0",
    "module": {
        "id": "mod_agent",
        "name": "main",
        "metadata": {
            "version": "1.0.0",
            "description": "Agent harness demo",
            "author": "agent-harness-py",
            "created_at": "2026-04-15T12:00:00Z",
        },
        "imports": [{"module": "std::io", "items": ["println"]}],
        "exports": [{"kind": "Function", "name": "main"}],
        "types": [],
        "functions": [
            {
                "id": "f_main",
                "name": "main",
                "params": [],
                "returns": "Unit",
                "effects": ["IO"],
                "body": {
                    "id": "n_1", "kind": "Call", "type": "Unit",
                    "target": "std::io::println",
                    "args": [{
                        "id": "n_2", "kind": "Literal", "type": "String",
                        "value": "hello from the agent"
                    }],
                },
            }
        ],
    },
}


def main():
    section("1. Create project from IR")
    status, resp = api_call("POST", "/project/create", {
        "name": "agent-demo",
        "module_json": json.dumps(HELLO_MODULE),
    })
    print(f"  status={status}  project={resp.get('name')}  version={resp.get('version','')[:12]}")

    section("2. Type check the initial module")
    status, resp = api_call("POST", "/typecheck", {})
    print(f"  success={resp.get('success')}  errors={len(resp.get('errors', []))}")

    section("3. Interpret (expect: hello from the agent)")
    status, resp = api_call("POST", "/interpret", {
        "max_steps": 1000000, "max_call_depth": 1000
    })
    print(f"  success={resp.get('success')}")
    print(f"  stdout: {resp.get('stdout', '').strip()}")

    section("4. Apply semantic patch (change the greeting)")
    # The server flattens the patch at the body root (no `{"patch": ...}` wrapper)
    patch = {
        "id": "p_change_greeting",
        "parent_version": "",
        "operations": [{
            "kind": "ReplaceNode",
            "target": "n_2",
            "replacement": {
                "id": "n_2", "kind": "Literal", "type": "String",
                "value": "GREETINGS FROM THE PATCHED AGENT!"
            },
        }],
        "rationale": "Excited greeting",
        "author": "agent-harness-py",
    }
    status, resp = api_call("POST", "/patch/apply", patch)
    print(f"  success={resp.get('success')}  new_version={resp.get('new_version','')[:12]}")
    print(f"  affected: {resp.get('impact', {}).get('affected_functions', [])}")

    section("5. Interpret again (expect: patched greeting)")
    status, resp = api_call("POST", "/interpret", {
        "max_steps": 1000000, "max_call_depth": 1000
    })
    print(f"  stdout: {resp.get('stdout', '').strip()}")

    section("6. Project module to TypeScript")
    status, resp = api_call("POST", "/project/text", {"language": "typescript"})
    text = resp.get("text", "")
    for line in text.splitlines()[:10]:
        print(f"  | {line}")
    if len(text.splitlines()) > 10:
        print(f"  | ... ({len(text.splitlines())} total lines)")

    section("7. Query builtin usage")
    status, resp = api_call("GET", "/query/builtin-usage", None)
    print(f"  unique builtins: {resp.get('unique_builtins', [])}")
    print(f"  counts: {resp.get('counts', {})}")

    section("8. Check architectural constraints")
    status, resp = api_call("POST", "/constraints/check", {
        "constraints": [
            {"kind": "MaxFunctionCount", "max": 5},
            {"kind": "MaxFunctionComplexity", "threshold": 10},
            {"kind": "ForbiddenTarget", "target": "std::process::exit"},
        ]
    })
    print(f"  ok={resp.get('ok')}  violations={len(resp.get('violations', []))}")
    for v in resp.get("violations", []):
        print(f"    - {v['constraint']}: {v['message']}")

    section("9. Undo the patch (restore original)")
    status, resp = api_call("POST", "/patch/undo", None)
    print(f"  success={resp.get('success')}  new_version={resp.get('new_version','')[:12]}")

    section("10. Interpret after undo (expect: original greeting)")
    status, resp = api_call("POST", "/interpret", {
        "max_steps": 1000000, "max_call_depth": 1000
    })
    print(f"  stdout: {resp.get('stdout', '').strip()}")

    print()
    print("=" * 60)
    print("  Agent harness demo complete.")
    print("=" * 60)


if __name__ == "__main__":
    main()
