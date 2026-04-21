//! AIRL CLI - Command-line interface for the AIRL toolchain.

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use std::path::PathBuf;
use std::process;

#[derive(Parser)]
#[command(
    name = "airl",
    version,
    about = "AIRL - AI-native Intermediate Representation Language"
)]
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
        /// Additional directories to load modules from (multi-module support)
        #[arg(long)]
        include: Vec<PathBuf>,
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
    /// Project module to TypeScript or Python
    Project {
        /// Path to the .airl.json file
        file: PathBuf,
        /// Target language: typescript, python
        #[arg(long, default_value = "typescript")]
        lang: String,
    },
    /// Interactive REPL: paste JSON IR, get results
    Repl,
    /// Start the HTTP API server
    Api {
        #[command(subcommand)]
        action: ApiAction,
    },
    /// Generate shell completions (prints to stdout)
    ///
    /// Example:
    ///   # Bash: write to system completions dir
    ///   airl completions bash > /etc/bash_completion.d/airl
    ///
    ///   # Zsh: write to a directory on fpath
    ///   airl completions zsh > ~/.zsh/completions/_airl
    ///
    ///   # Fish:
    ///   airl completions fish > ~/.config/fish/completions/airl.fish
    ///
    ///   # PowerShell:
    ///   airl completions powershell >> $PROFILE
    Completions {
        /// Shell to generate completions for (bash, zsh, fish, powershell, elvish)
        shell: Shell,
    },
}

#[derive(Subcommand)]
enum ApiAction {
    /// Start the API server
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "9090")]
        port: u16,
        /// API tokens for authentication (comma-separated). If set, all requests
        /// require `Authorization: Bearer <token>` header.
        #[arg(long)]
        auth_tokens: Option<String>,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run {
            file,
            compiled,
            include,
        } => {
            if !include.is_empty() {
                cmd_run_workspace(&file, &include, compiled);
            } else if compiled {
                cmd_compile(&file);
            } else {
                cmd_run(&file);
            }
        }
        Commands::Check { file } => cmd_check(&file),
        Commands::Compile {
            file,
            target,
            output,
        } => {
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
        Commands::Project { file, lang } => cmd_project(&file, &lang),
        Commands::Repl => cmd_repl(),
        Commands::Api { action } => match action {
            ApiAction::Serve { port, auth_tokens } => {
                if let Some(tokens_str) = auth_tokens {
                    let tokens: Vec<String> = tokens_str
                        .split(',')
                        .map(|t| t.trim().to_string())
                        .filter(|t| !t.is_empty())
                        .collect();
                    airl_api::serve_with_auth(port, tokens).await;
                } else {
                    airl_api::serve(port).await;
                }
            }
        },
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            let name = cmd.get_name().to_string();
            clap_complete::generate(shell, &mut cmd, name, &mut std::io::stdout());
        }
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

fn cmd_run_workspace(main_file: &std::path::Path, include_dirs: &[PathBuf], compiled: bool) {
    use airl_project::workspace::Workspace;

    let mut ws = Workspace::new();

    // Load main file
    if let Err(e) = ws.load_file(main_file) {
        eprintln!("error loading {}: {e}", main_file.display());
        process::exit(1);
    }

    // Load additional directories
    for dir in include_dirs {
        if let Err(e) = ws.load_dir(dir.as_path()) {
            eprintln!("error loading {}: {e}", dir.display());
            process::exit(1);
        }
    }

    // Resolve into merged module
    let merged = match ws.resolve() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("workspace error: {e}");
            process::exit(1);
        }
    };

    // Type check
    let tc = airl_typecheck::typecheck(&merged);
    for w in &tc.warnings {
        eprintln!("{w}");
    }
    if !tc.is_ok() {
        for e in &tc.errors {
            eprintln!("{e}");
        }
        process::exit(1);
    }

    eprintln!(
        "[workspace: {} module(s), {} function(s)]",
        ws.modules.len(),
        merged.functions().len()
    );

    if compiled {
        match airl_compile::compile_and_run(&merged) {
            Ok(output) => {
                print!("{}", output.stdout);
                process::exit(output.exit_code);
            }
            Err(e) => {
                eprintln!("compile error: {e}");
                process::exit(1);
            }
        }
    } else {
        match airl_interp::interpret(&merged) {
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
            eprintln!("[compiled in {}ms]", output.compile_time_ms);
            process::exit(output.exit_code);
        }
        Err(e) => {
            eprintln!("compile error: {e}");
            process::exit(1);
        }
    }
}

fn cmd_project(file: &PathBuf, lang: &str) {
    let graph = load_ir(file);
    typecheck_or_exit(&graph);

    if let Some(language) = airl_project::projection::Language::parse(lang) {
        let text = airl_project::projection::project_module(graph.module(), language);
        print!("{text}");
    } else {
        eprintln!("error: unknown language '{lang}' (supported: typescript, python)");
        process::exit(1);
    }
}

fn cmd_repl() {
    use std::io::{self, BufRead, Write};

    eprintln!("AIRL REPL — paste a complete .airl.json module, then press Ctrl+D (Unix) or Ctrl+Z (Windows)");
    eprintln!("Commands: :quit, :check, :compile, :typescript, :python");
    eprintln!();

    let stdin = io::stdin();
    let mut current_module: Option<airl_ir::Module> = None;
    let mut buffer = String::new();
    let mut collecting = false;

    print!("> ");
    io::stdout().flush().ok();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        let trimmed = line.trim();

        // Commands
        if trimmed == ":quit" || trimmed == ":q" {
            break;
        }

        if trimmed == ":check" {
            if let Some(ref module) = current_module {
                let result = airl_typecheck::typecheck(module);
                if result.is_ok() {
                    println!("OK — {} function(s)", module.functions().len());
                } else {
                    for e in &result.errors {
                        println!("  error: {}", e.message);
                    }
                }
            } else {
                println!("no module loaded");
            }
            print!("> ");
            io::stdout().flush().ok();
            continue;
        }

        if trimmed == ":compile" {
            if let Some(ref module) = current_module {
                match airl_compile::compile_and_run(module) {
                    Ok(out) => print!("{}", out.stdout),
                    Err(e) => println!("compile error: {e}"),
                }
            } else {
                println!("no module loaded");
            }
            print!("> ");
            io::stdout().flush().ok();
            continue;
        }

        if trimmed == ":typescript" || trimmed == ":ts" {
            if let Some(ref module) = current_module {
                let text = airl_project::projection::project_module(
                    module,
                    airl_project::projection::Language::TypeScript,
                );
                print!("{text}");
            } else {
                println!("no module loaded");
            }
            print!("> ");
            io::stdout().flush().ok();
            continue;
        }

        if trimmed == ":python" || trimmed == ":py" {
            if let Some(ref module) = current_module {
                let text = airl_project::projection::project_module(
                    module,
                    airl_project::projection::Language::Python,
                );
                print!("{text}");
            } else {
                println!("no module loaded");
            }
            print!("> ");
            io::stdout().flush().ok();
            continue;
        }

        // JSON collection
        if trimmed.starts_with('{') || collecting {
            collecting = true;
            buffer.push_str(&line);
            buffer.push('\n');

            // Try to parse
            if let Ok(module) = serde_json::from_str::<airl_ir::Module>(&buffer) {
                collecting = false;

                // Type check
                let tc = airl_typecheck::typecheck(&module);
                if !tc.is_ok() {
                    for e in &tc.errors {
                        println!("  type error: {}", e.message);
                    }
                } else {
                    // Interpret
                    match airl_interp::interpret(&module) {
                        Ok(output) => {
                            if !output.stdout.is_empty() {
                                print!("{}", output.stdout);
                            }
                            println!("OK — {} function(s)", module.functions().len());
                        }
                        Err(e) => println!("runtime error: {e}"),
                    }
                }

                current_module = Some(module);
                buffer.clear();
            }
            // else keep collecting more lines

            if !collecting {
                print!("> ");
                io::stdout().flush().ok();
            }
            continue;
        }

        if !trimmed.is_empty() {
            println!("unknown command: {trimmed}");
        }
        print!("> ");
        io::stdout().flush().ok();
    }
    eprintln!("\nbye!");
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
