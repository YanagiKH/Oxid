//! Qualified Linux stable-filesystem policy, not a TOCTOU sandbox.
use super::{
    add, budget::Allocator, io_error, policy, resource, Diagnostic, ProjectLimits, SourceUsage,
    Span,
};
use std::{
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
};

pub(super) fn qualified_host(origin: Span) -> Result<(), Box<Diagnostic>> {
    if cfg!(target_os = "linux") {
        Ok(())
    } else {
        Err(policy(
            "module source policy is not qualified on this host",
            origin,
        ))
    }
}

pub(super) fn path_units(path: &Path) -> usize {
    path.as_os_str().as_encoded_bytes().len()
}

pub(super) fn verify(
    root: &Path,
    relative: &str,
    limits: ProjectLimits,
    usage: &mut SourceUsage,
    allocator: &mut Allocator,
    origin: Span,
) -> Result<PathBuf, Box<Diagnostic>> {
    qualified_host(origin)?;
    let mut probe = PathBuf::new();
    let joined = add(
        path_units(root),
        usize::from(!root.as_os_str().as_encoded_bytes().ends_with(b"/")),
        Some(origin),
    )
    .and_then(|n| add(n, relative.len(), Some(origin)))?;
    allocator
        .path(&mut probe, joined, "module probe path")
        .map_err(|error| super::reserve_error(error, Some(origin)))?;
    probe.push(root);
    let mut components = relative.split('/').peekable();
    while let Some(component) = components.next() {
        usage.probes = add(usage.probes, 1, Some(origin))?;
        if usage.probes > limits.probes.min(8448) {
            return Err(resource(
                "module directory probe budget exceeded",
                Some(origin),
            ));
        }
        let entries = fs::read_dir(&probe).map_err(|e| io_error(e, "", Some(origin)))?;
        let (exact, folded) = scan_entries(
            entries.map(|entry| entry.map(|entry| entry.file_name())),
            component,
            limits,
            usage,
            origin,
        )?;
        if !exact {
            return Err(if folded {
                policy("module path case does not match its declaration", origin)
            } else {
                super::owned_diagnostic::diagnostic(
                    "E0002",
                    "source",
                    format_args!("declared module path component is missing"),
                    Some(origin),
                )
            });
        }
        probe.push(component);
        let metadata = fs::symlink_metadata(&probe).map_err(|e| io_error(e, "", Some(origin)))?;
        if metadata.file_type().is_symlink() {
            return Err(policy("module path contains a symbolic link", origin));
        }
        if components.peek().is_some() {
            if !metadata.is_dir() {
                return Err(policy(
                    "module intermediate component is not a directory",
                    origin,
                ));
            }
        } else if !metadata.is_file() {
            return Err(policy("module source is not a regular file", origin));
        }
    }
    let canonical = probe
        .canonicalize()
        .map_err(|e| io_error(e, "", Some(origin)))?;
    if !canonical.starts_with(root) {
        return Err(policy(
            "module canonical pathname escapes project root",
            origin,
        ));
    }
    Ok(canonical)
}

/// The production scan loop, with an iterator seam for deterministic I/O failures.
/// OS-returned names are transient; only two flags survive each yielded entry.
pub(super) fn scan_entries(
    entries: impl IntoIterator<Item = io::Result<OsString>>,
    component: &str,
    limits: ProjectLimits,
    usage: &mut SourceUsage,
    origin: Span,
) -> Result<(bool, bool), Box<Diagnostic>> {
    let mut exact = false;
    let mut folded = false;
    for entry in entries {
        let count = add(usage.directory_entries, 1, Some(origin))?;
        let name = match entry {
            Ok(name) => name,
            Err(error) => {
                usage.directory_entries = count;
                // A yielded error has no name units, but its already-known E
                // overflow/cap must be admitted before propagating metadata I/O.
                if count > limits.directory_entries.min(100_000) {
                    return Err(resource(
                        "module directory scan budget exceeded",
                        Some(origin),
                    ));
                }
                return Err(io_error(error, "", Some(origin)));
            }
        };
        let units = name.as_encoded_bytes().len();
        let names = add(usage.directory_name_units, units, Some(origin))?;
        usage.directory_entries = count;
        usage.directory_name_units = names;
        // For an available name, both checked arithmetic operations precede caps.
        if count > limits.directory_entries.min(100_000)
            || names > limits.directory_name_units.min(16 * 1024 * 1024)
        {
            return Err(resource(
                "module directory scan budget exceeded",
                Some(origin),
            ));
        }
        exact |= name == component;
        folded |= name
            .as_encoded_bytes()
            .eq_ignore_ascii_case(component.as_bytes());
    }
    Ok((exact, folded))
}
