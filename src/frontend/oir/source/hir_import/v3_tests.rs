//! Genuine frozen producer captures; no translated legacy observation is authority.
use super::*;
use crate::frontend::{
    declaration_index::IndexLimits,
    lexer,
    options::Operation,
    parser,
    project::budget::Allocator,
    source::{SourceMap, SourceView},
};
const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/checked_hir_import_v3/rich.ox"
));
const WIRE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/checked_hir_import_v3/rich.bin"
));

fn with_owner<T>(f: impl FnOnce(SourceOwner<'_>) -> T) -> T {
    let mut sources = SourceMap::new();
    let id = sources.add("v3.ox".into(), SOURCE.into());
    let source = sources.get(id);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    f(SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap())
}

#[test]
fn checked_hir_v3_exact_wire_and_raw_mutation_controls() {
    let wire = Wire::decode(WIRE, 511).unwrap();
    assert_eq!(wire.protocol(), Protocol::V3);
    assert_eq!(source_work_bound_for(Protocol::V1).unwrap(), 124501);
    assert_eq!(source_work_bound_for(Protocol::V2).unwrap(), 190160);
    assert_eq!(source_work_bound_for(Protocol::V3).unwrap(), 458424);
    assert_eq!(wire.source_len(), 511);
    assert_eq!(std::mem::size_of::<Wire<'_>>(), 24);
    assert!(wire.wide_cell(2, 0).unwrap() > 255);
    with_owner(|owner| {
        let bound = BoundObservation::bind(owner, SOURCE.as_bytes(), WIRE).unwrap();
        assert!(ast_compare::compare::<u16>(&bound).is_ok());
        assert!(ast_compare::compare::<u8>(&bound).is_err());
    });
    let mut cases = Vec::new();
    for (at, value) in [
        (3, b'2'),
        (1307, b'2'),
        (4, 1),
        (13, 2),
        (1309, 0),
        (1310, 1),
        (14 + 129, 255),
        (14 + 258 + 129, 2),
        (14 + 516 + 129, 2),
        (14 + 774 + 129, 129),
        (14 + 1032 + 129, 129),
    ] {
        let mut bytes = WIRE.to_vec();
        bytes[at] = value;
        cases.push(bytes);
    }
    for column in 0..5 {
        let mut bytes = WIRE.to_vec();
        bytes[14 + column * 258 + 128] = 1;
        cases.push(bytes);
    }
    for column in 0..2 {
        let mut bytes = WIRE.to_vec();
        bytes[1324 + column * 516 + 128] = 1;
        cases.push(bytes);
    }
    cases.push(WIRE[..WIRE.len() - 1].to_vec());
    let mut appended = WIRE.to_vec();
    appended.push(0);
    cases.push(appended);
    for bytes in cases {
        assert!(Wire::decode(&bytes, 511).is_err());
        with_owner(|owner| {
            let mut allocator = Allocator::default();
            assert!(leaf::verify(
                owner,
                SOURCE.as_bytes(),
                &bytes,
                &mut allocator,
                IndexLimits::default()
            )
            .is_err());
            assert_eq!(allocator.attempts, 0);
        });
    }
}

// Exercise the exact allocation-free result carriers, including nested closures.
#[allow(clippy::result_large_err)]
#[test]
fn checked_hir_v3_verify_run_emit_complete_and_exact_work_storage() {
    for operation in [Operation::Check, Operation::Run, Operation::Compile] {
        let attempt = |limits: IndexLimits| {
            with_owner(|owner| {
                let mut allocator = Allocator::default();
                match operation {
                    Operation::Check => {
                        leaf::verify(owner, SOURCE.as_bytes(), WIRE, &mut allocator, limits)
                            .map(|f| f.total_work)
                    }
                    Operation::Run => {
                        leaf::run(owner, SOURCE.as_bytes(), WIRE, &mut allocator, limits).map(|f| {
                            assert!(f.verified.runtime.as_ref().unwrap().is_ok());
                            f.total_work
                        })
                    }
                    Operation::Compile => {
                        leaf::emit(owner, SOURCE.as_bytes(), WIRE, &mut allocator, limits).map(
                            |f| {
                                assert!(!f.artifact.text.is_empty());
                                assert_eq!(
                                    f.artifact.connection_work,
                                    candidate::EMIT_CONNECTION_WORK_V3
                                );
                                let a = &f.artifact;
                                let v = &a.verified;
                                assert_eq!(
                                    f.total_work,
                                    f.source_work
                                        + f.canonical_work
                                        + v.candidate.charged_work
                                        + v.pass_work
                                        + v.typed_work
                                        + v.entry_work
                                        + a.connection_work
                                        + a.scan_work
                                        + a.formula_work
                                        + a.body_work
                                );
                                f.total_work
                            },
                        )
                    }
                }
            })
        };
        let d = IndexLimits::default();
        let used = attempt(d).unwrap();
        let mut minima = [0; 3];
        for axis in 0..3 {
            let ceiling = [d.retained, d.scratch, d.work][axis];
            let limit = |v| match axis {
                0 => IndexLimits { retained: v, ..d },
                1 => IndexLimits { scratch: v, ..d },
                _ => IndexLimits { work: v, ..d },
            };
            let (mut low, mut high) = (0, ceiling);
            while low < high {
                let mid = low + (high - low) / 2;
                if attempt(limit(mid)).is_ok() {
                    high = mid
                } else {
                    low = mid + 1
                }
            }
            assert!(attempt(limit(low)).is_ok());
            assert!(attempt(limit(low - 1)).is_err());
            minima[axis] = low;
        }
        let exact = IndexLimits {
            retained: minima[0],
            scratch: minima[1],
            work: minima[2],
        };
        assert_eq!(attempt(exact).unwrap(), used);
        println!(
            "V3_FULL_LIMITS {operation:?} retained={} scratch={} work={} actual_work={used}",
            minima[0], minima[1], minima[2]
        );
    }
    let project = tests::project(SOURCE);
    for operation in [Operation::Check, Operation::Run, Operation::Compile] {
        assert!(public_facade::import_checked(&project, WIRE, operation).is_ok());
    }
}

#[test]
fn checked_hir_v3_typed_facts_are_checked_and_supplied_diagnostic_never_imports() {
    for col in 0..2 {
        let mut bytes = WIRE.to_vec();
        bytes[1324 + col * 516] ^= 1;
        with_owner(|owner| {
            assert!(leaf::verify(
                owner,
                SOURCE.as_bytes(),
                &bytes,
                &mut Allocator::default(),
                IndexLimits::default()
            )
            .is_err())
        });
    }
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import_v3/duplicate.ox"
    ));
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import_v3/duplicate.bin"
    ));
    let project = tests::project(source);
    assert!(public_facade::import_checked(&project, bytes, Operation::Check).is_err());
}

#[test]
fn checked_hir_v3_global_function_partition_and_near_row_bound() {
    for (source, bytes, rows, functions) in [
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/checked_hir_import_v3/many-functions.ox"
            )),
            &include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/checked_hir_import_v3/many-functions.bin"
            ))[..],
            40,
            10,
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/checked_hir_import_v3/many-rows.ox"
            )),
            &include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/checked_hir_import_v3/many-rows.bin"
            ))[..],
            109,
            1,
        ),
    ] {
        let project = tests::project(source);
        let mut allocator = Allocator::default();
        let facts = leaf::verify(
            SourceOwner::project(&project),
            source.as_bytes(),
            bytes,
            &mut allocator,
            IndexLimits::default(),
        )
        .unwrap();
        assert_eq!(facts.verified.functions, functions);
        assert_eq!(facts.verified.typed_cells, rows);
        assert!(
            matches!(public_facade::import_checked(&project,bytes,Operation::Check),Ok(public_facade::Imported::Checked(n)) if n==functions)
        );
        // Synthetic count/tail/next mutations are rejection controls, not claims
        // that all128 rows/functions are grammar reachable under token128.
        for (offset, value) in [
            (10, 128),
            (10, 129),
            (14 + 129, 255),
            (14 + 258 + 128, 1),
            (1324 + 128, 1),
        ] {
            let mut bad = bytes.to_vec();
            bad[offset] = value;
            let mut allocator = Allocator::default();
            assert!(leaf::verify(
                SourceOwner::project(&project),
                source.as_bytes(),
                &bad,
                &mut allocator,
                IndexLimits::default()
            )
            .is_err());
            assert_eq!(allocator.attempts, 0);
        }
    }
}
