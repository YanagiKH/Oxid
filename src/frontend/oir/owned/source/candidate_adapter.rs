//! Test-only observations of the real source facade and its immutable witness.
//! Inputs are original source bytes, never oracle expectations or model facts.
use super::*;
use crate::frontend::{diagnostic::json_string, lexer, parser};
use std::{fs, path::Path};

fn object<const N: usize>(fields: [(&str, String); N]) -> String {
    format!(
        "{{{}}}",
        fields
            .into_iter()
            .map(|(key, value)| format!("{}:{value}", json_string(key)))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn array(values: impl IntoIterator<Item = String>) -> String {
    format!("[{}]", values.into_iter().collect::<Vec<_>>().join(","))
}

fn span(value: Span) -> String {
    format!("[{},{}]", value.start, value.end)
}

fn optional_span(value: Option<Span>) -> String {
    value.map_or_else(|| "null".into(), span)
}

fn diagnostic(value: &Diagnostic) -> String {
    object([
        ("stage", json_string(value.stage)),
        ("code", json_string(value.code)),
        ("span", optional_span(value.primary)),
        (
            "related",
            array(value.secondary.iter().map(|(s, _)| span(*s))),
        ),
    ])
}

fn failure(value: &Diagnostic) -> String {
    object([
        ("code", json_string(value.code)),
        ("span", optional_span(value.primary)),
    ])
}

fn result(value: Result<Scalar, Box<Diagnostic>>) -> String {
    let (ty, scalar, error) = match value {
        Ok(Scalar::Bool(value)) => (json_string("bool"), value.to_string(), "null".into()),
        Ok(Scalar::I32(value)) => (json_string("i32"), value.to_string(), "null".into()),
        Ok(Scalar::Unit) => (json_string("()"), "null".into(), "null".into()),
        Err(error) => ("null".into(), "null".into(), failure(&error)),
    };
    object([("result_type", ty), ("result", scalar), ("failure", error)])
}

fn checked_source(
    source: &SourceFile,
    sources: &SourceMap,
) -> Result<(SourceProgram, Option<hir::DefId>), Vec<Diagnostic>> {
    let tokens = lexer::lex(source).map_err(|error| vec![*error])?;
    let ast = parser::parse_with_mode(source, tokens, parser::SourceMode::OwnedCandidate)?;
    check_source(source, &ast, sources)
}

fn frames(program: &SourceProgram, sources: &SourceMap) -> String {
    match plan::ExecutionPlan::build(&program.witness) {
        Ok(plan) => format!(
            "{{{}}}",
            program
                .witness
                .functions()
                .iter()
                .map(|f| {
                    let usage = plan.function(f.id).usage();
                    let name = &sources.get(f.span.file).text()[f.span.start..f.span.end];
                    format!(
                        "{}:{}",
                        json_string(name),
                        object([
                            ("S", usage.scalar_slots.to_string()),
                            ("A", usage.arguments.to_string()),
                            ("P", usage.owner_cells.to_string()),
                            ("O", usage.owners.to_string()),
                            ("R", usage.references.to_string()),
                            ("L", usage.loans.to_string()),
                            ("C", usage.calls.to_string()),
                            ("X", usage.expanded_cells.to_string()),
                        ])
                    )
                })
                .collect::<Vec<_>>()
                .join(",")
        ),
        Err(error) => object([(
            "failure",
            object([
                ("code", json_string("E0605")),
                ("span", optional_span(error.span)),
                ("message", json_string(error.name)),
            ]),
        )]),
    }
}

fn scalar_type(ty: hir::Ty) -> String {
    json_string(match ty {
        hir::Ty::Bool => "bool",
        hir::Ty::I32 => "i32",
        hir::Ty::Unit => "()",
    })
}

fn value_type(ty: ValueTy) -> String {
    match ty {
        ValueTy::Scalar(ty) => object([("mode", json_string("scalar")), ("type", scalar_type(ty))]),
        ValueTy::Owned(aggregate) => object([
            ("mode", json_string("owned")),
            ("record", record_id(aggregate).0.to_string()),
        ]),
    }
}

fn borrow_kind(kind: BorrowKind) -> String {
    json_string(match kind {
        BorrowKind::Shared => "shared",
        BorrowKind::Exclusive => "exclusive",
    })
}

fn operand(value: Operand) -> String {
    object([
        ("local", value.local.0.to_string()),
        ("span", span(value.span)),
    ])
}

fn field_id(value: FieldId) -> String {
    object([
        ("record", value.record.0.to_string()),
        ("index", value.index.to_string()),
    ])
}

fn access_base(base: AccessBase) -> String {
    match base {
        AccessBase::Owner(id) => object([("mode", json_string("owner")), ("id", id.0.to_string())]),
        AccessBase::Parameter(id) => {
            object([("mode", json_string("reference")), ("id", id.0.to_string())])
        }
    }
}

fn origins(value: Option<DiagnosticOrigins>) -> String {
    value.map_or_else(
        || "null".into(),
        |value| {
            object([
                ("primary", span(value.primary)),
                ("cause", span(value.cause)),
            ])
        },
    )
}

fn call_result(value: CallResult) -> String {
    match value {
        CallResult::Scalar(id) => {
            object([("mode", json_string("scalar")), ("id", id.0.to_string())])
        }
        CallResult::Owned(id) => object([("mode", json_string("owned")), ("id", id.0.to_string())]),
    }
}

fn rvalue(value: &Rvalue) -> String {
    match value {
        Rvalue::Load(place) => object([
            ("kind", json_string("Load")),
            ("place", place.id.0.to_string()),
            ("span", span(place.span)),
        ]),
        Rvalue::CheckedNegateI32 {
            operand: value,
            operator_span,
        } => object([
            ("kind", json_string("CheckedNegateI32")),
            ("operand", operand(*value)),
            ("operator_span", span(*operator_span)),
        ]),
        Rvalue::NotBool {
            operand: value,
            operator_span,
        } => object([
            ("kind", json_string("NotBool")),
            ("operand", operand(*value)),
            ("operator_span", span(*operator_span)),
        ]),
        Rvalue::Bool(value) => {
            object([("kind", json_string("Bool")), ("value", value.to_string())])
        }
        Rvalue::I32(value) => object([("kind", json_string("I32")), ("value", value.to_string())]),
        Rvalue::Unit => object([("kind", json_string("Unit"))]),
        Rvalue::Copy(value) => {
            object([("kind", json_string("Copy")), ("operand", operand(*value))])
        }
        Rvalue::CompareScalar {
            op,
            left,
            right,
            operator_span,
        } => object([
            ("kind", json_string("CompareScalar")),
            ("operator", json_string(&format!("{op:?}"))),
            ("left", operand(*left)),
            ("right", operand(*right)),
            ("operator_span", span(*operator_span)),
        ]),
        Rvalue::CheckedI32 {
            op,
            left,
            right,
            operator_span,
        } => object([
            ("kind", json_string("CheckedI32")),
            ("operator", json_string(&format!("{op:?}"))),
            ("left", operand(*left)),
            ("right", operand(*right)),
            ("operator_span", span(*operator_span)),
        ]),
    }
}

fn scalar_statement(value: &Statement) -> String {
    match value {
        Statement::Assign(value) => object([
            ("kind", json_string("Assign")),
            ("destination", value.destination.0.to_string()),
            ("value", rvalue(&value.value)),
            ("span", span(value.span)),
        ]),
        Statement::Initialize {
            place,
            value,
            span: at,
        } => object([
            ("kind", json_string("Initialize")),
            ("place", place.id.0.to_string()),
            ("place_span", span(place.span)),
            ("value", operand(*value)),
            ("span", span(*at)),
        ]),
        Statement::Store {
            place,
            value,
            operator_span,
            span: at,
        } => object([
            ("kind", json_string("Store")),
            ("place", place.id.0.to_string()),
            ("place_span", span(place.span)),
            ("value", operand(*value)),
            ("operator_span", span(*operator_span)),
            ("span", span(*at)),
        ]),
    }
}

fn instruction(value: &OwnedInstruction) -> String {
    match value {
        OwnedInstruction::ReadStdin { .. } | OwnedInstruction::WriteStdout { .. } => {
            unreachable!("builtin effect source gate")
        }
        OwnedInstruction::ConstructEnum { .. } | OwnedInstruction::ConsumeVariant { .. } => {
            unreachable!("enum source gate")
        }
        // This frozen scalar-record observer is not the composition qualification path.
        OwnedInstruction::ConstructComposite { .. }
        | OwnedInstruction::ReadProjection { .. }
        | OwnedInstruction::WriteProjection { .. }
        | OwnedInstruction::ProjectionLength { .. }
        | OwnedInstruction::ConstructArray { .. }
        | OwnedInstruction::ReadIndex { .. }
        | OwnedInstruction::WriteIndex { .. }
        | OwnedInstruction::ArrayLength { .. } => {
            unreachable!("operation is outside this historical scalar-record observer")
        }
        OwnedInstruction::Scalar(value) => object([
            ("operation", json_string("Scalar")),
            ("scalar", scalar_statement(value)),
        ]),
        OwnedInstruction::StorageLive(id) => object([
            ("operation", json_string("StorageLive")),
            ("owner", id.0.to_string()),
        ]),
        OwnedInstruction::StorageEnd(id) => object([
            ("operation", json_string("StorageEnd")),
            ("owner", id.0.to_string()),
        ]),
        OwnedInstruction::Discard(id) => object([
            ("operation", json_string("Discard")),
            ("owner", id.0.to_string()),
        ]),
        OwnedInstruction::Construct {
            destination,
            fields,
        } => object([
            ("operation", json_string("Construct")),
            ("destination", destination.0.to_string()),
            (
                "fields",
                array(fields.iter().map(|(field, value)| {
                    object([("field", field_id(*field)), ("value", operand(*value))])
                })),
            ),
        ]),
        OwnedInstruction::MoveInitialize {
            destination,
            source,
        } => object([
            ("operation", json_string("MoveInitialize")),
            ("destination", destination.0.to_string()),
            ("source", source.0.to_string()),
        ]),
        OwnedInstruction::Replace {
            destination,
            source,
        } => object([
            ("operation", json_string("Replace")),
            ("destination", destination.0.to_string()),
            ("source", source.0.to_string()),
        ]),
        OwnedInstruction::ReadField {
            destination,
            base,
            field,
        } => object([
            ("operation", json_string("ReadField")),
            ("destination", destination.0.to_string()),
            ("base", access_base(*base)),
            ("field", field_id(*field)),
        ]),
        OwnedInstruction::WriteField { base, field, value } => object([
            ("operation", json_string("WriteField")),
            ("base", access_base(*base)),
            ("field", field_id(*field)),
            ("value", operand(*value)),
        ]),
        OwnedInstruction::OpenCall(id) => object([
            ("operation", json_string("OpenCall")),
            ("call", id.0.to_string()),
        ]),
        OwnedInstruction::PrepareScalar {
            call,
            argument,
            value,
        } => object([
            ("operation", json_string("PrepareScalar")),
            ("call", call.0.to_string()),
            ("argument", argument.to_string()),
            ("value", operand(*value)),
        ]),
        OwnedInstruction::PrepareOwned {
            call,
            argument,
            source,
        } => object([
            ("operation", json_string("PrepareOwned")),
            ("call", call.0.to_string()),
            ("argument", argument.to_string()),
            ("source", source.0.to_string()),
        ]),
        OwnedInstruction::PrepareBorrow {
            call,
            argument,
            loan,
        } => object([
            ("operation", json_string("PrepareBorrow")),
            ("call", call.0.to_string()),
            ("argument", argument.to_string()),
            ("loan", loan.0.to_string()),
        ]),
    }
}

fn terminator(value: &OwnedTerminatorKind) -> String {
    match value {
        OwnedTerminatorKind::MatchDispatch { .. } => unreachable!("enum source gate"),
        OwnedTerminatorKind::Branch {
            condition,
            then_block,
            else_block,
        } => object([
            ("operation", json_string("Branch")),
            ("condition", operand(*condition)),
            ("then_block", then_block.0.to_string()),
            ("else_block", else_block.0.to_string()),
        ]),
        OwnedTerminatorKind::Goto(id) => object([
            ("operation", json_string("Goto")),
            ("target", id.0.to_string()),
        ]),
        OwnedTerminatorKind::Invoke { call, continuation } => object([
            ("operation", json_string("Invoke")),
            ("call", call.0.to_string()),
            ("continuation", continuation.0.to_string()),
        ]),
        OwnedTerminatorKind::ReturnScalar(value) => object([
            ("operation", json_string("ReturnScalar")),
            ("value", operand(*value)),
        ]),
        OwnedTerminatorKind::ReturnOwned(id) => object([
            ("operation", json_string("ReturnOwned")),
            ("owner", id.0.to_string()),
        ]),
    }
}

fn owner_kind(value: OwnerKind) -> String {
    match value {
        OwnerKind::Parameter { position } => object([
            ("class", json_string("parameter")),
            ("position", position.to_string()),
        ]),
        OwnerKind::Local { mutable } => object([
            ("class", json_string("local")),
            ("mutable", mutable.to_string()),
        ]),
        OwnerKind::Temporary => object([("class", json_string("temporary"))]),
        OwnerKind::StagedArgument { call, argument } => object([
            ("class", json_string("staging")),
            ("call", call.0.to_string()),
            ("argument", argument.to_string()),
        ]),
        OwnerKind::CallResult { call } => object([
            ("class", json_string("call_result")),
            ("call", call.0.to_string()),
        ]),
    }
}

fn raw_function(value: &RawOwnedFunction) -> String {
    object([
        ("id", value.id.0.to_string()),
        ("span", span(value.span)),
        ("result", value_type(value.result)),
        ("entry", value.entry.0.to_string()),
        (
            "parameters",
            array(
                value
                    .parameters
                    .iter()
                    .enumerate()
                    .map(|(position, binding)| {
                        let (mode, id, at, ty) = match binding {
                            ParameterBinding::Scalar(id) => (
                                "scalar",
                                id.0,
                                value.locals[id.0].span,
                                value_type(ValueTy::Scalar(value.locals[id.0].ty)),
                            ),
                            ParameterBinding::Owned(id) => (
                                "owned",
                                id.0,
                                value.owners[id.0].span,
                                value_type(ValueTy::Owned(value.owners[id.0].aggregate())),
                            ),
                            ParameterBinding::Reference(id) => {
                                let reference = &value.references[id.0];
                                (
                                    "reference",
                                    id.0,
                                    reference.span,
                                    object([
                                        ("mode", borrow_kind(reference.kind)),
                                        (
                                            "record",
                                            borrowed_record_id(reference.referent()).0.to_string(),
                                        ),
                                    ]),
                                )
                            }
                        };
                        object([
                            ("position", position.to_string()),
                            ("mode", json_string(mode)),
                            ("id", id.to_string()),
                            ("span", span(at)),
                            ("type", ty),
                        ])
                    }),
            ),
        ),
        (
            "locals",
            array(value.locals.iter().enumerate().map(|(id, local)| {
                object([
                    ("id", id.to_string()),
                    ("type", scalar_type(local.ty)),
                    ("span", span(local.span)),
                    (
                        "kind",
                        json_string(match local.kind {
                            LocalKind::Parameter => "parameter",
                            LocalKind::Binding => "binding",
                            LocalKind::Temporary => "temporary",
                        }),
                    ),
                ])
            })),
        ),
        (
            "places",
            array(value.places.iter().enumerate().map(|(id, place)| {
                object([
                    ("id", id.to_string()),
                    ("type", scalar_type(place.ty)),
                    ("span", span(place.span)),
                ])
            })),
        ),
        (
            "owners",
            array(value.owners.iter().enumerate().map(|(id, owner)| {
                object([
                    ("id", id.to_string()),
                    ("record", record_id(owner.aggregate()).0.to_string()),
                    ("span", span(owner.span)),
                    ("kind", owner_kind(owner.kind)),
                ])
            })),
        ),
        (
            "references",
            array(value.references.iter().enumerate().map(|(id, reference)| {
                object([
                    ("id", id.to_string()),
                    (
                        "record",
                        borrowed_record_id(reference.referent()).0.to_string(),
                    ),
                    ("mode", borrow_kind(reference.kind)),
                    ("position", reference.position.to_string()),
                    ("span", span(reference.span)),
                ])
            })),
        ),
        (
            "loans",
            array(value.loans.iter().enumerate().map(|(id, loan)| {
                object([
                    ("id", id.to_string()),
                    ("call", loan.call.0.to_string()),
                    ("argument", loan.argument.to_string()),
                    ("authority", access_base(loan.authority)),
                    ("mode", borrow_kind(loan.kind)),
                    ("record", borrowed_record_id(loan.referent()).0.to_string()),
                    ("span", span(loan.span)),
                ])
            })),
        ),
        (
            "calls",
            array(value.calls.iter().enumerate().map(|(id, call)| {
                object([
                    ("id", id.to_string()),
                    ("callee", call.target.0.to_string()),
                    ("span", span(call.span)),
                    (
                        "parent",
                        call.parent.map_or_else(
                            || "null".into(),
                            |(parent, argument)| {
                                object([
                                    ("call", parent.0.to_string()),
                                    ("argument", argument.to_string()),
                                ])
                            },
                        ),
                    ),
                    (
                        "arguments",
                        array(
                            call.arguments
                                .iter()
                                .enumerate()
                                .map(|(position, argument)| {
                                    let (mode, id) = match argument {
                                        ArgumentSlot::Scalar => ("scalar", "null".into()),
                                        ArgumentSlot::Owned(id) => ("owned", id.0.to_string()),
                                        ArgumentSlot::Borrow(id) => ("borrow", id.0.to_string()),
                                    };
                                    object([
                                        ("position", position.to_string()),
                                        ("mode", json_string(mode)),
                                        ("id", id),
                                    ])
                                }),
                        ),
                    ),
                    ("result", call_result(call.result)),
                ])
            })),
        ),
        (
            "blocks",
            array(value.blocks.iter().enumerate().map(|(id, block)| {
                object([
                    ("id", id.to_string()),
                    ("span", span(block.span)),
                    (
                        "merge",
                        block.merge.as_ref().map_or_else(
                            || "null".into(),
                            |merge| {
                                object([
                                    ("operation", json_string("BoolMerge")),
                                    ("destination", merge.destination.0.to_string()),
                                    ("span", span(merge.span)),
                                    ("operator_span", span(merge.operator_span)),
                                    (
                                        "incoming",
                                        array(merge.incoming.iter().map(|input| {
                                            object([
                                                ("predecessor", input.predecessor.0.to_string()),
                                                ("value", operand(input.value)),
                                            ])
                                        })),
                                    ),
                                ])
                            },
                        ),
                    ),
                    (
                        "instructions",
                        array(
                            block
                                .statements
                                .iter()
                                .enumerate()
                                .map(|(position, statement)| {
                                    object([
                                        ("position", position.to_string()),
                                        ("span", span(statement.span)),
                                        ("charge_span", span(plan::instruction_span(statement))),
                                        ("origins", origins(statement.diagnostic_origins)),
                                        ("instruction", instruction(&statement.kind)),
                                    ])
                                }),
                        ),
                    ),
                    (
                        "terminator",
                        block.terminator.as_ref().map_or_else(
                            || "null".into(),
                            |value| {
                                object([
                                    ("position", block.statements.len().to_string()),
                                    ("span", span(value.span)),
                                    ("origins", origins(value.diagnostic_origins)),
                                    ("instruction", terminator(&value.kind)),
                                ])
                            },
                        ),
                    ),
                ])
            })),
        ),
    ])
}

/// These immutable tables preserve verifier-authoritative IDs, modes and slots.
/// Declaration spans correlate bindings without trusting source type summaries.
fn raw_view(program: &SourceProgram) -> String {
    let declarations = program.witness.declarations();
    object([
        (
            "records",
            array(declarations.records().iter().map(|record| {
                object([
                    ("id", record.id().0.to_string()),
                    ("span", span(record.span())),
                    (
                        "fields",
                        array(
                            declarations
                                .fields(record.id())
                                .expect("verified record fields")
                                .iter()
                                .map(|field| {
                                    object([
                                        ("id", field_id(field.id())),
                                        ("type", scalar_type(field.ty())),
                                        ("span", span(field.span())),
                                    ])
                                }),
                        ),
                    ),
                ])
            })),
        ),
        (
            "functions",
            array(program.witness.functions().iter().map(raw_function)),
        ),
    ])
}

fn runtime_usage(program: &SourceProgram, events: &[execute::Event]) -> String {
    let plan = match plan::ExecutionPlan::build(&program.witness) {
        Ok(plan) => plan,
        Err(_) => return "null".into(),
    };
    let mut active = Vec::new();
    let (mut cells, mut bytes, mut slots) = (0, 0, 0);
    let (mut max_frames, mut max_cells, mut max_bytes, mut max_slots) = (0, 0, 0, 0);
    let mut entries = Vec::new();
    for (index, event) in events.iter().enumerate() {
        match event {
            execute::Event::Enter(function, activation) => {
                let usage = plan.function(*function).usage();
                active.push(*function);
                cells += usage.expanded_cells;
                bytes += usage.reference_bytes;
                slots += usage.scalar_slots;
                max_frames = max_frames.max(active.len());
                max_cells = max_cells.max(cells);
                max_bytes = max_bytes.max(bytes);
                max_slots = max_slots.max(slots);
                entries.push(object([
                    ("event", json_string("Enter")),
                    ("event_index", index.to_string()),
                    ("function", function.0.to_string()),
                    ("activation", activation.to_string()),
                ]));
            }
            execute::Event::Return(function) => {
                assert_eq!(
                    active.pop(),
                    Some(*function),
                    "actual return must match active frame"
                );
                let usage = plan.function(*function).usage();
                cells -= usage.expanded_cells;
                bytes -= usage.reference_bytes;
                slots -= usage.scalar_slots;
                entries.push(object([
                    ("event", json_string("Return")),
                    ("event_index", index.to_string()),
                    ("function", function.0.to_string()),
                ]));
            }
            _ => {}
        }
    }
    object([
        (
            "basis",
            json_string("actual Enter/Return events and witness-bound ExecutionPlan usage"),
        ),
        ("peak_frames", max_frames.to_string()),
        ("peak_expanded_cells", max_cells.to_string()),
        ("peak_dynamic_reference_bytes", max_bytes.to_string()),
        ("peak_scalar_slots", max_slots.to_string()),
        ("activation_events", array(entries)),
        (
            "function_usage",
            array(program.witness.functions().iter().map(|function| {
                let usage = plan.function(function.id).usage();
                object([
                    ("function", function.id.0.to_string()),
                    ("payload_bytes", usage.payload_bytes.to_string()),
                    ("reference_bytes", usage.reference_bytes.to_string()),
                    ("native_bytes", usage.native_bytes.to_string()),
                ])
            })),
        ),
    ])
}

#[derive(Default)]
struct Options {
    budget_max: Option<usize>,
    emit_llvm: bool,
    run_native: bool,
}

fn compile_and_run_native(id: &str, module: &str, output: &Path) -> String {
    let directory_name = format!("{id}-run");
    let directory = output.join(&directory_name);
    fs::create_dir(&directory).expect("fresh isolated native run directory");
    let executable = directory.join("program.elf");
    if let Err(error) =
        crate::frontend::native::compile(module, executable.to_str().expect("UTF-8 output path"))
    {
        return object([("failure", diagnostic(&error))]);
    }
    assert_eq!(&fs::read(&executable).expect("read ELF")[..4], b"\x7fELF");
    assert_eq!(
        fs::read_dir(&directory)
            .expect("inspect isolated run directory")
            .count(),
        1,
        "native execution directory contains only the ELF"
    );
    let executable = fs::canonicalize(&executable).expect("absolute native executable");
    let execution = std::process::Command::new(executable)
        .current_dir(&directory)
        .env_clear()
        .env("PATH", "/no-tools")
        .output()
        .expect("execute source-free native ELF");
    let stdout_name = format!("{id}.native.stdout");
    let stderr_name = format!("{id}.native.stderr");
    fs::write(output.join(&stdout_name), &execution.stdout).expect("save exact native stdout");
    fs::write(output.join(&stderr_name), &execution.stderr).expect("save exact native stderr");
    object([
        ("elf", json_string(&format!("{directory_name}/program.elf"))),
        ("source_free", "true".into()),
        (
            "status",
            execution
                .status
                .code()
                .map_or_else(|| "null".into(), |code| code.to_string()),
        ),
        ("success", execution.status.success().to_string()),
        (
            "stdout",
            json_string(&String::from_utf8_lossy(&execution.stdout)),
        ),
        (
            "stderr",
            json_string(&String::from_utf8_lossy(&execution.stderr)),
        ),
        ("stdout_file", json_string(&stdout_name)),
        ("stderr_file", json_string(&stderr_name)),
        ("failure", "null".into()),
    ])
}

fn receipt(id: &str, text: String, options: Options, output: &Path) -> String {
    let mut sources = SourceMap::new();
    let file = sources.add(format!("{id}.ox"), text);
    let source = sources.get(file);
    let (program, entry) = match checked_source(source, &sources) {
        Ok(value) => value,
        Err(errors) => {
            return object([
                ("id", json_string(id)),
                (
                    "check",
                    object([
                        ("accepted", "false".into()),
                        ("diagnostics", array(errors.iter().map(diagnostic))),
                    ]),
                ),
            ])
        }
    };
    let reference = result(program.run(entry, &sources));
    let mut fields = vec![
        format!("\"id\":{}", json_string(id)),
        "\"check\":{\"accepted\":true,\"diagnostics\":[]}".into(),
        format!("\"reference\":{reference}"),
        format!("\"frames\":{}", frames(&program, &sources)),
        format!("\"raw_view\":{}", raw_view(&program)),
    ];

    // run_observed expects an already admitted entry signature. Keep the same
    // guard as the normal sealed consumer; invalid entry receipts use run above.
    if let Some(entry) = entry.filter(|id| {
        let function = &program.witness.functions()[id.0];
        function.parameters.is_empty() && matches!(function.result, ValueTy::Scalar(_))
    }) {
        let mut events = Vec::new();
        let observed = execute::run_observed(
            &program.witness,
            entry,
            execute::Limits::default(),
            &mut events,
        )
        .map_err(|error| error.diagnostic(&sources));
        assert_eq!(
            result(observed),
            reference,
            "observed and ordinary sealed execution disagree"
        );
        fields.push(format!(
            "\"charge_events\":{}",
            array(events.iter().filter_map(|event| {
                if let execute::Event::Charge(at, cost) = event {
                    Some(object([("cost", cost.to_string()), ("span", span(*at))]))
                } else {
                    None
                }
            }))
        ));
        fields.push("\"charge_mapping\":{\"status\":\"pending\",\"reason\":\"consumer events expose span and cost, not operation identity\"}".into());
        fields.push(format!(
            "\"runtime_usage\":{}",
            runtime_usage(&program, &events)
        ));
    }

    if let Some(maximum) = options.budget_max {
        fields.push(format!(
            "\"budgets\":{}",
            array((0..=maximum).map(|budget| {
                let observation = result(
                    execute::run_limits(
                        &program.witness,
                        entry,
                        execute::Limits {
                            fuel: budget,
                            ..execute::Limits::default()
                        },
                    )
                    .map_err(|error| error.diagnostic(&sources)),
                );
                format!("{{\"budget\":{budget},{}", &observation[1..])
            }))
        ));
    }

    if options.emit_llvm || options.run_native {
        let native = match program.native_module(entry, &sources) {
            Ok(module) => {
                let filename = format!("{id}.ll");
                fs::write(output.join(&filename), &module).expect("write real native LLVM module");
                let execution = if options.run_native {
                    compile_and_run_native(id, &module, output)
                } else {
                    "null".into()
                };
                object([
                    ("llvm", json_string(&filename)),
                    ("failure", "null".into()),
                    ("execution", execution),
                ])
            }
            Err(error) => object([("llvm", "null".into()), ("failure", diagnostic(&error))]),
        };
        fields.push(format!("\"native\":{native}"));
    }
    format!("{{{}}}", fields.join(","))
}

/// External collectors hash source, this binary, and emitted receipts. This
/// adapter deliberately knows no expected result or frozen model location.
/// The output directory must not already exist, preventing stale receipts.
#[test]
#[ignore = "source receipt collector; requires explicit source and fresh output directories"]
fn emit_candidate_receipts() {
    let input = std::env::var_os("OXID_UNIT4B_SOURCE_INPUT_DIR").expect("source input directory");
    let output =
        std::env::var_os("OXID_UNIT4B_CANDIDATE_OUTPUT_DIR").expect("fresh output directory");
    let output = Path::new(&output);
    let budget_max = std::env::var("OXID_UNIT4B_BUDGET_MAX").ok().map(|value| {
        let maximum = value.parse::<usize>().expect("integer budget maximum");
        assert!(
            maximum <= plan::MAX_FUEL,
            "test budget cannot exceed production fuel"
        );
        maximum
    });
    let budget_cases = std::env::var("OXID_UNIT4B_BUDGET_CASES").unwrap_or_default();
    let emit_llvm = std::env::var("OXID_UNIT4B_EMIT_LLVM").is_ok_and(|value| value == "1");
    let run_native = std::env::var("OXID_UNIT4B_RUN_NATIVE").is_ok_and(|value| value == "1");
    let mut paths = fs::read_dir(input)
        .expect("read source directory")
        .map(|entry| entry.expect("source directory entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "ox"))
        .collect::<Vec<_>>();
    paths.sort();
    assert!(!paths.is_empty(), "source directory contains no .ox files");
    fs::create_dir(output).expect("create fresh output directory (must not already exist)");
    for path in paths {
        let id = path
            .file_stem()
            .and_then(|name| name.to_str())
            .expect("UTF-8 source stem");
        let options = Options {
            budget_max: budget_max.filter(|_| budget_cases.split(',').any(|name| name == id)),
            emit_llvm,
            run_native,
        };
        let source = fs::read_to_string(&path).expect("read original UTF-8 source");
        let observation = receipt(id, source, options, output);
        fs::write(
            output.join(format!("{id}.json")),
            format!("{observation}\n"),
        )
        .expect("write receipt");
    }
}

#[test]
fn receipts_follow_real_acceptance_and_sealed_execution() {
    let accepted = receipt(
        "empty",
        "struct E {} fn main() -> i32 { let e = E {}; return 7; }".into(),
        Options::default(),
        Path::new("."),
    );
    assert!(accepted.contains("\"accepted\":true"));
    assert!(
        accepted.contains("\"reference\":{\"result_type\":\"i32\",\"result\":7,\"failure\":null}")
    );
    assert!(accepted.contains("\"charge_events\":["));
    let rejected = receipt(
        "moved",
        "struct E {} fn main() -> () { let e = E {}; e; e; return; }".into(),
        Options::default(),
        Path::new("."),
    );
    assert!(rejected.contains("\"accepted\":false"));
    assert!(rejected.contains("\"code\":\"E0310\""));
    assert!(!rejected.contains("\"reference\""));
}

#[test]
fn source_stage_failure_never_emits_execution_receipts() {
    let rejected = receipt(
        "bad",
        "struct E {".into(),
        Options::default(),
        Path::new("."),
    );
    assert!(rejected.contains("\"accepted\":false"));
    assert!(rejected.contains("\"stage\":\"parse\""));
    assert!(!rejected.contains("\"reference\""));
    assert!(!rejected.contains("\"frames\""));
}

#[test]
fn lowered_budgets_observe_real_failure_before_success() {
    let value = receipt(
        "budget",
        "struct E {} fn main() -> i32 { return 7; }".into(),
        Options {
            budget_max: Some(8),
            emit_llvm: false,
            run_native: false,
        },
        Path::new("."),
    );
    assert!(value.contains(
        "\"budget\":0,\"result_type\":null,\"result\":null,\"failure\":{\"code\":\"E0601\""
    ));
    assert!(value.contains("\"budget\":8,\"result_type\":\"i32\",\"result\":7,\"failure\":null"));
}

#[test]
fn raw_view_preserves_original_interleaved_parameter_authorities() {
    let text = "struct A { n: i32 } struct B { n: i32 } fn f(a: i32, x: A, p: &B, b: bool, y: B) -> () { return; }";
    let mut sources = SourceMap::new();
    let file = sources.add("mixed.ox".into(), text.into());
    let (program, entry) = checked_source(sources.get(file), &sources).unwrap();
    assert!(entry.is_none());
    let raw = raw_view(&program);
    for (position, mode, id) in [
        (0, "scalar", 0),
        (1, "owned", 0),
        (2, "reference", 0),
        (3, "scalar", 1),
        (4, "owned", 1),
    ] {
        assert!(raw.contains(&format!(
            "\"position\":{position},\"mode\":\"{mode}\",\"id\":{id}"
        )));
    }
    assert!(raw.contains("\"id\":0,\"record\":0"));
    assert!(raw.contains("\"id\":1,\"record\":1"));
    assert!(raw.contains("\"class\":\"parameter\",\"position\":4"));
    assert!(raw.contains("\"record\":1,\"mode\":\"shared\",\"position\":2"));
    let missing_entry = receipt("mixed", text.into(), Options::default(), Path::new("."));
    assert!(missing_entry.contains("\"code\":\"E0600\""));
    assert!(!missing_entry.contains("\"charge_events\""));
}

#[cfg(test)]
#[path = "candidate_mutations.rs"]
mod candidate_mutations;

#[cfg(test)]
#[path = "candidate_native.rs"]
mod candidate_native;

fn record_id(aggregate: AggregateTy) -> RecordId {
    match aggregate {
        AggregateTy::Record(record) => record,
        AggregateTy::FixedArray(_) | AggregateTy::Enum(_) => panic!("source aggregate gate"),
    }
}

fn borrowed_record_id(referent: BorrowedTy) -> RecordId {
    match referent {
        BorrowedTy::Exact(aggregate) => record_id(aggregate),
        BorrowedTy::ScalarSlice(_) => panic!("source slice gate"),
    }
}
