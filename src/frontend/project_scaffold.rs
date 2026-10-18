//! Minimal typed project creation. This module does not invoke the compiler.
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

const MAIN: &str = "mod answer;\nfn main() -> i32 {\n    return crate::answer::value();\n}\n";
const ANSWER: &str = "pub fn value() -> i32 {\n    return 7;\n}\n";
const README: &str = "# Typed Oxid project\n\nThis project uses the typed-preview edition.\n\nFrom this directory:\n\n- Check: `oxid check --manifest-path oxid.toml`\n- Interpret: `oxid run --manifest-path oxid.toml`\n- Build: `oxid build --manifest-path oxid.toml`\n\nThe example returns 7. Result-mode execution prints the value and exits successfully.\nEdit `src/answer.ox` to change it. Build currently requires Linux x86_64 and LLVM 19.1.7.\nThe generated project has no external dependencies. Typed-preview is not a stable language release.\n";

fn name(root: &Path) -> Result<&str, String> {
    // Reject syntactic dot components rather than silently treating `foo/.` as foo.
    let raw = root
        .as_os_str()
        .to_str()
        .ok_or("project path must be UTF-8")?;
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
        return Err("project path must not contain . or .. components".into());
    }
    let value = root
        .file_name()
        .and_then(|part| part.to_str())
        .ok_or("project path requires a directory name")?;
    if !(1..=64).contains(&value.len())
        || !value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphanumeric() || (index > 0 && matches!(byte, b'_' | b'-'))
        })
    {
        return Err("project name must match [A-Za-z0-9][A-Za-z0-9_-]{0,63}".into());
    }
    Ok(value)
}

fn write_new(root: &Path, relative: &str, content: &str) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join(relative))?;
    file.write_all(content.as_bytes())
}

/// Creates only a previously absent directory. A failed write leaves its partial
/// project for inspection; this function never deletes or overwrites user files.
pub(super) fn create(root: &Path) -> Result<(), String> {
    create_with(root, write_new)
}

fn create_with(
    root: &Path,
    mut write: impl FnMut(&Path, &str, &str) -> io::Result<()>,
) -> Result<(), String> {
    let project_name = name(root)?;
    let manifest = format!(
        "[project]\nname = \"{project_name}\"\nversion = \"0.1.0\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n"
    );
    fs::create_dir(root)
        .map_err(|error| format!("cannot create project `{}`: {error}", root.display()))?;
    let result = (|| -> Result<(), String> {
        fs::create_dir(root.join("src"))
            .map_err(|error| format!("cannot create src directory: {error}"))?;
        for (relative, content) in [
            ("oxid.toml", manifest.as_str()),
            ("src/main.ox", MAIN),
            ("src/answer.ox", ANSWER),
            ("README.md", README),
            (".gitignore", ".oxid/\n"),
        ] {
            write(root, relative, content)
                .map_err(|error| format!("cannot write {relative}: {error}"))?;
        }
        Ok(())
    })();
    result.map_err(|error| {
        format!(
            "project creation failed in `{}`: {error}; partial files were retained for inspection",
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
                    "oxid-scaffold-{}-{}",
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
    fn creates_only_supported_project_files() {
        let scratch = Scratch::new();
        let root = scratch.0.join("hello-world");
        create(&root).unwrap();
        assert_eq!(fs::read_to_string(root.join("oxid.toml")).unwrap(),
            "[project]\nname = \"hello-world\"\nversion = \"0.1.0\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n");
        assert_eq!(fs::read_to_string(root.join("src/main.ox")).unwrap(), MAIN);
        assert_eq!(
            fs::read_to_string(root.join("src/answer.ox")).unwrap(),
            ANSWER
        );
        assert_eq!(fs::read_to_string(root.join("README.md")).unwrap(), README);
        assert_eq!(
            fs::read_to_string(root.join(".gitignore")).unwrap(),
            ".oxid/\n"
        );
        assert_eq!(fs::read_dir(&root).unwrap().count(), 4);
        assert_eq!(fs::read_dir(root.join("src")).unwrap().count(), 2);
    }
    #[test]
    fn invalid_names_have_no_effects() {
        let scratch = Scratch::new();
        for value in ["-bad", "has space", "has.quote\"", "é", &"a".repeat(65)] {
            assert_eq!(
                create(&scratch.0.join(value)).unwrap_err(),
                "project name must match [A-Za-z0-9][A-Za-z0-9_-]{0,63}"
            );
        }
        assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 0);
        for value in ["", ".", "..", "ok/.", "ok/../bad"] {
            assert!(name(Path::new(value)).is_err());
        }
    }
    #[cfg(windows)]
    #[test]
    fn windows_dot_components_are_rejected() {
        for value in [
            r"a/../project",
            r"a/./project",
            r"a\..\project",
            r"a\.\project",
            r"C:./project",
            r"C:../project",
        ] {
            assert!(name(Path::new(value)).is_err());
        }
    }
    #[test]
    fn existing_destination_is_untouched() {
        let scratch = Scratch::new();
        let file = scratch.0.join("file");
        fs::write(&file, b"owned").unwrap();
        assert!(create(&file).is_err());
        assert_eq!(fs::read(&file).unwrap(), b"owned");
        let dir = scratch.0.join("directory");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("keep"), b"owned").unwrap();
        assert!(create(&dir).is_err());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        assert_eq!(fs::read(dir.join("keep")).unwrap(), b"owned");
    }
    #[test]
    fn independent_projects_accept_boundary_names() {
        let scratch = Scratch::new();
        for value in ["7", "a_b-c", &"a".repeat(64)] {
            let root = scratch.0.join(value);
            create(&root).unwrap();
            assert!(fs::read_to_string(root.join("oxid.toml"))
                .unwrap()
                .contains(&format!("name = \"{value}\"")));
            assert_eq!(fs::read_to_string(root.join("src/main.ox")).unwrap(), MAIN);
        }
        fs::write(scratch.0.join("7/src/main.ox"), "changed").unwrap();
        assert_eq!(
            fs::read_to_string(scratch.0.join("a_b-c/src/main.ox")).unwrap(),
            MAIN
        );
    }
    #[test]
    fn partial_failure_identifies_file_and_preserves_created_files() {
        let scratch = Scratch::new();
        let root = scratch.0.join("partial");
        let error = create_with(&root, |root, relative, content| {
            if relative == "src/main.ox" {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "injected write failure",
                ))
            } else {
                write_new(root, relative, content)
            }
        })
        .unwrap_err();
        assert!(error.contains("cannot write src/main.ox: injected write failure"));
        assert!(error.contains("partial files were retained for inspection"));
        assert!(root.join("oxid.toml").is_file());
        assert!(root.join("src").is_dir());
        assert!(!root.join("src/main.ox").exists());
        assert!(!root.join("README.md").exists());
        assert!(create(&root).is_err());
        assert!(root.join("oxid.toml").is_file());
    }
    #[test]
    fn missing_parent_is_not_created() {
        let scratch = Scratch::new();
        assert!(create(&scratch.0.join("missing/project")).is_err());
        assert!(!scratch.0.join("missing").exists());
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
}
