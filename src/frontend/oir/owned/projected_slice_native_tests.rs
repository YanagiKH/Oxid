//! Projected views use the ordinary verifier and native consumer. The expected
//! values are hand-derived; fuel sweeps compare consumers rather than claiming
//! an independent source-fuel oracle.
use super::*;
use crate::frontend::oir::owned::source::resource_fixtures as source;

const SUM: &str = "fn sum(xs:&[i32])->i32{let mut i=0;let mut total=0;while i<xs.len(){total=total+xs[i];i=i+1;}return total;}";
const BUMP: &str =
    "fn bump(xs:&mut[i32])->(){let mut i=0;while i<xs.len(){xs[i]=xs[i]+1;i=i+1;}return;}";
const PILOT: &str = "struct Meta{completed:i32}struct Batch{meta:Meta,samples:[i32;3],tail:bool}fn main()->i32{let mut batch=Batch{meta:Meta{completed:73},samples:[1,2,3],tail:true};bump(&mut batch.samples);if batch.meta.completed==73&&batch.tail&&batch.samples[0]==2&&batch.samples[1]==3&&batch.samples[2]==4{return sum(&batch.samples);}else{return -99;}}";
const FORWARD: &str = "struct Inner{tag:i32,samples:[i32;3]}struct Outer{prefix:bool,inner:Inner,suffix:i32}fn forward(xs:&mut[i32])->i32{bump(&mut *xs);return sum(&*xs);}fn edit(p:&mut Outer)->i32{return forward(&mut *p.inner.samples);}fn read(p:&Outer)->i32{return sum(&*p.inner.samples);}fn main()->i32{let mut x=Outer{prefix:true,inner:Inner{tag:41,samples:[1,2,3]},suffix:59};let a=edit(&mut x);let b=read(&x);if x.prefix&&x.inner.tag==41&&x.suffix==59{return a+b;}else{return -99;}}";
const MIXED: &str = "struct Inner{flags:[bool;2],units:[();3],empty:[i32;0]}struct Outer{before:i32,inner:Inner,after:i32}fn flip(xs:&mut[bool])->(){xs[0]=!xs[0];return;}fn units(xs:&mut[()])->i32{xs[2]=();xs[2];return xs.len();}fn empty(xs:&mut[i32])->i32{return xs.len();}fn main()->i32{let empty_values:[i32;0]=[];let mut x=Outer{before:17,inner:Inner{flags:[false,true],units:[(),(),()],empty:empty_values},after:29};flip(&mut x.inner.flags);let n=units(&mut x.inner.units);let z=empty(&mut x.inner.empty);if x.before==17&&x.after==29&&x.inner.flags[0]&&x.inner.flags[1]{return n+z+sum(&x.inner.empty);}else{return -99;}}";
const STAGING: &str = "struct Batch{tag:i32,samples:[i32;3]}fn join(xs:&[i32],n:i32)->i32{return xs[0]+xs.len()+n;}fn later(xs:&mut[i32])->i32{xs[0]=xs[0]+1;return xs[0];}fn main()->i32{let b=Batch{tag:73,samples:[8,13,21]};let mut counter=[0];return join(&b.samples,later(&mut counter));}";

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
    for (position, _) in module.match_indices("_index64 = zext i32") {
        let prior = &module[..position];
        let success = prior.rfind("_bounds_ok:\n").unwrap();
        let branch = prior[..success].rfind("_in_range, label").unwrap();
        let signed = prior[..branch]
            .rfind("_nonnegative = icmp sge i32")
            .unwrap();
        assert!(signed < branch && branch < success);
    }
}

#[test]
fn native_projected_slices_stage_checked_field_address_and_actual_length() {
    let (case, module) = checked_module(&format!("{SUM}{BUMP}{PILOT}"));
    assert_eq!(
        execute::run(&case.witness, Some(case.entry)),
        Ok(Scalar::I32(9))
    );
    assert_safe_module(&module);
    let mut projected = 0;
    for f in case.witness.functions() {
        for (b, block) in f.blocks.iter().enumerate() {
            for (i, statement) in block.statements.iter().enumerate() {
                let OwnedInstruction::PrepareBorrow { loan, .. } = statement.kind else {
                    continue;
                };
                let declaration = &f.loans[loan.0];
                if declaration.projection.is_empty() {
                    continue;
                }
                projected += 1;
                let name = format!("f{}_b{b}_i{i}", f.id.0);
                let pointer = format!("%{name}_borrow_projection_ptr");
                // Meta's i32 occupies the first four bytes. The slice must not
                // observe or mutate that whole-root address.
                assert!(module.contains(&format!("{pointer} = getelementptr i8, ptr %o")));
                let pointer_line = module
                    .lines()
                    .find(|line| line.contains(&format!("{pointer} =")))
                    .unwrap();
                assert!(pointer_line.ends_with(", i64 4"), "{pointer_line}");
                assert!(module.contains(&format!(
                    "store ptr {pointer}, ptr %r{}, align 8",
                    f.references.len() + loan.0
                )));
                assert!(module.contains(&format!("store i32 3, ptr %ll{}, align 4", loan.0)));
            }
        }
    }
    assert_eq!(projected, 2);
}

#[test]
fn native_projected_slices_stage_before_later_argument_and_forward_views() {
    let (case, module) = checked_module(STAGING);
    assert_eq!(
        execute::run(&case.witness, Some(case.entry)),
        Ok(Scalar::I32(12))
    );
    let pointer = module
        .find("_borrow_projection_ptr = getelementptr")
        .unwrap();
    let staged_pointer = module[pointer..].find("store ptr ").unwrap() + pointer;
    let staged_length = module[staged_pointer..]
        .find("store i32 3, ptr %ll")
        .unwrap()
        + staged_pointer;
    let later_call = module.rfind("call i32 @__oxid_owned_fn_1(").unwrap();
    assert!(
        pointer < staged_pointer && staged_pointer < staged_length && staged_length < later_call
    );
    let (case, module) = checked_module(&format!("{SUM}{BUMP}{FORWARD}"));
    assert_eq!(
        execute::run(&case.witness, Some(case.entry)),
        Ok(Scalar::I32(18))
    );
    assert!(module.contains("_length = load i32, ptr %rl0, align 4"));
    assert!(module.contains("_arg0_length = load i32, ptr %ll"));
    assert!(module.contains("_borrow_projection_ptr = getelementptr i8, ptr %"));
    assert_safe_module(&module);
    let (case, module) = checked_module(&format!("{SUM}{MIXED}"));
    assert_eq!(
        execute::run(&case.witness, Some(case.entry)),
        Ok(Scalar::I32(3))
    );
    for length in [0, 2, 3] {
        assert!(module.contains(&format!("store i32 {length}, ptr %ll")));
    }
    assert_safe_module(&module);
}

#[test]
fn native_projected_slices_account_bounded_path_walks_and_exact_output_cap() {
    let case = source::checked_arrays(&format!("{SUM}{BUMP}{FORWARD}"));
    let observation = run_array_observed(
        &case.witness,
        Some(case.entry),
        &case.sources,
        NativeControl::default(),
    );
    let module = observation.result.unwrap();
    let m = observation.metrics;
    let paths: usize = case
        .witness
        .functions()
        .iter()
        .flat_map(|f| {
            f.blocks.iter().flat_map(move |b| {
                b.statements.iter().map(move |s| match s.kind {
                    OwnedInstruction::PrepareBorrow { loan, .. } => {
                        f.loans[loan.0].projection.len()
                    }
                    _ => 0,
                })
            })
        })
        .sum();
    assert_eq!(paths, 4);
    assert_eq!(m.count_borrow_projection_visits, paths);
    assert_eq!(m.render_borrow_projection_visits, paths);
    assert_eq!(m.count_bytes, module.len());
    assert_eq!(m.render_bytes, module.len());
    assert_eq!(m.count_ordinary_visits, m.render_ordinary_visits);
    assert_eq!(m.count_expansions, m.render_expansions);
    assert_eq!(m.count_expansions, m.transfer_cells + m.message_bytes);
    assert_eq!(m.count_call_scratch_peak, m.render_call_scratch_peak);
    assert!(m.metadata_peak <= Limits::DEFAULT.metadata_bytes);
    assert!(m.render_call_scratch_peak < EMITTER_TRANSIENT_BYTES);
    for (cap, accepted) in [(module.len(), true), (module.len() - 1, false)] {
        let observation = run_array_observed(
            &case.witness,
            Some(case.entry),
            &case.sources,
            NativeControl {
                limits: Limits {
                    ir_bytes: cap,
                    ..Limits::DEFAULT
                },
                ..NativeControl::default()
            },
        );
        if accepted {
            assert_eq!(observation.result.unwrap(), module);
        } else {
            assert!(observation
                .result
                .unwrap_err()
                .message
                .contains("LLVM bytes"));
            assert_eq!(observation.metrics.render_bytes, 0);
            assert_eq!(observation.metrics.render_borrow_projection_visits, 0);
        }
    }
    for fail_after in 0..m.allocation_attempts {
        let failed = run_array_observed(
            &case.witness,
            Some(case.entry),
            &case.sources,
            NativeControl {
                fail_after: Some(fail_after),
                ..NativeControl::default()
            },
        );
        assert!(failed.result.unwrap_err().message.contains("allocation"));
        assert_eq!(failed.metrics.render_bytes, 0);
    }
}

fn assert_paid_slice_writes(case: &source::CheckedSource, module: &str) {
    let mut writes = 0;
    for f in case.witness.functions() {
        let cfg = LlvmCfg::parse(module, &format!("__oxid_owned_fn_{}", f.id.0));
        for (b, block) in f.blocks.iter().enumerate() {
            for (i, statement) in block.statements.iter().enumerate() {
                if let OwnedInstruction::WriteIndex { value, .. } = statement.kind {
                    writes += 1;
                    let name = format!("f{}_b{b}_i{i}", f.id.0);
                    let store = format!(
                        "store {} %{name}_value, ptr %{name}_ptr, align 1",
                        ty(f.locals[value.local.0].ty)
                    );
                    let guard = format!("f{}_b{b}_g{}", f.id.0, i + 1);
                    cfg.assert_success_dominates(
                        &format!("{guard}_ok"),
                        &format!("{guard}_error"),
                        &store,
                    );
                    cfg.assert_success_dominates(
                        &format!("{name}_bounds_ok"),
                        &format!("{name}_bounds_error"),
                        &store,
                    );
                }
            }
        }
    }
    assert!(writes > 0);
}

#[test]
fn native_projected_slices_no_unpaid_or_out_of_bounds_write() {
    let (case, module) = checked_module(&format!("{SUM}{BUMP}{PILOT}"));
    assert_paid_slice_writes(&case, &module);
}

fn depth_source() -> String {
    let mut text = format!("{SUM}{BUMP}");
    for depth in 0..64 {
        let child = if depth == 63 {
            "[i32;3]".into()
        } else {
            format!("R{}", depth + 1)
        };
        write!(text, "struct R{depth}{{value:{child}}}").unwrap();
    }
    let path = ".value".repeat(64);
    write!(
        text,
        "fn project(p:&mut R0)->i32{{bump(&mut *p{path});return sum(&*p{path});}}fn main()->i32{{"
    )
    .unwrap();
    // Segment construction to respect the existing parser-depth ceiling.
    for start in [48, 32, 16, 0] {
        write!(text, "let v{start}=").unwrap();
        for depth in start..start + 16 {
            write!(text, "R{depth}{{value:").unwrap();
        }
        if start == 48 {
            text.push_str("[1,2,3]");
        } else {
            write!(text, "v{}", start + 16).unwrap();
        }
        text.push_str(&"}".repeat(16));
        text.push(';');
    }
    text.push_str("let mut x=v0;return project(&mut x);}");
    text
}

#[test]
fn native_projected_slices_depth_sixty_four_has_bounded_walks() {
    let case = source::checked_arrays(&depth_source());
    assert_eq!(
        execute::run(&case.witness, Some(case.entry)),
        Ok(Scalar::I32(9))
    );
    let observation = run_array_observed(
        &case.witness,
        Some(case.entry),
        &case.sources,
        NativeControl::default(),
    );
    let module = observation.result.unwrap();
    assert_safe_module(&module);
    assert_eq!(observation.metrics.count_borrow_projection_visits, 128);
    assert_eq!(observation.metrics.render_borrow_projection_visits, 128);
    assert_eq!(
        observation.metrics.emitter_transient_bound,
        EMITTER_TRANSIENT_BYTES
    );
    assert_eq!(observation.metrics.count_bytes, module.len());
    assert_eq!(observation.metrics.render_bytes, module.len());
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7 and one serialized native execution slot"]
fn native_projected_slices_source_free_mutation_metadata_and_forwarding() {
    let scratch = Scratch::new();
    for (name, text, expected) in [
        ("projected-slice-sum9", format!("{SUM}{BUMP}{PILOT}"), 9),
        (
            "projected-slice-forward",
            format!("{SUM}{BUMP}{FORWARD}"),
            18,
        ),
        ("projected-slice-mixed", format!("{SUM}{MIXED}"), 3),
        ("projected-slice-staging", STAGING.into(), 12),
        ("projected-slice-depth64", depth_source(), 9),
    ] {
        let (case, module) = checked_module(&text);
        assert_eq!(
            execute::run(&case.witness, Some(case.entry)),
            Ok(Scalar::I32(expected))
        );
        assert_safe_module(&module);
        let binary = scratch.compile(&module, name);
        assert_result(
            scratch.run(&binary, &[]),
            format!("{expected}\n").as_bytes(),
            b"",
            0,
        );
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7 and one serialized native execution slot"]
fn native_projected_slices_source_free_mutation_every_fuel() {
    let (case, module) = checked_module(&format!("{SUM}{BUMP}{PILOT}"));
    assert_paid_slice_writes(&case, &module);
    let scratch = Scratch::new();
    let binary = scratch.compile(
        &argv_fuel_harness(&module, plan::MAX_FUEL),
        "projected-slice-fuel",
    );
    let mut successful = false;
    for fuel in 0..2000 {
        match execute::run_limits(
            &case.witness,
            Some(case.entry),
            execute::Limits {
                fuel,
                ..execute::Limits::default()
            },
        ) {
            Ok(value) => {
                assert_eq!(value, Scalar::I32(9));
                assert_result(scratch.run(&binary, &[fuel.to_string()]), b"9\n", b"", 0);
                let production =
                    native_module_with_fuel(&case.witness, case.entry, &case.sources, fuel)
                        .unwrap();
                let binary = scratch.compile(&production, "projected-slice-exact-fuel");
                assert_result(scratch.run(&binary, &[]), b"9\n", b"", 0);
                successful = true;
                break;
            }
            Err(error) => {
                let stderr = error.diagnostic(&case.sources).render_human(&case.sources);
                assert_result(
                    scratch.run(&binary, &[fuel.to_string()]),
                    b"",
                    stderr.as_bytes(),
                    1,
                );
            }
        }
    }
    assert!(successful, "small projected slice fixture terminates");
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7 and one serialized native execution slot"]
fn native_projected_slices_source_free_signed_bounds_and_fuel() {
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
        for index in [i32::MIN, -1, length, i32::MAX] {
            let text = format!("struct Inner{{xs:[{scalar};{length}]}}struct Outer{{pad:i32,inner:Inner,tail:i32}}fn read(xs:&[{scalar}])->{scalar}{{return xs[{index}];}}fn main()->{scalar}{{let xs:[{scalar};{length}]={initializer};let x=Outer{{pad:41,inner:Inner{{xs:xs}},tail:59}};return read(&x.inner.xs);}}fn guard()->(){{while false{{}}return;}}");
            let (case, module) = checked_module(&text);
            let span = case.span(&format!("xs[{index}]"));
            let bounds = || execute::OwnedRunFailure::Bounds(span);
            assert_eq!(execute::run(&case.witness, Some(case.entry)), Err(bounds()));
            let first_bounds = (1..2000)
                .find(|&fuel| {
                    execute::run_limits(
                        &case.witness,
                        Some(case.entry),
                        execute::Limits {
                            fuel,
                            ..execute::Limits::default()
                        },
                    ) == Err(bounds())
                })
                .unwrap();
            let binary = scratch.compile(
                &argv_fuel_harness(&module, plan::MAX_FUEL),
                &format!("projected-slice-bounds-{artifacts}"),
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
}
