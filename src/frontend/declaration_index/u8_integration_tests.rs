//! U8-INDEX-INTEGRATION-1: exercise the single production collect/finish path.
use super::tests::Fixture;
use super::*;

// Core index assertions run on every host without relaxing filesystem policy.
// On Linux, also run every case through the real supported source loader.
fn module_sources(files: &[(&str, &str)]) -> Vec<ProjectSources> {
    let sources = vec![ProjectSources::from_u8_index_test_files(files)];
    #[cfg(target_os = "linux")]
    let sources = {
        let mut sources = sources;
        let loaded = Fixture::new(files).load();
        let memory = &sources[0];
        assert_eq!(memory.modules().len(), loaded.modules().len());
        for (a, b) in memory.modules().iter().zip(loaded.modules()) {
            assert_eq!(
                (
                    a.file,
                    a.parent,
                    a.declaration,
                    a.public,
                    a.depth,
                    &a.relative_path
                ),
                (
                    b.file,
                    b.parent,
                    b.declaration,
                    b.public,
                    b.depth,
                    &b.relative_path
                )
            );
            let a_source = memory.sources().get(a.file);
            let b_source = loaded.sources().get(b.file);
            assert_eq!(a_source.text(), b_source.text());
            let a_ast = memory.try_file_ast(a.file).unwrap();
            let b_ast = loaded.try_file_ast(b.file).unwrap();
            assert!(a_ast.belongs_to(a_source));
            assert!(b_ast.belongs_to(b_source));
            assert!(!a_ast.belongs_to(b_source));
            assert!(!b_ast.belongs_to(a_source));
            assert!(a_ast.validate_spans_and_ids(|at| memory.try_text(at).is_some()));
            assert!(b_ast.validate_spans_and_ids(|at| loaded.try_text(at).is_some()));
        }
        sources.push(loaded);
        sources
    };
    sources
}

fn build<'s>(
    owner: SourceOwner<'s>,
    work: &WorkMeter,
) -> Result<DeclarationIndex<'s>, Vec<Diagnostic>> {
    let mut allocator = Allocator::default();
    collect_originals(owner, IndexLimits::default(), work, &mut allocator)
        .map_err(|e| vec![*e])?
        .finish(work, &mut allocator)
}

#[test]
fn in_memory_module_fixture_preserves_order_spans_and_source_identity() {
    let files = [
        ("main.ox", "pub mod z; mod a; use crate::z::T as U;"),
        ("a.ox", "pub struct A {}"),
        ("z.ox", "pub struct T {}"),
    ];
    let sources = ProjectSources::from_u8_index_test_files(&files);
    let other = ProjectSources::from_u8_index_test_files(&files);
    assert_eq!(sources.modules().len(), 3);
    for (index, (name, declaration, public)) in [
        ("main.ox", None, None),
        ("z.ox", Some((8, 9)), Some((0, 3))),
        ("a.ox", Some((15, 16)), None),
    ]
    .into_iter()
    .enumerate()
    {
        let id = SourceFileId(index);
        let header = &sources.modules()[index];
        assert_eq!(header.file, id);
        assert_eq!(header.parent, (index != 0).then_some(ModuleId(0)));
        assert_eq!(header.depth, usize::from(index != 0));
        let origin = |(start, end)| Span {
            file: SourceFileId(0),
            start,
            end,
        };
        assert_eq!(header.declaration, declaration.map(origin));
        assert_eq!(header.public, public.map(origin));
        let source = sources.sources().get(id);
        assert_eq!(
            source.text(),
            files.iter().find(|(path, _)| *path == name).unwrap().1
        );
        let ast = sources.try_file_ast(id).unwrap();
        assert!(ast.belongs_to(source));
        assert!(!ast.belongs_to(other.sources().get(id)));
        assert!(ast.validate_spans_and_ids(|at| sources.try_text(at).is_some()));
    }
    build(SourceOwner::project(&sources), &WorkMeter::default()).unwrap();
}

#[test]
fn in_memory_module_fixture_rejects_inexact_inputs() {
    for files in [
        vec![("main.ox", "mod ;"), ("m.ox", "")],
        vec![("main.ox", "mod m;"), ("m.ox", "pub struct {")],
        vec![("main.ox", "mod m;"), ("m.ox", "\"")],
        vec![
            ("main.ox", "mod m;"),
            ("m.ox", "mod nested;"),
            ("m/nested.ox", ""),
        ],
        vec![("main.ox", "mod m;"), ("m.ox", ""), ("unused.ox", "")],
        vec![("main.ox", "mod m;"), ("wrong.ox", "")],
        vec![("main.ox", "mod m;"), ("m.ox", ""), ("m.ox", "")],
        vec![("main.ox", "mod m; mod m;"), ("m.ox", "")],
        vec![
            ("main.ox", "mod m; mod m;"),
            ("m.ox", ""),
            ("unused.ox", ""),
        ],
    ] {
        assert!(
            std::panic::catch_unwind(|| { ProjectSources::from_u8_index_test_files(&files) })
                .is_err()
        );
    }
}

#[test]
fn actual_freeze_exact_no_u8_trace_and_each_failure_prefix() {
    let fixture = Fixture::new(&[("main.ox", "struct Zebra {} fn main()->i32{return 0;}")]);
    let sources = fixture.load();
    let owner = SourceOwner::project(&sources);
    let work = WorkMeter::default();
    work.enable_observation();
    let index = build(owner, &work).unwrap();
    let scan = WorkMeter::default();
    scan.enable_observation();
    index.scan_u8_for_test(&scan).unwrap();
    let events = scan.events.borrow();
    assert_eq!(
        events.iter().map(|e| e.operation).collect::<Vec<_>>(),
        [
            "u8 order module",
            "u8 order item",
            "u8 order comparison",
            "u8 order item",
            "u8 order comparison",
            "u8 reservation module",
            "u8 reservation item",
            "u8 reservation binding",
            "comparison",
            "compared byte",
            "u8 reservation item"
        ]
    );
    assert_eq!(scan.used(), 11);
    let all = work.events.borrow();
    let tail = &all[all.len() - events.len()..];
    assert_eq!(format!("{tail:?}"), format!("{:?}", &*events));
    println!(
        "U8-INDEX-INTEGRATION-1 actual_total={} predecessor_debits={} scan=11",
        work.used(),
        work.used() - 11
    );
    for (position, event) in events.iter().enumerate() {
        let limited = WorkMeter::new(position as u64);
        let error = index.scan_u8_for_test(&limited).unwrap_err();
        assert_eq!(error.code, "E0400");
        assert_eq!(error.primary, Some(event.origin));
        assert_eq!(limited.used(), position as u64);
    }
    index.scan_u8_for_test(&WorkMeter::new(11)).unwrap();
}

#[test]
fn actual_alias_provenance_and_module_preorder() {
    let child = "pub fn f() -> i32 { return 7; } pub struct T {} pub struct D {} pub fn D() -> i32 { return 8; }";
    for (root, start) in [
        (
            "mod m; use crate::m::f as u8; use crate::m::T as u8; fn main() -> i32 { return 0; }",
            49,
        ),
        (
            "mod m; use crate::m::T as u8; use crate::m::f as u8; fn main() -> i32 { return 0; }",
            26,
        ),
        (
            "mod m; use crate::m::D as u8; fn main() -> i32 { return 0; }",
            26,
        ),
    ] {
        for sources in module_sources(&[("main.ox", root), ("m.ox", child)]) {
            let errors = build(SourceOwner::project(&sources), &WorkMeter::default()).unwrap_err();
            assert_eq!(errors.len(), 1);
            let e = &errors[0];
            assert_eq!((e.code, e.stage), ("E0208", "resolve"));
            assert_eq!(e.message,"type name u8 is reserved for the unsigned-byte primitive; rename the type or import alias");
            assert_eq!(
                e.primary,
                Some(Span {
                    file: SourceFileId(0),
                    start,
                    end: start + 2
                })
            );
            assert!(e.secondary.is_empty());
            assert!(e.notes.is_empty());
        }
    }
    for sources in module_sources(&[
        (
            "main.ox",
            "mod m; use crate::m::f as u8; fn main()->i32{return u8();}",
        ),
        ("m.ox", child),
    ]) {
        build(SourceOwner::project(&sources), &WorkMeter::default()).unwrap();
    }
    for sources in module_sources(&[
        ("main.ox", "mod m; struct u8 {}"),
        ("m.ox", "pub struct u8 {}"),
    ]) {
        let errors = build(SourceOwner::project(&sources), &WorkMeter::default()).unwrap_err();
        assert_eq!(
            errors[0].primary,
            Some(Span {
                file: SourceFileId(0),
                start: 14,
                end: 16
            })
        );
    }
}

#[test]
fn graph_error_vectors_precede_reservation() {
    for text in [
        "struct u8 {} struct X {} struct X {}",
        "struct u8 {} use crate::missing as A; use crate::other as B;",
        "struct u8 {} enum E { V, V }",
    ] {
        // Replacing only the unused reserved declaration by an equal-length name
        // is an independent old-domain control; all graph error origins stay exact.
        let control = text.replacen("u8", "ZZ", 1);
        let mut vectors = Vec::new();
        for source in [text, control.as_str()] {
            let fixture = Fixture::new(&[("main.ox", source)]);
            let sources = fixture.load();
            let work = WorkMeter::default();
            work.enable_observation();
            let errors = build(SourceOwner::project(&sources), &work).unwrap_err();
            assert!(!work
                .events
                .borrow()
                .iter()
                .any(|e| e.operation.starts_with("u8")));
            vectors.push(format!("{errors:?}"));
        }
        assert_eq!(vectors[0], vectors[1]);
    }
}

#[test]
fn real_plan_exact_and_one_less_without_quota_growth() {
    let at = Span {
        file: SourceFileId(0),
        start: 0,
        end: 0,
    };
    // Independent arithmetic: visits3*16+128 + originals sorting(2*2)
    // + final originals2*2 = predecessor184, then U=2+3*2+4=12.
    let counts = Counts {
        modules: 1,
        originals: 2,
        records: 1,
        ..Counts::default()
    };
    let plan =
        IndexPlan::calculate(counts, 376, FIXED_SCRATCH, IndexLimits::default(), at).unwrap();
    assert_eq!(
        (plan.retained, plan.scratch, plan.build_work),
        (552, 4108, 196)
    );
    for (work, ok) in [(184, false), (195, false), (196, true)] {
        let limits = IndexLimits {
            retained: 552,
            scratch: 4108,
            work,
        };
        assert_eq!(
            IndexPlan::calculate(counts, 376, FIXED_SCRATCH, limits, at).is_ok(),
            ok
        );
    }
    assert_eq!(
        (
            IndexLimits::default().retained,
            IndexLimits::default().scratch,
            IndexLimits::default().work
        ),
        (32 * 1024 * 1024, 16 * 1024 * 1024, 256_000_000)
    );
    println!("U8-INDEX-INTEGRATION-1 synthetic predecessor_work=184 successor_work=196 retained=552 scratch=4108");
    let fixture = Fixture::new(&[("main.ox", "struct Zebra {} fn main()->i32{return 0;}")]);
    let sources = fixture.load();
    let owner = SourceOwner::project(&sources);
    let preflight_work = WorkMeter::default();
    preflight_work.enable_observation();
    let facts = collect_originals(
        owner,
        IndexLimits::default(),
        &preflight_work,
        &mut Allocator::default(),
    )
    .unwrap();
    let plan = facts.plan();
    let bound = u8_reservation::checked_bound(&plan.counts).unwrap();
    assert_eq!(bound, 12);
    let preflight: u64 = preflight_work
        .events
        .borrow()
        .iter()
        .filter(|e| e.operation == "preflight visit")
        .map(|e| e.units)
        .sum();
    for (work, ok) in [
        (preflight + plan.build_work - bound, false),
        (preflight + plan.build_work - 1, false),
        (preflight + plan.build_work, true),
    ] {
        let mut allocator = Allocator::default();
        let result = collect_originals(
            owner,
            IndexLimits {
                work,
                ..IndexLimits::default()
            },
            &WorkMeter::default(),
            &mut allocator,
        );
        assert_eq!(result.is_ok(), ok);
        if !ok {
            assert_eq!(allocator.attempts, 0);
        }
    }
    println!(
        "U8-INDEX-INTEGRATION-1 Zebra predecessor_work={} successor_work={} retained={} scratch={}",
        plan.build_work - bound,
        plan.build_work,
        plan.retained,
        plan.scratch
    );
}

#[test]
fn cross_kind_order_is_fully_validated_before_reservation() {
    for text in [
        "struct u8 {} fn main()->i32{return 0;}",
        "struct u8 {} fn main()->i32{return 0;} struct Zebra {}",
    ] {
        let mut map = super::super::source::SourceMap::new();
        let file = map.add("forged.ox".into(), text.into());
        let mut ast = super::super::parser::parse(
            map.get(file),
            super::super::lexer::lex(map.get(file)).unwrap(),
        )
        .unwrap();
        let n = ast.items.len();
        ast.items.swap(n - 2, n - 1);
        let owner = SourceOwner::original(map.get(file), &ast, SourceView::Map(&map)).unwrap();
        let work = WorkMeter::default();
        work.enable_observation();
        let errors = build(owner, &work).unwrap_err();
        assert_eq!(errors[0].code, "E0500");
        assert!(!work
            .events
            .borrow()
            .iter()
            .any(|e| e.operation == "u8 reservation module"));
    }
}

#[test]
fn malformed_owner_and_import_metadata_keep_checked_origins() {
    for target in [
        SourceFileId(0),
        SourceFileId(1),
        SourceFileId(2),
        SourceFileId(usize::MAX),
    ] {
        for mut sources in module_sources(&[
            ("main.ox", "mod m; fn main()->i32{return 0;}"),
            ("m.ox", "pub struct T {}"),
        ]) {
            sources.corrupt_header_file_for_u8_test(0, target);
            let owner = SourceOwner::project(&sources);
            for m in [0, 1, 2, usize::MAX] {
                assert_eq!(
                    owner.ast(ModuleId(m)).ok().map(|p| p as *const _),
                    owner.try_ast_borrowed(ModuleId(m)).map(|p| p as *const _)
                );
            }
        }
    }
    for alias in [false, true] {
        for sources in module_sources(&[
            ("main.ox", "mod m; use crate::m::T as U;"),
            ("m.ox", "pub struct T {}"),
        ]) {
            let owner = SourceOwner::project(&sources);
            let mut index = build(owner, &WorkMeter::default()).unwrap();
            let at = sources.try_file_ast(SourceFileId(0)).unwrap().imports[0].alias;
            index.corrupt_u8_import_for_test(alias);
            let error = index.scan_u8_for_test(&WorkMeter::default()).unwrap_err();
            assert_eq!(error.code, "E0500");
            assert_eq!(error.primary, Some(at));
        }
    }
}

#[test]
fn privacy_alias_conflicts_and_sibling_preorder() {
    for (root, child) in [
        (
            "struct u8 {} mod m; use crate::m::Private as X;",
            "struct Private {}",
        ),
        (
            "struct u8 {} mod m; use crate::m::A as X; use crate::m::B as X;",
            "pub struct A {} pub struct B {}",
        ),
    ] {
        let control = root.replacen("u8", "ZZ", 1);
        let mut vectors = Vec::new();
        for source in [root, control.as_str()] {
            for sources in module_sources(&[("main.ox", source), ("m.ox", child)]) {
                let work = WorkMeter::default();
                work.enable_observation();
                let errors = build(SourceOwner::project(&sources), &work).unwrap_err();
                assert!(!errors.iter().any(|e| e.code == "E0208"));
                assert!(!work
                    .events
                    .borrow()
                    .iter()
                    .any(|e| e.operation.starts_with("u8")));
                vectors.push(format!("{errors:?}"));
            }
        }
        // Compare both spelling controls and both source routes, when available.
        for pair in vectors.windows(2) {
            assert_eq!(pair[0], pair[1]);
        }
    }
    for root in ["mod z; mod a;", "mod a; mod z;"] {
        for sources in module_sources(&[
            ("main.ox", root),
            ("z.ox", "pub struct u8 {}"),
            ("a.ox", "pub enum u8 { V }"),
        ]) {
            let errors = build(SourceOwner::project(&sources), &WorkMeter::default()).unwrap_err();
            assert_eq!(errors[0].code, "E0208");
            // The first declared module is loaded at numeric preorder 1, regardless of filename sorting.
            assert_eq!(errors[0].primary.unwrap().file, SourceFileId(1));
            assert_eq!(
                errors[0].primary.unwrap().start,
                if root.starts_with("mod z") { 11 } else { 9 }
            );
        }
    }
}

#[test]
fn primitive_query_work_is_exact_and_keeps_predecessor_shortcuts() {
    for (name, expected, exact) in [
        ("bool", Ty::Bool, 6),
        ("i32", Ty::I32, 7),
        ("u8", Ty::U8, 8),
    ] {
        let text = format!("fn f()->{name}{{return;}}");
        let fixture = Fixture::new(&[("main.ox", &text)]);
        let sources = fixture.load();
        let index = build(SourceOwner::project(&sources), &WorkMeter::default()).unwrap();
        let ty = sources.try_file_ast(SourceFileId(0)).unwrap().functions[0].result;
        let work = WorkMeter::default();
        work.enable_observation();
        assert_eq!(
            index
                .query(&work)
                .value_type(ModuleId(0), ty, TypeContext::Scalar)
                .unwrap(),
            ValueTy::Scalar(expected)
        );
        // One query debit, then bool costs5; i32 bool mismatch2 + match4;
        // u8 bool mismatch2 + i32 mismatch2 + match3. No new bool/i32 comparison.
        assert_eq!(work.used(), exact);
        for cap in [exact - 1, exact] {
            let meter = WorkMeter::new(cap);
            let result = index
                .query(&meter)
                .value_type(ModuleId(0), ty, TypeContext::Scalar);
            assert_eq!(result.is_ok(), cap == exact);
            if cap < exact {
                assert_eq!(result.unwrap_err().primary, Some(ty.span));
            }
        }
        println!(
            "U8-PRIMITIVE-QUERY-1 name={name} work={exact} carrier={}/{}",
            size_of::<ValueTy>(),
            std::mem::align_of::<ValueTy>()
        );
    }
    let fixture = Fixture::new(&[("main.ox", "struct Zebra{} fn f()->Zebra{return;}")]);
    let sources = fixture.load();
    let index = build(SourceOwner::project(&sources), &WorkMeter::default()).unwrap();
    let ty = sources.try_file_ast(SourceFileId(0)).unwrap().functions[0].result;
    let meter = WorkMeter::default();
    meter.enable_observation();
    index
        .query(&meter)
        .value_type(ModuleId(0), ty, TypeContext::Value)
        .unwrap();
    let events = meter.events.borrow();
    assert_eq!(
        events[..7].iter().map(|e| e.operation).collect::<Vec<_>>(),
        [
            "query value type",
            "comparison",
            "compared byte",
            "comparison",
            "compared byte",
            "comparison",
            "compared byte"
        ]
    );
    // The first two comparisons are unchanged. The third is exactly +2 for Zebra.
    println!(
        "U8-PRIMITIVE-QUERY-1 nominal=Zebra predecessor={} successor={} delta=2",
        meter.used() - 2,
        meter.used()
    );
}
