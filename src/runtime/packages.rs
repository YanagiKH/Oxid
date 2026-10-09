use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};

const LOCKFILE_NAME: &str = "oxid.lock";
const LOCKFILE_VERSION: u32 = 1;
const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x00000100000001b3;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResolveOptions {
    pub locked: bool,
    pub offline: bool,
    pub update: bool,
}
impl From<&ResolveOptions> for ResolveOptions {
    fn from(value: &ResolveOptions) -> Self {
        *value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LockedPackage {
    pub name: String,
    pub source: String,
    pub revision: Option<String>,
    pub checksum: String,
    pub dependencies: Vec<String>,
}

#[derive(Debug)]
enum DependencySource {
    Path(PathBuf),
    Git {
        url: String,
        revision: String,
        network: bool,
    },
}

#[derive(Debug)]
struct MaterializedPackage {
    root: PathBuf,
    source: String,
    revision: Option<String>,
}

struct Resolver {
    project_root: PathBuf,
    options: ResolveOptions,
    packages: BTreeMap<String, LockedPackage>,
    checksum_algorithms: HashMap<String, ChecksumAlgorithm>,
    identities: HashMap<String, String>,
    visiting: Vec<String>,
    visited: HashSet<String>,
}

pub fn resolve_dependencies<O: Into<ResolveOptions>>(
    root: &Path,
    dependencies: &HashMap<String, String>,
    options: O,
) -> Result<Vec<LockedPackage>, String> {
    let options = options.into();
    if options.locked && options.update {
        return Err("locked and update modes cannot be used together".to_string());
    }
    if options.offline && options.update {
        return Err("offline and update modes cannot be used together".to_string());
    }

    let project_root = fs::canonicalize(root)
        .map_err(|error| format!("cannot resolve project root {}: {error}", root.display()))?;
    if !project_root.is_dir() {
        return Err(format!(
            "project root is not a directory: {}",
            project_root.display()
        ));
    }

    let lock_path = project_root.join(LOCKFILE_NAME);
    let previous = if lock_path.exists() {
        Some(read_lockfile(&lock_path)?)
    } else {
        None
    };
    if options.locked && previous.is_none() {
        return Err(format!("locked mode requires {}", lock_path.display()));
    }

    // Existing lock entries retain their digest contract until explicit update.
    let checksum_algorithms = if options.update {
        HashMap::new()
    } else {
        previous
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|package| {
                Ok((
                    package.name.clone(),
                    ChecksumAlgorithm::parse(&package.checksum)?,
                ))
            })
            .collect::<Result<HashMap<_, _>, String>>()?
    };
    let mut resolver = Resolver {
        project_root: project_root.clone(),
        options,
        packages: BTreeMap::new(),
        checksum_algorithms,
        identities: HashMap::new(),
        visiting: Vec::new(),
        visited: HashSet::new(),
    };

    let mut declared = dependencies.iter().collect::<Vec<_>>();
    declared.sort_by(|left, right| left.0.cmp(right.0));
    for (name, source) in declared {
        resolver.resolve_one(name, source, &project_root)?;
    }

    let resolved = resolver.packages.into_values().collect::<Vec<_>>();
    validate_lock_transition(previous.as_deref(), &resolved, options)?;
    if !options.locked {
        write_lockfile(&lock_path, &resolved)?;
    }
    Ok(resolved)
}

impl Resolver {
    fn resolve_one(&mut self, name: &str, spec: &str, declaring_root: &Path) -> Result<(), String> {
        validate_package_name(name)?;
        if let Some(index) = self.visiting.iter().position(|current| current == name) {
            let mut cycle = self.visiting[index..].to_vec();
            cycle.push(name.to_string());
            return Err(format!("dependency cycle detected: {}", cycle.join(" -> ")));
        }

        let materialized = self.materialize(name, spec, declaring_root)?;
        let identity = format!(
            "{}#{}",
            materialized.source,
            materialized.revision.as_deref().unwrap_or("")
        );
        if let Some(existing) = self.identities.get(name) {
            if existing != &identity {
                return Err(format!(
                    "dependency `{name}` resolves to conflicting sources: {existing} and {identity}"
                ));
            }
        } else {
            self.identities.insert(name.to_string(), identity);
        }
        if self.visited.contains(name) {
            return Ok(());
        }

        self.visiting.push(name.to_string());
        let manifest_path = materialized.root.join("oxid.toml");
        let nested = if manifest_path.is_file() {
            list_dependencies(&manifest_path)?
        } else {
            Vec::new()
        };
        let dependency_names = nested
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        for (nested_name, nested_source) in &nested {
            self.resolve_one(nested_name, nested_source, &materialized.root)?;
        }

        let algorithm = self
            .checksum_algorithms
            .get(name)
            .copied()
            .unwrap_or(ChecksumAlgorithm::Sha256);
        let checksum = hash_package_tree_with_algorithm(&materialized.root, algorithm)?;
        self.visiting.pop();
        self.visited.insert(name.to_string());
        self.packages.insert(
            name.to_string(),
            LockedPackage {
                name: name.to_string(),
                source: materialized.source,
                revision: materialized.revision,
                checksum,
                dependencies: dependency_names,
            },
        );
        Ok(())
    }

    fn materialize(
        &self,
        name: &str,
        spec: &str,
        declaring_root: &Path,
    ) -> Result<MaterializedPackage, String> {
        match parse_dependency_source(spec, declaring_root)? {
            DependencySource::Path(path) => {
                let source = format!("path+{}", relative_path_string(&self.project_root, &path)?);
                Ok(MaterializedPackage {
                    root: path,
                    source,
                    revision: None,
                })
            }
            DependencySource::Git {
                url,
                revision,
                network,
            } => {
                let root = materialize_git_dependency(
                    &self.project_root,
                    name,
                    &url,
                    &revision,
                    network,
                    self.options,
                )?;
                Ok(MaterializedPackage {
                    root,
                    source: format!("git+{url}"),
                    revision: Some(revision),
                })
            }
        }
    }
}

fn validate_lock_transition(
    previous: Option<&[LockedPackage]>,
    resolved: &[LockedPackage],
    options: ResolveOptions,
) -> Result<(), String> {
    let Some(previous) = previous else {
        return Ok(());
    };
    let old = previous
        .iter()
        .map(|package| (package.name.as_str(), package))
        .collect::<BTreeMap<_, _>>();
    let new = resolved
        .iter()
        .map(|package| (package.name.as_str(), package))
        .collect::<BTreeMap<_, _>>();

    if options.locked {
        if old.keys().ne(new.keys()) {
            return Err("locked dependency set differs from oxid.lock".to_string());
        }
        for (name, package) in &new {
            let locked = old[name];
            compare_locked_package(name, locked, package)?;
        }
        return Ok(());
    }

    if !options.update {
        for (name, package) in &new {
            if let Some(locked) = old.get(name) {
                compare_locked_package(name, locked, package)?;
            }
        }
    }
    Ok(())
}

fn compare_locked_package(
    name: &str,
    locked: &LockedPackage,
    resolved: &LockedPackage,
) -> Result<(), String> {
    if locked.source != resolved.source {
        return Err(format!(
            "locked source mismatch for `{name}`: expected {}, found {}; use update mode to accept the change",
            locked.source, resolved.source
        ));
    }
    if locked.revision != resolved.revision {
        return Err(format!(
            "locked revision mismatch for `{name}`: expected {:?}, found {:?}; use update mode to accept the change",
            locked.revision, resolved.revision
        ));
    }
    if locked.checksum != resolved.checksum {
        return Err(format!(
            "checksum mismatch for `{name}`: expected {}, found {}; use update mode only after verifying the source",
            locked.checksum, resolved.checksum
        ));
    }
    if locked.dependencies != resolved.dependencies {
        return Err(format!(
            "locked dependency list mismatch for `{name}`; use update mode to accept the change"
        ));
    }
    Ok(())
}

fn validate_package_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > 64 {
        return Err("dependency names must contain 1 to 64 characters".to_string());
    }
    let mut chars = name.chars();
    let first = chars.next().expect("name is not empty");
    if !first.is_ascii_alphanumeric()
        || !chars
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(format!(
            "invalid dependency name `{name}`; use only ASCII letters, numbers, '-' and '_'"
        ));
    }
    Ok(())
}

fn reject_unsafe_text(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{label} cannot be empty"));
    }
    if value.chars().any(|character| character.is_control()) {
        return Err(format!("{label} contains control characters"));
    }
    let lowercase = value.to_ascii_lowercase();
    if lowercase.contains("%00") || lowercase.contains("%0a") || lowercase.contains("%0d") {
        return Err(format!("{label} contains an encoded control character"));
    }
    Ok(())
}

fn parse_dependency_source(spec: &str, declaring_root: &Path) -> Result<DependencySource, String> {
    reject_unsafe_text(spec, "dependency source")?;
    if spec.starts_with("git+") || spec.starts_with("https://") {
        return parse_git_source(spec);
    }
    if spec.contains("://") {
        return Err(format!("unsupported dependency source scheme: {spec}"));
    }

    let candidate = Path::new(spec);
    let path = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        declaring_root.join(candidate)
    };
    let canonical = fs::canonicalize(&path)
        .map_err(|error| format!("cannot resolve path dependency {}: {error}", path.display()))?;
    if !canonical.is_dir() {
        return Err(format!(
            "path dependency is not a directory: {}",
            canonical.display()
        ));
    }
    Ok(DependencySource::Path(canonical))
}

fn parse_git_source(spec: &str) -> Result<DependencySource, String> {
    let (raw_url, raw_revision) = spec
        .rsplit_once('#')
        .ok_or_else(|| "git dependencies must include a pinned commit after `#`".to_string())?;
    if raw_url.contains('#') || raw_revision.contains('#') {
        return Err("git dependency contains more than one fragment delimiter".to_string());
    }
    let revision = raw_revision.to_ascii_lowercase();
    if !matches!(revision.len(), 40 | 64) || !revision.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(format!(
            "git revision `{raw_revision}` is not a full 40- or 64-character commit hash"
        ));
    }

    let (url, network, require_git_suffix) = if let Some(url) = raw_url.strip_prefix("git+https://")
    {
        (format!("https://{url}"), true, false)
    } else if let Some(url) = raw_url.strip_prefix("https://") {
        (format!("https://{url}"), true, true)
    } else if let Some(url) = raw_url.strip_prefix("git+file://") {
        (format!("file://{url}"), false, false)
    } else {
        return Err(format!(
            "unsupported git source `{raw_url}`; use git+https://, https://...git, or git+file://"
        ));
    };
    reject_unsafe_text(&url, "git URL")?;
    if url.contains('?') || url.contains('#') || url.contains('\\') {
        return Err("git URLs cannot contain queries, fragments, or backslashes".to_string());
    }
    if network {
        validate_https_url(&url)?;
        if require_git_suffix && !url.ends_with(".git") {
            return Err("plain https git dependencies must end in `.git`".to_string());
        }
    } else if !url.starts_with("file:///") && !url.starts_with("file://localhost/") {
        return Err("git+file dependencies must use an absolute local file URL".to_string());
    }

    Ok(DependencySource::Git {
        url,
        revision,
        network,
    })
}

fn validate_https_url(url: &str) -> Result<(), String> {
    let remainder = url
        .strip_prefix("https://")
        .ok_or_else(|| "git network sources must use HTTPS".to_string())?;
    let (authority, path) = remainder
        .split_once('/')
        .ok_or_else(|| "git HTTPS URL must include a repository path".to_string())?;
    if authority.is_empty() || path.is_empty() {
        return Err("git HTTPS URL must include a host and repository path".to_string());
    }
    if authority.contains('@') || authority.chars().any(char::is_whitespace) {
        return Err("git HTTPS URLs cannot contain credentials or whitespace".to_string());
    }
    Ok(())
}

fn relative_path_string(base: &Path, target: &Path) -> Result<String, String> {
    let base_components = base.components().collect::<Vec<_>>();
    let target_components = target.components().collect::<Vec<_>>();
    let mut shared = 0usize;
    while shared < base_components.len()
        && shared < target_components.len()
        && base_components[shared] == target_components[shared]
    {
        shared += 1;
    }

    let same_prefix = match (base_components.first(), target_components.first()) {
        (Some(Component::Prefix(left)), Some(Component::Prefix(right))) => left == right,
        (Some(Component::RootDir), Some(Component::RootDir)) => true,
        (None, None) => true,
        _ => false,
    };
    if !same_prefix {
        return normalized_path_string(target);
    }

    let mut relative = PathBuf::new();
    for _ in shared..base_components.len() {
        relative.push("..");
    }
    for component in &target_components[shared..] {
        relative.push(component.as_os_str());
    }
    if relative.as_os_str().is_empty() {
        return Ok(".".to_string());
    }
    normalized_path_string(&relative)
}

fn normalized_path_string(path: &Path) -> Result<String, String> {
    let mut value = path
        .to_str()
        .map(|value| value.replace('\\', "/"))
        .ok_or_else(|| format!("path is not valid UTF-8: {}", path.display()))?;
    if let Some(rest) = value.strip_prefix("//?/UNC/") {
        value = format!("//{rest}");
    } else if let Some(rest) = value.strip_prefix("//?/") {
        value = rest.to_string();
    }
    Ok(value)
}

fn materialize_git_dependency(
    project_root: &Path,
    name: &str,
    url: &str,
    revision: &str,
    network: bool,
    options: ResolveOptions,
) -> Result<PathBuf, String> {
    let dependency_root = project_root.join(".oxid").join("deps");
    fs::create_dir_all(&dependency_root).map_err(|error| {
        format!(
            "cannot create dependency cache {}: {error}",
            dependency_root.display()
        )
    })?;
    let checkout = dependency_root.join(name);
    let network_forbidden = network && (options.locked || options.offline);

    if checkout.exists() {
        if !checkout.is_dir() {
            return Err(format!(
                "dependency cache is not a directory: {}",
                checkout.display()
            ));
        }
        ensure_git_repository(&checkout)?;
        let origin = git_output(
            &checkout,
            &[
                OsString::from("remote"),
                OsString::from("get-url"),
                OsString::from("origin"),
            ],
        )?;
        if origin.trim() != url {
            return Err(format!(
                "cached dependency `{name}` uses {}, but the manifest requests {url}",
                origin.trim()
            ));
        }
        git_output(
            &checkout,
            &[
                OsString::from("config"),
                OsString::from("core.autocrlf"),
                OsString::from("false"),
            ],
        )?;

        let has_commit = git_commit_exists(&checkout, revision)?;
        if options.update || !has_commit {
            if network_forbidden {
                return Err(format!(
                    "dependency `{name}` commit {revision} is not available without network access"
                ));
            }
            git_fetch(&checkout, url, revision)?;
        }
        git_checkout(&checkout, revision)?;
    } else {
        if network_forbidden {
            return Err(format!(
                "dependency `{name}` is not cached and cannot be fetched in {} mode",
                if options.locked { "locked" } else { "offline" }
            ));
        }
        let temporary = unique_sibling_path(&dependency_root, name)?;
        let result: Result<(), String> = (|| {
            run_git_without_repo(&[
                OsString::from("init"),
                OsString::from("--quiet"),
                external_command_path(&temporary).into_os_string(),
            ])?;
            git_output(
                &temporary,
                &[
                    OsString::from("config"),
                    OsString::from("core.autocrlf"),
                    OsString::from("false"),
                ],
            )?;
            git_output(
                &temporary,
                &[
                    OsString::from("remote"),
                    OsString::from("add"),
                    OsString::from("origin"),
                    OsString::from(url),
                ],
            )?;
            git_fetch(&temporary, url, revision)?;
            git_checkout(&temporary, revision)?;
            fs::rename(&temporary, &checkout).map_err(|error| {
                format!(
                    "cannot publish dependency cache {}: {error}",
                    checkout.display()
                )
            })?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(&temporary);
        }
        result?;
    }

    let head = git_output(
        &checkout,
        &[OsString::from("rev-parse"), OsString::from("HEAD")],
    )?;
    if !head.trim().eq_ignore_ascii_case(revision) {
        return Err(format!(
            "git dependency `{name}` resolved to {}, expected {revision}",
            head.trim()
        ));
    }
    let status = git_output(
        &checkout,
        &[
            OsString::from("status"),
            OsString::from("--porcelain"),
            OsString::from("--untracked-files=all"),
        ],
    )?;
    if !status.trim().is_empty() {
        return Err(format!(
            "cached dependency `{name}` contains modified or untracked files; clear {} before resolving",
            checkout.display()
        ));
    }
    if checkout.join(".gitmodules").exists() {
        return Err(format!(
            "git dependency `{name}` uses unsupported submodules"
        ));
    }
    Ok(checkout)
}

fn unique_sibling_path(parent: &Path, name: &str) -> Result<PathBuf, String> {
    for attempt in 0..1_000u32 {
        let path = parent.join(format!(".{name}.tmp-{}-{attempt}", std::process::id()));
        if !path.exists() {
            return Ok(path);
        }
    }
    Err(format!(
        "cannot allocate a temporary cache path under {}",
        parent.display()
    ))
}

fn ensure_git_repository(path: &Path) -> Result<(), String> {
    let output = git_output(
        path,
        &[
            OsString::from("rev-parse"),
            OsString::from("--is-inside-work-tree"),
        ],
    )?;
    if output.trim() != "true" {
        return Err(format!(
            "dependency cache is not a Git work tree: {}",
            path.display()
        ));
    }
    Ok(())
}

fn git_commit_exists(path: &Path, revision: &str) -> Result<bool, String> {
    let object = format!("{revision}^{{commit}}");
    let output = git_command(
        Some(path),
        &[
            OsString::from("cat-file"),
            OsString::from("-e"),
            OsString::from(object),
        ],
    )
    .output()
    .map_err(|error| format!("cannot launch git: {error}"))?;
    Ok(output.status.success())
}

fn git_fetch(path: &Path, url: &str, revision: &str) -> Result<(), String> {
    git_output(
        path,
        &[
            OsString::from("fetch"),
            OsString::from("--quiet"),
            OsString::from("--depth=1"),
            OsString::from(url),
            OsString::from(revision),
        ],
    )?;
    Ok(())
}

fn git_checkout(path: &Path, revision: &str) -> Result<(), String> {
    git_output(
        path,
        &[
            OsString::from("checkout"),
            OsString::from("--quiet"),
            OsString::from("--detach"),
            OsString::from("--force"),
            OsString::from(revision),
        ],
    )?;
    Ok(())
}

fn git_command(repo: Option<&Path>, args: &[OsString]) -> Command {
    let mut command = Command::new("git");
    command
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", null_device())
        .arg("-c")
        .arg("core.autocrlf=false")
        .arg("-c")
        .arg("protocol.file.allow=always")
        .arg("-c")
        .arg("credential.helper=")
        .arg("-c")
        .arg(format!("core.hooksPath={}", null_device()));
    if let Some(repo) = repo {
        command.arg("-C").arg(external_command_path(repo));
    }
    command.args(args);
    command
}

#[cfg(windows)]
fn null_device() -> &'static str {
    "NUL"
}

#[cfg(not(windows))]
fn null_device() -> &'static str {
    "/dev/null"
}

#[cfg(windows)]
fn external_command_path(path: &Path) -> PathBuf {
    normalized_path_string(path)
        .map(PathBuf::from)
        .unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(not(windows))]
fn external_command_path(path: &Path) -> PathBuf {
    path.to_path_buf()
}

fn git_output(repo: &Path, args: &[OsString]) -> Result<String, String> {
    checked_git_output(git_command(Some(repo), args), args)
}

fn run_git_without_repo(args: &[OsString]) -> Result<String, String> {
    checked_git_output(git_command(None, args), args)
}

fn checked_git_output(mut command: Command, args: &[OsString]) -> Result<String, String> {
    let Output {
        status,
        stdout,
        stderr,
    } = command
        .output()
        .map_err(|error| format!("cannot launch git: {error}"))?;
    if !status.success() {
        let operation = args
            .first()
            .and_then(|arg| arg.to_str())
            .unwrap_or("operation");
        let detail = String::from_utf8_lossy(&stderr).trim().to_string();
        return Err(format!(
            "git {operation} failed with {status}{}",
            if detail.is_empty() {
                String::new()
            } else {
                format!(": {detail}")
            }
        ));
    }
    String::from_utf8(stdout).map_err(|_| "git output was not valid UTF-8".to_string())
}

#[cfg(test)]
fn hash_package_tree(root: &Path) -> Result<String, String> {
    hash_package_tree_with_algorithm(root, ChecksumAlgorithm::Sha256)
}

fn hash_package_tree_with_algorithm(
    root: &Path,
    algorithm: ChecksumAlgorithm,
) -> Result<String, String> {
    let mut entries = Vec::new();
    collect_package_entries(root, root, &mut entries, algorithm)?;
    entries.sort_by(|left, right| left.relative.cmp(&right.relative));
    if matches!(algorithm, ChecksumAlgorithm::Sha256)
        && entries
            .windows(2)
            .any(|pair| pair[0].relative == pair[1].relative)
    {
        return Err("duplicate normalized package paths cannot be hashed with SHA-256".into());
    }

    let mut hasher = PackageHasher::new(algorithm);
    hasher.write_bytes(match algorithm {
        ChecksumAlgorithm::Fnv1a64 => b"oxid-package-fnv1a64-v1",
        ChecksumAlgorithm::Sha256 => b"oxid-package-sha256-v1",
    });
    for entry in entries {
        hasher.write_u8(if entry.directory { b'D' } else { b'F' });
        hasher.write_bytes(entry.relative.as_bytes());
        if !entry.directory {
            let mut file = File::open(&entry.absolute).map_err(|error| {
                format!(
                    "cannot read package file {}: {error}",
                    entry.absolute.display()
                )
            })?;
            let length = file
                .metadata()
                .map_err(|error| format!("cannot inspect {}: {error}", entry.absolute.display()))?
                .len();
            hasher.write_u64(length);
            let mut buffer = [0u8; 16 * 1024];
            let mut remaining = length;
            loop {
                let count = file.read(&mut buffer).map_err(|error| {
                    format!("cannot read {}: {error}", entry.absolute.display())
                })?;
                if count == 0 {
                    break;
                }
                remaining = remaining.checked_sub(count as u64).ok_or_else(|| {
                    format!(
                        "package file changed length while hashing: {}",
                        entry.absolute.display()
                    )
                })?;
                hasher.write(&buffer[..count]);
            }
            if remaining != 0 {
                return Err(format!(
                    "package file changed length while hashing: {}",
                    entry.absolute.display()
                ));
            }
        }
    }
    Ok(hasher.finish())
}

struct PackageEntry {
    relative: String,
    absolute: PathBuf,
    directory: bool,
}

fn collect_package_entries(
    root: &Path,
    directory: &Path,
    entries: &mut Vec<PackageEntry>,
    algorithm: ChecksumAlgorithm,
) -> Result<(), String> {
    let mut children = fs::read_dir(directory)
        .map_err(|error| {
            format!(
                "cannot read package directory {}: {error}",
                directory.display()
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("cannot enumerate {}: {error}", directory.display()))?;
    children.sort_by_key(|entry| entry.file_name());

    for child in children {
        let file_name = child.file_name().into_string().map_err(|_| {
            format!(
                "package path is not valid UTF-8 under {}",
                directory.display()
            )
        })?;
        if matches!(
            file_name.as_str(),
            ".git" | ".oxid" | "target" | LOCKFILE_NAME
        ) {
            continue;
        }
        if matches!(algorithm, ChecksumAlgorithm::Sha256) && file_name.contains('\\') {
            return Err(format!("package entry `{file_name}` contains a backslash; rename it before creating or updating a SHA-256 lock"));
        }
        let path = child.path();
        let file_type = child
            .file_type()
            .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
        if file_type.is_symlink() {
            return Err(format!(
                "symbolic links are not allowed in package content: {}",
                path.display()
            ));
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|_| format!("package entry escaped its root: {}", path.display()))?;
        let relative = normalized_path_string(relative)?;
        if file_type.is_dir() {
            entries.push(PackageEntry {
                relative,
                absolute: path.clone(),
                directory: true,
            });
            collect_package_entries(root, &path, entries, algorithm)?;
        } else if file_type.is_file() {
            entries.push(PackageEntry {
                relative,
                absolute: path,
                directory: false,
            });
        } else {
            return Err(format!(
                "unsupported package entry type: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
enum ChecksumAlgorithm {
    Fnv1a64,
    Sha256,
}

impl ChecksumAlgorithm {
    fn parse(checksum: &str) -> Result<Self, String> {
        let (algorithm, value, length, label) =
            if let Some(value) = checksum.strip_prefix("fnv1a64:") {
                (Self::Fnv1a64, value, 16, "FNV")
            } else if let Some(value) = checksum.strip_prefix("sha256:") {
                (Self::Sha256, value, 64, "SHA-256")
            } else {
                return Err(format!("unsupported package checksum `{checksum}`"));
            };
        if value.len() != length
            || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
            || (matches!(algorithm, Self::Sha256)
                && value.bytes().any(|byte| byte.is_ascii_uppercase()))
        {
            return Err(format!("invalid {label} package checksum `{checksum}`"));
        }
        Ok(algorithm)
    }
}

enum PackageHasher {
    Fnv1a64(u64),
    Sha256(Sha256),
}

impl PackageHasher {
    fn new(algorithm: ChecksumAlgorithm) -> Self {
        match algorithm {
            ChecksumAlgorithm::Fnv1a64 => Self::Fnv1a64(FNV_OFFSET_BASIS),
            ChecksumAlgorithm::Sha256 => Self::Sha256(Sha256::new()),
        }
    }

    fn write(&mut self, bytes: &[u8]) {
        match self {
            Self::Fnv1a64(hash) => {
                for byte in bytes {
                    *hash ^= u64::from(*byte);
                    *hash = hash.wrapping_mul(FNV_PRIME);
                }
            }
            Self::Sha256(hash) => hash.update(bytes),
        }
    }

    fn write_u8(&mut self, value: u8) {
        self.write(&[value]);
    }

    fn write_u64(&mut self, value: u64) {
        self.write(&value.to_le_bytes());
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        self.write_u64(bytes.len() as u64);
        self.write(bytes);
    }

    fn finish(self) -> String {
        match self {
            Self::Fnv1a64(hash) => format!("fnv1a64:{hash:016x}"),
            Self::Sha256(hash) => format!("sha256:{:x}", hash.finalize()),
        }
    }
}

pub fn read_lockfile(path: &Path) -> Result<Vec<LockedPackage>, String> {
    let path = lockfile_path(path);
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("cannot read lockfile {}: {error}", path.display()))?;
    parse_lockfile(&text)
}

pub fn write_lockfile(path: &Path, packages: &[LockedPackage]) -> Result<(), String> {
    let path = lockfile_path(path);
    let mut sorted = packages.to_vec();
    sorted.sort_by(|left, right| left.name.cmp(&right.name));
    let mut seen = HashSet::new();
    for package in &mut sorted {
        validate_package_name(&package.name)?;
        if !seen.insert(package.name.clone()) {
            return Err(format!("duplicate locked package `{}`", package.name));
        }
        validate_checksum(&package.checksum)?;
        package.dependencies.sort();
        package.dependencies.dedup();
        for dependency in &package.dependencies {
            validate_package_name(dependency)?;
        }
    }

    let mut output = format!("version = {LOCKFILE_VERSION}\n");
    for package in sorted {
        output.push_str("\n[[package]]\n");
        output.push_str(&format!("name = {}\n", quote_string(&package.name)));
        output.push_str(&format!("source = {}\n", quote_string(&package.source)));
        output.push_str(&format!(
            "revision = {}\n",
            quote_string(package.revision.as_deref().unwrap_or(""))
        ));
        output.push_str(&format!("checksum = {}\n", quote_string(&package.checksum)));
        output.push_str("dependencies = [");
        for (index, dependency) in package.dependencies.iter().enumerate() {
            if index > 0 {
                output.push_str(", ");
            }
            output.push_str(&quote_string(dependency));
        }
        output.push_str("]\n");
    }
    atomic_write(&path, output.as_bytes())
}

fn lockfile_path(path: &Path) -> PathBuf {
    if path.is_dir() {
        path.join(LOCKFILE_NAME)
    } else {
        path.to_path_buf()
    }
}

#[derive(Default)]
struct LockPackageBuilder {
    name: Option<String>,
    source: Option<String>,
    revision: Option<String>,
    checksum: Option<String>,
    dependencies: Option<Vec<String>>,
}

impl LockPackageBuilder {
    fn finish(self, index: usize) -> Result<LockedPackage, String> {
        let name = self
            .name
            .ok_or_else(|| format!("lock package {index} is missing name"))?;
        validate_package_name(&name)?;
        let source = self
            .source
            .ok_or_else(|| format!("lock package `{name}` is missing source"))?;
        reject_unsafe_text(&source, "locked source")?;
        let revision = self
            .revision
            .ok_or_else(|| format!("lock package `{name}` is missing revision"))?;
        let revision = if revision.is_empty() {
            None
        } else {
            Some(revision)
        };
        let checksum = self
            .checksum
            .ok_or_else(|| format!("lock package `{name}` is missing checksum"))?;
        validate_checksum(&checksum)?;
        let mut dependencies = self
            .dependencies
            .ok_or_else(|| format!("lock package `{name}` is missing dependencies"))?;
        dependencies.sort();
        if dependencies.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(format!(
                "lock package `{name}` contains duplicate dependencies"
            ));
        }
        for dependency in &dependencies {
            validate_package_name(dependency)?;
        }
        Ok(LockedPackage {
            name,
            source,
            revision,
            checksum,
            dependencies,
        })
    }
}

fn parse_lockfile(text: &str) -> Result<Vec<LockedPackage>, String> {
    let mut version = None;
    let mut current = None::<LockPackageBuilder>;
    let mut packages = Vec::new();
    for (line_index, raw_line) in text.lines().enumerate() {
        let line_number = line_index + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "[[package]]" {
            if let Some(builder) = current.take() {
                packages.push(builder.finish(packages.len() + 1)?);
            }
            current = Some(LockPackageBuilder::default());
            continue;
        }
        let (key, raw_value) = line
            .split_once('=')
            .ok_or_else(|| format!("invalid lockfile line {line_number}: expected key = value"))?;
        let key = key.trim();
        let raw_value = raw_value.trim();
        if let Some(builder) = current.as_mut() {
            match key {
                "name" => set_once(
                    &mut builder.name,
                    parse_string(raw_value)?,
                    key,
                    line_number,
                )?,
                "source" => set_once(
                    &mut builder.source,
                    parse_string(raw_value)?,
                    key,
                    line_number,
                )?,
                "revision" => set_once(
                    &mut builder.revision,
                    parse_string(raw_value)?,
                    key,
                    line_number,
                )?,
                "checksum" => set_once(
                    &mut builder.checksum,
                    parse_string(raw_value)?,
                    key,
                    line_number,
                )?,
                "dependencies" => set_once(
                    &mut builder.dependencies,
                    parse_string_array(raw_value)?,
                    key,
                    line_number,
                )?,
                _ => {
                    return Err(format!(
                        "unknown lock package field `{key}` on line {line_number}"
                    ))
                }
            }
        } else if key == "version" {
            if version.is_some() {
                return Err("lockfile contains multiple version fields".to_string());
            }
            version = Some(
                raw_value
                    .parse::<u32>()
                    .map_err(|_| format!("invalid lockfile version on line {line_number}"))?,
            );
        } else {
            return Err(format!(
                "unknown lockfile field `{key}` on line {line_number}"
            ));
        }
    }
    if let Some(builder) = current {
        packages.push(builder.finish(packages.len() + 1)?);
    }
    match version {
        Some(LOCKFILE_VERSION) => {}
        Some(found) => {
            return Err(format!(
                "unsupported oxid.lock version {found}; expected {LOCKFILE_VERSION}"
            ))
        }
        None => return Err("oxid.lock is missing version = 1".to_string()),
    }
    packages.sort_by(|left, right| left.name.cmp(&right.name));
    if packages.windows(2).any(|pair| pair[0].name == pair[1].name) {
        return Err("oxid.lock contains duplicate package names".to_string());
    }
    Ok(packages)
}

fn set_once<T>(
    slot: &mut Option<T>,
    value: T,
    key: &str,
    line_number: usize,
) -> Result<(), String> {
    if slot.replace(value).is_some() {
        return Err(format!("duplicate `{key}` on lockfile line {line_number}"));
    }
    Ok(())
}

fn validate_checksum(checksum: &str) -> Result<(), String> {
    ChecksumAlgorithm::parse(checksum).map(|_| ())
}

fn quote_string(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 2);
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                output.push_str(&format!("\\u{:04x}", u32::from(character)))
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

fn parse_string(raw: &str) -> Result<String, String> {
    let bytes = raw.as_bytes();
    if bytes.len() < 2 || bytes[0] != b'"' || bytes[bytes.len() - 1] != b'"' {
        return Err(format!("expected quoted string, found `{raw}`"));
    }
    let mut output = String::new();
    let mut chars = raw[1..raw.len() - 1].chars();
    while let Some(character) = chars.next() {
        if character != '\\' {
            output.push(character);
            continue;
        }
        let escaped = chars
            .next()
            .ok_or_else(|| "unterminated string escape".to_string())?;
        match escaped {
            '"' => output.push('"'),
            '\\' => output.push('\\'),
            'n' => output.push('\n'),
            'r' => output.push('\r'),
            't' => output.push('\t'),
            'u' => {
                let digits = chars.by_ref().take(4).collect::<String>();
                if digits.len() != 4 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                    return Err("invalid Unicode escape in string".to_string());
                }
                let value = u32::from_str_radix(&digits, 16)
                    .map_err(|_| "invalid Unicode escape in string".to_string())?;
                let character = char::from_u32(value)
                    .ok_or_else(|| "invalid Unicode scalar in string".to_string())?;
                output.push(character);
            }
            other => return Err(format!("unsupported string escape `\\{other}`")),
        }
    }
    Ok(output)
}

fn parse_string_array(raw: &str) -> Result<Vec<String>, String> {
    let raw = raw.trim();
    if !raw.starts_with('[') || !raw.ends_with(']') {
        return Err(format!("expected string array, found `{raw}`"));
    }
    let inner = &raw[1..raw.len() - 1];
    let mut values = Vec::new();
    let mut index = 0usize;
    while index < inner.len() {
        while index < inner.len() && inner.as_bytes()[index].is_ascii_whitespace() {
            index += 1;
        }
        if index == inner.len() {
            break;
        }
        if inner.as_bytes()[index] != b'"' {
            return Err("lockfile dependency arrays accept only quoted strings".to_string());
        }
        let start = index;
        index += 1;
        let mut escaped = false;
        while index < inner.len() {
            let byte = inner.as_bytes()[index];
            index += 1;
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                break;
            }
        }
        if index > inner.len() || inner.as_bytes().get(index.saturating_sub(1)) != Some(&b'"') {
            return Err("unterminated string in dependency array".to_string());
        }
        values.push(parse_string(&inner[start..index])?);
        while index < inner.len() && inner.as_bytes()[index].is_ascii_whitespace() {
            index += 1;
        }
        if index == inner.len() {
            break;
        }
        if inner.as_bytes()[index] != b',' {
            return Err("dependency array entries must be separated by commas".to_string());
        }
        index += 1;
    }
    Ok(values)
}

pub fn list_dependencies(manifest: &Path) -> Result<Vec<(String, String)>, String> {
    let manifest = manifest_path(manifest);
    let text = fs::read_to_string(&manifest)
        .map_err(|error| format!("cannot read manifest {}: {error}", manifest.display()))?;
    let mut in_dependencies = false;
    let mut dependencies = BTreeMap::new();
    for (line_index, raw_line) in text.lines().enumerate() {
        let line_number = line_index + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            in_dependencies = line == "[dependencies]";
            continue;
        }
        if !in_dependencies {
            continue;
        }
        let (raw_key, raw_value) = line
            .split_once('=')
            .ok_or_else(|| format!("invalid dependency entry on line {line_number}"))?;
        let name = parse_manifest_key(raw_key.trim())?;
        validate_package_name(&name)?;
        let source = parse_manifest_value(raw_value.trim())?;
        reject_unsafe_text(&source, "dependency source")?;
        if dependencies.insert(name.clone(), source).is_some() {
            return Err(format!(
                "duplicate dependency `{name}` in {}",
                manifest.display()
            ));
        }
    }
    Ok(dependencies.into_iter().collect())
}

pub fn remove_dependency_from_manifest(manifest: &Path, name: &str) -> Result<bool, String> {
    validate_package_name(name)?;
    let manifest = manifest_path(manifest);
    let text = fs::read_to_string(&manifest)
        .map_err(|error| format!("cannot read manifest {}: {error}", manifest.display()))?;
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let ended_with_newline = text.ends_with('\n');
    let mut in_dependencies = false;
    let mut removed = false;
    let mut output = Vec::new();
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_dependencies = line == "[dependencies]";
            output.push(raw_line.to_string());
            continue;
        }
        if in_dependencies {
            if let Some((raw_key, _)) = line.split_once('=') {
                if parse_manifest_key(raw_key.trim()).as_deref() == Ok(name) {
                    removed = true;
                    continue;
                }
            }
        }
        output.push(raw_line.to_string());
    }
    if !removed {
        return Ok(false);
    }
    let mut rendered = output.join(newline);
    if ended_with_newline {
        rendered.push_str(newline);
    }
    atomic_write(&manifest, rendered.as_bytes())?;
    Ok(true)
}

fn manifest_path(path: &Path) -> PathBuf {
    if path.is_dir() {
        path.join("oxid.toml")
    } else {
        path.to_path_buf()
    }
}

fn parse_manifest_key(raw: &str) -> Result<String, String> {
    if raw.starts_with('"') {
        parse_string(raw)
    } else {
        Ok(raw.to_string())
    }
}

fn parse_manifest_value(raw: &str) -> Result<String, String> {
    if raw.starts_with('"') {
        let mut index = 1usize;
        let bytes = raw.as_bytes();
        let mut escaped = false;
        while index < bytes.len() {
            let byte = bytes[index];
            index += 1;
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                let suffix = raw[index..].trim();
                if !suffix.is_empty() && !suffix.starts_with('#') {
                    return Err(format!("unexpected text after dependency value: {suffix}"));
                }
                return parse_string(&raw[..index]);
            }
        }
        Err("unterminated quoted dependency value".to_string())
    } else {
        let value = raw.split('#').next().unwrap_or("").trim();
        if value.is_empty() {
            Err("dependency value cannot be empty".to_string())
        } else {
            Ok(value.to_string())
        }
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if fs::read(path).ok().as_deref() == Some(bytes) {
        return Ok(());
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    let (temporary, mut file) = create_temporary_file(
        parent,
        path.file_name().unwrap_or_else(|| OsStr::new("oxid")),
    )?;
    let result = (|| {
        file.write_all(bytes)
            .map_err(|error| format!("cannot write {}: {error}", temporary.display()))?;
        file.sync_all()
            .map_err(|error| format!("cannot sync {}: {error}", temporary.display()))?;
        drop(file);
        replace_file(&temporary, path)
            .map_err(|error| format!("cannot replace {}: {error}", path.display()))?;
        sync_parent_directory(parent)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn create_temporary_file(parent: &Path, name: &OsStr) -> Result<(PathBuf, File), String> {
    let name = name.to_string_lossy();
    for attempt in 0..1_000u32 {
        let path = parent.join(format!(".{name}.tmp-{}-{attempt}", std::process::id()));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "cannot create temporary file {}: {error}",
                    path.display()
                ))
            }
        }
    }
    Err(format!(
        "cannot allocate a temporary file under {}",
        parent.display()
    ))
}

#[cfg(not(windows))]
fn replace_file(temporary: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(temporary, destination)
}

#[cfg(windows)]
fn replace_file(temporary: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;

    #[link(name = "Kernel32")]
    extern "system" {
        fn MoveFileExW(existing: *const u16, replacement: *const u16, flags: u32) -> i32;
    }

    let existing = temporary
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let replacement = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let result = unsafe {
        MoveFileExW(
            existing.as_ptr(),
            replacement.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(unix)]
fn sync_parent_directory(parent: &Path) -> Result<(), String> {
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("cannot sync directory {}: {error}", parent.display()))
}

#[cfg(not(unix))]
fn sync_parent_directory(_parent: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "oxid-packages-{label}-{}-{nonce}-{sequence}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create test directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write(path: impl AsRef<Path>, contents: &str) {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(path, contents).expect("write fixture");
    }

    #[test]
    fn sha256_tree_known_answer_and_ordering() {
        // Independently computed with Python hashlib over the RFC's exact bytes.
        let first = TestDir::new("sha-vector-a");
        let second = TestDir::new("sha-vector-b");
        fs::write(first.path().join("data.bin"), [0, 255, 65]).unwrap();
        fs::create_dir(first.path().join("empty")).unwrap();
        fs::create_dir(second.path().join("empty")).unwrap();
        fs::write(second.path().join("data.bin"), [0, 255, 65]).unwrap();
        let expected = "sha256:70b7e1ffaa103e8f0e161878a0a53306cf31ea63163c54d67ae141f04aa89c72";
        assert_eq!(hash_package_tree(first.path()).unwrap(), expected);
        assert_eq!(hash_package_tree(second.path()).unwrap(), expected);
        write(first.path().join("target/cache"), "ignored");
        write(first.path().join(".git/config"), "ignored");
        write(first.path().join(".oxid/cache"), "ignored");
        write(first.path().join("oxid.lock"), "ignored");
        assert_eq!(hash_package_tree(first.path()).unwrap(), expected);
        fs::remove_dir(first.path().join("empty")).unwrap();
        assert_ne!(hash_package_tree(first.path()).unwrap(), expected);
        fs::create_dir(first.path().join("empty")).unwrap();
        fs::rename(
            first.path().join("data.bin"),
            first.path().join("renamed.bin"),
        )
        .unwrap();
        assert_ne!(hash_package_tree(first.path()).unwrap(), expected);
        fs::rename(
            first.path().join("renamed.bin"),
            first.path().join("data.bin"),
        )
        .unwrap();
        fs::write(first.path().join("data.bin"), [0, 255, 66]).unwrap();
        assert_ne!(hash_package_tree(first.path()).unwrap(), expected);
    }

    #[test]
    fn legacy_lock_preserved_until_explicit_update() {
        let root = TestDir::new("legacy-migration");
        write(root.path().join("dep/value.ox"), "const value = 1;\n");
        let dependencies = HashMap::from([("dep".to_string(), "dep".to_string())]);
        let legacy = LockedPackage {
            name: "dep".into(),
            source: "path+dep".into(),
            revision: None,
            checksum: "fnv1a64:137966a53d847cbc".into(),
            dependencies: vec![],
        };
        write_lockfile(root.path(), std::slice::from_ref(&legacy)).unwrap();
        let bytes = fs::read(root.path().join(LOCKFILE_NAME)).unwrap();
        for options in [
            ResolveOptions::default(),
            ResolveOptions {
                offline: true,
                ..Default::default()
            },
            ResolveOptions {
                locked: true,
                offline: true,
                update: false,
            },
        ] {
            assert_eq!(
                resolve_dependencies(root.path(), &dependencies, options).unwrap(),
                std::slice::from_ref(&legacy)
            );
            assert_eq!(fs::read(root.path().join(LOCKFILE_NAME)).unwrap(), bytes);
        }
        let updated = resolve_dependencies(
            root.path(),
            &dependencies,
            ResolveOptions {
                update: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(updated[0].checksum.starts_with("sha256:"));
        assert_eq!(
            resolve_dependencies(
                root.path(),
                &dependencies,
                ResolveOptions {
                    locked: true,
                    ..Default::default()
                }
            )
            .unwrap(),
            updated
        );
    }

    #[test]
    fn offline_without_lock_keeps_local_resolution_behavior() {
        let root = TestDir::new("offline-new-lock");
        write(root.path().join("dep/value.ox"), "1");
        let dependencies = HashMap::from([("dep".into(), "dep".into())]);
        let packages = resolve_dependencies(
            root.path(),
            &dependencies,
            ResolveOptions {
                offline: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(root.path().join(LOCKFILE_NAME).is_file());
        assert_eq!(read_lockfile(root.path()).unwrap(), packages);
    }

    #[test]
    fn checksum_algorithm_validation_is_closed() {
        for checksum in [
            "md5:abcd".to_string(),
            "sha256:".to_string(),
            format!("sha256:{}", "a".repeat(63)),
            format!("sha256:{}", "g".repeat(64)),
            format!("sha256:{}", "A".repeat(64)),
        ] {
            assert!(validate_checksum(&checksum).is_err(), "{checksum}");
        }
        assert!(validate_checksum(&format!("sha256:{}", "a".repeat(64))).is_ok());
        assert!(validate_checksum("fnv1a64:ABCDEF0123456789").is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn sha256_rejects_ambiguous_unix_path_components_but_preserves_fnv() {
        let first = TestDir::new("ambiguous-name");
        let second = TestDir::new("nested-name");
        fs::create_dir(first.path().join("a")).unwrap();
        fs::write(first.path().join("a\\b"), b"same").unwrap();
        fs::create_dir(second.path().join("a")).unwrap();
        fs::write(second.path().join("a/b"), b"same").unwrap();
        assert_eq!(
            hash_package_tree_with_algorithm(first.path(), ChecksumAlgorithm::Fnv1a64).unwrap(),
            hash_package_tree_with_algorithm(second.path(), ChecksumAlgorithm::Fnv1a64).unwrap()
        );
        let error = hash_package_tree(first.path()).unwrap_err();
        assert!(error.contains("rename"), "{error}");
        assert!(hash_package_tree(second.path()).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn sha256_rejects_included_symlinks() {
        use std::os::unix::fs::symlink;
        let root = TestDir::new("sha-symlink-rejection");
        write(root.path().join("file"), "data");
        symlink("file", root.path().join("link")).unwrap();
        assert!(hash_package_tree(root.path())
            .unwrap_err()
            .contains("symbolic links"));
    }

    // macOS CI's filesystem rejects this name with EILSEQ before Oxid reads it.
    // Keep raw-byte filename coverage on Linux; fixture I/O failures still fail.
    #[cfg(target_os = "linux")]
    #[test]
    fn sha256_rejects_non_utf8_names_on_linux() {
        use std::os::unix::ffi::OsStringExt;
        let root = TestDir::new("sha-non-utf8-rejection");
        fs::write(root.path().join(OsString::from_vec(vec![255])), b"data").unwrap();
        assert!(hash_package_tree(root.path())
            .unwrap_err()
            .contains("UTF-8"));
    }

    #[test]
    fn path_dependencies_are_recursive_sorted_and_locked() {
        let root = TestDir::new("paths");
        let alpha = root.path().join("deps/alpha");
        let leaf = root.path().join("deps/leaf");
        write(
            alpha.join("oxid.toml"),
            "[project]\nname = \"alpha\"\n[dependencies]\nleaf = \"../leaf\"\n",
        );
        write(alpha.join("src/lib.ox"), "fn alpha() => 1;\n");
        write(leaf.join("src/lib.ox"), "fn leaf() => 2;\n");

        let dependencies = HashMap::from([("alpha".to_string(), "deps/alpha".to_string())]);
        let packages = resolve_dependencies(root.path(), &dependencies, ResolveOptions::default())
            .expect("resolve paths");
        assert_eq!(
            packages
                .iter()
                .map(|package| package.name.as_str())
                .collect::<Vec<_>>(),
            ["alpha", "leaf"]
        );
        assert_eq!(packages[0].dependencies, ["leaf"]);
        assert!(packages
            .iter()
            .all(|package| package.checksum.starts_with("sha256:")));

        let lock_text = fs::read_to_string(root.path().join(LOCKFILE_NAME)).expect("lockfile");
        assert_eq!(lock_text.matches("[[package]]").count(), 2);
        assert!(lock_text.find("name = \"alpha\"") < lock_text.find("name = \"leaf\""));

        let locked = resolve_dependencies(
            root.path(),
            &dependencies,
            ResolveOptions {
                locked: true,
                offline: true,
                update: false,
            },
        )
        .expect("locked path resolution");
        assert_eq!(packages, locked);
    }

    #[test]
    fn checksum_changes_require_explicit_update() {
        let root = TestDir::new("checksum");
        write(root.path().join("dep/value.ox"), "const value = 1;\n");
        let dependencies = HashMap::from([("dep".to_string(), "dep".to_string())]);
        let first = resolve_dependencies(root.path(), &dependencies, ResolveOptions::default())
            .expect("first resolution");
        write(root.path().join("dep/value.ox"), "const value = 2;\n");

        let error = resolve_dependencies(root.path(), &dependencies, ResolveOptions::default())
            .expect_err("checksum change must fail");
        assert!(error.contains("checksum mismatch"), "{error}");

        let updated = resolve_dependencies(
            root.path(),
            &dependencies,
            ResolveOptions {
                update: true,
                ..ResolveOptions::default()
            },
        )
        .expect("explicit update");
        assert_ne!(first[0].checksum, updated[0].checksum);
    }

    #[test]
    fn declared_cycles_are_rejected() {
        let root = TestDir::new("cycle");
        write(
            root.path().join("a/oxid.toml"),
            "[dependencies]\nb = \"../b\"\n",
        );
        write(
            root.path().join("b/oxid.toml"),
            "[dependencies]\na = \"../a\"\n",
        );
        let dependencies = HashMap::from([("a".to_string(), "a".to_string())]);
        let error = resolve_dependencies(root.path(), &dependencies, ResolveOptions::default())
            .expect_err("cycle must fail");
        assert!(error.contains("a -> b -> a"), "{error}");
    }

    #[test]
    fn manifest_dependencies_can_be_listed_and_removed() {
        let root = TestDir::new("manifest");
        let manifest = root.path().join("oxid.toml");
        write(
            &manifest,
            "[project]\nname = \"demo\"\n\n[dependencies]\nzeta = \"./z\"\nalpha = \"https://example.test/a.git#0123456789012345678901234567890123456789\" # pinned\n\n[features]\nasync = true\n",
        );
        let dependencies = list_dependencies(&manifest).expect("list dependencies");
        assert_eq!(
            dependencies[0],
            (
                "alpha".to_string(),
                "https://example.test/a.git#0123456789012345678901234567890123456789".to_string()
            )
        );
        assert_eq!(dependencies[1], ("zeta".to_string(), "./z".to_string()));
        assert!(remove_dependency_from_manifest(&manifest, "zeta").expect("remove dependency"));
        assert!(
            !remove_dependency_from_manifest(&manifest, "zeta").expect("remove missing dependency")
        );
        let remaining = fs::read_to_string(&manifest).expect("read manifest");
        assert!(!remaining.contains("zeta ="));
        assert!(remaining.contains("[features]"));
    }

    #[test]
    fn lockfile_round_trip_is_deterministic() {
        let root = TestDir::new("lock-roundtrip");
        let packages = vec![
            LockedPackage {
                name: "zeta".to_string(),
                source: "path+deps/zeta".to_string(),
                revision: None,
                checksum: "fnv1a64:0123456789abcdef".to_string(),
                dependencies: vec![],
            },
            LockedPackage {
                name: "alpha".to_string(),
                source: "git+https://example.test/alpha.git".to_string(),
                revision: Some("0123456789012345678901234567890123456789".to_string()),
                checksum: "fnv1a64:fedcba9876543210".to_string(),
                dependencies: vec!["zeta".to_string()],
            },
        ];
        write_lockfile(root.path(), &packages).expect("write lockfile");
        let first = fs::read(root.path().join(LOCKFILE_NAME)).expect("first bytes");
        let parsed = read_lockfile(root.path()).expect("parse lockfile");
        write_lockfile(root.path(), &parsed).expect("rewrite lockfile");
        let second = fs::read(root.path().join(LOCKFILE_NAME)).expect("second bytes");
        assert_eq!(first, second);
        assert_eq!(parsed[0].name, "alpha");
    }

    #[test]
    fn unsafe_names_and_sources_are_rejected() {
        let root = TestDir::new("unsafe");
        write(root.path().join("dep/file.ox"), "fn ok() => 1;\n");
        let unsafe_name = HashMap::from([("../escape".to_string(), "dep".to_string())]);
        assert!(
            resolve_dependencies(root.path(), &unsafe_name, ResolveOptions::default()).is_err()
        );
        let unsafe_scheme = HashMap::from([(
            "dep".to_string(),
            "http://example.test/dep.git#0123456789012345678901234567890123456789".to_string(),
        )]);
        assert!(
            resolve_dependencies(root.path(), &unsafe_scheme, ResolveOptions::default()).is_err()
        );
        let symbolic_revision = HashMap::from([(
            "dep".to_string(),
            "git+https://example.test/dep.git#main".to_string(),
        )]);
        assert!(
            resolve_dependencies(root.path(), &symbolic_revision, ResolveOptions::default())
                .is_err()
        );
    }

    #[test]
    fn offline_https_never_attempts_an_uncached_fetch() {
        let root = TestDir::new("offline-network");
        let dependencies = HashMap::from([(
            "remote".to_string(),
            "git+https://example.invalid/repository#0123456789012345678901234567890123456789"
                .to_string(),
        )]);
        let error = resolve_dependencies(
            root.path(),
            &dependencies,
            ResolveOptions {
                locked: false,
                offline: true,
                update: false,
            },
        )
        .expect_err("offline resolution must reject an uncached network dependency");
        assert!(
            error.contains("cannot be fetched in offline mode"),
            "{error}"
        );
    }

    #[test]
    fn local_git_dependency_is_pinned_and_works_locked_offline() {
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }
        let root = TestDir::new("git");
        let repository = root.path().join("repository");
        fs::create_dir_all(&repository).expect("repository directory");
        test_git(&repository, &["init", "--quiet"]);
        test_git(
            &repository,
            &["config", "user.email", "oxid@example.invalid"],
        );
        test_git(&repository, &["config", "user.name", "Oxid Test"]);
        write(repository.join("src/lib.ox"), "fn value() => 42;\n");
        test_git(&repository, &["add", "."]);
        test_git(&repository, &["commit", "--quiet", "-m", "fixture"]);
        let revision = test_git(&repository, &["rev-parse", "HEAD"]);
        let repository_path =
            normalized_path_string(&fs::canonicalize(&repository).expect("canonical repo"))
                .expect("UTF-8 repo");
        let url = if cfg!(windows) {
            format!(
                "git+file:///{}#{}",
                repository_path.trim_start_matches('/'),
                revision.trim()
            )
        } else {
            format!("git+file://{}#{}", repository_path, revision.trim())
        };
        let dependencies = HashMap::from([("fixture".to_string(), url)]);
        let first = resolve_dependencies(root.path(), &dependencies, ResolveOptions::default())
            .expect("resolve local git");
        assert_eq!(first[0].revision.as_deref(), Some(revision.trim()));
        fs::remove_dir_all(&repository).expect("remove origin fixture");

        let locked = resolve_dependencies(
            root.path(),
            &dependencies,
            ResolveOptions {
                locked: true,
                offline: true,
                update: false,
            },
        )
        .expect("cached locked offline resolution");
        assert_eq!(first, locked);
    }

    fn test_git(repository: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(repository)
            .args(args)
            .output()
            .expect("launch git");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("git UTF-8 output")
    }
}
