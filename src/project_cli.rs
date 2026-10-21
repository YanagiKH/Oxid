//! Manifest orchestration around the existing typed compiler entry points.
//!
//! Explicit file editions retain the original pure frontend option boundary.
//! Projects never reinterpret typed sources using the legacy interpreter.
use crate::{frontend, project_lock, runtime};
use std::{
    collections::{HashMap, HashSet},
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
    project_test: Option<String>,
    project_test_seen: bool,
    locked: bool,
    offline: bool,
    frozen: bool,
    update: bool,
    lock_option_error: Option<String>,
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
            let flag = match name {
                "--locked" => Some(&mut result.locked),
                "--offline" => Some(&mut result.offline),
                "--frozen" => Some(&mut result.frozen),
                "--update" => Some(&mut result.update),
                _ => None,
            };
            if let Some(flag) = flag {
                if *flag || args[index].contains('=') {
                    result.lock_option_error =
                        Some(format!("{name} is a flag and may only be specified once"));
                }
                *flag = true;
                index += 1;
                continue;
            }
            let slot = match name {
                "--edition" => &mut result.edition,
                "--entry-mode" => &mut result.entry_mode,
                "--message-format" => &mut result.format,
                "--project-test" => {
                    result.project_test_seen = true;
                    &mut result.project_test
                }
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

    fn requires_lock(&self) -> bool {
        self.locked || self.frozen
    }

    fn has_lock_options(&self) -> bool {
        self.locked || self.offline || self.frozen || self.update
    }

    fn validate_lock_options(&self) -> Result<(), String> {
        if let Some(error) = &self.lock_option_error {
            return Err(error.clone());
        }
        if self.update && (self.command() != "lock" || self.locked || self.offline || self.frozen) {
            return Err("--update is valid only for `oxid lock` and conflicts with --locked, --offline and --frozen".into());
        }
        if self.has_lock_options()
            && !matches!(self.command(), "lock" | "check" | "run" | "build" | "test")
        {
            return Err(
                "locking options require a typed project lock, check, run, build or test command"
                    .into(),
            );
        }
        Ok(())
    }
}

/// Returns `None` only when the original file/legacy router owns this request.
pub(crate) fn dispatch(args: &mut Vec<String>) -> Option<i32> {
    let options = Options::scan(args);
    if options.project_test_seen
        && (options.invalid
            || options.command() != "run"
            || options.words.len() != 1
            || options.edition.as_deref() != Some("typed-preview")
            || options.entry_mode.as_deref() != Some("process")
            || options.json())
    {
        return Some(report_error(
            &options,
            "invalid internal project test invocation",
            true,
        ));
    }
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
    let project_operation = matches!(command, "lock" | "build" | "test" | "new" | "init")
        || (matches!(command, "check" | "run") && operands.is_empty());
    if explicit && !project_operation {
        return None;
    }
    if matches!(command, "new" | "init") {
        if !explicit {
            return None;
        }
        if let Err(error) = options.validate_lock_options() {
            return Some(report_error(&options, &error, false));
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
    let snapshot = match runtime::load_typed_manifest(&manifest_path) {
        Ok(manifest) => manifest,
        Err(error) => {
            return Some(report_error(
                &options,
                &error,
                options.entry_mode.as_deref() != Some("result"),
            ));
        }
    };
    let manifest = &snapshot.manifest;
    if !explicit && manifest.edition.as_deref() != Some("typed-preview") {
        return Some(report_error(
            &options,
            "manifest edition changed during typed selection; retry the command",
            options.entry_mode.as_deref() != Some("result"),
        ));
    }
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
    if let Err(error) = validate_settings(manifest) {
        return Some(report_error(&options, &error, process));
    }
    if let Err(error) = options.validate_lock_options() {
        return Some(report_error(&options, &error, process));
    }
    let root = manifest_path.parent().expect("manifest has a parent");
    let result = match command {
        "lock" => lock_project(root, &snapshot, &options),
        "build" => build(root, &snapshot, &options, process),
        "test" => test(root, &snapshot, &options),
        "check" | "run" if operands.is_empty() => {
            if options
                .words
                .iter()
                .any(|v| v.starts_with("--experimental-"))
            {
                Err("experimental provider options are unsupported for project commands".into())
            } else {
                project_check_run(root, &snapshot, &options, process)
            }
        }
        "check" | "run" | "compile" | "fmt" => {
            if options.has_lock_options() {
                Err("locking options require a source-less project command".into())
            } else if !manifest.dependencies.is_empty() {
                Err("package dependencies require a source-less project command: use `oxid check`, `oxid run`, `oxid build` or `oxid test`".into())
            } else if command == "fmt"
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

/// Canonical package directories own identities; aliases are local graph edges.
/// Admission reads manifests only and never invokes the legacy resolver.
struct PackageGraph {
    specs: Vec<frontend::PackageSpec>,
    manifests: Vec<runtime::ManifestSnapshot>,
}

fn package_graph(
    root: &Path,
    snapshot: &runtime::ManifestSnapshot,
) -> Result<PackageGraph, String> {
    struct Graph {
        packages: Vec<frontend::PackageSpec>,
        manifests: Vec<runtime::ManifestSnapshot>,
        identities: HashMap<PathBuf, usize>,
        active: Vec<PathBuf>,
        edges: usize,
    }
    impl Graph {
        fn visit(
            &mut self,
            root: PathBuf,
            snapshot: runtime::ManifestSnapshot,
        ) -> Result<usize, String> {
            if self.active.contains(&root) {
                let mut chain: Vec<_> = self
                    .active
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect();
                chain.push(root.display().to_string());
                return Err(format!(
                    "dependencies contain a cycle: {}",
                    chain.join(" -> ")
                ));
            }
            if let Some(&identity) = self.identities.get(&root) {
                return Ok(identity);
            }
            if self.packages.len() >= 256 || self.active.len() > 32 {
                return Err("dependencies exceed the 256-package or 32-level graph limit".into());
            }
            let manifest = &snapshot.manifest;
            let dependency = !self.packages.is_empty();
            if dependency && manifest.edition.as_deref() != Some("typed-preview") {
                return Err(format!(
                    "dependency {} requires explicit edition = \"typed-preview\"; found {}",
                    root.display(),
                    manifest.edition.as_deref().unwrap_or("no edition")
                ));
            }
            if dependency && manifest.name.is_none() {
                return Err(format!(
                    "dependency {} requires a [project] name",
                    root.display()
                ));
            }
            if let Some(name) = &manifest.name {
                validate_name(name)
                    .map_err(|error| format!("package {}: {error}", root.display()))?;
            }
            validate_settings(manifest)
                .map_err(|error| format!("package {}: {error}", root.display()))?;
            let entry = if dependency {
                local_source(&root, manifest.entry.as_deref().unwrap_or("src/lib.ox"))?
            } else {
                entry_path(&root, manifest)?
            };
            let identity = self.packages.len();
            self.identities.insert(root.clone(), identity);
            self.packages.push(frontend::PackageSpec {
                root: root.clone(),
                entry,
                dependencies: Vec::new(),
            });
            self.active.push(root.clone());
            let mut dependencies: Vec<_> = manifest.dependencies.clone().into_iter().collect();
            self.manifests.push(snapshot);
            dependencies.sort_by(|a, b| a.0.cmp(&b.0));
            self.edges = self
                .edges
                .checked_add(dependencies.len())
                .ok_or("dependencies graph size overflow")?;
            if self.edges > 1024 {
                return Err("dependencies exceed the 1024-edge graph limit".into());
            }
            let mut aliases = HashSet::new();
            for (alias, _) in &dependencies {
                validate_alias(alias)?;
                if !aliases.insert(alias.to_ascii_lowercase()) {
                    return Err(format!(
                        "dependency alias `{alias}` collides under ASCII case folding"
                    ));
                }
            }
            for (alias, source) in dependencies {
                let path = Path::new(&source);
                if source.is_empty()
                    || source.len() > 4096
                    || source.chars().any(char::is_control)
                    || source.starts_with("git+")
                    || source.contains("://")
                    || (!path.is_absolute() && source.contains(':'))
                    || source.starts_with(['{', '['])
                {
                    return Err(format!("dependencies support only local directory paths; unsupported source for `{alias}`"));
                }
                let target = fs::canonicalize(root.join(path)).map_err(|error| {
                    format!(
                        "dependencies: cannot open local path for `{alias}` ({source}): {error}"
                    )
                })?;
                if !target.is_dir() {
                    return Err(format!(
                        "dependencies: path for `{alias}` must be a package directory"
                    ));
                }
                if target.to_str().is_none_or(|path| path.len() > 4096) {
                    return Err(format!(
                        "dependencies: path for `{alias}` must be bounded UTF-8"
                    ));
                }
                let child = if let Some(&child) = self.identities.get(&target) {
                    if self.active.contains(&target) {
                        return Err(format!(
                            "dependencies contain a cycle through alias `{alias}` at {}",
                            target.display()
                        ));
                    }
                    child
                } else {
                    let path = target.join("oxid.toml");
                    let metadata = fs::symlink_metadata(&path).map_err(|error| {
                        format!("dependencies: cannot inspect package `{alias}` manifest: {error}")
                    })?;
                    if !metadata.file_type().is_file() {
                        return Err(format!("dependencies: package `{alias}` manifest must be a regular file, not a symbolic link or special file"));
                    }
                    let child_manifest = runtime::load_typed_manifest(&path).map_err(|error| {
                        format!("dependencies: cannot load package `{alias}`: {error}")
                    })?;
                    self.visit(target, child_manifest)?
                };
                self.packages[identity]
                    .dependencies
                    .push(frontend::PackageDependency {
                        alias,
                        package: child,
                    });
            }
            self.active.pop();
            Ok(identity)
        }
    }
    let canonical_root =
        fs::canonicalize(root).map_err(|error| format!("cannot open project root: {error}"))?;
    let mut graph = Graph {
        packages: Vec::new(),
        manifests: Vec::new(),
        identities: HashMap::new(),
        active: Vec::new(),
        edges: 0,
    };
    graph.visit(canonical_root, snapshot.clone())?;
    fn longest_path(
        package: usize,
        graph: &[frontend::PackageSpec],
        lengths: &mut [Option<usize>],
    ) -> usize {
        if let Some(length) = lengths[package] {
            return length;
        }
        let length = graph[package]
            .dependencies
            .iter()
            .map(|edge| 1 + longest_path(edge.package, graph, lengths))
            .max()
            .unwrap_or(0);
        lengths[package] = Some(length);
        length
    }
    if longest_path(0, &graph.packages, &mut vec![None; graph.packages.len()]) > 32 {
        return Err("dependencies exceed the 32-level graph path limit".into());
    }
    Ok(PackageGraph {
        specs: graph.packages,
        manifests: graph.manifests,
    })
}

pub(crate) fn validate_alias(alias: &str) -> Result<(), String> {
    let mut bytes = alias.bytes();
    if alias.len() > 64
        || !bytes
            .next()
            .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        || !bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        || matches!(
            alias,
            "crate"
                | "std"
                | "self"
                | "super"
                | "bool"
                | "i32"
                | "u8"
                | "fn"
                | "struct"
                | "mod"
                | "use"
                | "pub"
                | "let"
                | "mut"
                | "return"
                | "break"
                | "continue"
                | "if"
                | "while"
                | "else"
                | "true"
                | "false"
                | "import"
                | "macro"
                | "macro_rules"
                | "const"
                | "for"
                | "loop"
                | "match"
                | "async"
                | "await"
                | "move"
                | "ref"
                | "unsafe"
                | "extern"
                | "enum"
                | "trait"
                | "impl"
                | "type"
                | "null"
                | "and"
                | "or"
        )
    {
        return Err(format!("invalid typed dependency alias `{alias}`; use an ASCII identifier of at most 64 bytes that is not reserved"));
    }
    Ok(())
}

fn project_check_run(
    root: &Path,
    snapshot: &runtime::ManifestSnapshot,
    options: &Options,
    process: bool,
) -> Result<i32, String> {
    if options.words.len() != 1 && !(options.words.len() == 2 && options.words[1] == "--") {
        return Err(format!(
            "unsupported options for typed-preview project {}",
            options.command()
        ));
    }
    if options.command() == "check" && options.entry_mode.is_some() {
        return Err("project check does not support --entry-mode".into());
    }
    let entry_override = options
        .project_test
        .as_ref()
        .map(|name| {
            snapshot
                .manifest
                .tests
                .get(name)
                .map(String::as_str)
                .ok_or_else(|| format!("unknown declared project test `{name}`"))
        })
        .transpose()?;
    let command = if options.command() == "check" {
        frontend::PackageCommand::Check
    } else {
        frontend::PackageCommand::Run
    };
    execute_project(root, snapshot, options, process, command, entry_override)
}

fn lock_snapshot(
    graph: &PackageGraph,
    prepared: &frontend::PreparedPackageProject<'_>,
) -> Result<project_lock::Snapshot, String> {
    let fingerprints = prepared
        .dependency_fingerprints()
        .iter()
        .map(|fingerprint| (fingerprint.package, fingerprint.checksum.clone()))
        .collect::<Vec<_>>();
    project_lock::expected(&graph.specs, &graph.manifests, &fingerprints)
}

/// Diagnose only admitted package inputs. No user script, program or test runs,
/// and an existing typed lock is checked using the same retained source session.
pub(crate) fn native_doctor_project() -> Result<Option<String>, String> {
    // A broken nearest manifest is a project error, not permission to silently
    // select an ancestor or diagnose the directory as having no project.
    let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
    let mut path = None;
    for directory in cwd.ancestors() {
        let candidate = directory.join("oxid.toml");
        match fs::symlink_metadata(&candidate) {
            Ok(_) => {
                path = Some(candidate);
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("cannot inspect project manifest: {error}")),
        }
    }
    let Some(path) = path else {
        return Ok(None);
    };
    let snapshot = runtime::load_typed_manifest(&path)?;
    match snapshot.manifest.edition.as_deref() {
        None | Some("legacy-0.9") => return Ok(None),
        Some("typed-preview") => {}
        Some(_) => {
            return Err("unknown manifest edition; expected legacy-0.9 or typed-preview".into())
        }
    }
    validate_settings(&snapshot.manifest)?;
    let root = path.parent().expect("manifest parent");
    let existing = project_lock::read(root, false)?;
    let graph = package_graph(root, &snapshot)?;
    let prepared =
        frontend::prepare_package_project(&graph.specs, None).map_err(|error| error.message())?;
    if existing.is_some() {
        project_lock::verify(&lock_snapshot(&graph, &prepared)?, existing.as_ref())?;
    }
    Ok(Some(format!("{} local package(s) admitted; {}. Project sources were parsed, not executed or type checked",
        graph.specs.len(), if existing.is_some() { "oxid.lock verified against retained inputs" } else { "oxid.lock absent (optional); reproducibility is not locked" })))
}

fn execute_project(
    root: &Path,
    snapshot: &runtime::ManifestSnapshot,
    options: &Options,
    process: bool,
    command: frontend::PackageCommand<'_>,
    entry_override: Option<&str>,
) -> Result<i32, String> {
    if options.command() == "run" && process && options.json() {
        // Retain the pure process-option rejection before source loading.
        return Ok(frontend::process_package_project(
            &[],
            None,
            command,
            true,
            true,
        ));
    }
    let existing = project_lock::read(root, options.requires_lock())?;
    let graph = package_graph(root, snapshot)?;
    let entry_override = entry_override
        .map(|entry| local_source(&graph.specs[0].root, entry))
        .transpose()?;
    let prepared = match frontend::prepare_package_project(&graph.specs, entry_override.as_deref())
    {
        Ok(prepared) => prepared,
        Err(error) => return Ok(error.report(command, options.json(), process)),
    };
    if existing.is_some() {
        project_lock::verify(&lock_snapshot(&graph, &prepared)?, existing.as_ref())?;
    }
    Ok(prepared.execute(command, options.json(), process))
}

fn lock_project(
    root: &Path,
    snapshot: &runtime::ManifestSnapshot,
    options: &Options,
) -> Result<i32, String> {
    if options.words.len() != 1 || options.entry_mode.is_some() {
        return Err(
            "typed-preview lock accepts edition, message-format and locking options only".into(),
        );
    }
    let existing = project_lock::read(root, options.requires_lock())?;
    let graph = package_graph(root, snapshot)?;
    let prepared =
        frontend::prepare_package_project(&graph.specs, None).map_err(|error| error.message())?;
    let expected = lock_snapshot(&graph, &prepared)?;
    let changed = project_lock::publish(root, &expected, existing.as_ref(), options.update)?;
    let message = if options.json() {
        format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"lock-summary\",\"success\":true,\"changed\":{changed}}}\n")
    } else {
        format!(
            "typed lock {}\n",
            if changed {
                if existing.is_some() {
                    "updated"
                } else {
                    "created"
                }
            } else {
                "verified"
            }
        )
    };
    io::stdout()
        .lock()
        .write_all(message.as_bytes())
        .map_err(|error| format!("cannot report typed lock: {error}"))?;
    Ok(0)
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
    snapshot: &runtime::ManifestSnapshot,
    options: &Options,
    process: bool,
) -> Result<i32, String> {
    let mut words = options.words.iter().skip(1);
    let mut seen = std::collections::HashSet::new();
    let mut incremental = false;
    while let Some(word) = words.next() {
        if word == "--incremental" {
            if incremental {
                return Err("--incremental may only be specified once".into());
            }
            incremental = true;
            continue;
        }
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
    let name = snapshot.manifest.name.as_deref().unwrap_or("app");
    validate_name(name)?;
    let output = root.join(".oxid/bin").join(name);
    execute_project(
        root,
        snapshot,
        options,
        process,
        frontend::PackageCommand::Compile {
            output: output.to_str().ok_or("project output path is not UTF-8")?,
            managed: true,
            incremental,
        },
        None,
    )
}

fn test(
    root: &Path,
    snapshot: &runtime::ManifestSnapshot,
    options: &Options,
) -> Result<i32, String> {
    let manifest = &snapshot.manifest;
    if options.words.len() != 1 || options.entry_mode.is_some() {
        return Err("typed-preview test accepts edition, message-format and locking options; test entries always use process mode".into());
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
    // Admit every package manifest before starting any declared user test.
    // Each child repeats admission and compiles its own entry in one session.
    let existing = project_lock::read(root, options.requires_lock())?;
    let graph = package_graph(root, snapshot)?;
    if existing.is_some() {
        let test_entry = local_source(&graph.specs[0].root, &manifest.tests[entries[0].0])?;
        let prepared = frontend::prepare_package_project(&graph.specs, Some(&test_entry))
            .map_err(|error| error.message())?;
        project_lock::verify(&lock_snapshot(&graph, &prepared)?, existing.as_ref())?;
    }
    let mut passed = 0;
    for (name, _) in &entries {
        let mut child = Command::new(&executable);
        child
            .current_dir(root)
            .args(["run", "--edition=typed-preview", "--entry-mode=process"])
            .arg(format!("--project-test={name}"))
            .stdin(Stdio::null());
        if options.frozen {
            child.arg("--frozen");
        } else {
            if options.locked || existing.is_some() {
                child.arg("--locked");
            }
            if options.offline {
                child.arg("--offline");
            }
        }
        let output = child
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
