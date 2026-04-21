//! End-to-end integration test for the SDK client against a live API server.
//!
//! Spawns `airl-api` in a background thread on a random port, then exercises
//! every SDK method to make sure the wire format matches between client and server.

use airl_sdk::{Client, ProjectionLang};
use std::net::TcpListener;
use std::time::Duration;

/// Bind to port 0 and immediately drop to let the OS pick a free port.
/// Returns the port that was assigned.
fn pick_free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

/// Spawn an AIRL API server on a free port in a background thread.
/// Returns the port. Polls until the server is ready to accept connections.
fn spawn_server() -> u16 {
    let port = pick_free_port();
    std::thread::spawn(move || {
        // airl_api::serve is async; we need a runtime.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            airl_api::serve(port).await;
        });
    });

    // Poll until the server is listening
    let client =
        Client::new(format!("http://127.0.0.1:{port}")).with_timeout(Duration::from_millis(200));
    for _ in 0..50 {
        // /project returns NO_PROJECT (400) when no project is loaded —
        // that's enough to know the server is up.
        match client.get_project() {
            Err(airl_sdk::SdkError::Api { .. }) => return port,
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
            Ok(_) => return port,
        }
    }
    panic!("server on port {port} did not start within 5s");
}

const HELLO_MODULE: &str = r#"{
    "format_version":"0.1.0",
    "module":{"id":"m","name":"main",
        "metadata":{"version":"1","description":"","author":"","created_at":""},
        "imports":[],"exports":[],"types":[],
        "functions":[{
            "id":"f","name":"main","params":[],"returns":"Unit","effects":["IO"],
            "body":{"id":"n1","kind":"Call","type":"Unit","target":"std::io::println",
                "args":[{"id":"n2","kind":"Literal","type":"String","value":"hello from sdk"}]}
        }]}
}"#;

#[test]
fn test_full_workflow() {
    let port = spawn_server();
    let client = Client::new(format!("http://127.0.0.1:{port}"));

    // 1. Create project
    let info = client.create_project("sdk-test", HELLO_MODULE).unwrap();
    assert_eq!(info.name, "sdk-test");
    assert_eq!(info.function_count, 1);
    assert_eq!(info.history_length, 0);

    // 2. Get project
    let info = client.get_project().unwrap();
    assert_eq!(info.name, "sdk-test");

    // 3. Get module
    let module_resp = client.get_module().unwrap();
    assert_eq!(module_resp.module.name(), "main");
    assert!(!module_resp.version.is_empty());

    // 4. Type check
    let tc = client.typecheck().unwrap();
    assert!(tc.success);
    assert!(tc.errors.is_empty());

    // 5. Interpret
    let output = client.interpret_default().unwrap();
    assert!(output.success);
    assert_eq!(output.stdout, "hello from sdk\n");

    // 6. Compile (Cranelift JIT)
    let compile_out = client.compile().unwrap();
    assert!(compile_out.success);
    assert_eq!(compile_out.stdout, "hello from sdk\n");

    // 7. Compile to WASM
    let wasm = client.compile_wasm().unwrap();
    assert!(wasm.starts_with(b"\0asm"), "should have WASM magic bytes");
    assert!(wasm.len() > 20);

    // 8. Project to TypeScript
    let ts = client.project_to_text(ProjectionLang::TypeScript).unwrap();
    assert!(ts.text.contains("console.log"));

    // 9. Project to Python
    let py = client.project_to_text(ProjectionLang::Python).unwrap();
    assert!(py.text.contains("print("));

    // 10. Find functions
    let funcs = client.find_functions("main").unwrap();
    assert_eq!(funcs.len(), 1);
    assert_eq!(funcs[0].name, "main");

    // 11. Call graph
    let edges = client.get_call_graph("main").unwrap();
    assert!(edges.iter().any(|e| e.to == "std::io::println"));

    // 12. Get effects
    let effects = client.get_effects("main").unwrap();
    assert!(effects.declared_effects.contains(&"IO".to_string()));

    // 13. Dead code
    let dc = client.find_dead_code("main").unwrap();
    assert!(dc.reachable.contains(&"main".to_string()));
    assert!(dc.dead.is_empty());

    // 14. Builtin usage
    let usage = client.builtin_usage().unwrap();
    assert_eq!(usage.counts.get("std::io::println"), Some(&1));

    // 15. Effect surface
    let surface = client.effect_surface().unwrap();
    assert!(surface.io_functions.contains(&"main".to_string()));

    // 16. Constraints
    use airl_project::constraints::Constraint;
    let report = client
        .check_constraints(&[
            Constraint::MaxFunctionCount { max: 10 },
            Constraint::ForbiddenTarget {
                target: "std::process::exit".to_string(),
            },
        ])
        .unwrap();
    assert!(report.ok);
    assert!(report.violations.is_empty());
}

#[test]
fn test_patch_workflow() {
    use airl_ir::node::{LiteralValue, Node};
    use airl_ir::types::Type;
    use airl_ir::NodeId;
    use airl_patch::{Patch, PatchOp};

    let port = spawn_server();
    let client = Client::new(format!("http://127.0.0.1:{port}"));

    client.create_project("patch-test", HELLO_MODULE).unwrap();

    // Preview a patch
    let patch = Patch {
        id: "p1".to_string(),
        parent_version: String::new(),
        operations: vec![PatchOp::ReplaceNode {
            target: NodeId::new("n2"),
            replacement: Node::Literal {
                id: NodeId::new("n2"),
                node_type: Type::String,
                value: LiteralValue::Str("patched!".to_string()),
            },
        }],
        rationale: "test".to_string(),
        author: "sdk-test".to_string(),
    };

    let preview = client.preview_patch(&patch).unwrap();
    assert!(preview.would_succeed);

    // Apply it
    let apply = client.apply_patch(&patch).unwrap();
    assert!(apply.success);
    assert!(!apply.impact.affected_functions.is_empty());

    // Verify via interpret
    let output = client.interpret_default().unwrap();
    assert_eq!(output.stdout, "patched!\n");

    // Undo
    let undo = client.undo_patch().unwrap();
    assert!(undo.success);

    // Verify we're back to original
    let output = client.interpret_default().unwrap();
    assert_eq!(output.stdout, "hello from sdk\n");
}

#[test]
fn test_api_error_surfaced() {
    // Call an endpoint that requires a project when none is loaded.
    let port = spawn_server();
    let client = Client::new(format!("http://127.0.0.1:{port}"));

    // Don't create_project first. Calling /project returns 400 NO_PROJECT.
    let err = client.get_project().unwrap_err();
    match err {
        airl_sdk::SdkError::Api { status, code, .. } => {
            assert_eq!(status, 400);
            assert_eq!(code, "NO_PROJECT");
        }
        other => panic!("expected Api error, got {other:?}"),
    }
}
