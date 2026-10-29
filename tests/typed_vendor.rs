//! Detached offline checkouts use the public commands and the same canonical
//! source-package bytes. Native acceptance is a separate explicit LLVM test.
#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod qualified {

    use sha2::{Digest, Sha256};
    use std::{
        collections::BTreeMap,
        fs,
        os::unix::fs::{symlink, PermissionsExt},
        path::{Path, PathBuf},
        process::{Command, Output, Stdio},
        sync::atomic::{AtomicUsize, Ordering},
    };

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "oxid-vendor-雪-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
        fn write(&self, path: &str, value: &str) {
            let path = self.0.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, value).unwrap();
        }
        fn command(&self, cwd: &Path, args: &[&str]) -> Command {
            let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
            command
                .args(args)
                .current_dir(cwd)
                .stdin(Stdio::null())
                .env("OXID_LLVM_BIN", self.0.join("missing-tools"))
                .env("OXID_CACHE_DIR", self.0.join("native-cache"))
                .env_remove("OXID_PATH");
            command
        }
        fn run(&self, directory: &str, args: &[&str]) -> Output {
            self.command(&self.0.join(directory), args)
                .output()
                .unwrap()
        }
        fn local(&self) {
            self.write("app/oxid.toml", "[project]\nname=\"app\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\n[dependencies]\nlocal=\"../local\"\n[tests]\nsmoke=\"tests/smoke.ox\"\n");
            self.write(
                "app/main.ox",
                "fn main()->i32{return crate::local::value();}",
            );
            self.write("app/tests/smoke.ox", "fn main()->i32{return 0;}");
            self.write("local/oxid.toml", "[project]\nname=\"local\"\nedition=\"typed-preview\"\nkind=\"lib\"\n[tests]\nsmoke=\"tests/smoke.ox\"\n");
            self.write("local/src/lib.ox", "pub fn value()->i32{return 11;}");
            self.write("local/tests/smoke.ox", "fn main()->i32{return 0;}");
        }
        fn git(&self, directory: &str, args: &[&str]) -> String {
            let output = Command::new("git")
                .args([
                    "-c",
                    "user.name=Oxid tests",
                    "-c",
                    "user.email=tests@oxid.invalid",
                ])
                .args(args)
                .current_dir(self.0.join(directory))
                .output()
                .unwrap();
            assert!(output.status.success(), "{output:?}");
            String::from_utf8(output.stdout).unwrap().trim().into()
        }
        fn git_source(&self) -> String {
            self.write("repository/oxid.toml", "[project]\nname=\"remote\"\nversion=\"1.0.0\"\nedition=\"typed-preview\"\nkind=\"lib\"\n");
            self.write("repository/src/lib.ox", "pub fn value()->i32{return 17;}");
            self.git("repository", &["init", "--quiet"]);
            self.git("repository", &["add", "."]);
            self.git(
                "repository",
                &["commit", "--quiet", "-m", "Pinned dependency"],
            );
            let revision = self.git("repository", &["rev-parse", "HEAD"]);
            format!(
                "git+file://{}#{revision}",
                self.0.join("repository").display()
            )
        }
        fn registry(&self) -> String {
            good(self.run("", &["registry", "init", "registry"]));
            let source = format!("file://{}", self.0.join("registry").display());
            for version in ["1.0.0", "2.0.0"] {
                self.write("publisher/oxid.toml", &format!("[project]\nname=\"counts\"\nversion=\"{version}\"\nedition=\"typed-preview\"\nkind=\"lib\"\n"));
                // Equal source bytes remain two independently named release tuples.
                self.write("publisher/src/lib.ox", "pub fn value()->i32{return 7;}");
                good(self.run("publisher", &["lock", "--update"]));
                good(self.run("publisher", &["publish", "--registry", &source]));
            }
            source
        }
        fn mixed(&self) {
            self.local();
            let git = self.git_source();
            let registry = self.registry();
            self.write("local/oxid.toml", &format!("[project]\nname=\"local\"\nedition=\"typed-preview\"\nkind=\"lib\"\n[dependencies]\nshared={git:?}\n[tests]\nsmoke=\"tests/smoke.ox\"\n"));
            self.write("app/oxid.toml", &format!("[project]\nname=\"app\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\n[dependencies]\nlocal=\"../local\"\nremote={git:?}\nold={{registry=\"{registry}\",package=\"counts\",version=\"1.0.0\"}}\nold_alias={{registry=\"{registry}\",package=\"counts\",version=\"1.0.0\"}}\ncurrent={{registry=\"{registry}\",package=\"counts\",version=\"2.0.0\"}}\n[tests]\nsmoke=\"tests/smoke.ox\"\n"));
            self.write("app/main.ox", "fn main()->i32{return crate::local::value()+crate::remote::value()+crate::old::value()+crate::current::value();}");
            good(self.run("app", &["lock"]));
        }
        fn vendor(&self, directory: &str, output: &Path) -> serde_json::Value {
            let output = good(self.run(
                directory,
                &[
                    "vendor",
                    "--frozen",
                    "--output",
                    output.to_str().unwrap(),
                    "--message-format=json",
                ],
            ));
            assert!(output.stderr.is_empty(), "{output:?}");
            let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(value["schema"], "oxid-typed-vendor-summary-v1");
            assert_eq!(value["kind"], "vendor-summary");
            assert_eq!(value["success"], true);
            assert_eq!(value["dependency_mode"], "self-contained-local");
            value
        }
        fn no_staging(&self) {
            assert!(fs::read_dir(&self.0).unwrap().all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".oxid-package-")));
        }
        fn no_git(&self) -> (std::ffi::OsString, PathBuf) {
            let directory = self.0.join("forbidden-git");
            fs::create_dir(&directory).unwrap();
            let program = directory.join("git");
            fs::write(
                &program,
                "#!/bin/sh\nprintf invoked > \"$OXID_FORBIDDEN_GIT_MARKER\"\nexit 97\n",
            )
            .unwrap();
            fs::set_permissions(program, fs::Permissions::from_mode(0o755)).unwrap();
            let mut paths = vec![directory];
            paths.extend(std::env::split_paths(
                &std::env::var_os("PATH").unwrap_or_default(),
            ));
            (
                std::env::join_paths(paths).unwrap(),
                self.0.join("git-invoked"),
            )
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn good(output: Output) -> Output {
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        output
    }
    fn bad(output: Output) -> Output {
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        output
    }
    fn tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        fn collect(root: &Path, current: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
            for entry in fs::read_dir(current).unwrap() {
                let entry = entry.unwrap();
                if entry.file_type().unwrap().is_dir() {
                    collect(root, &entry.path(), files);
                } else {
                    assert!(entry.file_type().unwrap().is_file());
                    files.insert(
                        entry.path().strip_prefix(root).unwrap().into(),
                        fs::read(entry.path()).unwrap(),
                    );
                }
            }
        }
        let mut files = BTreeMap::new();
        collect(root, root, &mut files);
        files
    }

    fn original_sources(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        // The inherited successful Git admission may refresh its own Git index or
        // configuration. Source files, manifests, locks and cached payloads retain
        // their bytes. Do not exclude metadata from exported-tree checks below.
        tree(root)
            .into_iter()
            .filter(|(path, _)| {
                !path
                    .components()
                    .any(|component| component.as_os_str() == ".git")
            })
            .collect()
    }

    #[test]
    fn mixed_locked_checkout_is_offline_and_byte_equivalent_to_the_source_bundle() {
        let f = Fixture::new();
        f.mixed();
        let reference = f.0.join("reference.tar");
        good(f.run(
            "app",
            &[
                "package",
                "--frozen",
                "--output",
                reference.to_str().unwrap(),
            ],
        ));
        let reference = fs::read(reference).unwrap();
        let original = original_sources(&f.0.join("app"));
        let destination = f.0.join("offline 雪 \" quoted");
        let report = f.vendor("app", &destination);
        assert_eq!(original_sources(&f.0.join("app")), original);
        assert_eq!(
            report["source_package_sha256"],
            format!("{:x}", Sha256::digest(&reference))
        );
        assert_eq!(report["source_package_bytes"], reference.len() as u64);
        let root = PathBuf::from(report["root"].as_str().unwrap());
        assert!(root.starts_with(&destination));
        assert_eq!(report["output"], destination.to_str().unwrap());
        let lock = fs::read_to_string(root.join("oxid.lock")).unwrap();
        assert!(lock.starts_with("version = 2\n"), "{lock}");
        assert!(!lock.contains("typed-registry") && !lock.contains("artifact ="));
        let manifest = fs::read_to_string(root.join("oxid.toml")).unwrap();
        assert!(
            !manifest.contains("git+") && !manifest.contains("registry="),
            "{manifest}"
        );
        let dependencies = manifest
            .split("[dependencies]")
            .nth(1)
            .unwrap()
            .split("[tests]")
            .next()
            .unwrap();
        let source = |alias: &str| {
            dependencies
                .lines()
                .find_map(|line| line.strip_prefix(&format!("{alias}=")).map(str::to_owned))
                .unwrap()
        };
        assert_eq!(source("old"), source("old_alias"));
        assert_ne!(source("old"), source("current"));
        for path in tree(&destination).keys() {
            assert!(!path
                .components()
                .any(|component| component.as_os_str() == ".git"));
            assert!(!matches!(
                path.file_name().and_then(|name| name.to_str()),
                Some("release.toml" | "artifact.tar")
            ));
        }
        for directory in ["app", "local", "repository", "publisher", "registry"] {
            fs::remove_dir_all(f.0.join(directory)).unwrap();
        }
        let (path, marker) = f.no_git();
        for command in ["check", "run", "test"] {
            let result = good(
                f.command(&root, &[command, "--frozen"])
                    .env("PATH", &path)
                    .env("OXID_FORBIDDEN_GIT_MARKER", &marker)
                    .output()
                    .unwrap(),
            );
            let expected_stderr: &[u8] = match command {
                "test" => b"test \"smoke\": ok (exit status: 0)\ntests: 1 passed; 0 failed\n",
                "check" | "run" => b"",
                _ => unreachable!(),
            };
            assert_eq!(result.stderr, expected_stderr, "{result:?}");
            if command == "run" {
                assert_eq!(result.stdout, b"42\n");
            }
        }
        let repacked = f.0.join("repacked.tar");
        good(
            f.command(
                &root,
                &[
                    "package",
                    "--frozen",
                    "--output",
                    repacked.to_str().unwrap(),
                ],
            )
            .env("PATH", path)
            .env("OXID_FORBIDDEN_GIT_MARKER", &marker)
            .output()
            .unwrap(),
        );
        assert_eq!(fs::read(repacked).unwrap(), reference);
        assert!(!marker.exists());
        f.no_staging();
    }

    #[test]
    fn complete_workspace_and_declared_dependency_tests_cannot_be_hidden_by_selection() {
        let f = Fixture::new();
        f.local();
        f.write("oxid.toml", "[workspace]\nedition=\"typed-preview\"\nmembers=[\"app\",\"local\"]\ndefault-members=[\"app\"]\n");
        good(f.run("", &["lock"]));
        let lock = fs::read(f.0.join("oxid.lock")).unwrap();
        let location = Fixture::new();
        let destination = location.0.join("offline");
        let report = f.vendor("local", &destination);
        let root = PathBuf::from(report["root"].as_str().unwrap());
        assert_eq!(fs::read(root.join("oxid.lock")).unwrap(), lock);
        assert!(root.join("app/main.ox").is_file());
        assert!(root.join("local/tests/smoke.ox").is_file());
        for command in ["check", "run", "test"] {
            let result = good(f.command(&root, &[command, "--frozen"]).output().unwrap());
            let expected_stderr: &[u8] = match command {
                "check" => b"app: ok\nworkspace check: 1 passed; 0 failed\n",
                "run" => b"",
                "test" => {
                    b"test app::smoke: ok (exit status: 0)\nworkspace test: 1 passed; 0 failed\n"
                }
                _ => unreachable!(),
            };
            assert_eq!(result.stderr, expected_stderr, "{result:?}");
            if command == "run" {
                assert_eq!(result.stdout, b"11\n");
            }
        }
        let archive = location.0.join("workspace.tar");
        good(
            f.command(
                &root,
                &["package", "--frozen", "--output", archive.to_str().unwrap()],
            )
            .output()
            .unwrap(),
        );
        assert!(archive.is_file());
        let build = bad(f
            .command(&root, &["build", "--frozen", "--message-format=json"])
            .output()
            .unwrap());
        let message = format!(
            "{}{}",
            String::from_utf8_lossy(&build.stdout),
            String::from_utf8_lossy(&build.stderr)
        );
        assert!(
            message.contains("missing-tools") && !message.contains("nested workspace"),
            "{build:?}"
        );
        f.write("local/tests/smoke.ox", "fn main()->i32{return false;}");
        let refused = location.0.join("refused");
        bad(f.run(
            "app",
            &["vendor", "--frozen", "--output", refused.to_str().unwrap()],
        ));
        assert!(!refused.exists());
        assert_eq!(fs::read(f.0.join("oxid.lock")).unwrap(), lock);
        assert!(root.join("local/tests/smoke.ox").is_file());
        f.no_staging();
        location.no_staging();
    }

    #[test]
    fn destinations_inside_existing_workspaces_are_refused_before_staging_without_changing_sources()
    {
        let f = Fixture::new();
        f.local();
        f.write("oxid.toml", "[workspace]\nedition=\"typed-preview\"\nmembers=[\"app\",\"local\"]\ndefault-members=[\"app\"]\n");
        good(f.run("", &["lock"]));
        fs::create_dir(f.0.join("subdirectory")).unwrap();
        let original = tree(&f.0);
        for name in ["offline", "app/offline", "subdirectory/offline"] {
            let output = f.0.join(name);
            let result = bad(f.run(
                "local",
                &["vendor", "--frozen", "--output", output.to_str().unwrap()],
            ));
            let message = String::from_utf8_lossy(&result.stderr);
            assert!(
                message.contains("inside the workspace")
                    && message.contains("choose an output parent outside"),
                "{result:?}"
            );
            assert!(!output.exists());
            assert_eq!(tree(&f.0), original);
            f.no_staging();
            assert!(fs::read_dir(output.parent().unwrap())
                .unwrap()
                .all(|entry| !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".oxid-package-")));
        }
    }

    #[test]
    fn manifest_named_output_is_refused_without_effects_and_the_linux_comparison_is_exact_case() {
        let source = Fixture::new();
        source.local();
        good(source.run("app", &["lock"]));
        let location = Fixture::new();
        let original = tree(&source.0);
        let untouched_output_parent = tree(&location.0);
        let destination = location.0.join("oxid.toml");
        let output = bad(source.run(
            "app",
            &[
                "vendor",
                "--frozen",
                "--output",
                destination.to_str().unwrap(),
            ],
        ));
        assert!(output.stdout.is_empty(), "{output:?}");
        let message = String::from_utf8_lossy(&output.stderr);
        assert!(
            message.contains("cannot be named oxid.toml")
                && message.contains("choose a different directory name"),
            "{output:?}"
        );
        assert!(!destination.exists());
        assert_eq!(tree(&source.0), original);
        assert_eq!(tree(&location.0), untouched_output_parent);
        source.no_staging();
        location.no_staging();
        // Linux discovery treats only the exact lowercase component as a manifest.
        let report = source.vendor("app", &location.0.join("Oxid.toml"));
        let root = PathBuf::from(report["root"].as_str().unwrap());
        good(
            source
                .command(&root, &["check", "--frozen"])
                .output()
                .unwrap(),
        );
        let archive = location.0.join("case-check.tar");
        good(
            source
                .command(
                    &root,
                    &["package", "--frozen", "--output", archive.to_str().unwrap()],
                )
                .output()
                .unwrap(),
        );
        assert!(archive.is_file());
        assert_eq!(tree(&source.0), original);
        source.no_staging();
        location.no_staging();
    }

    #[test]
    fn destination_discovery_uses_real_workspace_grammar_and_preserves_orphan_workspace_locks() {
        let source = Fixture::new();
        source.local();
        good(source.run("app", &["lock"]));
        let location = Fixture::new();
        location.write(
            "oxid.toml",
            "[metadata]\nnote = '''\n[workspace]\nmembers = [\"inert\"]\n'''\n",
        );
        let report = source.vendor("app", &location.0.join("ordinary"));
        let root = PathBuf::from(report["root"].as_str().unwrap());
        good(
            source
                .command(&root, &["check", "--frozen"])
                .output()
                .unwrap(),
        );
        good(
            source
                .command(&root, &["run", "--frozen"])
                .output()
                .unwrap(),
        );
        good(
            source
                .command(&root, &["test", "--frozen"])
                .output()
                .unwrap(),
        );
        location.write(
            "oxid.toml",
            "[\"workspace\"]\nedition=\"typed-preview\"\nmembers=[\"ordinary/source/app\"]\n",
        );
        let output = location.0.join("refused");
        let before = tree(&location.0);
        let result = bad(source.run("app", &["vendor", "--output", output.to_str().unwrap()]));
        assert!(String::from_utf8_lossy(&result.stderr).contains("inside the workspace"));
        assert_eq!(tree(&location.0), before);
        fs::remove_file(location.0.join("oxid.toml")).unwrap();
        for lock in [
            "version = 3\nkind = \"typed-workspace-v1\"\n",
            "version = 4\nkind = \"typed-registry-workspace-v1\"\n",
        ] {
            location.write("oxid.lock", lock);
            let result = bad(source.run("app", &["vendor", "--output", output.to_str().unwrap()]));
            assert!(
                String::from_utf8_lossy(&result.stderr).contains("workspace lock"),
                "{result:?}"
            );
            assert_eq!(
                fs::read_to_string(location.0.join("oxid.lock")).unwrap(),
                lock
            );
            assert!(!output.exists());
            location.no_staging();
        }
    }

    #[test]
    fn missing_stale_and_malformed_locks_and_changed_sources_create_no_checkout() {
        let f = Fixture::new();
        f.local();
        let destination = f.0.join("offline");
        let args = [
            "vendor",
            "--frozen",
            "--output",
            destination.to_str().unwrap(),
        ];
        bad(f.run("app", &args));
        assert!(!f.0.join("app/oxid.lock").exists());
        assert!(!destination.exists());
        good(f.run("app", &["lock"]));
        let lock = fs::read(f.0.join("app/oxid.lock")).unwrap();
        for bytes in [b"version = 1\n".as_slice(), &lock[..lock.len() / 2]] {
            fs::write(f.0.join("app/oxid.lock"), bytes).unwrap();
            bad(f.run("app", &args));
            assert_eq!(fs::read(f.0.join("app/oxid.lock")).unwrap(), bytes);
            assert!(!destination.exists());
        }
        fs::write(f.0.join("app/oxid.lock"), &lock).unwrap();
        for (name, replacement) in [
            ("local/src/lib.ox", "pub fn value()->i32{return 12;}"),
            (
                "app/oxid.toml",
                "[project]\nname=\"changed\"\nedition=\"typed-preview\"\n",
            ),
            ("app/main.ox", "fn main()->i32{return false;}"),
        ] {
            let original = fs::read(f.0.join(name)).unwrap();
            f.write(name, replacement);
            bad(f.run("app", &args));
            assert!(!destination.exists());
            assert_eq!(fs::read(f.0.join("app/oxid.lock")).unwrap(), lock);
            fs::write(f.0.join(name), original).unwrap();
        }
        f.no_staging();
    }

    #[test]
    fn output_conflicts_links_and_concurrent_publishers_never_replace_existing_bytes() {
        let f = Fixture::new();
        f.local();
        good(f.run("app", &["lock"]));
        f.write("sentinel", "keep this file");
        for name in ["file", "directory", "link", "dangling", "hardlink"] {
            let output = f.0.join(name);
            match name {
                "file" => {
                    fs::write(&output, "keep output").unwrap();
                }
                "directory" => {
                    fs::create_dir(&output).unwrap();
                }
                "link" => symlink(f.0.join("sentinel"), &output).unwrap(),
                "dangling" => symlink(f.0.join("absent"), &output).unwrap(),
                "hardlink" => fs::hard_link(f.0.join("sentinel"), &output).unwrap(),
                _ => unreachable!(),
            }
            bad(f.run("app", &["vendor", "--output", output.to_str().unwrap()]));
            assert!(fs::symlink_metadata(&output).is_ok());
            assert_eq!(
                fs::read_to_string(f.0.join("sentinel")).unwrap(),
                "keep this file"
            );
        }
        let destination = f.0.join("raced");
        let command = || {
            f.command(
                &f.0.join("app"),
                &[
                    "vendor",
                    "--frozen",
                    "--output",
                    destination.to_str().unwrap(),
                ],
            )
        };
        let one = command()
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let two = command()
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let results = [
            one.wait_with_output().unwrap(),
            two.wait_with_output().unwrap(),
        ];
        assert_eq!(
            results
                .iter()
                .filter(|result| result.status.success())
                .count(),
            1,
            "{results:?}"
        );
        assert!(results
            .iter()
            .all(|result| matches!(result.status.code(), Some(0 | 1))));
        assert!(destination.join("source/app/oxid.lock").is_file());
        let bytes = tree(&destination);
        bad(f.run(
            "app",
            &["vendor", "--output", destination.to_str().unwrap()],
        ));
        assert_eq!(tree(&destination), bytes);
        f.no_staging();
    }

    #[test]
    fn missing_cached_registry_releases_are_not_fetched_and_conflicting_cache_bytes_are_preserved()
    {
        let f = Fixture::new();
        let registry = f.registry();
        f.write("app/oxid.toml", &format!("[project]\nname=\"app\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\n[dependencies]\ncounts={{registry=\"{registry}\",package=\"counts\",version=\"1.0.0\"}}\n"));
        f.write(
            "app/main.ox",
            "fn main()->i32{return crate::counts::value();}",
        );
        good(f.run("app", &["lock"]));
        let lock = fs::read(f.0.join("app/oxid.lock")).unwrap();
        let cache = fs::read_dir(f.0.join("app/.oxid/registry"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let changed = cache.join("source/src/lib.ox");
        fs::write(&changed, "preserve conflicting local bytes").unwrap();
        let destination = f.0.join("offline");
        bad(f.run(
            "app",
            &[
                "vendor",
                "--frozen",
                "--output",
                destination.to_str().unwrap(),
            ],
        ));
        assert_eq!(
            fs::read_to_string(&changed).unwrap(),
            "preserve conflicting local bytes"
        );
        assert!(!destination.exists());
        fs::remove_dir_all(&cache).unwrap();
        let output = bad(f.run(
            "app",
            &[
                "vendor",
                "--offline",
                "--output",
                destination.to_str().unwrap(),
            ],
        ));
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("not cached"),
            "{output:?}"
        );
        assert!(
            !cache.exists(),
            "vendor must not fetch even from an available file registry"
        );
        assert_eq!(fs::read(f.0.join("app/oxid.lock")).unwrap(), lock);
        assert!(!destination.exists());
        f.no_staging();
    }

    #[test]
    fn strict_options_and_vendor_operands_never_mutate_an_unrelated_project() {
        let f = Fixture::new();
        f.write("vendor", "operand sentinel");
        for args in [
            vec!["vendor"],
            vec!["vendor", "--output"],
            vec!["vendor", "--output=first", "--output=second"],
            vec!["vendor", "--output=first", "--update"],
            vec!["vendor", "--output=first", "--package=app"],
            vec!["vendor", "--output=first", "--edition=legacy-0.9"],
            vec!["vendor", "--output=first", "--profile=release"],
            vec!["vendor", "--output=first", "--incremental"],
            vec!["vendor", "--output=first", "--frozen=false"],
        ] {
            bad(f.run("", &args));
            for path in ["first", "second", ".oxid", "oxid.lock"] {
                assert!(!f.0.join(path).exists());
            }
        }
        for args in [
            vec!["check", "vendor", "--edition=typed-preview"],
            vec!["run", "vendor", "--edition=typed-preview"],
        ] {
            let output = bad(f.run("", &args));
            assert!(!String::from_utf8_lossy(&output.stderr).contains("vendor requires --output"));
        }
        assert_eq!(
            fs::read_to_string(f.0.join("vendor")).unwrap(),
            "operand sentinel"
        );
        f.no_staging();
    }

    #[test]
    fn dirty_and_uncached_pinned_git_dependencies_are_preserved_without_fetching() {
        let f = Fixture::new();
        let source = f.git_source();
        f.write("app/oxid.toml", &format!("[project]\nname=\"app\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\n[dependencies]\nremote={source:?}\n"));
        f.write(
            "app/main.ox",
            "fn main()->i32{return crate::remote::value();}",
        );
        good(f.run("app", &["lock"]));
        let lock = fs::read(f.0.join("app/oxid.lock")).unwrap();
        let cache = fs::read_dir(f.0.join("app/.oxid/deps"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        fs::write(cache.join("src/lib.ox"), "preserve dirty Git source").unwrap();
        let dirty = tree(&cache);
        let destination = f.0.join("offline");
        bad(f.run(
            "app",
            &[
                "vendor",
                "--frozen",
                "--output",
                destination.to_str().unwrap(),
            ],
        ));
        assert_eq!(tree(&cache), dirty);
        assert!(!destination.exists());
        fs::remove_dir_all(&cache).unwrap();
        let (path, marker) = f.no_git();
        let output = bad(f
            .command(
                &f.0.join("app"),
                &[
                    "vendor",
                    "--frozen",
                    "--output",
                    destination.to_str().unwrap(),
                ],
            )
            .env("PATH", path)
            .env("OXID_FORBIDDEN_GIT_MARKER", &marker)
            .output()
            .unwrap());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("not cached"),
            "{output:?}"
        );
        assert!(
            !marker.exists(),
            "an uncached frozen export must refuse before invoking Git"
        );
        assert!(!cache.exists());
        assert_eq!(fs::read(f.0.join("app/oxid.lock")).unwrap(), lock);
        assert!(!destination.exists());
        f.no_staging();
    }

    #[test]
    fn linked_source_and_missing_output_parent_are_refused_without_effects() {
        let f = Fixture::new();
        f.local();
        good(f.run("app", &["lock"]));
        let lock = fs::read(f.0.join("app/oxid.lock")).unwrap();
        let source = fs::read(f.0.join("local/src/lib.ox")).unwrap();
        fs::write(f.0.join("linked-source"), &source).unwrap();
        fs::remove_file(f.0.join("local/src/lib.ox")).unwrap();
        symlink(f.0.join("linked-source"), f.0.join("local/src/lib.ox")).unwrap();
        let destination = f.0.join("offline");
        bad(f.run(
            "app",
            &["vendor", "--output", destination.to_str().unwrap()],
        ));
        assert_eq!(fs::read(f.0.join("linked-source")).unwrap(), source);
        assert!(fs::symlink_metadata(f.0.join("local/src/lib.ox"))
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(!destination.exists());
        let missing_parent = f.0.join("missing/parent/offline");
        bad(f.run(
            "app",
            &["vendor", "--output", missing_parent.to_str().unwrap()],
        ));
        assert!(!f.0.join("missing").exists());
        assert_eq!(fs::read(f.0.join("app/oxid.lock")).unwrap(), lock);
        f.no_staging();
    }

    #[test]
    #[ignore = "requires actual LLVM, Clang and LLD 19.1.7"]
    fn mixed_offline_checkout_rebuilds_both_profiles_and_deploys_after_all_sources_are_removed() {
        let f = Fixture::new();
        f.mixed();
        let destination = f.0.join("offline");
        let report = f.vendor("app", &destination);
        let root = PathBuf::from(report["root"].as_str().unwrap());
        for directory in ["app", "local", "repository", "publisher", "registry"] {
            fs::remove_dir_all(f.0.join(directory)).unwrap();
        }
        let (path, marker) = f.no_git();
        let deploy = f.0.join("deploy");
        fs::create_dir(&deploy).unwrap();
        for profile in ["dev", "release"] {
            let mut command = f.command(
                &root,
                &["build", "--frozen", "--profile", profile, "--reproducible"],
            );
            if let Some(llvm) = std::env::var_os("OXID_LLVM_BIN") {
                command.env("OXID_LLVM_BIN", llvm);
            } else {
                command.env_remove("OXID_LLVM_BIN");
            }
            good(
                command
                    .env("PATH", &path)
                    .env("OXID_FORBIDDEN_GIT_MARKER", &marker)
                    .output()
                    .unwrap(),
            );
            fs::copy(root.join(".oxid/bin/app"), deploy.join(profile)).unwrap();
        }
        assert!(!marker.exists());
        fs::remove_dir_all(destination).unwrap();
        if f.0.join("native-cache").exists() {
            fs::remove_dir_all(f.0.join("native-cache")).unwrap();
        }
        for profile in ["dev", "release"] {
            let result = good(
                Command::new(deploy.join(profile))
                    .current_dir(&deploy)
                    .output()
                    .unwrap(),
            );
            assert_eq!(result.stdout, b"42\n");
            assert!(result.stderr.is_empty());
        }
    }

    #[test]
    #[ignore = "requires actual LLVM, Clang and LLD 19.1.7"]
    fn workspace_checkout_builds_from_its_printed_root_and_deploys_after_source_removal() {
        let source = Fixture::new();
        source.local();
        source.write("oxid.toml", "[workspace]\nedition=\"typed-preview\"\nmembers=[\"app\",\"local\"]\ndefault-members=[\"app\"]\n");
        good(source.run("", &["lock"]));
        let location = Fixture::new();
        let destination = location.0.join("offline");
        let report = source.vendor("local", &destination);
        let root = PathBuf::from(report["root"].as_str().unwrap());
        for directory in ["app", "local"] {
            fs::remove_dir_all(source.0.join(directory)).unwrap();
        }
        for name in ["oxid.toml", "oxid.lock"] {
            fs::remove_file(source.0.join(name)).unwrap();
        }
        for command in ["check", "run", "test"] {
            let result = good(
                source
                    .command(&root, &[command, "--frozen"])
                    .output()
                    .unwrap(),
            );
            if command == "run" {
                assert_eq!(result.stdout, b"11\n");
            }
            let expected_stderr: &[u8] = match command {
                "check" => b"app: ok\nworkspace check: 1 passed; 0 failed\n",
                "run" => b"",
                "test" => {
                    b"test app::smoke: ok (exit status: 0)\nworkspace test: 1 passed; 0 failed\n"
                }
                _ => unreachable!(),
            };
            assert_eq!(result.stderr, expected_stderr, "{result:?}");
        }
        let deploy = location.0.join("deploy");
        fs::create_dir(&deploy).unwrap();
        for profile in ["dev", "release"] {
            let mut command = source.command(
                &root,
                &["build", "--frozen", "--profile", profile, "--reproducible"],
            );
            if let Some(llvm) = std::env::var_os("OXID_LLVM_BIN") {
                command.env("OXID_LLVM_BIN", llvm);
            } else {
                command.env_remove("OXID_LLVM_BIN");
            }
            good(command.output().unwrap());
            fs::copy(root.join("app/.oxid/bin/app"), deploy.join(profile)).unwrap();
        }
        fs::remove_dir_all(destination).unwrap();
        if source.0.join("native-cache").exists() {
            fs::remove_dir_all(source.0.join("native-cache")).unwrap();
        }
        for profile in ["dev", "release"] {
            let result = good(
                Command::new(deploy.join(profile))
                    .current_dir(&deploy)
                    .output()
                    .unwrap(),
            );
            assert_eq!(result.stdout, b"11\n");
            assert!(result.stderr.is_empty());
        }
    }
}

#[cfg(not(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
)))]
#[test]
fn unsupported_hosts_refuse_before_source_admission_or_output_creation() {
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "oxid-vendor-unsupported-{}-{stamp:x}",
        std::process::id()
    ));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("oxid.toml"), "this manifest must not be admitted").unwrap();
    for json in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command
            .args(["vendor", "--frozen", "--output", "offline"])
            .current_dir(&root);
        if json {
            command.arg("--message-format=json");
        }
        let output = command.output().unwrap();
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        if json {
            assert!(output.stderr.is_empty(), "{output:?}");
            let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(value["success"], false);
            assert_eq!(value["root"], serde_json::Value::Null);
            assert!(value["error"]
                .as_str()
                .unwrap()
                .contains("require Linux x86_64 or ARM64"));
        } else {
            assert!(output.stdout.is_empty(), "{output:?}");
            assert!(
                String::from_utf8_lossy(&output.stderr).contains("require Linux x86_64 or ARM64")
            );
        }
        for path in ["offline", ".oxid", "oxid.lock"] {
            assert!(!root.join(path).exists());
        }
        assert_eq!(
            fs::read_to_string(root.join("oxid.toml")).unwrap(),
            "this manifest must not be admitted"
        );
    }
    fs::remove_dir_all(root).unwrap();
}
