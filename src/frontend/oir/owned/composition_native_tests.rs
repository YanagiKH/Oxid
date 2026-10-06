//! Native composition coverage uses raw and source witnesses. Numeric results
//! are hand-derived; fuel sweeps compare consumers, not an independent cost oracle.
use super::*;
use crate::frontend::oir::owned::source::resource_fixtures as source;

const PILOT: &str = "struct Meta{completed:i32} struct Batch{meta:Meta,samples:[i32;3]} fn relay(x:Batch)->Batch{return x;} fn bump(p:&mut Batch)->(){let mut i=0;while i<p.samples.len(){p.samples[i]=p.samples[i]+1;p.meta.completed=p.meta.completed+1;i=i+1;}return;} fn main()->i32{let m=Meta{completed:0};let a=[1,2,3];let mut b=relay(Batch{meta:m,samples:a});bump(&mut b);return b.meta.completed*100+b.samples[0]*10+b.samples[2];}";

fn checked_module(text: &str) -> (source::CheckedSource, String) {
    let case = source::checked_arrays(text);
    let module = native_module(&case.witness, Some(case.entry), &case.sources).unwrap();
    (case, module)
}

fn assert_safe_module(module: &str) {
    for forbidden in [
        "inbounds",
        "noalias",
        "nonnull",
        "dereferenceable",
        "memcpy",
        "undef",
        "poison",
    ] {
        assert!(!module.contains(forbidden), "{forbidden}");
    }
    for (position, _) in module.match_indices("_projection_ptr = getelementptr") {
        let prior = &module[..position];
        let success = prior
            .rfind("_bounds_ok:\n")
            .expect("projection only on successful bounds edge");
        let branch = prior[..success].rfind("_in_range, label").unwrap();
        let signed = prior[..branch]
            .rfind("_nonnegative = icmp sge i32")
            .unwrap();
        assert!(signed < branch && branch < success);
    }
}

#[test]
fn native_composition_pilot_has_checked_paths_and_exact_expansion_inventory() {
    let (case, module) = checked_module(PILOT);
    assert_eq!(
        execute::run(&case.witness, Some(case.entry)),
        Ok(Scalar::I32(324))
    );
    assert_safe_module(&module);
    let observation = run_array_observed(
        &case.witness,
        Some(case.entry),
        &case.sources,
        NativeControl::default(),
    );
    assert_eq!(observation.result.unwrap(), module);
    let m = observation.metrics;
    assert_eq!(m.count_bytes, module.len());
    assert_eq!(m.render_bytes, module.len());
    assert_eq!(m.count_expansions, m.transfer_cells + m.message_bytes);
    assert_eq!(m.count_expansions, m.render_expansions);
    assert_eq!(m.count_expansion_kinds, m.render_expansion_kinds);
    assert_eq!(m.count_ordinary_visits, m.render_ordinary_visits);
    assert_eq!(m.count_predecessor_visits, m.render_predecessor_visits);
    assert!(m.metadata_peak <= Limits::DEFAULT.metadata_bytes);
    assert!(size_of::<ScalarLeaves<'_>>() < EMITTER_TRANSIENT_BYTES / 2);
    let exact = NativeControl {
        limits: Limits {
            ir_bytes: module.len(),
            ..Limits::DEFAULT
        },
        ..NativeControl::default()
    };
    assert_eq!(
        run_array_observed(&case.witness, Some(case.entry), &case.sources, exact)
            .result
            .unwrap(),
        module
    );
    let short = NativeControl {
        limits: Limits {
            ir_bytes: module.len() - 1,
            ..Limits::DEFAULT
        },
        ..NativeControl::default()
    };
    let failed = run_array_observed(&case.witness, Some(case.entry), &case.sources, short);
    assert!(failed.result.unwrap_err().message.contains("LLVM bytes"));
    assert_eq!(failed.metrics.render_bytes, 0);
}

#[test]
fn native_composition_every_new_allocation_remains_fallible() {
    let case = source::checked_arrays(PILOT);
    let success = run_array_observed(
        &case.witness,
        Some(case.entry),
        &case.sources,
        NativeControl::default(),
    );
    assert!(success.result.is_ok());
    for failure in 0..success.metrics.allocation_attempts {
        let failed = run_array_observed(
            &case.witness,
            Some(case.entry),
            &case.sources,
            NativeControl {
                fail_after: Some(failure),
                ..NativeControl::default()
            },
        );
        let diagnostic = failed.result.unwrap_err();
        assert_eq!(diagnostic.code, "E0700");
        assert!(diagnostic.message.contains("allocation"));
        assert!(failed.metrics.failed_allocation.is_some());
        assert_eq!(failed.metrics.render_bytes, 0);
    }
}

fn wide_parameter(length: usize) -> (SourceMap, RawOwnedProgram) {
    let (sources, s) = fixtures::context();
    let span = s(0);
    let field = |record, index, ty| RawFieldDecl {
        id: FieldId {
            record: RecordId(record),
            index,
        },
        ty: ParameterTy::Value(ty),
        span,
    };
    let records = vec![
        RawRecordDecl {
            id: RecordId(0),
            span,
            fields: vec![
                field(0, 0, ValueTy::Scalar(hir::Ty::Bool)),
                field(0, 1, ValueTy::Owned(AggregateTy::Record(RecordId(1)))),
                field(0, 2, ValueTy::Scalar(hir::Ty::Unit)),
            ],
        },
        RawRecordDecl {
            id: RecordId(1),
            span,
            fields: (0..4)
                .map(|index| {
                    field(
                        1,
                        index,
                        ValueTy::Owned(AggregateTy::FixedArray(
                            FixedArrayTy::check(hir::Ty::I32, length).unwrap(),
                        )),
                    )
                })
                .collect(),
        },
    ];
    let mut main = fixtures::function(0, ValueTy::Scalar(hir::Ty::Unit), span);
    main.locals = vec![fixtures::scalar(hir::Ty::Unit, span)];
    main.blocks.push(block(
        vec![fixtures::assign(0, Rvalue::Unit, s(1))],
        OwnedTerminatorKind::ReturnScalar(fixtures::operand(0, s(2))),
        s(2),
    ));
    let mut f = fixtures::function(1, ValueTy::Scalar(hir::Ty::Unit), span);
    f.parameters = vec![ParameterBinding::Owned(OwnerPlaceId(0))];
    f.owners = vec![OwnerDecl {
        aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(0))).unwrap(),
        kind: OwnerKind::Parameter { position: 0 },
        span,
    }];
    f.locals = vec![fixtures::scalar(hir::Ty::Unit, span)];
    f.blocks.push(block(
        vec![
            fixtures::instruction(OwnedInstruction::Discard(OwnerPlaceId(0)), s(3)),
            fixtures::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(0)), s(4)),
            fixtures::assign(0, Rvalue::Unit, s(5)),
        ],
        OwnedTerminatorKind::ReturnScalar(fixtures::operand(0, s(6))),
        s(6),
    ));
    (
        sources,
        RawOwnedProgram {
            builtins: BuiltinOrigins::None,
            enums: vec![],
            records,
            functions: vec![main, f],
        },
    )
}

#[test]
fn native_composition_lazy_nested_copy_is_padding_free_and_stops_at_output_cap() {
    let (sources, raw) = wide_parameter(1024);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    assert_eq!(plan.owner_width(hir::DefId(1), OwnerPlaceId(0)), 4098);
    let (cells, visits) = transfer_inventory(&plan).unwrap();
    assert_eq!(cells, 4098);
    assert!(visits < 20);
    let mut count = Emission::count(512);
    transfer(
        &mut count,
        &plan,
        "copy",
        AggregateTy::Record(RecordId(0)),
        "%source",
        "%destination",
    );
    assert!(count.exceeded);
    assert!(count.expansions < 10);
    assert!(count.field_visits < 10);
    let mut out = Emission {
        text: Some(String::new()),
        ..Emission::count(MAX_IR_BYTES)
    };
    transfer(
        &mut out,
        &plan,
        "copy",
        AggregateTy::Record(RecordId(0)),
        "%source",
        "%destination",
    );
    assert_eq!(out.expansions, 4098);
    assert_eq!(out.field_visits, 4098);
    let text = out.text.unwrap();
    assert!(text.contains("%copy_in0_ptr = getelementptr i8, ptr %source, i64 0"));
    assert!(text.contains("%copy_in1_ptr = getelementptr i8, ptr %source, i64 4"));
    assert!(text.contains("%copy_in4097_ptr = getelementptr i8, ptr %source, i64 16388"));
    for padding in [1, 2, 3, 16389, 16390, 16391] {
        assert!(!text.contains(&format!("ptr %source, i64 {padding}\n")));
    }
    assert_safe_module(&text);
    let observed = run_array_observed(
        &witness,
        Some(hir::DefId(0)),
        &sources,
        NativeControl {
            limits: Limits {
                ir_bytes: 512,
                ..Limits::DEFAULT
            },
            ..NativeControl::default()
        },
    );
    assert!(observed.result.unwrap_err().message.contains("LLVM bytes"));
    assert_eq!(observed.metrics.transfer_cells, 4098);
    assert_eq!(observed.metrics.render_bytes, 0);
    assert!(observed.metrics.count_expansions < 10);
}

#[test]
fn native_composition_projected_bounds_are_in_diagnostic_budget() {
    for guard in ["", "fn guard()->(){while false{}return;}"] {
        let text = format!("struct Inner{{xs:[i32;1]}}struct Outer{{pad:bool,inner:Inner}}fn read(p:&Outer)->i32{{return p.inner.xs[-1];}}fn main()->i32{{let x=Outer{{pad:true,inner:Inner{{xs:[7]}}}};return read(&x);}}{guard}");
        let (case, module) = checked_module(&text);
        assert_safe_module(&module);
        let plan = ExecutionPlan::build(&case.witness).unwrap();
        let diagnostics = Diagnostics::new(
            &plan,
            case.entry,
            &case.sources,
            !guard.is_empty(),
            MAX_DIAGNOSTIC_BYTES,
        )
        .unwrap();
        let bounds: Vec<_> = diagnostics
            .ids
            .iter()
            .filter(|(key, _)| key.0 == FailureKind::Bounds)
            .collect();
        assert_eq!(bounds.len(), 1);
        let bytes = diagnostics.messages.iter().map(String::len).sum();
        assert!(native_module_limits(
            &case.witness,
            Some(case.entry),
            &case.sources,
            plan::MAX_FUEL,
            Limits {
                diagnostic_bytes: bytes,
                ..Limits::DEFAULT
            }
        )
        .is_ok());
        assert!(native_module_limits(
            &case.witness,
            Some(case.entry),
            &case.sources,
            plan::MAX_FUEL,
            Limits {
                diagnostic_bytes: bytes - 1,
                ..Limits::DEFAULT
            }
        )
        .unwrap_err()
        .message
        .contains("diagnostic bytes"));
    }
}

#[test]
fn native_composition_maximum_depth_uses_bounded_projection_and_copy_scratch() {
    let (sources, mut raw) = wide_parameter(0);
    let span = raw.functions[0].span;
    raw.records = (0..64)
        .map(|record| RawRecordDecl {
            id: RecordId(record),
            span,
            fields: vec![RawFieldDecl {
                id: FieldId {
                    record: RecordId(record),
                    index: 0,
                },
                ty: ParameterTy::Value(if record == 63 {
                    ValueTy::Scalar(hir::Ty::I32)
                } else {
                    ValueTy::Owned(AggregateTy::Record(RecordId(record + 1)))
                }),
                span,
            }],
        })
        .collect();
    let f = &mut raw.functions[1];
    f.locals.push(fixtures::scalar(hir::Ty::I32, span));
    f.blocks[0].statements.insert(
        0,
        fixtures::instruction(
            OwnedInstruction::ReadProjection {
                destination: LocalId(1),
                base: AccessBase::Owner(OwnerPlaceId(0)),
                path: (0..64)
                    .map(|record| FieldId {
                        record: RecordId(record),
                        index: 0,
                    })
                    .collect(),
                index: None,
            },
            span,
        ),
    );
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let observation = run_array_observed(
        &witness,
        Some(hir::DefId(0)),
        &sources,
        NativeControl::default(),
    );
    let module = observation.result.unwrap();
    assert_eq!(observation.metrics.transfer_cells, 1);
    assert_eq!(observation.metrics.count_expansions, 1);
    assert_eq!(observation.metrics.render_expansions, 1);
    assert!(module.contains("%f1_b0_i0_value = load i32"));
    assert_safe_module(&module);
}

const EMPTY_MIXED: &str = "struct E{} struct Inner{flag:bool,n:i32,u:(),empty:E,zeros:[bool;0]}struct Outer{first:bool,inner:Inner,last:i32}fn relay(x:Outer)->Outer{return x;}fn read(p:&Outer)->i32{p.inner.u;if p.inner.flag&&p.first{return p.inner.n+p.last+p.inner.zeros.len();}else{return 0;}}fn main()->i32{let z:[bool;0]=[];let x=Outer{last:5,inner:Inner{zeros:z,empty:E{},u:(),n:12,flag:true},first:true};let mut y=relay(x);let z2:[bool;0]=[];y=Outer{first:true,inner:Inner{flag:true,n:19,u:(),empty:E{},zeros:z2},last:4};return read(&y);}";
const EFFECTS: &str = "struct Inner{xs:[i32;2]}struct Outer{inner:Inner}fn index(p:&mut Outer)->i32{p.inner.xs[0]=9;return 0;}fn edit(p:&mut Outer)->i32{p.inner.xs[index(&mut *p)]=p.inner.xs[0];return p.inner.xs[0];}fn main()->i32{let mut x=Outer{inner:Inner{xs:[1,2]}};return edit(&mut x);}";
const PHI: &str = "struct Inner{xs:[bool;2]}struct Outer{inner:Inner}fn read(p:&Outer)->bool{return p.inner.xs[0]&&p.inner.xs[1];}fn main()->bool{let x=Outer{inner:Inner{xs:[true,false]}};return read(&x);}";

const MUTATE_TYPES: &str = "struct I{flag:bool,u:(),bs:[bool;2],us:[();2]}struct O{pad:i32,inner:I}fn mutate(p:&mut O)->i32{p.inner.flag=!p.inner.flag;p.inner.u=();p.inner.bs[0]=!p.inner.bs[0];p.inner.us[1]=();if p.inner.flag&&p.inner.bs[0]{return p.inner.us.len()+p.inner.bs.len();}else{return 99;}}fn main()->i32{let mut x=O{pad:7,inner:I{flag:false,u:(),bs:[false,true],us:[(),()]}};return mutate(&mut x);}";

#[test]
fn native_composition_mixed_sentinels_effects_and_phi_emit() {
    for (text, expected) in [
        (EMPTY_MIXED, Scalar::I32(23)),
        (EFFECTS, Scalar::I32(1)),
        (PHI, Scalar::Bool(false)),
        (MUTATE_TYPES, Scalar::I32(4)),
    ] {
        let (case, module) = checked_module(text);
        assert_eq!(execute::run(&case.witness, Some(case.entry)), Ok(expected));
        assert_safe_module(&module);
        if text == PHI {
            let phi = module
                .lines()
                .find(|line| line.contains(" = phi ptr "))
                .unwrap();
            assert!(phi.contains("_bounds_ok ]"));
        }
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7 and one serialized native execution slot"]
fn native_composition_source_free_pilot_sentinels_effects_and_phi() {
    let scratch = Scratch::new();
    for (name, text, expected) in [
        ("composition-pilot", PILOT, Scalar::I32(324)),
        ("composition-sentinels", EMPTY_MIXED, Scalar::I32(23)),
        ("composition-effects", EFFECTS, Scalar::I32(1)),
        ("composition-phi", PHI, Scalar::Bool(false)),
        ("composition-mutate-types", MUTATE_TYPES, Scalar::I32(4)),
    ] {
        let (case, module) = checked_module(text);
        assert_eq!(execute::run(&case.witness, Some(case.entry)), Ok(expected));
        let binary = scratch.compile(&module, name);
        assert_result(scratch.run(&binary, &[]), &scalar_output(expected), b"", 0);
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7 and one serialized native execution slot"]
fn native_composition_raw_pilot_source_free_every_fuel() {
    let (sources, mut raw) = crate::frontend::oir::owned::composition_reference_tests::batch();
    append_cycle(&mut raw);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let entry = hir::DefId(0);
    assert_eq!(execute::run(&witness, Some(entry)), Ok(Scalar::I32(324)));
    let module = native_module(&witness, Some(entry), &sources).unwrap();
    let scratch = Scratch::new();
    let binary = scratch.compile(
        &argv_fuel_harness(&module, plan::MAX_FUEL),
        "composition-raw-fuel",
    );
    let mut successful = false;
    for fuel in 0..10_000 {
        let expected = execute::run_limits(
            &witness,
            Some(entry),
            execute::Limits {
                fuel,
                ..execute::Limits::default()
            },
        );
        match expected {
            Ok(value) => {
                // Frozen independently in the raw reference fixture schedule.
                assert_eq!(fuel, 269);
                assert_eq!(value, Scalar::I32(324));
                assert_result(scratch.run(&binary, &[fuel.to_string()]), b"324\n", b"", 0);
                let production = native_module_with_fuel(&witness, entry, &sources, fuel).unwrap();
                let binary = scratch.compile(&production, "composition-raw-exact-fuel");
                assert_result(scratch.run(&binary, &[]), b"324\n", b"", 0);
                successful = true;
                break;
            }
            Err(error) => {
                let stderr = error.diagnostic(&sources).render_human(&sources);
                assert_result(
                    scratch.run(&binary, &[fuel.to_string()]),
                    b"",
                    stderr.as_bytes(),
                    1,
                );
            }
        }
    }
    assert!(successful, "small raw composition fixture must terminate");
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7 and one serialized native execution slot"]
fn native_composition_projected_array_bounds_fuel_and_rhs_precedence() {
    let scratch = Scratch::new();
    let mut artifacts = 0;
    for (scalar, initializer, length) in [
        ("i32", "[17,29]", 2),
        ("bool", "[true,false]", 2),
        ("()", "[(),()]", 2),
        ("i32", "[]", 0),
        ("bool", "[]", 0),
        ("()", "[]", 0),
    ] {
        for index in [-1, length] {
            let text = format!("struct Inner{{xs:[{scalar};{length}]}}struct Outer{{pad:bool,inner:Inner}}fn read(p:&Outer)->{scalar}{{return p.inner.xs[{index}];}}fn main()->{scalar}{{let xs:[{scalar};{length}]={initializer};let x=Outer{{pad:true,inner:Inner{{xs:xs}}}};return read(&x);}}fn guard()->(){{while false{{}}return;}}");
            let (case, module) = checked_module(&text);
            let span = case.span(&format!("p.inner.xs[{index}]"));
            let expected = || execute::OwnedRunFailure::Bounds(span);
            assert_eq!(
                execute::run(&case.witness, Some(case.entry)),
                Err(expected())
            );
            let first_bounds = (0..2000)
                .find(|&fuel| {
                    execute::run_limits(
                        &case.witness,
                        Some(case.entry),
                        execute::Limits {
                            fuel,
                            ..execute::Limits::default()
                        },
                    ) == Err(expected())
                })
                .unwrap();
            let binary = scratch.compile(
                &argv_fuel_harness(&module, plan::MAX_FUEL),
                &format!("composition-bounds-{artifacts}"),
            );
            artifacts += 1;
            for fuel in [0, first_bounds - 1, first_bounds, first_bounds + 1] {
                let failure = execute::run_limits(
                    &case.witness,
                    Some(case.entry),
                    execute::Limits {
                        fuel,
                        ..execute::Limits::default()
                    },
                )
                .unwrap_err();
                if fuel == first_bounds - 1 {
                    assert_eq!(
                        failure,
                        execute::OwnedRunFailure::Scalar(RunFailure::Fuel(span))
                    );
                }
                let stderr = failure
                    .diagnostic(&case.sources)
                    .render_human(&case.sources);
                assert_result(
                    scratch.run(&binary, &[fuel.to_string()]),
                    b"",
                    stderr.as_bytes(),
                    1,
                );
            }
        }
    }
    // RHS arithmetic fails before evaluation of an invalid projected index.
    for index in [i32::MIN, i32::MAX] {
        let text = format!("struct I{{xs:[i32;1]}}struct O{{inner:I}}fn main()->i32{{let mut x=O{{inner:I{{xs:[7]}}}};x.inner.xs[{index}]=2147483647+1;return 0;}}");
        let (case, module) = checked_module(&text);
        let error = execute::run(&case.witness, Some(case.entry)).unwrap_err();
        assert!(matches!(
            error,
            execute::OwnedRunFailure::Scalar(RunFailure::Overflow(_))
        ));
        let stderr = error.diagnostic(&case.sources).render_human(&case.sources);
        let binary = scratch.compile(&module, &format!("composition-rhs-{artifacts}"));
        artifacts += 1;
        assert_result(scratch.run(&binary, &[]), b"", stderr.as_bytes(), 1);
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7 and one serialized native execution slot"]
fn native_composition_raw_empty_sentinels_source_free() {
    let scratch = Scratch::new();
    for guarded in [false, true] {
        let (sources, mut raw) =
            crate::frontend::oir::owned::composition_reference_tests::empty_composition();
        if guarded {
            append_cycle(&mut raw);
        }
        let witness = verified::verify_owned(raw, &sources).unwrap();
        let entry = hir::DefId(0);
        assert_eq!(execute::run(&witness, Some(entry)), Ok(Scalar::Unit));
        let module = native_module(&witness, Some(entry), &sources).unwrap();
        assert_safe_module(&module);
        let binary = scratch.compile(&module, &format!("composition-empty-{guarded}"));
        assert_result(scratch.run(&binary, &[]), b"()\n", b"", 0);
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7 and one serialized native execution slot"]
fn native_composition_source_free_depth_sixty_four() {
    let mut text = String::new();
    for depth in 0..64 {
        let child = if depth == 63 {
            "i32".into()
        } else {
            format!("R{}", depth + 1)
        };
        write!(text, "struct R{depth}{{value:{child}}}").unwrap();
    }
    text.push_str("fn relay(x:R0)->R0{return x;}fn main()->i32{");
    // Keep each expression inside the existing parser-depth and owner-slot
    // limits while exercising the full admitted containment/access depth.
    for start in [48, 32, 16, 0] {
        write!(text, "let v{start}=").unwrap();
        for depth in start..start + 16 {
            write!(text, "R{depth}{{value:").unwrap();
        }
        if start == 48 {
            text.push_str("37");
        } else {
            write!(text, "v{}", start + 16).unwrap();
        }
        text.push_str(&"}".repeat(16));
        text.push(';');
    }
    text.push_str("let x=relay(v0);return x");
    text.push_str(&".value".repeat(64));
    text.push_str(";}");
    let (case, module) = checked_module(&text);
    assert_eq!(
        execute::run(&case.witness, Some(case.entry)),
        Ok(Scalar::I32(37))
    );
    assert_safe_module(&module);
    let scratch = Scratch::new();
    let binary = scratch.compile(&module, "composition-depth64");
    assert_result(scratch.run(&binary, &[]), b"37\n", b"", 0);
}
