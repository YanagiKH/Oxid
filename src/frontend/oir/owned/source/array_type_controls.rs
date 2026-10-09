//! Inert receipts from the frozen, focused guard and reservation controls.
//! This test module never provides a compiler factory or execution authority.
use super::{budget, diagnostic, lower, resolve, typeck};
use crate::frontend::{
    ast,
    declaration_index::{IndexLimits, SourceOwner, WorkMeter},
    diagnostic::{json_string, Diagnostic},
    lexer, parser,
    project::budget::Allocator,
    source::{SourceFileId, SourceMap, SourceView},
};

const ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/fixed_array_source_unit3"
);
const SCHEMA: &str = "oxid-array-types-controls-v1";

fn single(case: &str, authority: &str) -> (SourceMap, ast::Program) {
    let text =
        std::fs::read_to_string(format!("{ROOT}/{authority}/fixtures/{case}/main.ox")).unwrap();
    let mut sources = SourceMap::new();
    let id = sources.add("main.ox".into(), text);
    let file = sources.get(id);
    let (ast, _) = parser::parse_counted_with_arrays(
        file,
        lexer::lex(file).unwrap(),
        parser::SourceMode::OwnedCandidate,
        parser::MAX_NODES,
        &mut Allocator::default(),
        parser::ArraySyntaxPolicy::Candidate,
    )
    .unwrap();
    (sources, ast)
}
fn diagnostics(error: &Diagnostic, sources: &SourceMap) -> String {
    format!(
        "\"diagnostic\":{},\"human\":{},\"json_line\":{}",
        error.render_json(sources),
        json_string(&error.render_human(sources)),
        json_string(&format!("{}\n", error.render_json(sources)))
    )
}
fn guard_row(
    case: &str,
    entry: &str,
    function: Option<usize>,
    error: &Diagnostic,
    sources: &SourceMap,
) {
    let c = budget::guard_counts();
    println!("CONTROL {{\"schema\":\"{SCHEMA}\",\"kind\":\"guard\",\"case_id\":{},\"entry\":{},\"function_id\":{},\"injected_first_raw_reservation\":true,\"sentinels\":{{\"declaration_admission\":{},\"function_iteration\":{},\"count_entry\":{},\"inventory_entry\":{},\"block_allocation\":{},\"raw_reservation\":{},\"emit_step\":{}}},{}}}",
        json_string(case), json_string(entry), function.map_or("null".into(), |id| id.to_string()),
        c[0],c[1],c[2],c[3],c[4],c[5],c[6], diagnostics(error,sources));
    assert_eq!(c, [0; 7]);
    assert_eq!(
        (error.code, error.stage, error.message.as_str()),
        (
            "E0500",
            "oir-owned-lower",
            "internal compiler error: owned invariant violation"
        )
    );
    assert!(error.secondary.is_empty() && error.notes.is_empty());
}
fn guards(case: &str, typed: &typeck::TypedOwnedProgram<'_>, display: &SourceMap) {
    let eof = typed.index().sources().eof();
    for entry in ["check_typed", "budget::preflight", "lower_with_limits"] {
        budget::reset_guard_counts();
        let error = budget::fail_allocation_after(0, || match entry {
            "check_typed" => {
                let mut errors = super::program::check_typed(typed).unwrap_err();
                assert_eq!(errors.len(), 1);
                Box::new(errors.remove(0))
            }
            "budget::preflight" => diagnostic::lower(
                &budget::preflight(typed, budget::Limits::DEFAULT).unwrap_err(),
                display,
            ),
            "lower_with_limits" => diagnostic::lower(
                &lower::lower_with_limits(typed, budget::Limits::DEFAULT).unwrap_err(),
                display,
            ),
            _ => panic!("fixed control entry"),
        });
        assert_eq!(error.primary, Some(eof));
        guard_row(case, entry, None, &error, display);
    }
    for view in typed.functions() {
        for entry in [
            "count_function(None)",
            "count_function(Some(block_counts))",
            "Walk::new emission constructor",
        ] {
            budget::reset_guard_counts();
            let mut counts = [usize::MAX; 3];
            let error = budget::fail_allocation_after(0, || match entry {
                "count_function(None)" => lower::count_function(&view, None).unwrap_err(),
                "count_function(Some(block_counts))" => {
                    lower::count_function(&view, Some(&mut counts)).unwrap_err()
                }
                "Walk::new emission constructor" => {
                    lower::check_array_type_emission_fence(&view).unwrap_err()
                }
                _ => panic!("fixed control entry"),
            });
            assert_eq!(counts, [usize::MAX; 3]);
            let error = diagnostic::lower(&error, display);
            assert_eq!(error.primary, Some(view.signature().span));
            guard_row(case, entry, Some(view.hir().id.0), &error, display);
        }
    }
}
fn reservation_rows(case: &str, owner: SourceOwner<'_>, display: &SourceMap) {
    let work = WorkMeter::default();
    let mut baseline = Allocator::default();
    let resolved =
        resolve::resolve_array_types(owner, IndexLimits::default(), &work, &mut baseline).unwrap();
    drop(resolved);
    let attempts: Vec<_> = baseline
        .trace
        .iter()
        .enumerate()
        .filter(|(_, e)| e.kind == "array HIR elements")
        .map(|(i, e)| (i + 1, e.length))
        .collect();
    for (ordinal, (attempt, requested)) in attempts.into_iter().enumerate() {
        let work = WorkMeter::default();
        let mut allocator = Allocator {
            fail_at: Some(attempt),
            ..Allocator::default()
        };
        let errors =
            resolve::resolve_array_types(owner, IndexLimits::default(), &work, &mut allocator)
                .unwrap_err();
        assert_eq!(errors.len(), 1);
        let error = &errors[0];
        let event = &allocator.trace[attempt - 1];
        assert_eq!(
            (event.kind, event.length, event.success),
            ("array HIR elements", requested, false)
        );
        let trace = allocator
            .trace
            .iter()
            .take(attempt)
            .map(|e| {
                format!(
                    "{{\"kind\":{},\"length\":{},\"element_bytes\":{},\"success\":{}}}",
                    json_string(e.kind),
                    e.length,
                    e.element_bytes,
                    e.success
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        println!("CONTROL {{\"schema\":\"{SCHEMA}\",\"kind\":\"reservation\",\"case_id\":{},\"literal_ordinal\":{},\"allocator_attempt\":{},\"requested_length\":{},\"element_bytes\":{},\"real_fallible\":true,\"trace\":[{}],{}}}", json_string(case),ordinal,attempt,requested,event.element_bytes,trace,diagnostics(error,display));
        assert_eq!(
            (error.code, error.stage, error.message.as_str()),
            ("E0400", "resolve", "array HIR allocation failed")
        );
        assert!(error.notes.is_empty() && error.secondary.is_empty());
        assert!(work.events.borrow().is_empty() && work.observations.borrow().is_empty());
    }
}
#[test]
fn unit3b1_frozen_single_guard_and_reservation_receipts() {
    for (case, authority) in [
        ("guard-empty", "typing-contracts-v1"),
        ("guard-record-only", "typing-contracts-v1"),
        ("guard-array-free", "typing-contracts-v1"),
        ("structural-identities-pairwise-distinct", "contracts-v2"),
        ("reference-access-modes", "typing-contracts-v1"),
    ] {
        let (sources, ast) = single(case, authority);
        let work = WorkMeter::default();
        let owner = SourceOwner::original(
            sources.get(SourceFileId(0)),
            &ast,
            SourceView::Map(&sources),
        )
        .unwrap();
        let typed = typeck::check(
            resolve::resolve_array_types(
                owner,
                IndexLimits::default(),
                &work,
                &mut Allocator::default(),
            )
            .unwrap(),
        )
        .unwrap();
        guards(case, &typed, &sources);
    }
    for case in [
        "scalar-i32-length-0",
        "scalar-i32-length-1",
        "grouped-complete-access-and-index",
        "literal-length-max-trailing-comma",
        "literal-nested-nonempty-is-nonscalar",
    ] {
        let (sources, ast) = single(case, "contracts-v2");
        let owner = SourceOwner::original(
            sources.get(SourceFileId(0)),
            &ast,
            SourceView::Map(&sources),
        )
        .unwrap();
        reservation_rows(case, owner, &sources);
    }
}
#[cfg(target_os = "linux")]
#[test]
fn unit3b1_frozen_project_guard_and_reservation_receipts() {
    use crate::frontend::project::{ProjectLimits, ProjectSources};
    for (case, authority, is_guard) in [
        ("guard-project-root-eof", "typing-contracts-v1", true),
        ("child-route-parameter", "contracts-v2", true),
        ("child-route-shared-parameter", "contracts-v2", true),
        ("child-route-exclusive-parameter", "contracts-v2", true),
        ("child-route-annotation", "contracts-v2", true),
        ("reserve-across-modules", "typing-contracts-v1", false),
    ] {
        let directory = format!("{ROOT}/{authority}/fixtures/{case}");
        let project = ProjectSources::load_array_candidate(
            &format!("{directory}/main.ox"),
            ProjectLimits::default(),
            &mut Allocator::default(),
        )
        .unwrap();
        // Inert presentation map retains original file order and bytes; only
        // the path prefix becomes the source contract's logical fixture path.
        let mut display = SourceMap::new();
        for file in project.sources().files() {
            let logical = std::path::Path::new(file.path())
                .strip_prefix(&directory)
                .unwrap()
                .to_str()
                .unwrap();
            display.add(logical.into(), file.text().into());
        }
        let owner = SourceOwner::project(&project);
        if is_guard {
            let work = WorkMeter::default();
            let typed = typeck::check(
                resolve::resolve_array_types(
                    owner,
                    IndexLimits::default(),
                    &work,
                    &mut Allocator::default(),
                )
                .unwrap(),
            )
            .unwrap();
            guards(case, &typed, &display);
        } else {
            let work = WorkMeter::default();
            work.enable_observation(); // Fixed 116-byte, two-file source control.
            let typed = typeck::check(
                resolve::resolve_array_types(
                    owner,
                    IndexLimits::default(),
                    &work,
                    &mut Allocator::default(),
                )
                .unwrap(),
            )
            .unwrap();
            assert_eq!(typed.functions().len(), 2);
            let added: u64 = work
                .events
                .borrow()
                .iter()
                .filter(|e| e.operation.starts_with("array "))
                .map(|e| e.units)
                .sum();
            println!("CONTROL {{\"schema\":\"oxid-array-types-work-rfc0030\",\"kind\":\"work\",\"reservation_scan_delta\":13,\"seam\":\"private-owned-without-selector\",\"case_id\":\"reserve-across-modules\",\"limit\":{},\"used\":{},\"added_units\":{added},\"success\":true,\"predecessor_ledger_sha256\":\"b49d52901fe25cb190701cf228dd8cb6c1fd7f3375a0543a27e06fcaa156a67b\",\"diagnostic\":null,\"human\":null,\"json_line\":null}}",work.limit(),work.used());
            const PREDECESSOR_WORK: u64 = 321;
            const RFC0030_RESERVATION_SCAN: u64 = 2 * 2 + 3 * 3;
            assert_eq!(
                (work.used(), added),
                (PREDECESSOR_WORK + RFC0030_RESERVATION_SCAN, 16)
            );
            reservation_rows(case, owner, &display);
        }
    }
}

#[test]
fn unit3b1_guard_sentinels_observe_the_existing_executable_path() {
    let (sources, ast) = single("record-len-field-stays-field", "contracts-v2");
    let source = sources.get(SourceFileId(0));
    let typed = typeck::check(resolve::resolve_in_map(source, &ast, &sources).unwrap()).unwrap();
    budget::reset_guard_counts();
    assert!(super::program::check_typed(&typed).is_ok());
    let c = budget::guard_counts();
    println!("CONTROL {{\"schema\":\"{SCHEMA}\",\"kind\":\"sentinel_calibration\",\"case_id\":\"record-len-field-stays-field\",\"success\":true,\"sentinels\":{{\"declaration_admission\":{},\"function_iteration\":{},\"count_entry\":{},\"inventory_entry\":{},\"block_allocation\":{},\"raw_reservation\":{},\"emit_step\":{}}}}}",c[0],c[1],c[2],c[3],c[4],c[5],c[6]);
    assert!(c.into_iter().all(|count| count > 0));
}
