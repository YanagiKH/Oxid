include!("main.rs");

fn help() {
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

fn run_tool(interp: &mut Interpreter, path: &str) -> Result<(), String> {
    run_file(Path::new(path), interp)
}

fn run_check(path: &Path) -> Result<(), String> {
    let canonical = fs::canonicalize(path).map_err(|e| format!("cannot open file: {} ({})", path.display(), e))?;
    let source = fs::read_to_string(&canonical).map_err(|e| format!("cannot read file: {} ({})", canonical.display(), e))?;
    let base_dir = canonical.parent().unwrap_or(Path::new("."));
    let processed = cached_preprocess(&source, base_dir)?;
    let mut parser = Parser::new(&processed);
    parser.parse_program()?;
    println!("syntax ok: {}", canonical.display());
    Ok(())
}

fn clean_project(root: &Path) -> Result<(), String> {
    let oxid_dir = root.join(".oxid");
    if oxid_dir.exists() {
        fs::remove_dir_all(&oxid_dir).map_err(|e| format!("cannot clean {}: {}", oxid_dir.display(), e))?;
    }
    println!("clean ok");
    Ok(())
}

fn watch_file(path: &Path, interp: &mut Interpreter) -> Result<(), String> {
    let canonical = fs::canonicalize(path).map_err(|e| format!("cannot open file: {} ({})", path.display(), e))?;
    let mut last_modified = fs::metadata(&canonical)
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);

    loop {
        run_file(&canonical, interp)?;
        println!("watching {}", canonical.display());
        loop {
            thread::sleep(Duration::from_millis(500));
            let current = fs::metadata(&canonical)
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            if current != last_modified {
                last_modified = current;
                break;
            }
        }
    }
}

fn dispatch(args: &[String], interp: &mut Interpreter) -> Result<(), String> {
    match args.get(1).map(|s| s.as_str()) {
        Some("run") => {
            let file = args.get(2).ok_or_else(|| "`oxid run` requires a file path".to_string())?;
            run_file(Path::new(file), interp)
        }
        Some("script") => match args.get(2) {
            Some(name) => run_manifest_script(Path::new("."), name, &args[3..]),
            None => Err("`oxid script` requires a script name".to_string()),
        },
        Some("bootstrap") => run_tool(interp, "tools/bootstrap.ox"),
        Some("compile") => run_tool(interp, "tools/compile.ox"),
        Some("self-compile") => run_tool(interp, "tools/self_compile.ox"),
        Some("frontend") => run_tool(interp, "tools/frontend_preview.ox"),
        Some("diagnose") => run_tool(interp, "tools/diagnose.ox"),
        Some("lint") => run_tool(interp, "tools/lint.ox"),
        Some("emit") => run_tool(interp, "tools/emit.ox"),
        Some("module") => run_tool(interp, "tools/module.ox"),
        Some("syntax") => run_tool(interp, "tools/syntax.ox"),
        Some("interop") => run_tool(interp, "tools/interop.ox"),
        Some("bridge") => run_tool(interp, "tools/bridge.ox"),
        Some("self-host") => run_tool(interp, "tools/self_host.ox"),
        Some("check") => {
            let file = args.get(2).ok_or_else(|| "`oxid check` requires a file path".to_string())?;
            run_check(Path::new(file))
        }
        Some("repl") => repl(interp),
        Some("new") | Some("init") => {
            let name = args.get(2).ok_or_else(|| "`oxid new` requires a project name".to_string())?;
            scaffold_project(name)
        }
        Some("add") => {
            let name = args.get(2).ok_or_else(|| "`oxid add` requires a dependency name".to_string())?;
            let target = args.get(3).ok_or_else(|| "`oxid add` requires a dependency target".to_string())?;
            add_dependency(Path::new("."), name, target)
        }
        Some("watch") => {
            let file = args.get(2).ok_or_else(|| "`oxid watch` requires a file path".to_string())?;
            watch_file(Path::new(file), interp)
        }
        Some("build") => build_project(Path::new(".")),
        Some("clean") => clean_project(Path::new(".")),
        Some("fmt") => {
            if let Some(path) = args.get(2) {
                let target = Path::new(path);
                if target.is_file() {
                    let source = fs::read_to_string(target)
                        .map_err(|e| format!("cannot read file: {} ({})", target.display(), e))?;
                    let formatted = format_source(&source);
                    if formatted != source {
                        fs::write(target, formatted).map_err(|e| format!("cannot write file: {} ({})", target.display(), e))?;
                    }
                    Ok(())
                } else {
                    format_project(target)
                }
            } else {
                format_project(Path::new("."))
            }
        }
        Some("test") => run_test_suite(Path::new(".")),
        Some("doctor") => doctor_project(Path::new(".")),
        Some("doc") => document_project(Path::new(".")),
        Some("help") | Some("--help") | Some("-h") => {
            help();
            Ok(())
        }
        Some("--version") | Some("-V") => {
            println!("Oxid 0.7.0");
            Ok(())
        }
        Some(other) => Err(format!("unknown command: {}", other)),
        None => {
            help();
            Ok(())
        }
    }
}

fn main() {
    let args = env::args().collect::<Vec<_>>();
    let mut interp = Interpreter::new();
    if let Err(err) = dispatch(&args, &mut interp) {
        eprintln!("{}", err);
        std::process::exit(1);
    }
}
