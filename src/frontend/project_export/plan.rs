//! Pure portable layout planning from an already checked source forest.
//! Generated manifests are intentionally normalized, not original-file copies.
use super::super::{
    lexer, manifest,
    project::{forest::Package, ProjectLimits, ProjectSources},
};
use super::Entry;
use std::{
    borrow::Cow,
    fmt::{self, Write},
};

const PATH_BYTES: usize = 4 * 1024 * 1024;
const PATH_LENGTH: usize = 4096;
const ROOT_SOURCE: &str = "src/.root.ox";
const INVALID: &str = "invalid checked source-export graph or path";
const RESOURCE: &str = "source-export output planning exceeds its resource limits";
const ALLOCATION: &str = "source-export output planning allocation failed";

struct Count {
    bytes: usize,
    limit: usize,
}
impl Write for Count {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.bytes = self
            .bytes
            .checked_add(text.len())
            .filter(|n| *n <= self.limit)
            .ok_or(fmt::Error)?;
        Ok(())
    }
}

fn text(arguments: fmt::Arguments<'_>, limit: usize) -> Result<String, &'static str> {
    let mut count = Count { bytes: 0, limit };
    count.write_fmt(arguments).map_err(|_| RESOURCE)?;
    let mut result = String::new();
    result
        .try_reserve_exact(count.bytes)
        .map_err(|_| ALLOCATION)?;
    result.write_fmt(arguments).map_err(|_| INVALID)?;
    if result.len() != count.bytes {
        return Err(INVALID);
    }
    Ok(result)
}

/// Exactly the escapes understood by the existing typed manifest scanner.
struct Quoted<'a>(&'a str);
impl fmt::Display for Quoted<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_char('"')?;
        for character in self.0.chars() {
            match character {
                '"' => out.write_str("\\\"")?,
                '\\' => out.write_str("\\\\")?,
                '\n' => out.write_str("\\n")?,
                '\r' => out.write_str("\\r")?,
                '\t' => out.write_str("\\t")?,
                c if c.is_control() => write!(out, "\\u{:04X}", u32::from(c))?,
                c => out.write_char(c)?,
            }
        }
        out.write_char('"')
    }
}

struct Manifest<'a> {
    package: &'a Package,
    ordinal: usize,
}
impl fmt::Display for Manifest<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            out,
            "[project]\nedition = \"typed-preview\"\nentry = \"{ROOT_SOURCE}\""
        )?;
        if !self.package.name.is_empty() {
            writeln!(out, "name = {}", Quoted(&self.package.name))?;
        }
        if !self.package.dependencies.is_empty() {
            writeln!(out, "\n[dependencies]")?;
            for dependency in &self.package.dependencies {
                let target = dependency.target.0;
                if target == 0 {
                    return Err(fmt::Error);
                }
                if self.ordinal == 0 {
                    writeln!(out, "{} = \"packages/p{target}\"", dependency.alias)?;
                } else {
                    writeln!(out, "{} = \"../p{target}\"", dependency.alias)?;
                }
            }
        }
        Ok(())
    }
}

fn valid_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= PATH_LENGTH
        && !path.contains('\\')
        && !path.chars().any(char::is_control)
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != ".." && !part.contains(':'))
}
fn prefix_file(file: &str, other: &str) -> bool {
    other
        .strip_prefix(file)
        .is_some_and(|tail| tail.starts_with('/'))
}
fn append<'a>(
    entries: &mut Vec<Entry<'a>>,
    path_bytes: &mut usize,
    relative_path: String,
    bytes: Cow<'a, [u8]>,
) -> Result<(), &'static str> {
    if !valid_path(&relative_path)
        || entries.iter().any(|entry| {
            entry.relative_path == relative_path
                || prefix_file(&entry.relative_path, &relative_path)
                || prefix_file(&relative_path, &entry.relative_path)
        })
    {
        return Err(INVALID);
    }
    *path_bytes = path_bytes
        .checked_add(relative_path.len())
        .filter(|n| *n <= PATH_BYTES)
        .ok_or(RESOURCE)?;
    entries.push(Entry {
        relative_path,
        bytes,
    });
    Ok(())
}
fn package_path(ordinal: usize, relative: &str) -> Result<String, &'static str> {
    if ordinal == 0 {
        text(format_args!("{relative}"), PATH_LENGTH)
    } else {
        text(format_args!("packages/p{ordinal}/{relative}"), PATH_LENGTH)
    }
}
fn valid_alias(alias: &str) -> bool {
    let mut bytes = alias.bytes();
    bytes
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == b'_')
        && bytes.all(|c| c.is_ascii_alphanumeric() || c == b'_')
        && lexer::identifier_kind(alias) == lexer::Kind::Ident
        && !matches!(alias, "crate" | "std" | "as" | "self" | "Self" | "super")
}

fn validate_graph(packages: &[Package]) -> Result<(), &'static str> {
    if packages.is_empty() || packages.len() > ProjectLimits::default().modules {
        return Err(INVALID);
    }
    for (ordinal, package) in packages.iter().enumerate() {
        for (index, dependency) in package.dependencies.iter().enumerate() {
            if !valid_alias(&dependency.alias)
                || dependency.target.0 >= packages.len()
                || dependency.target.0 == ordinal
                || package.dependencies[..index].iter().any(|previous| {
                    previous.alias == dependency.alias || previous.target == dependency.target
                })
            {
                return Err(INVALID);
            }
        }
    }
    // Reproduce only the graph's structural DFS invariant, not source loading.
    // No recursion, graph cloning or persistent compiler-carrier changes.
    let mut state = Vec::new();
    state
        .try_reserve_exact(packages.len())
        .map_err(|_| ALLOCATION)?;
    state.resize(packages.len(), 0u8);
    let mut stack = Vec::new();
    stack
        .try_reserve_exact(packages.len())
        .map_err(|_| ALLOCATION)?;
    stack.push((0usize, 0usize));
    state[0] = 1;
    let mut discovered = 1;
    while let Some((ordinal, edge)) = stack.last().copied() {
        if edge == packages[ordinal].dependencies.len() {
            state[ordinal] = 2;
            stack.pop();
            continue;
        }
        stack.last_mut().ok_or(INVALID)?.1 += 1;
        let target = packages[ordinal].dependencies[edge].target.0;
        match state[target] {
            0 => {
                if target != discovered {
                    return Err(INVALID);
                }
                discovered += 1;
                state[target] = 1;
                stack.push((target, 0));
            }
            1 => return Err(INVALID),
            2 => {}
            _ => return Err(INVALID),
        }
    }
    if discovered != packages.len() {
        return Err(INVALID);
    }
    Ok(())
}

pub(super) fn build(project: &ProjectSources) -> Result<Vec<Entry<'_>>, &'static str> {
    let graph = project.graph().ok_or(INVALID)?;
    let modules = project.modules();
    let files = project.sources().files();
    let limits = ProjectLimits::default();
    if graph.packages.is_empty()
        || graph.packages.len() > limits.modules
        || modules.len() > limits.modules
        || files.len() != modules.len()
    {
        return Err(INVALID);
    }
    validate_graph(&graph.packages)?;
    let count = graph
        .packages
        .len()
        .checked_add(modules.len())
        .ok_or(RESOURCE)?;
    let mut entries = Vec::new();
    entries.try_reserve_exact(count).map_err(|_| ALLOCATION)?;
    let mut manifest_bytes = 0usize;
    let mut path_bytes = 0usize;
    let mut next_root = 0usize;
    for (ordinal, package) in graph.packages.iter().enumerate() {
        if package.root.0 != next_root || package.end <= next_root || package.end > modules.len() {
            return Err(INVALID);
        }
        let manifest = text(
            format_args!("{}", Manifest { package, ordinal }),
            manifest::M_RAW - manifest_bytes,
        )?;
        manifest_bytes = manifest_bytes
            .checked_add(manifest.len())
            .filter(|n| *n <= manifest::M_RAW)
            .ok_or(RESOURCE)?;
        append(
            &mut entries,
            &mut path_bytes,
            package_path(ordinal, "oxid.toml")?,
            Cow::Owned(manifest.into_bytes()),
        )?;
        for index in package.root.0..package.end {
            let module = &modules[index];
            if module.file.0 != index {
                return Err(INVALID);
            }
            let relative = if index == package.root.0 {
                if module.parent.is_some() || !module.relative_path.is_empty() {
                    return Err(INVALID);
                }
                text(format_args!("{ROOT_SOURCE}"), PATH_LENGTH)?
            } else {
                let parent = module.parent.ok_or(INVALID)?.0;
                if parent < package.root.0 || parent >= index || !valid_path(&module.relative_path)
                {
                    return Err(INVALID);
                }
                text(format_args!("src/{}", module.relative_path), PATH_LENGTH)?
            };
            append(
                &mut entries,
                &mut path_bytes,
                package_path(ordinal, &relative)?,
                Cow::Borrowed(files[index].text().as_bytes()),
            )?;
        }
        next_root = package.end;
    }
    if next_root != modules.len() || entries.len() != count {
        return Err(INVALID);
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::super::super::project::{
        forest::{Dependency, PackageId},
        ModuleId,
    };
    use super::*;

    fn package() -> Package {
        Package {
            manifest: "unused".into(),
            canonical_manifest: "unused".into(),
            entry: "unused".into(),
            name: "say \"hello\"\\\n\t\r\u{7f}雪".into(),
            root: ModuleId(0),
            end: 1,
            dependencies: vec![Dependency {
                alias: "math".into(),
                directory: "ignored-original-path".into(),
                target: PackageId(2),
                line: 1,
            }],
        }
    }
    #[test]
    fn manifest_escapes_metadata_and_relocates_edges() {
        let package = package();
        let root = text(
            format_args!(
                "{}",
                Manifest {
                    package: &package,
                    ordinal: 0
                }
            ),
            manifest::M_RAW,
        )
        .unwrap();
        assert_eq!(root, "[project]\nedition = \"typed-preview\"\nentry = \"src/.root.ox\"\nname = \"say \\\"hello\\\"\\\\\\n\\t\\r\\u007F雪\"\n\n[dependencies]\nmath = \"packages/p2\"\n");
        let dependency = text(
            format_args!(
                "{}",
                Manifest {
                    package: &package,
                    ordinal: 1
                }
            ),
            manifest::M_RAW,
        )
        .unwrap();
        assert!(dependency.ends_with("math = \"../p2\"\n"));
        assert!(!dependency.contains("ignored-original-path"));
    }
    #[test]
    fn paths_reject_traversal_and_collisions_without_new_source_bytes() {
        for path in [
            "", "/root", "a/../b", "a/./b", "a//b", "a\\b", "C:/root", "a\n",
        ] {
            assert!(!valid_path(path), "{path:?}");
        }
        let mut entries = Vec::with_capacity(3);
        let mut count = 0;
        append(
            &mut entries,
            &mut count,
            "src/a.ox".into(),
            Cow::Borrowed(b"source"),
        )
        .unwrap();
        assert!(append(
            &mut entries,
            &mut count,
            "src/a.ox".into(),
            Cow::Borrowed(b"other")
        )
        .is_err());
        assert!(append(
            &mut entries,
            &mut count,
            "src/a.ox/child".into(),
            Cow::Borrowed(b"other")
        )
        .is_err());
        assert!(append(
            &mut entries,
            &mut count,
            "src".into(),
            Cow::Borrowed(b"other")
        )
        .is_err());
        assert!(matches!(entries[0].bytes, Cow::Borrowed(_)));
    }

    #[test]
    fn graph_validation_rejects_cycles_orphans_reordering_and_reserved_aliases() {
        fn node(targets: &[usize]) -> Package {
            let mut result = package();
            result.dependencies = targets
                .iter()
                .enumerate()
                .map(|(index, target)| Dependency {
                    alias: format!("dep{index}"),
                    directory: "ignored".into(),
                    target: PackageId(*target),
                    line: index + 1,
                })
                .collect();
            result
        }
        assert!(validate_graph(&[node(&[1, 3]), node(&[2]), node(&[]), node(&[2])]).is_ok());
        for invalid in [
            vec![node(&[1]), node(&[0])],
            vec![node(&[]), node(&[])],
            vec![node(&[2, 1]), node(&[]), node(&[])],
        ] {
            assert!(validate_graph(&invalid).is_err());
        }
        for alias in ["crate", "std", "as", "self", "Self", "super", "fn"] {
            let mut root = node(&[1]);
            root.dependencies[0].alias = alias.into();
            assert!(validate_graph(&[root, node(&[])]).is_err(), "{alias}");
        }
    }

    #[test]
    fn output_text_limit_is_checked_before_allocation() {
        assert_eq!(text(format_args!("12345"), 4), Err(RESOURCE));
        assert_eq!(text(format_args!("雪"), 3).unwrap(), "雪");
    }
}
