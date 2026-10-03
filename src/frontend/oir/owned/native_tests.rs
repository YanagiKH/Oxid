use super::super::consumer_fixtures as fixtures;
use super::super::execute;
type Fixture = fn() -> (SourceMap, RawOwnedProgram, fixtures::Schedule);

fn block(statements: Vec<OwnedStatement>, kind: OwnedTerminatorKind, span: Span) -> OwnedBlock {
    OwnedBlock {
        merge: None,
        span,
        statements,
        terminator: fixtures::end(kind, span),
    }
}
fn append_cycle(raw: &mut RawOwnedProgram) {
    let span = raw.functions[0].span;
    let mut f = fixtures::function(raw.functions.len(), ValueTy::Scalar(hir::Ty::Unit), span);
    f.locals.push(fixtures::scalar(hir::Ty::Unit, span));
    f.blocks.push(block(
        vec![fixtures::assign(0, Rvalue::Unit, span)],
        OwnedTerminatorKind::Goto(BlockId(0)),
        span,
    ));
    raw.functions.push(f);
}
fn verified_guarded(
    build: fn() -> (SourceMap, RawOwnedProgram, fixtures::Schedule),
) -> (SourceMap, VerifiedOwnedProgram, fixtures::Schedule) {
    let (sources, mut raw, schedule) = build();
    append_cycle(&mut raw);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    (sources, witness, schedule)
}
use super::*;

#[path = "source/native_resource_tests.rs"]
mod source_resources;

#[test]
fn native_owned_tiny_storage_and_exact_acyclic_costs() {
    for (build, expected) in [
        (fixtures::empty_record as Fixture, 17),
        (fixtures::owned_relay as Fixture, 52),
        (fixtures::shared_read as Fixture, 49),
    ] {
        let (sources, raw, schedule) = build();
        let witness = verified::verify_owned(raw, &sources).unwrap();
        let plan = ExecutionPlan::build(&witness).unwrap();
        assert_eq!(
            admit(&plan, Limits::DEFAULT).unwrap()[schedule.entry.0].cost + 1,
            expected
        );
        assert_eq!(schedule.fuel(), expected);
        let module = native_module(&witness, Some(schedule.entry), &sources).unwrap();
        assert!(module.contains("%scalars = alloca"));
        assert!(module.contains("%owners = alloca"));
        assert!(!module.contains("%fuel = alloca"));
        for forbidden in [
            "noalias",
            "inbounds",
            "nonnull",
            "dereferenceable",
            "sret",
            "byval",
            "llvm.lifetime",
            "memcpy",
            "undef",
            "poison",
        ] {
            assert!(!module.contains(forbidden), "{forbidden}");
        }
    }
}

#[test]
fn native_owned_entry_denial_precedes_plan_allocation() {
    let (sources, raw, _) = fixtures::empty_record();
    let witness = verified::verify_owned(raw, &sources).unwrap();
    for (entry, code, text) in [
        (None, "E0700", "declared"),
        (Some(hir::DefId(999)), "E0500", "identity"),
    ] {
        let error = plan::fail_allocation_after(0, || native_module(&witness, entry, &sources))
            .unwrap_err();
        assert_eq!(error.code, code);
        assert!(error.message.contains(text));
    }
    for build in [
        fixtures::owned_relay as Fixture,
        fixtures::shared_read as Fixture,
    ] {
        let (sources, raw, _) = build();
        let witness = verified::verify_owned(raw, &sources).unwrap();
        let error = plan::fail_allocation_after(0, || {
            native_module(&witness, Some(hir::DefId(1)), &sources)
        })
        .unwrap_err();
        assert_eq!(error.code, "E0700");
        assert!(error.message.contains("no parameters"));
    }
    let (sources, mut raw, _) = fixtures::empty_record();
    let f = &mut raw.functions[0];
    f.result = ValueTy::Owned(AggregateTy::Record(RecordId(0)));
    f.locals.clear();
    f.blocks[0].statements.truncate(2);
    f.blocks[0].terminator.as_mut().unwrap().kind =
        OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0));
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let error =
        plan::fail_allocation_after(0, || native_module(&witness, Some(hir::DefId(0)), &sources))
            .unwrap_err();
    assert_eq!(error.code, "E0700");
    assert!(error.message.contains("return a scalar"));

    let (sources, s) = fixtures::context();
    let mut f = fixtures::function(0, ValueTy::Scalar(hir::Ty::Bool), s(0));
    f.parameters.push(ParameterBinding::Scalar(LocalId(0)));
    f.locals.push(LocalDecl {
        ty: hir::Ty::Bool,
        kind: LocalKind::Parameter,
        span: s(0),
    });
    f.blocks.push(block(
        vec![],
        OwnedTerminatorKind::ReturnScalar(fixtures::operand(0, s(1))),
        s(1),
    ));
    let witness = verified::verify_owned(
        RawOwnedProgram {
            records: vec![],
            functions: vec![f],
        },
        &sources,
    )
    .unwrap();
    let error =
        plan::fail_allocation_after(0, || native_module(&witness, Some(hir::DefId(0)), &sources))
            .unwrap_err();
    assert_eq!(error.code, "E0700");
    assert!(error.message.contains("no parameters"));
}

#[test]
fn native_owned_caps_are_inclusive_and_account_exact_arenas() {
    let (sources, raw, schedule) = fixtures::empty_record();
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let limits = Limits {
        functions: 1,
        parameters: 0,
        function_slots: 2,
        scalar_slots: 1,
        blocks: 1,
        depth: 1,
        cost: 17,
        cells: 6,
        live_cells: 6,
        bytes: 12,
        live_bytes: 12,
        ..Limits::DEFAULT
    };
    assert!(native_module_limits(&witness, Some(schedule.entry), &sources, 0, limits).is_ok());
    for (lowered, marker) in [
        (
            Limits {
                functions: 0,
                ..limits
            },
            "function count",
        ),
        (
            Limits {
                function_slots: 1,
                ..limits
            },
            "scalar and owner",
        ),
        (
            Limits {
                scalar_slots: 0,
                ..limits
            },
            "aggregate scalar",
        ),
        (
            Limits {
                blocks: 0,
                ..limits
            },
            "blocks",
        ),
        (Limits { depth: 0, ..limits }, "depth"),
        (Limits { cost: 16, ..limits }, "fuel upper bound"),
        (Limits { cells: 5, ..limits }, "aggregate expanded"),
        (
            Limits {
                live_cells: 5,
                ..limits
            },
            "live expanded",
        ),
        (
            Limits {
                bytes: 11,
                ..limits
            },
            "aggregate storage",
        ),
        (
            Limits {
                live_bytes: 11,
                ..limits
            },
            "live storage",
        ),
    ] {
        let error =
            native_module_limits(&witness, Some(schedule.entry), &sources, 0, lowered).unwrap_err();
        assert_eq!(error.code, "E0700");
        assert!(error.message.contains(marker), "{}", error.message);
    }
    assert!(add(usize::MAX, 1).is_err());
    assert_eq!(add(usize::MAX, 0).unwrap(), usize::MAX);
    let plan = ExecutionPlan::build(&witness).unwrap();
    let module = native_module(&witness, Some(schedule.entry), &sources).unwrap();
    let diagnostics =
        Diagnostics::new(&plan, schedule.entry, &sources, false, MAX_DIAGNOSTIC_BYTES).unwrap();
    let bytes = diagnostics.messages.iter().map(String::len).sum();
    let exact = Limits {
        diagnostic_bytes: bytes,
        ir_bytes: module.len(),
        ..limits
    };
    assert_eq!(
        native_module_limits(&witness, Some(schedule.entry), &sources, 0, exact).unwrap(),
        module
    );
    assert!(native_module_limits(
        &witness,
        Some(schedule.entry),
        &sources,
        0,
        Limits {
            ir_bytes: module.len() - 1,
            ..exact
        }
    )
    .unwrap_err()
    .message
    .contains("LLVM bytes"));
    let error = plan::fail_allocation_after(0, || {
        native_module(&witness, Some(schedule.entry), &sources)
    })
    .unwrap_err();
    assert!(error.message.contains("injected owned allocation"));
}

#[test]
fn native_owned_guarded_table_has_independent_costs_and_origins() {
    for build in [
        fixtures::empty_record as Fixture,
        fixtures::owned_relay as Fixture,
        fixtures::shared_read as Fixture,
    ] {
        let (sources, witness, schedule) = verified_guarded(build);
        let plan = ExecutionPlan::build(&witness).unwrap();
        let module =
            native_module_with_fuel(&witness, schedule.entry, &sources, schedule.fuel()).unwrap();
        let diag =
            Diagnostics::new(&plan, schedule.entry, &sources, true, MAX_DIAGNOSTIC_BYTES).unwrap();
        let (root_span, root_cost) = schedule.events[0];
        assert!(module.contains(&format!(
            "%root_exhausted = icmp ult i64 %root_remaining, {root_cost}"
        )));
        let (id, len) = diag.get(FailureKind::Fuel, root_span);
        assert!(module.contains(&format!("root_error:\n  call void @__oxid_overflow(ptr @__oxid_owned_error_{id}, i64 {len})\n  unreachable")));
        for &(span, cost) in &schedule.events[1..] {
            let mut found = false;
            for f in witness
                .functions()
                .iter()
                .take(witness.functions().len() - 1)
            {
                for (b, block) in f.blocks.iter().enumerate() {
                    for (i, statement) in block.statements.iter().enumerate() {
                        if plan::instruction_span(statement) == span {
                            assert_eq!(plan.statement_cost(f.id, &statement.kind), cost);
                            assert!(module.contains(&format!("%f{}_b{b}_g{}_exhausted = icmp ult i64 %f{}_b{b}_g{}_remaining, {cost}", f.id.0, i + 1, f.id.0, i + 1)));
                            found = true;
                        }
                    }
                    let term = block.terminator.as_ref().unwrap();
                    if term.span == span {
                        assert_eq!(plan.terminator_cost(f.id, &term.kind), cost);
                        found = true;
                    }
                }
            }
            assert!(found, "{span:?}");
        }
        let data_bytes = diag.messages.iter().map(String::len).sum();
        assert!(native_module_limits(
            &witness,
            Some(schedule.entry),
            &sources,
            schedule.fuel(),
            Limits {
                diagnostic_bytes: data_bytes,
                ir_bytes: module.len(),
                ..Limits::DEFAULT
            }
        )
        .is_ok());
        assert!(native_module_limits(
            &witness,
            Some(schedule.entry),
            &sources,
            schedule.fuel(),
            Limits {
                diagnostic_bytes: data_bytes - 1,
                ..Limits::DEFAULT
            }
        )
        .unwrap_err()
        .message
        .contains("diagnostic bytes"));
    }
}

struct Scratch(std::path::PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "oxid-owned-native-{}-{stamp}-{n}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn compile(&self, module: &str, name: &str) -> std::path::PathBuf {
        let assembler = std::env::var_os("OXID_LLVM_BIN").map_or_else(
            || std::path::PathBuf::from("llvm-as-19"),
            |p| std::path::PathBuf::from(p).join("llvm-as"),
        );
        let version = std::process::Command::new(&assembler)
            .arg("--version")
            .output()
            .unwrap();
        assert!(version.status.success());
        assert!(String::from_utf8_lossy(&version.stdout)
            .lines()
            .any(|line| line
                .trim()
                .split_once("LLVM version 19.1.7")
                .is_some_and(|(_, after)| after.is_empty() || after.starts_with([' ', '(']))));
        let input = self.0.join(format!("{name}.ll"));
        let bitcode = self.0.join(format!("{name}.bc"));
        std::fs::write(&input, module).unwrap();
        let assembled = std::process::Command::new(assembler)
            .arg(&input)
            .arg("-o")
            .arg(&bitcode)
            .output()
            .unwrap();
        assert!(
            assembled.status.success(),
            "{}",
            String::from_utf8_lossy(&assembled.stderr)
        );
        std::fs::remove_file(input).unwrap();
        std::fs::remove_file(bitcode).unwrap();
        let output = self.0.join(name);
        crate::frontend::native::compile(module, output.to_str().unwrap()).unwrap();
        assert_eq!(&std::fs::read(&output).unwrap()[..4], b"\x7fELF");
        if let Some(evidence) = std::env::var_os("OXID_OWNED_NATIVE_EVIDENCE") {
            let evidence = std::path::PathBuf::from(evidence);
            std::fs::create_dir_all(&evidence).unwrap();
            std::fs::write(evidence.join(format!("{name}.ll")), module).unwrap();
            std::fs::copy(&output, evidence.join(format!("{name}.elf"))).unwrap();
        }
        output
    }
    fn run(&self, output: &std::path::Path, args: &[String]) -> std::process::Output {
        std::process::Command::new(output)
            .args(args)
            .current_dir(&self.0)
            .env_clear()
            .env("PATH", self.0.join("no-tools"))
            .output()
            .unwrap()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn scalar_output(value: Scalar) -> Vec<u8> {
    match value {
        Scalar::Bool(v) => format!("{v}\n"),
        Scalar::I32(v) => format!("{v}\n"),
        Scalar::Unit => "()\n".into(),
    }
    .into_bytes()
}
fn assert_result(result: std::process::Output, stdout: &[u8], stderr: &[u8], status: i32) {
    assert_eq!(result.status.code(), Some(status));
    assert_eq!(result.stdout, stdout);
    assert_eq!(result.stderr, stderr);
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7; explicitly run the owned native gate"]
fn native_owned_tiny_fixtures_use_real_llvm() {
    let scratch = Scratch::new();
    let mut artifacts = 0;
    for build in [
        fixtures::empty_record as Fixture,
        fixtures::owned_relay as Fixture,
        fixtures::shared_read as Fixture,
    ] {
        let (sources, raw, schedule) = build();
        let witness = verified::verify_owned(raw, &sources).unwrap();
        assert_eq!(
            execute::run(&witness, Some(schedule.entry)),
            Ok(schedule.result)
        );
        let module = native_module(&witness, Some(schedule.entry), &sources).unwrap();
        let binary = scratch.compile(&module, &format!("a{artifacts}"));
        artifacts += 1;
        assert_result(
            scratch.run(&binary, &[]),
            &scalar_output(schedule.result),
            b"",
            0,
        );
        let (sources, witness, schedule) = verified_guarded(build);
        for fuel in 0..=schedule.fuel() {
            let expected_error = schedule.failure(fuel).map(|span| {
                RunFailure::Fuel(span)
                    .diagnostic(&sources)
                    .render_human(&sources)
            });
            let reference = execute::run_limits(
                &witness,
                Some(schedule.entry),
                execute::Limits {
                    fuel,
                    ..execute::Limits::default()
                },
            );
            match &expected_error {
                Some(expected) => assert_eq!(
                    reference
                        .unwrap_err()
                        .diagnostic(&sources)
                        .render_human(&sources),
                    *expected
                ),
                None => assert_eq!(reference, Ok(schedule.result)),
            }
            let module = native_module_with_fuel(&witness, schedule.entry, &sources, fuel).unwrap();
            let binary = scratch.compile(&module, &format!("a{artifacts}"));
            artifacts += 1;
            match expected_error {
                Some(error) => assert_result(scratch.run(&binary, &[]), b"", error.as_bytes(), 1),
                None => assert_result(
                    scratch.run(&binary, &[]),
                    &scalar_output(schedule.result),
                    b"",
                    0,
                ),
            }
        }
    }
    assert_eq!(artifacts, 124);
    eprintln!(
        "owned tiny real LLVM: {artifacts} source-free ELF artifacts, exact all-budget parity"
    );
}

// Test-only wrapper takes a fuel budget from argv. The complete production
// function bodies, diagnostics and root guard remain byte-for-byte unchanged.
// This avoids recompiling 1,087 identical Batch bodies; production-wrapper
// binaries at the exact boundary are separately exercised below.
fn argv_fuel_harness(module: &str, budget: usize) -> String {
    let old_header = "define i32 @main() {\nentry:\n";
    let new_header = "declare i64 @strtoull(ptr, ptr, i32)\ndefine i32 @main(i32 %argc, ptr %argv) {\nentry:\n  %test_arg_slot = getelementptr ptr, ptr %argv, i64 1\n  %test_arg = load ptr, ptr %test_arg_slot\n  %test_parsed_fuel = call i64 @strtoull(ptr %test_arg, ptr null, i32 10)\n  %test_over_cap = icmp ugt i64 %test_parsed_fuel, 1000000\n  %test_fuel = select i1 %test_over_cap, i64 1000000, i64 %test_parsed_fuel\n";
    let old_store = format!("store i64 {budget}, ptr %fuel, align 8");
    assert_eq!(module.matches(old_header).count(), 1);
    assert_eq!(module.matches(&old_store).count(), 1);
    let body = module.split_once(old_header).unwrap().0;
    let result = module.replacen(old_header, new_header, 1).replacen(
        &old_store,
        "store i64 %test_fuel, ptr %fuel, align 8",
        1,
    );
    assert!(result.starts_with(body));
    result
}

#[test]
fn native_owned_batch_census_and_whole_call_path_are_independent() {
    let (sources, raw, schedule) = super::super::consumer_pilot::batch();
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    let bounds = admit(&plan, Limits::DEFAULT).unwrap();
    assert_eq!(schedule.fuel(), 1_086);
    assert_eq!(schedule.result, Scalar::I32(816));
    let expected = [
        (111, 304),
        (12, 40),
        (17, 80),
        (26, 48),
        (11, 32),
        (8, 16),
        (15, 72),
    ];
    for (f, &(cells, bytes)) in witness.functions().iter().zip(&expected) {
        assert_eq!(plan.function(f.id).usage().expanded_cells, cells);
        assert_eq!(plan.function(f.id).usage().native_bytes, bytes);
    }
    assert_eq!(bounds[0].cells, 154);
    assert_eq!(bounds[0].bytes, 432);
    assert_eq!(bounds[0].depth, 3);
    assert_eq!(bounds[0].scalar_slots, 30);
    assert!(bounds[0].cyclic);
    let exact = Limits {
        cells: 200,
        live_cells: 154,
        bytes: 600,
        live_bytes: 440,
        ..Limits::DEFAULT
    };
    assert!(admit(&plan, exact).is_ok());
    for (limits, message) in [
        (
            Limits {
                cells: 199,
                ..exact
            },
            "aggregate expanded",
        ),
        (
            Limits {
                live_cells: 153,
                ..exact
            },
            "live expanded",
        ),
        (
            Limits {
                bytes: 599,
                ..exact
            },
            "aggregate storage",
        ),
        (
            Limits {
                live_bytes: 439,
                ..exact
            },
            "live storage",
        ),
    ] {
        assert!(admit(&plan, limits).unwrap_err().message.contains(message));
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7; explicitly run the owned native gate"]
fn native_owned_batch_every_budget_uses_real_llvm() {
    let scratch = Scratch::new();
    let (sources, raw, schedule) = super::super::consumer_pilot::batch();
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let module =
        native_module_with_fuel(&witness, schedule.entry, &sources, schedule.fuel()).unwrap();
    let harness = argv_fuel_harness(&module, schedule.fuel());
    let binary = scratch.compile(&harness, "batch-all-budgets");
    for fuel in 0..=schedule.fuel() {
        let reference = execute::run_limits(
            &witness,
            Some(schedule.entry),
            execute::Limits {
                fuel,
                ..execute::Limits::default()
            },
        );
        let actual = scratch.run(&binary, &[fuel.to_string()]);
        match schedule.failure(fuel) {
            Some(span) => {
                let expected = RunFailure::Fuel(span)
                    .diagnostic(&sources)
                    .render_human(&sources);
                assert_eq!(
                    reference
                        .unwrap_err()
                        .diagnostic(&sources)
                        .render_human(&sources),
                    expected
                );
                assert_result(actual, b"", expected.as_bytes(), 1);
            }
            None => {
                assert_eq!(reference, Ok(Scalar::I32(816)));
                assert_result(actual, b"816\n", b"", 0);
            }
        }
    }
    for fuel in [schedule.fuel() - 1, schedule.fuel(), plan::MAX_FUEL] {
        let module = native_module_with_fuel(&witness, schedule.entry, &sources, fuel).unwrap();
        let binary = scratch.compile(&module, &format!("batch-production-{fuel}"));
        match schedule.failure(fuel) {
            Some(span) => assert_result(
                scratch.run(&binary, &[]),
                b"",
                RunFailure::Fuel(span)
                    .diagnostic(&sources)
                    .render_human(&sources)
                    .as_bytes(),
                1,
            ),
            None => assert_result(scratch.run(&binary, &[]), b"816\n", b"", 0),
        }
    }
    eprintln!("owned Batch real LLVM: 1,087 independent all-budget processes plus 3 production wrappers; result 816");
}

fn overflow_before_field(overflow: bool) -> (SourceMap, RawOwnedProgram) {
    use fixtures::*;
    let (sources, s) = context();
    let field = FieldId {
        record: RecordId(0),
        index: 0,
    };
    let base = AccessBase::Owner(OwnerPlaceId(0));
    let mut f = function(0, ValueTy::Scalar(hir::Ty::I32), s(0));
    f.locals = (0..5).map(|i| scalar(hir::Ty::I32, s(i + 1))).collect();
    f.owners = vec![owner(OwnerKind::Local { mutable: true }, s(0))];
    f.blocks = vec![block(
        vec![
            assign(0, Rvalue::I32(17), s(1)),
            instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), s(2)),
            instruction(
                OwnedInstruction::Construct {
                    destination: OwnerPlaceId(0),
                    fields: vec![(field, operand(0, s(3)))],
                },
                s(3),
            ),
            assign(1, Rvalue::I32(if overflow { i32::MAX } else { 40 }), s(4)),
            assign(2, Rvalue::I32(1), s(5)),
            instruction(
                OwnedInstruction::WriteField {
                    base,
                    field,
                    value: operand(2, s(6)),
                },
                s(6),
            ),
            assign(
                3,
                Rvalue::CheckedI32 {
                    op: hir::ArithmeticOp::Add,
                    left: operand(1, s(7)),
                    right: operand(2, s(7)),
                    operator_span: s(30),
                },
                s(7),
            ),
            instruction(
                OwnedInstruction::WriteField {
                    base,
                    field,
                    value: operand(3, s(8)),
                },
                s(8),
            ),
            instruction(
                OwnedInstruction::ReadField {
                    destination: LocalId(4),
                    base,
                    field,
                },
                s(9),
            ),
            instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(0)), s(10)),
        ],
        OwnedTerminatorKind::ReturnScalar(operand(4, s(11))),
        s(11),
    )];
    let mut raw = RawOwnedProgram {
        records: vec![record(&[hir::Ty::I32], s(0))],
        functions: vec![f],
    };
    append_cycle(&mut raw);
    (sources, raw)
}

// Independent LLVM-text CFG reader. No production predecessor, guard or
// dominance helper is imported. Exact emitted instruction lines are retained.
struct LlvmCfg {
    blocks: std::collections::BTreeMap<String, Vec<String>>,
    edges: std::collections::BTreeMap<String, Vec<String>>,
}
impl LlvmCfg {
    fn parse(module: &str, function: &str) -> Self {
        let mut in_function = false;
        let mut current = String::new();
        let mut blocks = std::collections::BTreeMap::<String, Vec<String>>::new();
        for line in module.lines() {
            if line.starts_with("define ") && line.contains(&format!("@{function}(")) {
                in_function = true;
                continue;
            }
            if !in_function {
                continue;
            }
            if line == "}" {
                break;
            }
            if let Some(label) = line.strip_suffix(':') {
                current = label.to_string();
                assert!(blocks.insert(current.clone(), vec![]).is_none());
            } else if !line.trim().is_empty() {
                blocks
                    .get_mut(&current)
                    .unwrap()
                    .push(line.trim().to_string());
            }
        }
        assert!(!blocks.is_empty());
        let mut edges = std::collections::BTreeMap::new();
        for (name, lines) in &blocks {
            let last = lines.last().unwrap();
            let next: Vec<_> = if last.starts_with("br ") {
                last.split("label %")
                    .skip(1)
                    .map(|s| s.split([',', ' ']).next().unwrap().to_string())
                    .collect()
            } else {
                assert!(last.starts_with("ret ") || last == "unreachable", "{last}");
                vec![]
            };
            for target in &next {
                assert!(blocks.contains_key(target), "{target}");
            }
            edges.insert(name.clone(), next);
        }
        Self { blocks, edges }
    }
    fn reaching(&self, start: &str, excluded: Option<&str>) -> std::collections::BTreeSet<String> {
        let mut reached = std::collections::BTreeSet::new();
        let mut ready = vec![start.to_string()];
        while let Some(n) = ready.pop() {
            if Some(n.as_str()) == excluded || !reached.insert(n.clone()) {
                continue;
            }
            ready.extend(self.edges[&n].iter().cloned());
        }
        reached
    }
    fn store_block(&self, instruction: &str) -> &str {
        let matches: Vec<_> = self
            .blocks
            .iter()
            .filter(|(_, lines)| lines.iter().any(|line| line == instruction))
            .collect();
        assert_eq!(matches.len(), 1, "store: {instruction}");
        matches[0].0
    }
    fn assert_success_dominates(&self, success: &str, failure: &str, store: &str) {
        assert!(
            self.success_dominates(success, failure, store),
            "{success}/{failure} do not protect {store}"
        );
    }
    fn success_dominates(&self, success: &str, failure: &str, store: &str) -> bool {
        let target = self.store_block(store);
        let error = &self.blocks[failure];
        self.reaching("entry", None).contains(target)
            && self.reaching(success, None).contains(target)
            && !self.reaching("entry", Some(success)).contains(target)
            && !self.reaching(failure, None).contains(target)
            && error
                .iter()
                .any(|line| line.starts_with("call void @__oxid_overflow("))
            && error.last().unwrap() == "unreachable"
    }
}

fn hoist_field_store_before_fuel_guard(module: &str) -> String {
    let movement = "  %f0_b0_i7_value_wide = load i64, ptr %s3, align 8\n  %f0_b0_i7_value = trunc i64 %f0_b0_i7_value_wide to i32\n  %f0_b0_i7_ptr = getelementptr i8, ptr %o0, i64 0\n  store i32 %f0_b0_i7_value, ptr %f0_b0_i7_ptr, align 1\n";
    assert_eq!(module.matches(movement).count(), 1);
    let marker = "  %f0_b0_g8_remaining = load i64, ptr %fuel";
    let mutant =
        module
            .replacen(movement, "", 1)
            .replacen(marker, &format!("{movement}{marker}"), 1);
    let cfg = LlvmCfg::parse(&mutant, "__oxid_owned_fn_0");
    assert!(!cfg.success_dominates(
        "f0_b0_g8_ok",
        "f0_b0_g8_error",
        "store i32 %f0_b0_i7_value, ptr %f0_b0_i7_ptr, align 1"
    ));
    mutant
}

fn verify_store_guards(module: &str) {
    let cfg = LlvmCfg::parse(module, "__oxid_owned_fn_0");
    let final_store = "store i32 %f0_b0_i7_value, ptr %f0_b0_i7_ptr, align 1";
    cfg.assert_success_dominates("f0_b0_g8_ok", "f0_b0_g8_error", final_store);
    cfg.assert_success_dominates("f0_b0_i6_checked_ok", "f0_b0_i6_checked_error", final_store);
    cfg.assert_success_dominates(
        "f0_b0_g7_ok",
        "f0_b0_g7_error",
        "store i64 %f0_b0_i6_store_wide, ptr %s3, align 8",
    );
    cfg.assert_success_dominates(
        "f0_b0_i6_checked_ok",
        "f0_b0_i6_checked_error",
        "store i64 %f0_b0_i6_store_wide, ptr %s3, align 8",
    );
    let first_store = cfg.store_block("store i32 %f0_b0_i5_value, ptr %f0_b0_i5_ptr, align 1");
    assert!(!cfg
        .reaching("entry", Some(first_store))
        .contains("f0_b0_i6_checked_error"));
}

#[test]
fn native_owned_guard_success_edges_dominate_field_and_return_stores() {
    let (sources, raw) = overflow_before_field(true);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    verify_store_guards(&native_module(&witness, Some(hir::DefId(0)), &sources).unwrap());
    hoist_field_store_before_fuel_guard(
        &native_module(&witness, Some(hir::DefId(0)), &sources).unwrap(),
    );
    let (sources, witness, _) = verified_guarded(fixtures::owned_relay);
    let module = native_module(&witness, Some(hir::DefId(0)), &sources).unwrap();
    let callee = LlvmCfg::parse(&module, "__oxid_owned_fn_1");
    callee.assert_success_dominates(
        "f1_b0_g1_ok",
        "f1_b0_g1_error",
        "store i32 %f1_b0_term_field0, ptr %f1_b0_term_out0_ptr, align 1",
    );
    let caller = LlvmCfg::parse(&module, "__oxid_owned_fn_0");
    assert_eq!(
        caller.store_block("call void @__oxid_owned_fn_1(ptr %fuel, ptr %o2, ptr %o1)"),
        "f0_b0_g6_ok"
    );
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7; explicitly run the owned native gate"]
fn native_owned_actual_llvm_guard_dominance_and_failure_order() {
    let scratch = Scratch::new();
    for overflow in [false, true] {
        let (sources, raw) = overflow_before_field(overflow);
        let witness = verified::verify_owned(raw, &sources).unwrap();
        // Root11; assign/live/construct +4; constants +2; first write +1;
        // arithmetic +1 gives19. Final write's charge is the next operation.
        for fuel in [18, 19, 20, plan::MAX_FUEL] {
            let module = native_module_with_fuel(&witness, hir::DefId(0), &sources, fuel).unwrap();
            verify_store_guards(&module);
            let binary = scratch.compile(&module, &format!("failure-{overflow}-{fuel}"));
            let reference = execute::run_limits(
                &witness,
                Some(hir::DefId(0)),
                execute::Limits {
                    fuel,
                    ..execute::Limits::default()
                },
            );
            match reference {
                Ok(value) => {
                    assert_result(scratch.run(&binary, &[]), &scalar_output(value), b"", 0)
                }
                Err(error) => assert_result(
                    scratch.run(&binary, &[]),
                    b"",
                    error.diagnostic(&sources).render_human(&sources).as_bytes(),
                    1,
                ),
            }
            if !overflow && fuel == 19 {
                // LLVM verification accepts this hoisted-store mutant, and
                // stderr/status parity alone cannot see its premature write.
                // The independent CFG checker above must reject it.
                let mutant = hoist_field_store_before_fuel_guard(&module);
                let mutant_binary = scratch.compile(&mutant, "misordered-field-store-mutant");
                let expected = execute::run_limits(
                    &witness,
                    Some(hir::DefId(0)),
                    execute::Limits {
                        fuel,
                        ..execute::Limits::default()
                    },
                )
                .unwrap_err()
                .diagnostic(&sources)
                .render_human(&sources);
                assert_result(
                    scratch.run(&mutant_binary, &[]),
                    b"",
                    expected.as_bytes(),
                    1,
                );
            }
        }
    }
}

fn scalar_call_chain(
    depth: usize,
    fanout: usize,
    cyclic_leaf: bool,
) -> (SourceMap, RawOwnedProgram) {
    use fixtures::*;
    let (sources, s) = context();
    let mut functions = Vec::new();
    let mut leaf = function(0, ValueTy::Scalar(hir::Ty::Unit), s(0));
    leaf.locals.push(scalar(hir::Ty::Unit, s(0)));
    leaf.blocks.push(block(
        vec![assign(0, Rvalue::Unit, s(1))],
        if cyclic_leaf {
            OwnedTerminatorKind::Goto(BlockId(0))
        } else {
            OwnedTerminatorKind::ReturnScalar(operand(0, s(2)))
        },
        s(2),
    ));
    functions.push(leaf);
    for id in 1..depth {
        let mut f = function(id, ValueTy::Scalar(hir::Ty::Unit), s(id * 32));
        for call in 0..fanout {
            let span = s(id * 32 + call + 1);
            f.locals.push(scalar(hir::Ty::Unit, span));
            f.calls.push(CallDecl {
                target: hir::DefId(id - 1),
                arguments: vec![],
                result: CallResult::Scalar(LocalId(call)),
                parent: None,
                span,
            });
            f.blocks.push(block(
                vec![instruction(
                    OwnedInstruction::OpenCall(CallSiteId(call)),
                    span,
                )],
                OwnedTerminatorKind::Invoke {
                    call: CallSiteId(call),
                    continuation: BlockId(call + 1),
                },
                span,
            ));
        }
        f.blocks.push(block(
            vec![],
            OwnedTerminatorKind::ReturnScalar(operand(fanout - 1, s(id * 32 + 20))),
            s(id * 32 + 20),
        ));
        functions.push(f);
    }
    (
        sources,
        RawOwnedProgram {
            records: vec![],
            functions,
        },
    )
}

#[test]
fn native_owned_unknown_cyclic_cost_never_becomes_a_false_overflow_gate() {
    let (sources, raw) = scalar_call_chain(22, 8, true);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    let bounds = admit(&plan, Limits::DEFAULT).unwrap();
    assert!(bounds.iter().all(|b| b.cyclic));
    assert_eq!(bounds[21].depth, 22);
    assert_eq!(bounds[21].cost, usize::MAX);
    assert_eq!(bounds[21].cells, 505);
    assert!(native_module(&witness, Some(hir::DefId(21)), &sources).is_ok());
    // An unrelated cyclic function must not erase an expensive finite bound.
    let (sources, mut raw) = scalar_call_chain(9, 8, false);
    append_cycle(&mut raw);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    assert!(native_module(&witness, Some(hir::DefId(0)), &sources)
        .unwrap_err()
        .message
        .contains("fuel upper bound"));
}

#[test]
fn native_owned_whole_graph_depth_recursion_and_arity_boundaries() {
    for depth in [32, 33] {
        let (sources, raw) = scalar_call_chain(depth, 1, false);
        let witness = verified::verify_owned(raw, &sources).unwrap();
        let result = native_module(&witness, Some(hir::DefId(depth - 1)), &sources);
        if depth == 32 {
            assert!(result.is_ok());
        } else {
            assert!(result.unwrap_err().message.contains("call depth"));
        }
    }
    let (sources, mut raw) = scalar_call_chain(2, 1, false);
    raw.functions[1].calls[0].target = hir::DefId(1);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    assert!(native_module(&witness, Some(hir::DefId(0)), &sources)
        .unwrap_err()
        .message
        .contains("recursive"));
    for parameters in [64, 65, 256] {
        let (sources, mut raw, _) = fixtures::empty_record();
        let span = raw.functions[0].span;
        let mut f = fixtures::function(1, ValueTy::Scalar(hir::Ty::Bool), span);
        f.parameters = (0..parameters)
            .map(|i| ParameterBinding::Scalar(LocalId(i)))
            .collect();
        f.locals = (0..parameters)
            .map(|_| LocalDecl {
                ty: hir::Ty::Bool,
                kind: LocalKind::Parameter,
                span,
            })
            .collect();
        f.blocks.push(block(
            vec![],
            OwnedTerminatorKind::ReturnScalar(fixtures::operand(0, span)),
            span,
        ));
        raw.functions.push(f);
        let witness = verified::verify_owned(raw, &sources).unwrap();
        let result = native_module(&witness, Some(hir::DefId(0)), &sources);
        if parameters == 64 {
            assert!(result.is_ok());
        } else {
            assert!(result.unwrap_err().message.contains("parameter count"));
        }
    }
}

fn looping_merge(reversed: bool) -> (SourceMap, RawOwnedProgram) {
    use fixtures::*;
    let (sources, s) = context();
    let mut f = function(0, ValueTy::Scalar(hir::Ty::Bool), s(0));
    let types = [
        hir::Ty::I32,
        hir::Ty::I32,
        hir::Ty::I32,
        hir::Ty::Bool,
        hir::Ty::I32,
        hir::Ty::Bool,
        hir::Ty::Bool,
        hir::Ty::Bool,
        hir::Ty::I32,
        hir::Ty::Bool,
        hir::Ty::I32,
        hir::Ty::Bool,
        hir::Ty::I32,
        hir::Ty::I32,
        hir::Ty::Bool,
    ];
    f.locals = types
        .iter()
        .enumerate()
        .map(|(i, &t)| scalar(t, s(i + 1)))
        .collect();
    f.places = vec![
        PlaceDecl {
            ty: hir::Ty::I32,
            span: s(0),
        },
        PlaceDecl {
            ty: hir::Ty::Bool,
            span: s(0),
        },
    ];
    let p = |i| Place {
        id: PlaceId(i),
        span: s(0),
    };
    let assign_place = |i, v, init| {
        instruction(
            OwnedInstruction::Scalar(if init {
                Statement::Initialize {
                    place: p(i),
                    value: operand(v, s(0)),
                    span: s(0),
                }
            } else {
                Statement::Store {
                    place: p(i),
                    value: operand(v, s(0)),
                    operator_span: s(0),
                    span: s(0),
                }
            }),
            s(0),
        )
    };
    let checked = |destination, op, left, right| {
        assign(
            destination,
            Rvalue::CheckedI32 {
                op,
                left: operand(left, s(20)),
                right: operand(right, s(20)),
                operator_span: s(21),
            },
            s(20),
        )
    };
    f.blocks = vec![
        block(
            vec![
                assign(0, Rvalue::I32(0), s(1)),
                assign(1, Rvalue::I32(1), s(2)),
                assign(2, Rvalue::I32(3), s(3)),
                assign(3, Rvalue::Bool(false), s(4)),
                assign_place(0, 0, true),
                assign_place(1, 3, true),
            ],
            OwnedTerminatorKind::Goto(BlockId(1)),
            s(0),
        ),
        block(
            vec![
                assign(4, Rvalue::Load(p(0)), s(5)),
                assign(
                    5,
                    Rvalue::CompareScalar {
                        op: hir::ComparisonOp::Less,
                        left: operand(4, s(6)),
                        right: operand(2, s(6)),
                        operator_span: s(6),
                    },
                    s(6),
                ),
            ],
            OwnedTerminatorKind::Branch {
                condition: operand(5, s(7)),
                then_block: BlockId(2),
                else_block: BlockId(6),
            },
            s(7),
        ),
        block(
            vec![assign(
                6,
                Rvalue::CompareScalar {
                    op: hir::ComparisonOp::Equal,
                    left: operand(4, s(8)),
                    right: operand(1, s(8)),
                    operator_span: s(8),
                },
                s(8),
            )],
            OwnedTerminatorKind::Branch {
                condition: operand(6, s(9)),
                then_block: BlockId(3),
                else_block: BlockId(4),
            },
            s(9),
        ),
        block(
            vec![
                assign(7, Rvalue::Bool(true), s(10)),
                checked(8, hir::ArithmeticOp::Add, 1, 1),
            ],
            OwnedTerminatorKind::Goto(BlockId(5)),
            s(11),
        ),
        block(
            vec![
                assign(9, Rvalue::Bool(false), s(12)),
                checked(10, hir::ArithmeticOp::Multiply, 1, 1),
            ],
            OwnedTerminatorKind::Goto(BlockId(5)),
            s(13),
        ),
        block(
            vec![
                assign_place(1, 11, false),
                assign(12, Rvalue::Load(p(0)), s(14)),
                checked(13, hir::ArithmeticOp::Add, 12, 1),
                assign_place(0, 13, false),
            ],
            OwnedTerminatorKind::Goto(BlockId(1)),
            s(15),
        ),
        block(
            vec![assign(14, Rvalue::Load(p(1)), s(16))],
            OwnedTerminatorKind::ReturnScalar(operand(14, s(17))),
            s(17),
        ),
    ];
    f.blocks[5].merge = Some(BoolMerge {
        destination: LocalId(11),
        incoming: [
            MergeInput {
                predecessor: BlockId(3),
                value: operand(7, s(18)),
            },
            MergeInput {
                predecessor: BlockId(4),
                value: operand(9, s(18)),
            },
        ],
        operator_span: s(18),
        span: s(18),
    });
    if reversed {
        let n = f.blocks.len();
        f.entry = BlockId(n - 1);
        f.blocks.reverse();
        for b in &mut f.blocks {
            if let Some(m) = &mut b.merge {
                for input in &mut m.incoming {
                    input.predecessor.0 = n - 1 - input.predecessor.0;
                }
            }
            match &mut b.terminator.as_mut().unwrap().kind {
                OwnedTerminatorKind::Branch {
                    then_block,
                    else_block,
                    ..
                } => {
                    then_block.0 = n - 1 - then_block.0;
                    else_block.0 = n - 1 - else_block.0;
                }
                OwnedTerminatorKind::Goto(target) => target.0 = n - 1 - target.0,
                _ => {}
            }
        }
    }
    (
        sources,
        RawOwnedProgram {
            records: vec![],
            functions: vec![f],
        },
    )
}

#[test]
fn native_owned_guarded_merges_select_only_the_taken_slot() {
    for reversed in [false, true] {
        let (sources, raw) = looping_merge(reversed);
        let witness = verified::verify_owned(raw, &sources).unwrap();
        assert_eq!(
            execute::run(&witness, Some(hir::DefId(0))),
            Ok(Scalar::Bool(false))
        );
        let module = native_module(&witness, Some(hir::DefId(0)), &sources).unwrap();
        let f = &witness.functions()[0];
        let (i, block) = f
            .blocks
            .iter()
            .enumerate()
            .find(|(_, b)| b.merge.is_some())
            .unwrap();
        let merge = block.merge.as_ref().unwrap();
        for input in merge.incoming {
            assert!(module.contains(&format!(
                "[ %s{}, %f0_b{}_g3_ok ]",
                input.value.local.0, input.predecessor.0
            )));
        }
        let cfg = LlvmCfg::parse(&module, "__oxid_owned_fn_0");
        assert!(cfg.blocks[&format!("b{i}")][0].contains("phi ptr"));
        assert!(!cfg.blocks[&format!("b{i}")]
            .iter()
            .any(|line| line.contains("load i64, ptr %s7") || line.contains("load i64, ptr %s9")));
        assert!(cfg.blocks[&format!("f0_b{i}_g0_ok")]
            .iter()
            .any(|line| line.contains("load i64, ptr %f0_") && line.contains("_merge_slot")));
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7; explicitly run the owned native gate"]
fn native_owned_merge_and_depth_boundaries_use_real_llvm() {
    let scratch = Scratch::new();
    for reversed in [false, true] {
        let (sources, raw) = looping_merge(reversed);
        let witness = verified::verify_owned(raw, &sources).unwrap();
        let module = native_module(&witness, Some(hir::DefId(0)), &sources).unwrap();
        let binary = scratch.compile(&module, &format!("merge-{reversed}"));
        assert_result(scratch.run(&binary, &[]), b"false\n", b"", 0);
    }
    let (sources, raw) = scalar_call_chain(32, 1, false);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let module = native_module(&witness, Some(hir::DefId(31)), &sources).unwrap();
    let binary = scratch.compile(&module, "depth32");
    assert_result(scratch.run(&binary, &[]), b"()\n", b"", 0);
    let (sources, raw) = scalar_call_chain(22, 8, true);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let module = native_module_with_fuel(&witness, hir::DefId(21), &sources, 600).unwrap();
    let binary = scratch.compile(&module, "unknown-cost");
    let expected = execute::run_limits(
        &witness,
        Some(hir::DefId(21)),
        execute::Limits {
            fuel: 600,
            ..execute::Limits::default()
        },
    )
    .unwrap_err()
    .diagnostic(&sources)
    .render_human(&sources);
    assert_result(scratch.run(&binary, &[]), b"", expected.as_bytes(), 1);
}

fn empty_relay() -> (SourceMap, RawOwnedProgram, fixtures::Schedule) {
    let (sources, mut raw, mut schedule) = fixtures::owned_relay();
    let span = raw.functions[0].span;
    raw.records = vec![fixtures::record(&[], span)];
    let f = &mut raw.functions[0];
    f.result = ValueTy::Scalar(hir::Ty::Unit);
    for l in &mut f.locals {
        l.ty = hir::Ty::Unit;
    }
    f.blocks[0].statements[0] = fixtures::assign(0, Rvalue::Unit, f.blocks[0].statements[0].span);
    let OwnedInstruction::Construct { fields, .. } = &mut f.blocks[0].statements[2].kind else {
        unreachable!()
    };
    fields.clear();
    f.blocks[1].statements[0] = fixtures::assign(1, Rvalue::Unit, f.blocks[1].statements[0].span);
    schedule.result = Scalar::Unit;
    (sources, raw, schedule)
}
fn nested_relay() -> (SourceMap, RawOwnedProgram) {
    use fixtures::*;
    let (sources, mut raw, _) = owned_relay();
    let mut leaf = raw.functions[1].clone();
    leaf.id = hir::DefId(2);
    let f = &mut raw.functions[1];
    let span = f.span;
    f.owners.extend([
        owner(
            OwnerKind::StagedArgument {
                call: CallSiteId(0),
                argument: 0,
            },
            span,
        ),
        owner(
            OwnerKind::CallResult {
                call: CallSiteId(0),
            },
            span,
        ),
    ]);
    f.calls.push(CallDecl {
        target: hir::DefId(2),
        arguments: vec![ArgumentSlot::Owned(OwnerPlaceId(1))],
        result: CallResult::Owned(OwnerPlaceId(2)),
        parent: None,
        span,
    });
    f.blocks = vec![
        block(
            vec![
                instruction(OwnedInstruction::OpenCall(CallSiteId(0)), span),
                instruction(
                    OwnedInstruction::PrepareOwned {
                        call: CallSiteId(0),
                        argument: 0,
                        source: OwnerPlaceId(0),
                    },
                    span,
                ),
            ],
            OwnedTerminatorKind::Invoke {
                call: CallSiteId(0),
                continuation: BlockId(1),
            },
            span,
        ),
        block(
            vec![],
            OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(2)),
            span,
        ),
    ];
    raw.functions.push(leaf);
    (sources, raw)
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7; explicitly run the owned native gate"]
fn native_owned_extended_storage_every_budget_uses_real_llvm() {
    let scratch = Scratch::new();
    let mut cases = vec![
        ("shared-children", fixtures::shared_children(false)),
        ("exclusive-restoration", fixtures::shared_children(true)),
        ("mixed-bool", fixtures::mixed_relay(0)),
        ("mixed-unit", fixtures::mixed_relay(1)),
        ("mixed-i32", fixtures::mixed_relay(2)),
        ("owner-loop", fixtures::owner_loop()),
        ("replace-initialized", fixtures::replacement(false)),
        ("replace-moved", fixtures::replacement(true)),
        ("empty-relay", empty_relay()),
        ("distinct-owned-results", fixtures::distinct_owned_results()),
        ("later-argument-loop", fixtures::later_argument_loop()),
        ("empty-adjacent-sentinels", empty_adjacent_sentinels()),
    ];
    let mut executions = 0;
    for (name, (sources, mut raw, schedule)) in cases.drain(..) {
        // Empty/mixed transfers and all acyclic fixtures also get production
        // unguarded coverage before forcing the distinct shared-fuel ABI.
        if name != "owner-loop" && name != "later-argument-loop" {
            let unguarded = verified::verify_owned(
                RawOwnedProgram {
                    records: raw
                        .records
                        .iter()
                        .map(|r| RawRecordDecl {
                            id: r.id,
                            span: r.span,
                            fields: r.fields.clone(),
                        })
                        .collect(),
                    functions: raw.functions.clone(),
                },
                &sources,
            )
            .unwrap();
            let module = native_module(&unguarded, Some(schedule.entry), &sources).unwrap();
            let binary = scratch.compile(&module, &format!("{name}-production"));
            assert_result(
                scratch.run(&binary, &[]),
                &scalar_output(schedule.result),
                b"",
                0,
            );
            executions += 1;
        }
        append_cycle(&mut raw);
        let witness = verified::verify_owned(raw, &sources).unwrap();
        let module =
            native_module_with_fuel(&witness, schedule.entry, &sources, schedule.fuel()).unwrap();
        let binary = scratch.compile(
            &argv_fuel_harness(&module, schedule.fuel()),
            &format!("{name}-all-budgets"),
        );
        for fuel in 0..=schedule.fuel() {
            let reference = execute::run_limits(
                &witness,
                Some(schedule.entry),
                execute::Limits {
                    fuel,
                    ..execute::Limits::default()
                },
            );
            let actual = scratch.run(&binary, &[fuel.to_string()]);
            match schedule.failure(fuel) {
                Some(span) => {
                    let expected = RunFailure::Fuel(span)
                        .diagnostic(&sources)
                        .render_human(&sources);
                    assert_eq!(
                        reference
                            .unwrap_err()
                            .diagnostic(&sources)
                            .render_human(&sources),
                        expected
                    );
                    assert_result(actual, b"", expected.as_bytes(), 1);
                }
                None => {
                    assert_eq!(reference, Ok(schedule.result));
                    assert_result(actual, &scalar_output(schedule.result), b"", 0);
                }
            }
            executions += 1;
        }
    }
    let (sources, mut raw) = nested_relay();
    append_cycle(&mut raw);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    assert_eq!(
        execute::run(&witness, Some(hir::DefId(0))),
        Ok(Scalar::I32(73))
    );
    let module = native_module(&witness, Some(hir::DefId(0)), &sources).unwrap();
    let binary = scratch.compile(&module, "nested-owned-relay");
    assert_result(scratch.run(&binary, &[]), b"73\n", b"", 0);
    eprintln!("extended owned storage: {} source-free ELF executions; mixed/empty records, shared alias/reborrow restoration, replacement, owner reuse and nested result relay", executions + 1);
}

fn expanded_depth_boundary(extra_field: bool) -> (SourceMap, RawOwnedProgram) {
    use fixtures::*;
    let (sources, s) = context();
    let big = record(&[hir::Ty::I32; 256], s(0));
    let mut small = record(&vec![hir::Ty::I32; 2 + usize::from(extra_field)], s(0));
    small.id = RecordId(1);
    for field in &mut small.fields {
        field.id.record = RecordId(1);
    }
    let mut functions = Vec::new();
    for id in 0..32 {
        let mut f = function(id, ValueTy::Scalar(hir::Ty::Unit), s(id));
        f.locals = vec![scalar(hir::Ty::I32, s(0)), scalar(hir::Ty::Unit, s(0))];
        let record = if id == 0 { RecordId(1) } else { RecordId(0) };
        let fields = if id == 0 { small.fields.len() } else { 256 };
        f.owners = vec![OwnerDecl {
            aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Record(record)).unwrap(),
            kind: OwnerKind::Local { mutable: false },
            span: s(0),
        }];
        let mut prefix = vec![
            assign(0, Rvalue::I32(id as i32 + 1), s(0)),
            instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), s(0)),
            instruction(
                OwnedInstruction::Construct {
                    destination: OwnerPlaceId(0),
                    fields: (0..fields)
                        .map(|index| (FieldId { record, index }, operand(0, s(0))))
                        .collect(),
                },
                s(0),
            ),
        ];
        if id == 0 {
            prefix.push(assign(1, Rvalue::Unit, s(0)));
            prefix.push(instruction(
                OwnedInstruction::StorageEnd(OwnerPlaceId(0)),
                s(0),
            ));
            f.blocks.push(block(
                prefix,
                OwnedTerminatorKind::ReturnScalar(operand(1, s(0))),
                s(0),
            ));
        } else {
            f.calls.push(CallDecl {
                target: hir::DefId(id - 1),
                arguments: vec![],
                result: CallResult::Scalar(LocalId(1)),
                parent: None,
                span: s(0),
            });
            prefix.push(instruction(OwnedInstruction::OpenCall(CallSiteId(0)), s(0)));
            f.blocks.push(block(
                prefix,
                OwnedTerminatorKind::Invoke {
                    call: CallSiteId(0),
                    continuation: BlockId(1),
                },
                s(0),
            ));
            f.blocks.push(block(
                vec![instruction(
                    OwnedInstruction::StorageEnd(OwnerPlaceId(0)),
                    s(0),
                )],
                OwnedTerminatorKind::ReturnScalar(operand(1, s(0))),
                s(0),
            ));
        }
        functions.push(f);
    }
    (
        sources,
        RawOwnedProgram {
            records: vec![big, small],
            functions,
        },
    )
}

#[test]
fn native_owned_actual_expanded_cell_boundary_is_inclusive() {
    for extra in [false, true] {
        let (sources, raw) = expanded_depth_boundary(extra);
        let witness = verified::verify_owned(raw, &sources).unwrap();
        let plan = ExecutionPlan::build(&witness).unwrap();
        let actual_cells: usize = plan
            .functions()
            .iter()
            .map(|f| f.usage().expanded_cells)
            .sum();
        assert_eq!(actual_cells, 8_192 + usize::from(extra));
        let result = native_module(&witness, Some(hir::DefId(31)), &sources);
        if extra {
            assert!(result
                .unwrap_err()
                .message
                .contains("aggregate expanded cells"));
        } else {
            let bounds = admit(&plan, Limits::DEFAULT).unwrap();
            assert_eq!(bounds[31].depth, 32);
            assert_eq!(bounds[31].cells, 8_192);
            assert_eq!(bounds[31].bytes, 32_264);
            assert!(result.is_ok());
        }
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7; explicitly run the owned native gate"]
fn native_owned_expanded_boundary_uses_real_llvm_and_measures_stack() {
    let scratch = Scratch::new();
    let (sources, raw) = expanded_depth_boundary(false);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let module = native_module(&witness, Some(hir::DefId(31)), &sources).unwrap();
    let binary = scratch.compile(&module, "expanded-depth32");
    assert_eq!(
        execute::run(&witness, Some(hir::DefId(31))),
        Ok(Scalar::Unit)
    );
    assert_result(scratch.run(&binary, &[]), b"()\n", b"", 0);
    // Measure the same qualified O0 module's backend stack-size section. These
    // bytes include backend spills and must not be confused with Dnative.
    let llvm = std::env::var_os("OXID_LLVM_BIN").map(std::path::PathBuf::from);
    let clang = llvm
        .as_ref()
        .map_or_else(|| "clang-19".into(), |p| p.join("clang"));
    let readobj = llvm
        .as_ref()
        .map_or_else(|| "llvm-readobj-19".into(), |p| p.join("llvm-readobj"));
    let input = scratch.0.join("stack.ll");
    let object = scratch.0.join("stack.o");
    std::fs::write(&input, &module).unwrap();
    let built = std::process::Command::new(clang)
        .args([
            "--no-default-config",
            "--target=x86_64-unknown-linux-gnu",
            "-O0",
            "-fPIE",
            "-fstack-size-section",
            "-x",
            "ir",
            "-c",
        ])
        .arg(&input)
        .arg("-o")
        .arg(&object)
        .env_remove("CCC_OVERRIDE_OPTIONS")
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let measured = std::process::Command::new(readobj)
        .arg("--stack-sizes")
        .arg(&object)
        .output()
        .unwrap();
    assert!(
        measured.status.success(),
        "{}",
        String::from_utf8_lossy(&measured.stderr)
    );
    let report = String::from_utf8(measured.stdout).unwrap();
    let sizes: Vec<usize> = report
        .lines()
        .filter_map(|line| line.trim().strip_prefix("Size: "))
        .map(|v| usize::from_str_radix(v.trim_start_matches("0x"), 16).unwrap())
        .collect();
    assert_eq!(sizes.len(), 33); // 32 noinline owned functions and the C main wrapper.
    assert!(sizes.iter().sum::<usize>() > 32_264);
    if let Some(evidence) = std::env::var_os("OXID_OWNED_NATIVE_EVIDENCE") {
        std::fs::write(
            std::path::PathBuf::from(evidence).join("expanded-depth32-stack.txt"),
            &report,
        )
        .unwrap();
    }
    eprintln!("expanded depth32: admitted X8192, explicit arenas32264 bytes, measured backend frame-size sum{} bytes (plus ABI return addresses/runtime frames; not an RSS bound)", sizes.iter().sum::<usize>());
}

fn empty_adjacent_sentinels() -> (SourceMap, RawOwnedProgram, fixtures::Schedule) {
    use fixtures::*;
    let (sources, s) = context();
    let empty = record(&[], s(0));
    let mut mixed = record(&[hir::Ty::Bool, hir::Ty::I32], s(0));
    mixed.id = RecordId(1);
    for field in &mut mixed.fields {
        field.id.record = RecordId(1);
    }
    let field = |index| FieldId {
        record: RecordId(1),
        index,
    };
    let own = |record, kind| OwnerDecl {
        aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(record)))
            .unwrap(),
        kind,
        span: s(0),
    };
    let mut f = function(0, ValueTy::Scalar(hir::Ty::I32), s(0));
    f.locals = [
        hir::Ty::Bool,
        hir::Ty::I32,
        hir::Ty::Bool,
        hir::Ty::I32,
        hir::Ty::I32,
        hir::Ty::Bool,
        hir::Ty::I32,
        hir::Ty::Bool,
        hir::Ty::I32,
        hir::Ty::I32,
    ]
    .into_iter()
    .map(|ty| scalar(ty, s(0)))
    .collect();
    f.owners = vec![
        own(1, OwnerKind::Local { mutable: false }),
        own(0, OwnerKind::Local { mutable: false }),
        own(
            0,
            OwnerKind::StagedArgument {
                call: CallSiteId(0),
                argument: 0,
            },
        ),
        own(
            0,
            OwnerKind::CallResult {
                call: CallSiteId(0),
            },
        ),
        own(1, OwnerKind::Local { mutable: false }),
        own(
            1,
            OwnerKind::StagedArgument {
                call: CallSiteId(1),
                argument: 0,
            },
        ),
        own(
            1,
            OwnerKind::CallResult {
                call: CallSiteId(1),
            },
        ),
    ];
    f.calls = vec![
        CallDecl {
            target: hir::DefId(1),
            arguments: vec![ArgumentSlot::Owned(OwnerPlaceId(2))],
            result: CallResult::Owned(OwnerPlaceId(3)),
            parent: None,
            span: s(12),
        },
        CallDecl {
            target: hir::DefId(2),
            arguments: vec![ArgumentSlot::Owned(OwnerPlaceId(5))],
            result: CallResult::Owned(OwnerPlaceId(6)),
            parent: None,
            span: s(16),
        },
    ];
    f.blocks.push(block(
        vec![
            assign(0, Rvalue::Bool(true), s(1)),
            assign(1, Rvalue::I32(0x11223344), s(2)),
            assign(2, Rvalue::Bool(false), s(3)),
            assign(3, Rvalue::I32(-12345), s(4)),
            assign(4, Rvalue::I32(-1), s(5)),
            instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), s(6)),
            instruction(
                OwnedInstruction::Construct {
                    destination: OwnerPlaceId(0),
                    fields: vec![(field(0), operand(0, s(7))), (field(1), operand(1, s(7)))],
                },
                s(7),
            ),
            instruction(OwnedInstruction::StorageLive(OwnerPlaceId(1)), s(8)),
            instruction(
                OwnedInstruction::Construct {
                    destination: OwnerPlaceId(1),
                    fields: vec![],
                },
                s(9),
            ),
            instruction(OwnedInstruction::StorageLive(OwnerPlaceId(4)), s(10)),
            instruction(
                OwnedInstruction::Construct {
                    destination: OwnerPlaceId(4),
                    fields: vec![(field(1), operand(3, s(11))), (field(0), operand(2, s(11)))],
                },
                s(11),
            ),
            instruction(OwnedInstruction::OpenCall(CallSiteId(0)), s(12)),
            instruction(
                OwnedInstruction::PrepareOwned {
                    call: CallSiteId(0),
                    argument: 0,
                    source: OwnerPlaceId(1),
                },
                s(13),
            ),
        ],
        OwnedTerminatorKind::Invoke {
            call: CallSiteId(0),
            continuation: BlockId(1),
        },
        s(14),
    ));
    f.blocks.push(block(
        vec![
            instruction(OwnedInstruction::OpenCall(CallSiteId(1)), s(16)),
            instruction(
                OwnedInstruction::PrepareOwned {
                    call: CallSiteId(1),
                    argument: 0,
                    source: OwnerPlaceId(4),
                },
                s(17),
            ),
        ],
        OwnedTerminatorKind::Invoke {
            call: CallSiteId(1),
            continuation: BlockId(2),
        },
        s(18),
    ));
    let mut after = vec![];
    for (destination, owner, field_index, label) in
        [(5, 0, 0, 20), (6, 0, 1, 21), (7, 6, 0, 22), (8, 6, 1, 23)]
    {
        after.push(instruction(
            OwnedInstruction::ReadField {
                destination: LocalId(destination),
                base: AccessBase::Owner(OwnerPlaceId(owner)),
                field: field(field_index),
            },
            s(label),
        ));
    }
    after.push(assign(
        9,
        Rvalue::CheckedI32 {
            op: hir::ArithmeticOp::Add,
            left: operand(6, s(24)),
            right: operand(8, s(24)),
            operator_span: s(24),
        },
        s(24),
    ));
    for (kind, label) in [
        (OwnedInstruction::Discard(OwnerPlaceId(3)), 25),
        (OwnedInstruction::StorageEnd(OwnerPlaceId(1)), 26),
        (OwnedInstruction::StorageEnd(OwnerPlaceId(3)), 27),
        (OwnedInstruction::Discard(OwnerPlaceId(6)), 28),
        (OwnedInstruction::StorageEnd(OwnerPlaceId(4)), 29),
        (OwnedInstruction::StorageEnd(OwnerPlaceId(6)), 30),
        (OwnedInstruction::StorageEnd(OwnerPlaceId(0)), 31),
    ] {
        after.push(instruction(kind, s(label)));
    }
    f.blocks.push(block(
        after,
        OwnedTerminatorKind::Branch {
            condition: operand(5, s(32)),
            then_block: BlockId(3),
            else_block: BlockId(5),
        },
        s(32),
    ));
    f.blocks.push(block(
        vec![],
        OwnedTerminatorKind::Branch {
            condition: operand(7, s(33)),
            then_block: BlockId(5),
            else_block: BlockId(4),
        },
        s(33),
    ));
    f.blocks.push(block(
        vec![],
        OwnedTerminatorKind::ReturnScalar(operand(9, s(34))),
        s(34),
    ));
    f.blocks.push(block(
        vec![],
        OwnedTerminatorKind::ReturnScalar(operand(4, s(35))),
        s(35),
    ));
    let mut functions = vec![f];
    for (id, record, return_label) in [(1, 0, 15), (2, 1, 19)] {
        let mut f = function(
            id,
            ValueTy::Owned(AggregateTy::Record(RecordId(record))),
            s(40 + id),
        );
        f.parameters.push(ParameterBinding::Owned(OwnerPlaceId(0)));
        f.owners
            .push(own(record, OwnerKind::Parameter { position: 0 }));
        f.blocks.push(block(
            vec![],
            OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0)),
            s(return_label),
        ));
        functions.push(f);
    }
    // Independent hand trace: owner offsets 0,8,9,10,12,20,28; main
    // S10+A2+P11+4*O7+2*C2=X55. Empty/mixed child X5/X6 respectively.
    let costs = [
        56, 1, 1, 1, 1, 1, 1, 3, 1, 2, 1, 3, 2, 2, 8, 3, 2, 3, 10, 5, 1, 1, 1, 1, 1, 2, 2, 2, 3, 3,
        3, 3, 1, 1, 14,
    ];
    assert_eq!(costs.iter().sum::<usize>(), 146);
    (
        sources,
        RawOwnedProgram {
            records: vec![empty, mixed],
            functions,
        },
        Schedule {
            entry: hir::DefId(0),
            result: Scalar::I32(0x11223344 - 12345),
            events: costs
                .into_iter()
                .enumerate()
                .map(|(i, cost)| (s(i), cost))
                .collect(),
        },
    )
}

#[test]
fn native_owned_empty_transfers_preserve_adjacent_live_sentinels_and_padding() {
    let (sources, raw, schedule) = empty_adjacent_sentinels();
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    let fp = plan.function(hir::DefId(0));
    assert_eq!(fp.usage().payload_bytes, 36);
    assert_eq!(fp.usage().native_bytes, 132);
    for (owner, offset) in [0, 8, 9, 10, 12, 20, 28].into_iter().enumerate() {
        assert_eq!(fp.owner_offset(OwnerPlaceId(owner)), offset);
    }
    assert_eq!(
        execute::run(&witness, Some(schedule.entry)),
        Ok(schedule.result)
    );
    for fuel in 0..schedule.fuel() {
        let error = execute::run_limits(
            &witness,
            Some(schedule.entry),
            execute::Limits {
                fuel,
                ..execute::Limits::default()
            },
        )
        .unwrap_err();
        assert_eq!(
            error.diagnostic(&sources).render_human(&sources),
            RunFailure::Fuel(schedule.failure(fuel).unwrap())
                .diagnostic(&sources)
                .render_human(&sources)
        );
    }
    let module = native_module(&witness, Some(schedule.entry), &sources).unwrap();
    assert!(module.contains("%owners = alloca [36 x i8], align 4"));
    assert!(module.contains("store i8 0, ptr %o1, align 1"));
    assert!(module.contains("store i8 %f1_b0_term_empty, ptr %result, align 1"));
    assert!(!module.contains("memcpy"));
}

#[test]
fn native_owned_acyclic_overflow_data_and_embedded_scalar_origins_are_bounded() {
    let (sources, mut raw) = overflow_before_field(true);
    raw.functions.pop(); // The production acyclic route still has data/text caps.
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    let diagnostics =
        Diagnostics::new(&plan, hir::DefId(0), &sources, false, MAX_DIAGNOSTIC_BYTES).unwrap();
    let bytes: usize = diagnostics.messages.iter().map(String::len).sum();
    assert!(bytes > 0);
    assert!(native_module_limits(
        &witness,
        Some(hir::DefId(0)),
        &sources,
        0,
        Limits {
            diagnostic_bytes: bytes,
            ..Limits::DEFAULT
        }
    )
    .is_ok());
    assert!(native_module_limits(
        &witness,
        Some(hir::DefId(0)),
        &sources,
        0,
        Limits {
            diagnostic_bytes: bytes - 1,
            ..Limits::DEFAULT
        }
    )
    .unwrap_err()
    .message
    .contains("diagnostic bytes"));
    let (sources, mut raw, schedule) = fixtures::empty_record();
    let statement = &mut raw.functions[0].blocks[0].statements[4];
    let embedded = plan::instruction_span(statement);
    statement.span.start += 100;
    statement.span.end += 100;
    let outer = statement.span;
    append_cycle(&mut raw);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    let diagnostics =
        Diagnostics::new(&plan, schedule.entry, &sources, true, MAX_DIAGNOSTIC_BYTES).unwrap();
    assert!(diagnostics.contains(FailureKind::Fuel, embedded));
    assert!(!diagnostics.contains(FailureKind::Fuel, outer));
    let module = native_module_with_fuel(&witness, schedule.entry, &sources, 14).unwrap();
    let (id, len) = diagnostics.get(FailureKind::Fuel, embedded);
    assert!(module.contains(&format!("f0_b0_g5_error:\n  call void @__oxid_overflow(ptr @__oxid_owned_error_{id}, i64 {len})\n  unreachable")));
}

#[test]
fn native_owned_actual_representation_caps_precede_text_allocation() {
    use fixtures::*;
    // An acyclic module with many distinct overflow origins reaches the real
    // 16 MiB data cap despite satisfying every function/cost/storage cap.
    let mut sources = SourceMap::new();
    let file = sources.add("x".repeat(100_000), "x\n".repeat(300));
    let s = |i| Span {
        file,
        start: 2 * i,
        end: 2 * i + 1,
    };
    let mut f = function(0, ValueTy::Scalar(hir::Ty::I32), s(0));
    f.locals = (0..202).map(|_| scalar(hir::Ty::I32, s(0))).collect();
    let mut statements = vec![
        assign(0, Rvalue::I32(1), s(0)),
        assign(1, Rvalue::I32(2), s(1)),
    ];
    for i in 2..202 {
        statements.push(assign(
            i,
            Rvalue::CheckedI32 {
                op: hir::ArithmeticOp::Add,
                left: operand(0, s(i)),
                right: operand(1, s(i)),
                operator_span: s(i),
            },
            s(i),
        ));
    }
    f.blocks.push(block(
        statements,
        OwnedTerminatorKind::ReturnScalar(operand(201, s(202))),
        s(202),
    ));
    let witness = verified::verify_owned(
        RawOwnedProgram {
            records: vec![],
            functions: vec![f],
        },
        &sources,
    )
    .unwrap();
    let error = native_module(&witness, Some(hir::DefId(0)), &sources).unwrap_err();
    assert_eq!(error.code, "E0700");
    assert!(error.message.contains("diagnostic bytes"));

    // A cyclic move/replace body has small fixed storage and metadata but large
    // field-wise lowering. Counting must reject its actual >64 MiB LLVM text.
    let (sources, s) = context();
    let mut f = function(0, ValueTy::Scalar(hir::Ty::Unit), s(0));
    f.locals = vec![scalar(hir::Ty::I32, s(0))];
    f.owners = vec![owner(OwnerKind::Local { mutable: true }, s(0)); 2];
    let mut initial = vec![assign(0, Rvalue::I32(1), s(0))];
    for id in 0..2 {
        initial.push(instruction(
            OwnedInstruction::StorageLive(OwnerPlaceId(id)),
            s(0),
        ));
        initial.push(instruction(
            OwnedInstruction::Construct {
                destination: OwnerPlaceId(id),
                fields: (0..1024)
                    .map(|index| {
                        (
                            FieldId {
                                record: RecordId(0),
                                index,
                            },
                            operand(0, s(0)),
                        )
                    })
                    .collect(),
            },
            s(0),
        ));
    }
    f.blocks
        .push(block(initial, OwnedTerminatorKind::Goto(BlockId(1)), s(0)));
    f.blocks.push(block(
        (0..512)
            .map(|i| {
                instruction(
                    OwnedInstruction::Replace {
                        destination: OwnerPlaceId((i + 1) % 2),
                        source: OwnerPlaceId(i % 2),
                    },
                    s(1),
                )
            })
            .collect(),
        OwnedTerminatorKind::Goto(BlockId(1)),
        s(1),
    ));
    let witness = verified::verify_owned(
        RawOwnedProgram {
            records: vec![record(&[hir::Ty::I32; 1024], s(0))],
            functions: vec![f],
        },
        &sources,
    )
    .unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    assert!(admit(&plan, Limits::DEFAULT).is_ok());
    let error = native_module(&witness, Some(hir::DefId(0)), &sources).unwrap_err();
    assert_eq!(error.code, "E0700");
    assert!(error.message.contains("LLVM bytes"));
}

fn verify_batch_write_guards(module: &str) {
    let cfg = LlvmCfg::parse(module, "__oxid_owned_fn_2");
    let completed = "store i32 %f2_b0_i3_value, ptr %f2_b0_i3_ptr, align 1";
    cfg.assert_success_dominates("f2_b0_g4_ok", "f2_b0_g4_error", completed);
    cfg.assert_success_dominates("f2_b0_i2_checked_ok", "f2_b0_i2_checked_error", completed);
    let checksum = "store i32 %f2_b0_i8_value, ptr %f2_b0_i8_ptr, align 1";
    cfg.assert_success_dominates("f2_b0_g9_ok", "f2_b0_g9_error", checksum);
    cfg.assert_success_dominates("f2_b0_i7_checked_ok", "f2_b0_i7_checked_error", checksum);
    cfg.assert_success_dominates("f2_b0_i6_checked_ok", "f2_b0_i6_checked_error", checksum);
    // The earlier successful completed-field store is on every path to the
    // checksum overflow. The failure cannot reach the later checksum store.
    assert!(!cfg
        .reaching("entry", Some(cfg.store_block(completed)))
        .contains("f2_b0_i7_checked_error"));
}

#[test]
fn native_owned_batch_guard_cfg_preserves_earlier_successful_write() {
    let (sources, raw, _, _) = super::super::consumer_pilot::overflow_after_write();
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let module = native_module(&witness, Some(hir::DefId(0)), &sources).unwrap();
    verify_batch_write_guards(&module);
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7; explicitly run the owned native gate"]
fn native_owned_batch_write_failure_every_budget_uses_real_llvm() {
    let scratch = Scratch::new();
    let (sources, raw, overflow, charges) = super::super::consumer_pilot::overflow_after_write();
    assert_eq!(charges.iter().map(|(_, cost)| cost).sum::<usize>(), 235);
    let expected_failure = |mut fuel: usize| {
        for &(span, cost) in &charges {
            if fuel < cost {
                return RunFailure::Fuel(span);
            }
            fuel -= cost;
        }
        RunFailure::Overflow(overflow)
    };
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let module = native_module_with_fuel(&witness, hir::DefId(0), &sources, 235).unwrap();
    verify_batch_write_guards(&module);
    let binary = scratch.compile(
        &argv_fuel_harness(&module, 235),
        "batch-write-failure-all-budgets",
    );
    for fuel in 0..=235 {
        let expected = expected_failure(fuel)
            .diagnostic(&sources)
            .render_human(&sources);
        let reference = execute::run_limits(
            &witness,
            Some(hir::DefId(0)),
            execute::Limits {
                fuel,
                ..execute::Limits::default()
            },
        )
        .unwrap_err();
        assert_eq!(
            reference.diagnostic(&sources).render_human(&sources),
            expected
        );
        assert_result(
            scratch.run(&binary, &[fuel.to_string()]),
            b"",
            expected.as_bytes(),
            1,
        );
    }
    for fuel in [230, 231, 234, 235, plan::MAX_FUEL] {
        let module = native_module_with_fuel(&witness, hir::DefId(0), &sources, fuel).unwrap();
        verify_batch_write_guards(&module);
        let binary = scratch.compile(&module, &format!("batch-write-failure-production-{fuel}"));
        let expected = expected_failure(fuel)
            .diagnostic(&sources)
            .render_human(&sources);
        assert_result(scratch.run(&binary, &[]), b"", expected.as_bytes(), 1);
    }
    eprintln!("Batch earlier-write/overflow:236 argv-harness budgets +5 production-wrapper failures; independent LLVM guard/store dominance");
}

fn wide_replacement_count_fixture(replacements: usize) -> (SourceMap, VerifiedOwnedProgram) {
    use fixtures::*;
    let (sources, s) = context();
    let mut f = function(0, ValueTy::Scalar(hir::Ty::Unit), s(0));
    f.locals = vec![scalar(hir::Ty::I32, s(0))];
    f.owners = vec![owner(OwnerKind::Local { mutable: true }, s(0)); 2];
    let mut statements = vec![assign(0, Rvalue::I32(1), s(0))];
    for owner in 0..2 {
        statements.push(instruction(
            OwnedInstruction::StorageLive(OwnerPlaceId(owner)),
            s(0),
        ));
        statements.push(instruction(
            OwnedInstruction::Construct {
                destination: OwnerPlaceId(owner),
                fields: (0..64)
                    .map(|index| {
                        (
                            FieldId {
                                record: RecordId(0),
                                index,
                            },
                            operand(0, s(0)),
                        )
                    })
                    .collect(),
            },
            s(0),
        ));
    }
    f.blocks.push(block(
        statements,
        OwnedTerminatorKind::Goto(BlockId(1)),
        s(0),
    ));
    f.blocks.push(block(
        (0..replacements)
            .map(|i| {
                instruction(
                    OwnedInstruction::Replace {
                        destination: OwnerPlaceId((i + 1) % 2),
                        source: OwnerPlaceId(i % 2),
                    },
                    s(1),
                )
            })
            .collect(),
        OwnedTerminatorKind::Goto(BlockId(1)),
        s(1),
    ));
    let witness = verified::verify_owned(
        RawOwnedProgram {
            records: vec![record(&[hir::Ty::I32; 64], s(0))],
            functions: vec![f],
        },
        &sources,
    )
    .unwrap();
    (sources, witness)
}

#[test]
fn native_owned_count_pass_stops_wide_expansion_at_the_byte_cap() {
    let mut observations = vec![];
    for replacements in [32, 32_768] {
        let (sources, witness) = wide_replacement_count_fixture(replacements);
        let plan = ExecutionPlan::build(&witness).unwrap();
        assert!(admit(&plan, Limits::DEFAULT).is_ok());
        let diagnostics =
            Diagnostics::new(&plan, hir::DefId(0), &sources, true, MAX_DIAGNOSTIC_BYTES).unwrap();
        let mut visits = vec![];
        for cap in [1_024, 4_096, 65_536] {
            let mut count = Emission::count(cap);
            emit(
                &plan,
                hir::DefId(0),
                &diagnostics,
                true,
                plan::MAX_FUEL,
                &mut count,
            );
            assert!(count.exceeded);
            assert!(count.text.is_none());
            assert!(count.len <= cap);
            assert!(
                count.field_visits < 512,
                "lowered {cap} cap visited {} fields",
                count.field_visits
            );
            if cap == 65_536 {
                assert!(
                    count.field_visits > 128,
                    "must reach the alternating replacements"
                );
            }
            visits.push((count.len, count.field_visits));
            let error = native_module_limits(
                &witness,
                Some(hir::DefId(0)),
                &sources,
                plan::MAX_FUEL,
                Limits {
                    ir_bytes: cap,
                    ..Limits::DEFAULT
                },
            )
            .unwrap_err();
            assert_eq!(error.code, "E0700");
            assert!(error.message.contains("LLVM bytes"));
        }
        observations.push(visits);
    }
    // Adding 32,736 raw replacements must not add any field-expansion work
    // after a fixed prefix has already exhausted the emitted-byte budget.
    assert_eq!(observations[0], observations[1]);
    eprintln!("fail-fast field visits at1024/4096/65536 bytes: {:?}; identical for32 and32768 replacements", observations[0]);
    let mut overflow = Emission::count(usize::MAX);
    overflow.len = usize::MAX;
    overflow.write_str("x").unwrap();
    assert!(overflow.exceeded);
    assert_eq!(overflow.len, usize::MAX);
    let mut exact = Emission::count(3);
    exact.write_str("abc").unwrap();
    assert!(!exact.exceeded);
    exact.write_str("d").unwrap();
    assert!(exact.exceeded);
    assert_eq!(exact.len, 3);
}

#[path = "native_heldout_review.rs"]
mod heldout_review;

#[path = "source/reviewer_resource_native.rs"]
mod reviewer_resources;

#[path = "array_native_resource_tests.rs"]
mod array_resources;
