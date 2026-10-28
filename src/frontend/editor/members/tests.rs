use super::*;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("retained record completion fixture: {}", self.0.display());
        } else {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
}
fn with_source(text: &str, run: impl FnOnce(&QuerySources<'_>, &[PackageSpec], &Path, usize)) {
    let directory = std::env::temp_dir().join(format!(
        "oxid-record-fields-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&directory).unwrap();
    let fixture = Fixture(directory.canonicalize().unwrap());
    let path = fixture.0.join("main.ox");
    fs::write(&path, text).unwrap();
    let packages = [PackageSpec {
        root: fixture.0.clone(),
        entry: path.clone(),
        dependencies: vec![],
    }];
    let overlay = SourceOverlay::new(&[]).unwrap();
    let session = QuerySources::load(&packages, None, &overlay).unwrap();
    let offset = text.rfind("point.").unwrap() + "point.".len();
    run(&session, &packages, &path, offset);
}
fn query(
    session: &QuerySources<'_>,
    packages: &[PackageSpec],
    path: &Path,
    offset: usize,
    work: &WorkMeter,
    allocator: &mut Allocator,
    limit: u64,
) -> MemberCompletions {
    complete_limited(session, packages, path, offset, work, allocator, limit)
        .unwrap_or_else(|error| panic!("{}: {}", error.code, error.message))
}
const TEXT: &str =
    "struct Point{z:i32,a:i32}fn main()->i32{let point=Point{z:1,a:2};return point.;}";

#[test]
#[cfg(target_pointer_width = "64")]
fn member_exact_scratch_and_one_less_refuse_before_the_corresponding_reserve() {
    with_source(TEXT, |session, packages, path, offset| {
        let fixed = resources::tests::independently_modeled_fixed_bytes();
        // Existing namespace: 5 complete 2112B carriers, merge Vec24,
        // unchanged PathQuery872, two Row40/order4/merge4 entries.
        let namespace = 5 * 2112 + 24 + 872 + 2 * (40 + 4 + 4);
        assert_eq!(size_of::<FieldName>(), 32);
        assert_eq!(size_of::<EditorCompletionItem>(), 48);
        let fields = 2 * 32;
        let output = 2 * 48 + 2 + path.to_str().unwrap().len();
        let exact = fixed + namespace + fields + output;
        for (limit, attempts) in [
            (fixed - 1, 0),
            (fixed + namespace - 1, 0),
            (fixed + namespace + fields - 1, 3),
            (exact - 1, 4),
        ] {
            let mut allocator = Allocator::default();
            let result = complete_limited(
                session,
                packages,
                path,
                offset,
                &WorkMeter::default(),
                &mut allocator,
                limit as u64,
            );
            assert_eq!(result.err().unwrap().code, "E0400");
            assert_eq!(allocator.attempts, attempts);
        }
        let mut allocator = Allocator::default();
        let answer = query(
            session,
            packages,
            path,
            offset,
            &WorkMeter::default(),
            &mut allocator,
            exact as u64,
        );
        assert_eq!(answer.charged, exact);
        assert_eq!(
            answer
                .completions
                .entries
                .iter()
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>(),
            ["a", "z"]
        );
        assert_eq!(allocator.attempts, 8);
    });
}

#[test]
fn member_every_real_reserve_failure_and_work_boundary_return_no_partial_result() {
    with_source(TEXT, |session, packages, path, offset| {
        for ordinal in 1..=8 {
            let mut allocator = Allocator {
                fail_at: Some(ordinal),
                ..Allocator::default()
            };
            let result = complete_limited(
                session,
                packages,
                path,
                offset,
                &WorkMeter::default(),
                &mut allocator,
                IndexLimits::default().scratch,
            );
            assert_eq!(
                result.err().unwrap().code,
                "E0400",
                "failure ordinal {ordinal}"
            );
            assert_eq!(allocator.attempts, ordinal);
            assert!(!allocator.trace.last().unwrap().success);
            assert!(allocator.trace[..ordinal - 1]
                .iter()
                .all(|event| event.success));
        }
        let work = WorkMeter::default();
        let answer = query(
            session,
            packages,
            path,
            offset,
            &work,
            &mut Allocator::default(),
            IndexLimits::default().scratch,
        );
        assert!(answer.identity.is_some());
        let exact = work.used();
        for (cap, pass) in [(0, false), (exact - 1, false), (exact, true)] {
            let mut allocator = Allocator::default();
            let result = complete_limited(
                session,
                packages,
                path,
                offset,
                &WorkMeter::new(cap),
                &mut allocator,
                IndexLimits::default().scratch,
            );
            assert_eq!(result.is_ok(), pass, "work cap {cap}");
            if cap == 0 {
                assert_eq!(allocator.attempts, 0);
            }
            if !pass {
                assert!(result.err().unwrap().message.contains("work limit"));
            }
        }
    });
}

#[test]
fn member_params_references_constructor_and_annotation_use_actual_record_identity() {
    for text in [
        "struct Point{value:i32}fn read(point:Point)->i32{return point.;}",
        "struct Point{value:i32}fn read(point:&Point)->i32{return point.;}",
        "struct Point{value:i32}fn read(point:&mut Point)->i32{return point.;}",
        "struct Point{value:i32}fn main()->i32{let point:crate::Point=Point{value:1};return point.;}",
        "struct Point{value:i32}fn main()->i32{let point=crate::Point{value:1};let unrelated=2;return point.;}",
        "struct Point{value:i32}fn main()->i32{let point=Point{value:1};let next=point.;}",
    ] {
        with_source(text, |session, packages, path, offset| {
            let answer = query(session, packages, path, offset, &WorkMeter::default(), &mut Allocator::default(), IndexLimits::default().scratch);
            assert_eq!(answer.completions.entries.iter().map(|item| item.label.as_str()).collect::<Vec<_>>(), ["value"], "{text}");
        });
    }
}

#[test]
fn member_unknown_moved_generic_shadowed_and_control_flow_receivers_stay_closed() {
    for body in [
        "return point.;",
        "let point=1;return point.;",
        "let point=Point{value:1};let moved=point;return point.;",
        "let point=Point{value:1};consume(point);return point.;",
        "let point=Point{value:1};if true{return 0;}return point.;",
        "if true{let point=Point{value:1};return point.;}",
        "let point=Point{value:1};let point=Point{value:2};return point.;",
        "let point:Other=Point{value:1};return point.;",
        "let point=Point{};return point.;",
        "let point=Point{value:1,extra:2};return point.;",
        "let point=Point{value:};return point.;",
        "let point=make();return point.;",
        "let point=Point{value:1};return other.point.;",
    ] {
        let text =
            format!("struct Point{{value:i32}}struct Other{{value:i32}}fn main()->i32{{{body}}}");
        with_source(&text, |session, packages, path, offset| {
            let answer = query(
                session,
                packages,
                path,
                offset,
                &WorkMeter::default(),
                &mut Allocator::default(),
                IndexLimits::default().scratch,
            );
            assert!(
                answer.present
                    && answer.identity.is_none()
                    && answer.completions.entries.is_empty(),
                "{text}"
            );
        });
    }
    for text in [
        "struct Point<T>{value:T}fn f(point:Point<i32>)->i32{return point.;}",
        "struct Point{value:i32}fn f<Point>(point:Point)->i32{return point.;}",
    ] {
        with_source(text, |session, packages, path, offset| {
            assert!(query(
                session,
                packages,
                path,
                offset,
                &WorkMeter::default(),
                &mut Allocator::default(),
                IndexLimits::default().scratch
            )
            .identity
            .is_none());
        });
    }
}

#[test]
fn member_retained_source_survives_removal_and_independent_sessions_keep_nominal_identity() {
    with_source(TEXT, |session, packages, path, offset| {
        let second = QuerySources::load(packages, None, &SourceOverlay::new(&[]).unwrap()).unwrap();
        let a = query(
            session,
            packages,
            path,
            offset,
            &WorkMeter::default(),
            &mut Allocator::default(),
            IndexLimits::default().scratch,
        );
        let b = query(
            &second,
            packages,
            path,
            offset,
            &WorkMeter::default(),
            &mut Allocator::default(),
            IndexLimits::default().scratch,
        );
        assert_eq!(a.identity, b.identity);
        fs::remove_file(path).unwrap();
        let retained = query(
            session,
            packages,
            path,
            offset,
            &WorkMeter::default(),
            &mut Allocator::default(),
            IndexLimits::default().scratch,
        );
        assert_eq!(a.identity, retained.identity);
        assert_eq!(retained.completions.entries.len(), 2);
        let mut merged = Merge::default();
        merged.push(a);
        merged.push(b);
        assert_eq!(merged.finish().unwrap().entries.len(), 2);
    });
    let mut identities = Vec::new();
    for _ in 0..2 {
        with_source(TEXT, |session, packages, path, offset| {
            identities.push(
                query(
                    session,
                    packages,
                    path,
                    offset,
                    &WorkMeter::default(),
                    &mut Allocator::default(),
                    IndexLimits::default().scratch,
                )
                .identity,
            );
        });
    }
    assert_ne!(
        identities[0], identities[1],
        "equal source and display names in distinct package roots are distinct"
    );
}

#[test]
fn member_graph_merge_never_unions_distinct_or_unresolved_records() {
    with_source(TEXT, |session, packages, path, offset| {
        for unresolved_first in [false, true] {
            let known = query(
                session,
                packages,
                path,
                offset,
                &WorkMeter::default(),
                &mut Allocator::default(),
                IndexLimits::default().scratch,
            );
            let unknown = empty(session.project().sources(), true, 0);
            let mut merge = Merge::default();
            if unresolved_first {
                merge.push(unknown);
                merge.push(known);
            } else {
                merge.push(known);
                merge.push(unknown);
            }
            assert!(merge.finish().is_none());
        }
        let a = query(
            session,
            packages,
            path,
            offset,
            &WorkMeter::default(),
            &mut Allocator::default(),
            IndexLimits::default().scratch,
        );
        let mut b = query(
            session,
            packages,
            path,
            offset,
            &WorkMeter::default(),
            &mut Allocator::default(),
            IndexLimits::default().scratch,
        );
        b.identity.as_mut().unwrap().0[0] ^= 1;
        let mut merge = Merge::default();
        merge.push(a);
        merge.push(b);
        assert!(merge.finish().is_none());
        let mut merge = Merge::default();
        merge.push(empty(session.project().sources(), false, 0));
        merge.push(query(
            session,
            packages,
            path,
            offset,
            &WorkMeter::default(),
            &mut Allocator::default(),
            IndexLimits::default().scratch,
        ));
        assert_eq!(merge.finish().unwrap().entries.len(), 2);
    });
}
