//! Direct source identity and exact-origin adversaries for RFC0030.
use super::*;
use crate::frontend::{lexer, parser};

fn fixture() -> (SourceMap, ast::Program, Program) {
    let text = "fn a(x:i32,y:i32)->i32{let b=x.to_u8_checked();let c=y.to_u8_checked();return b.to_i32();} fn other(x:i32)->u8{return x.to_u8_checked();}";
    let mut sources = SourceMap::new();
    let file = sources.add("byte-auth.ox".into(), text.into());
    let source = sources.get(file);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let typed = typeck::check(hir::resolve(source, &ast).unwrap()).unwrap();
    let raw = lower::lower(&typed).unwrap();
    (sources, ast, raw)
}
fn positions(raw: &Program) -> Vec<(usize, usize)> {
    raw.functions
        .iter()
        .enumerate()
        .flat_map(|(f, function)| {
            function.blocks[0]
                .statements
                .iter()
                .enumerate()
                .filter_map(move |(s, statement)| {
                    statement
                        .as_assignment()
                        .filter(|assign| {
                            matches!(
                                assign.value,
                                Rvalue::CheckedI32ToU8 { .. } | Rvalue::U8ToI32 { .. }
                            )
                        })
                        .map(|_| (f, s))
                })
        })
        .collect()
}
fn assignment(raw: &mut Program, at: (usize, usize)) -> &mut Assign {
    raw.functions[at.0].blocks[0].statements[at.1].assignment_mut()
}
fn reject(raw: Program, sources: &SourceMap, ast: &ast::Program) {
    let error = authenticate_scalar(raw, sources, Declarations::Original(ast)).unwrap_err();
    assert_eq!((error.code, error.stage), ("E0500", "oir-project-bind"));
    assert_eq!(
        error.primary, None,
        "unauthenticated origins must not be rendered"
    );
}
#[test]
fn direct_raw_seam_cannot_create_a_conversion_witness() {
    let (sources, ast, raw) = fixture();
    let failure = verify::verify(raw.clone(), &sources).unwrap_err();
    assert_eq!(failure.kind, FailureKind::UnauthenticatedConversion);
    assert_eq!(failure.diagnostic(&sources).primary, None);
    let mut forged = raw.clone();
    let at = positions(&forged)[0];
    assignment(&mut forged, at).span = sources
        .get(crate::frontend::source::SourceFileId(0))
        .span(0, 1);
    let failure = verify::verify(forged, &sources).unwrap_err();
    assert_eq!(failure.kind, FailureKind::UnauthenticatedConversion);
    assert_eq!(
        failure.diagnostic(&sources).primary,
        None,
        "in-bounds is not authenticated"
    );
    verify::verify_associated(
        authenticate_scalar(raw, &sources, Declarations::Original(&ast)).unwrap(),
    )
    .unwrap();
}
#[test]
fn source_identity_operation_ids_and_individually_valid_origins_are_authenticated() {
    let (sources, ast, baseline) = fixture();
    let positions = positions(&baseline);
    let first = positions[0];
    let second = positions[1];
    let cross_function = positions[3];
    for mutation in 0..8 {
        let mut raw = baseline.clone();
        let other = assignment(
            &mut raw,
            if mutation == 4 {
                cross_function
            } else {
                second
            },
        )
        .clone();
        let target = assignment(&mut raw, first);
        let Rvalue::CheckedI32ToU8 {
            operand: other_operand,
            name_span: other_name,
            source_expr: other_id,
        } = other.value
        else {
            panic!()
        };
        let Rvalue::CheckedI32ToU8 {
            operand,
            name_span,
            source_expr,
        } = &mut target.value
        else {
            panic!()
        };
        match mutation {
            0 => *source_expr = ast::ExprId(usize::MAX),
            1 => *source_expr = ast::ExprId(0), // receiver Name, not Conversion
            2 => *name_span = other_name,
            3 => operand.span = other_operand.span,
            4 | 5 => *source_expr = other_id,
            6 => target.span = other.span,
            7 => {
                target.value = Rvalue::U8ToI32 {
                    operand: *operand,
                    name_span: *name_span,
                    source_expr: *source_expr,
                }
            }
            _ => unreachable!(),
        }
        reject(raw, &sources, &ast);
    }
    let mut stale = SourceMap::new();
    stale.add(
        "byte-auth.ox".into(),
        sources
            .get(crate::frontend::source::SourceFileId(0))
            .text()
            .into(),
    );
    reject(baseline, &stale, &ast);
}
#[test]
fn conversion_authentication_rejects_reused_snapshots_and_different_named_bindings() {
    let (sources, ast, baseline) = fixture();
    let first = positions(&baseline)[0];
    for mutation in 0..5 {
        let mut raw = baseline.clone();
        let foreign_same_name = raw.functions[1].locals[0].span;
        let f = &mut raw.functions[first.0];
        match mutation {
            0 => {
                f.blocks[0].statements.swap(first.1 - 1, first.1);
            }
            1 => {
                let snapshot = f.blocks[0].statements[first.1 - 1].assignment_mut();
                let Rvalue::Copy(operand) = &mut snapshot.value else {
                    panic!()
                };
                operand.local = LocalId(1); // y instead of x
            }
            2 => {
                let destination = f.blocks[0].statements[first.1 - 1].assignment().destination;
                f.locals[destination.0].kind = LocalKind::Binding;
            }
            3 => {
                let target = f.blocks[0].statements[first.1].assignment_mut();
                let Rvalue::CheckedI32ToU8 { operand, .. } = &mut target.value else {
                    panic!()
                };
                operand.local = LocalId(0); // bypass the ordinary snapshot
            }
            4 => {
                f.locals[0].span = foreign_same_name;
            }
            _ => unreachable!(),
        }
        reject(raw, &sources, &ast);
    }
}
#[test]
fn conversion_authentication_carriers_and_actual_work_are_reported() {
    use std::mem::{align_of, size_of};
    let (sources, ast, raw) = fixture();
    let usage = scalar(&raw, &sources, Declarations::Original(&ast)).unwrap();
    let block = &raw.functions[0].blocks[0];
    let previous_lookup = |index: usize| block.statements.get(index);
    assert_eq!(
        std::mem::size_of_val(&previous_lookup),
        size_of::<&BasicBlock>()
    );
    assert_eq!(usage.count, usage.validation);
    let assignments = raw
        .functions
        .iter()
        .flat_map(|f| &f.blocks)
        .flat_map(|b| &b.statements)
        .filter(|s| s.as_assignment().is_some())
        .count();
    let conversion_work: usize = raw
        .functions
        .iter()
        .flat_map(|f| &f.blocks)
        .flat_map(|b| &b.statements)
        .filter_map(Statement::as_assignment)
        .map(|assign| match assign.value {
            Rvalue::CheckedI32ToU8 { operand, .. } => {
                29 + 13 + operand.span.end - operand.span.start
            }
            Rvalue::U8ToI32 { operand, .. } => 29 + 6 + operand.span.end - operand.span.start,
            _ => 0,
        })
        .sum();
    // One function-count check, two owner walks, one prefix zero, each word
    // zero, two observed-capacity checks, then actual assignment/auth work.
    let constructor = 2 + 1 + ast.expressions.len().div_ceil(64) + 2;
    assert_eq!(
        usage.dimensions,
        1 + constructor + assignments + conversion_work
    );
    println!("u8_auth carriers_bytes={} carriers_align={} associated_bytes={} associated_align={} result_bytes={} usage={usage:?}", conversion_carrier_bytes(), align_of::<ConversionAuthenticationCarriers>(), size_of::<AssociatedScalar<'_>>(), align_of::<AssociatedScalar<'_>>(), size_of::<Result<AssociatedScalar<'_>,Box<Diagnostic>>>());
    println!(
        "u8_seen_carriers={} error_envelope={} visitor=({}, {}) owners=({}, {})",
        conversion_seen_carrier_bytes(),
        association_error_bytes(),
        size_of::<Visitor<'_>>(),
        align_of::<Visitor<'_>>(),
        size_of::<ConversionOwners<'_>>(),
        align_of::<ConversionOwners<'_>>()
    );
    println!(
        "u8_scalar_caller_carriers={} caller_align={} modeled_scratch={:?}",
        scalar_association_carrier_bytes(),
        align_of::<ScalarAssociationCarriers>(),
        LAST_SCALAR_SCRATCH.get()
    );
    assert!(conversion_carrier_bytes() > 0);
    // This is counted proof work, not stack or whole-process memory evidence.
    assert!(
        usage.dimensions
            > raw
                .functions
                .iter()
                .flat_map(|f| &f.blocks)
                .map(|b| b.statements.len())
                .sum::<usize>()
    );
}

#[test]
fn exact_duplicate_conversion_occurrence_cannot_reuse_a_source_expression() {
    let (sources, ast, mut raw) = fixture();
    let (function, conversion) = positions(&raw)[0];
    let f = &mut raw.functions[function];
    let mut snapshot = f.blocks[0].statements[conversion - 1].assignment().clone();
    let mut duplicate = f.blocks[0].statements[conversion].assignment().clone();
    let snapshot_local = LocalId(f.locals.len());
    f.locals.push(f.locals[snapshot.destination.0].clone());
    let conversion_local = LocalId(f.locals.len());
    f.locals.push(f.locals[duplicate.destination.0].clone());
    snapshot.destination = snapshot_local;
    duplicate.destination = conversion_local;
    let Rvalue::CheckedI32ToU8 { operand, .. } = &mut duplicate.value else {
        panic!()
    };
    operand.local = snapshot_local;
    f.blocks[0].statements.push(Statement::Assign(snapshot));
    f.blocks[0].statements.push(Statement::Assign(duplicate));
    // Every individual origin and the fresh SSA destinations look plausible;
    // source occurrence identity must independently prohibit the second use.
    reject(raw, &sources, &ast);
}

#[test]
fn builtin_index_is_rejected_before_any_seen_allocation() {
    use crate::frontend::{
        declaration_index::{collect_originals, IndexLimits, SourceOwner, WorkMeter},
        project::budget::Allocator,
    };
    let project = super::root_projection_tests::project("use std::io::write_stdout; fn main()->i32{let x=255;let b=x.to_u8_checked();return b.to_i32();}");
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let index = collect_originals(
        SourceOwner::project(&project),
        IndexLimits::default(),
        &work,
        &mut allocator,
    )
    .unwrap()
    .finish(&work, &mut allocator)
    .unwrap();
    assert!(index.function_count() > index.source_function_count());
    let (_, _, mut raw) = fixture();
    raw.functions
        .resize(index.function_count(), raw.functions[0].clone());
    conversion_seen::assert_no_reservation(|| {
        let failure =
            authenticate_scalar(raw, project.sources(), Declarations::Project(&index)).unwrap_err();
        assert_eq!((failure.code, failure.stage), ("E0500", "oir-project-bind"));
        assert_eq!(failure.primary, None);
    });
}

#[test]
fn real_no_conversion_association_never_enters_tracker_reservation() {
    let mut sources = SourceMap::new();
    let file = sources.add(
        "no-byte-auth.ox".into(),
        "fn main()->i32{return 91;}".into(),
    );
    let source = sources.get(file);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let typed = typeck::check(hir::resolve(source, &ast).unwrap()).unwrap();
    let raw = lower::lower(&typed).unwrap();
    conversion_seen::assert_no_reservation(|| {
        let associated = authenticate_scalar(raw, &sources, Declarations::Original(&ast)).unwrap();
        assert_eq!(
            LAST_SCALAR_SCRATCH.get(),
            Some(scalar_association_carrier_bytes() + association_error_bytes())
        );
        verify::verify_associated(associated).unwrap();
    });
}

#[test]
fn forged_conversion_against_genuine_no_conversion_source_is_extent_bounded_and_rejected() {
    let mut sources = SourceMap::new();
    let file = sources.add(
        "no-byte-forgery.ox".into(),
        "fn main()->i32{let x=91;return x;}".into(),
    );
    let source = sources.get(file);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let typed = typeck::check(hir::resolve(source, &ast).unwrap()).unwrap();
    let mut raw = lower::lower(&typed).unwrap();
    let target = raw.functions[0].blocks[0].statements[0].assignment_mut();
    target.value = Rvalue::CheckedI32ToU8 {
        operand: Operand {
            local: LocalId(0),
            span: target.span,
        },
        name_span: target.span,
        source_expr: ast::ExprId(0), // genuine AST node is an i32 literal
    };
    reject(raw, &sources, &ast);
    // Raw conversion presence does not supply the extent or a caller token.
    // This source-derived request remains bounded before the kind rejection.
    assert_eq!(
        LAST_SCALAR_SCRATCH.get(),
        Some(
            scalar_association_carrier_bytes()
                + conversion_seen_bytes(1, ast.expressions.len()).unwrap()
        )
    );
}

#[test]
fn byte_storage_inherited_auth_walk_counters_refuse_overflow_and_mismatch() {
    // Association is a structurally bounded count/validate walk, not a tunable
    // work budget. Exercise its actual checked arithmetic/equality contracts.
    let mut sources = SourceMap::new();
    let id = sources.add("auth-count.ox".into(), "fn main()->i32{return 0;}".into());
    let at = sources.get(id).span(0, 1);
    let mut declarations = Visitor::count();
    declarations.counts.declarations = usize::MAX;
    assert!(declarations.declaration().is_err());
    assert_eq!(declarations.counts.declarations, usize::MAX);
    let mut spans = Visitor::count();
    spans.counts.spans = usize::MAX;
    assert!(spans.span(at).is_err());
    assert_eq!(spans.counts.spans, usize::MAX);
    let mut dimensions = Visitor::count();
    dimensions.dimensions = usize::MAX;
    assert!(dimensions.dimension(1, 1).is_err());
    assert_eq!(dimensions.dimensions, usize::MAX);
    assert!(Counts {
        declarations: usize::MAX,
        spans: 1
    }
    .total()
    .is_err());
    let mut count = Visitor::count();
    count.declaration().unwrap();
    assert!(Visitor::validate(&sources).finish(count).is_err());
    assert!(Visitor::validate(&sources).dimension(0, 1).is_err());
    let mut count = Visitor::count();
    count.declaration().unwrap();
    count.span(at).unwrap();
    let mut validate = Visitor::validate(&sources);
    validate.declaration().unwrap();
    validate.span(at).unwrap();
    let result = validate.finish(count).unwrap();
    assert_eq!(
        result.count,
        Counts {
            declarations: 1,
            spans: 1
        }
    );
    assert_eq!(result.validation, result.count);
}
