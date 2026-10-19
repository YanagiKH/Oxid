//! Bounded, deterministic manifest graph. No legacy resolver or network access.
use super::*;
use crate::frontend::{
    lexer,
    project::{
        forest::{Dependency, Package, PackageGraph, PackageId},
        ModuleId, ProjectLimits,
    },
};
use std::path::Path;

const EDGE_BYTES: usize = M_RAW;
#[cfg(test)]
mod tests;
mod workspace;

pub(in crate::frontend) struct Selection {
    pub graph: PackageGraph,
    pub parent: String,
}

#[derive(Default)]
struct Budget {
    manifest_bytes: usize,
    edge_bytes: usize,
    path_bytes: usize,
}
impl Budget {
    fn manifest(&mut self, bytes: usize) -> Result<(), Failure> {
        let total = add(self.manifest_bytes, bytes)?;
        if total > M_RAW {
            return Err(Failure::Resource(
                "aggregate manifest input exceeds 16384 bytes",
            ));
        }
        self.manifest_bytes = total;
        Ok(())
    }
    fn paths(&mut self, bytes: usize) -> Result<(), Failure> {
        let total = add(self.path_bytes, bytes)?;
        if total > ProjectLimits::default().path_bytes {
            return Err(Failure::Resource("retained manifest path budget exceeded"));
        }
        self.path_bytes = total;
        Ok(())
    }
    fn edges(&mut self, bytes: usize) -> Result<(), Failure> {
        let total = add(self.edge_bytes, bytes)?;
        if total > EDGE_BYTES {
            return Err(Failure::Resource(
                "manifest member/edge storage exceeds 16384 bytes",
            ));
        }
        self.edge_bytes = total;
        Ok(())
    }
}

fn reserve<T>(items: &mut Vec<T>, allocator: &mut Allocator) -> Result<(), Failure> {
    let count = add(items.len(), 1)?;
    allocator.attempt(count, size_of::<T>())?;
    items.try_reserve_exact(1).map_err(|_| allocation())
}

fn owned(value: &str, allocator: &mut Allocator) -> Result<String, Failure> {
    let mut text = allocator.string(value.len())?;
    text.push_str(value);
    Ok(text)
}

fn decoded(text: &str, field: Field, allocator: &mut Allocator) -> Result<String, Failure> {
    let mut result = allocator.string(field.decoded)?;
    visit(text, field, |c| {
        result.push(c);
        Ok(())
    })?;
    Ok(result)
}

fn alias_valid(alias: &str) -> bool {
    let mut bytes = alias.bytes();
    bytes
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == b'_')
        && bytes.all(|c| c.is_ascii_alphanumeric() || c == b'_')
        && lexer::identifier_kind(alias) == lexer::Kind::Ident
        && !matches!(alias, "crate" | "std" | "as" | "self" | "Self" | "super")
}

fn canonical(path: &str) -> Result<std::path::PathBuf, Failure> {
    let result = std::fs::canonicalize(path)
        .map_err(|e| Failure::Io(format!("cannot read manifest `{path}`: {e}")))?;
    if result.as_os_str().as_encoded_bytes().len() > P_JOINED {
        return Err(Failure::Resource(
            "canonical manifest path exceeds 8192 bytes",
        ));
    }
    Ok(result)
}

fn joined_manifest(path: &str, directory: &str) -> Result<std::path::PathBuf, Failure> {
    let base = Path::new(path).parent().unwrap_or(Path::new(""));
    let joined = add(
        add(base.as_os_str().as_encoded_bytes().len(), directory.len())?,
        12,
    )?;
    if joined > P_MANIFEST {
        return Err(Failure::Resource(
            "dependency/member manifest path exceeds 4096 bytes",
        ));
    }
    Ok(base.join(directory).join("oxid.toml"))
}

fn package(
    path: &str,
    canonical_manifest: std::path::PathBuf,
    raw: &[u8],
    operation: Command,
    needs_name: bool,
    budget: &mut Budget,
    allocator: &mut Allocator,
) -> Result<Package, Failure> {
    budget.manifest(raw.len())?;
    let text = std::str::from_utf8(raw)
        .map_err(|_| Failure::Utf8(format!("manifest `{path}` is not valid UTF-8")))?;
    let mut dependencies: Vec<Dependency> = Vec::new();
    let parsed = parse_with_dependencies(
        path,
        text,
        operation,
        Some(&mut |alias, field, line| {
            let invalid =
                |reason: &str| Failure::Cli(format!("manifest `{path}` line {line}: {reason}"));
            if !alias_valid(alias) {
                return Err(invalid(
                    "dependency alias must be a non-reserved source identifier",
                ));
            }
            if dependencies.iter().any(|d| d.alias == alias) {
                return Err(invalid("duplicate dependency alias"));
            }
            if field.decoded == 0 || field.decoded > D_LIMITS[1] {
                return Err(invalid(
                    "dependency directory must contain 1..2048 decoded bytes",
                ));
            }
            let bytes = add(size_of::<Dependency>(), add(alias.len(), field.decoded)?)?;
            budget.edges(bytes)?;
            budget.paths(field.decoded)?;
            visit(text, field, |c| {
                if c.is_control() || c == '\\' {
                    Err(invalid(
                        "dependency directory contains controls or backslashes",
                    ))
                } else {
                    Ok(())
                }
            })?;
            let directory = decoded(text, field, allocator)?;
            let alias = owned(alias, allocator)?;
            reserve(&mut dependencies, allocator)?;
            dependencies.push(Dependency {
                alias,
                directory,
                target: PackageId(usize::MAX),
                line,
            });
            Ok(())
        }),
    )?;
    // Charge the complete retained spelling before prepare_parsed allocates it.
    let entry_bytes = add(
        parent(path, cfg!(windows)).len(),
        parsed.fields[1].map_or(0, |f| f.decoded),
    )?;
    budget.paths(add(
        add(path.len(), entry_bytes)?,
        canonical_manifest.as_os_str().as_encoded_bytes().len(),
    )?)?;
    let mut name = String::new();
    let entry = prepare_parsed(
        path,
        text,
        allocator,
        operation,
        needs_name.then_some(&mut name),
        &parsed,
    )?;
    if !needs_name {
        if let Some(field) = parsed.fields[2] {
            name = decoded(text, field, allocator)?;
        }
    }
    Ok(Package {
        manifest: owned(path, allocator)?,
        canonical_manifest,
        entry,
        name,
        dependencies,
        root: ModuleId(usize::MAX),
        end: 0,
    })
}

pub(in crate::frontend) fn resolve(
    request: Box<[Request; 1]>,
    operation: Command,
    needs_name: bool,
) -> Result<Selection, Failure> {
    with_manifest(request, |path, raw, selector, allocator| {
        let mut budget = Budget::default();
        let selected = workspace::select(
            path,
            raw,
            selector,
            operation,
            needs_name,
            &mut budget,
            allocator,
        )?;
        finish(selected, operation, &mut budget, allocator)
    })
}

fn finish(
    selected: workspace::Selected,
    operation: Command,
    budget: &mut Budget,
    allocator: &mut Allocator,
) -> Result<Selection, Failure> {
    let workspace::Selected {
        root,
        mut cached,
        workspace,
    } = selected;
    let mut packages = Vec::new();
    reserve(&mut packages, allocator)?;
    packages.push(root);
    // Every graph node requires a source module. No recursion or per-node
    // budget multiplier; both node count and traversal fit the existing cap.
    let mut stack = [(PackageId(0), 0usize); 256];
    let mut height = 1;
    while height > 0 {
        let (id, next) = stack[height - 1];
        if next == packages[id.0].dependencies.len() {
            height -= 1;
            continue;
        }
        stack[height - 1].1 += 1;
        let node = &packages[id.0];
        let edge = &node.dependencies[next];
        let failure = |reason: &str| {
            Failure::Cli(format!(
                "manifest `{}` line {}: {reason}: {}",
                node.manifest, edge.line, edge.alias
            ))
        };
        let target_path = joined_manifest(&node.manifest, &edge.directory)?;
        let target_path = target_path.to_str().ok_or(Failure::Invariant)?;
        let target = canonical(target_path)?;
        if workspace.as_ref() == Some(&target) {
            return Err(failure("dependency cycle through virtual workspace"));
        }
        let existing = packages.iter().position(|p| p.canonical_manifest == target);
        if let Some(found) = existing {
            if stack[..height]
                .iter()
                .any(|(ancestor, _)| ancestor.0 == found)
            {
                return Err(failure("dependency cycle"));
            }
            if node.dependencies[..next]
                .iter()
                .any(|d| d.target == PackageId(found))
            {
                return Err(failure("duplicate canonical dependency target"));
            }
            packages[id.0].dependencies[next].target = PackageId(found);
            continue;
        }
        if packages.len() >= ProjectLimits::default().modules {
            return Err(Failure::Resource("package count exceeds module limit"));
        }
        let cached_member = cached.iter_mut().find(|snapshot| {
            snapshot
                .package
                .as_ref()
                .is_some_and(|p| p.canonical_manifest == target)
        });
        let child = if let Some(snapshot) = cached_member {
            snapshot.package.take().ok_or(Failure::Invariant)?
        } else {
            let raw = read_path(
                target_path,
                allocator,
                M_RAW
                    .checked_sub(budget.manifest_bytes)
                    .ok_or_else(overflow)?,
            )?;
            let child = package(
                target_path,
                target,
                &raw,
                operation,
                false,
                budget,
                allocator,
            )
            .map_err(|failure| match failure {
                Failure::Cli(message) if !message.starts_with("manifest `") => {
                    Failure::Cli(format!("manifest `{target_path}`: {message}"))
                }
                failure => failure,
            })?;
            if workspace.is_some() {
                workspace::retain(&mut cached, None, &raw, budget, allocator)?;
            }
            child
        };
        let child_id = PackageId(packages.len());
        reserve(&mut packages, allocator)?;
        packages.push(child);
        packages[id.0].dependencies[next].target = child_id;
        stack[height] = (child_id, 0);
        height += 1;
    }
    let output_parent = parent(&packages[0].manifest, cfg!(windows));
    budget.paths(output_parent.len())?;
    let output_parent = owned(output_parent, allocator)?;
    Ok(Selection {
        graph: PackageGraph {
            packages,
            retained_path_bytes: budget.path_bytes,
        },
        parent: output_parent,
    })
}
