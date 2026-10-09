//! Named RFC0030 successors for unchanged frozen-provider fixtures.
//! Historical constants stay visible; these are controls, not review approval.
use super::{allocation::Failure, candidate};
use crate::frontend::{
    ast::ItemId,
    declaration_index::{IndexLimits, WorkMeter},
    hir, lexer,
    oir::native::{emit_cost, emit_work, private_emit, scalar_resource},
    parser,
    project::budget::Allocator,
    source::{SourceFileId, SourceMap, SourceView},
};
use std::mem::size_of;

// These literal fixtures have one module, only ordinary function originals and
// no imports or candidate type bindings. Scanner S = 2M + 3(O+I), paid once by
// canonical resolution. Exact ordered event controls below prove those counts.
pub(super) const RICH_RESERVATION_WORK: u64 = 2 + 3 * 2;
pub(super) const V2_RESERVATION_WORK: u64 = 2 + 3;

// Closed wire projection: returned + held Result and its one extra inventory
// element in the array, moved array and concrete IntoIter (three usize slots).
const PROJECTION_GROWTH: usize = 2 * size_of::<Result<i32, Failure>>() + 3 * size_of::<usize>();
// ArithmeticBindings: two widened return/caller tuples and the destructured
// u64/bool pair. The selector OR adds three bools plus three inventory slots.
const SCAN_GROWTH: usize = 2 * (size_of::<(u64, bool)>() - size_of::<u64>())
    + size_of::<(u64, bool)>()
    + size_of::<(bool, bool, bool)>()
    + 3 * size_of::<usize>();
// DiagnosticLocals gains fixed_human:u64 (selector bool fits existing padding).
// Its two argument tuples remain16. New selector-incidence role plus array,
// moved array and two complete IntoIter transports adds four inventory slots.
const FORMULA_GROWTH: usize =
    size_of::<u64>() + size_of::<(bool, u64, bool, bool)>() + 4 * size_of::<usize>();
// Native admission/emission successor: complete owning-scope role aggregate,
// plus one row in the native inventory array, moved array and IntoIter.
const NATIVE_GROWTH: usize = scalar_resource::named_bytes() + 3 * size_of::<usize>();
pub(super) const EMIT_FIXED_GROWTH: u64 =
    (PROJECTION_GROWTH + SCAN_GROWTH + FORMULA_GROWTH + NATIVE_GROWTH) as u64;

#[test]
fn u8_provider_fixed_bank_growth_is_the_sum_of_actual_new_roles() {
    assert_eq!(PROJECTION_GROWTH, 40);
    assert_eq!(SCAN_GROWTH, 59);
    assert_eq!(FORMULA_GROWTH, 56);
    // Independent native decomposition: admission960, emission1944 (includes
    // MIR-backed format arrays), preflight96, inventory return8, then24 for
    // the enclosing native row/array/iterator transports.
    assert_eq!(NATIVE_GROWTH, 960 + 1944 + 96 + 8 + 24);
    assert_eq!(EMIT_FIXED_GROWTH, 3_187);
    assert_eq!(private_emit::named_bytes().unwrap(), 2_155 + NATIVE_GROWTH);
    assert_eq!(
        candidate::u8_projection_named_bytes(),
        3_465 + PROJECTION_GROWTH
    );
    assert_eq!(emit_work::named_bytes().unwrap(), 4_051 + SCAN_GROWTH);
    assert_eq!(emit_cost::named_bytes().unwrap(), 10_384 + FORMULA_GROWTH);
    println!("U8_PROVIDER_FIXED_SUCCESSOR predecessor=140203 projection={PROJECTION_GROWTH} scan={SCAN_GROWTH} formula={FORMULA_GROWTH} native={NATIVE_GROWTH} successor={}", 140_203 + EMIT_FIXED_GROWTH);
}

#[test]
fn u8_provider_reservation_delta_has_exact_fixture_counts_and_ordered_events() {
    let rich = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import/rich-source.txt"
    ));
    let v2 = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import_v2/source-255.txt"
    ));
    for (text, functions, expected_delta) in [
        (rich, 2, RICH_RESERVATION_WORK),
        (v2, 1, V2_RESERVATION_WORK),
    ] {
        let mut sources = SourceMap::new();
        sources.add("reservation-successor.ox".into(), text.into());
        let source = sources.get(SourceFileId(0));
        let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
        assert_eq!(ast.items.len(), functions);
        assert_eq!(ast.functions.len(), functions);
        assert!(ast
            .items
            .iter()
            .all(|item| matches!(item, ItemId::Function(_))));
        assert!(
            ast.modules.is_empty()
                && ast.imports.is_empty()
                && ast.records.is_empty()
                && ast.enums.is_empty()
        );
        let owner = super::SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
        assert_eq!(owner.count(), 1);
        let work = WorkMeter::default();
        work.enable_observation();
        hir::resolve_sources_with_meter(owner, &work, &mut Allocator::default()).unwrap();
        let events = work.events.borrow();
        let start = events
            .iter()
            .position(|event| event.operation == "u8 order module")
            .unwrap();
        let end = start + expected_delta as usize;
        let actual: Vec<_> = events[start..end]
            .iter()
            .map(|event| (event.operation, event.units))
            .collect();
        let mut expected = vec![("u8 order module", 1)];
        for _ in 0..functions {
            expected.extend([("u8 order item", 1), ("u8 order comparison", 1)]);
        }
        expected.push(("u8 reservation module", 1));
        expected.extend(vec![("u8 reservation item", 1); functions]);
        assert_eq!(actual, expected);
        assert_eq!(
            events
                .iter()
                .filter(|event| event.operation.starts_with("u8 "))
                .map(|event| event.units)
                .sum::<u64>(),
            expected_delta
        );
        let before: u64 = events[..start].iter().map(|event| event.units).sum();
        let after: u64 = events[end..].iter().map(|event| event.units).sum();
        assert_eq!(work.used(), before + expected_delta + after);
        println!("U8_PROVIDER_WORK_SUCCESSOR functions={functions} modules=1 imports=0 prefix={before} delta={expected_delta} suffix={after} total={}", work.used());
    }
    let limits = IndexLimits::default();
    assert_eq!(
        (limits.retained, limits.scratch, limits.work),
        (32 * 1024 * 1024, 16 * 1024 * 1024, 256_000_000)
    );
}
