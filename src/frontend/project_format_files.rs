//! Same-directory staging for explicit project formatter writes.
//!
//! This is a stable-filesystem policy, not a sandbox against adversarial races.
//! Each successful rename replaces one file. A group of files is NOT a
//! transaction, and failed commits never roll back over a concurrent edit.
use std::{
    fs::{self, File, Metadata, OpenOptions},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_STAGE: AtomicU64 = AtomicU64::new(0);

fn invalid(message: &str) -> io::Error {
    io::Error::other(message)
}

/// Reject symlinks in the selected spelling, including directory components.
/// Checking a canonical path alone would hide a symlink in the input spelling.
fn regular_path(path: &Path) -> io::Result<Metadata> {
    let mut prefix = PathBuf::new();
    let mut final_metadata = None;
    for component in path.components() {
        prefix.push(component.as_os_str());
        if matches!(component, Component::Prefix(_) | Component::RootDir) {
            continue;
        }
        let metadata = fs::symlink_metadata(&prefix)?;
        if metadata.file_type().is_symlink() {
            return Err(invalid(
                "project fmt --write refuses symbolic links in source paths",
            ));
        }
        final_metadata = Some(metadata);
    }
    let metadata = final_metadata.ok_or_else(|| invalid("source path must name a file"))?;
    if !metadata.is_file() {
        return Err(invalid("project fmt --write requires regular source files"));
    }
    if metadata.permissions().readonly() {
        return Err(invalid(
            "project fmt --write refuses read-only source files",
        ));
    }
    Ok(metadata)
}

/// Compare in fixed-size chunks, including an EOF check. Do not allocate another
/// source-sized buffer or trust metadata length as proof of unchanged content.
fn same_bytes(file: &mut impl Read, expected: &[u8]) -> io::Result<bool> {
    let mut scratch = [0_u8; 8192];
    let mut offset = 0;
    loop {
        let count = match file.read(&mut scratch) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if count == 0 {
            return Ok(offset == expected.len());
        }
        let Some(end) = offset.checked_add(count) else {
            return Ok(false);
        };
        if expected.get(offset..end) != Some(&scratch[..count]) {
            return Ok(false);
        }
        offset = end;
    }
}

/// Return the current permissions only after confirming the snapshot bytes.
pub(super) fn verify_original(path: &Path, expected: &[u8]) -> io::Result<Metadata> {
    let metadata = regular_path(path)?;
    let mut file = File::open(path)?;
    if !file.metadata()?.is_file() || !same_bytes(&mut file, expected)? {
        return Err(invalid(
            "source changed since it was loaded; no replacement was attempted",
        ));
    }
    Ok(metadata)
}

struct StageDirectory(PathBuf);
impl StageDirectory {
    fn create(parent: &Path) -> io::Result<Self> {
        for _ in 0..100 {
            let path = parent.join(format!(
                ".oxid-fmt-{}-{}",
                std::process::id(),
                NEXT_STAGE.fetch_add(1, Ordering::Relaxed)
            ));
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(invalid(
            "cannot reserve a fresh formatter staging directory",
        ))
    }
}
impl Drop for StageDirectory {
    fn drop(&mut self) {
        // Remove only the one file and directory created by this owner. Never
        // recursively delete content that might have appeared inside it.
        let _ = fs::remove_file(self.0.join("candidate"));
        let _ = fs::remove_dir(&self.0);
    }
}

pub(super) struct StagedFile<'a> {
    path: &'a Path,
    expected: &'a [u8],
    stage: StageDirectory,
}
impl<'a> StagedFile<'a> {
    pub(super) fn prepare(
        path: &'a Path,
        expected: &'a [u8],
        candidate: &[u8],
    ) -> io::Result<Self> {
        let original = verify_original(path, expected)?;
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let stage = StageDirectory::create(parent)?;
        let mut output = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(stage.0.join("candidate"))?;
        output.write_all(candidate)?;
        output.set_permissions(original.permissions())?;
        output.sync_all()?;
        Ok(Self {
            path,
            expected,
            stage,
        })
    }

    pub(super) fn verify(&self) -> io::Result<()> {
        verify_original(self.path, self.expected).map(|_| ())
    }

    /// The caller preflights every original before starting the commit loop.
    /// This second comparison catches ordinary edits between that barrier and
    /// this replacement. No delete-target fallback is permitted on any host.
    pub(super) fn commit(&self) -> io::Result<()> {
        let original = verify_original(self.path, self.expected)?;
        fs::set_permissions(self.stage.0.join("candidate"), original.permissions())?;
        fs::rename(self.stage.0.join("candidate"), self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            for _ in 0..100 {
                let path = std::env::temp_dir().join(format!(
                    "oxid-project-format-files-{}-{}",
                    std::process::id(),
                    NEXT_STAGE.fetch_add(1, Ordering::Relaxed)
                ));
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(error) => panic!("create test directory: {error}"),
                }
            }
            panic!("test directory collisions")
        }
        fn file(&self, name: &str, content: &[u8]) -> PathBuf {
            let path = self.0.join(name);
            fs::write(&path, content).unwrap();
            path
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn comparison_checks_eof_length_empty_and_interrupted_reads() {
        use std::io::Cursor;
        for (bytes, expected, equal) in [
            (&b""[..], &b""[..], true),
            (&b"abc"[..], &b"abc"[..], true),
            (&b"ab"[..], &b"abc"[..], false),
            (&b"abcd"[..], &b"abc"[..], false),
            (&b"axc"[..], &b"abc"[..], false),
        ] {
            assert_eq!(
                same_bytes(&mut Cursor::new(bytes), expected).unwrap(),
                equal
            );
        }
        struct Interrupted<'a>(bool, Cursor<&'a [u8]>);
        impl Read for Interrupted<'_> {
            fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
                if !self.0 {
                    self.0 = true;
                    Err(io::Error::from(io::ErrorKind::Interrupted))
                } else {
                    self.1.read(bytes)
                }
            }
        }
        assert!(same_bytes(&mut Interrupted(false, Cursor::new(b"abc")), b"abc").unwrap());
    }

    #[test]
    fn stage_does_not_change_original_and_drop_cleans_only_owned_files() {
        let scratch = Scratch::new();
        let path = scratch.file("main.ox", b"old");
        let stage = StagedFile::prepare(&path, b"old", b"formatted").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"old");
        assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 2);
        drop(stage);
        assert_eq!(fs::read(&path).unwrap(), b"old");
        assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 1);
    }

    #[test]
    fn commit_replaces_exact_original_and_leaves_no_stage() {
        let scratch = Scratch::new();
        let path = scratch.file("main.ox", b"old");
        let stage = StagedFile::prepare(&path, b"old", b"formatted").unwrap();
        stage.verify().unwrap();
        stage.commit().unwrap();
        drop(stage);
        assert_eq!(fs::read(&path).unwrap(), b"formatted");
        assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 1);
    }

    #[test]
    fn stale_source_is_never_replaced_and_missing_source_is_not_recreated() {
        let scratch = Scratch::new();
        let path = scratch.file("main.ox", b"old");
        assert!(StagedFile::prepare(&path, b"stale", b"formatted").is_err());
        let stage = StagedFile::prepare(&path, b"old", b"formatted").unwrap();
        fs::write(&path, b"concurrent edit").unwrap();
        assert!(stage.verify().is_err());
        assert!(stage.commit().is_err());
        assert_eq!(fs::read(&path).unwrap(), b"concurrent edit");
        fs::remove_file(&path).unwrap();
        assert!(stage.commit().is_err());
        assert!(!path.exists());
    }

    #[test]
    fn whole_set_preflight_can_abort_without_replacing_earlier_files() {
        let scratch = Scratch::new();
        let first = scratch.file("first.ox", b"first");
        let second = scratch.file("second.ox", b"second");
        let stages = [
            StagedFile::prepare(&first, b"first", b"formatted first").unwrap(),
            StagedFile::prepare(&second, b"second", b"formatted second").unwrap(),
        ];
        fs::write(&second, b"editor change").unwrap();
        assert!(stages.iter().try_for_each(StagedFile::verify).is_err());
        assert_eq!(fs::read(&first).unwrap(), b"first");
        assert_eq!(fs::read(&second).unwrap(), b"editor change");
    }

    #[test]
    fn later_commit_failure_never_rolls_back_an_already_written_file() {
        let scratch = Scratch::new();
        let first = scratch.file("first.ox", b"first");
        let second = scratch.file("second.ox", b"second");
        let stages = [
            StagedFile::prepare(&first, b"first", b"formatted first").unwrap(),
            StagedFile::prepare(&second, b"second", b"formatted second").unwrap(),
        ];
        stages.iter().try_for_each(StagedFile::verify).unwrap();
        stages[0].commit().unwrap();
        fs::write(&second, b"editor change").unwrap();
        assert!(stages[1].commit().is_err());
        assert_eq!(fs::read(&first).unwrap(), b"formatted first");
        assert_eq!(fs::read(&second).unwrap(), b"editor change");
    }

    #[cfg(unix)]
    #[test]
    fn symlink_file_and_directory_spellings_are_rejected() {
        use std::os::unix::fs::symlink;
        let scratch = Scratch::new();
        let source = scratch.file("main.ox", b"original");
        let link = scratch.0.join("alias.ox");
        symlink(&source, &link).unwrap();
        assert!(StagedFile::prepare(&link, b"original", b"formatted").is_err());
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        let directory_link = scratch.0.join("directory-link");
        symlink(&scratch.0, &directory_link).unwrap();
        assert!(
            StagedFile::prepare(&directory_link.join("main.ox"), b"original", b"formatted")
                .is_err()
        );
        assert_eq!(fs::read(&source).unwrap(), b"original");
    }

    #[cfg(unix)]
    #[test]
    fn unix_permissions_are_preserved_without_making_readonly_files_writable() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = Scratch::new();
        let path = scratch.file("main.ox", b"old");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        let stage = StagedFile::prepare(&path, b"old", b"formatted").unwrap();
        stage.commit().unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o440)).unwrap();
        assert!(StagedFile::prepare(&path, b"formatted", b"next").is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    }
}
