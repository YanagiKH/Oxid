//! Explicit virtual-workspace selection, before any source bodies are loaded.
use super::*;
use std::path::PathBuf;

pub(super) struct Cached {
    pub package: Option<Package>,
    // Retain the exact bytes validated for every member through graph resolution.
    // These exact-sized copies share the aggregate input bound, including the root.
    _raw: Vec<u8>,
}
pub(super) struct Selected {
    pub root: Package,
    pub cached: Vec<Cached>,
    pub workspace: Option<PathBuf>,
}

fn gap(scan: &mut Scanner<'_>, path: &str) -> Result<(), Failure> {
    loop {
        scan.horizontal()?;
        if matches!(scan.peek(), Some('#' | '\n' | '\r')) {
            scan.end_line(path)?;
        } else {
            return Ok(());
        }
    }
}

fn header<'a>(path: &str, text: &'a str) -> Result<Option<Scanner<'a>>, Failure> {
    let mut scan = Scanner::new(text);
    gap(&mut scan, path)?;
    if scan.next()? != Some('[') {
        return Ok(None);
    }
    scan.horizontal()?;
    if scan.identifier()? != "workspace" {
        return Ok(None);
    }
    scan.horizontal()?;
    if scan.next()? != Some(']') {
        return Err(scan.syntax(path, "invalid workspace table header"));
    }
    scan.end_line(path)?;
    Ok(Some(scan))
}

fn members(
    path: &str,
    mut scan: Scanner<'_>,
    budget: &mut Budget,
    allocator: &mut Allocator,
) -> Result<Vec<Field>, Failure> {
    let mut seen = false;
    let mut fields = Vec::new();
    loop {
        gap(&mut scan, path)?;
        if scan.peek().is_none() {
            break;
        }
        if scan.peek() == Some('[') {
            return Err(scan.syntax(path, "virtual workspace permits exactly one [workspace] table; project and dependencies cannot be combined with it"));
        }
        if scan.identifier()? != "members" {
            return Err(scan.syntax(path, "workspace supports only the members key"));
        }
        if seen {
            return Err(scan.syntax(path, "duplicate workspace.members"));
        }
        seen = true;
        scan.horizontal()?;
        if scan.next()? != Some('=') {
            return Err(scan.syntax(path, "expected = after workspace.members"));
        }
        scan.horizontal()?;
        if scan.next()? != Some('[') {
            return Err(scan.syntax(path, "workspace.members requires a string array"));
        }
        gap(&mut scan, path)?;
        while scan.peek() != Some(']') {
            let field = scan.string(path)?;
            if field.decoded == 0 || field.decoded > D_LIMITS[1] {
                return Err(scan.syntax(
                    path,
                    "workspace member directory must contain 1..2048 decoded bytes",
                ));
            }
            if fields.len() >= ProjectLimits::default().modules {
                return Err(Failure::Resource(
                    "workspace member count exceeds module limit",
                ));
            }
            budget.edges(add(size_of::<Field>(), field.decoded)?)?;
            reserve(&mut fields, allocator)?;
            fields.push(field);
            gap(&mut scan, path)?;
            match scan.peek() {
                Some(']') => break,
                Some(',') => {
                    scan.next()?;
                    gap(&mut scan, path)?;
                }
                _ => return Err(scan.syntax(path, "expected comma or ] in workspace.members")),
            }
        }
        scan.next()?;
        scan.end_line(path)?;
    }
    if fields.is_empty() {
        return Err(scan.syntax(path, "workspace requires at least one explicit member"));
    }
    Ok(fields)
}

pub(super) fn retain(
    cache: &mut Vec<Cached>,
    package: Option<Package>,
    raw: &[u8],
    budget: &mut Budget,
    allocator: &mut Allocator,
) -> Result<(), Failure> {
    if cache.len() >= ProjectLimits::default().modules {
        return Err(Failure::Resource(
            "workspace manifest snapshot count exceeds module limit",
        ));
    }
    budget.edges(size_of::<Cached>())?;
    reserve(cache, allocator)?;
    let mut copy = allocator.vector(raw.len())?;
    copy.extend_from_slice(raw);
    cache.push(Cached {
        package,
        _raw: copy,
    });
    Ok(())
}

fn available(cache: &[Cached], allocator: &mut Allocator) -> Result<String, Failure> {
    let length = cache.iter().try_fold(0, |sum, item| {
        add(
            sum,
            add(
                item.package.as_ref().ok_or(Failure::Invariant)?.name.len(),
                2,
            )?,
        )
    })?;
    let mut names = allocator.string(length)?;
    for item in cache {
        if !names.is_empty() {
            names.push_str(", ");
        }
        names.push_str(&item.package.as_ref().ok_or(Failure::Invariant)?.name);
    }
    Ok(names)
}

// The catalog and ordinary selection share this exact parser/allocation path.
// Keep all Budget charges before the corresponding retained allocations.
fn read_members(
    path: &str,
    raw: &[u8],
    identity: &Path,
    scan: Scanner<'_>,
    operation: Command,
    budget: &mut Budget,
    allocator: &mut Allocator,
) -> Result<Vec<Cached>, Failure> {
    let text = std::str::from_utf8(raw)
        .map_err(|_| Failure::Utf8(format!("manifest `{path}` is not valid UTF-8")))?;
    budget.manifest(raw.len())?;
    budget.paths(add(
        path.len(),
        identity.as_os_str().as_encoded_bytes().len(),
    )?)?;
    let fields = members(path, scan, budget, allocator)?;
    let mut cache: Vec<Cached> = Vec::new();
    for field in fields {
        visit(text, field, |c| {
            if c.is_control() || matches!(c, '\\' | '*' | '?' | '[' | ']') {
                Err(Failure::Cli(
                    "workspace member paths do not support controls, backslashes or globs".into(),
                ))
            } else {
                Ok(())
            }
        })?;
        budget.paths(field.decoded)?;
        let directory = decoded(text, field, allocator)?;
        let bytes = directory.as_bytes();
        if Path::new(&directory).is_absolute()
            || (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
        {
            return Err(Failure::Cli(
                "workspace member directories must be relative to the workspace manifest".into(),
            ));
        }
        let member = joined_manifest(path, &directory)?;
        let member = member.to_str().ok_or(Failure::Invariant)?;
        let canonical_member = canonical(member)?;
        if canonical_member == identity {
            return Err(Failure::Cli(format!(
                "manifest `{path}`: workspace membership cycle"
            )));
        }
        if cache.iter().any(|s| {
            s.package
                .as_ref()
                .is_some_and(|p| p.canonical_manifest == canonical_member)
        }) {
            return Err(Failure::Cli(format!(
                "manifest `{path}`: duplicate canonical workspace member `{directory}`"
            )));
        }
        let raw = read_path(
            member,
            allocator,
            M_RAW
                .checked_sub(budget.manifest_bytes)
                .ok_or_else(overflow)?,
        )?;
        let member_text = std::str::from_utf8(&raw)
            .map_err(|_| Failure::Utf8(format!("manifest `{member}` is not valid UTF-8")))?;
        if header(member, member_text)?.is_some() {
            return Err(Failure::Cli(format!(
                "manifest `{member}`: nested workspaces are not supported"
            )));
        }
        let member_package = package(
            member,
            canonical_member,
            &raw,
            operation,
            true,
            budget,
            allocator,
        )
        .map_err(|failure| match failure {
            Failure::Cli(message) if !message.starts_with("manifest `") => {
                Failure::Cli(format!("manifest `{member}`: {message}"))
            }
            failure => failure,
        })?;
        if cache.iter().any(|s| {
            s.package
                .as_ref()
                .is_some_and(|p| p.name == member_package.name)
        }) {
            return Err(Failure::Cli(format!(
                "manifest `{path}`: duplicate workspace member name `{}`",
                member_package.name
            )));
        }
        retain(&mut cache, Some(member_package), &raw, budget, allocator)?;
    }
    Ok(cache)
}

/// Validate every declared manifest without choosing a root or loading sources.
pub(super) fn enumerate(
    path: &str,
    raw: &[u8],
    selector: Option<&str>,
    operation: Command,
    budget: &mut Budget,
    allocator: &mut Allocator,
) -> Result<Vec<Cached>, Failure> {
    if selector.is_some() {
        return Err(Failure::Cli("--workspace conflicts with --package".into()));
    }
    let identity = canonical(path)?;
    let text = std::str::from_utf8(raw)
        .map_err(|_| Failure::Utf8(format!("manifest `{path}` is not valid UTF-8")))?;
    let scan = header(path, text)?
        .ok_or_else(|| Failure::Cli("--workspace requires a virtual workspace manifest".into()))?;
    read_members(path, raw, &identity, scan, operation, budget, allocator)
}

pub(super) fn select(
    path: &str,
    raw: &[u8],
    selector: Option<&str>,
    operation: Command,
    needs_name: bool,
    budget: &mut Budget,
    allocator: &mut Allocator,
) -> Result<Selected, Failure> {
    let identity = canonical(path)?;
    let text = std::str::from_utf8(raw)
        .map_err(|_| Failure::Utf8(format!("manifest `{path}` is not valid UTF-8")))?;
    let Some(scan) = header(path, text)? else {
        if selector.is_some() {
            return Err(Failure::Cli("--package requires a virtual workspace manifest; direct project manifests do not accept it".into()));
        }
        return Ok(Selected {
            root: package(
                path, identity, raw, operation, needs_name, budget, allocator,
            )?,
            cached: Vec::new(),
            workspace: None,
        });
    };
    let mut cache = read_members(path, raw, &identity, scan, operation, budget, allocator)?;
    let index = match selector {
        Some(name) => match cache
            .iter()
            .position(|s| s.package.as_ref().is_some_and(|p| p.name == name))
        {
            Some(index) => index,
            None => {
                return Err(Failure::Cli(format!(
                    "unknown workspace package `{name}`; available packages: {}",
                    available(&cache, allocator)?
                )))
            }
        },
        None if cache.len() == 1 => 0,
        None => {
            return Err(Failure::Cli(format!(
                "workspace requires --package; available packages: {}",
                available(&cache, allocator)?
            )))
        }
    };
    Ok(Selected {
        root: cache[index].package.take().ok_or(Failure::Invariant)?,
        cached: cache,
        workspace: Some(identity),
    })
}

#[cfg(test)]
mod tests;
