//! Workspace orchestration reuses package admission and retained compiler
//! sessions. A selection never narrows an existing workspace lock's authority.
use super::*;

const MAX_MEMBERS: usize = 32;
const MAX_SESSIONS_FILES: usize = 1024;
const MAX_SESSIONS_BYTES: usize = 8 * 1024 * 1024;

struct Member {
    path: String,
    root: PathBuf,
    snapshot: runtime::ManifestSnapshot,
}
impl Member {
    fn library(&self) -> bool {
        self.snapshot.manifest.kind.as_deref() == Some("lib")
    }
    fn process(&self, options: &Options) -> bool {
        options
            .entry_mode
            .as_deref()
            .or(self.snapshot.manifest.entry_mode.as_deref())
            .unwrap_or("process")
            == "process"
    }
}

pub(super) struct Workspace {
    root: PathBuf,
    snapshot: runtime::ManifestSnapshot,
    members: Vec<Member>,
    defaults: Vec<usize>,
    current: Option<usize>,
}

fn member_path(value: &str) -> Result<(), String> {
    if value == "." {
        return Ok(());
    }
    if value.is_empty()
        || value.len() > 4096
        || value.contains(['\\', ':', '*', '?', '[', ']'])
        || value.chars().any(char::is_control)
        || value.split('/').count() > 32
        || value
            .split('/')
            .any(|part| matches!(part, "" | "." | "..") || part.len() > 255)
    {
        return Err("workspace members must be exact relative directories without globs, `..`, or special path components".into());
    }
    Ok(())
}

fn member_directory(root: &Path, value: &str) -> Result<PathBuf, String> {
    member_path(value)?;
    let mut path = root.to_path_buf();
    if value != "." {
        for part in value.split('/') {
            path.push(part);
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("cannot inspect workspace member {value}: {error}"))?;
            if !metadata.file_type().is_dir() {
                return Err(format!(
                    "workspace member {value} must use ordinary directories without symbolic links"
                ));
            }
        }
    }
    let canonical = fs::canonicalize(&path)
        .map_err(|error| format!("cannot open workspace member {value}: {error}"))?;
    if !canonical.starts_with(root) {
        return Err(format!("workspace member {value} escapes its workspace"));
    }
    Ok(canonical)
}

pub(super) fn discover() -> Result<Option<Workspace>, String> {
    let cwd = fs::canonicalize(
        std::env::current_dir()
            .map_err(|error| format!("cannot read current directory: {error}"))?,
    )
    .map_err(|error| format!("cannot locate workspace: {error}"))?;
    let mut found = None;
    for (depth, directory) in cwd.ancestors().enumerate() {
        if depth > 64 {
            return Err("workspace discovery exceeds 64 ancestor directories".into());
        }
        let path = directory.join("oxid.toml");
        let declared = path.is_file() && runtime::workspace_declaration(&path)?.is_some();
        if !declared && project_lock::has_workspace_marker(directory)? {
            return Err(format!("workspace lock at {} requires its [workspace] manifest declaration; restore the manifest before running member commands", directory.display()));
        }
        if declared {
            if found.is_some() {
                return Err("nested workspaces are unsupported".into());
            }
            found = Some(directory.to_owned());
        }
    }
    let Some(root) = found else {
        return Ok(None);
    };
    let snapshot = runtime::load_typed_manifest(&root.join("oxid.toml"))?;
    load_roster(root, &cwd, snapshot, &mut RegularProjectReader).map(Some)
}

/// Reuse workspace member/default/nesting admission with already retained root
/// bytes and a caller-supplied bounded manifest reader. No cwd mutation.
pub(super) fn load_roster(
    root: PathBuf,
    cwd: &Path,
    snapshot: runtime::ManifestSnapshot,
    reader: &mut dyn ProjectReader,
) -> Result<Workspace, String> {
    let declaration = snapshot
        .workspace
        .as_ref()
        .ok_or("workspace declaration changed during discovery")?;
    if declaration.edition.as_deref() != Some("typed-preview") {
        return Err("workspace requires explicit edition = \"typed-preview\"".into());
    }
    validate_settings(&snapshot.manifest)?;
    let package_root = snapshot.manifest.edition.is_some()
        || snapshot.manifest.name.is_some()
        || snapshot.manifest.entry.is_some()
        || snapshot.manifest.version.is_some();
    if !package_root
        && (!snapshot.manifest.dependencies.is_empty()
            || !snapshot.manifest.tests.is_empty()
            || snapshot.manifest.kind.is_some()
            || snapshot.manifest.backend.is_some()
            || snapshot.manifest.target.is_some()
            || snapshot.manifest.profile.is_some()
            || snapshot.manifest.reproducible.is_some()
            || snapshot.manifest.entry_mode.is_some()
            || !snapshot.manifest.scripts.is_empty()
            || !snapshot.manifest.features.is_empty())
    {
        return Err("virtual workspace roots cannot declare package dependencies, targets, tests or features".into());
    }
    let mut paths = declaration
        .members
        .clone()
        .ok_or("workspace requires a members array")?;
    if package_root && !paths.iter().any(|path| path == ".") {
        paths.push(".".into());
    }
    if paths.is_empty() || paths.len() > MAX_MEMBERS {
        return Err("workspace requires between 1 and 32 members".into());
    }
    paths.sort();
    let mut members = Vec::new();
    let mut identities = HashSet::new();
    for path in paths {
        if path == "." && !package_root {
            return Err("a virtual workspace root cannot also be a package member".into());
        }
        let directory = member_directory(&root, &path)?;
        if !identities.insert(directory.clone()) {
            return Err(format!("duplicate canonical workspace member {path}"));
        }
        let member_snapshot = if directory == root {
            snapshot.clone()
        } else {
            reader.manifest(&directory.join("oxid.toml"))?
        };
        if member_snapshot.manifest.edition.as_deref() != Some("typed-preview") {
            return Err(format!("workspace member {path} requires explicit typed-preview edition; mixed legacy workspaces are unsupported"));
        }
        if member_snapshot.workspace.is_some() && directory != root {
            return Err(format!("nested workspace at member {path} is unsupported"));
        }
        validate_settings(&member_snapshot.manifest)?;
        let name = member_snapshot
            .manifest
            .name
            .as_deref()
            .ok_or_else(|| format!("workspace member {path} requires a project name"))?;
        validate_name(name)?;
        if directory != root {
            match fs::symlink_metadata(directory.join("oxid.lock")) {
                Ok(_) => return Err(format!("workspace member {path} has a conflicting local oxid.lock; workspace commands use only the root lock")),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {},
                Err(error) => return Err(format!("cannot inspect member lock {path}: {error}")),
            }
        }
        members.push(Member {
            path,
            root: directory,
            snapshot: member_snapshot,
        });
    }
    let defaults = match &declaration.default_members {
        None => (0..members.len()).collect(),
        Some(paths) => {
            if paths.is_empty() {
                return Err("workspace default-members must not be empty".into());
            }
            let mut selected = Vec::new();
            for path in paths {
                member_path(path)?;
                let index = members
                    .iter()
                    .position(|member| &member.path == path)
                    .ok_or_else(|| {
                        format!("workspace default-member {path} is not declared in members")
                    })?;
                if selected.contains(&index) {
                    return Err(format!("duplicate workspace default-member {path}"));
                }
                selected.push(index);
            }
            selected.sort();
            selected
        }
    };
    let nearest = cwd
        .ancestors()
        .take_while(|directory| directory.starts_with(&root))
        .find(|directory| directory.join("oxid.toml").is_file());
    let current = if cwd == root {
        None
    } else {
        nearest.and_then(|directory| members.iter().position(|member| member.root == directory))
    };
    if nearest.is_some_and(|directory| directory != root) && current.is_none() {
        return Err("current package is not a declared workspace member".into());
    }
    Ok(Workspace {
        root,
        snapshot,
        members,
        defaults,
        current,
    })
}

impl Workspace {
    pub(super) fn source_members(
        &self,
    ) -> impl Iterator<Item = (&Path, &runtime::ManifestSnapshot)> {
        self.members
            .iter()
            .map(|member| (member.root.as_path(), &member.snapshot))
    }

    fn graphs(&self, scope: &[usize]) -> Result<Vec<PackageGraph>, String> {
        let mut graphs = Vec::new();
        let mut nodes = 0usize;
        let mut edges = 0usize;
        for &index in scope {
            let graph = package_graph(&self.members[index].root, &self.members[index].snapshot)?;
            nodes += graph.specs.len();
            edges += graph
                .specs
                .iter()
                .map(|package| package.dependencies.len())
                .sum::<usize>();
            if nodes > 256 || edges > 1024 {
                return Err(
                    "workspace exceeds the aggregate 256-package or 1024-edge session limit".into(),
                );
            }
            for dependency in graph.specs.iter().skip(1) {
                if self
                    .members
                    .iter()
                    .any(|member| member.root == dependency.root && !member.library())
                {
                    return Err("binary workspace members cannot be imported as library dependencies; declare kind = \"lib\"".into());
                }
            }
            graphs.push(graph);
        }
        Ok(graphs)
    }

    fn lock_snapshot(
        &self,
        graphs: &[PackageGraph],
        prepared: &[frontend::PreparedPackageProject<'_>],
    ) -> Result<project_lock::Snapshot, String> {
        let fingerprints = prepared
            .iter()
            .enumerate()
            .map(|(slot, session)| session.workspace_fingerprints(&graphs[slot].specs[0]))
            .collect::<Result<Vec<_>, _>>()?;
        let inputs = graphs
            .iter()
            .enumerate()
            .map(|(slot, graph)| project_lock::WorkspaceInputs {
                graph: &graph.specs,
                manifests: &graph.manifests,
                fingerprints: &fingerprints[slot],
            })
            .collect::<Vec<_>>();
        project_lock::expected_workspace(&self.root, &self.snapshot.digest, &inputs)
    }

    pub(super) fn validate_for_doctor(&self) -> Result<String, String> {
        let existing = project_lock::read_workspace(&self.root, false)?;
        let scope = (0..self.members.len()).collect::<Vec<_>>();
        let graphs = self.graphs(&scope)?;
        let mut prepared = Vec::new();
        let (mut files, mut bytes) = (0, 0);
        for graph in &graphs {
            let session = frontend::prepare_package_project(&graph.specs, None)
                .map_err(|error| error.message())?;
            add_usage(&session, &mut files, &mut bytes)?;
            prepared.push(session);
        }
        project_lock::verify(&self.lock_snapshot(&graphs, &prepared)?, existing.as_ref())?;
        let packages = graphs
            .iter()
            .flat_map(|graph| graph.specs.iter().map(|package| &package.root))
            .collect::<HashSet<_>>()
            .len();
        Ok(format!("{} workspace member(s) and {packages} local package(s) admitted; {}. Workspace sources were parsed, not executed or type checked",
            self.members.len(), if existing.is_some() { "workspace oxid.lock verified against retained inputs" } else { "workspace oxid.lock absent (optional); reproducibility is not locked" }))
    }

    pub(super) fn process_run(&self, options: &Options) -> bool {
        options.command() == "run"
            && self
                .selected(options)
                .ok()
                .filter(|selected| selected.len() == 1)
                .map_or(
                    options.entry_mode.as_deref() != Some("result"),
                    |selected| self.members[selected[0]].process(options),
                )
    }

    fn selected(&self, options: &Options) -> Result<Vec<usize>, String> {
        options.validate_workspace_options()?;
        if options.project_test.is_some() {
            let cwd = fs::canonicalize(
                std::env::current_dir()
                    .map_err(|error| format!("cannot read test directory: {error}"))?,
            )
            .map_err(|error| format!("cannot locate test member: {error}"))?;
            let index = self
                .members
                .iter()
                .position(|member| member.root == cwd)
                .ok_or("internal workspace test invocation requires the member root directory")?;
            return Ok(vec![index]);
        }
        if options.workspace {
            return Ok((0..self.members.len()).collect());
        }
        if let Some(selector) = &options.package {
            if let Some(index) = self
                .members
                .iter()
                .position(|member| &member.path == selector)
            {
                return Ok(vec![index]);
            }
            let matching = self
                .members
                .iter()
                .enumerate()
                .filter(|(_, member)| member.snapshot.manifest.name.as_deref() == Some(selector))
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            return match matching.as_slice() {
                [index] => Ok(vec![*index]),
                [] => Err(format!("unknown workspace member `{selector}`")),
                _ => Err(format!(
                    "workspace member name `{selector}` is ambiguous; select an exact member path"
                )),
            };
        }
        Ok(self
            .current
            .map_or_else(|| self.defaults.clone(), |index| vec![index]))
    }

    pub(super) fn execute(&self, options: &Options) -> Result<i32, String> {
        options.validate_lock_options()?;
        let selected = self.selected(options)?;
        let build_options = match options.command() {
            "build" => validate_build_options(options)?,
            "check" | "run" | "test" | "lock" => {
                if options.words.len() != 1
                    && !(matches!(options.command(), "check" | "run")
                        && options.words.len() == 2
                        && options.words[1] == "--")
                {
                    return Err(format!(
                        "unsupported options for workspace {}",
                        options.command()
                    ));
                }
                BuildOptions::default()
            }
            command => {
                return Err(format!(
                    "command `{command}` is unavailable for typed workspaces"
                ))
            }
        };
        let native_settings: Vec<_> = if options.command() == "build" {
            selected
                .iter()
                .map(|&index| build_options.resolve(&self.members[index].snapshot.manifest))
                .collect::<Result<_, _>>()?
        } else {
            Vec::new()
        };
        if matches!(options.command(), "check" | "test" | "lock") && options.entry_mode.is_some() {
            return Err(format!(
                "workspace {} does not accept --entry-mode",
                options.command()
            ));
        }
        let test_child = options.project_test.is_some();
        if options.command() == "run"
            && (selected.len() != 1 || self.members[selected[0]].library() && !test_child)
        {
            return Err(
                "workspace run requires one binary member; select it with --package".into(),
            );
        }
        if options.command() == "build"
            && selected.len() == 1
            && self.members[selected[0]].library()
            && !options.workspace
        {
            return Err("library packages do not produce standalone executables; use check or build --workspace".into());
        }
        let process = options.command() == "run" && self.members[selected[0]].process(options);
        if process && options.json() {
            return Ok(frontend::process_package_project(
                &[],
                None,
                frontend::PackageCommand::Run,
                true,
                true,
            ));
        }
        let existing = project_lock::read_workspace(&self.root, options.requires_lock())?;
        let scope = if existing.is_some() || options.command() == "lock" {
            (0..self.members.len()).collect::<Vec<_>>()
        } else {
            selected.clone()
        };
        let graphs = self.graphs(&scope)?;
        let mut prepared = Vec::new();
        let mut files = 0usize;
        let mut bytes = 0usize;
        for (slot, &index) in scope.iter().enumerate() {
            let member = &self.members[index];
            let entry =
                if !member.library() && (options.command() == "test" || test_child) {
                    let name = if selected == [index] {
                        options.project_test.as_ref()
                    } else {
                        None
                    };
                    let declared =
                        match name {
                            Some(name) => {
                                Some(member.snapshot.manifest.tests.get(name).ok_or_else(|| {
                                    format!("unknown declared project test `{name}`")
                                })?)
                            }
                            None => member
                                .snapshot
                                .manifest
                                .tests
                                .iter()
                                .min_by_key(|(name, _)| *name)
                                .map(|(_, path)| path),
                        };
                    declared
                        .map(|entry| local_source(&member.root, entry))
                        .transpose()?
                } else {
                    None
                };
            let session =
                match frontend::prepare_package_project(&graphs[slot].specs, entry.as_deref()) {
                    Ok(session) => session,
                    Err(error) if options.command() == "lock" => return Err(error.message()),
                    Err(error) => {
                        return Ok(error.report(
                            if options.command() == "run" {
                                frontend::PackageCommand::Run
                            } else {
                                frontend::PackageCommand::Check
                            },
                            options.json(),
                            process,
                        ))
                    }
                };
            add_usage(&session, &mut files, &mut bytes)?;
            prepared.push(session);
        }
        let expected = self.lock_snapshot(&graphs, &prepared)?;
        if options.command() == "lock" {
            let changed =
                project_lock::publish(&self.root, &expected, existing.as_ref(), options.update)?;
            output(options, &format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"workspace-lock-summary\",\"success\":true,\"members\":{},\"changed\":{changed}}}", self.members.len()), if changed { "workspace lock written" } else { "workspace lock verified" })?;
            return Ok(0);
        }
        project_lock::verify(&expected, existing.as_ref())?;
        if test_child {
            let index = selected[0];
            let slot = scope
                .iter()
                .position(|candidate| *candidate == index)
                .expect("selected session");
            let member = &self.members[index];
            let name = options.project_test.as_ref().expect("test child");
            let path = member
                .snapshot
                .manifest
                .tests
                .get(name)
                .ok_or_else(|| format!("unknown declared project test `{name}`"))?;
            let entry = local_source(&member.root, path)?;
            if !member.library() {
                return Ok(prepared[slot].execute(frontend::PackageCommand::Run, false, true));
            }
            let session = frontend::prepare_package_project(&graphs[slot].specs, Some(&entry))
                .map_err(|error| error.message())?;
            verify_test_dependencies(&prepared[slot], &session)?;
            return Ok(session.execute(frontend::PackageCommand::Run, false, true));
        }
        if options.command() == "test" {
            return self.tests(
                options,
                &selected,
                &scope,
                &graphs,
                &prepared,
                existing.is_some(),
                files,
                bytes,
            );
        }
        if options.command() != "check" {
            for (selection, &index) in selected.iter().enumerate() {
                let slot = scope
                    .iter()
                    .position(|candidate| *candidate == index)
                    .expect("selected session");
                let member = &self.members[index];
                let native = options.command() == "build" && !member.library();
                let status = if native {
                    prepared[slot].workspace_preflight_with_native_settings(
                        options.json(),
                        member.process(options),
                        &native_settings[selection],
                        &self.root,
                    )
                } else {
                    prepared[slot].workspace_preflight(options.json(), process, false, process)
                };
                if status != 0 {
                    return Ok(status);
                }
            }
        }
        if options.command() == "run" {
            let slot = scope
                .iter()
                .position(|candidate| *candidate == selected[0])
                .expect("selected session");
            return Ok(prepared[slot].execute_with_execution(
                frontend::PackageCommand::Run,
                options.json(),
                process,
                execution_policy(&self.members[selected[0]].snapshot.manifest)?,
            ));
        }
        let mut failed = 0;
        for (selection, &index) in selected.iter().enumerate() {
            let slot = scope
                .iter()
                .position(|candidate| *candidate == index)
                .expect("selected session");
            let member = &self.members[index];
            let artifact = member.root.join(".oxid/bin").join(
                member
                    .snapshot
                    .manifest
                    .name
                    .as_deref()
                    .expect("admitted name"),
            );
            let command = if options.command() == "build" && !member.library() {
                frontend::PackageCommand::Compile {
                    output: artifact
                        .to_str()
                        .ok_or("workspace output path must be UTF-8")?,
                    managed: true,
                    incremental: build_options.incremental,
                }
            } else {
                frontend::PackageCommand::Check
            };
            let status = if options.command() == "build" && !member.library() {
                prepared[slot].execute_with_native_settings(
                    command,
                    options.json(),
                    member.process(options),
                    &native_settings[selection],
                    &self.root,
                )
            } else {
                prepared[slot].execute(command, options.json(), member.process(options))
            };
            failed += usize::from(status != 0);
            let state = if status != 0 {
                "failed"
            } else if options.command() == "build" && member.library() {
                "checked-library"
            } else {
                "ok"
            };
            output(options, &format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"workspace-member\",\"member\":{},\"status\":{}}}", json_string(&member.path), json_string(state)), &format!("{}: {state}", member.path))?;
        }
        summary(options, selected.len(), failed)?;
        Ok(i32::from(failed != 0))
    }

    #[allow(clippy::too_many_arguments)]
    fn tests(
        &self,
        options: &Options,
        selected: &[usize],
        scope: &[usize],
        graphs: &[PackageGraph],
        prepared: &[frontend::PreparedPackageProject<'_>],
        locked: bool,
        mut files: usize,
        mut bytes: usize,
    ) -> Result<i32, String> {
        let mut tests = Vec::new();
        let mut sessions = Vec::new();
        for &index in selected {
            let member = &self.members[index];
            let slot = scope
                .iter()
                .position(|candidate| *candidate == index)
                .expect("selected session");
            let mut declared = member.snapshot.manifest.tests.iter().collect::<Vec<_>>();
            declared.sort_by_key(|(name, _)| *name);
            for (name, path) in declared {
                if name.len() > 255 || name.is_empty() || name.chars().any(char::is_control) {
                    return Err("workspace test names must be nonempty bounded text without control characters".into());
                }
                if tests.len() == 256 {
                    return Err("workspace exceeds the 256-test limit".into());
                }
                let entry = local_source(&member.root, path)?;
                let session = frontend::prepare_package_project(&graphs[slot].specs, Some(&entry))
                    .map_err(|error| error.message())?;
                add_usage(&session, &mut files, &mut bytes)?;
                verify_test_dependencies(&prepared[slot], &session)?;
                let status = session.workspace_preflight(options.json(), true, false, false);
                if status != 0 {
                    return Ok(status);
                }
                sessions.push(session);
                tests.push((index, name));
            }
        }
        let executable = std::env::current_exe()
            .map_err(|error| format!("cannot locate workspace test runner: {error}"))?;
        let mut failed = 0;
        for (index, name) in &tests {
            let member = &self.members[*index];
            let mut child = Command::new(&executable);
            child
                .current_dir(&member.root)
                .args(["run", "--edition=typed-preview", "--entry-mode=process"])
                .arg(format!("--project-test={name}"))
                .stdin(Stdio::null());
            if options.frozen {
                child.arg("--frozen");
            } else {
                if locked || options.locked {
                    child.arg("--locked");
                }
                if options.offline {
                    child.arg("--offline");
                }
            }
            let result = child
                .output()
                .map_err(|error| format!("cannot run workspace test {name}: {error}"))?;
            let passed = result.status.success();
            failed += usize::from(!passed);
            if options.json() {
                output(options, &format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"test-result\",\"member\":{},\"name\":{},\"success\":{passed},\"exit_status\":{},\"stdout\":{},\"stderr\":{}}}", json_string(&member.path), json_string(name), result.status.code().map_or("null".into(), |code| code.to_string()), json_string(&String::from_utf8_lossy(&result.stdout)), json_string(&String::from_utf8_lossy(&result.stderr))), "")?;
            } else {
                output(
                    options,
                    "",
                    &format!(
                        "test {}::{name}: {} ({})",
                        member.path,
                        if passed { "ok" } else { "FAILED" },
                        result.status
                    ),
                )?;
                let mut stderr = io::stderr().lock();
                stderr
                    .write_all(&result.stdout)
                    .and_then(|()| stderr.write_all(&result.stderr))
                    .map_err(|error| format!("cannot report workspace test: {error}"))?;
            }
        }
        summary(options, tests.len(), failed)?;
        Ok(i32::from(failed != 0))
    }
}

fn verify_test_dependencies(
    normal: &frontend::PreparedPackageProject<'_>,
    test: &frontend::PreparedPackageProject<'_>,
) -> Result<(), String> {
    if normal.dependency_fingerprints() != test.dependency_fingerprints() {
        return Err(
            "workspace dependency sources changed while preparing tests; retry the command".into(),
        );
    }
    Ok(())
}

fn add_usage(
    session: &frontend::PreparedPackageProject<'_>,
    files: &mut usize,
    bytes: &mut usize,
) -> Result<(), String> {
    let (loaded_files, loaded_bytes) = session.input_usage();
    *files = files
        .checked_add(loaded_files)
        .ok_or("workspace source count overflow")?;
    *bytes = bytes
        .checked_add(loaded_bytes)
        .ok_or("workspace source size overflow")?;
    if *files > MAX_SESSIONS_FILES || *bytes > MAX_SESSIONS_BYTES {
        return Err(
            "workspace exceeds the aggregate 1024-file or 8 MiB retained source limit".into(),
        );
    }
    Ok(())
}

fn output(options: &Options, json: &str, human: &str) -> Result<(), String> {
    let result = if options.json() {
        io::stdout()
            .lock()
            .write_all(format!("{json}\n").as_bytes())
    } else {
        io::stderr()
            .lock()
            .write_all(format!("{human}\n").as_bytes())
    };
    result.map_err(|error| format!("cannot report workspace result: {error}"))
}

fn summary(options: &Options, total: usize, failed: usize) -> Result<(), String> {
    output(options, &format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"workspace-summary\",\"operation\":{},\"success\":{},\"total\":{total},\"failed\":{failed}}}", json_string(options.command()), failed == 0), &format!("workspace {}: {} passed; {failed} failed", options.command(), total - failed))
}
