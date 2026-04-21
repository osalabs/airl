//! Example agent workflow using the Rust SDK against a running AIRL API server.
//!
//! Mirrors the flow in `examples/agent_harness.py` and `examples/agent_harness.js`
//! but uses the typed Rust client.
//!
//! Run:
//! ```text
//! # Terminal 1:
//! cargo run -p airl-cli -- api serve --port 9090
//!
//! # Terminal 2:
//! cargo run -p airl-sdk --example agent_workflow
//! ```

use airl_ir::node::{LiteralValue, Node};
use airl_ir::types::Type;
use airl_ir::NodeId;
use airl_patch::{Patch, PatchOp};
use airl_project::constraints::Constraint;
use airl_sdk::{Client, ProjectionLang};

const HELLO_MODULE: &str = r#"{
    "format_version":"0.1.0",
    "module":{"id":"mod_agent","name":"main",
        "metadata":{"version":"1","description":"Rust SDK demo","author":"agent-sdk-rust","created_at":""},
        "imports":[{"module":"std::io","items":["println"]}],
        "exports":[{"kind":"Function","name":"main"}],
        "types":[],
        "functions":[{
            "id":"f_main","name":"main","params":[],"returns":"Unit","effects":["IO"],
            "body":{"id":"n_1","kind":"Call","type":"Unit","target":"std::io::println",
                "args":[{"id":"n_2","kind":"Literal","type":"String","value":"hello from the Rust agent"}]}
        }]}
}"#;

fn section(title: &str) {
    println!();
    println!("{}", "=".repeat(60));
    println!("  {title}");
    println!("{}", "=".repeat(60));
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new("http://127.0.0.1:9090");

    section("1. Create project");
    let info = client.create_project("agent-demo-rust", HELLO_MODULE)?;
    println!(
        "  name={}  version={}  functions={}",
        info.name,
        &info.version[..12.min(info.version.len())],
        info.function_count
    );

    section("2. Type check");
    let tc = client.typecheck()?;
    println!(
        "  success={}  errors={}  warnings={}",
        tc.success,
        tc.errors.len(),
        tc.warnings.len()
    );

    section("3. Interpret");
    let output = client.interpret_default()?;
    println!("  stdout: {}", output.stdout.trim());

    section("4. Apply semantic patch");
    let patch = Patch {
        id: "p_change_greeting".to_string(),
        parent_version: String::new(),
        operations: vec![PatchOp::ReplaceNode {
            target: NodeId::new("n_2"),
            replacement: Node::Literal {
                id: NodeId::new("n_2"),
                node_type: Type::String,
                value: LiteralValue::Str("GREETINGS FROM THE RUST AGENT!".to_string()),
            },
        }],
        rationale: "Excited greeting".to_string(),
        author: "agent-sdk-rust".to_string(),
    };
    let apply = client.apply_patch(&patch)?;
    println!(
        "  success={}  new_version={}",
        apply.success,
        &apply.new_version[..12]
    );

    section("5. Interpret patched module");
    let output = client.interpret_default()?;
    println!("  stdout: {}", output.stdout.trim());

    section("6. Project to TypeScript");
    let ts = client.project_to_text(ProjectionLang::TypeScript)?;
    for line in ts.text.lines().take(8) {
        println!("  | {line}");
    }

    section("7. Dead code analysis");
    let dc = client.find_dead_code("main")?;
    println!("  reachable: {:?}", dc.reachable);
    println!(
        "  dead: {}",
        if dc.dead.is_empty() {
            "(none)".to_string()
        } else {
            dc.dead.join(", ")
        }
    );

    section("8. Effect surface");
    let surface = client.effect_surface()?;
    println!("  effects: {:?}", surface.effects);
    println!("  IO functions: {:?}", surface.io_functions);

    section("9. Check constraints");
    let report = client.check_constraints(&[
        Constraint::MaxFunctionCount { max: 5 },
        Constraint::MaxFunctionComplexity { threshold: 10 },
        Constraint::ForbiddenTarget {
            target: "std::process::exit".to_string(),
        },
    ])?;
    println!("  ok={}  violations={}", report.ok, report.violations.len());

    section("10. Undo patch");
    let undo = client.undo_patch()?;
    println!(
        "  success={}  new_version={}",
        undo.success,
        &undo.new_version[..12]
    );

    section("11. Interpret after undo");
    let output = client.interpret_default()?;
    println!("  stdout: {}", output.stdout.trim());

    println!();
    println!("{}", "=".repeat(60));
    println!("  Rust SDK agent workflow complete.");
    println!("{}", "=".repeat(60));
    Ok(())
}
