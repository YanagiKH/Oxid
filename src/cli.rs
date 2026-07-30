mod runtime {
    include!("main.rs");

    pub fn entry() {
        main();
    }
}

fn print_help() {
    println!("Oxid 0.7.0");
    println!("Usage:");
    println!("  oxid run <file.ox>");
    println!("  oxid script <name> [args...]");
    println!("  oxid bootstrap");
    println!("  oxid compile");
    println!("  oxid self-compile");
    println!("  oxid frontend");
    println!("  oxid diagnose");
    println!("  oxid lint");
    println!("  oxid emit");
    println!("  oxid module");
    println!("  oxid syntax");
    println!("  oxid interop");
    println!("  oxid bridge");
    println!("  oxid self-host");
    println!("  oxid check <file.ox>");
    println!("  oxid repl");
    println!("  oxid new <project-name>");
    println!("  oxid init <project-name>");
    println!("  oxid add <name> <path-or-target>");
    println!("  oxid watch <file.ox>");
    println!("  oxid build");
    println!("  oxid clean");
    println!("  oxid fmt [path]");
    println!("  oxid test");
    println!("  oxid doctor");
    println!("  oxid doc");
}

fn run_tool(script_path: &str) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("failed to resolve current executable: {}", e))?;
    let status = std::process::Command::new(exe)
        .args(["run", script_path])
        .status()
        .map_err(|e| format!("failed to launch {}: {}", script_path, e))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{} exited with status {}", script_path, status))
    }
}

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let result = match args.get(1).map(|s| s.as_str()) {
        None | Some("help") | Some("--help") | Some("-h") => {
            print_help();
            Ok(())
        }
        Some("--version") | Some("-V") => {
            println!("Oxid 0.7.0");
            Ok(())
        }
        Some("bootstrap") => run_tool("tools/bootstrap.ox"),
        Some("compile") => run_tool("tools/compile.ox"),
        Some("self-compile") => run_tool("tools/self_compile.ox"),
        Some("frontend") => run_tool("tools/frontend_preview.ox"),
        Some("diagnose") => run_tool("tools/diagnose.ox"),
        Some("lint") => run_tool("tools/lint.ox"),
        Some("emit") => run_tool("tools/emit.ox"),
        Some("module") => run_tool("tools/module.ox"),
        Some("syntax") => run_tool("tools/syntax.ox"),
        Some("interop") => run_tool("tools/interop.ox"),
        Some("bridge") => run_tool("tools/bridge.ox"),
        Some("self-host") => run_tool("tools/self_host.ox"),
        _ => {
            runtime::entry();
            Ok(())
        }
    };

    if let Err(err) = result {
        eprintln!("error: {}", err);
        std::process::exit(1);
    }
}
