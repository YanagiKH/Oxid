use super::{entry_path, local_source, Options};
use crate::{
    frontend,
    project_package::{same_file, PrivateDirectory},
    runtime::ProjectManifest,
};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, Metadata, OpenOptions},
    io::{self, Read, Write},
    path::Path,
};

pub(super) fn execute<'a>(
    options: &Options,
    root: &Path,
    projects: impl IntoIterator<Item = (&'a Path, &'a ProjectManifest)>,
) -> Result<i32, String> {
    let write = options.words == ["fmt", "--write"];
    if (!write && options.words != ["fmt", "--check"])
        || options.entry_mode.is_some()
        || options.json()
    {
        return Err(
            "use oxid fmt --check or oxid fmt --write for a project, or specify one file to format"
                .into(),
        );
    }

    let mut changes = BTreeMap::new();
    for (root, manifest) in projects {
        let package = [frontend::PackageSpec {
            root: root.to_path_buf(),
            entry: entry_path(root, manifest)?,
            dependencies: Vec::new(),
        }];
        let mut checked = BTreeSet::new();
        let mut visit = |path: &Path, original: &str, formatted: &str| {
            if !changes.contains_key(path) {
                let replacement = write
                    .then(|| Replacement::stage(path, original, formatted))
                    .transpose()
                    .map_err(|error| {
                        format!("cannot prepare formatting for {}: {error}", path.display())
                    })?;
                changes.insert(path.to_path_buf(), replacement);
            }
            Ok(())
        };
        let main =
            frontend::prepare_package_project(&package, None).map_err(|error| error.message())?;
        main.visit_format_changes(root, &mut checked, &mut visit)?;
        drop(main);
        for path in manifest.tests.values() {
            let entry = local_source(root, path)?;
            let test = frontend::prepare_package_project(&package, Some(&entry))
                .map_err(|error| error.message())?;
            test.visit_format_changes(root, &mut checked, &mut visit)?;
        }
    }

    if write {
        for (path, replacement) in &changes {
            if let Some(replacement) = replacement {
                replacement.verify(path)?;
            }
        }
        for (path, replacement) in changes {
            if let Some(replacement) = replacement {
                replacement.verify(&path)?;
                fs::rename(replacement.directory.path.join("source"), &path)
                    .map_err(|error| format!("cannot update {}: {error}", path.display()))?;
            }
        }
        return Ok(0);
    }

    let status = i32::from(!changes.is_empty());
    let mut stderr = io::stderr().lock();
    for path in changes.keys() {
        let relative = path.strip_prefix(root).unwrap_or(path);
        writeln!(stderr, "formatting required for {}", relative.display())
            .map_err(|error| format!("cannot report formatting changes: {error}"))?;
    }
    Ok(status)
}

struct Replacement {
    directory: PrivateDirectory,
    digest: [u8; 32],
    metadata: Metadata,
}

impl Replacement {
    fn stage(path: &Path, original: &str, formatted: &str) -> Result<Self, String> {
        let digest = Sha256::digest(original).into();
        let metadata = current_source(path, original.len() as u64, &digest)
            .map_err(|error| error.to_string())?;
        if metadata.permissions().readonly() {
            return Err("source is read-only".into());
        }
        let directory = PrivateDirectory::create(path.parent().ok_or("source has no parent")?)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.path.join("source"))
            .map_err(|error| error.to_string())?;
        file.write_all(formatted.as_bytes())
            .and_then(|()| file.set_permissions(metadata.permissions()))
            .and_then(|()| file.sync_all())
            .map_err(|error| error.to_string())?;
        Ok(Self {
            directory,
            digest,
            metadata,
        })
    }

    fn verify(&self, path: &Path) -> Result<(), String> {
        let current = current_source(path, self.metadata.len(), &self.digest)
            .map_err(|error| format!("cannot update {}: {error}", path.display()))?;
        if !same_file(&self.metadata, &current)
            || self.metadata.permissions() != current.permissions()
        {
            return Err(format!(
                "{} changed during formatting; retry the command",
                path.display()
            ));
        }
        Ok(())
    }
}

fn current_source(
    path: &Path,
    expected_bytes: u64,
    expected_digest: &[u8; 32],
) -> io::Result<Metadata> {
    let changed = || io::Error::other("source changed during formatting; retry the command");
    let before = fs::symlink_metadata(path)?;
    if !before.is_file() || before.len() != expected_bytes || fs::canonicalize(path)? != path {
        return Err(changed());
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let mut file = options.open(path)?;
    let admitted = file.metadata()?;
    if !same_file(&before, &admitted) {
        return Err(changed());
    }
    let mut digest = Sha256::new();
    let copied = io::copy(&mut (&mut file).take(expected_bytes + 1), &mut digest)?;
    let current = fs::symlink_metadata(path)?;
    if copied != expected_bytes
        || &<[u8; 32]>::from(digest.finalize()) != expected_digest
        || !same_file(&admitted, &file.metadata()?)
        || !same_file(&admitted, &current)
        || fs::canonicalize(path)? != path
    {
        return Err(changed());
    }
    Ok(current)
}
