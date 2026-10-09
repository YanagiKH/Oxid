//! U8-INDEX-INTEGRATION-1: exercise the single production collect/finish path.
use super::tests::Fixture;
use super::*;

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
        let fixture = Fixture::new(&[("main.ox", root), ("m.ox", child)]);
        let sources = fixture.load();
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
    let fixture = Fixture::new(&[
        (
            "main.ox",
            "mod m; use crate::m::f as u8; fn main()->i32{return u8();}",
        ),
        ("m.ox", child),
    ]);
    let sources = fixture.load();
    build(SourceOwner::project(&sources), &WorkMeter::default()).unwrap();
    let fixture = Fixture::new(&[
        ("main.ox", "mod m; struct u8 {}"),
        ("m.ox", "pub struct u8 {}"),
    ]);
    let sources = fixture.load();
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
        let fixture = Fixture::new(&[
            ("main.ox", "mod m; fn main()->i32{return 0;}"),
            ("m.ox", "pub struct T {}"),
        ]);
        let mut sources = fixture.load();
        sources.corrupt_header_file_for_u8_test(0, target);
        let owner = SourceOwner::project(&sources);
        for m in [0, 1, 2, usize::MAX] {
            assert_eq!(
                owner.ast(ModuleId(m)).ok().map(|p| p as *const _),
                owner.try_ast_borrowed(ModuleId(m)).map(|p| p as *const _)
            );
        }
    }
    for alias in [false, true] {
        let fixture = Fixture::new(&[
            ("main.ox", "mod m; use crate::m::T as U;"),
            ("m.ox", "pub struct T {}"),
        ]);
        let sources = fixture.load();
        let owner = SourceOwner::project(&sources);
        let mut index = build(owner, &WorkMeter::default()).unwrap();
        let at = sources.try_file_ast(SourceFileId(0)).unwrap().imports[0].alias;
        index.corrupt_u8_import_for_test(alias);
        let error = index.scan_u8_for_test(&WorkMeter::default()).unwrap_err();
        assert_eq!(error.code, "E0500");
        assert_eq!(error.primary, Some(at));
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
            let fixture = Fixture::new(&[("main.ox", source), ("m.ox", child)]);
            let sources = fixture.load();
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
        assert_eq!(vectors[0], vectors[1]);
    }
    for root in ["mod z; mod a;", "mod a; mod z;"] {
        let fixture = Fixture::new(&[
            ("main.ox", root),
            ("z.ox", "pub struct u8 {}"),
            ("a.ox", "pub enum u8 { V }"),
        ]);
        let sources = fixture.load();
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
