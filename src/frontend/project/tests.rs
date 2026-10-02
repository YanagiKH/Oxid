use super::*;
use std::{
    fs,
    sync::atomic::{AtomicUsize, Ordering},
};

static SERIAL: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "oxid-unit1-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, path: &str, bytes: impl AsRef<[u8]>) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
    fn entry(&self) -> String {
        self.0
            .join("app.ox")
            .into_os_string()
            .into_string()
            .unwrap()
    }
    #[allow(clippy::result_large_err)] // Test-only trace is deliberately retained by value.
    fn load(&self, limits: ProjectLimits) -> Result<ProjectSources, LoadFailure> {
        ProjectSources::load_modules(&self.entry(), limits)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn error(
    fixture: &Fixture,
    limits: ProjectLimits,
    code: &str,
    stage: &str,
    spelling: &str,
) -> LoadFailure {
    let failure = fixture.load(limits).unwrap_err();
    assert_eq!(failure.diagnostics[0].code, code);
    assert_eq!(failure.diagnostics[0].stage, stage);
    let primary = failure.diagnostics[0]
        .primary
        .expect("module diagnostic has declaring-file origin");
    assert_eq!(failure.sources.text(primary), spelling);
    failure
}

#[test]
fn original_facade_has_no_module_path_admission_and_preserves_bytes() {
    let f = Fixture::new();
    f.write(
        "app.ox",
        "// é\r\nfn crate() -> i32 { return 7; }\nfn as() -> () { return; }",
    );
    let limits = ProjectLimits {
        path_bytes: 0,
        component_bytes: 0,
        relative_bytes: 0,
        probes: 0,
        directory_entries: 0,
        directory_name_units: 0,
        ..ProjectLimits::default()
    };
    for project in [
        ProjectSources::load_original(&f.entry(), limits).unwrap(),
        f.load(limits).unwrap(),
    ] {
        assert_eq!(project.syntax_flavor(), SyntaxFlavor::OriginalSingleFile);
        assert!(project.canonical_root.is_none());
        assert!(project.modules[0].canonical_path.is_none());
        assert_eq!(project.usage.probes, 0);
        let (source, ast) = project.original_file().unwrap();
        assert_eq!(source.path(), f.entry());
        assert_eq!(source.text().as_bytes(), fs::read(f.entry()).unwrap());
        assert_eq!(ast.functions.len(), 2);
        assert_eq!(
            project.function_handles().collect::<Vec<_>>(),
            [
                FunctionAstKey {
                    file: SourceFileId(0),
                    index: 0
                },
                FunctionAstKey {
                    file: SourceFileId(0),
                    index: 1
                }
            ]
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn private_loader_assigns_dfs_files_and_module_major_original_handles() {
    let f = Fixture::new();
    f.write(
        "app.ox",
        "fn r0()->(){return;} mod a; fn r1()->(){return;} pub mod b;",
    );
    f.write("a.ox", "fn a0()->(){return;} mod c; fn a1()->(){return;}");
    f.write("a/c.ox", "fn c0()->(){return;}");
    f.write("b.ox", "fn b0()->(){return;}");
    let project = f.load(ProjectLimits::default()).unwrap();
    assert_eq!(project.syntax_flavor(), SyntaxFlavor::ProjectSyntax);
    assert_eq!(
        project
            .modules
            .iter()
            .map(|m| m.relative_path.as_str())
            .collect::<Vec<_>>(),
        ["", "a.ox", "a/c.ox", "b.ox"]
    );
    assert_eq!(
        project.modules.iter().map(|m| m.depth).collect::<Vec<_>>(),
        [0, 1, 2, 1]
    );
    let handles = project.function_handles().collect::<Vec<_>>();
    assert_eq!(
        handles
            .iter()
            .map(|&h| project.text(project.try_function(h).unwrap().name))
            .collect::<Vec<_>>(),
        ["r0", "r1", "a0", "a1", "c0", "b0"]
    );
    assert_eq!(
        handles.iter().map(|h| h.file.0).collect::<Vec<_>>(),
        [0, 0, 1, 1, 2, 3]
    );
    assert!(project.modules[3].public.is_some());
    assert!(project.original_file().is_none());
    assert_eq!(project.usage.probes, 4);
}

#[cfg(target_os = "linux")]
#[test]
fn file_and_function_handles_do_not_flatten_local_arenas() {
    let f = Fixture::new();
    f.write("app.ox", "fn r()->i32{return 1;} mod a;");
    f.write("a.ox", "fn s()->i32{return 2;}");
    let p = f.load(ProjectLimits::default()).unwrap();
    for (file, value) in [(0, "1"), (1, "2")] {
        let key = ExprKey {
            file: SourceFileId(file),
            expression: ast::ExprId(0),
        };
        let expression = p.try_expression(key).unwrap();
        assert_eq!(p.text(expression.span), value);
        let function = FunctionAstKey {
            file: SourceFileId(file),
            index: 0,
        };
        assert_eq!(
            p.try_block(BlockKey {
                function,
                block: ast::BodyBlockId(0)
            })
            .unwrap()
            .span
            .file,
            SourceFileId(file)
        );
    }
    assert!(p
        .try_function(FunctionAstKey {
            file: SourceFileId(9),
            index: 0
        })
        .is_none());
    assert!(p
        .try_function(FunctionAstKey {
            file: SourceFileId(1),
            index: 1
        })
        .is_none());
    assert!(p
        .try_expression(ExprKey {
            file: SourceFileId(1),
            expression: ast::ExprId(1)
        })
        .is_none());
    assert!(p
        .try_block(BlockKey {
            function: FunctionAstKey {
                file: SourceFileId(1),
                index: 0
            },
            block: ast::BodyBlockId(1)
        })
        .is_none());
}

#[test]
fn source_access_checks_owner_unicode_offsets_and_independent_labels() {
    let mut sources = SourceMap::new();
    let a = sources.add("a.ox".into(), "aé\r\nz".into());
    let b = sources.add("b.ox".into(), "xyz\nz".into());
    let at = sources.get(a).span(1, 3);
    let bt = sources.get(b).span(1, 3);
    assert_eq!(sources.text(at), "é");
    assert_eq!(sources.text(bt), "yz");
    assert!(sources.get(a).try_text(bt).is_none());
    assert!(sources
        .try_text(Span {
            file: a,
            start: 2,
            end: 3
        })
        .is_none());
    assert!(sources
        .try_text(Span {
            file: b,
            start: 3,
            end: 2
        })
        .is_none());
    assert!(sources
        .try_text(Span {
            file: SourceFileId(55),
            start: 0,
            end: 0
        })
        .is_none());
    assert_eq!(sources.get(a).location(5), (2, 1));
    let d = Diagnostic::new("E0201", "resolve", "two files", Some(at)).secondary(bt, "other file");
    let json = d.render_json(&sources);
    assert!(json.contains("a.ox"));
    assert!(json.contains("b.ox"));
}

#[test]
fn public_parser_keeps_all_project_syntax_closed() {
    for text in [
        "mod missing;",
        "pub mod missing;",
        "use crate::x;",
        "pub fn main()->(){return;}",
        "fn main()->(){crate::f();return;}",
    ] {
        let f = Fixture::new();
        f.write("app.ox", text);
        let failure =
            ProjectSources::load_original(&f.entry(), ProjectLimits::default()).unwrap_err();
        assert_eq!(failure.sources.files().len(), 1);
        assert!(failure.diagnostics.iter().all(|d| d.stage == "parse"));
        assert_eq!(failure.usage.probes, 0);
    }
}

#[cfg(target_os = "linux")]
#[test]
fn parse_errors_prevent_child_reads_and_child_errors_precede_later_siblings() {
    let f = Fixture::new();
    f.write("app.ox", "mod missing; fn broken");
    let fail = f.load(ProjectLimits::default()).unwrap_err();
    assert_eq!(fail.diagnostics[0].stage, "parse");
    assert_eq!(fail.usage.probes, 0);
    f.write("app.ox", "mod a; mod missing;");
    f.write("a.ox", "/*");
    let fail = error(&f, ProjectLimits::default(), "E0100", "lex", "/*");
    assert_eq!(fail.diagnostics[0].primary.unwrap().file, SourceFileId(1));
    assert_eq!(fail.sources.files().len(), 2);
}

#[cfg(target_os = "linux")]
#[test]
fn aggregate_source_token_and_node_boundaries_include_trivia_but_not_eof() {
    let f = Fixture::new();
    f.write("app.ox", "mod a;");
    f.write("a.ox", "fn f()->i32{return 7;}");
    let bytes = 6 + 22;
    let base = ProjectLimits {
        source_bytes: bytes,
        tokens: 17,
        nodes: 4,
        ..ProjectLimits::default()
    };
    let p = f.load(base).unwrap();
    assert_eq!(
        (
            p.usage.source_bytes,
            p.usage.non_eof_tokens,
            p.usage.syntax_nodes
        ),
        (28, 17, 4)
    );
    assert_eq!(p.programs.iter().map(|a| a.tokens.len()).sum::<usize>(), 19);
    error(
        &f,
        ProjectLimits {
            source_bytes: bytes - 1,
            ..base
        },
        "E0400",
        "source",
        "a",
    );
    error(
        &f,
        ProjectLimits { tokens: 16, ..base },
        "E0400",
        "lex",
        "}",
    );
    error(
        &f,
        ProjectLimits { nodes: 3, ..base },
        "E0400",
        "parse",
        "7",
    );
}

#[cfg(target_os = "linux")]
#[test]
fn child_encoding_and_oxbc_errors_keep_declaring_file_origin() {
    let f = Fixture::new();
    f.write("app.ox", "mod a;");
    for (bytes, code) in [
        (b"OXBC\xff".as_slice(), "E0004"),
        (b"\xff".as_slice(), "E0003"),
    ] {
        f.write("a.ox", bytes);
        let failure = error(&f, ProjectLimits::default(), code, "source", "a");
        assert_eq!(failure.sources.files().len(), 1);
    }
}

#[cfg(target_os = "linux")]
#[test]
fn duplicate_and_case_collisions_are_discovered_in_dfs_order() {
    let f = Fixture::new();
    f.write("a.ox", "");
    f.write("app.ox", "mod a; mod a;");
    let failure = error(&f, ProjectLimits::default(), "E0201", "resolve", "a");
    assert_eq!(failure.diagnostics[0].primary.unwrap().start, 11);
    assert_eq!(failure.diagnostics[0].secondary[0].0.start, 4);
    f.write("app.ox", "mod a; mod A;");
    error(&f, ProjectLimits::default(), "E0005", "source", "A");
    f.write("a.ox", "mod missing;");
    error(&f, ProjectLimits::default(), "E0002", "source", "missing");
}

#[cfg(target_os = "linux")]
#[test]
fn exact_case_wins_over_folded_entries_but_scan_caps_hide_partial_facts() {
    let f = Fixture::new();
    f.write("app.ox", "mod a;");
    f.write("a.ox", "");
    f.write("A.ox", "invalid");
    let p = f.load(ProjectLimits::default()).unwrap();
    assert_eq!(p.usage.directory_entries, 3);
    assert_eq!(p.usage.directory_name_units, 14); // app.ox + a.ox + A.ox
    for limits in [
        ProjectLimits {
            directory_entries: 2,
            ..ProjectLimits::default()
        },
        ProjectLimits {
            directory_name_units: 13,
            ..ProjectLimits::default()
        },
    ] {
        let fail = error(&f, limits, "E0400", "source-project", "a");
        assert_eq!(
            fail.diagnostics[0].message,
            "module directory scan budget exceeded"
        );
    }
    fs::remove_file(f.0.join("a.ox")).unwrap();
    error(&f, ProjectLimits::default(), "E0005", "source", "a");
    fs::remove_file(f.0.join("A.ox")).unwrap();
    error(&f, ProjectLimits::default(), "E0002", "source", "a");
}

#[cfg(target_os = "linux")]
#[test]
fn symlink_entry_uses_invoked_directory_and_repeated_identity_has_root_secondary() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    f.write("else/app.ox", "mod a;");
    f.write("view/a.ox", "fn found()->(){return;}");
    f.write("else/a.ox", "invalid");
    symlink(f.0.join("else/app.ox"), f.0.join("view/app.ox")).unwrap();
    let entry = f.0.join("view/app.ox").to_str().unwrap().to_string();
    let p = ProjectSources::load_modules(&entry, ProjectLimits::default()).unwrap();
    assert_eq!(p.sources.get(SourceFileId(0)).path(), entry);
    assert!(p.sources.get(SourceFileId(1)).path().ends_with("view/a.ox"));
    f.write("a.ox", "mod a;");
    symlink(f.0.join("a.ox"), f.0.join("app.ox")).unwrap();
    let fail = error(&f, ProjectLimits::default(), "E0005", "source", "a");
    assert_eq!(
        fail.diagnostics[0].secondary[0].0,
        fail.sources.get(SourceFileId(0)).span(6, 6)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn module_symlinks_directories_and_indirection_are_rejected_hardlinks_distinct() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    f.write("app.ox", "mod a;");
    f.write("target.ox", "");
    symlink(f.0.join("target.ox"), f.0.join("a.ox")).unwrap();
    error(&f, ProjectLimits::default(), "E0005", "source", "a");
    fs::remove_file(f.0.join("a.ox")).unwrap();
    fs::create_dir(f.0.join("a.ox")).unwrap();
    error(&f, ProjectLimits::default(), "E0005", "source", "a");
    fs::remove_dir(f.0.join("a.ox")).unwrap();
    f.write("a.ox", "mod b;");
    f.write("other/b.ox", "");
    symlink(f.0.join("other"), f.0.join("a")).unwrap();
    error(&f, ProjectLimits::default(), "E0005", "source", "b");
    f.write("a.ox", "struct Item {} fn f()->(){return;}");
    fs::hard_link(f.0.join("a.ox"), f.0.join("b.ox")).unwrap();
    f.write("app.ox", "mod a; mod b;");
    let p = f.load(ProjectLimits::default()).unwrap();
    assert_ne!(p.modules[1].canonical_path, p.modules[2].canonical_path);
    assert_eq!(
        p.record_handles().map(|h| h.file.0).collect::<Vec<_>>(),
        [1, 2]
    );
}

#[cfg(target_os = "linux")]
#[test]
fn lower_module_depth_relative_path_probe_and_retained_payload_limits() {
    let f = Fixture::new();
    f.write("app.ox", "mod a;");
    f.write("a.ox", "mod b;");
    f.write("a/b.ox", "");
    let p = f.load(ProjectLimits::default()).unwrap();
    assert_eq!(p.usage.probes, 3);
    let exact = ProjectLimits {
        modules: 3,
        depth: 2,
        relative_bytes: 6,
        probes: 3,
        path_bytes: p.usage.retained_path_bytes,
        ..ProjectLimits::default()
    };
    f.load(exact).unwrap();
    for limits in [
        ProjectLimits {
            modules: 2,
            ..exact
        },
        ProjectLimits { depth: 1, ..exact },
        ProjectLimits {
            relative_bytes: 5,
            ..exact
        },
        ProjectLimits { probes: 2, ..exact },
        ProjectLimits {
            path_bytes: exact.path_bytes - 1,
            ..exact
        },
    ] {
        error(&f, limits, "E0400", "source-project", "b");
    }
    error(
        &f,
        ProjectLimits {
            component_bytes: 0,
            ..exact
        },
        "E0400",
        "source-project",
        "a",
    );
}

#[test]
fn checked_preflight_arithmetic_wins_before_caps_without_reserving() {
    let limits = ProjectLimits {
        modules: 0,
        ..ProjectLimits::default()
    };
    for (usage, depth, prefix, component, display, canonical) in [
        (
            SourceUsage {
                modules: usize::MAX,
                ..SourceUsage::default()
            },
            0,
            0,
            1,
            0,
            1,
        ),
        (SourceUsage::default(), usize::MAX, 0, 1, 0, 1),
        (SourceUsage::default(), 0, usize::MAX, 1, 0, 1),
        (SourceUsage::default(), 0, 0, usize::MAX, 0, 1),
        (SourceUsage::default(), 0, 0, 1, usize::MAX, 1),
        (SourceUsage::default(), 0, 0, 1, 0, usize::MAX),
        (
            SourceUsage {
                retained_path_bytes: usize::MAX,
                ..SourceUsage::default()
            },
            0,
            0,
            1,
            0,
            1,
        ),
        (
            SourceUsage {
                probes: usize::MAX,
                ..SourceUsage::default()
            },
            0,
            0,
            1,
            0,
            1,
        ),
    ] {
        let e = ChildPlan::new(
            usage, limits, depth, prefix, component, display, canonical, None,
        )
        .unwrap_err();
        assert_eq!(e.message, "project source count overflow");
    }
    let mut allocator = Allocator::default();
    let mut vector = Vec::<usize>::new();
    assert!(allocator
        .vector(&mut vector, usize::MAX, "overflow")
        .is_err());
    assert_eq!(allocator.attempts, 0);
    assert_eq!(vector.capacity(), 0);
}

#[test]
fn every_new_original_reserve_has_handled_failure_and_honest_origin() {
    let f = Fixture::new();
    f.write("app.ox", "fn main()->(){return;}");
    let mut allocator = Allocator::default();
    ProjectSources::load(
        &f.entry(),
        ProjectLimits::default(),
        parser::SourceMode::OwnedCandidate,
        &mut allocator,
    )
    .unwrap();
    assert_eq!(allocator.attempts, 6);
    for fail_at in 1..=allocator.attempts {
        let mut injected = Allocator {
            fail_at: Some(fail_at),
            ..Allocator::default()
        };
        let failure = ProjectSources::load(
            &f.entry(),
            ProjectLimits::default(),
            parser::SourceMode::OwnedCandidate,
            &mut injected,
        )
        .unwrap_err();
        let d = &failure.diagnostics[0];
        assert_eq!((d.code, d.stage), ("E0400", "source-project"));
        assert_eq!(failure.allocator.attempts, fail_at);
        assert!(!failure.allocator.trace.last().unwrap().success);
        if fail_at <= 4 {
            assert!(d.primary.is_none());
            assert!(failure.sources.files().is_empty());
        } else {
            assert_eq!(
                d.primary,
                Some(failure.sources.get(SourceFileId(0)).span(22, 22))
            );
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
fn every_new_module_reserve_fails_without_subsequent_reserves() {
    let f = Fixture::new();
    f.write("app.ox", "pub mod a;");
    f.write("a.ox", "mod b;");
    f.write("a/b.ox", "fn f()->(){return;}");
    let mut allocator = Allocator::default();
    ProjectSources::load(
        &f.entry(),
        ProjectLimits::default(),
        parser::SourceMode::ModuleCandidate,
        &mut allocator,
    )
    .unwrap();
    let mut kinds = std::collections::BTreeSet::new();
    for event in &allocator.trace {
        assert!(event.length.checked_mul(event.element_bytes).is_some());
        kinds.insert(event.kind);
    }
    assert!(kinds.contains("module declarations"));
    assert!(kinds.contains("module items"));
    assert!(kinds.contains("module probe path"));
    for fail_at in 1..=allocator.attempts {
        let mut injected = Allocator {
            fail_at: Some(fail_at),
            ..Allocator::default()
        };
        let failure = ProjectSources::load(
            &f.entry(),
            ProjectLimits::default(),
            parser::SourceMode::ModuleCandidate,
            &mut injected,
        )
        .unwrap_err();
        assert_eq!(failure.diagnostics[0].code, "E0400");
        assert_eq!(failure.allocator.attempts, fail_at);
        assert!(failure.allocator.trace[..fail_at - 1]
            .iter()
            .all(|e| e.success));
        assert!(!failure.allocator.trace.last().unwrap().success);
    }
}

#[test]
fn newline_dense_source_inventory_uses_bytes_not_token_count() {
    let f = Fixture::new();
    let chunk = "\n".repeat(60_000) + "//x\n";
    f.write("app.ox", chunk.repeat(17));
    let p = f.load(ProjectLimits::default()).unwrap();
    assert_eq!(p.usage.source_bytes, 1_020_068);
    assert_eq!(p.usage.line_starts, 1_020_018);
    assert_eq!(p.usage.non_eof_tokens, 35);
    assert_eq!(p.usage.syntax_nodes, 0);
    assert_eq!(
        p.inventory().unwrap().line_starts,
        1_020_018 * size_of::<usize>()
    );
    assert!(p.inventory().unwrap().line_starts > 8 * p.usage.non_eof_tokens);
}

#[cfg(target_os = "linux")]
#[test]
fn measured_layout_and_requested_inventory() {
    macro_rules! sizes { ($($ty:ty),* $(,)?) => { $(println!("layout {} {}", stringify!($ty), size_of::<$ty>());)* }; }
    sizes!(
        SourceMap,
        super::super::source::SourceFile,
        Span,
        lexer::Token,
        ast::Program,
        ast::Function,
        ast::Param,
        ast::BodyBlock,
        ast::Stmt,
        ast::StructDecl,
        ast::StructField,
        ast::Expr,
        ast::Argument,
        ast::FieldInit,
        ast::ItemId,
        ast::ModuleDecl,
        ModuleHeader,
        ProjectSources,
        FunctionAstKey,
        RecordAstKey,
        ExprKey,
        BlockKey,
        Frame,
        SourceUsage
    );
    let f = Fixture::new();
    f.write("app.ox", "mod a;");
    f.write("a.ox", "fn f()->i32{return 7;}");
    let p = f.load(ProjectLimits::default()).unwrap();
    println!("usage {:?}", p.usage);
    println!("inventory {:?}", p.inventory().unwrap());
    assert_eq!(
        p.inventory().unwrap().tokens,
        19 * size_of::<lexer::Token>()
    );
}

#[test]
fn scan_iterator_admission_precedes_io_and_hides_partial_case_facts() {
    use std::ffi::OsString;
    let at = Span {
        file: SourceFileId(0),
        start: 0,
        end: 1,
    };
    let bad = || Err(io::Error::other("controlled read_dir failure"));
    let mut usage = SourceUsage::default();
    let cap = ProjectLimits {
        directory_entries: 0,
        ..ProjectLimits::default()
    };
    let e = filesystem::scan_entries([bad()], "a", cap, &mut usage, at).unwrap_err();
    assert_eq!(
        (e.code, e.stage, e.message.as_str()),
        (
            "E0400",
            "source-project",
            "module directory scan budget exceeded"
        )
    );
    assert_eq!(usage.directory_entries, 1);
    let e = filesystem::scan_entries(
        [bad()],
        "a",
        ProjectLimits::default(),
        &mut SourceUsage::default(),
        at,
    )
    .unwrap_err();
    assert_eq!((e.code, e.stage), ("E0002", "source"));
    let mut overflowing = SourceUsage {
        directory_entries: usize::MAX,
        ..SourceUsage::default()
    };
    assert_eq!(
        filesystem::scan_entries([bad()], "a", cap, &mut overflowing, at)
            .unwrap_err()
            .message,
        "project source count overflow"
    );
    let mut overflowing = SourceUsage {
        directory_name_units: usize::MAX,
        ..SourceUsage::default()
    };
    assert_eq!(
        filesystem::scan_entries([Ok(OsString::from("a"))], "a", cap, &mut overflowing, at)
            .unwrap_err()
            .message,
        "project source count overflow"
    );
    for entries in [["a", "A", "extra"], ["extra", "A", "a"]] {
        let e = filesystem::scan_entries(
            entries.map(|s| Ok(OsString::from(s))),
            "a",
            ProjectLimits {
                directory_entries: 2,
                ..ProjectLimits::default()
            },
            &mut SourceUsage::default(),
            at,
        )
        .unwrap_err();
        assert_eq!(e.message, "module directory scan budget exceeded");
    }
}

#[test]
fn first_module_dimensions_precede_root_canonical_path_admission() {
    let f = Fixture::new();
    f.write("app.ox", "mod missing;");
    let tiny = ProjectLimits {
        modules: 1,
        depth: 0,
        component_bytes: 0,
        relative_bytes: 0,
        path_bytes: 0,
        probes: 0,
        ..ProjectLimits::default()
    };
    for (limits, expected) in [
        (tiny, "module count limit exceeded"),
        (
            ProjectLimits { modules: 2, ..tiny },
            "module depth limit exceeded",
        ),
        (
            ProjectLimits {
                modules: 2,
                depth: 1,
                ..tiny
            },
            "module component limit exceeded",
        ),
        (
            ProjectLimits {
                modules: 2,
                depth: 1,
                component_bytes: 7,
                ..tiny
            },
            "module relative path limit exceeded",
        ),
    ] {
        let fail = error(&f, limits, "E0400", "source-project", "missing");
        assert_eq!(fail.diagnostics[0].message, expected);
        assert_eq!(fail.usage.probes, 0);
    }
}

#[cfg(not(target_os = "linux"))]
#[test]
fn private_module_policy_is_explicit_on_unqualified_hosts() {
    let f = Fixture::new();
    f.write("app.ox", "mod a;");
    let failure = error(&f, ProjectLimits::default(), "E0005", "source", "a");
    assert_eq!(
        failure.diagnostics[0].message,
        "module source policy is not qualified on this host"
    );
    assert_eq!(failure.usage.probes, 0);
}

#[test]
fn direct_scalar_and_public_owned_diagnostic_schedules_stay_distinct() {
    let f = Fixture::new();
    f.write("app.ox", "fn x()->Missing{return;} fn x()->Other{return;}");
    let p = ProjectSources::load_original(&f.entry(), ProjectLimits::default()).unwrap();
    let (source, ast) = p.original_file().unwrap();
    let scalar = super::super::hir::resolve(source, ast).unwrap_err();
    assert_eq!(
        scalar
            .iter()
            .map(|d| (d.code, p.text(d.primary.unwrap())))
            .collect::<Vec<_>>(),
        [("E0202", "Missing"), ("E0201", "x"), ("E0202", "Other")]
    );
    // Nominal-looking signatures select owned publicly: declaration conflicts first.
    let public = super::super::oir::check_source(source, ast, p.sources()).unwrap_err();
    assert_eq!(public.len(), 1);
    assert_eq!(
        (public[0].code, p.text(public[0].primary.unwrap())),
        ("E0201", "x")
    );
}

#[test]
fn default_token_limit_is_aggregate_and_retains_one_eof_per_file() {
    let f = Fixture::new();
    f.write("app.ox", "//\n".repeat(50_000));
    let p = f.load(ProjectLimits::default()).unwrap();
    assert_eq!(p.usage.non_eof_tokens, 100_000);
    assert_eq!(p.programs[0].tokens.len(), 100_001);
    f.write("app.ox", "//\n".repeat(50_000) + "//");
    let failure = f.load(ProjectLimits::default()).unwrap_err();
    assert_eq!(
        (failure.diagnostics[0].code, failure.diagnostics[0].stage),
        ("E0400", "lex")
    );
}

#[test]
fn reserve_arithmetic_overflow_is_not_reported_as_an_allocation_failure() {
    let mut allocator = Allocator::default();
    assert_eq!(
        allocator.vector(&mut Vec::<usize>::new(), usize::MAX, "overflow"),
        Err(ReserveFailure::Overflow)
    );
    assert_eq!(allocator.attempts, 0);
    let mut allocator = Allocator {
        fail_at: Some(1),
        ..Allocator::default()
    };
    assert_eq!(
        allocator.vector(&mut Vec::<usize>::new(), 1, "injected"),
        Err(ReserveFailure::Allocation)
    );
    assert_eq!(allocator.attempts, 1);
    let f = Fixture::new();
    f.write("app.ox", "fn f()->(){return;}");
    let mut allocator = Allocator {
        attempts: usize::MAX,
        ..Allocator::default()
    };
    let failure = ProjectSources::load(
        &f.entry(),
        ProjectLimits::default(),
        parser::SourceMode::OwnedCandidate,
        &mut allocator,
    )
    .unwrap_err();
    let d = &failure.diagnostics[0];
    assert_eq!(
        (d.code, d.stage, d.message.as_str()),
        ("E0400", "source-project", "project source count overflow")
    );
    assert!(failure.allocator.trace.is_empty());
    assert!(d.primary.is_none());
}
