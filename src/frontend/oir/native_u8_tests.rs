//! RFC0030 scalar native semantics and independent type admission controls.
use super::tests::verified_with_sources;
use super::*;

const ROUNDTRIP: &str = "fn main()->i32{let x=255;let b=x.to_u8_checked();return b.to_i32();}";

#[test]
fn u8_native_admission_independently_checks_every_conversion_pair() {
    let (program, _) = verified_with_sources("fn main()->i32{return 0;}");
    let mut function = program.program.functions[0].clone();
    let span = function.span;
    function.locals.push(function.locals[0].clone());
    for narrowing in [true, false] {
        for input in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::U8, hir::Ty::Unit] {
            for output in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::U8, hir::Ty::Unit] {
                function.locals[0].ty = input;
                function.locals[1].ty = output;
                let operand = Operand {
                    local: LocalId(0),
                    span,
                };
                let assignment = Assign {
                    destination: LocalId(1),
                    value: if narrowing {
                        Rvalue::CheckedI32ToU8 {
                            operand,
                            name_span: span,
                            source_expr: crate::frontend::ast::ExprId(0),
                        }
                    } else {
                        Rvalue::U8ToI32 {
                            operand,
                            name_span: span,
                            source_expr: crate::frontend::ast::ExprId(0),
                        }
                    },
                    span,
                };
                let result = admit_assignment(&function, &assignment);
                let expected = if narrowing {
                    (hir::Ty::I32, hir::Ty::U8)
                } else {
                    (hir::Ty::U8, hir::Ty::I32)
                };
                assert_eq!(result.is_ok(), (input, output) == expected);
                if let Err(error) = result {
                    assert_eq!((error.code, error.stage), ("E0500", "native-admission"));
                    assert_eq!(error.primary, None);
                }
            }
        }
    }
    function.locals[1].ty = hir::Ty::U8;
    for value in [
        Rvalue::I32(0),
        Rvalue::I32(255),
        Rvalue::CheckedI32ToU8 {
            operand: Operand {
                local: LocalId(usize::MAX),
                span,
            },
            name_span: span,
            source_expr: crate::frontend::ast::ExprId(0),
        },
    ] {
        assert!(admit_assignment(
            &function,
            &Assign {
                destination: LocalId(1),
                value,
                span,
            }
        )
        .is_err());
    }
}

#[test]
fn u8_native_admission_rejects_mixed_comparisons_and_byte_arithmetic() {
    let (program, _) = verified_with_sources("fn main()->i32{return 0;}");
    let mut function = program.program.functions[0].clone();
    let span = function.span;
    function.locals.resize(3, function.locals[0].clone());
    function.locals[2].ty = hir::Ty::Bool;
    let left = Operand {
        local: LocalId(0),
        span,
    };
    let right = Operand {
        local: LocalId(1),
        span,
    };
    for op in [
        hir::ComparisonOp::Equal,
        hir::ComparisonOp::NotEqual,
        hir::ComparisonOp::Less,
        hir::ComparisonOp::LessEqual,
        hir::ComparisonOp::Greater,
        hir::ComparisonOp::GreaterEqual,
    ] {
        for left_ty in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::U8, hir::Ty::Unit] {
            for right_ty in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::U8, hir::Ty::Unit] {
                function.locals[0].ty = left_ty;
                function.locals[1].ty = right_ty;
                let equality = matches!(op, hir::ComparisonOp::Equal | hir::ComparisonOp::NotEqual);
                let expected = left_ty == right_ty
                    && (matches!(left_ty, hir::Ty::I32 | hir::Ty::U8)
                        || (equality && left_ty == hir::Ty::Bool));
                let result = admit_assignment(
                    &function,
                    &Assign {
                        destination: LocalId(2),
                        value: Rvalue::CompareScalar {
                            op,
                            left,
                            right,
                            operator_span: span,
                        },
                        span,
                    },
                );
                assert_eq!(result.is_ok(), expected, "{op:?} {left_ty:?} {right_ty:?}");
            }
        }
    }
    function.locals[0].ty = hir::Ty::U8;
    function.locals[1].ty = hir::Ty::U8;
    for op in [
        hir::ArithmeticOp::Add,
        hir::ArithmeticOp::Subtract,
        hir::ArithmeticOp::Multiply,
        hir::ArithmeticOp::Divide,
        hir::ArithmeticOp::Remainder,
    ] {
        assert!(admit_assignment(
            &function,
            &Assign {
                destination: LocalId(0),
                value: Rvalue::CheckedI32 {
                    op,
                    left,
                    right,
                    operator_span: span
                },
                span,
            }
        )
        .is_err());
    }
    for value in [
        Rvalue::CheckedNegateI32 {
            operand: left,
            operator_span: span,
        },
        Rvalue::NotBool {
            operand: left,
            operator_span: span,
        },
    ] {
        assert!(admit_assignment(
            &function,
            &Assign {
                destination: LocalId(0),
                value,
                span,
            }
        )
        .is_err());
    }
}

#[test]
fn u8_native_helpers_use_i8_and_all_orderings_are_unsigned() {
    for (op, predicate, expected) in [
        ("==", "eq", false),
        ("!=", "ne", true),
        ("<", "ult", true),
        ("<=", "ule", true),
        (">", "ugt", false),
        (">=", "uge", false),
    ] {
        let text = format!("fn cmp(a:u8,b:u8)->bool{{return a{op}b;}}fn pass(b:u8)->u8{{return b;}}fn main()->bool{{let x=127;let y=128;let a=x.to_u8_checked();let b=y.to_u8_checked();return cmp(pass(a),b);}}");
        let (program, sources) = verified_with_sources(&text);
        let ir = program
            .native_module(Some(hir::DefId(2)), &sources)
            .unwrap();
        assert!(ir.contains("define internal i1 @__oxid_fn_0(i8 %v0, i8 %v1)"));
        assert!(ir.contains("define internal i8 @__oxid_fn_1(i8 %v0)"));
        assert_eq!(ir.matches(&format!(" = icmp {predicate} i8 ")).count(), 1);
        assert_eq!(program.run(Some(hir::DefId(2))), Ok(Scalar::Bool(expected)));
    }
}

#[test]
fn u8_native_checks_full_i32_before_trunc_and_widens_with_zero_extension() {
    for value in [-2_147_483_648_i32, -1, 0, 127, 128, 255, 256, 2_147_483_647] {
        let text = ROUNDTRIP.replace("255", &value.to_string());
        let (program, sources) = verified_with_sources(&text);
        let ir = program
            .native_module(Some(hir::DefId(0)), &sources)
            .unwrap();
        let function = &program.program.functions[0];
        let narrow = function
            .blocks
            .iter()
            .flat_map(|b| &b.statements)
            .filter_map(Statement::as_assignment)
            .find(|a| matches!(a.value, Rvalue::CheckedI32ToU8 { .. }))
            .unwrap();
        let Rvalue::CheckedI32ToU8 {
            operand, name_span, ..
        } = narrow.value
        else {
            unreachable!()
        };
        let n = narrow.destination.0;
        let low = ir
            .find(&format!(
                "%byte{n}_negative = icmp slt i32 %v{}, 0",
                operand.local.0
            ))
            .unwrap();
        let high = ir
            .find(&format!(
                "%byte{n}_large = icmp sgt i32 %v{}, 255",
                operand.local.0
            ))
            .unwrap();
        let branch = ir
            .find(&format!(
                "br i1 %byte{n}_invalid, label %byte{n}_error, label %checked{n}_ok"
            ))
            .unwrap();
        let trunc = ir
            .find(&format!(
                "checked{n}_ok:\n  %v{n} = trunc i32 %v{} to i8",
                operand.local.0
            ))
            .unwrap();
        assert!(low < high && high < branch && branch < trunc);
        assert_eq!(ir.matches(" = trunc i32 ").count(), 1);
        assert_eq!(ir.matches(" = zext i8 ").count(), 1);
        assert!(!ir.contains(" = sext i8 "));
        let message = RunFailure::ByteRange(name_span)
            .diagnostic(&sources)
            .render_human(&sources);
        let escaped: String = message
            .bytes()
            .map(|byte| format!("\\{byte:02X}"))
            .collect();
        assert!(ir.contains(&format!("[{} x i8] c\"{escaped}\"", message.len())));
        assert_eq!(&text[name_span.start..name_span.end], "to_u8_checked");
        if (0..=255).contains(&value) {
            assert_eq!(program.run(Some(hir::DefId(0))), Ok(Scalar::I32(value)));
        } else {
            assert_eq!(
                program.run(Some(hir::DefId(0))),
                Err(RunFailure::ByteRange(name_span))
            );
        }
    }
}

#[test]
fn u8_native_conversion_guards_precede_work_and_update_phi_exits() {
    for guarded in [false, true] {
        let prefix = if guarded { "while false{}" } else { "" };
        let text = format!("fn main()->bool{{{prefix}let x=128;let b=x.to_u8_checked();return x.to_u8_checked()==b&&true;}}");
        let (program, sources) = verified_with_sources(&text);
        let ir = program
            .native_module(Some(hir::DefId(0)), &sources)
            .unwrap();
        let function = &program.program.functions[0];
        for (block_index, block) in function.blocks.iter().enumerate() {
            for (index, statement) in block.statements.iter().enumerate() {
                if let Some(assignment) = statement.as_assignment() {
                    if matches!(
                        assignment.value,
                        Rvalue::CheckedI32ToU8 { .. } | Rvalue::U8ToI32 { .. }
                    ) && guarded
                    {
                        let guard = ir
                            .find(&format!("g{block_index}_{}_ok:", index + 1))
                            .unwrap();
                        let work = ir
                            .find(&format!("%byte{}_negative =", assignment.destination.0))
                            .unwrap();
                        assert!(guard < work);
                    }
                }
            }
            if let Some(merge) = &block.merge {
                for input in merge.incoming {
                    let predecessor = &function.blocks[input.predecessor.0];
                    let label = if guarded {
                        format!(
                            "g{}_{}_ok",
                            input.predecessor.0,
                            predecessor.statements.len() + 1
                        )
                    } else {
                        predecessor
                            .statements
                            .iter()
                            .filter_map(Statement::as_assignment)
                            .rfind(|a| checked_failures(&a.value).is_some())
                            .map_or_else(
                                || format!("b{}", input.predecessor.0),
                                |a| format!("checked{}_ok", a.destination.0),
                            )
                    };
                    assert!(ir.contains(&format!("[ %v{}, %{label} ]", input.value.local.0)));
                }
            }
        }
    }
}

#[test]
fn u8_native_metadata_and_text_have_exact_and_one_under_caps() {
    for (prefix, policy) in [
        ("while false{}", EntryPolicy::Result),
        ("", EntryPolicy::Process),
    ] {
        let text = ROUNDTRIP.replacen("{", &format!("{{{prefix}"), 1);
        let (program, sources) = verified_with_sources(&text);
        let entry = hir::DefId(0);
        let diagnostics = GuardedDiagnostics::new_policy(
            &program.program,
            entry,
            &sources,
            usize::MAX,
            policy == EntryPolicy::Result,
            policy,
        )
        .unwrap();
        let bytes: usize = diagnostics.messages.iter().map(String::len).sum();
        let byte_keys: Vec<_> = diagnostics
            .ids
            .keys()
            .filter(|key| key.0 == FailureKind::ByteRange)
            .collect();
        assert_eq!(byte_keys.len(), 1);
        let key = byte_keys[0];
        assert_eq!(&text[key.2..key.3], "to_u8_checked");
        let ir = program
            .native_module_policy_limits(
                Some(entry),
                &sources,
                execute::MAX_FUEL,
                bytes,
                usize::MAX,
                policy,
            )
            .unwrap();
        assert_eq!(
            program
                .native_module_policy_limits(
                    Some(entry),
                    &sources,
                    execute::MAX_FUEL,
                    bytes,
                    ir.len(),
                    policy,
                )
                .unwrap(),
            ir
        );
        for (diagnostic_cap, ir_cap, marker) in [
            (bytes - 1, ir.len(), "diagnostic bytes"),
            (bytes, ir.len() - 1, "LLVM bytes"),
        ] {
            let error = program
                .native_module_policy_limits(
                    Some(entry),
                    &sources,
                    execute::MAX_FUEL,
                    diagnostic_cap,
                    ir_cap,
                    policy,
                )
                .unwrap_err();
            assert_eq!((error.code, error.stage), ("E0700", "native-admission"));
            assert!(error.message.contains(marker));
        }
        println!(
            "U8_NATIVE_REPRESENTATION policy={policy:?} diagnostic_bytes={bytes} ir_bytes={}",
            ir.len()
        );
    }
}

#[test]
fn u8_native_selected_entry_is_rejected_but_main_can_be_a_byte_helper() {
    let (program, sources) = verified_with_sources(
        "fn main()->u8{let x=255;return x.to_u8_checked();}fn caller()->i32{let b=main();return b.to_i32();}",
    );
    let error = program
        .native_module(Some(hir::DefId(0)), &sources)
        .unwrap_err();
    assert_eq!((error.code, error.stage), ("E0700", "native-admission"));
    assert_eq!(error.primary, Some(program.program.functions[0].span));
    assert_eq!(error.message, "native main must return bool, i32 or ()");
    let process = program
        .native_process_module(Some(hir::DefId(0)), &sources)
        .unwrap_err();
    assert_eq!(process.message, "process main must return i32");
    let ir = program
        .native_module(Some(hir::DefId(1)), &sources)
        .unwrap();
    assert!(ir.contains("call i8 @__oxid_fn_0()"));
}

#[test]
fn u8_native_cost_counts_real_read_and_conversion_assignments_once() {
    let (program, _) = verified_with_sources(ROUNDTRIP);
    let function = &program.program.functions[0];
    assert_eq!(function.locals.len(), 7);
    assert!(function.places.is_empty());
    assert_eq!(function.blocks.len(), 1);
    let assignments: Vec<_> = function.blocks[0]
        .statements
        .iter()
        .map(Statement::assignment)
        .collect();
    assert_eq!(assignments.len(), 7);
    assert!(matches!(assignments[2].value, Rvalue::Copy(_)));
    assert!(matches!(
        assignments[3].value,
        Rvalue::CheckedI32ToU8 { .. }
    ));
    assert!(matches!(assignments[5].value, Rvalue::Copy(_)));
    assert!(matches!(assignments[6].value, Rvalue::U8ToI32 { .. }));
    let bound = program.admit().unwrap()[0];
    assert_eq!(bound.slots, 7);
    // Entry unit + slots + seven assignments + one return.
    assert_eq!(1 + bound.cost, 16);
}

#[test]
fn u8_native_mutable_storage_preserves_i8_snapshots() {
    let (program, sources) = verified_with_sources(
        "fn main()->i32{let x=127;let y=128;let mut b=x.to_u8_checked();let a=b;b=y.to_u8_checked();return a.to_i32();}",
    );
    let ir = program
        .native_module(Some(hir::DefId(0)), &sources)
        .unwrap();
    assert_eq!(ir.matches("alloca i8").count(), 1);
    assert_eq!(ir.matches("store i8 ").count(), 2);
    assert_eq!(ir.matches("load i8, ptr %p0").count(), 1);
    assert_eq!(program.run(Some(hir::DefId(0))), Ok(Scalar::I32(127)));
}

/// Explicit opt-in tool proof. Ordinary Rust tests do not require native tools;
/// this test preserves source, emitted IR, source-free ELF and process outputs
/// in the caller-selected evidence directory instead of deleting the receipts.
#[test]
#[ignore = "requires LLVM 19.1.7 on Linux x86_64 and OXID_U8_NATIVE_EVIDENCE"]
fn u8_scalar_native_pinned_boundary_and_fuel_proof() {
    use std::{fs, path::PathBuf, process::Command};
    let root =
        PathBuf::from(std::env::var_os("OXID_U8_NATIVE_EVIDENCE").expect("evidence directory"));
    fs::create_dir_all(&root).unwrap();
    let mut receipt = String::new();
    let mut observe = |label: &str,
                       text: &str,
                       fuel: Option<usize>,
                       process: bool,
                       expected_stdout: &str,
                       expected_status: i32,
                       failure: Option<(&'static str, &str, &str)>| {
        let directory = root.join(label);
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("input.ox"), text).unwrap();
        let (program, sources) = verified_with_sources(text);
        let ir = if process {
            program.native_process_module(Some(hir::DefId(0)), &sources)
        } else if let Some(fuel) = fuel {
            program.native_module_with_fuel(hir::DefId(0), &sources, fuel)
        } else {
            program.native_module(Some(hir::DefId(0)), &sources)
        }
        .unwrap();
        fs::write(directory.join("module.ll"), &ir).unwrap();
        let run_directory = directory.join("source-free");
        fs::create_dir(&run_directory).unwrap();
        let executable = run_directory.join("program");
        crate::frontend::native::compile(&ir, executable.to_str().unwrap()).unwrap();
        let binary = fs::read(&executable).unwrap();
        assert_eq!(&binary[..4], b"\x7fELF");
        assert_eq!(fs::read_dir(&run_directory).unwrap().count(), 1);
        let output = Command::new(&executable)
            .current_dir(&run_directory)
            .output()
            .unwrap();
        fs::write(directory.join("stdout.bin"), &output.stdout).unwrap();
        fs::write(directory.join("stderr.bin"), &output.stderr).unwrap();
        fs::write(
            directory.join("status.txt"),
            format!("{:?}\n", output.status.code()),
        )
        .unwrap();
        assert_eq!(output.status.code(), Some(expected_status), "{label}");
        assert_eq!(output.stdout, expected_stdout.as_bytes(), "{label}");
        if let Some((code, message, primary)) = failure {
            let start = text
                .find(if primary == "x" {
                    "x.to_u8_checked()"
                } else {
                    primary
                })
                .unwrap();
            let span = sources
                .get(crate::frontend::source::SourceFileId(0))
                .span(start, start + primary.len());
            let expected =
                Diagnostic::new(code, "oir-run", message, Some(span)).render_human(&sources);
            assert_eq!(output.stderr, expected.as_bytes(), "{label}");
        } else {
            assert!(output.stderr.is_empty(), "{label}");
        }
        writeln!(&mut receipt, "{label} status={expected_status} source_bytes={} ir_bytes={} elf_bytes={} fuel={fuel:?} process={process}", text.len(), ir.len(), binary.len()).unwrap();
        println!(
            "U8_SCALAR_NATIVE_ELF {label} status={expected_status} fuel={fuel:?} process={process}"
        );
    };
    for value in [0, 127, 128, 255] {
        let text = ROUNDTRIP.replace("255", &value.to_string());
        observe(
            &format!("roundtrip-{value}"),
            &text,
            None,
            false,
            &format!("{value}\n"),
            0,
            None,
        );
    }
    for value in [-2_147_483_648_i32, -1, 256, 2_147_483_647] {
        let text = ROUNDTRIP.replace("255", &value.to_string());
        observe(
            &format!("range-{value}"),
            &text,
            None,
            false,
            "",
            1,
            Some((
                "E0610",
                "checked i32 to u8 conversion out of range",
                "to_u8_checked",
            )),
        );
    }
    let guarded = ROUNDTRIP.replacen("{", "{while false{}", 1);
    // Independent dynamic census: activation9, entry goto1, false assignment1,
    // branch1, then seven straight-line assignments and return1. Thus narrowing
    // is the16th paid unit, widening19th and successful return20th.
    for (fuel, primary) in [
        (14, "x"),
        (15, "x.to_u8_checked()"),
        (18, "b.to_i32()"),
        (19, "return b.to_i32();"),
    ] {
        observe(
            &format!("fuel-{fuel}"),
            &guarded,
            Some(fuel),
            false,
            "",
            1,
            Some(("E0601", "execution fuel exhausted", primary)),
        );
    }
    observe("fuel-20", &guarded, Some(20), false, "255\n", 0, None);
    let invalid = guarded.replace("255", "256");
    observe(
        "fuel-16-range",
        &invalid,
        Some(16),
        false,
        "",
        1,
        Some((
            "E0610",
            "checked i32 to u8 conversion out of range",
            "to_u8_checked",
        )),
    );
    observe("process-255", ROUNDTRIP, None, true, "", 255, None);
    observe(
        "process-range",
        &ROUNDTRIP.replace("255", "256"),
        None,
        true,
        "",
        1,
        Some((
            "E0610",
            "checked i32 to u8 conversion out of range",
            "to_u8_checked",
        )),
    );
    fs::write(root.join("receipts.txt"), receipt).unwrap();
}
