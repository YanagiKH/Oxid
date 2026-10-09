//! Source producer controls for distinct Current/Executable and private
//! Output/BuiltinPipeline routes. Neither route may relabel the other.
use super::super::super::{budget as raw_budget, consumer_fixtures as fixture};
use super::super::{budget as source_budget, builtin_lower, lower, resolve, reviewer_source};
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
    with_index_kind(input, output, false, run)
}
fn with_each_index(input: u8, output: u8, mut run: impl FnMut(&DeclarationIndex<'_>, &SourceMap)) {
    with_index_kind(input, output, false, &mut run);
    with_index_kind(input, output, true, &mut run);
}
fn with_index_kind(
    input: u8,
    output: u8,
    current: bool,
    run: impl FnOnce(&DeclarationIndex<'_>, &SourceMap),
) {
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
    with_project_kind(&[("main.ox", &text)], current, run);
}

fn with_each_project(
    files: &[(&str, &str)],
    mut run: impl FnMut(&DeclarationIndex<'_>, &SourceMap),
) {
    with_project_kind(files, false, &mut run);
    with_project_kind(files, true, &mut run);
}
fn with_project_kind(
    files: &[(&str, &str)],
    current: bool,
    run: impl FnOnce(&DeclarationIndex<'_>, &SourceMap),
) {
    with_project_load_kind(files, current, |loaded| {
        let project = loaded.unwrap();
        let owner = SourceOwner::project(&project);
        let work = WorkMeter::default();
        let mut allocator = Allocator::default();
        let collect = if current {
            declaration_index::collect_originals
        } else {
            declaration_index::collect_output_candidate
        };
        let index = collect(owner, IndexLimits::default(), &work, &mut allocator)
            .unwrap()
            .finish(&work, &mut allocator)
            .unwrap();
        run(&index, project.sources());
    });
}

#[cfg(not(target_os = "linux"))]
fn with_project_load(
    files: &[(&str, &str)],
    run: impl FnOnce(Result<ProjectSources, crate::frontend::project::LoadFailure>),
) {
    with_project_load_kind(files, false, run)
}
fn with_project_load_kind(
    files: &[(&str, &str)],
    current: bool,
    run: impl FnOnce(Result<ProjectSources, crate::frontend::project::LoadFailure>),
) {
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
    for (name, text) in files {
        std::fs::write(fixture.0.join(name), text).unwrap();
    }
    let path = fixture.0.join("main.ox");
    run(if current {
        ProjectSources::load_typed(path.to_str().unwrap(), ProjectLimits::default())
    } else {
        ProjectSources::load_output_candidate(
            path.to_str().unwrap(),
            ProjectLimits::default(),
            &mut Allocator::default(),
        )
    });
}

fn type_source<'s>(
    index: &'s DeclarationIndex<'s>,
    work: &'s WorkMeter,
    allocator: &mut Allocator,
) -> Result<super::super::typeck::TypedOwnedProgram<'s>, Vec<Diagnostic>> {
    if index.is_current_source_pipeline() {
        resolve::type_enum_source(index, work, allocator)
    } else {
        resolve::type_builtin_source(index, work, allocator)
    }
}
fn check_source(
    raw: &RawOwnedProgram,
    index: &DeclarationIndex<'_>,
    sources: &SourceMap,
) -> Result<BindUsage, Box<Diagnostic>> {
    if index.is_current_source_pipeline() {
        check(raw, index, sources)
    } else {
        check_builtin_candidate(raw, index, sources)
    }
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
            let usage = check_builtin_candidate(&raw, index, sources).unwrap();
            assert_eq!(usage.count, usage.validation);
            assert!(check(&raw, index, sources).is_err());
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
            function(
                &raw,
                &mut count,
                true,
                Some(kind),
                FunctionSource::Count,
                &mut ConversionSeen::empty(),
            )
            .unwrap();
            let mut validation = Visitor::validate(sources);
            validation.file(raw.span.file);
            function(
                &raw,
                &mut validation,
                true,
                Some(kind),
                FunctionSource::Count,
                &mut ConversionSeen::empty(),
            )
            .unwrap();
            assert!(function(
                &raw,
                &mut Visitor::count(),
                true,
                None,
                FunctionSource::Count,
                &mut ConversionSeen::empty()
            )
            .is_err());
            let other = if output {
                BuiltinFunction::ReadStdin
            } else {
                BuiltinFunction::WriteStdout
            };
            assert!(function(
                &raw,
                &mut Visitor::count(),
                true,
                Some(other),
                FunctionSource::Count,
                &mut ConversionSeen::empty()
            )
            .is_err());
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
            assert!(function(
                &raw,
                &mut Visitor::count(),
                true,
                Some(kind),
                FunctionSource::Count,
                &mut ConversionSeen::empty()
            )
            .is_err());
            let mut validation = Visitor::validate(sources);
            validation.file(raw.span.file);
            assert!(function(
                &raw,
                &mut validation,
                true,
                Some(kind),
                FunctionSource::Count,
                &mut ConversionSeen::empty()
            )
            .is_err());
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
    println!("OUTPUT_SOURCE_LOWER_CARRIERS producer={} association={} invocation_controls={} raw_function_payload={} output_source_admission=CURRENT_AND_PRIVATE_SEPARATE",
        builtin_lower::carrier_bytes(), builtin_carrier_bytes(),
        super::super::lower::invocation_control_bytes(), raw_payload);
}

#[test]
fn bounded_output_paid_lowering_all_nine_reconcile_both_preflights_and_exact_boundaries() {
    for (expected, input, output) in CASES {
        with_each_index(input, output, |index, sources| {
            let work = WorkMeter::default();
            let typed = type_source(index, &work, &mut Allocator::default()).unwrap();
            let seed = typed.source_storage_bytes().unwrap();
            let (usage, heap) = reviewer_source::integration_measured(|| {
                source_budget::preflight(&typed, source_budget::Limits::DEFAULT)
            });
            let usage = usage.unwrap();
            assert_eq!(heap, (0, 0, 0));
            source_budget::reset_guard_counts();
            let (denied, heap) = reviewer_source::integration_measured(|| {
                source_budget::fail_allocation_after(0, || {
                    lower::lower_with_limits(
                        &typed,
                        source_budget::Limits {
                            raw_bytes: usage.raw_bytes - 1,
                        },
                    )
                })
            });
            assert_eq!(
                denied.unwrap_err().kind,
                OwnedFailureKind::Resource("source raw payload")
            );
            assert_eq!(heap, (0, 0, 0));
            assert_eq!(source_budget::guard_counts()[5], 0);
            let raw = lower::lower_with_limits(
                &typed,
                source_budget::Limits {
                    raw_bytes: usage.raw_bytes,
                },
            )
            .unwrap();
            assert_eq!(raw.builtins, expected);
            assert_eq!(
                raw.enums.len(),
                1 + usize::from(input != 0) + usize::from(output != 0)
            );
            assert_eq!(
                raw.functions.len(),
                2 + usize::from(input == 2) + usize::from(output == 2)
            );
            let actual = raw_budget::preflight(&raw, raw_budget::Limits::DEFAULT).unwrap();
            assert_eq!(usage.analysis, actual);
            assert_eq!(
                raw_budget::preflight(
                    &raw,
                    raw_budget::Limits {
                        metadata: actual.metadata_bytes,
                        ..raw_budget::Limits::DEFAULT
                    }
                )
                .unwrap(),
                actual
            );
            let (denied, heap) = reviewer_source::integration_measured(|| {
                raw_budget::preflight(
                    &raw,
                    raw_budget::Limits {
                        metadata: actual.metadata_bytes - 1,
                        ..raw_budget::Limits::DEFAULT
                    },
                )
            });
            assert_eq!(
                denied.unwrap_err().kind,
                OwnedFailureKind::Resource("ownership metadata")
            );
            assert_eq!(heap, (0, 0, 0));
            let association = check_source(&raw, index, sources).unwrap();
            assert_eq!(association.count, association.validation);
            if index.is_current_source_pipeline() {
                assert!(check_builtin_candidate(&raw, index, sources).is_err());
            } else {
                assert!(check(&raw, index, sources).is_err());
            }
            if expected != BuiltinOrigins::None || !index.is_current_source_pipeline() {
                assert!(check_enum_candidate(&raw, index, sources).is_err());
            }
            verified::verify_owned(raw, sources).unwrap();
            assert_eq!(typed.source_storage_bytes(), Some(seed));
            if index.is_current_source_pipeline() {
                // Exercise the ordinary production checked-owner constructor too.
                drop(super::super::program::check_typed(&typed).unwrap());
                assert_eq!(typed.source_storage_bytes(), Some(seed));
            }
            println!("OUTPUT_PAID_LOWER route={:?} inventory={expected:?} source_bytes={} metadata={} scratch={} seed={seed}",
                typed.admission(), usage.raw_bytes, actual.metadata_bytes, usage.scratch_bytes);
        });
    }
}

#[test]
fn bounded_output_paid_lowering_every_reservation_failure_drops_prior_payload() {
    for (input, output) in [(0, 2), (2, 2)] {
        with_each_index(input, output, |index, sources| {
            let work = WorkMeter::default();
            let typed = type_source(index, &work, &mut Allocator::default()).unwrap();
            let seed = typed.source_storage_bytes();
            source_budget::reset_guard_counts();
            let raw = lower::lower(&typed).unwrap();
            let attempts = source_budget::guard_counts()[5];
            check_source(&raw, index, sources).unwrap();
            verified::verify_owned(raw, sources).unwrap();
            assert!(attempts > 5 * (1 + usize::from(input == 2)));
            for attempt in 0..attempts {
                let (failure, (_, live, _)) = reviewer_source::integration_measured(|| {
                    source_budget::fail_allocation_after(attempt, || lower::lower(&typed))
                        .unwrap_err()
                });
                assert_eq!(
                    failure.kind,
                    OwnedFailureKind::Resource("injected source allocation failure"),
                    "attempt {attempt}"
                );
                assert_eq!(live, 0, "attempt {attempt}");
                assert_eq!(typed.source_storage_bytes(), seed);
            }
            let (_, (_, live, peak)) = reviewer_source::integration_measured(|| {
                drop(
                    source_budget::fail_allocation_after(attempts, || lower::lower(&typed))
                        .unwrap(),
                );
            });
            assert_eq!(live, 0);
            assert!(peak > 0);
            println!("OUTPUT_LOWER_RESERVATIONS inventory={:?} attempts={attempts} live={live} peak={peak}", index.builtin_set());
        });
    }
}

#[test]
fn bounded_output_paid_association_rejects_identity_anchor_opcode_and_role_tampering() {
    with_each_index(2, 2, |index, sources| {
        let work = WorkMeter::default();
        let typed = type_source(index, &work, &mut Allocator::default()).unwrap();
        let input = index
            .builtin_function_id(BuiltinFunction::ReadStdin)
            .unwrap()
            .0;
        let output = index
            .builtin_function_id(BuiltinFunction::WriteStdout)
            .unwrap()
            .0;
        let status = index.builtin_enum_id(BuiltinEnum::WriteStatus).unwrap().0;
        // Byte-identical replacement text still cannot replace the source
        // allocation bound to the real production parser/index owner.
        let file = index
            .sources()
            .file(crate::frontend::project::ModuleId(0))
            .unwrap();
        let ast = index
            .sources()
            .ast(crate::frontend::project::ModuleId(0))
            .unwrap();
        let mut substitute = SourceMap::new();
        substitute.add(file.path().into(), file.text().into());
        assert!(SourceOwner::original(
            file,
            ast,
            crate::frontend::source::SourceView::Map(&substitute)
        )
        .is_err());
        for mutation in 0..17 {
            let mut raw = lower::lower(&typed).unwrap();
            check_source(&raw, index, sources).unwrap();
            let wrong_anchor = index
                .builtin_function_anchor(BuiltinFunction::ReadStdin)
                .unwrap();
            match mutation {
                0 => raw.builtins = BuiltinOrigins::None,
                1 => raw.functions[output].references[0].kind = BorrowKind::Exclusive,
                2 => raw.functions[output].result = raw.functions[input].result,
                3 => {
                    raw.enums[status].variants[0].payload = raw.enums[status].variants[2].payload;
                    raw.enums[status].variants[2].payload = None;
                }
                4 => {
                    let function = &mut raw.functions[output];
                    function.span = wrong_anchor;
                    function.owners[0].span = wrong_anchor;
                    function.references[0].span = wrong_anchor;
                    function.blocks[0].span = wrong_anchor;
                    for statement in &mut function.blocks[0].statements {
                        statement.span = wrong_anchor;
                    }
                    function.blocks[0].terminator.as_mut().unwrap().span = wrong_anchor;
                    builtins::check(&raw).unwrap();
                }
                5 => {
                    raw.enums[status].span = wrong_anchor;
                    for variant in &mut raw.enums[status].variants {
                        variant.span = wrong_anchor;
                    }
                    builtins::check(&raw).unwrap();
                }
                6 => raw.functions.swap(input, output),
                7 => {
                    raw.functions[output].blocks[0].statements[1].kind =
                        OwnedInstruction::ReadStdin {
                            buffer: ReferenceParamId(0),
                            destination: OwnerPlaceId(0),
                        }
                }
                8 | 10 => {
                    let span = raw.functions[0].span;
                    raw.functions[0].blocks[0]
                        .statements
                        .push(fixture::instruction(
                            if mutation == 8 {
                                OwnedInstruction::WriteStdout {
                                    buffer: ReferenceParamId(0),
                                    destination: OwnerPlaceId(0),
                                }
                            } else {
                                OwnedInstruction::ReadStdin {
                                    buffer: ReferenceParamId(0),
                                    destination: OwnerPlaceId(0),
                                }
                            },
                            span,
                        ));
                    builtins::check(&raw).unwrap();
                }
                9 => {
                    let mut block = raw.functions[0].blocks[0].clone();
                    block.statements.push(fixture::instruction(
                        OwnedInstruction::WriteStdout {
                            buffer: ReferenceParamId(0),
                            destination: OwnerPlaceId(0),
                        },
                        block.span,
                    ));
                    raw.functions[0].blocks.push(block);
                    builtins::check(&raw).unwrap();
                }
                11 => raw.enums[status].id = EnumId(0),
                12 => raw.functions[output].id = hir::DefId(0),
                13 => {
                    raw.enums.pop();
                }
                14 => {
                    raw.functions.pop();
                }
                15 => raw.functions[output].references[0].position = 1,
                16 => {
                    raw.functions[output].references[0].referent =
                        BorrowedSlot::check(BorrowedTy::ScalarSlice(hir::Ty::Bool)).unwrap()
                }
                _ => unreachable!(),
            }
            assert_eq!(
                check_source(&raw, index, sources).unwrap_err().code,
                "E0500",
                "mutation {mutation}"
            );
        }
    });
}

#[test]
fn bounded_output_paid_source_calls_and_child_aliases_preserve_earliest_family_anchors() {
    const ROOT: &str = r#"
use std::io::WriteStatus as FirstStatus;
use std::io::read_stdin as input;
use std::io::ReadStatus as InputStatus;
pub mod child;
use crate::child::relay;
fn write_stdout()->i32{return 9;}
fn main()->i32{
    let mut bytes:[i32;0]=[];
    let status=input(&mut bytes);
    match status {
        InputStatus::Eof(n)=>{return n;},
        InputStatus::Full=>{return relay(&bytes);},
        InputStatus::IoError=>{return 7;},
    }
}
"#;
    const CHILD: &str = r#"
use std::io::write_stdout as send;
use std::io::WriteStatus as Status;
pub fn relay(bytes:&[i32])->i32{
    let status=send(&*bytes);
    match status {
        Status::Complete=>{return 0;},
        Status::InvalidInput=>{return 1;},
        Status::IoError(n)=>{return n;},
    }
}
"#;
    #[cfg(not(target_os = "linux"))]
    with_project_load(&[("main.ox", ROOT), ("child.ox", CHILD)], |loaded| {
        let failure = loaded.unwrap_err();
        assert_eq!(failure.diagnostics.len(), 1);
        assert_eq!(
            (failure.diagnostics[0].code, failure.diagnostics[0].stage),
            ("E0005", "source"),
        );
    });
    #[cfg(target_os = "linux")]
    with_each_project(
        &[("main.ox", ROOT), ("child.ox", CHILD)],
        |index, sources| {
            let work = WorkMeter::default();
            let typed = type_source(index, &work, &mut Allocator::default()).unwrap();
            let source_usage =
                source_budget::preflight(&typed, source_budget::Limits::DEFAULT).unwrap();
            let raw = lower::lower(&typed).unwrap();
            assert_eq!(
                source_usage.analysis,
                raw_budget::preflight(&raw, raw_budget::Limits::DEFAULT).unwrap()
            );
            let output = index
                .builtin_function_id(BuiltinFunction::WriteStdout)
                .unwrap();
            let status = index.builtin_enum_id(BuiltinEnum::WriteStatus).unwrap();
            assert_eq!((status.0, output.0), (1, 4));
            assert_eq!(raw.enums[status.0].span.file.0, 0);
            assert_eq!(
                raw.enums[status.0].span.start,
                ROOT.find("WriteStatus").unwrap()
            );
            assert_eq!(raw.functions[output.0].span.file.0, 1);
            assert_eq!(
                raw.functions[output.0].span.start,
                CHILD.find("write_stdout").unwrap()
            );
            assert_eq!(raw.functions[0].result, ValueTy::Scalar(hir::Ty::I32));
            assert!(raw.functions[0].blocks.iter().all(|block| block
                .statements
                .iter()
                .all(|statement| !matches!(statement.kind, OwnedInstruction::WriteStdout { .. }))));
            let usage = check_source(&raw, index, sources).unwrap();
            assert_eq!(usage.count, usage.validation);
            verified::verify_owned(raw, sources).unwrap();
        },
    );
}
