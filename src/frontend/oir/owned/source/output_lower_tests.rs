//! Closed producer controls. These exercise inert raw construction and the
//! ordinary verifier; no output source entry or executable policy is opened.
use super::super::super::{budget as raw_budget, consumer_fixtures as fixture};
use super::super::{budget as source_budget, builtin_lower};
use super::*;
use crate::frontend::{
    declaration_index::{self, IndexLimits, SourceOwner, WorkMeter},
    project::{budget::Allocator, ProjectLimits, ProjectSources},
};
use std::mem::size_of;

// The rows are independent of catalog membership/rank helpers. Source enum and
// function prefix lengths deliberately differ: one enum and two functions.
const CASES: [(BuiltinOrigins, u8, u8); 9] = [
    (BuiltinOrigins::None, 0, 0),
    (BuiltinOrigins::ReadStatus, 1, 0),
    (BuiltinOrigins::ReadStdin, 2, 0),
    (BuiltinOrigins::WriteStatus, 0, 1),
    (BuiltinOrigins::WriteStdout, 0, 2),
    (BuiltinOrigins::ReadStatusWriteStatus, 1, 1),
    (BuiltinOrigins::ReadStatusWriteStdout, 1, 2),
    (BuiltinOrigins::ReadStdinWriteStatus, 2, 1),
    (BuiltinOrigins::ReadStdinWriteStdout, 2, 2),
];

fn with_index(input: u8, output: u8, run: impl FnOnce(&DeclarationIndex<'_>, &SourceMap)) {
    let input_import = match input {
        0 => "",
        1 => "use std::io::ReadStatus as InputStatus;",
        _ => "use std::io::read_stdin as input;",
    };
    let output_import = match output {
        0 => "",
        1 => "use std::io::WriteStatus as OutputStatus;",
        _ => "use std::io::write_stdout as output;",
    };
    // Import order differs from canonical family order.
    let text = format!("{output_import}{input_import} enum User{{Only}} fn helper()->i32{{return 1;}} fn main()->i32{{return 0;}}");
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Fixture(std::path::PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "oxid-closed-output-lowering-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
    )));
    std::fs::create_dir(&fixture.0).unwrap();
    let path = fixture.0.join("main.ox");
    std::fs::write(&path, text).unwrap();
    let project = ProjectSources::load_output_candidate(
        path.to_str().unwrap(),
        ProjectLimits::default(),
        &mut Allocator::default(),
    )
    .unwrap();
    let owner = SourceOwner::project(&project);
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let index = declaration_index::collect_output_candidate(
        owner,
        IndexLimits::default(),
        &work,
        &mut allocator,
    )
    .unwrap()
    .finish(&work, &mut allocator)
    .unwrap();
    run(&index, project.sources());
}

fn raw_prefix(index: &DeclarationIndex<'_>) -> RawOwnedProgram {
    let enums = (0..index.enum_count())
        .map(|ordinal| {
            let id = EnumId(ordinal);
            let view = index.enum_view(id).unwrap();
            RawEnumDecl {
                id,
                span: view.diagnostic_span(),
                variants: (0..view.variant_count())
                    .map(|index| {
                        let id = VariantId {
                            enumeration: id,
                            index,
                        };
                        let variant = view.variant(id).unwrap();
                        RawVariantDecl {
                            id,
                            span: variant.diagnostic_span(),
                            payload: variant
                                .payload()
                                .map(|ty| ParameterTy::Value(ValueTy::Scalar(ty))),
                        }
                    })
                    .collect(),
            }
        })
        .collect();
    let functions = (0..index.source_function_count())
        .map(|ordinal| {
            let (key, module) = index.function(hir::DefId(ordinal)).unwrap();
            let span = index.sources().ast(module).unwrap().functions[key.index].name;
            let mut function = fixture::function(ordinal, ValueTy::Scalar(hir::Ty::I32), span);
            function.locals = vec![fixture::scalar(hir::Ty::I32, span)];
            function.blocks = vec![OwnedBlock {
                span,
                merge: None,
                statements: vec![fixture::assign(
                    0,
                    Rvalue::I32(if ordinal == 0 { 1 } else { 0 }),
                    span,
                )],
                terminator: Some(OwnedTerminator {
                    span,
                    diagnostic_origins: None,
                    kind: OwnedTerminatorKind::ReturnScalar(fixture::operand(0, span)),
                }),
            }];
            function
        })
        .collect();
    RawOwnedProgram {
        builtins: index.builtin_set(),
        enums,
        records: vec![],
        functions,
    }
}

#[test]
fn bounded_output_lower_nine_suffixes_preserve_independent_ranks_and_raw_proofs() {
    for (expected, input, output) in CASES {
        with_index(input, output, |index, sources| {
            assert_eq!(index.builtin_set(), expected);
            for ordinal in [0, 1, 2, 3, 4, usize::MAX] {
                let permission = if input == 2 && ordinal == 2 {
                    Some(BuiltinFunction::ReadStdin)
                } else if output == 2 && ordinal == 2 + usize::from(input == 2) {
                    Some(BuiltinFunction::WriteStdout)
                } else {
                    None
                };
                assert_eq!(
                    builtin_kind(expected, hir::DefId(2), hir::DefId(ordinal)),
                    permission,
                );
            }
            let mut raw = raw_prefix(index);
            if input == 2 {
                let function = builtin_lower::function(index, BuiltinFunction::ReadStdin).unwrap();
                assert_eq!(function.id, hir::DefId(2));
                assert_eq!(
                    function.result,
                    ValueTy::Owned(AggregateTy::Enum(EnumId(1)))
                );
                assert_eq!(function.references[0].kind, BorrowKind::Exclusive);
                raw.functions.push(function);
            }
            if output == 2 {
                let function =
                    builtin_lower::function(index, BuiltinFunction::WriteStdout).unwrap();
                assert_eq!(function.id, hir::DefId(2 + usize::from(input == 2)));
                assert_eq!(
                    function.result,
                    ValueTy::Owned(AggregateTy::Enum(EnumId(1 + usize::from(input != 0))))
                );
                assert_eq!(function.references[0].kind, BorrowKind::Shared);
                raw.functions.push(function);
            }
            if output != 0 {
                let status = &raw.enums[1 + usize::from(input != 0)];
                assert_eq!(status.variants[0].payload, None);
                assert_eq!(status.variants[1].payload, None);
                assert_eq!(
                    status.variants[2].payload,
                    Some(ParameterTy::Value(ValueTy::Scalar(hir::Ty::I32)))
                );
            }
            let mut counts = raw_budget::ProgramCounts::default();
            let enum_usage = admit_enum_counts(
                raw.enums.iter().map(|item| item.variants.len()),
                DeclarationUsage::default(),
            )
            .unwrap();
            raw_budget::account_enum_declarations(
                enum_usage,
                raw_budget::Limits::DEFAULT,
                &mut counts,
            )
            .unwrap();
            raw_budget::account_builtin_descriptors(
                expected,
                raw_budget::Limits::DEFAULT,
                &mut counts,
            )
            .unwrap();
            for _ in 0..2 {
                raw_budget::account_function(
                    raw_budget::FunctionCounts {
                        locals: 1,
                        blocks: 1,
                        statements: 1,
                        ..raw_budget::FunctionCounts::default()
                    },
                    raw_budget::Limits::DEFAULT,
                    &mut counts,
                )
                .unwrap();
            }
            for function in &raw.functions[2..] {
                for (length, capacity, requested) in [
                    (function.parameters.len(), function.parameters.capacity(), 1),
                    (function.owners.len(), function.owners.capacity(), 1),
                    (function.references.len(), function.references.capacity(), 1),
                    (function.blocks.len(), function.blocks.capacity(), 1),
                    (
                        function.blocks[0].statements.len(),
                        function.blocks[0].statements.capacity(),
                        2,
                    ),
                ] {
                    assert_eq!((length, capacity), (requested, requested));
                }
                raw_budget::account_function(
                    builtin_lower::counts(),
                    raw_budget::Limits::DEFAULT,
                    &mut counts,
                )
                .unwrap();
            }
            assert_eq!(
                raw_budget::preflight(&raw, raw_budget::Limits::DEFAULT).unwrap(),
                counts.usage()
            );
            builtins::check(&raw).unwrap();
            if output != 0 {
                // The positive source gate stays shut despite valid inert raw
                // shape, identity and ownership. No source walk is bypassed.
                assert_eq!(
                    check_impl(&raw, index, sources, true, true)
                        .unwrap_err()
                        .code,
                    "E0500"
                );
            } else {
                let usage = check_impl(&raw, index, sources, true, true).unwrap();
                assert_eq!(usage.count, usage.validation);
            }
            verified::verify_owned(raw, sources).unwrap();
        });
    }
}

#[test]
fn bounded_output_lower_missing_family_denies_before_reservation_and_all_requests_fail_closed() {
    for (_, input, output) in CASES {
        with_index(input, output, |index, _| {
            for (kind, present) in [
                (BuiltinFunction::ReadStdin, input == 2),
                (BuiltinFunction::WriteStdout, output == 2),
            ] {
                if !present {
                    source_budget::reset_guard_counts();
                    assert!(
                        source_budget::fail_allocation_after(0, || builtin_lower::function(
                            index, kind
                        ))
                        .is_err()
                    );
                    assert_eq!(source_budget::guard_counts()[5], 0);
                    continue;
                }
                for request in 0..5 {
                    let error = source_budget::fail_allocation_after(request, || {
                        builtin_lower::function(index, kind)
                    })
                    .unwrap_err();
                    assert_eq!(
                        error.kind,
                        OwnedFailureKind::Resource("injected source allocation failure")
                    );
                }
                assert!(
                    source_budget::fail_allocation_after(5, || builtin_lower::function(
                        index, kind
                    ))
                    .is_ok()
                );
            }
        });
    }
}

#[test]
fn bounded_output_association_walk_scopes_each_opcode_and_checks_unreachable_blocks() {
    with_index(2, 2, |index, sources| {
        for (kind, input, output) in [
            (BuiltinFunction::ReadStdin, true, false),
            (BuiltinFunction::WriteStdout, false, true),
        ] {
            let mut raw = builtin_lower::function(index, kind).unwrap();
            let mut count = Visitor::count();
            function(&raw, &mut count, true, Some(kind)).unwrap();
            let mut validation = Visitor::validate(sources);
            validation.file(raw.span.file);
            function(&raw, &mut validation, true, Some(kind)).unwrap();
            assert!(function(&raw, &mut Visitor::count(), true, None).is_err());
            let other = if output {
                BuiltinFunction::ReadStdin
            } else {
                BuiltinFunction::WriteStdout
            };
            assert!(function(&raw, &mut Visitor::count(), true, Some(other)).is_err());
            let mut unreachable = raw.blocks[0].clone();
            unreachable.statements[1].kind = if input {
                OwnedInstruction::WriteStdout {
                    buffer: ReferenceParamId(0),
                    destination: OwnerPlaceId(0),
                }
            } else {
                OwnedInstruction::ReadStdin {
                    buffer: ReferenceParamId(0),
                    destination: OwnerPlaceId(0),
                }
            };
            raw.blocks.push(unreachable);
            assert!(function(&raw, &mut Visitor::count(), true, Some(kind)).is_err());
            let mut validation = Visitor::validate(sources);
            validation.file(raw.span.file);
            assert!(function(&raw, &mut validation, true, Some(kind)).is_err());
        }
    });
}

#[test]
fn bounded_output_lower_measures_complete_finite_producer_and_association_envelopes() {
    assert_eq!(size_of::<Option<BuiltinFunction>>(), size_of::<bool>());
    let raw_payload = size_of::<RawOwnedFunction>()
        + size_of::<ParameterBinding>()
        + size_of::<OwnerDecl>()
        + size_of::<ReferenceDecl>()
        + size_of::<OwnedBlock>()
        + 2 * size_of::<OwnedStatement>();
    assert_eq!(
        source_budget::function_bytes(builtin_lower::counts()).unwrap(),
        raw_payload
    );
    println!("OUTPUT_SOURCE_LOWER_CARRIERS producer={} association={} invocation_controls={} raw_function_payload={} output_source_admission=DENIED",
        builtin_lower::carrier_bytes(), builtin_carrier_bytes(),
        super::super::lower::invocation_control_bytes(), raw_payload);
}
