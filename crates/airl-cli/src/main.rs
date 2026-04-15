//! AIRL CLI - Command-line interface for the AIRL toolchain.

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process;

#[derive(Parser)]
#[command(name = "airl", version, about = "AIRL - AI-native Intermediate Representation Language")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run an AIRL program (interpret from JSON IR)
    Run {
        /// Path to the .airl.json file
        file: PathBuf,
        /// Use the Cranelift JIT compiler instead of the interpreter
        #[arg(long)]
        compiled: bool,
    },
    /// Type-check an AIRL program without running it
    Check {
        /// Path to the .airl.json file
        file: PathBuf,
    },
    /// Compile and run an AIRL program via Cranelift JIT
    Compile {
        /// Path to the .airl.json file
        file: PathBuf,
        /// Compilation target: "native" (default) or "wasm"
        #[arg(long, default_value = "native")]
        target: String,
        /// Output file path (for WASM target)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Apply a JSON patch to an AIRL module
    Patch {
        /// Path to the .airl.json module file
        module_file: PathBuf,
        /// Path to the .patch.json patch file
        patch_file: PathBuf,
        /// Output path for the patched module (default: overwrite input)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Start the HTTP API server
    Api {
        #[command(subcommand)]
        action: ApiAction,
    },
}

#[derive(Subcommand)]
enum ApiAction {
    /// Start the API server
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "9090")]
        port: u16,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run { file, compiled } => {
            if compiled {
                cmd_compile(&file);
            } else {
                cmd_run(&file);
            }
        }
        Commands::Check { file } => cmd_check(&file),
        Commands::Compile { file, target, output } => {
            if target == "wasm" {
                cmd_compile_wasm(&file, output.as_deref());
            } else {
                cmd_compile(&file);
            }
        }
        Commands::Patch {
            module_file,
            patch_file,
            output,
        } => cmd_patch(&module_file, &patch_file, output.as_deref()),
        Commands::Api { action } => match action {
            ApiAction::Serve { port } => {
                airl_api::serve(port).await;
            }
        },
    }
}

fn load_ir(file: &PathBuf) -> airl_ir::IRGraph {
    let json = match std::fs::read_to_string(file) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("error: cannot read {}: {e}", file.display());
            process::exit(1);
        }
    };
    match airl_ir::IRGraph::from_json(&json) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("error: invalid IR: {e}");
            process::exit(1);
        }
    }
}

fn typecheck_or_exit(graph: &airl_ir::IRGraph) {
    let tc_result = airl_typecheck::typecheck(graph.module());
    for w in &tc_result.warnings {
        eprintln!("{w}");
    }
    if !tc_result.is_ok() {
        for e in &tc_result.errors {
            eprintln!("{e}");
        }
        process::exit(1);
    }
}

fn cmd_run(file: &PathBuf) {
    let graph = load_ir(file);
    typecheck_or_exit(&graph);

    match airl_interp::interpret(graph.module()) {
        Ok(output) => {
            print!("{}", output.stdout);
            process::exit(output.exit_code);
        }
        Err(e) => {
            eprintln!("runtime error: {e}");
            process::exit(1);
        }
    }
}

fn cmd_check(file: &PathBuf) {
    let graph = load_ir(file);

    let result = airl_typecheck::typecheck(graph.module());
    for w in &result.warnings {
        eprintln!("{w}");
    }
    for e in &result.errors {
        eprintln!("{e}");
    }

    if result.is_ok() {
        println!(
            "OK — {} function(s) checked",
            graph.module().functions().len()
        );
    } else {
        process::exit(1);
    }
}

fn cmd_compile(file: &PathBuf) {
    let graph = load_ir(file);
    typecheck_or_exit(&graph);

    match airl_compile::compile_and_run(graph.module()) {
        Ok(output) => {
            print!("{}", output.stdout);
            eprintln!(
                "[compiled in {}ms]",
                output.compile_time_ms
            );
            process::exit(output.exit_code);
        }
        Err(e) => {
            eprintln!("compile error: {e}");
            process::exit(1);
        }
    }
}

fn cmd_compile_wasm(file: &PathBuf, output: Option<&std::path::Path>) {
    let graph = load_ir(file);
    typecheck_or_exit(&graph);

    match airl_compile::wasm::compile_to_wasm(graph.module()) {
        Ok(wasm_bytes) => {
            let out_path = output.unwrap_or_else(|| {
                // Default: same name as input but with .wasm extension
                std::path::Path::new("output.wasm")
            });
            if let Err(e) = std::fs::write(out_path, &wasm_bytes) {
                eprintln!("error: cannot write {}: {e}", out_path.display());
                process::exit(1);
            }
            println!(
                "OK — compiled to WASM: {} ({} bytes)",
                out_path.display(),
                wasm_bytes.len()
            );
        }
        Err(e) => {
            eprintln!("WASM compile error: {e}");
            process::exit(1);
        }
    }
}

fn cmd_patch(module_file: &PathBuf, patch_file: &PathBuf, output: Option<&std::path::Path>) {
    let graph = load_ir(module_file);

    let patch_json = match std::fs::read_to_string(patch_file) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("error: cannot read {}: {e}", patch_file.display());
            process::exit(1);
        }
    };

    let patch: airl_patch::Patch = match serde_json::from_str(&patch_json) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: invalid patch JSON: {e}");
            process::exit(1);
        }
    };

    match airl_patch::apply_patch(graph.module(), &patch) {
        Ok(result) => {
            // Type check the result
            let tc = airl_typecheck::typecheck(&result.new_module);
            for w in &tc.warnings {
                eprintln!("{w}");
            }
            if !tc.is_ok() {
                eprintln!("patch applied but type check failed:");
                for e in &tc.errors {
                    eprintln!("  {e}");
                }
                process::exit(1);
            }

            // Write output
            let out_json = serde_json::to_string_pretty(&result.new_module).unwrap();
            let out_path = output.unwrap_or(module_file.as_path());
            if let Err(e) = std::fs::write(out_path, &out_json) {
                eprintln!("error: cannot write {}: {e}", out_path.display());
                process::exit(1);
            }

            println!("OK — patch applied, version: {}", result.new_version);
            if !result.impact.affected_functions.is_empty() {
                println!(
                    "  affected functions: {}",
                    result
                        .impact
                        .affected_functions
                        .iter()
                        .map(|f| f.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
        }
        Err(e) => {
            eprintln!("patch error: {e}");
            process::exit(1);
        }
    }
}
