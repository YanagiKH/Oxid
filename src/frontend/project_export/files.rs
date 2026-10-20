//! Fresh-directory writer for the normalized compiler-source export.
//!
//! Stable-filesystem assumption: like the module loader's qualified Linux
//! source policy, this writer is not a sandbox and gives no protection against
//! arbitrary noncooperating concurrent mutation of the destination or its
//! ancestors between a check and the following operation. Within that
//! assumption it guarantees:
//!
//! - every entry path, duplicate and file/directory prefix collision is
//!   validated before any filesystem operation;
//! - the output path must name a new directory below an existing parent whose
//!   ancestors are real directories, never symbolic links; syntactic `.`/`..`
//!   components and a trailing separator are refused, never normalized;
//! - the destination root is created once with no-replacement semantics, every
//!   internal directory is created by this invocation (none is adopted) and
//!   every file uses exclusive creation, so nothing is replaced or merged;
//! - nothing is ever removed: on failure any partial destination, including a
//!   partially written file, is retained for inspection and reported.
//!
//! Success means every file was fully written and flushed. There is no
//! whole-tree atomicity and no crash-durability claim.
use super::Entry;
use std::{
    cmp::Ordering,
    fmt::Write as _,
    fs,
    io::{self, Write},
    path::Path,
};

pub(super) struct Failure {
    pub message: String,
    pub destination_created: bool,
}

const ALLOCATION: &str = "export writer allocation failed";

pub(super) fn write_new(output: &Path, entries: &[Entry<'_>]) -> Result<(), Failure> {
    write_with(output, entries, |path| {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
    })
}

/// The production sequence. `create` must create a new file exclusively; it
/// is a seam only for deterministic write/flush fault injection in tests.
fn write_with<W: Write>(
    output: &Path,
    entries: &[Entry<'_>],
    mut create: impl FnMut(&Path) -> io::Result<W>,
) -> Result<(), Failure> {
    let not_created = |message: String| Failure {
        message,
        destination_created: false,
    };
    let created = |message: String| Failure {
        message,
        destination_created: true,
    };
    let directories = validate_entries(entries).map_err(not_created)?;
    check_output(output).map_err(not_created)?;
    fs::create_dir(output).map_err(|error| {
        not_created(format!(
            "cannot create export directory {}: {error}",
            quoted_path(output)
        ))
    })?;
    // This invocation created the root; from here every failure retains it.
    if !fs::symlink_metadata(output).is_ok_and(|metadata| metadata.is_dir()) {
        return Err(created(format!(
            "export directory {} changed after it was created",
            quoted_path(output)
        )));
    }
    for directory in directories {
        let path = output.join(directory);
        fs::create_dir(&path).map_err(|error| {
            created(format!(
                "cannot create export directory {}: {error}",
                quoted_path(&path)
            ))
        })?;
    }
    for entry in entries {
        let path = output.join(&entry.relative_path);
        let mut file = create(&path).map_err(|error| {
            created(format!(
                "cannot create export file {}: {error}",
                quoted_path(&path)
            ))
        })?;
        file.write_all(&entry.bytes).map_err(|error| {
            created(format!(
                "failed writing export file {} (a partially written file may remain): {error}",
                quoted_path(&path)
            ))
        })?;
        file.flush().map_err(|error| {
            created(format!(
                "failed flushing export file {} (a partially written file may remain): {error}",
                quoted_path(&path)
            ))
        })?;
    }
    Ok(())
}

/// Validates every entry path and collision, returning the implied internal
/// directories, parents before children. Performs no filesystem I/O.
fn validate_entries<'a>(entries: &'a [Entry<'_>]) -> Result<Vec<&'a str>, String> {
    let mut separators = 0usize;
    for entry in entries {
        let path = entry.relative_path.as_str();
        if let Some(reason) = invalid_entry(path) {
            return Err(format!(
                "invalid export entry path {}: {reason}",
                quoted(path)
            ));
        }
        let count = path.bytes().filter(|byte| *byte == b'/').count();
        separators = separators
            .checked_add(count)
            .ok_or_else(|| "export entry paths exceed the writer's limits".to_owned())?;
    }
    let mut order: Vec<&str> = Vec::new();
    order
        .try_reserve_exact(entries.len())
        .map_err(|_| ALLOCATION.to_owned())?;
    order.extend(entries.iter().map(|entry| entry.relative_path.as_str()));
    // With `/` ordered before every other admitted byte, a path that would be
    // a directory of another entry sorts immediately before that entry's
    // subtree, so adjacent comparison finds every duplicate and prefix.
    order.sort_unstable_by(|left, right| hierarchical(left, right));
    for pair in order.windows(2) {
        let (left, right) = (pair[0], pair[1]);
        if left == right {
            return Err(format!("duplicate export entry path {}", quoted(left)));
        }
        if right
            .strip_prefix(left)
            .is_some_and(|tail| tail.starts_with('/'))
        {
            return Err(format!(
                "export entry file {} collides with directory prefix of {}",
                quoted(left),
                quoted(right)
            ));
        }
    }
    let mut directories: Vec<&str> = Vec::new();
    directories
        .try_reserve_exact(separators)
        .map_err(|_| ALLOCATION.to_owned())?;
    for entry in entries {
        let path = entry.relative_path.as_str();
        for (index, _) in path.match_indices('/') {
            directories.push(&path[..index]);
        }
    }
    directories.sort_unstable_by(|left, right| hierarchical(left, right));
    directories.dedup();
    Ok(directories)
}

fn hierarchical(left: &str, right: &str) -> Ordering {
    let key = |byte: u8| if byte == b'/' { 0 } else { byte };
    left.bytes().map(key).cmp(right.bytes().map(key))
}

fn invalid_entry(path: &str) -> Option<&'static str> {
    if path.is_empty() {
        Some("empty path")
    } else if path.starts_with('/') {
        Some("absolute path")
    } else if path.contains('\\') {
        Some("backslash")
    } else if path.chars().any(char::is_control) {
        Some("NUL or control character")
    } else if path.split('/').any(str::is_empty) {
        Some("empty path component")
    } else if path.split('/').any(|part| part == "." || part == "..") {
        Some("`.` or `..` component")
    } else {
        None
    }
}

/// Refuses an unusable destination before anything is created.
fn check_output(output: &Path) -> Result<(), String> {
    let spelling = output.as_os_str().as_encoded_bytes();
    if spelling.is_empty() {
        return Err("export output path is empty".into());
    }
    if spelling.ends_with(b"/") {
        return Err(format!(
            "export output {} must name a new directory without a trailing separator",
            quoted_path(output)
        ));
    }
    if spelling
        .split(|byte| *byte == b'/')
        .any(|part| part == b"." || part == b"..")
    {
        return Err(format!(
            "export output {} must not contain `.` or `..` components",
            quoted_path(output)
        ));
    }
    let depth = output.ancestors().count();
    let mut ancestors: Vec<&Path> = Vec::new();
    ancestors
        .try_reserve_exact(depth)
        .map_err(|_| ALLOCATION.to_owned())?;
    ancestors.extend(
        output
            .ancestors()
            .skip(1)
            .filter(|path| !path.as_os_str().is_empty() && path.parent().is_some()),
    );
    // Outermost first, so a missing ancestor is reported where it starts.
    for ancestor in ancestors.iter().rev() {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "export output ancestor {} is a symbolic link",
                    quoted_path(ancestor)
                ))
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(format!(
                    "export output ancestor {} is not a directory",
                    quoted_path(ancestor)
                ))
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(format!(
                "export output parent {} does not exist; the parent directory must already exist",
                quoted_path(ancestor)
            ))
            }
            Err(error) => {
                return Err(format!(
                    "cannot inspect export output ancestor {}: {error}",
                    quoted_path(ancestor)
                ))
            }
        }
    }
    match fs::symlink_metadata(output) {
        Ok(metadata) => {
            let kind = if metadata.file_type().is_symlink() {
                "symbolic link"
            } else if metadata.is_dir() {
                "directory"
            } else if metadata.is_file() {
                "file"
            } else {
                "special file"
            };
            Err(format!(
                "export output {} already exists as a {kind}; choose a new directory",
                quoted_path(output)
            ))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "cannot inspect export output {}: {error}",
            quoted_path(output)
        )),
    }
}

/// Double-quoted, terminal-safe spelling: quotes, backslashes and every
/// control character (C0, DEL and C1) are escaped.
fn quoted(text: &str) -> String {
    let mut result = String::with_capacity(text.len().saturating_add(2));
    result.push('"');
    for character in text.chars() {
        match character {
            '"' | '\\' => {
                result.push('\\');
                result.push(character);
            }
            c if c.is_control() => {
                let _ = write!(result, "\\u{{{:04x}}}", u32::from(c));
            }
            c => result.push(c),
        }
    }
    result.push('"');
    result
}

fn quoted_path(path: &Path) -> String {
    quoted(&path.to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        borrow::Cow,
        os::unix::fs::{symlink, PermissionsExt},
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering as AtomicOrdering},
    };

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    /// A disposable test root. Only the test harness removes it; the writer
    /// under test never removes anything.
    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "oxid-export-files-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, AtomicOrdering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
        fn path(&self, relative: &str) -> PathBuf {
            self.0.join(relative)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn entry<'a>(path: &str, bytes: &'a [u8]) -> Entry<'a> {
        Entry {
            relative_path: path.into(),
            bytes: Cow::Borrowed(bytes),
        }
    }

    fn sample() -> Vec<Entry<'static>> {
        vec![
            entry("oxid.toml", b"[project]\nname = \"x\"\n"),
            entry(
                "src/.root.ox",
                b"mod a;\r\nfn main() -> i32 {\r\n    return 0;\r\n}\r\n",
            ),
            entry(
                "src/a.ox",
                "/// caf\u{e9} \u{65e5}\u{672c} \u{1f980}\n".as_bytes(),
            ),
            entry("src/a/b.ox", b""),
            entry("packages/p1/oxid.toml", b"[project]\n"),
            entry(
                "packages/p1/src/.root.ox",
                b"pub fn one() -> i32 {\n    return 1;\n}\n",
            ),
        ]
    }

    #[derive(Debug, PartialEq)]
    enum Node {
        Directory,
        File(Vec<u8>),
        Link(PathBuf),
    }

    fn tree(root: &Path) -> Vec<(String, Node)> {
        fn visit(root: &Path, path: &Path, out: &mut Vec<(String, Node)>) {
            let mut children: Vec<_> = fs::read_dir(path)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .collect();
            children.sort();
            for child in children {
                let relative = child
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                let metadata = fs::symlink_metadata(&child).unwrap();
                if metadata.file_type().is_symlink() {
                    out.push((relative, Node::Link(fs::read_link(&child).unwrap())));
                } else if metadata.is_dir() {
                    out.push((relative, Node::Directory));
                    visit(root, &child, out);
                } else {
                    out.push((relative, Node::File(fs::read(&child).unwrap())));
                }
            }
        }
        let mut out = Vec::new();
        visit(root, root, &mut out);
        out
    }

    fn expected_sample() -> Vec<(String, Node)> {
        let mut expected = vec![
            (
                "oxid.toml".to_owned(),
                Node::File(b"[project]\nname = \"x\"\n".to_vec()),
            ),
            ("packages".to_owned(), Node::Directory),
            ("packages/p1".to_owned(), Node::Directory),
            (
                "packages/p1/oxid.toml".to_owned(),
                Node::File(b"[project]\n".to_vec()),
            ),
            ("packages/p1/src".to_owned(), Node::Directory),
            (
                "packages/p1/src/.root.ox".to_owned(),
                Node::File(b"pub fn one() -> i32 {\n    return 1;\n}\n".to_vec()),
            ),
            ("src".to_owned(), Node::Directory),
            (
                "src/.root.ox".to_owned(),
                Node::File(b"mod a;\r\nfn main() -> i32 {\r\n    return 0;\r\n}\r\n".to_vec()),
            ),
            ("src/a".to_owned(), Node::Directory),
            ("src/a/b.ox".to_owned(), Node::File(Vec::new())),
            (
                "src/a.ox".to_owned(),
                Node::File(
                    "/// caf\u{e9} \u{65e5}\u{672c} \u{1f980}\n"
                        .as_bytes()
                        .to_vec(),
                ),
            ),
        ];
        expected.sort_by(|left, right| left.0.cmp(&right.0));
        expected
    }

    fn assert_safe(message: &str) {
        assert!(!message.chars().any(char::is_control), "{message:?}");
    }

    #[test]
    fn writes_every_entry_exactly_and_only_implied_directories() {
        let scratch = Scratch::new();
        let output = scratch.path("out");
        assert!(write_new(&output, &sample()).is_ok());
        let mut actual = tree(&output);
        actual.sort_by(|left, right| left.0.cmp(&right.0));
        assert_eq!(actual, expected_sample());
        // An empty entry list still creates exactly one fresh root.
        let empty = scratch.path("empty");
        assert!(write_new(&empty, &[]).is_ok());
        assert!(tree(&empty).is_empty());
    }

    #[test]
    fn every_entry_path_is_validated_before_the_destination_is_created() {
        let scratch = Scratch::new();
        let output = scratch.path("out");
        for invalid in [
            "",
            "/abs",
            "a//b",
            "a/",
            "/",
            "./a",
            "a/./b",
            "a/..",
            "../a",
            "..",
            ".",
            "a\\b",
            "a\0b",
            "a\nb",
            "a\u{1b}[31mb",
            "a\u{7f}",
            "a\u{85}b",
            "a\u{9b}b",
        ] {
            let mut entries = sample();
            entries.push(entry(invalid, b"late"));
            let failure = write_new(&output, &entries).expect_err(invalid);
            assert!(!failure.destination_created, "{invalid:?}");
            assert!(
                failure.message.starts_with("invalid export entry path"),
                "{}",
                failure.message
            );
            assert_safe(&failure.message);
            assert!(fs::symlink_metadata(&output).is_err(), "{invalid:?}");
        }
    }

    #[test]
    fn duplicates_and_prefix_collisions_are_rejected_in_any_order() {
        let scratch = Scratch::new();
        let output = scratch.path("out");
        for (paths, needle) in [
            (
                &["a/b", "c", "a/b"][..],
                "duplicate export entry path \"a/b\"",
            ),
            (&["a", "a-b", "a/c"][..], "collides with directory prefix"),
            (&["a/c", "a.b", "a"][..], "collides with directory prefix"),
            (
                &["src/x.ox/y", "src/x.ox0", "src/x.ox"][..],
                "collides with directory prefix",
            ),
            (&["p/q/r/s", "p/q"][..], "collides with directory prefix"),
        ] {
            let entries: Vec<_> = paths.iter().map(|path| entry(path, b"x")).collect();
            let failure = write_new(&output, &entries).expect_err(needle);
            assert!(failure.message.contains(needle), "{}", failure.message);
            assert!(!failure.destination_created);
            assert!(fs::symlink_metadata(&output).is_err());
        }
        // Sibling spellings that merely sort between a directory and its
        // children are not collisions.
        let entries: Vec<_> = ["a-b", "a/c", "a.b", "ab", "a/d/e"]
            .iter()
            .map(|path| entry(path, b"x"))
            .collect();
        assert_eq!(validate_entries(&entries).unwrap(), ["a", "a/d"]);
    }

    #[test]
    fn existing_destinations_of_every_type_are_refused_unchanged() {
        let scratch = Scratch::new();
        fs::create_dir(scratch.path("dir")).unwrap();
        fs::write(scratch.path("dir/keep"), b"keep").unwrap();
        fs::write(scratch.path("file"), b"file").unwrap();
        symlink(scratch.path("dir"), scratch.path("link")).unwrap();
        symlink(scratch.path("nowhere"), scratch.path("dangling")).unwrap();
        let before = tree(&scratch.0);
        for (name, kind) in [
            ("dir", "directory"),
            ("file", "file"),
            ("link", "symbolic link"),
            ("dangling", "symbolic link"),
        ] {
            let failure = write_new(&scratch.path(name), &sample()).expect_err(name);
            assert!(!failure.destination_created, "{name}");
            assert!(
                failure
                    .message
                    .contains(&format!("already exists as a {kind}")),
                "{}",
                failure.message
            );
            assert_eq!(tree(&scratch.0), before, "{name}");
        }
        assert!(fs::symlink_metadata(scratch.path("nowhere")).is_err());
    }

    #[test]
    fn ancestors_must_be_existing_real_directories() {
        let scratch = Scratch::new();
        fs::create_dir(scratch.path("real")).unwrap();
        symlink(scratch.path("real"), scratch.path("alias")).unwrap();
        fs::write(scratch.path("plain"), b"plain").unwrap();
        let before = tree(&scratch.0);
        for (relative, needle) in [
            ("alias/new", "is a symbolic link"),
            ("alias/deeper/new", "is a symbolic link"),
            ("plain/new", "is not a directory"),
            ("missing/new", "does not exist"),
            ("missing/deeper/new", "does not exist"),
        ] {
            let failure = write_new(&scratch.path(relative), &sample()).expect_err(relative);
            assert!(!failure.destination_created, "{relative}");
            assert!(failure.message.contains(needle), "{}", failure.message);
            assert_eq!(tree(&scratch.0), before, "{relative}");
        }
        let missing = write_new(&scratch.path("missing/deeper/new"), &sample()).unwrap_err();
        assert!(
            missing.message.contains("missing\" does not exist"),
            "{}",
            missing.message
        );
    }

    #[test]
    fn dot_components_and_trailing_separators_are_refused_not_normalized() {
        let scratch = Scratch::new();
        fs::create_dir(scratch.path("real")).unwrap();
        let before = tree(&scratch.0);
        let base = scratch.0.to_str().unwrap();
        for spelling in [
            format!("{base}/new/."),
            format!("{base}/new/.."),
            format!("{base}/real/../new"),
            format!("{base}/./new"),
            format!("{base}/new/"),
            String::new(),
        ] {
            let failure = write_new(Path::new(&spelling), &sample()).expect_err(&spelling);
            assert!(!failure.destination_created, "{spelling}");
            assert_eq!(tree(&scratch.0), before, "{spelling}");
        }
    }

    #[test]
    fn failure_messages_escape_terminal_controls() {
        let scratch = Scratch::new();
        let output = scratch.path("missing\u{1b}[31m\u{9b}\u{7}/new");
        let failure = write_new(&output, &sample()).unwrap_err();
        assert!(!failure.destination_created);
        assert_safe(&failure.message);
        assert!(
            failure
                .message
                .contains("missing\\u{001b}[31m\\u{009b}\\u{0007}"),
            "{}",
            failure.message
        );
    }

    /// Wraps a real exclusively created file: accepts at most `chunk` bytes per
    /// call and `budget` bytes in total, then fails; optionally fails flush.
    struct Faulty {
        file: fs::File,
        budget: usize,
        chunk: usize,
        fail_flush: bool,
    }
    impl Write for Faulty {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.budget == 0 {
                return Err(io::Error::other("injected write failure"));
            }
            let count = bytes.len().min(self.budget).min(self.chunk);
            let written = self.file.write(&bytes[..count])?;
            self.budget -= written;
            Ok(written)
        }
        fn flush(&mut self) -> io::Result<()> {
            if self.fail_flush {
                return Err(io::Error::other("injected flush failure"));
            }
            self.file.flush()
        }
    }

    fn faulty(
        fail_at: usize,
        budget: usize,
        chunk: usize,
        fail_flush: bool,
    ) -> impl FnMut(&Path) -> io::Result<Faulty> {
        let mut calls = 0usize;
        move |path: &Path| {
            calls += 1;
            let file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)?;
            let failing = calls == fail_at;
            Ok(Faulty {
                file,
                budget: if failing { budget } else { usize::MAX },
                chunk,
                fail_flush: failing && fail_flush,
            })
        }
    }

    #[test]
    fn short_writes_still_write_every_file_completely() {
        let scratch = Scratch::new();
        let output = scratch.path("out");
        assert!(write_with(&output, &sample(), faulty(0, 0, 1, false)).is_ok());
        let mut actual = tree(&output);
        actual.sort_by(|left, right| left.0.cmp(&right.0));
        assert_eq!(actual, expected_sample());
    }

    #[test]
    fn partial_write_failure_retains_output_and_reports_creation() {
        let scratch = Scratch::new();
        let output = scratch.path("out");
        // Entry 2 (`src/.root.ox`) accepts its first line (8 bytes), then fails.
        let failure = write_with(&output, &sample(), faulty(2, 8, 3, false)).unwrap_err();
        assert!(failure.destination_created);
        assert!(
            failure.message.contains("failed writing export file"),
            "{}",
            failure.message
        );
        assert!(
            failure.message.contains("src/.root.ox"),
            "{}",
            failure.message
        );
        assert!(
            failure.message.contains("partially written"),
            "{}",
            failure.message
        );
        assert_safe(&failure.message);
        assert_eq!(
            fs::read(output.join("oxid.toml")).unwrap(),
            b"[project]\nname = \"x\"\n"
        );
        assert_eq!(
            fs::read(output.join("src/.root.ox")).unwrap(),
            b"mod a;\r\n"
        );
        // Later files were never created; implied directories remain; nothing
        // was removed.
        assert!(fs::symlink_metadata(output.join("src/a.ox")).is_err());
        assert!(output.join("packages/p1/src").is_dir());
        assert!(output.join("src/a").is_dir());
    }

    #[test]
    fn flush_failure_is_reported_after_a_full_write() {
        let scratch = Scratch::new();
        let output = scratch.path("out");
        let failure =
            write_with(&output, &sample(), faulty(3, usize::MAX, usize::MAX, true)).unwrap_err();
        assert!(failure.destination_created);
        assert!(
            failure.message.contains("failed flushing export file"),
            "{}",
            failure.message
        );
        assert!(failure.message.contains("src/a.ox"), "{}", failure.message);
        assert_eq!(
            fs::read(output.join("src/a.ox")).unwrap(),
            "/// caf\u{e9} \u{65e5}\u{672c} \u{1f980}\n".as_bytes()
        );
        assert!(fs::symlink_metadata(output.join("src/a/b.ox")).is_err());
    }

    #[test]
    fn file_creation_failure_after_root_creation_is_reported_as_created() {
        let scratch = Scratch::new();
        let output = scratch.path("out");
        let mut calls = 0usize;
        let failure = write_with(&output, &sample(), |path| {
            calls += 1;
            if calls == 4 {
                return Err(io::Error::other("injected create failure"));
            }
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
        })
        .unwrap_err();
        assert!(failure.destination_created);
        assert!(
            failure.message.contains("cannot create export file"),
            "{}",
            failure.message
        );
        assert!(
            failure.message.contains("src/a/b.ox"),
            "{}",
            failure.message
        );
        assert!(output.join("src/a.ox").is_file());
    }

    #[test]
    fn root_creation_failure_is_not_reported_as_created() {
        let scratch = Scratch::new();
        let parent = scratch.path("readonly");
        fs::create_dir(&parent).unwrap();
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o555)).unwrap();
        // A privileged runner may bypass directory permissions; then there is
        // no unprivileged root-creation failure to observe.
        let probe = parent.join("probe");
        let privileged = fs::create_dir(&probe).is_ok();
        if !privileged {
            let failure = write_new(&parent.join("out"), &sample()).unwrap_err();
            assert!(!failure.destination_created);
            assert!(
                failure.message.contains("cannot create export directory"),
                "{}",
                failure.message
            );
            assert!(fs::symlink_metadata(parent.join("out")).is_err());
        }
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o755)).unwrap();
    }
}
