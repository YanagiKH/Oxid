use super::*;

#[test]
fn manifest_registry_allowance_is_lazy_shared_and_paid_before_descriptor_allocation() {
    let path = Path::new("oxid.toml");
    let mut context = None;
    let ordinary = runtime::parse_typed_manifest_bytes_registry(
        path,
        b"[project]\nedition=\"typed-preview\"\n[dependencies]\nlocal=\"../lib\"\n",
        &mut context,
        false,
    )
    .unwrap();
    assert!(context.is_none());
    assert!(ordinary.manifest.registry_dependencies.is_none());
    let raw = b"[project]\nedition=\"typed-preview\"\n[dependencies]\nlib={registry=\"file:///registry\",package=\"counts\",version=\"1.0.0\"}\n";
    let parsed =
        runtime::parse_typed_manifest_bytes_registry(path, raw, &mut context, false).unwrap();
    let allowance = context.as_ref().unwrap();
    let declarations = parsed.manifest.registry_dependencies.as_ref().unwrap();
    assert!(Rc::ptr_eq(
        allowance,
        declarations.allowance.as_ref().unwrap()
    ));
    let paid = (allowance.borrow().metadata, allowance.borrow().reserves);
    let cloned = parsed.clone();
    assert!(Rc::ptr_eq(
        declarations,
        cloned.manifest.registry_dependencies.as_ref().unwrap()
    ));
    assert_eq!(
        (allowance.borrow().metadata, allowance.borrow().reserves),
        paid
    );
    let denied = operation_budget().unwrap();
    denied.borrow_mut().fail_reserve = Some(0);
    let error =
        runtime::parse_typed_manifest_bytes_registry(path, raw, &mut Some(denied.clone()), false)
            .err()
            .unwrap();
    assert!(error.contains("injected registry reservation"), "{error}");
    assert_eq!(denied.borrow().reserves, 1);
}

#[test]
fn manifest_heavy_registry_parse_has_real_exact_and_one_less_admission() {
    let mut raw = String::from("[project]\nname=\"counts\"\nversion=\"1.0.0\"\nedition=\"typed-preview\"\n[dependencies]\n");
    for index in 0..96 {
        use std::fmt::Write as _;
        writeln!(
            raw,
            "alias{index}={{registry=\"file:///registry\",package=\"counts\",version=\"1.0.0\"}}"
        )
        .unwrap();
    }
    let mut measured = Budget::new().unwrap();
    let snapshot = parse_manifest(Path::new("oxid.toml"), raw.as_bytes(), &mut measured).unwrap();
    assert_eq!(
        snapshot
            .manifest
            .registry_dependencies
            .as_ref()
            .unwrap()
            .len(),
        96
    );
    assert!(snapshot
        .manifest
        .registry_dependencies
        .as_ref()
        .unwrap()
        .allowance
        .is_none());
    let mut exact = Budget::new().unwrap();
    exact.metadata_limit = measured.peak_metadata;
    let repeated = parse_manifest(Path::new("oxid.toml"), raw.as_bytes(), &mut exact).unwrap();
    assert_eq!(repeated.digest, snapshot.digest);
    assert_eq!(exact.peak_metadata, measured.peak_metadata);
    let mut short = Budget::new().unwrap();
    short.metadata_limit = measured.peak_metadata - 1;
    assert!(
        parse_manifest(Path::new("oxid.toml"), raw.as_bytes(), &mut short)
            .err()
            .unwrap()
            .contains("metadata")
    );
    // Scope admission precedes the opaque parser, including its descriptor Rc.
    assert_eq!(short.reserves, 0);
}

#[test]
fn registry_publication_lock_envelopes_preserve_local_bytes_and_exact_boundaries() {
    let root = std::env::temp_dir().join("oxid-registry-lock-model");
    let mut graph = Vec::new();
    let mut manifests = Vec::new();
    let mut fingerprints = Vec::new();
    for index in 0..12 {
        let package = root.join(format!("p{index}"));
        graph.push(crate::frontend::PackageSpec {
            root: package.clone(),
            entry: package.join("src").join("lib.ox"),
            dependencies: if index == 0 {
                (1..12)
                    .map(|package| crate::frontend::PackageDependency {
                        alias: format!("dep{package}"),
                        package,
                    })
                    .collect()
            } else {
                Vec::new()
            },
        });
        let raw = format!("[project]\nname=\"p{index}\"\nversion=\"1.0.0\"\nedition=\"typed-preview\"\nkind=\"lib\"\n");
        manifests.push(
            runtime::parse_typed_manifest_bytes(Path::new("oxid.toml"), raw.as_bytes()).unwrap(),
        );
        if index != 0 {
            fingerprints.push((index, format!("sha256:{}", "a".repeat(64))));
        }
    }
    let ordinary = crate::project_lock::expected(&graph, &manifests, &fingerprints).unwrap();
    let mut measured = Budget::new().unwrap();
    let snapshot =
        crate::project_lock::expected_registry(&graph, &manifests, &fingerprints, &mut measured)
            .unwrap();
    assert_eq!(snapshot, ordinary);
    let mut exact = Budget::new().unwrap();
    exact.metadata_limit = measured.peak_metadata;
    assert_eq!(
        crate::project_lock::expected_registry(&graph, &manifests, &fingerprints, &mut exact)
            .unwrap(),
        ordinary
    );
    let mut short = Budget::new().unwrap();
    short.metadata_limit = measured.peak_metadata - 1;
    assert!(
        crate::project_lock::expected_registry(&graph, &manifests, &fingerprints, &mut short)
            .err()
            .unwrap()
            .contains("metadata")
    );
    assert_eq!(short.reserves, 0);
    let mut all = vec![(0, format!("sha256:{}", "b".repeat(64)))];
    all.extend(fingerprints);
    let inputs = [crate::project_lock::WorkspaceInputs {
        graph: &graph,
        manifests: &manifests,
        fingerprints: &all,
    }];
    let ordinary =
        crate::project_lock::expected_workspace(&root, &"c".repeat(64), &inputs).unwrap();
    let mut paid = Budget::new().unwrap();
    assert_eq!(
        crate::project_lock::expected_registry_workspace(
            &root,
            &"c".repeat(64),
            &inputs,
            &mut paid
        )
        .unwrap(),
        ordinary
    );
}

#[test]
fn exact_registry_descriptors_do_not_reinterpret_quoted_paths() {
    let valid = "{ registry = \"file:///srv/oxid\", package = \"counts\", version = \"1.2.3\" }";
    let dependency = Dependency::parse(valid).unwrap();
    assert_eq!(dependency.encode(), valid);
    for invalid in [
        "{ registry = \"file:///srv/oxid\", package = \"counts\", version = \"^1.2.3\" }",
        "{ registry = \"file:///srv/oxid\", package = \"counts\", version = \"1.2.03\" }",
        "{ registry = \"file:///srv/oxid\", package = \"counts\", version = \"1.2.3\", }",
        "{ registry = \"file:///srv/oxid\", package = \"counts\", package = \"1.2.3\" }",
        "{ registry = \"file:///srv/oxid\", package = \"counts\", version = \"1.2.3\", features = [] }",
        "{ registry = \"https://example.invalid\", package = \"counts\", version = \"1.2.3\" }",
        "{ registry = \"file:///srv/../oxid\", package = \"counts\", version = \"1.2.3\" }",
        "{ registry = \"file:///srv/%2Foxid\", package = \"counts\", version = \"1.2.3\" }",
    ] { assert!(Dependency::parse(invalid).is_err(), "{invalid}"); }
    for version in ["0.0.0", "1.2.3", "4294967295.0.1"] {
        super::version(version).unwrap();
    }
    for version in [
        "1",
        "1.2",
        "1.2.3.4",
        "01.2.3",
        "1.2.3-alpha",
        "1.2.3+hash",
        "4294967296.0.1",
    ] {
        assert!(super::version(version).is_err(), "{version}");
    }
    let manifest = runtime::parse_typed_manifest_bytes(
        Path::new("oxid.toml"),
        b"[project]\nedition=\"typed-preview\"\n[dependencies]\na=\"1.2.3\"\nb=\"{registry}\"\n",
    )
    .unwrap();
    assert!(manifest.manifest.registry_dependencies.is_none());
    assert_eq!(manifest.manifest.dependencies["a"], "1.2.3");
    assert_eq!(manifest.manifest.dependencies["b"], "{registry}");
}

#[test]
fn canonical_registry_uri_limit_applies_after_percent_encoding() {
    assert_eq!(
        normalized_uri("file:///srv/共享 空間").unwrap(),
        "file:///srv/%E5%85%B1%E4%BA%AB%20%E7%A9%BA%E9%96%93"
    );
    assert_eq!(
        normalized_uri("file:///srv/%6Fxid").unwrap(),
        "file:///srv/oxid"
    );
    let exact = format!("file:///{}", "a".repeat(4088));
    assert_eq!(exact.len(), 4096);
    assert_eq!(normalized_uri(&exact).unwrap(), exact);
    assert!(normalized_uri(&format!("file:///{}", " ".repeat(1363))).is_err());
    assert!(normalized_uri(&format!("file:///{}", "雪".repeat(455))).is_err());
    for source in [
        "file:///",
        "file:///a//b",
        "file:///a/./b",
        "file:///a/%00b",
        "file://host/a",
        "file:///a?token=x",
    ] {
        assert!(normalized_uri(source).is_err(), "{source}");
    }
}

#[test]
fn registry_identity_is_source_name_and_version_not_artifact_hash() {
    let a = Pin {
        source: "file:///a".into(),
        name: "counts".into(),
        version: "1.0.0".into(),
        artifact: "a".repeat(64),
        bytes: 5120,
    };
    let mut b = a.clone();
    b.artifact = "b".repeat(64);
    assert_eq!(a.id(), b.id());
    b.source = "file:///b".into();
    assert_ne!(a.id(), b.id());
    b = a.clone();
    b.name = "report".into();
    assert_ne!(a.id(), b.id());
    b = a.clone();
    b.version = "1.0.1".into();
    assert_ne!(a.id(), b.id());
    let mut independent = Sha256::new();
    for value in [
        "oxid-typed-registry-package-v1",
        "file:///a",
        "counts",
        "1.0.0",
    ] {
        independent.update((value.len() as u64).to_le_bytes());
        independent.update(value.as_bytes());
    }
    assert_eq!(a.id(), format!("{:x}", independent.finalize()));
}

#[cfg(target_os = "linux")]
mod files {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Fixture {
        root: PathBuf,
        dependency: Dependency,
        archive: package::LibraryArchive,
    }
    impl Fixture {
        fn new(wide: usize, deep: usize) -> Self {
            let root = std::env::temp_dir().join(format!(
                "oxid-private-registry-unit-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            fs::create_dir(root.join("owner")).unwrap();
            let registry = root.join("registry");
            fs::create_dir(&registry).unwrap();
            fs::write(registry.join("registry.toml"), MARKER).unwrap();
            let release = registry.join("packages/counts/1.0.0");
            fs::create_dir_all(&release).unwrap();
            let mut files = vec![package::LibraryMember { path: "source/oxid.toml".into(), bytes: b"[project]\nname=\"counts\"\nversion=\"1.0.0\"\nedition=\"typed-preview\"\nkind=\"lib\"\n".to_vec() }, package::LibraryMember { path: "source/src/lib.ox".into(), bytes: b"pub fn answer()->i32{return 7;}".to_vec() }];
            for index in 0..wide {
                files.push(package::LibraryMember {
                    path: format!("source/w{index}/part.ox"),
                    bytes: b"pub fn part()->i32{return 0;}".to_vec(),
                });
            }
            if deep != 0 {
                files.push(package::LibraryMember {
                    path: format!("source/{}last.ox", "a/".repeat(deep)),
                    bytes: Vec::new(),
                });
            }
            files.sort_by(|a, b| a.path.cmp(&b.path));
            let archive = package::LibraryArchive {
                root: "source".into(),
                files,
            };
            let mut budget = Budget::new().unwrap();
            let info = package::write_library(
                &archive,
                &mut fs::File::create(release.join("artifact.tar")).unwrap(),
                &mut budget,
            )
            .unwrap();
            let record = Record {
                name: "counts".into(),
                version: "1.0.0".into(),
                entry: "src/lib.ox".into(),
                manifest: hash(
                    &archive
                        .files
                        .iter()
                        .find(|file| file.path == "source/oxid.toml")
                        .unwrap()
                        .bytes,
                ),
                artifact: info.sha256,
                bytes: info.bytes,
            };
            fs::write(
                release.join("release.toml"),
                record.encode(&mut budget).unwrap(),
            )
            .unwrap();
            let dependency = Dependency {
                source: normalized_uri(&format!("file://{}", registry.display())).unwrap(),
                name: "counts".into(),
                version: "1.0.0".into(),
            };
            Self {
                root,
                dependency,
                archive,
            }
        }
        fn resolve(&self, no_fetch: bool, budget: Allowance) -> Result<(PathBuf, Session), String> {
            let owner = self.root.join("owner");
            let mut session = Session::with_budget(&owner, no_fetch, budget)?;
            let path = session.resolve(&owner, &self.dependency)?;
            Ok((path, session))
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn typed_regular_file_growth_has_actual_exact_one_less_and_fault_admission() {
        const LIMIT: usize = 1024 * 1024;
        let fixture = Fixture::new(0, 0);
        let input = fixture.root.join("typed-input");
        let mut lengths = vec![0, 1, 105, 106, 4096, 8192, LIMIT];
        for power in 0..=20 {
            let boundary = 1usize << power;
            lengths.extend([boundary - 1, boundary, boundary + 1]);
        }
        lengths.retain(|length| *length <= LIMIT);
        lengths.sort_unstable();
        lengths.dedup();
        for length in lengths {
            let contents = vec![(length % 251) as u8; length];
            fs::write(&input, &contents).unwrap();
            let mut measured = Budget::new().unwrap();
            let before = measured.metadata;
            let bytes = measured.typed_file(&input, LIMIT, "test input").unwrap();
            assert_eq!(bytes, contents, "length {length}");
            // Independent algebraic bound: twice the maximum delivered bytes
            // (length + one overflow probe), plus two 32-byte spare requests.
            assert!(
                bytes.capacity() <= 2 * length + 66,
                "length {length}, capacity {}",
                bytes.capacity()
            );
            assert_eq!(measured.metadata - before, bytes.capacity());
            assert_eq!(measured.reserves, 1);
            let mut exact = Budget::new().unwrap();
            exact.metadata_limit = measured.peak_metadata;
            assert_eq!(
                exact.typed_file(&input, LIMIT, "test input").unwrap(),
                contents
            );
            assert_eq!(exact.peak_metadata, measured.peak_metadata);
            let mut short = Budget::new().unwrap();
            short.metadata_limit = measured.peak_metadata - 1;
            assert!(
                short
                    .typed_file(&input, LIMIT, "test input")
                    .unwrap_err()
                    .contains("metadata"),
                "length {length}"
            );
            assert_eq!(short.reserves, 0);
            let mut denied = Budget::new().unwrap();
            denied.fail_reserve = Some(0);
            assert!(
                denied
                    .typed_file(&input, LIMIT, "test input")
                    .unwrap_err()
                    .contains("injected"),
                "length {length}"
            );
            assert_eq!(fs::read(&input).unwrap(), contents);
        }
        fs::write(&input, vec![0; LIMIT + 1]).unwrap();
        let mut oversized = Budget::new().unwrap();
        let before = oversized.metadata;
        assert!(oversized
            .typed_file(&input, LIMIT, "test input")
            .unwrap_err()
            .contains("at most"));
        assert_eq!(oversized.metadata, before);
        assert_eq!(oversized.reserves, 0);
        let alias = fixture.root.join("typed-alias");
        std::os::unix::fs::symlink(&input, &alias).unwrap();
        let mut linked = Budget::new().unwrap();
        assert!(linked
            .typed_file(&alias, LIMIT + 1, "test input")
            .unwrap_err()
            .contains("regular file"));
        assert_eq!(linked.reserves, 0);
        assert!(fs::symlink_metadata(&alias)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[test]
    fn cached_wide_deep_tree_has_actual_exact_and_one_less_admission() {
        let fixture = Fixture::new(64, 20);
        let (root, _) = fixture.resolve(false, operation_budget().unwrap()).unwrap();
        let baseline = operation_budget().unwrap();
        fixture.resolve(true, baseline.clone()).unwrap();
        let exact = baseline.borrow().peak_metadata;
        let reservations = baseline.borrow().reserves;
        let paid = operation_budget().unwrap();
        paid.borrow_mut().metadata_limit = exact;
        fixture.resolve(true, paid.clone()).unwrap();
        assert_eq!(paid.borrow().peak_metadata, exact);
        let short = operation_budget().unwrap();
        short.borrow_mut().metadata_limit = exact - 1;
        assert!(fixture
            .resolve(true, short)
            .err()
            .unwrap()
            .contains("metadata"));
        // Each actual reservation site is independently denied, with the verified
        // cache left byte-identical. This is not merely a sizeof-bank assertion.
        for ordinal in 0..reservations {
            let denied = operation_budget().unwrap();
            denied.borrow_mut().fail_reserve = Some(ordinal);
            assert!(
                fixture.resolve(true, denied).is_err(),
                "reservation {ordinal}"
            );
            for file in &fixture.archive.files {
                assert_eq!(
                    fs::read(root.parent().unwrap().join(&file.path)).unwrap(),
                    file.bytes
                );
            }
        }
    }

    #[test]
    fn fresh_cache_late_origin_denial_preserves_verified_offline_reuse() {
        let baseline = Fixture::new(0, 0);
        let measured = operation_budget().unwrap();
        baseline.resolve(false, measured.clone()).unwrap();
        let last_reservation = measured.borrow().reserves.checked_sub(1).unwrap();

        // The final session-origin slot follows complete, independently checked
        // cache publication. Deny that real reservation in a separate cold tree.
        let fixture = Fixture::new(0, 0);
        let denied = operation_budget().unwrap();
        denied.borrow_mut().fail_reserve = Some(last_reservation);
        assert!(fixture
            .resolve(false, denied)
            .err()
            .unwrap()
            .contains("injected"));
        let cache = fixture.root.join("owner/.oxid/registry").join(release_id(
            &fixture.dependency.source,
            &fixture.dependency.name,
            &fixture.dependency.version,
        ));
        assert!(cache.is_dir());
        for file in &fixture.archive.files {
            assert_eq!(fs::read(cache.join(&file.path)).unwrap(), file.bytes);
        }
        let release = fixture.root.join("registry/packages/counts/1.0.0");
        let record = fs::read(release.join("release.toml")).unwrap();
        let artifact = fs::read(release.join("artifact.tar")).unwrap();
        assert_eq!(fs::read(cache.join("release.toml")).unwrap(), record);
        assert_eq!(fs::read(cache.join("artifact.tar")).unwrap(), artifact);
        assert!(!fixture.root.join("owner/oxid.lock").exists());

        fs::rename(
            fixture.root.join("registry"),
            fixture.root.join("unavailable-registry"),
        )
        .unwrap();
        let (root, session) = fixture.resolve(true, operation_budget().unwrap()).unwrap();
        assert_eq!(root, cache.join("source"));
        session
            .origin(&root)
            .unwrap()
            .source("src/lib.ox", b"pub fn answer()->i32{return 7;}")
            .unwrap();
        assert_eq!(fs::read(cache.join("release.toml")).unwrap(), record);
        assert_eq!(fs::read(cache.join("artifact.tar")).unwrap(), artifact);
        assert!(!fixture.root.join("owner/oxid.lock").exists());
    }

    #[test]
    fn offline_is_no_fetch_and_cached_bytes_cannot_be_replaced_or_aliased() {
        let fixture = Fixture::new(0, 0);
        assert!(fixture.resolve(true, operation_budget().unwrap()).is_err());
        assert!(!fixture.root.join("owner/.oxid").exists());
        let (root, session) = fixture.resolve(false, operation_budget().unwrap()).unwrap();
        let origin = session.origin(&root).unwrap();
        origin
            .source("src/lib.ox", b"pub fn answer()->i32{return 7;}")
            .unwrap();
        assert!(origin
            .source("src/lib.ox", b"pub fn answer()->i32{return 8;}")
            .is_err());
        fs::rename(
            fixture.root.join("registry"),
            fixture.root.join("offline-registry"),
        )
        .unwrap();
        fixture.resolve(true, operation_budget().unwrap()).unwrap();
        let source = root.join("src/lib.ox");
        let original = fs::read(&source).unwrap();
        fs::write(&source, b"edited").unwrap();
        assert!(fixture.resolve(true, operation_budget().unwrap()).is_err());
        assert_eq!(fs::read(&source).unwrap(), b"edited");
        fs::write(&source, original).unwrap();
        fs::hard_link(&source, fixture.root.join("alias")).unwrap();
        assert!(fixture.resolve(true, operation_budget().unwrap()).is_err());
        assert!(fixture.root.join("alias").exists());
    }

    #[test]
    fn marker_record_and_artifact_require_single_link_regular_files() {
        for name in [
            "registry.toml",
            "packages/counts/1.0.0/release.toml",
            "packages/counts/1.0.0/artifact.tar",
        ] {
            let fixture = Fixture::new(0, 0);
            let input = fixture.root.join("registry").join(name);
            fs::hard_link(&input, fixture.root.join("alias")).unwrap();
            assert!(
                fixture.resolve(false, operation_budget().unwrap()).is_err(),
                "{name}"
            );
            assert!(!fixture.root.join("owner/.oxid").exists());
        }
    }

    #[test]
    fn registry_lock_decode_selects_one_shared_allowance_before_new_pin_storage() {
        let fixture = Fixture::new(0, 0);
        let allowance = operation_budget().unwrap();
        let (library, session) = fixture.resolve(false, allowance.clone()).unwrap();
        let owner = fixture.root.join("owner");
        let graph = [
            crate::frontend::PackageSpec {
                root: owner.clone(),
                entry: owner.join("main.ox"),
                dependencies: vec![crate::frontend::PackageDependency {
                    alias: "counts".into(),
                    package: 1,
                }],
            },
            crate::frontend::PackageSpec {
                root: library.clone(),
                entry: library.join("src/lib.ox"),
                dependencies: vec![],
            },
        ];
        let manifests = [
            runtime::parse_typed_manifest_bytes(
                Path::new("oxid.toml"),
                b"[project]\nname=\"application\"\nversion=\"0.1.0\"\nedition=\"typed-preview\"\n",
            )
            .unwrap(),
            runtime::parse_typed_manifest_bytes(
                Path::new("oxid.toml"),
                &fixture
                    .archive
                    .files
                    .iter()
                    .find(|file| file.path == "source/oxid.toml")
                    .unwrap()
                    .bytes,
            )
            .unwrap(),
        ];
        let fingerprints = [(1, format!("sha256:{}", "d".repeat(64)))];
        let origin = session.origin(&library).unwrap();
        let snapshot = crate::project_lock::expected(&graph, &manifests, &fingerprints)
            .unwrap()
            .with_registries(
                &owner,
                std::iter::once((library.as_path(), &origin.pin, &allowance)),
            )
            .unwrap();
        crate::project_lock::publish(&owner, &snapshot, None, false).unwrap();
        let original = fs::read(owner.join("oxid.lock")).unwrap();
        let mut selected = None;
        let admitted = crate::project_lock::read_with_registry(&owner, true, false, &mut selected)
            .unwrap()
            .unwrap();
        assert_eq!(admitted.bytes(), original);
        let baseline = selected.unwrap();
        let mut exact = Some(operation_budget().unwrap());
        exact.as_ref().unwrap().borrow_mut().metadata_limit = baseline.borrow().peak_metadata;
        let retained = exact.as_ref().unwrap().clone();
        crate::project_lock::read_with_registry(&owner, true, false, &mut exact).unwrap();
        assert!(Rc::ptr_eq(exact.as_ref().unwrap(), &retained));
        let mut short = Some(operation_budget().unwrap());
        short.as_ref().unwrap().borrow_mut().metadata_limit = baseline.borrow().peak_metadata - 1;
        assert!(
            crate::project_lock::read_with_registry(&owner, true, false, &mut short)
                .err()
                .unwrap()
                .contains("metadata")
        );
        assert_eq!(fs::read(owner.join("oxid.lock")).unwrap(), original);
        // The retained decoder accepts empty lines and CRLF. Ledger selection
        // follows that grammar even across multiple fixed input-buffer fills.
        for bytes in [
            [vec![b'\n'; 9000], original.clone()].concat(),
            String::from_utf8(original.clone())
                .unwrap()
                .replace('\n', "\r\n")
                .into_bytes(),
        ] {
            fs::write(owner.join("oxid.lock"), &bytes).unwrap();
            let mut context = None;
            let read = crate::project_lock::read_with_registry(&owner, true, false, &mut context)
                .unwrap()
                .unwrap();
            assert!(context.is_some());
            assert_eq!(read.bytes(), bytes);
        }
        // The predecessor local lock and absent-lock paths allocate no registry
        // session. Their original bytes and decoder remain authoritative.
        let local_graph = [crate::frontend::PackageSpec {
            root: owner.clone(),
            entry: owner.join("main.ox"),
            dependencies: vec![],
        }];
        let local = crate::project_lock::expected(&local_graph, &manifests[..1], &[]).unwrap();
        fs::remove_file(owner.join("oxid.lock")).unwrap();
        crate::project_lock::publish(&owner, &local, None, false).unwrap();
        let mut no_registry = None;
        crate::project_lock::read_with_registry(&owner, true, false, &mut no_registry).unwrap();
        assert!(no_registry.is_none());
    }
}
