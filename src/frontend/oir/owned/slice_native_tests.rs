//! Borrowed slices exercise the production source/verifier/native path. Numeric
//! outcomes below are hand-calculated; fuel sweeps are consumer differential
//! checks and do not claim an independent source-fuel oracle.
use super::*;
use crate::frontend::oir::owned::source::resource_fixtures as source;

const SUM: &str = "fn sum(xs:&[i32])->i32{let mut i=0;let mut total=0;while i<xs.len(){total=total+xs[i];i=i+1;}return total;}";
const SHARED_MAIN: &str = "fn main()->i32{let a=[1,2];let b=[3,4,5];let empty:[i32;0]=[];return sum(&a)*100+sum(&b)+sum(&empty);}";
const MUTATION: &str = "fn bump(xs:&mut[i32])->(){let mut i=0;while i<xs.len(){xs[i]=xs[i]+1;i=i+1;}return;} fn relay(xs:&mut[i32])->i32{bump(&mut *xs);return sum(&*xs);} fn main()->i32{let mut a=[1,2];let mut b=[3,4,5];let empty:[i32;0]=[];let first=relay(&mut a);let second=relay(&mut b);return first*100+second+sum(&empty);}";

fn checked_module(text: &str) -> (source::CheckedSource, String) {
    let case = source::checked_arrays(text);
    let module = native_module(&case.witness, Some(case.entry), &case.sources).unwrap();
    (case, module)
}

fn assert_safe_index_emission(module: &str) {
    for forbidden in [
        "inbounds",
        "noalias",
        "nonnull",
        "dereferenceable",
        "undef",
        "poison",
    ] {
        assert!(!module.contains(forbidden), "{forbidden}");
    }
    for (prefix, _) in module.match_indices("_index64 = zext i32") {
        let prior = &module[..prefix];
        let bounds = prior.rfind("_bounds_ok:\n").expect("bounds success block");
        let branch = prior[..bounds]
            .rfind("_in_range, label")
            .expect("bounds branch");
        let signed = prior[..branch]
            .rfind("_nonnegative = icmp sge i32")
            .expect("signed bound");
        assert!(signed < branch && branch < bounds);
    }
}

#[test]
fn native_slices_shared_sum_has_one_dynamic_body_for_lengths_two_three_zero() {
    let (case, module) = checked_module(&format!("{SUM}{SHARED_MAIN}"));
    assert_eq!(
        execute::run(&case.witness, Some(case.entry)),
        Ok(Scalar::I32(312))
    );
    assert_eq!(
        module
            .matches("define internal i32 @__oxid_owned_fn_0(")
            .count(),
        1
    );
    assert!(module.contains("ptr %arg0, i32 %arg0_length"));
    assert!(module.contains("= load i32, ptr %rl0, align 4"));
    for length in [0, 2, 3] {
        assert!(module.contains(&format!("store i32 {length}, ptr %ll")));
    }
    assert!(module.contains("_below = icmp slt i32"));
    assert_safe_index_emission(&module);
}

#[test]
fn native_slices_reborrow_forwards_dynamic_lengths_and_keeps_exact_abi() {
    let (case, module) = checked_module(&format!("{SUM}{MUTATION}"));
    assert_eq!(
        execute::run(&case.witness, Some(case.entry)),
        Ok(Scalar::I32(515))
    );
    assert!(module.contains("_length = load i32, ptr %rl0, align 4"));
    assert!(module.contains("_arg0_length = load i32, ptr %ll"));
    assert_safe_index_emission(&module);
    let (_, exact) = checked_module(
        "fn read(p:&[i32;2])->i32{return p[1];}fn main()->i32{let a=[3,7];return read(&a);}",
    );
    assert!(exact.contains("define internal i32 @__oxid_owned_fn_0(ptr %arg0)"));
    assert!(!exact.contains("slice_lengths"));
    assert!(!exact.contains("arg0_length"));
    assert_safe_index_emission(&exact);
}

#[test]
fn native_slices_length_sidecars_are_the_only_added_frame_storage() {
    let exact = source::checked_arrays(
        "fn read(p:&[i32;2])->i32{return p.len();}fn main()->i32{let a=[3,7];return read(&a);}",
    );
    let slice = source::checked_arrays(
        "fn read(p:&[i32])->i32{return p.len();}fn main()->i32{let a=[3,7];return read(&a);}",
    );
    let exact_plan = ExecutionPlan::build(&exact.witness).unwrap();
    let slice_plan = ExecutionPlan::build(&slice.witness).unwrap();
    assert_eq!(exact_plan.metadata_bytes(), slice_plan.metadata_bytes());
    for (before, after) in exact_plan.functions().iter().zip(slice_plan.functions()) {
        let mut expected = before.usage();
        expected.native_bytes += 4;
        assert_eq!(after.usage(), expected);
    }
    let bytes = slice_plan
        .functions()
        .iter()
        .map(|f| f.usage().native_bytes)
        .sum();
    assert!(admit(
        &slice_plan,
        Limits {
            bytes,
            ..Limits::DEFAULT
        }
    )
    .is_ok());
    assert!(admit(
        &slice_plan,
        Limits {
            bytes: bytes - 1,
            ..Limits::DEFAULT
        }
    )
    .unwrap_err()
    .message
    .contains("aggregate storage bytes"));
    let observation = run_array_observed(
        &slice.witness,
        Some(slice.entry),
        &slice.sources,
        NativeControl::default(),
    );
    let module = observation.result.unwrap();
    assert_eq!(observation.metrics.count_bytes, module.len());
    assert_eq!(observation.metrics.render_bytes, module.len());
    assert_eq!(
        observation.metrics.count_ordinary_visits,
        observation.metrics.render_ordinary_visits
    );
    assert_eq!(
        observation.metrics.count_call_scratch_peak,
        observation.metrics.render_call_scratch_peak
    );
    assert!(observation.metrics.render_call_scratch_peak < EMITTER_TRANSIENT_BYTES);
    assert!(native_module_limits(
        &slice.witness,
        Some(slice.entry),
        &slice.sources,
        plan::MAX_FUEL,
        Limits {
            ir_bytes: module.len(),
            ..Limits::DEFAULT
        }
    )
    .is_ok());
    assert!(native_module_limits(
        &slice.witness,
        Some(slice.entry),
        &slice.sources,
        plan::MAX_FUEL,
        Limits {
            ir_bytes: module.len() - 1,
            ..Limits::DEFAULT
        }
    )
    .unwrap_err()
    .message
    .contains("LLVM bytes"));
}

#[test]
fn native_slices_keep_sixty_four_source_parameters_with_bounded_flattening() {
    for count in [64, 65] {
        let parameters = (0..count)
            .map(|i| format!("p{i}:&[i32]"))
            .collect::<Vec<_>>()
            .join(",");
        let arguments = vec!["&a"; count].join(",");
        let case = source::checked_arrays(&format!("fn read({parameters})->i32{{return p{}.len();}}fn main()->i32{{let a=[4];return read({arguments});}}", count-1));
        let observation = run_array_observed(
            &case.witness,
            Some(case.entry),
            &case.sources,
            NativeControl::default(),
        );
        if count == 64 {
            let module = observation.result.unwrap();
            assert!(module.contains("ptr %arg63, i32 %arg63_length"));
            assert!(observation.metrics.render_call_scratch_peak < EMITTER_TRANSIENT_BYTES);
            assert_eq!(
                observation.metrics.count_call_scratch_peak,
                observation.metrics.render_call_scratch_peak
            );
        } else {
            assert!(observation
                .result
                .unwrap_err()
                .message
                .contains("parameter count"));
            assert_eq!(observation.metrics.count_bytes, 0);
        }
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7 and one serialized native execution slot"]
fn native_slices_checkpoint_and_mutation_use_source_free_llvm() {
    let scratch = Scratch::new();
    for (name, text, expected) in [
        ("slice-shared-312", format!("{SUM}{SHARED_MAIN}"), 312),
        ("slice-mutate-515", format!("{SUM}{MUTATION}"), 515),
        ("slice-fixed-forward", format!("{SUM}fn relay(p:&[i32;2])->i32{{return sum(&*p);}}fn main()->i32{{let a=[8,13];return relay(&a);}}"), 21),
        ("slice-rhs-snapshot", "fn index(p:&mut[i32])->i32{p[0]=9;return 0;}fn edit(p:&mut[i32])->i32{p[index(&mut *p)]=p[0];return p[0];}fn main()->i32{let mut a=[1,2];return edit(&mut a);}".into(), 1),
        ("slice-bool-unit", "fn flip(p:&mut[bool])->(){p[0]=!p[0];return;}fn unit(p:&mut[()])->i32{p[0]=();p[0];return p.len();}fn main()->i32{let mut a=[false,true];let mut b=[(),(),()];flip(&mut a);if a[0]&&a[1]{return unit(&mut b);}else{return 99;}}".into(), 3),
    ] {
        let (case, module) = checked_module(&text);
        assert_eq!(execute::run(&case.witness, Some(case.entry)), Ok(Scalar::I32(expected)));
        let binary = scratch.compile(&module, name);
        assert_result(scratch.run(&binary, &[]), format!("{expected}\n").as_bytes(), b"", 0);
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7 and one serialized native execution slot"]
fn native_slices_signed_bounds_and_fuel_use_source_free_llvm() {
    let scratch = Scratch::new();
    for (case_index, (ty, initializer, result_ty, n)) in [
        ("i32", "[17,29]", "i32", 2),
        ("bool", "[true,false]", "bool", 2),
        ("()", "[(),()]", "()", 2),
        ("i32", "[]", "i32", 0),
        ("bool", "[]", "bool", 0),
        ("()", "[]", "()", 0),
    ]
    .into_iter()
    .enumerate()
    {
        for index in [i32::MIN, -1, n, i32::MAX] {
            let text = format!("fn read(p:&[{ty}])->{result_ty}{{return p[{index}];}}fn main()->{result_ty}{{let a:[{ty};{n}]={initializer};return read(&a);}}fn guard()->(){{while false{{}}return;}}");
            let (case, module) = checked_module(&text);
            let span = case.span(&format!("p[{index}]"));
            let bounds = || execute::OwnedRunFailure::Bounds(span);
            assert_eq!(execute::run(&case.witness, Some(case.entry)), Err(bounds()));
            let harness = argv_fuel_harness(&module, plan::MAX_FUEL);
            let binary = scratch.compile(&harness, &format!("slice-bounds-{case_index}-{index}"));
            let first_bounds = (0..1000)
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
                .expect("small fixture reaches bounds");
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

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7 and one serialized native execution slot"]
fn native_slices_acyclic_phi_and_rhs_failure_use_source_free_llvm() {
    let scratch = Scratch::new();
    for prefix in ["", "while false {}"] {
        let (case, module) = checked_module(&format!("fn test(p:&[i32])->bool{{return p[0]/2==4&&p[1]%3==1;}}fn main()->bool{{{prefix}let a=[8,7];return test(&a);}}"));
        assert_eq!(
            execute::run(&case.witness, Some(case.entry)),
            Ok(Scalar::Bool(true))
        );
        let binary = scratch.compile(&module, &format!("slice-phi-{}", prefix.len()));
        assert_result(scratch.run(&binary, &[]), b"true\n", b"", 0);
    }
    let (case, module) = checked_module("fn edit(p:&mut[i32])->(){p[1/0]=7%0;return;}fn main()->(){let mut a=[1];edit(&mut a);return;}");
    let failure = execute::run(&case.witness, Some(case.entry)).unwrap_err();
    assert_eq!(
        failure,
        execute::OwnedRunFailure::Scalar(RunFailure::DivisionByZero(case.span("%")))
    );
    let stderr = failure
        .diagnostic(&case.sources)
        .render_human(&case.sources);
    let binary = scratch.compile(&module, "slice-rhs-failure");
    assert_result(scratch.run(&binary, &[]), b"", stderr.as_bytes(), 1);
}
