//! Manifest orchestration around the existing typed compiler entry points.
//!
//! Explicit file editions retain the original pure frontend option boundary.
//! Projects never reinterpret typed sources using the legacy interpreter.
use crate::{frontend, runtime};
use std::{
    fs,
    io::{self, Write},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Default)]
struct Options {
    edition: Option<String>,
    entry_mode: Option<String>,
    format: Option<String>,
    words: Vec<String>,
    invalid: bool,
}

impl Options {
    fn scan(args: &[String]) -> Self {
        let mut result = Self::default();
        let mut index = 0;
        while index < args.len() {
            if args[index] == "--"
                || (result.words.first().map(String::as_str) == Some("script")
                    && result.words.len() == 2)
            {
                result.words.extend_from_slice(&args[index..]);
                break;
            }
            let name = args[index].split('=').next().unwrap_or("");
            let slot = match name {
                "--edition" => &mut result.edition,
                "--entry-mode" => &mut result.entry_mode,
                "--message-format" => &mut result.format,
                _ => {
                    result.words.push(args[index].clone());
                    index += 1;
                    continue;
                }
            };
            let value = if let Some((_, value)) = args[index].split_once('=') {
                Some(value.to_owned())
            } else if let Some(value) = args.get(index + 1).filter(|v| !v.starts_with('-')) {
                index += 1;
                Some(value.clone())
            } else {
                None
            };
            if slot.is_some() || value.as_deref().is_none_or(str::is_empty) {
                result.invalid = true;
            }
            *slot = value;
            index += 1;
        }
        result.invalid |= result
            .edition
            .as_deref()
            .is_some_and(|v| !matches!(v, "legacy-0.9" | "typed-preview"));
        result.invalid |= result
            .entry_mode
            .as_deref()
            .is_some_and(|v| !matches!(v, "result" | "process"));
        result.invalid |= result
            .format
            .as_deref()
            .is_some_and(|v| !matches!(v, "text" | "json"));
        result
    }

    fn json(&self) -> bool {
        self.format.as_deref() == Some("json")
    }

    fn command(&self) -> &str {
        self.words.first().map_or("", String::as_str)
    }
}

/// Returns `None` only when the original file/legacy router owns this request.
pub(crate) fn dispatch(args: &mut Vec<String>) -> Option<i32> {
    let options = Options::scan(args);
    if options.invalid || options.edition.as_deref() == Some("legacy-0.9") {
        return None;
    }
    let command = options.command();
    if options.edition.as_deref() == Some("typed-preview")
        && command == "run"
        && options.entry_mode.as_deref() == Some("process")
        && options.json()
    {
        // This invalid combination is independent of project discovery. Keep
        // the frontend's fallible process stderr reporting on every host.
        return None;
    }
    // Tool metadata remains usable inside any project. It cannot execute source
    // or mutate build state; explicit edition selection keeps the old boundary.
    if options.edition.is_none()
        && matches!(command, "help" | "--help" | "-h" | "--version" | "-V" | "")
    {
        return None;
    }
    let operands = operands(&options.words);
    let explicit = options.edition.is_some();
    let project_operation = matches!(command, "build" | "test" | "new" | "init")
        || (matches!(command, "check" | "run") && operands.is_empty());
    if explicit && !project_operation {
        return None;
    }
    if matches!(command, "new" | "init") {
        if !explicit {
            return None;
        }
        return Some(match scaffold(&options) {
            Ok(()) => 0,
            Err(error) => report_error(&options, &error, false),
        });
    }

    let location = if matches!(command, "check" | "run" | "compile" | "fmt") {
        operands.first().map(PathBuf::from)
    } else if !command.starts_with('-') && Path::new(command).extension().is_some() {
        Some(PathBuf::from(command))
    } else {
        None
    };
    let manifest_path = match find_manifest(location.as_deref()) {
        Ok(path) => path,
        Err(error) => return Some(report_error(&options, &error, false)),
    };
    let Some(manifest_path) = manifest_path else {
        if matches!(command, "check" | "run") {
            // With no project entry, the original missing-source CLI error owns
            // this request, including its JSON and process stderr contracts.
            return None;
        }
        return if explicit {
            Some(report_error(
                &options,
                "typed-preview project commands require an oxid.toml manifest",
                false,
            ))
        } else {
            None
        };
    };
    if !explicit {
        match runtime::manifest_edition(&manifest_path) {
            Ok(None) => return None,
            Ok(Some(edition)) if edition == "legacy-0.9" => return None,
            Ok(Some(edition)) if edition == "typed-preview" => {}
            Ok(Some(edition)) => {
                return Some(report_error(
                    &options,
                    &format!("unknown manifest edition `{edition}`; expected legacy-0.9 or typed-preview"),
                    false,
                ));
            }
            Err(error) => return Some(report_error(&options, &error, false)),
        }
    }
    let manifest = match runtime::load_manifest(&manifest_path) {
        Ok(manifest) => manifest,
        Err(error) => {
            return Some(report_error(
                &options,
                &error,
                options.entry_mode.as_deref() != Some("result"),
            ));
        }
    };
    let edition = options
        .edition
        .as_deref()
        .or(manifest.edition.as_deref())
        .unwrap_or("legacy-0.9");
    match edition {
        "legacy-0.9" => return None,
        "typed-preview" => {}
        other => {
            return Some(report_error(
                &options,
                &format!(
                    "unknown manifest edition `{other}`; expected legacy-0.9 or typed-preview"
                ),
                false,
            ));
        }
    }
    let process = options
        .entry_mode
        .as_deref()
        .or(manifest.entry_mode.as_deref())
        .unwrap_or("process")
        == "process";
    if let Err(error) = validate_settings(&manifest) {
        return Some(report_error(&options, &error, process));
    }
    let root = manifest_path.parent().expect("manifest has a parent");
    let result = match command {
        "build" => build(root, &manifest, &options, process),
        "test" => test(root, &manifest, &options),
        "check" | "run" if operands.is_empty() => {
            if options
                .words
                .iter()
                .any(|v| v.starts_with("--experimental-"))
            {
                Err("experimental provider options are unsupported for project commands".into())
            } else {
                match entry_path(root, &manifest) {
                    Ok(entry) => {
                        let Some(entry) = entry.to_str() else {
                            return Some(report_error(
                                &options,
                                "project entry path is not UTF-8",
                                process,
                            ));
                        };
                        args.push(entry.to_owned());
                        inherit(args, &options, process, command);
                        return frontend::dispatch(args);
                    }
                    Err(error) => Err(error),
                }
            }
        }
        "check" | "run" | "compile" | "fmt" => {
            if command == "fmt"
                && operands
                    .first()
                    .is_none_or(|source| Path::new(source).is_dir())
            {
                Err("typed-preview fmt requires one source file; directory formatting is unsupported".into())
            } else {
                inherit(args, &options, process, command);
                return frontend::dispatch(args);
            }
        }
        _ => Err(format!(
            "command `{command}` is unavailable for typed-preview projects"
        )),
    };
    Some(match result {
        Ok(status) => status,
        Err(error) => report_error(&options, &error, process),
    })
}

fn operands(words: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    let mut separated = false;
    let mut words = words.iter().skip(1);
    while let Some(word) = words.next() {
        if !separated && word == "--" {
            separated = true;
        } else if !separated && word.starts_with('-') {
            if matches!(
                word.as_str(),
                "--backend"
                    | "--target"
                    | "--output"
                    | "-o"
                    | "--profile"
                    | "--experimental-hir-import"
                    | "--experimental-hir-producers"
                    | "--experimental-lexical-provider"
            ) {
                words.next();
            }
        } else {
            result.push(word.clone());
        }
    }
    result
}

fn find_manifest(source: Option<&Path>) -> Result<Option<PathBuf>, String> {
    let cwd = std::env::current_dir().map_err(|e| format!("cannot read current directory: {e}"))?;
    let location = source.map_or_else(|| cwd.clone(), |path| cwd.join(path));
    let location = fs::canonicalize(&location).unwrap_or(location);
    let start = if source.is_none() || location.is_dir() {
        location.as_path()
    } else {
        location.parent().unwrap_or(&cwd)
    };
    Ok(start
        .ancestors()
        .map(|dir| dir.join("oxid.toml"))
        .find(|candidate| candidate.is_file()))
}

fn validate_settings(manifest: &runtime::ProjectManifest) -> Result<(), String> {
    if let Some(error) = &manifest.configuration_error {
        return Err(error.clone());
    }
    for (name, value, supported) in [
        ("backend", manifest.backend.as_deref(), "llvm"),
        (
            "target",
            manifest.target.as_deref(),
            "x86_64-unknown-linux-gnu",
        ),
        ("profile", manifest.profile.as_deref(), "bounded"),
    ] {
        if value.is_some_and(|value| value != supported) {
            return Err(format!(
                "typed-preview project {name} supports only `{supported}`"
            ));
        }
    }
    if manifest
        .entry_mode
        .as_deref()
        .is_some_and(|mode| !matches!(mode, "result" | "process"))
    {
        return Err("project entry-mode must be `result` or `process`".into());
    }
    if !manifest.dependencies.is_empty() {
        return Err("typed-preview package dependencies are not implemented; project builds do not download or run legacy dependencies".into());
    }
    if !manifest.features.is_empty() {
        return Err("typed-preview project features are not implemented".into());
    }
    Ok(())
}

fn local_source(root: &Path, value: &str) -> Result<PathBuf, String> {
    let path = Path::new(value);
    if value.is_empty()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
    {
        return Err("project source paths must be nonempty relative paths without `..`".into());
    }
    Ok(root.join(path))
}

fn entry_path(root: &Path, manifest: &runtime::ProjectManifest) -> Result<PathBuf, String> {
    if let Some(entry) = manifest.entry.as_deref() {
        return local_source(root, entry);
    }
    for path in ["src/main.ox", "main.ox"] {
        if root.join(path).is_file() {
            return Ok(root.join(path));
        }
    }
    Err("project entry not found; set [project] entry or create src/main.ox".into())
}

fn inherit(args: &mut Vec<String>, options: &Options, process: bool, command: &str) {
    if options.edition.is_none() {
        args.insert(0, "--edition=typed-preview".into());
    }
    if matches!(command, "run" | "compile") && options.entry_mode.is_none() {
        args.insert(
            0,
            format!(
                "--entry-mode={}",
                if process { "process" } else { "result" }
            ),
        );
    }
    if command == "compile"
        && !args
            .iter()
            .take_while(|word| word.as_str() != "--")
            .any(|word| word == "--backend" || word.starts_with("--backend="))
    {
        let index = args
            .iter()
            .position(|word| word == "--")
            .unwrap_or(args.len());
        args.insert(index, "--backend=llvm".into());
    }
}

fn build(
    root: &Path,
    manifest: &runtime::ProjectManifest,
    options: &Options,
    process: bool,
) -> Result<i32, String> {
    let mut words = options.words.iter().skip(1);
    let mut seen = std::collections::HashSet::new();
    while let Some(word) = words.next() {
        let (name, inline) = word
            .split_once('=')
            .map_or((word.as_str(), None), |(n, v)| (n, Some(v)));
        let supported = match name {
            "--backend" => "llvm",
            "--target" => "x86_64-unknown-linux-gnu",
            "--profile" => "bounded",
            _ => {
                return Err(format!(
                    "unsupported option for typed-preview build: {word}"
                ))
            }
        };
        let value = inline.or_else(|| words.next().map(String::as_str));
        if !seen.insert(name) {
            return Err(format!("{name} may only be specified once"));
        }
        if value != Some(supported) {
            return Err(format!(
                "typed-preview build {name} supports only `{supported}`"
            ));
        }
    }
    let name = manifest.name.as_deref().unwrap_or("app");
    validate_name(name)?;
    let entry = entry_path(root, manifest)?;
    let output = root.join(".oxid/bin").join(name);
    Ok(frontend::build_project(
        entry.to_str().ok_or("project entry path is not UTF-8")?,
        output.to_str().ok_or("project output path is not UTF-8")?,
        options.json(),
        process,
    ))
}

fn test(
    root: &Path,
    manifest: &runtime::ProjectManifest,
    options: &Options,
) -> Result<i32, String> {
    if options.words.len() != 1 || options.entry_mode.is_some() {
        return Err("typed-preview test accepts only edition and message-format options; test entries always use process mode".into());
    }
    if manifest.tests.is_empty() {
        return Err(
            "no typed test entries declared; add name = \"tests/name.ox\" under [tests]".into(),
        );
    }
    let executable =
        std::env::current_exe().map_err(|e| format!("cannot locate test runner: {e}"))?;
    let mut entries: Vec<_> = manifest.tests.iter().collect();
    entries.sort_by_key(|(name, _)| *name);
    // Validate every configured path before running the first user test.
    let entries = entries
        .into_iter()
        .map(|(name, path)| local_source(root, path).map(|path| (name, path)))
        .collect::<Result<Vec<_>, _>>()?;
    let mut passed = 0;
    for (name, path) in &entries {
        let output = Command::new(&executable)
            .current_dir(root)
            .args([
                "run",
                "--edition=typed-preview",
                "--entry-mode=process",
                "--",
            ])
            .arg(path)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("cannot run test {name:?}: {e}"))?;
        let success = output.status.success();
        passed += usize::from(success);
        if options.json() {
            println!(
                "{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"test-result\",\"name\":{},\"success\":{},\"exit_status\":{},\"stdout\":{},\"stderr\":{}}}",
                json_string(name),
                success,
                output.status.code().map_or("null".into(), |code| code.to_string()),
                json_string(&String::from_utf8_lossy(&output.stdout)),
                json_string(&String::from_utf8_lossy(&output.stderr)),
            );
        } else {
            eprintln!(
                "test {name:?}: {} ({})",
                if success { "ok" } else { "FAILED" },
                output.status
            );
            io::stderr()
                .write_all(&output.stdout)
                .map_err(|e| format!("cannot report test output: {e}"))?;
            io::stderr()
                .write_all(&output.stderr)
                .map_err(|e| format!("cannot report test diagnostics: {e}"))?;
        }
    }
    let failed = entries.len() - passed;
    if options.json() {
        println!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"test-summary\",\"success\":{},\"passed\":{passed},\"failed\":{failed}}}", failed == 0);
    } else {
        eprintln!("tests: {passed} passed; {failed} failed");
    }
    Ok(i32::from(failed != 0))
}

fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
    {
        return Err(
            "typed-preview project name must contain only ASCII letters, digits, `_` or `-`".into(),
        );
    }
    Ok(())
}

fn scaffold(options: &Options) -> Result<(), String> {
    if options.entry_mode.is_some() || options.words.len() != 2 || options.words[1].starts_with('-')
    {
        return Err("typed-preview new requires one project directory and accepts only edition and message-format options".into());
    }
    let root = Path::new(&options.words[1]);
    let name = root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("project directory must end with a UTF-8 name")?;
    validate_name(name)?;
    // create_dir reserves the new root without replacing an existing tree.
    if let Some(parent) = root
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|e| format!("cannot create project parent: {e}"))?;
    }
    fs::create_dir(root).map_err(|e| format!("cannot create project {}: {e}", root.display()))?;
    let write = |relative: &str, contents: &str| {
        fs::write(root.join(relative), contents)
            .map_err(|e| format!("cannot create {relative}: {e}"))
    };
    for directory in ["src", "tests"] {
        fs::create_dir(root.join(directory))
            .map_err(|e| format!("cannot create {directory}: {e}"))?;
    }
    write("oxid.toml", &format!("[project]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n\n[build]\nbackend = \"llvm\"\nprofile = \"bounded\"\nentry-mode = \"process\"\n\n[tests]\nsmoke = \"tests/smoke.ox\"\n"))?;
    write("src/main.ox", "fn main() -> i32 {\n    return 0;\n}\n")?;
    write("tests/smoke.ox", "fn double(value: i32) -> i32 {\n    return value * 2;\n}\n\nfn main() -> i32 {\n    if double(21) == 42 {\n        return 0;\n    } else {\n        return 1;\n    }\n}\n")?;
    write(".gitignore", ".oxid/\n")?;
    write("README.md", &format!("# {name}\n\nExperimental typed-preview project.\n\nRun `oxid check`, `oxid run`, and `oxid test` from this directory.\n`oxid build` writes `.oxid/bin/{name}` and safely replaces it on successful rebuilds.\nNative builds and process execution currently require Linux x86_64; builds also require LLVM 19.1.7.\nThe bounded execution policy and typed-preview language limits remain in effect.\n"))?;
    if options.json() {
        println!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"new-summary\",\"success\":true,\"path\":{}}}", json_string(&root.to_string_lossy()));
    } else {
        println!("created typed-preview project: {}", root.display());
    }
    Ok(())
}

fn report_error(options: &Options, message: &str, process: bool) -> i32 {
    let result = if options.json() && !(options.command() == "run" && process) {
        let diagnostic = format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"diagnostic\",\"severity\":\"error\",\"code\":\"E0001\",\"stage\":\"project\",\"message\":{},\"primary\":null,\"secondary\":[],\"notes\":[]}}\n", json_string(message));
        let summary = format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":{},\"success\":false,\"errors\":1}}\n", json_string(&format!("{}-summary", options.command())));
        let mut stdout = io::stdout().lock();
        stdout
            .write_all(diagnostic.as_bytes())
            .and_then(|()| stdout.write_all(summary.as_bytes()))
    } else {
        io::stderr()
            .lock()
            .write_all(format!("error[E0001] (project): {message}\n").as_bytes())
    };
    if result.is_err() {
        // Reporting errors must not panic or switch a process stream to stdout.
        74
    } else {
        1
    }
}

fn json_string(value: &str) -> String {
    let mut output = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                output.push_str(&format!("\\u{:04x}", character as u32))
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}
