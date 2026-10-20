//! Minimal typed workspace creation. This module does not invoke the compiler.
//!
//! The templates use the existing explicit-workspace and local path-dependency
//! formats. Both members are ordinary projects, not a separate library kind.
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

const MANIFEST: &str = "[workspace]\nmembers = [\"app\", \"library\"]\n";
const APP_MANIFEST: &str = "[project]\nname = \"workspace-app\"\nedition = \"typed-preview\"\nentry = \"main.ox\"\n\n[dependencies]\nmath = \"../library\"\n";
const APP_MAIN: &str = "use math::number;\nfn main() -> i32 { return number(); }\nfn test_answer() -> bool { return number() == 7; }\nfn test_unit() -> () { return; }\n";
const LIBRARY_MANIFEST: &str =
    "[project]\nname = \"workspace-library\"\nedition = \"typed-preview\"\nentry = \"lib.ox\"\n";
const LIBRARY_ROOT: &str = "pub fn number() -> i32 { return 7; }\nfn main() -> i32 { return 91; }\nfn test_library() -> bool { return true; }\n";
const README: &str = "# Typed Oxid workspace\n\nThis workspace uses the experimental typed-preview edition.\nIt contains two ordinary projects, workspace-app and workspace-library.\nThe app explicitly imports its local library through the math path dependency.\nWorkspace membership alone does not make an import available.\n\nFrom this directory:\n\n- Check: `oxid check --manifest-path oxid.toml --package workspace-app`\n- Interpret: `oxid run --manifest-path oxid.toml --package workspace-app`\n- Test: `oxid test --manifest-path oxid.toml --package workspace-app`\n- Build: `oxid build --manifest-path oxid.toml --package workspace-app`\n- Run the library project: `oxid run --manifest-path oxid.toml --package workspace-library`\n\nThe app prints 7 and its two tests pass. The library project prints 91.\nEdit `library/lib.ox` to change the public number function used by the app.\nWith two members, --package is required and selects the project name, not a path.\nBuild requires Linux x86_64 and LLVM 19.1.7 (clang, opt and ld.lld).\nApplication outputs belong under the selected member's .oxid directory, not the virtual root.\nThis scaffold has only local path dependencies and creates no lockfile or downloaded packages.\nTyped-preview is not a stable language release; workspace-wide execution is not provided.\n";
const FILES: [(&str, &str); 7] = [
    ("oxid.toml", MANIFEST),
    ("app/oxid.toml", APP_MANIFEST),
    ("app/main.ox", APP_MAIN),
    ("library/oxid.toml", LIBRARY_MANIFEST),
    ("library/lib.ox", LIBRARY_ROOT),
    ("README.md", README),
    (".gitignore", ".oxid/\n"),
];

fn validate_name(root: &Path) -> Result<(), String> {
    // Match typed project creation's spelling rules, including syntactic dots
    // that Path::components would otherwise normalize away.
    let raw = root
        .as_os_str()
        .to_str()
        .ok_or("workspace path must be UTF-8")?;
    if raw.is_empty()
        || raw
            .split(std::path::is_separator)
            .any(|part| part == "." || part == "..")
        || root.components().any(|part| {
            matches!(
                part,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
    {
        return Err("workspace path must not contain . or .. components".into());
    }
    let value = root
        .file_name()
        .and_then(|part| part.to_str())
        .ok_or("workspace path requires a directory name")?;
    if !(1..=64).contains(&value.len())
        || !value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphanumeric() || (index > 0 && matches!(byte, b'_' | b'-'))
        })
    {
        return Err("workspace directory name must match [A-Za-z0-9][A-Za-z0-9_-]{0,63}".into());
    }
    Ok(())
}

fn write_new(root: &Path, relative: &str, content: &str) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join(relative))?;
    file.write_all(content.as_bytes())
}

/// Creates only a previously absent directory. A later failure retains every
/// partial file for inspection; this function never deletes or overwrites files.
///
/// As with ordinary typed project creation, the surrounding filesystem must be
/// stable. This is not an atomic transaction or a sandbox against concurrent
/// replacement of directories.
pub(super) fn create(root: &Path) -> Result<(), String> {
    create_with(root, write_new)
}

fn create_with(
    root: &Path,
    mut write: impl FnMut(&Path, &str, &str) -> io::Result<()>,
) -> Result<(), String> {
    validate_name(root)?;
    fs::create_dir(root)
        .map_err(|error| format!("cannot create workspace `{}`: {error}", root.display()))?;
    let result = (|| -> Result<(), String> {
        for relative in ["app", "library"] {
            fs::create_dir(root.join(relative))
                .map_err(|error| format!("cannot create {relative} directory: {error}"))?;
        }
        for (relative, content) in FILES {
            write(root, relative, content)
                .map_err(|error| format!("cannot write {relative}: {error}"))?;
        }
        Ok(())
    })();
    result.map_err(|error| {
        format!(
            "workspace creation failed in `{}`: {error}; partial files were retained for inspection",
            root.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            for _ in 0..100 {
                let path = std::env::temp_dir().join(format!(
                    "oxid-workspace-scaffold-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("scratch creation failed: {error}"),
                }
            }
            panic!("scratch name collisions");
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn creates_only_the_exact_workspace_files() {
        let scratch = Scratch::new();
        let root = scratch.0.join("hello-world");
        create(&root).unwrap();
        for (relative, expected) in FILES {
            assert_eq!(
                fs::read(root.join(relative)).unwrap(),
                expected.as_bytes(),
                "{relative}"
            );
            assert!(expected.ends_with('\n'), "{relative}");
        }
        assert_eq!(fs::read_dir(&root).unwrap().count(), 5);
        assert_eq!(fs::read_dir(root.join("app")).unwrap().count(), 2);
        assert_eq!(fs::read_dir(root.join("library")).unwrap().count(), 2);
        for relative in [
            ".oxid",
            "app/.oxid",
            "library/.oxid",
            "oxid.lock",
            "app/oxid.lock",
            "library/oxid.lock",
            ".git",
        ] {
            assert!(!root.join(relative).exists(), "{relative}");
        }
    }

    #[test]
    fn manifests_match_the_existing_supported_workspace_example() {
        let example = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/projects/typed_workspace_selection");
        for (relative, expected) in [
            ("oxid.toml", MANIFEST),
            ("app/oxid.toml", APP_MANIFEST),
            ("library/oxid.toml", LIBRARY_MANIFEST),
        ] {
            assert_eq!(
                fs::read(example.join(relative)).unwrap(),
                expected.as_bytes(),
                "{relative}"
            );
        }
    }

    #[test]
    fn source_templates_are_normalized_existing_examples_and_idempotent() {
        use crate::frontend::source::SourceMap;
        let example = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/projects/typed_workspace_selection");
        for (relative, expected) in [("app/main.ox", APP_MAIN), ("library/lib.ox", LIBRARY_ROOT)] {
            let mut sources = SourceMap::new();
            let original = sources.add(
                relative.into(),
                fs::read_to_string(example.join(relative)).unwrap(),
            );
            assert_eq!(
                crate::frontend::format::format_source(sources.get(original)).unwrap(),
                expected,
                "{relative}"
            );
            let generated = sources.add(relative.into(), expected.into());
            assert_eq!(
                crate::frontend::format::format_source(sources.get(generated)).unwrap(),
                expected,
                "{relative}"
            );
        }
    }

    #[test]
    fn invalid_names_have_no_effects() {
        let scratch = Scratch::new();
        for value in [
            "-bad",
            "_bad",
            "has space",
            "has.quote\"",
            "é",
            &"a".repeat(65),
        ] {
            assert_eq!(
                create(&scratch.0.join(value)).unwrap_err(),
                "workspace directory name must match [A-Za-z0-9][A-Za-z0-9_-]{0,63}"
            );
        }
        assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 0);
        for value in ["", ".", "..", "ok/.", "ok/./child", "ok/../bad"] {
            assert!(validate_name(Path::new(value)).is_err(), "{value:?}");
        }
        assert!(validate_name(Path::new(std::path::MAIN_SEPARATOR_STR)).is_err());
    }

    #[test]
    fn fixed_package_names_accept_boundary_directory_names() {
        let scratch = Scratch::new();
        for value in [
            "7",
            "a_b-c",
            "workspace-app",
            "workspace-library",
            &"a".repeat(64),
        ] {
            let root = scratch.0.join(value);
            create(&root).unwrap();
            assert_eq!(
                fs::read_to_string(root.join("app/oxid.toml")).unwrap(),
                APP_MANIFEST
            );
            assert_eq!(
                fs::read_to_string(root.join("library/oxid.toml")).unwrap(),
                LIBRARY_MANIFEST
            );
        }
        fs::write(scratch.0.join("7/library/lib.ox"), "changed").unwrap();
        assert_eq!(
            fs::read_to_string(scratch.0.join("a_b-c/library/lib.ox")).unwrap(),
            LIBRARY_ROOT
        );
    }

    #[test]
    fn existing_destinations_are_untouched() {
        let scratch = Scratch::new();
        let file = scratch.0.join("file");
        fs::write(&file, b"owned").unwrap();
        assert!(create(&file).is_err());
        assert_eq!(fs::read(&file).unwrap(), b"owned");

        let empty = scratch.0.join("empty");
        fs::create_dir(&empty).unwrap();
        assert!(create(&empty).is_err());
        assert_eq!(fs::read_dir(&empty).unwrap().count(), 0);

        let directory = scratch.0.join("directory");
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("keep"), b"owned").unwrap();
        assert!(create(&directory).is_err());
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        assert_eq!(fs::read(directory.join("keep")).unwrap(), b"owned");
    }

    #[test]
    fn missing_parent_is_not_created() {
        let scratch = Scratch::new();
        assert!(create(&scratch.0.join("missing/workspace")).is_err());
        assert!(!scratch.0.join("missing").exists());
    }

    #[test]
    fn every_injected_write_failure_retains_exact_prior_files() {
        let scratch = Scratch::new();
        for (failure_index, (failed_path, _)) in FILES.iter().enumerate() {
            let root = scratch.0.join(format!("partial-{failure_index}"));
            let error = create_with(&root, |root, relative, content| {
                if relative == *failed_path {
                    Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "injected write failure",
                    ))
                } else {
                    write_new(root, relative, content)
                }
            })
            .unwrap_err();
            assert!(
                error.contains(&format!(
                    "cannot write {failed_path}: injected write failure"
                )),
                "{error}"
            );
            assert!(error.contains("partial files were retained for inspection"));
            assert!(root.join("app").is_dir());
            assert!(root.join("library").is_dir());
            for (index, (relative, expected)) in FILES.iter().enumerate() {
                if index < failure_index {
                    assert_eq!(
                        fs::read(root.join(relative)).unwrap(),
                        expected.as_bytes(),
                        "{relative}"
                    );
                } else {
                    assert!(!root.join(relative).exists(), "{relative}");
                }
            }
            assert!(create(&root).is_err());
            for (relative, expected) in &FILES[..failure_index] {
                assert_eq!(fs::read(root.join(relative)).unwrap(), expected.as_bytes());
            }
        }
    }

    #[test]
    fn a_partly_written_file_is_retained_and_never_retried() {
        let scratch = Scratch::new();
        let root = scratch.0.join("partial-content");
        let error = create_with(&root, |root, relative, content| {
            if relative == "library/lib.ox" {
                write_new(root, relative, "partial content")?;
                Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "injected partial write",
                ))
            } else {
                write_new(root, relative, content)
            }
        })
        .unwrap_err();
        assert!(error.contains("cannot write library/lib.ox: injected partial write"));
        assert!(error.contains("partial files were retained for inspection"));
        assert_eq!(
            fs::read(root.join("library/lib.ox")).unwrap(),
            b"partial content"
        );
        assert!(!root.join("README.md").exists());
        assert!(create(&root).is_err());
        assert_eq!(
            fs::read(root.join("library/lib.ox")).unwrap(),
            b"partial content"
        );
    }

    #[test]
    fn exclusive_file_creation_preserves_an_existing_child() {
        let scratch = Scratch::new();
        let root = scratch.0.join("child-collision");
        let error = create_with(&root, |root, relative, content| {
            if relative == "app/main.ox" {
                fs::write(root.join(relative), b"owned child")?;
            }
            write_new(root, relative, content)
        })
        .unwrap_err();
        assert!(error.contains("cannot write app/main.ox:"));
        assert!(error.contains("partial files were retained for inspection"));
        assert_eq!(fs::read(root.join("app/main.ox")).unwrap(), b"owned child");
        assert_eq!(
            fs::read(root.join("oxid.toml")).unwrap(),
            MANIFEST.as_bytes()
        );
        assert!(!root.join("library/oxid.toml").exists());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_destinations_are_never_followed() {
        let scratch = Scratch::new();
        let target = scratch.0.join("target");
        fs::create_dir(&target).unwrap();
        for (link, destination) in [
            ("live", target.clone()),
            ("dangling", scratch.0.join("absent")),
        ] {
            let link = scratch.0.join(link);
            std::os::unix::fs::symlink(destination, &link).unwrap();
            assert!(create(&link).is_err());
            assert!(fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink());
        }
        assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
        assert!(!scratch.0.join("absent").exists());
    }

    #[cfg(unix)]
    #[test]
    fn exclusive_file_creation_does_not_follow_child_symlinks() {
        let scratch = Scratch::new();
        let target = scratch.0.join("owned-target");
        fs::write(&target, b"owned target").unwrap();
        for (name, destination) in [
            ("live-child", target.clone()),
            ("dangling-child", scratch.0.join("absent")),
        ] {
            let root = scratch.0.join(name);
            let error = create_with(&root, |root, relative, content| {
                if relative == "app/main.ox" {
                    std::os::unix::fs::symlink(&destination, root.join(relative))?;
                }
                write_new(root, relative, content)
            })
            .unwrap_err();
            assert!(error.contains("cannot write app/main.ox:"));
            assert!(fs::symlink_metadata(root.join("app/main.ox"))
                .unwrap()
                .file_type()
                .is_symlink());
            assert!(!root.join("library/oxid.toml").exists());
        }
        assert_eq!(fs::read(&target).unwrap(), b"owned target");
        assert!(!scratch.0.join("absent").exists());
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_paths_are_rejected_without_effects() {
        use std::os::unix::ffi::OsStringExt;
        let scratch = Scratch::new();
        let root = scratch
            .0
            .join(std::ffi::OsString::from_vec(vec![b'a', 0xff]));
        assert_eq!(create(&root).unwrap_err(), "workspace path must be UTF-8");
        assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 0);
    }

    #[cfg(windows)]
    #[test]
    fn windows_dot_components_are_rejected() {
        for value in [
            r"a/../workspace",
            r"a/./workspace",
            r"a\..\workspace",
            r"a\.\workspace",
            r"C:./workspace",
            r"C:../workspace",
        ] {
            assert!(validate_name(Path::new(value)).is_err(), "{value:?}");
        }
    }
}
