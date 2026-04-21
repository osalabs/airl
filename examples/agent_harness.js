#!/usr/bin/env node
/**
 * AIRL agent harness demo (Node.js).
 *
 * Same workflow as examples/agent_harness.py but in JavaScript.
 *
 * Run:
 *   # Terminal 1:
 *   cargo run -p airl-cli -- api serve --port 9090
 *
 *   # Terminal 2:
 *   node examples/agent_harness.js
 *
 * Requires Node 18+ (for built-in fetch).
 */

const API = "http://127.0.0.1:9090";

async function apiCall(method, path, body) {
  const opts = { method };
  if (body !== undefined) {
    opts.headers = { "Content-Type": "application/json" };
    opts.body = JSON.stringify(body);
  }
  try {
    const resp = await fetch(`${API}${path}`, opts);
    const text = await resp.text();
    return [resp.status, text ? JSON.parse(text) : {}];
  } catch (e) {
    console.error(`ERROR: cannot reach ${API} — is the server running?`);
    console.error(`  Start with: cargo run -p airl-cli -- api serve --port 9090`);
    console.error(`  (${e.message})`);
    process.exit(1);
  }
}

function section(title) {
  console.log();
  console.log("=".repeat(60));
  console.log(`  ${title}`);
  console.log("=".repeat(60));
}

const HELLO_MODULE = {
  format_version: "0.1.0",
  module: {
    id: "mod_agent",
    name: "main",
    metadata: {
      version: "1.0.0",
      description: "Agent harness demo",
      author: "agent-harness-js",
      created_at: "2026-04-15T12:00:00Z",
    },
    imports: [{ module: "std::io", items: ["println"] }],
    exports: [{ kind: "Function", name: "main" }],
    types: [],
    functions: [
      {
        id: "f_main",
        name: "main",
        params: [],
        returns: "Unit",
        effects: ["IO"],
        body: {
          id: "n_1", kind: "Call", type: "Unit",
          target: "std::io::println",
          args: [{
            id: "n_2", kind: "Literal", type: "String",
            value: "hello from the JS agent",
          }],
        },
      },
    ],
  },
};

async function main() {
  section("1. Create project from IR");
  let [status, resp] = await apiCall("POST", "/project/create", {
    name: "agent-demo-js",
    module_json: JSON.stringify(HELLO_MODULE),
  });
  console.log(`  status=${status}  project=${resp.name}  version=${(resp.version || "").slice(0, 12)}`);

  section("2. Type check");
  [status, resp] = await apiCall("POST", "/typecheck", {});
  console.log(`  success=${resp.success}  errors=${(resp.errors || []).length}`);

  section("3. Interpret");
  [status, resp] = await apiCall("POST", "/interpret", {
    max_steps: 1000000, max_call_depth: 1000,
  });
  console.log(`  stdout: ${(resp.stdout || "").trim()}`);

  section("4. Apply semantic patch");
  // The server flattens the patch at the body root (no { patch: ... } wrapper)
  [status, resp] = await apiCall("POST", "/patch/apply", {
    id: "p_change_greeting",
    parent_version: "",
    operations: [{
      kind: "ReplaceNode",
      target: "n_2",
      replacement: {
        id: "n_2", kind: "Literal", type: "String",
        value: "GREETINGS FROM THE JS AGENT!",
      },
    }],
    rationale: "Excited greeting",
    author: "agent-harness-js",
  });
  console.log(`  success=${resp.success}  new_version=${(resp.new_version || "").slice(0, 12)}`);

  section("5. Interpret patched module");
  [status, resp] = await apiCall("POST", "/interpret", {
    max_steps: 1000000, max_call_depth: 1000,
  });
  console.log(`  stdout: ${(resp.stdout || "").trim()}`);

  section("6. Project to Python");
  [status, resp] = await apiCall("POST", "/project/text", { language: "python" });
  const text = resp.text || "";
  const lines = text.split("\n").slice(0, 10);
  lines.forEach((l) => console.log(`  | ${l}`));
  if (text.split("\n").length > 10) {
    console.log(`  | ... (${text.split("\n").length} total lines)`);
  }

  section("7. Dead code analysis");
  [status, resp] = await apiCall("GET", "/query/dead-code?entry=main", null);
  console.log(`  reachable: ${resp.reachable}`);
  console.log(`  dead: ${resp.dead.length === 0 ? "(none)" : resp.dead.join(", ")}`);

  section("8. Effect surface");
  [status, resp] = await apiCall("GET", "/query/effect-surface", null);
  console.log(`  effects: ${resp.effects.join(", ")}`);
  console.log(`  IO functions: ${resp.io_functions.join(", ")}`);

  console.log();
  console.log("=".repeat(60));
  console.log("  Agent harness demo (JS) complete.");
  console.log("=".repeat(60));
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
