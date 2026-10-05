//! Explicitly invoked, test-only source/native receipts. This collector accepts
//! source bytes and budget controls, never model expectations. Instrumented
//! store probes are separate artifacts, not unchanged production emissions.
use super::*;
use std::{io::Write, path::PathBuf, process::Command};

const PROBE_MARKER: &str = "; UNIT4B_PROBE";
const STORE_PREFIX: &str = "__UNIT4B_STORE ";

fn fields(values: Vec<(&str, String)>) -> String {
    format!(
        "{{{}}}",
        values
            .into_iter()
            .map(|(k, v)| format!("{}:{v}", json_string(k)))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn owner(key: storage::OwnerKey) -> String {
    object([
        ("frame", key.frame.to_string()),
        ("activation", key.activation.to_string()),
        ("owner", key.owner.to_string()),
        ("generation", key.generation.to_string()),
    ])
}

fn loan(key: storage::LoanKey) -> String {
    object([
        ("frame", key.frame.to_string()),
        ("activation", key.activation.to_string()),
        ("loan", key.loan.to_string()),
        ("instance", key.instance.to_string()),
    ])
}

fn scalar(value: Scalar) -> String {
    match value {
        Scalar::I32(v) => object([("type", json_string("i32")), ("value", v.to_string())]),
        Scalar::Bool(v) => object([("type", json_string("bool")), ("value", v.to_string())]),
        Scalar::Unit => object([("type", json_string("()")), ("value", "null".into())]),
    }
}

fn event_receipts(events: &[execute::Event]) -> String {
    let mut last_charge = None;
    array(events.iter().enumerate().map(|(index, event)| {
        let mut values = vec![("index", index.to_string())];
        match event {
            execute::Event::EnumTagRead(..)
            | execute::Event::EnumPayloadRead(..)
            | execute::Event::EnumBind(..) => {
                unreachable!("enum events are outside this predecessor qualification schema")
            }
            execute::Event::ReadIndex(..)
            | execute::Event::WriteIndex(..)
            | execute::Event::ArrayLength(..) => {
                unreachable!("source arrays and production array witnesses remain gated")
            }
            execute::Event::Charge(at, cost) => {
                last_charge = Some(*at);
                values.extend([
                    ("kind", json_string("Charge")),
                    ("span", span(*at)),
                    ("cost", cost.to_string()),
                ]);
            }
            execute::Event::Enter(function, activation) => values.extend([
                ("kind", json_string("Enter")),
                ("function", function.0.to_string()),
                ("activation", activation.to_string()),
            ]),
            execute::Event::WriteField(key, field, value) => values.extend([
                ("kind", json_string("WriteField")),
                ("owner", owner(*key)),
                ("field", field_id(*field)),
                ("value", scalar(*value)),
            ]),
            execute::Event::Transfer(from, to) => values.extend([
                ("kind", json_string("Transfer")),
                ("from", owner(*from)),
                ("to", owner(*to)),
            ]),
            execute::Event::Acquire(key, target, kind) => values.extend([
                ("kind", json_string("Acquire")),
                ("loan", loan(*key)),
                ("owner", owner(*target)),
                ("mode", borrow_kind(*kind)),
            ]),
            execute::Event::Release(key) => {
                values.extend([("kind", json_string("Release")), ("loan", loan(*key))])
            }
            execute::Event::Return(function) => values.extend([
                ("kind", json_string("Return")),
                ("function", function.0.to_string()),
            ]),
        }
        values.push(("last_successful_charge_span", optional_span(last_charge)));
        fields(values)
    }))
}

// Matches the reviewed native_tests argv harness. No function-body byte changes.
fn argv_fuel_harness(module: &str, budget: usize) -> String {
    let old_header = "define i32 @main() {\nentry:\n";
    let new_header = "declare i64 @strtoull(ptr, ptr, i32)\ndefine i32 @main(i32 %argc, ptr %argv) {\nentry:\n  %test_arg_slot = getelementptr ptr, ptr %argv, i64 1\n  %test_arg = load ptr, ptr %test_arg_slot\n  %test_parsed_fuel = call i64 @strtoull(ptr %test_arg, ptr null, i32 10)\n  %test_over_cap = icmp ugt i64 %test_parsed_fuel, 1000000\n  %test_fuel = select i1 %test_over_cap, i64 1000000, i64 %test_parsed_fuel\n";
    let old_store = format!("store i64 {budget}, ptr %fuel, align 8");
    assert_eq!(module.matches(old_header).count(), 1);
    assert_eq!(
        module.matches(&old_store).count(),
        1,
        "source must itself select guarded ABI"
    );
    let result = module.replacen(old_header, new_header, 1).replacen(
        &old_store,
        "store i64 %test_fuel, ptr %fuel, align 8",
        1,
    );
    assert!(result.starts_with(module.split_once(old_header).unwrap().0));
    assert_eq!(
        result.replacen(new_header, old_header, 1).replacen(
            "store i64 %test_fuel, ptr %fuel, align 8",
            &old_store,
            1
        ),
        module
    );
    result
}

fn kind(statement: &OwnedInstruction) -> &'static str {
    match statement {
        OwnedInstruction::ConstructEnum { .. } | OwnedInstruction::ConsumeVariant { .. } => {
            unreachable!("enum source gate")
        }
        OwnedInstruction::ConstructComposite { .. } => "ConstructComposite",
        OwnedInstruction::ReadProjection { .. } => "ReadProjection",
        OwnedInstruction::WriteProjection { .. } => "WriteProjection",
        OwnedInstruction::ProjectionLength { .. } => "ProjectionLength",
        OwnedInstruction::ConstructArray { .. }
        | OwnedInstruction::ReadIndex { .. }
        | OwnedInstruction::WriteIndex { .. }
        | OwnedInstruction::ArrayLength { .. } => {
            unreachable!("source array production and executable witnesses remain gated")
        }
        OwnedInstruction::Scalar(Statement::Assign(_)) => "Scalar(Assign)",
        OwnedInstruction::Scalar(Statement::Initialize { .. }) => "Scalar(Initialize)",
        OwnedInstruction::Scalar(Statement::Store { .. }) => "Scalar(Store)",
        OwnedInstruction::Construct { .. } => "Construct",
        OwnedInstruction::MoveInitialize { .. } => "MoveInitialize",
        OwnedInstruction::Replace { .. } => "Replace",
        OwnedInstruction::ReadField { .. } => "ReadField",
        OwnedInstruction::WriteField { .. } => "WriteField",
        OwnedInstruction::PrepareScalar { .. } => "PrepareScalar",
        OwnedInstruction::PrepareOwned { .. } => "PrepareOwned",
        OwnedInstruction::PrepareBorrow { .. } => "PrepareBorrow",
        OwnedInstruction::StorageLive(_) => "StorageLive",
        OwnedInstruction::StorageEnd(_) => "StorageEnd",
        OwnedInstruction::Discard(_) => "Discard",
        OwnedInstruction::OpenCall(_) => "OpenCall",
    }
}

fn operations(program: &SourceProgram, entry: hir::DefId) -> String {
    let plan = plan::ExecutionPlan::build(&program.witness).unwrap();
    let mut values = vec![object([
        ("function", "null".into()),
        ("block", "null".into()),
        ("position", "null".into()),
        ("kind", json_string("RootAdmission")),
        ("guard", json_string("root_ok")),
        (
            "charge_span",
            span(program.witness.functions()[entry.0].span),
        ),
        (
            "cost",
            (1 + plan.function(entry).usage().expanded_cells).to_string(),
        ),
        ("instruction", object([("target", entry.0.to_string())])),
    ])];
    for function in program.witness.functions() {
        for (block_id, block) in function.blocks.iter().enumerate() {
            if let Some(merge) = &block.merge {
                values.push(object([
                    ("function", function.id.0.to_string()),
                    ("block", block_id.to_string()),
                    ("position", "null".into()),
                    ("kind", json_string("BoolMerge")),
                    (
                        "guard",
                        json_string(&format!("f{}_b{block_id}_g0_ok", function.id.0)),
                    ),
                    ("charge_span", span(merge.span)),
                    ("cost", "1".into()),
                    ("instruction", "null".into()),
                ]));
            }
            for (position, statement) in block.statements.iter().enumerate() {
                values.push(object([
                    ("function", function.id.0.to_string()),
                    ("block", block_id.to_string()),
                    ("position", position.to_string()),
                    ("kind", json_string(kind(&statement.kind))),
                    (
                        "guard",
                        json_string(&format!(
                            "f{}_b{block_id}_g{}_ok",
                            function.id.0,
                            position + 1
                        )),
                    ),
                    ("charge_span", span(plan::instruction_span(statement))),
                    (
                        "cost",
                        plan.statement_cost(function.id, &statement.kind)
                            .to_string(),
                    ),
                    ("instruction", instruction(&statement.kind)),
                ]));
            }
            let term = block.terminator.as_ref().unwrap();
            values.push(object([
                ("function", function.id.0.to_string()),
                ("block", block_id.to_string()),
                ("position", block.statements.len().to_string()),
                (
                    "kind",
                    json_string(match term.kind {
                        OwnedTerminatorKind::MatchDispatch { .. } => {
                            unreachable!("enum source gate")
                        }
                        OwnedTerminatorKind::ReturnOwned(_) => "ReturnOwned",
                        OwnedTerminatorKind::ReturnScalar(_) => "ReturnScalar",
                        OwnedTerminatorKind::Invoke { .. } => "Invoke",
                        OwnedTerminatorKind::Goto(_) => "Goto",
                        OwnedTerminatorKind::Branch { .. } => "Branch",
                    }),
                ),
                (
                    "guard",
                    json_string(&format!(
                        "f{}_b{block_id}_g{}_ok",
                        function.id.0,
                        block.statements.len() + 1
                    )),
                ),
                ("charge_span", span(term.span)),
                (
                    "cost",
                    plan.terminator_cost(function.id, &term.kind).to_string(),
                ),
                ("instruction", terminator(&term.kind)),
            ]));
        }
    }
    array(values)
}

fn strip_probe(module: &str) -> String {
    module
        .split_inclusive('\n')
        .filter(|line| !line.trim_end().ends_with(PROBE_MARKER))
        .collect()
}

/// Every original owned-function store is catalogued by its original LLVM line.
/// Fuel is separately identified; all other stores receive an observation site.
/// The independently audited witness/guard join is descriptive, not a proof.
fn probe_module(module: &str, program: &SourceProgram) -> (String, String) {
    assert!(!module.contains(PROBE_MARKER));
    let mut output = String::new();
    let mut sites = Vec::new();
    let (mut function, mut block, mut guard) = (None, String::new(), String::new());
    let mut ordinal = 0usize;
    for (line_index, line) in module.split_inclusive('\n').enumerate() {
        output.push_str(line);
        let trimmed = line.trim();
        if trimmed.starts_with("define internal ") && trimmed.contains("@__oxid_owned_fn_") {
            function = Some(
                trimmed
                    .split_once("@__oxid_owned_fn_")
                    .unwrap()
                    .1
                    .split_once('(')
                    .unwrap()
                    .0
                    .parse::<usize>()
                    .unwrap(),
            );
            ordinal = 0;
            guard.clear();
        }
        if trimmed == "}" {
            function = None;
        }
        let Some(function_id) = function else {
            continue;
        };
        if let Some(label) = trimmed.strip_suffix(':') {
            block = label.into();
            if label.starts_with('b') || label == "entry" {
                guard.clear();
            }
            if label.contains("_g") && label.ends_with("_ok") {
                guard = label.into();
            }
        }
        let Some(store) = trimmed.strip_prefix("store ") else {
            continue;
        };
        let (value, target) = store
            .split_once(", ptr ")
            .expect("known emitted store syntax");
        let (ty, _) = value.split_once(' ').unwrap();
        let pointer = target.split(',').next().unwrap();
        ordinal += 1;
        if pointer == "%fuel" {
            continue;
        }
        let site = sites.len();
        let f = &program.witness.functions()[function_id];
        let (op_kind, at, position, raw_block, raw_kind) = if guard.is_empty() {
            assert_eq!(
                block, "entry",
                "every body store must be in a guarded operation"
            );
            let suffix = trimmed
                .split_once("_param")
                .map(|value| value.1)
                .or_else(|| trimmed.split_once("%arg").map(|value| value.1))
                .expect("parameter store carries an argument identity");
            let index = suffix
                .chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect::<String>()
                .parse::<usize>()
                .unwrap();
            let at = match f.parameters[index] {
                ParameterBinding::Scalar(local) => f.locals[local.0].span,
                ParameterBinding::Owned(owner) => f.owners[owner.0].span,
                ParameterBinding::Reference(reference) => f.references[reference.0].span,
            };
            ("ParameterCopy", at, Some(index), None, "parameter")
        } else {
            let (_, rest) = guard.split_once("_b").unwrap();
            let (b, g) = rest.split_once("_g").unwrap();
            let b = b.parse::<usize>().unwrap();
            let g = g.strip_suffix("_ok").unwrap().parse::<usize>().unwrap();
            let body = &f.blocks[b];
            if g == 0 {
                (
                    "BoolMerge",
                    body.merge.as_ref().unwrap().span,
                    None,
                    Some(b),
                    "merge",
                )
            } else if g <= body.statements.len() {
                let statement = &body.statements[g - 1];
                (
                    kind(&statement.kind),
                    plan::instruction_span(statement),
                    Some(g - 1),
                    Some(b),
                    "statement",
                )
            } else {
                assert_eq!(g, body.statements.len() + 1);
                let term = body.terminator.as_ref().unwrap();
                (
                    match term.kind {
                        OwnedTerminatorKind::ReturnOwned(_) => "ReturnOwned",
                        OwnedTerminatorKind::Invoke { .. } => "Invoke",
                        _ => panic!("unexpected storing terminator"),
                    },
                    term.span,
                    Some(body.statements.len()),
                    Some(b),
                    "terminator",
                )
            }
        };
        sites.push(object([
            ("site", site.to_string()),
            ("function", function_id.to_string()),
            ("llvm_line", (line_index + 1).to_string()),
            ("store_ordinal", ordinal.to_string()),
            ("llvm_block", json_string(&block)),
            ("store", json_string(trimmed)),
            ("store_text", json_string(trimmed)),
            ("type", json_string(ty)),
            ("pointer", json_string(pointer)),
            (
                "guard",
                if guard.is_empty() {
                    "null".into()
                } else {
                    json_string(&guard)
                },
            ),
            ("kind", json_string(op_kind)),
            ("class", json_string(op_kind)),
            (
                "raw_block",
                raw_block.map_or_else(|| "null".into(), |v| v.to_string()),
            ),
            (
                "raw_operation",
                object([
                    ("kind", json_string(raw_kind)),
                    (
                        "index",
                        position.map_or_else(|| "null".into(), |v| v.to_string()),
                    ),
                ]),
            ),
            ("charge_span", span(at)),
            (
                "position",
                position.map_or_else(|| "null".into(), |v| v.to_string()),
            ),
            (
                "value_semantics",
                json_string(if ty == "ptr" {
                    "occurrence-only"
                } else {
                    "unsigned-stored-bits"
                }),
            ),
        ]));
        let name = format!("unit4b_probe_{site}");
        let observed = if ty == "ptr" {
            "0".to_string()
        } else {
            output.push_str(&format!(
                "  %{name}_load = load {ty}, ptr {pointer}, align 1 {PROBE_MARKER}\n"
            ));
            if ty == "i64" {
                format!("%{name}_load")
            } else {
                assert!(matches!(ty, "i1" | "i8" | "i32"));
                output.push_str(&format!(
                    "  %{name}_wide = zext {ty} %{name}_load to i64 {PROBE_MARKER}\n"
                ));
                format!("%{name}_wide")
            }
        };
        output.push_str(&format!("  %{name}_status = call i32 (i32, ptr, ...) @dprintf(i32 2, ptr @__unit4b_store_format, i64 {site}, i64 {observed}) {PROBE_MARKER}\n"));
    }
    let format = b"__UNIT4B_STORE %llu %llu\n\0";
    let encoded = format
        .iter()
        .map(|byte| format!("\\{byte:02X}"))
        .collect::<String>();
    output.push_str(&format!("declare i32 @dprintf(i32, ptr, ...) {PROBE_MARKER}\n@__unit4b_store_format = private constant [{} x i8] c\"{encoded}\" {PROBE_MARKER}\n", format.len()));
    assert_eq!(
        strip_probe(&output),
        module,
        "only added instrumentation may differ"
    );
    (output, array(sites))
}

fn compile(output: &Path, name: &str, module: &str) -> PathBuf {
    fs::write(output.join(format!("{name}.ll")), module).unwrap();
    let executable = output.join(format!("{name}.elf"));
    crate::frontend::native::compile(module, executable.to_str().unwrap()).unwrap();
    assert_eq!(&fs::read(&executable).unwrap()[..4], b"\x7fELF");
    executable
}

fn native_receipt(
    output: &Path,
    name: &str,
    executable: &Path,
    arguments: &[String],
    probe: bool,
) -> (String, Vec<u8>, Vec<u8>, Option<i32>) {
    let directory = output.join(format!("{name}-run"));
    fs::create_dir(&directory).expect("fresh native execution directory");
    let binary = directory.join("program.elf");
    fs::hard_link(executable, &binary).unwrap();
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
    let process = Command::new(fs::canonicalize(binary).unwrap())
        .args(arguments)
        .current_dir(&directory)
        .env_clear()
        .env("PATH", "/no-tools")
        .output()
        .unwrap();
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
    fs::write(output.join(format!("{name}.stdout")), &process.stdout).unwrap();
    fs::write(output.join(format!("{name}.stderr")), &process.stderr).unwrap();
    let stderr =
        String::from_utf8(process.stderr.clone()).expect("ASCII runtime/probe diagnostics");
    let mut stores = Vec::new();
    let mut diagnostic = String::new();
    for line in stderr.split_inclusive('\n') {
        if let Some(value) = line.strip_prefix(STORE_PREFIX) {
            assert!(
                probe,
                "unexpected probe output from unchanged production module"
            );
            let (site, bits) = value.trim_end().split_once(' ').unwrap();
            stores.push(object([
                ("site", site.parse::<usize>().unwrap().to_string()),
                ("bits", bits.parse::<u64>().unwrap().to_string()),
            ]));
        } else {
            diagnostic.push_str(line);
        }
    }
    let code = diagnostic
        .split_once("error[")
        .and_then(|(_, rest)| rest.split_once(']').map(|v| v.0));
    let location = diagnostic
        .lines()
        .find_map(|line| line.trim().strip_prefix("--> "));
    let status = process.status.code();
    let receipt = object([("name", json_string(name)), ("artifact", json_string(executable.file_name().unwrap().to_str().unwrap())), ("run_directory", json_string(directory.file_name().unwrap().to_str().unwrap())), ("arguments", array(arguments.iter().map(|v| json_string(v)))), ("source_free", "true".into()), ("environment", json_string("env_clear; PATH=/no-tools")), ("status", status.map_or_else(|| "null".into(), |v| v.to_string())), ("stdout", json_string(&String::from_utf8(process.stdout.clone()).unwrap())), ("stderr", json_string(&stderr)), ("diagnostic_stderr", json_string(&diagnostic)), ("code", code.map_or_else(|| "null".into(), json_string)), ("source_location", location.map_or_else(|| "null".into(), json_string)), ("span", "null".into()), ("span_limitation", json_string("process reports line/column; full source span requires immutable witness/error-site binding")), ("stores", array(stores)), ("instrumented", probe.to_string())]);
    (receipt, process.stdout, diagnostic.into_bytes(), status)
}

fn run_case(
    output: &Path,
    name: &str,
    executable: &Path,
    args: &[String],
    checked: (&SourceProgram, hir::DefId, &SourceMap),
    fuel: usize,
    probe: bool,
) -> String {
    let (program, entry, sources) = checked;
    let mut events = Vec::new();
    let reference = execute::run_observed(
        &program.witness,
        entry,
        execute::Limits {
            fuel,
            ..execute::Limits::default()
        },
        &mut events,
    )
    .map_err(|error| error.diagnostic(sources));
    let (stdout, stderr, status) = match &reference {
        Ok(Scalar::I32(value)) => (format!("{value}\n").into_bytes(), Vec::new(), Some(0)),
        Ok(Scalar::Bool(value)) => (format!("{value}\n").into_bytes(), Vec::new(), Some(0)),
        Ok(Scalar::Unit) => (b"()\n".to_vec(), Vec::new(), Some(0)),
        Err(error) => (
            Vec::new(),
            error.render_human(sources).into_bytes(),
            Some(1),
        ),
    };
    let (native, actual_stdout, actual_stderr, actual_status) =
        native_receipt(output, name, executable, args, probe);
    let agrees = actual_stdout == stdout && actual_stderr == stderr && actual_status == status;
    let receipt = object([("name", json_string(name)), ("budget", fuel.to_string()), ("reference", result(reference)), ("reference_events", event_receipts(&events)), ("reference_event_limit", json_string("actual successful Charge, Enter, WriteField, Transfer, Acquire, Release, Return only; no scalar/whole payload store trace")), ("native", native), ("reference_native_equal", agrees.to_string())]);
    // Preserve the first discrepancy before stopping; never rewrite expectations.
    fs::write(output.join(format!("{name}.json")), format!("{receipt}\n")).unwrap();
    assert!(
        agrees,
        "reference/native discrepancy retained in {name}.json"
    );
    receipt
}

fn budgets(name: &str) -> Vec<usize> {
    let value = std::env::var(name).unwrap_or_default();
    let mut values = Vec::new();
    for item in value.split(',').filter(|v| !v.is_empty()) {
        if let Some((start, end)) = item.split_once("..=") {
            let (start, end) = (
                start.parse::<usize>().unwrap(),
                end.parse::<usize>().unwrap(),
            );
            assert!(start <= end && end <= plan::MAX_FUEL && end - start < 2048);
            values.extend(start..=end);
        } else {
            values.push(item.parse().unwrap());
        }
    }
    assert!(values.len() <= 2048 && values.iter().all(|v| *v <= plan::MAX_FUEL));
    values.sort_unstable();
    values.dedup();
    values
}

/// Runtime controls contain paths and budgets only. Hash/snapshot manifests are
/// bound externally before executing the copied test binary. Each run creates a
/// fresh directory containing only a linked immutable ELF and clears its env.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "bounded native source collector; requires frozen original source, budget controls, pinned LLVM and a fresh output path"]
fn emit_candidate_native_receipts() {
    let input =
        PathBuf::from(std::env::var_os("OXID_UNIT4B_NATIVE_SOURCE").expect("original source file"));
    let output = PathBuf::from(
        std::env::var_os("OXID_UNIT4B_NATIVE_OUTPUT").expect("fresh output directory"),
    );
    let selected = budgets("OXID_UNIT4B_NATIVE_BUDGETS");
    let fixed = budgets("OXID_UNIT4B_NATIVE_FIXED_BUDGETS");
    let probes = budgets("OXID_UNIT4B_NATIVE_PROBE_BUDGETS");
    assert!(!selected.is_empty());
    let text = fs::read_to_string(&input).unwrap();
    let name = input.file_name().unwrap().to_str().unwrap();
    let mut sources = SourceMap::new();
    let file = sources.add(name.into(), text.clone());
    let (program, entry) = checked_source(sources.get(file), &sources)
        .expect("complete original-source admission and sealed witness");
    let entry = entry.expect("source has main");
    let function = &program.witness.functions()[entry.0];
    assert!(function.parameters.is_empty() && matches!(function.result, ValueTy::Scalar(_)));
    // No observation, native emission or external compilation precedes witness.
    fs::create_dir(&output).expect("output directory must not already exist");
    fs::write(output.join(name), text).unwrap();
    fs::write(output.join("raw-view.json"), raw_view(&program)).unwrap();
    fs::write(output.join("operations.json"), operations(&program, entry)).unwrap();
    fs::write(output.join("frames.json"), frames(&program, &sources)).unwrap();
    let mut receipts = fs::File::create(output.join("receipts.jsonl")).unwrap();
    // Ordinary production wrapper first, as a separate unchanged artifact.
    let default = program.native_module(Some(entry), &sources).unwrap();
    fs::write(
        output.join("admission.json"),
        object([
            ("schema", json_string("unit4b-native-source-receipts-v1")),
            ("entry", entry.0.to_string()),
            ("guarded", default.contains("ptr %fuel").to_string()),
            (
                "native_default_admission",
                json_string("accepted by actual native_module"),
            ),
            (
                "static_exemption",
                json_string("none; collector requires source-selected guarded ABI"),
            ),
            ("default_fuel", plan::MAX_FUEL.to_string()),
        ]),
    )
    .unwrap();
    let default_binary = compile(&output, "default", &default);
    writeln!(
        receipts,
        "{}",
        run_case(
            &output,
            "default",
            &default_binary,
            &[],
            (&program, entry, &sources),
            plan::MAX_FUEL,
            false
        )
    )
    .unwrap();
    let maximum = *selected.iter().chain(&fixed).chain(&probes).max().unwrap();
    let bounded =
        native::native_module_with_fuel(&program.witness, entry, &sources, maximum).unwrap();
    fs::write(output.join("bounded.ll"), &bounded).unwrap();
    let prefix = bounded.split_once("define i32 @main() {\n").unwrap().0;
    assert_eq!(
        default.split_once("define i32 @main() {\n").unwrap().0,
        prefix
    );
    fs::write(output.join("production-prefix.ll.txt"), prefix).unwrap();
    let argv = argv_fuel_harness(&bounded, maximum);
    let argv_binary = compile(&output, "argv", &argv);
    let (probe_original, catalog) = probe_module(&bounded, &program);
    fs::write(output.join("probe-original.ll"), &probe_original).unwrap();
    fs::write(
        output.join("probe-original-stripped.ll"),
        strip_probe(&probe_original),
    )
    .unwrap();
    let probe = argv_fuel_harness(&probe_original, maximum);
    assert_eq!(strip_probe(&probe), argv);
    fs::write(output.join("stores.json"), catalog).unwrap();
    fs::write(output.join("probe.ll"), &probe).unwrap();
    fs::write(output.join("probe-stripped.ll"), strip_probe(&probe)).unwrap();
    for fuel in selected {
        writeln!(
            receipts,
            "{}",
            run_case(
                &output,
                &format!("budget-{fuel}"),
                &argv_binary,
                &[fuel.to_string()],
                (&program, entry, &sources),
                fuel,
                false
            )
        )
        .unwrap();
    }
    for fuel in fixed {
        let module =
            native::native_module_with_fuel(&program.witness, entry, &sources, fuel).unwrap();
        assert_eq!(
            module.split_once("define i32 @main() {\n").unwrap().0,
            prefix
        );
        let name = format!("fixed-{fuel}");
        let executable = compile(&output, &name, &module);
        writeln!(
            receipts,
            "{}",
            run_case(
                &output,
                &name,
                &executable,
                &[],
                (&program, entry, &sources),
                fuel,
                false
            )
        )
        .unwrap();
    }
    if !probes.is_empty() {
        let executable = compile(&output, "probe", &probe);
        for fuel in probes {
            writeln!(
                receipts,
                "{}",
                run_case(
                    &output,
                    &format!("probe-{fuel}"),
                    &executable,
                    &[fuel.to_string()],
                    (&program, entry, &sources),
                    fuel,
                    true
                )
            )
            .unwrap();
        }
    }
    fs::write(output.join("COMPLETE"), b"all requested native observations saved; independent oracle/CFG qualification is separate\n").unwrap();
}

#[test]
fn candidate_native_budget_controls_and_probe_transforms_are_bounded() {
    let text = "struct E {} fn unused() -> () { while false {} return; } fn main() -> i32 { let mut x = 1; x = 2; let e = E {}; return x; }";
    let mut sources = SourceMap::new();
    let file = sources.add("probe-transform.ox".into(), text.into());
    let (program, entry) = checked_source(sources.get(file), &sources).unwrap();
    let original =
        native::native_module_with_fuel(&program.witness, entry.unwrap(), &sources, 100).unwrap();
    let argv = argv_fuel_harness(&original, 100);
    let (probe, sites) = probe_module(&argv, &program);
    assert_eq!(strip_probe(&probe), argv);
    assert!(sites.contains("Scalar(Store)"));
    assert!(sites.contains("Construct"));
    assert!(sites.contains("\"type\":\"i8\""));
    assert!(!original.contains("@dprintf"));
}
