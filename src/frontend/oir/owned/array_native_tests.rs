//! Unit2D producer native semantics, frozen from the accepted source contract.
//!
//! Their author did not compile or execute candidate code while constructing
//! these models. They use scalar sequences and the contract's charge table, never ExecutionPlan,
//! emitter accounting, reference schedules, or a caller-created witness.
//! The reference probe is a second observed consumer, not the expected oracle.
//!
//! Planned input families: 195 small core, 39 width-1024 core, 4 extreme-payload,
//! 9 transfer, 8 continuation, and 4 effect cases. Each LLVM family compiles one
//! acyclic production module and one guarded argv-fuel derivative per input.
//! Six additional guarded production wrappers exercise exact fuel boundaries.
//! Source-only totals: 259 distinct inputs, 524 ELF artifacts, 7,058 ELF
//! executions, and 6,793 reference comparisons. The six ignored test functions
//! are not those case/process/artifact counts. Evidence retention additionally
//! keeps 259 original guarded LLVM modules beside the compiled module/ELF pairs.
//! Physical poison/one-byte mutants and independent held-outs belong to the
//! reviewer and are deliberately not claimed by these logical tests.
use super::super::super::consumer_fixtures as raw;
use super::super::super::execute::{Event, ObservationControl, OwnedRunFailure};
use super::super::*;
use super::{append_cycle, block};
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use super::{argv_fuel_harness, assert_result, scalar_output, Scratch};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Failure {
    Fuel(Span),
    Bounds(Span),
    Overflow(Span),
}

impl Failure {
    fn observed(self) -> OwnedRunFailure {
        match self {
            Self::Fuel(span) => OwnedRunFailure::Scalar(RunFailure::Fuel(span)),
            Self::Bounds(span) => OwnedRunFailure::Bounds(span),
            Self::Overflow(span) => OwnedRunFailure::Scalar(RunFailure::Overflow(span)),
        }
    }
}

#[derive(Debug)]
struct Effect {
    /// Index of the independently inventoried charge that completes this effect.
    step: usize,
    write: bool,
    index: usize,
    value: Scalar,
}

#[derive(Debug)]
struct Model {
    charges: Vec<(Span, usize)>,
    effects: Vec<Effect>,
    result: Result<Scalar, Failure>,
}

impl Model {
    fn new(result: Result<Scalar, Failure>) -> Self {
        Self {
            charges: vec![],
            effects: vec![],
            result,
        }
    }

    fn charge(&mut self, span: Span, cost: usize) {
        self.charges.push((span, cost));
    }

    fn effect(&mut self, write: bool, index: usize, value: Scalar) {
        self.effects.push(Effect {
            step: self.charges.len() - 1,
            write,
            index,
            value,
        });
    }

    fn fuel(&self) -> usize {
        self.charges.iter().map(|&(_, cost)| cost).sum()
    }

    fn paid(&self, mut fuel: usize) -> (usize, usize) {
        for (i, &(_, cost)) in self.charges.iter().enumerate() {
            if fuel < cost {
                return (i, fuel);
            }
            fuel -= cost;
        }
        (self.charges.len(), fuel)
    }

    fn expected(&self, fuel: usize) -> Result<Scalar, Failure> {
        let (paid, _) = self.paid(fuel);
        self.charges
            .get(paid)
            .map_or(self.result, |&(span, _)| Err(Failure::Fuel(span)))
    }

    fn boundary_fuels(&self, selected: &[Span]) -> Vec<usize> {
        let mut fuels = vec![0, self.fuel() - 1, self.fuel()];
        let mut prefix = 0;
        for &(span, cost) in &self.charges {
            prefix += cost;
            if selected.contains(&span) {
                fuels.extend([prefix - 1, prefix]);
            }
        }
        fuels.sort_unstable();
        fuels.dedup();
        fuels
    }

    fn compare_reference(&self, program: RawOwnedProgram, sources: &SourceMap, fuel: usize) {
        let observed = verified::probe_array_reference(
            program,
            sources,
            budget::Limits::DEFAULT,
            Some(hir::DefId(0)),
            execute::Limits {
                fuel,
                ..execute::Limits::default()
            },
            ObservationControl::default(),
        )
        .expect("the real verifier must admit this model input");
        assert!(!observed.truncated);
        assert_eq!(
            observed.result,
            self.expected(fuel).map_err(Failure::observed),
            "fuel={fuel}"
        );
        let (paid, remaining) = self.paid(fuel);
        let charges: Vec<_> = observed
            .events
            .iter()
            .filter_map(|event| match event {
                Event::Charge(span, cost) => Some((*span, *cost)),
                _ => None,
            })
            .collect();
        assert_eq!(charges, self.charges[..paid], "fuel={fuel}");
        assert_eq!(observed.remaining_fuel, remaining, "fuel={fuel}");
        let actual_effects: Vec<_> = observed
            .events
            .iter()
            .filter_map(|event| match event {
                Event::ReadIndex(_, index, value) => Some((false, *index, *value)),
                Event::WriteIndex(_, index, value) => Some((true, *index, *value)),
                _ => None,
            })
            .collect();
        let expected_effects: Vec<_> = self
            .effects
            .iter()
            .filter(|effect| effect.step < paid)
            .map(|effect| (effect.write, effect.index, effect.value))
            .collect();
        assert_eq!(actual_effects, expected_effects, "fuel={fuel}");
    }
}

fn array(ty: hir::Ty, n: usize) -> AggregateTy {
    AggregateTy::FixedArray(FixedArrayTy::check(ty, n).unwrap())
}

fn owner(ty: hir::Ty, n: usize, kind: OwnerKind, span: Span) -> OwnerDecl {
    OwnerDecl {
        aggregate: AggregateSlot::try_from_aggregate(array(ty, n)).unwrap(),
        kind,
        span,
    }
}

fn literal(value: Scalar) -> Rvalue {
    match value {
        Scalar::Bool(value) => Rvalue::Bool(value),
        Scalar::I32(value) => Rvalue::I32(value),
        Scalar::Unit => Rvalue::Unit,
    }
}

fn module(mut program: RawOwnedProgram, sources: &SourceMap, guarded: bool, fuel: usize) -> String {
    if guarded {
        append_cycle(&mut program);
    }
    verified::probe_array_native(
        program,
        sources,
        budget::Limits::DEFAULT,
        Some(hir::DefId(0)),
        sources,
        NativeControl {
            fuel,
            ..NativeControl::default()
        },
    )
    .expect("native models must traverse the authoritative verifier")
    .result
    .expect("native admission must admit this finite fixture")
}

fn escaped(text: &str) -> String {
    text.bytes().map(|byte| format!("\\{byte:02X}")).collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Access {
    Read,
    Write,
    Length,
}

#[derive(Clone, Copy, Debug)]
struct Core {
    ty: hir::Ty,
    n: usize,
    index: i32,
    access: Access,
    extremes: bool,
}

impl Core {
    fn seed_count(self) -> usize {
        if self.n == 1024 {
            2
        } else {
            self.n
        }
    }

    fn value(self, index: usize) -> Scalar {
        match self.ty {
            hir::Ty::I32 if self.extremes || self.n == 1024 => {
                Scalar::I32(if index.is_multiple_of(2) {
                    i32::MIN
                } else {
                    i32::MAX
                })
            }
            hir::Ty::I32 => Scalar::I32(31 * index as i32 - 47),
            hir::Ty::Bool => Scalar::Bool((index + self.n).is_multiple_of(2)),
            hir::Ty::Unit => Scalar::Unit,
        }
    }

    fn replacement(self) -> Scalar {
        match self.ty {
            hir::Ty::I32 if self.extremes => {
                Scalar::I32(if self.index == 0 { i32::MAX } else { i32::MIN })
            }
            hir::Ty::I32 => Scalar::I32(700 + self.index.clamp(0, 1023)),
            hir::Ty::Bool => Scalar::Bool(!(self.index.max(0) as usize + self.n).is_multiple_of(2)),
            hir::Ty::Unit => Scalar::Unit,
        }
    }

    fn build(self, s: &impl Fn(usize) -> Span) -> RawOwnedProgram {
        let count = self.seed_count();
        let length = self.access == Access::Length;
        let result_ty = if length { hir::Ty::I32 } else { self.ty };
        let result = count + if length { 0 } else { 2 };
        let mut f = raw::function(0, ValueTy::Scalar(result_ty), s(0));
        f.locals = (0..count).map(|_| raw::scalar(self.ty, s(0))).collect();
        if !length {
            f.locals
                .extend([raw::scalar(hir::Ty::I32, s(0)), raw::scalar(self.ty, s(0))]);
        }
        f.locals.push(raw::scalar(result_ty, s(0)));
        f.owners = vec![owner(
            self.ty,
            self.n,
            OwnerKind::Local { mutable: true },
            s(0),
        )];
        let mut statements: Vec<_> = (0..count)
            .map(|i| raw::assign(i, literal(self.value(i)), s(10 + i)))
            .collect();
        if !length {
            statements.extend([
                raw::assign(count, Rvalue::I32(self.index), s(20)),
                raw::assign(count + 1, literal(self.replacement()), s(21)),
            ]);
        }
        statements.extend([
            raw::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), s(30)),
            raw::instruction(
                OwnedInstruction::ConstructArray {
                    destination: OwnerPlaceId(0),
                    elements: (0..self.n)
                        .map(|i| raw::operand(i % count, s(31)))
                        .collect(),
                },
                s(31),
            ),
        ]);
        let read = |span| {
            raw::instruction(
                OwnedInstruction::ReadIndex {
                    destination: LocalId(result),
                    base: AccessBase::Owner(OwnerPlaceId(0)),
                    index: raw::operand(count, span),
                },
                span,
            )
        };
        match self.access {
            Access::Read => statements.push(read(s(40))),
            Access::Write => statements.extend([
                raw::instruction(
                    OwnedInstruction::WriteIndex {
                        base: AccessBase::Owner(OwnerPlaceId(0)),
                        index: raw::operand(count, s(40)),
                        value: raw::operand(count + 1, s(40)),
                    },
                    s(40),
                ),
                read(s(41)),
            ]),
            Access::Length => statements.push(raw::instruction(
                OwnedInstruction::ArrayLength {
                    destination: LocalId(result),
                    base: AccessBase::Owner(OwnerPlaceId(0)),
                },
                s(40),
            )),
        }
        f.blocks.push(block(
            statements,
            OwnedTerminatorKind::ReturnScalar(raw::operand(result, s(50))),
            s(50),
        ));
        RawOwnedProgram {
            records: vec![],
            functions: vec![f],
        }
    }

    fn model(self, s: &impl Fn(usize) -> Span) -> Model {
        let valid = self.index >= 0 && (self.index as usize) < self.n;
        let result = match self.access {
            Access::Length => Ok(Scalar::I32(self.n as i32)),
            _ if !valid => Err(Failure::Bounds(s(40))),
            Access::Read => Ok(self.value(self.index as usize)),
            Access::Write => Ok(self.replacement()),
        };
        let mut model = Model::new(result);
        let width = self.n.max(1);
        let scalar_slots = self.seed_count() + if self.access == Access::Length { 1 } else { 3 };
        // Root X=S+P+4O; no arguments, loans, references, places, or calls.
        model.charge(s(0), 1 + scalar_slots + width + 4);
        for i in 0..self.seed_count() {
            model.charge(s(10 + i), 1);
        }
        if self.access != Access::Length {
            model.charge(s(20), 1);
            model.charge(s(21), 1);
        }
        model.charge(s(30), 1);
        model.charge(s(31), 1 + width);
        model.charge(s(40), 1);
        if self.access != Access::Length {
            if !valid {
                return model;
            }
            let value = result.unwrap();
            model.effect(self.access == Access::Write, self.index as usize, value);
            if self.access == Access::Write {
                model.charge(s(41), 1);
                model.effect(false, self.index as usize, value);
            }
        }
        model.charge(s(50), 1 + width);
        model
    }
}

fn core_inputs(maximum: bool) -> Vec<Core> {
    let mut cases = vec![];
    for ty in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit] {
        for n in if maximum {
            vec![1024]
        } else {
            (0..=4).collect()
        } {
            let mut indices = vec![i32::MIN, -1, n as i32, i32::MAX];
            if maximum {
                indices.extend([0, 1023]);
            } else {
                indices.extend(0..n as i32);
            }
            indices.sort_unstable();
            indices.dedup();
            for access in [Access::Read, Access::Write, Access::Length] {
                for &index in if access == Access::Length {
                    &[0][..]
                } else {
                    &indices
                } {
                    cases.push(Core {
                        ty,
                        n,
                        index,
                        access,
                        extremes: false,
                    });
                }
            }
        }
    }
    cases
}

#[test]
fn native_arrays_core_matrix_and_maximum_width_emit_without_privileged_witnesses() {
    let (sources, s) = raw::context();
    let mut counts = [0; 2];
    for (group, maximum) in [false, true].into_iter().enumerate() {
        for case in core_inputs(maximum) {
            let text = module(case.build(&s), &sources, false, case.model(&s).fuel());
            assert!(!text.contains("%fuel = alloca"));
            let bounds = case.access != Access::Length;
            assert_eq!(text.contains(&escaped("E0606")), bounds, "{case:?}");
            assert_eq!(text.contains("icmp sge i32"), bounds, "{case:?}");
            assert_eq!(text.contains("icmp slt i32"), bounds, "{case:?}");
            for forbidden in ["memcpy", "inbounds", "noalias", "undef", "poison"] {
                assert!(!text.contains(forbidden), "{case:?}: {forbidden}");
            }
            if case.access == Access::Length {
                let error =
                    verified::verify_with_limits(case.build(&s), &sources, budget::Limits::DEFAULT)
                        .unwrap_err();
                assert_eq!(
                    error.kind,
                    OwnedFailureKind::Malformed(Malformed::UnsupportedArray)
                );
            }
            counts[group] += 1;
        }
    }
    assert_eq!(counts, [195, 39]);
}

// This adapter changes only main's fuel source. Production function bodies and
// diagnostics are retained as their own artifact before creating a derivative.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn preserve_production(scratch: &Scratch, name: &str, text: &str) {
    let filename = format!("{name}-guarded-production.ll");
    std::fs::write(scratch.0.join(&filename), text).unwrap();
    if let Some(evidence) = std::env::var_os("OXID_OWNED_NATIVE_EVIDENCE") {
        std::fs::create_dir_all(&evidence).unwrap();
        std::fs::write(std::path::PathBuf::from(evidence).join(filename), text).unwrap();
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn check_process(
    scratch: &Scratch,
    binary: &std::path::Path,
    arguments: &[String],
    expected: Result<Scalar, Failure>,
    sources: &SourceMap,
) {
    match expected {
        Ok(value) => assert_result(
            scratch.run(binary, arguments),
            &scalar_output(value),
            b"",
            0,
        ),
        Err(error) => assert_result(
            scratch.run(binary, arguments),
            b"",
            error
                .observed()
                .diagnostic(sources)
                .render_human(sources)
                .as_bytes(),
            1,
        ),
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn run_input(
    scratch: &Scratch,
    name: &str,
    sources: &SourceMap,
    build: impl Fn() -> RawOwnedProgram,
    model: &Model,
    fuels: &[usize],
) -> usize {
    let production = module(build(), sources, false, model.fuel());
    let binary = scratch.compile(&production, &format!("{name}-acyclic"));
    check_process(scratch, &binary, &[], model.result, sources);

    let guarded = module(build(), sources, true, model.fuel());
    preserve_production(scratch, name, &guarded);
    let derivative = argv_fuel_harness(&guarded, model.fuel());
    let body = guarded.split_once("define i32 @main()").unwrap().0;
    assert!(derivative.starts_with(body));
    let binary = scratch.compile(&derivative, &format!("{name}-fuel-argv"));
    for &fuel in fuels {
        // Both consumers are compared to the same predeclared source model.
        model.compare_reference(build(), sources, fuel);
        check_process(
            scratch,
            &binary,
            &[fuel.to_string()],
            model.expected(fuel),
            sources,
        );
    }
    1 + fuels.len()
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7 and one serialized native execution slot"]
fn native_arrays_all_195_small_cases_use_source_free_elf_and_every_fuel() {
    let scratch = Scratch::new();
    let (sources, s) = raw::context();
    let mut processes = 0;
    let cases = core_inputs(false);
    for (i, &case) in cases.iter().enumerate() {
        let model = case.model(&s);
        processes += run_input(
            &scratch,
            &format!("array-core-{i}"),
            &sources,
            || case.build(&s),
            &model,
            &(0..=model.fuel()).collect::<Vec<_>>(),
        );
    }
    assert_eq!(cases.len(), 195);
    assert_eq!(processes, 5_037);
    eprintln!("array small core: 195 input cases, 390 compiled artifacts, {processes} ELF executions; 195 additional preserved guarded production modules");
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7 and one serialized native execution slot"]
fn native_arrays_maximum_width_39_cases_use_source_free_elf_at_charge_boundaries() {
    let scratch = Scratch::new();
    let (sources, s) = raw::context();
    let cases = core_inputs(true);
    let mut processes = 0;
    for (i, &case) in cases.iter().enumerate() {
        let model = case.model(&s);
        let fuels = model.boundary_fuels(&[s(0), s(31), s(40), s(41), s(50)]);
        processes += run_input(
            &scratch,
            &format!("array-max-{i}"),
            &sources,
            || case.build(&s),
            &model,
            &fuels,
        );
    }
    assert_eq!(cases.len(), 39);
    assert_eq!(processes, 309);
    eprintln!("array maximum width: 39 input cases, 78 compiled artifacts, {processes} ELF executions; 39 additional preserved guarded production modules");
}

#[derive(Clone, Copy, Debug)]
struct Transfer {
    ty: hir::Ty,
    n: usize,
}

impl Transfer {
    fn seed(self, i: usize) -> Scalar {
        match self.ty {
            hir::Ty::Bool => Scalar::Bool(i.is_multiple_of(2)),
            hir::Ty::I32 => Scalar::I32(113 - 41 * i as i32),
            hir::Ty::Unit => Scalar::Unit,
        }
    }

    fn build(self, s: &impl Fn(usize) -> Span) -> RawOwnedProgram {
        let result_ty = if self.n == 0 { hir::Ty::I32 } else { self.ty };
        let mut caller = raw::function(0, ValueTy::Scalar(result_ty), s(0));
        caller.locals = (0..self.n).map(|_| raw::scalar(self.ty, s(0))).collect();
        caller.locals.extend([
            raw::scalar(hir::Ty::I32, s(0)),
            raw::scalar(result_ty, s(0)),
        ]);
        caller.owners = [
            OwnerKind::Local { mutable: true },
            OwnerKind::Temporary,
            OwnerKind::Local { mutable: true },
            OwnerKind::Temporary,
            OwnerKind::StagedArgument {
                call: CallSiteId(0),
                argument: 0,
            },
            OwnerKind::CallResult {
                call: CallSiteId(0),
            },
        ]
        .into_iter()
        .map(|kind| owner(self.ty, self.n, kind, s(0)))
        .collect();
        caller.calls.push(CallDecl {
            target: hir::DefId(1),
            arguments: vec![ArgumentSlot::Owned(OwnerPlaceId(4))],
            result: CallResult::Owned(OwnerPlaceId(5)),
            parent: None,
            span: s(40),
        });
        let mut statements: Vec<_> = (0..self.n)
            .map(|i| raw::assign(i, literal(self.seed(i)), s(10 + i)))
            .collect();
        statements.push(raw::assign(
            self.n,
            Rvalue::I32(self.n.saturating_sub(1) as i32),
            s(20),
        ));
        for (destination, live, construct, reverse) in [(0, 30, 31, false), (1, 32, 33, true)] {
            statements.extend([
                raw::instruction(
                    OwnedInstruction::StorageLive(OwnerPlaceId(destination)),
                    s(live),
                ),
                raw::instruction(
                    OwnedInstruction::ConstructArray {
                        destination: OwnerPlaceId(destination),
                        elements: (0..self.n)
                            .map(|i| {
                                raw::operand(if reverse { self.n - 1 - i } else { i }, s(construct))
                            })
                            .collect(),
                    },
                    s(construct),
                ),
            ]);
        }
        statements.extend([
            raw::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(2)), s(34)),
            raw::instruction(
                OwnedInstruction::MoveInitialize {
                    destination: OwnerPlaceId(2),
                    source: OwnerPlaceId(1),
                },
                s(35),
            ),
            // A replacement, then source-style self-replacement through a
            // distinct temporary: the second replacement targets M.
            raw::instruction(
                OwnedInstruction::Replace {
                    destination: OwnerPlaceId(0),
                    source: OwnerPlaceId(2),
                },
                s(36),
            ),
            raw::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(3)), s(37)),
            raw::instruction(
                OwnedInstruction::MoveInitialize {
                    destination: OwnerPlaceId(3),
                    source: OwnerPlaceId(0),
                },
                s(38),
            ),
            raw::instruction(
                OwnedInstruction::Replace {
                    destination: OwnerPlaceId(0),
                    source: OwnerPlaceId(3),
                },
                s(39),
            ),
            raw::instruction(OwnedInstruction::OpenCall(CallSiteId(0)), s(40)),
            raw::instruction(
                OwnedInstruction::PrepareOwned {
                    call: CallSiteId(0),
                    argument: 0,
                    source: OwnerPlaceId(0),
                },
                s(41),
            ),
        ]);
        caller.blocks.push(block(
            statements,
            OwnedTerminatorKind::Invoke {
                call: CallSiteId(0),
                continuation: BlockId(1),
            },
            s(42),
        ));
        caller.blocks.push(block(
            vec![raw::instruction(
                if self.n == 0 {
                    OwnedInstruction::ArrayLength {
                        destination: LocalId(self.n + 1),
                        base: AccessBase::Owner(OwnerPlaceId(5)),
                    }
                } else {
                    OwnedInstruction::ReadIndex {
                        destination: LocalId(self.n + 1),
                        base: AccessBase::Owner(OwnerPlaceId(5)),
                        index: raw::operand(self.n, s(50)),
                    }
                },
                s(50),
            )],
            OwnedTerminatorKind::ReturnScalar(raw::operand(self.n + 1, s(51))),
            s(51),
        ));
        let mut callee = raw::function(1, ValueTy::Owned(array(self.ty, self.n)), s(59));
        callee.locals.push(raw::scalar(hir::Ty::I32, s(59)));
        callee
            .parameters
            .push(ParameterBinding::Owned(OwnerPlaceId(0)));
        callee.owners.push(owner(
            self.ty,
            self.n,
            OwnerKind::Parameter { position: 0 },
            s(59),
        ));
        callee.blocks.push(block(
            vec![raw::instruction(
                OwnedInstruction::ArrayLength {
                    destination: LocalId(0),
                    base: AccessBase::Owner(OwnerPlaceId(0)),
                },
                s(60),
            )],
            OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0)),
            s(61),
        ));
        RawOwnedProgram {
            records: vec![],
            functions: vec![caller, callee],
        }
    }

    fn model(self, s: &impl Fn(usize) -> Span) -> Model {
        let width = self.n.max(1);
        // The reverse constructor becomes the final owner through all six
        // transfer destinations. Reading its last element gives original[0].
        let value = if self.n == 0 {
            Scalar::I32(0)
        } else {
            self.seed(0)
        };
        let mut model = Model::new(Ok(value));
        // Root S=N+2, A=1, P=6w, O=6, C=1, R=L=0.
        model.charge(s(0), self.n + 30 + 6 * width);
        for i in 0..self.n {
            model.charge(s(10 + i), 1);
        }
        for (span, cost) in [
            (20, 1),
            (30, 1),
            (31, 1 + width),
            (32, 1),
            (33, 1 + width),
            (34, 1),
            (35, 1 + width),
            (36, 1 + 2 * width),
            (37, 1),
            (38, 1 + width),
            (39, 1 + 2 * width),
            (40, 2),
            (41, 1 + width),
            // Invoke includes child X=1+w+4 and one incoming width.
            (42, 7 + 2 * width),
            (60, 1),
            (61, 1 + 2 * width),
            (50, 1),
        ] {
            model.charge(s(span), cost);
        }
        if self.n > 0 {
            model.effect(false, self.n - 1, value);
        }
        model.charge(s(51), 2 + 6 * width);
        model
    }
}

fn transfer_inputs() -> Vec<Transfer> {
    [hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit]
        .into_iter()
        .flat_map(|ty| [0, 2, 4].into_iter().map(move |n| Transfer { ty, n }))
        .collect()
}

#[test]
fn native_arrays_transfer_endpoints_and_full_types_precede_native_work() {
    let (sources, s) = raw::context();
    for case in transfer_inputs() {
        let text = module(case.build(&s), &sources, false, case.model(&s).fuel());
        assert!(text.contains("@__oxid_owned_fn_1("));
        assert!(text.contains("f1_param0"));
        assert!(!text.contains("memcpy"));
    }
    // Storage size does not grant type equality, including empty arrays. These are
    // source-level mutations; an armed plan failpoint catches verifier bypass.
    let mut denials = 0;
    for (ty, n, other) in [
        (hir::Ty::Bool, 0, array(hir::Ty::I32, 0)),
        (hir::Ty::Bool, 4, array(hir::Ty::Unit, 4)),
        (hir::Ty::Bool, 4, array(hir::Ty::I32, 1)),
        (hir::Ty::Bool, 4, AggregateTy::Record(RecordId(0))),
    ] {
        for site in 0..3 {
            let mut program = Transfer { ty, n }.build(&s);
            program.records = vec![raw::record(&[hir::Ty::I32], s(0))];
            let slot = AggregateSlot::try_from_aggregate(other).unwrap();
            let expected_span = match site {
                0 => {
                    program.functions[0].owners[2].aggregate = slot;
                    s(35)
                }
                1 => {
                    program.functions[1].owners[0].aggregate = slot;
                    s(40)
                }
                _ => {
                    program.functions[0].owners[5].aggregate = slot;
                    s(40)
                }
            };
            let failure = plan::fail_allocation_after(0, || {
                verified::probe_array_native(
                    program,
                    &sources,
                    budget::Limits::DEFAULT,
                    Some(hir::DefId(0)),
                    &sources,
                    NativeControl::default(),
                )
            })
            .err()
            .expect("a malformed complete type must fail before any native plan allocation");
            assert_eq!(failure.kind, OwnedFailureKind::Malformed(Malformed::Type));
            assert_eq!(failure.primary.get(), Some(expected_span));
            denials += 1;
        }
    }
    assert_eq!(denials, 12);
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7 and one serialized native execution slot"]
fn native_arrays_nine_transfer_inputs_use_source_free_elf_and_every_fuel() {
    let scratch = Scratch::new();
    let (sources, s) = raw::context();
    let mut processes = 0;
    let cases = transfer_inputs();
    for (i, &case) in cases.iter().enumerate() {
        let model = case.model(&s);
        processes += run_input(
            &scratch,
            &format!("array-transfer-{i}"),
            &sources,
            || case.build(&s),
            &model,
            &(0..=model.fuel()).collect::<Vec<_>>(),
        );
    }
    assert_eq!(cases.len(), 9);
    assert_eq!(processes, 1_083);
    eprintln!("array transfers: 9 input cases, 18 compiled artifacts, {processes} ELF executions; 9 additional preserved guarded production modules");
}

#[derive(Clone, Copy, Debug)]
enum Continuation {
    NoSplit,
    ArithmeticBounds,
    BoundsArithmetic,
    RepeatedBoundsSuffix,
}

impl Continuation {
    fn statement_count(self) -> usize {
        match self {
            Self::NoSplit => 2,
            Self::ArithmeticBounds | Self::BoundsArithmetic => 3,
            Self::RepeatedBoundsSuffix => 4,
        }
    }

    fn predecessor(self, b: usize, guarded: bool) -> String {
        if guarded {
            return format!("f0_b{b}_g{}_ok", self.statement_count() + 1);
        }
        match self {
            Self::NoSplit => format!("b{b}"),
            Self::ArithmeticBounds | Self::RepeatedBoundsSuffix => format!("f0_b{b}_i2_bounds_ok"),
            Self::BoundsArithmetic => format!("f0_b{b}_i2_checked_ok"),
        }
    }

    fn build(self, taken: bool, s: &impl Fn(usize) -> Span) -> RawOwnedProgram {
        let mut f = raw::function(0, ValueTy::Scalar(hir::Ty::Bool), s(0));
        f.locals = [
            hir::Ty::Bool,
            hir::Ty::Bool,
            hir::Ty::I32,
            hir::Ty::Bool,
            hir::Ty::Bool,
            hir::Ty::I32,
            hir::Ty::I32,
            hir::Ty::Bool,
            hir::Ty::Bool,
            hir::Ty::Bool,
            hir::Ty::Bool,
            hir::Ty::I32,
            hir::Ty::I32,
            hir::Ty::Bool,
        ]
        .into_iter()
        .map(|ty| raw::scalar(ty, s(0)))
        .collect();
        f.owners.push(owner(
            hir::Ty::Bool,
            1,
            OwnerKind::Local { mutable: false },
            s(0),
        ));
        f.blocks.push(block(
            vec![
                raw::assign(0, Rvalue::Bool(taken), s(1)),
                raw::assign(1, Rvalue::Bool(true), s(2)),
                raw::assign(2, Rvalue::I32(0), s(3)),
                raw::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), s(4)),
                raw::instruction(
                    OwnedInstruction::ConstructArray {
                        destination: OwnerPlaceId(0),
                        elements: vec![raw::operand(1, s(5))],
                    },
                    s(5),
                ),
            ],
            OwnedTerminatorKind::Branch {
                condition: raw::operand(0, s(6)),
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
            s(6),
        ));
        for side in 0..2 {
            let origin = 100 * (side + 1);
            let arithmetic = |span| {
                raw::assign(
                    5 + side,
                    Rvalue::CheckedI32 {
                        op: hir::ArithmeticOp::Add,
                        left: raw::operand(2, span),
                        right: raw::operand(2, span),
                        operator_span: span,
                    },
                    span,
                )
            };
            let read = |destination, index, span| {
                raw::instruction(
                    OwnedInstruction::ReadIndex {
                        destination: LocalId(destination),
                        base: AccessBase::Owner(OwnerPlaceId(0)),
                        index: raw::operand(index, span),
                    },
                    span,
                )
            };
            // The other branch's merge input is intentionally never assigned
            // on this path. A value phi with eager loads would read that slot.
            let mut statements = vec![raw::assign(3 + side, Rvalue::Bool(side == 0), s(origin))];
            match self {
                Self::NoSplit => {
                    statements.push(raw::assign(11 + side, Rvalue::I32(9), s(origin + 1)))
                }
                Self::ArithmeticBounds => statements.extend([
                    arithmetic(s(origin + 1)),
                    read(7 + side, 5 + side, s(origin + 2)),
                ]),
                Self::BoundsArithmetic => {
                    statements.extend([read(7 + side, 2, s(origin + 1)), arithmetic(s(origin + 2))])
                }
                Self::RepeatedBoundsSuffix => statements.extend([
                    read(7 + side, 2, s(origin + 1)),
                    read(9 + side, 2, s(origin + 2)),
                    raw::assign(11 + side, Rvalue::I32(9), s(origin + 3)),
                ]),
            }
            f.blocks.push(block(
                statements,
                OwnedTerminatorKind::Goto(BlockId(3)),
                s(origin + 4),
            ));
        }
        f.blocks.push(block(
            vec![],
            OwnedTerminatorKind::ReturnScalar(raw::operand(13, s(301))),
            s(301),
        ));
        f.blocks[3].merge = Some(BoolMerge {
            destination: LocalId(13),
            incoming: [
                MergeInput {
                    predecessor: BlockId(1),
                    value: raw::operand(3, s(300)),
                },
                MergeInput {
                    predecessor: BlockId(2),
                    value: raw::operand(4, s(300)),
                },
            ],
            operator_span: s(300),
            span: s(300),
        });
        RawOwnedProgram {
            records: vec![],
            functions: vec![f],
        }
    }

    fn model(self, taken: bool, s: &impl Fn(usize) -> Span) -> Model {
        let mut model = Model::new(Ok(Scalar::Bool(taken)));
        // Fourteen scalar snapshots, one owner, one payload cell: X=19.
        model.charge(s(0), 20);
        for (span, cost) in [(1, 1), (2, 1), (3, 1), (4, 1), (5, 2), (6, 1)] {
            model.charge(s(span), cost);
        }
        let origin = if taken { 100 } else { 200 };
        model.charge(s(origin), 1);
        match self {
            Self::NoSplit => model.charge(s(origin + 1), 1),
            Self::ArithmeticBounds => {
                model.charge(s(origin + 1), 1);
                model.charge(s(origin + 2), 1);
                model.effect(false, 0, Scalar::Bool(true));
            }
            Self::BoundsArithmetic => {
                model.charge(s(origin + 1), 1);
                model.effect(false, 0, Scalar::Bool(true));
                model.charge(s(origin + 2), 1);
            }
            Self::RepeatedBoundsSuffix => {
                for i in 1..=2 {
                    model.charge(s(origin + i), 1);
                    model.effect(false, 0, Scalar::Bool(true));
                }
                model.charge(s(origin + 3), 1);
            }
        }
        model.charge(s(origin + 4), 1);
        model.charge(s(300), 1);
        model.charge(s(301), 2);
        model
    }

    fn check_phi(self, text: &str, guarded: bool) {
        let expected = format!(
            "%f0_b3_merge_slot = phi ptr [ %s3, %{} ], [ %s4, %{} ]",
            self.predecessor(1, guarded),
            self.predecessor(2, guarded)
        );
        assert!(text.contains(&expected), "missing {expected}");
        assert_eq!(text.matches("load i64, ptr %f0_b3_merge_slot").count(), 1);
        assert!(!text.contains("load i64, ptr %s3,"));
        assert!(!text.contains("load i64, ptr %s4,"));
    }
}

fn continuations() -> [Continuation; 4] {
    [
        Continuation::NoSplit,
        Continuation::ArithmeticBounds,
        Continuation::BoundsArithmetic,
        Continuation::RepeatedBoundsSuffix,
    ]
}

#[test]
fn native_array_phis_name_the_actual_final_predecessor_and_load_only_the_selected_slot() {
    let (sources, s) = raw::context();
    let mut profiles = 0;
    for case in continuations() {
        for taken in [false, true] {
            let model = case.model(taken, &s);
            for guarded in [false, true] {
                let text = module(case.build(taken, &s), &sources, guarded, model.fuel());
                case.check_phi(&text, guarded);
                profiles += 1;
            }
        }
    }
    assert_eq!(profiles, 16);
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7 and one serialized native execution slot"]
fn native_array_continuations_use_source_free_elf_on_both_paths_and_every_fuel() {
    let scratch = Scratch::new();
    let (sources, s) = raw::context();
    let mut inputs = 0;
    let mut processes = 0;
    for case in continuations() {
        for taken in [false, true] {
            let model = case.model(taken, &s);
            processes += run_input(
                &scratch,
                &format!("array-phi-{inputs}"),
                &sources,
                || case.build(taken, &s),
                &model,
                &(0..=model.fuel()).collect::<Vec<_>>(),
            );
            inputs += 1;
        }
    }
    assert_eq!(inputs, 8);
    assert_eq!(processes, 288);
    eprintln!("array continuations: 8 input cases, 16 compiled artifacts, {processes} ELF executions; 8 additional preserved guarded production modules");
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Effects {
    Success,
    RhsBounds,
    IndexOverflow,
    FinalBounds,
}

impl Effects {
    fn build(self, s: &impl Fn(usize) -> Span) -> RawOwnedProgram {
        let mut caller = raw::function(0, ValueTy::Scalar(hir::Ty::I32), s(0));
        caller.locals = (0..6).map(|_| raw::scalar(hir::Ty::I32, s(0))).collect();
        caller.owners.push(owner(
            hir::Ty::I32,
            2,
            OwnerKind::Local { mutable: true },
            s(0),
        ));
        caller.calls.push(CallDecl {
            target: hir::DefId(1),
            arguments: vec![ArgumentSlot::Borrow(LoanId(0))],
            result: CallResult::Scalar(LocalId(4)),
            parent: None,
            span: s(7),
        });
        caller.loans.push(LoanDecl {
            call: CallSiteId(0),
            argument: 0,
            authority: AccessBase::Owner(OwnerPlaceId(0)),
            kind: BorrowKind::Exclusive,
            aggregate: AggregateSlot::try_from_aggregate(array(hir::Ty::I32, 2)).unwrap(),
            span: s(8),
        });
        caller.blocks.push(block(
            vec![
                raw::assign(0, Rvalue::I32(17), s(1)),
                raw::assign(1, Rvalue::I32(29), s(2)),
                raw::assign(
                    2,
                    Rvalue::I32(if self == Self::RhsBounds { 2 } else { 0 }),
                    s(3),
                ),
                raw::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), s(4)),
                raw::instruction(
                    OwnedInstruction::ConstructArray {
                        destination: OwnerPlaceId(0),
                        elements: vec![raw::operand(0, s(5)), raw::operand(1, s(5))],
                    },
                    s(5),
                ),
                // RHS scalar snapshot is completed before entering the helper
                // that computes the index and mutates the same array element.
                raw::instruction(
                    OwnedInstruction::ReadIndex {
                        destination: LocalId(3),
                        base: AccessBase::Owner(OwnerPlaceId(0)),
                        index: raw::operand(2, s(6)),
                    },
                    s(6),
                ),
                raw::instruction(OwnedInstruction::OpenCall(CallSiteId(0)), s(7)),
                raw::instruction(
                    OwnedInstruction::PrepareBorrow {
                        call: CallSiteId(0),
                        argument: 0,
                        loan: LoanId(0),
                    },
                    s(8),
                ),
            ],
            OwnedTerminatorKind::Invoke {
                call: CallSiteId(0),
                continuation: BlockId(1),
            },
            s(9),
        ));
        caller.blocks.push(block(
            vec![
                raw::instruction(
                    OwnedInstruction::WriteIndex {
                        base: AccessBase::Owner(OwnerPlaceId(0)),
                        index: raw::operand(4, s(20)),
                        value: raw::operand(3, s(20)),
                    },
                    s(20),
                ),
                raw::instruction(
                    OwnedInstruction::ReadIndex {
                        destination: LocalId(5),
                        base: AccessBase::Owner(OwnerPlaceId(0)),
                        index: raw::operand(2, s(21)),
                    },
                    s(21),
                ),
            ],
            OwnedTerminatorKind::ReturnScalar(raw::operand(5, s(22))),
            s(22),
        ));
        let mut helper = raw::function(1, ValueTy::Scalar(hir::Ty::I32), s(29));
        helper.locals = (0..5).map(|_| raw::scalar(hir::Ty::I32, s(29))).collect();
        helper
            .parameters
            .push(ParameterBinding::Reference(ReferenceParamId(0)));
        helper.references.push(ReferenceDecl {
            aggregate: AggregateSlot::try_from_aggregate(array(hir::Ty::I32, 2)).unwrap(),
            kind: BorrowKind::Exclusive,
            position: 0,
            span: s(29),
        });
        let mut statements = vec![
            raw::assign(0, Rvalue::I32(0), s(10)),
            raw::assign(1, Rvalue::I32(-71), s(11)),
            raw::instruction(
                OwnedInstruction::WriteIndex {
                    base: AccessBase::Parameter(ReferenceParamId(0)),
                    index: raw::operand(0, s(12)),
                    value: raw::operand(1, s(12)),
                },
                s(12),
            ),
            raw::assign(
                2,
                Rvalue::I32(if self == Self::FinalBounds { 2 } else { 0 }),
                s(13),
            ),
        ];
        if self == Self::IndexOverflow {
            statements.extend([
                raw::assign(3, Rvalue::I32(i32::MIN), s(14)),
                raw::assign(
                    4,
                    Rvalue::CheckedI32 {
                        op: hir::ArithmeticOp::Add,
                        left: raw::operand(3, s(15)),
                        right: raw::operand(1, s(15)),
                        operator_span: s(15),
                    },
                    s(15),
                ),
            ]);
        }
        helper.blocks.push(block(
            statements,
            OwnedTerminatorKind::ReturnScalar(raw::operand(2, s(16))),
            s(16),
        ));
        RawOwnedProgram {
            records: vec![],
            functions: vec![caller, helper],
        }
    }

    fn model(self, s: &impl Fn(usize) -> Span) -> Model {
        let mut model = Model::new(match self {
            Self::Success => Ok(Scalar::I32(17)),
            Self::RhsBounds => Err(Failure::Bounds(s(6))),
            Self::IndexOverflow => Err(Failure::Overflow(s(15))),
            Self::FinalBounds => Err(Failure::Bounds(s(20))),
        });
        // Root: S=6,A=1,P=2,O=1,L=1,C=1 gives X=27.
        model.charge(s(0), 28);
        for (span, cost) in [(1, 1), (2, 1), (3, 1), (4, 1), (5, 3), (6, 1)] {
            model.charge(s(span), cost);
        }
        if self == Self::RhsBounds {
            return model;
        }
        model.effect(false, 0, Scalar::I32(17));
        model.charge(s(7), 1);
        model.charge(s(8), 1);
        // Helper X=5 scalar slots + 8 for its one incoming reference.
        model.charge(s(9), 1 + 1 + 13);
        model.charge(s(10), 1);
        model.charge(s(11), 1);
        model.charge(s(12), 1);
        model.effect(true, 0, Scalar::I32(-71));
        model.charge(s(13), 1);
        if self == Self::IndexOverflow {
            model.charge(s(14), 1);
            model.charge(s(15), 1);
            return model;
        }
        model.charge(s(16), 2);
        // One unit must be paid before the final index can fail its bound.
        model.charge(s(20), 1);
        if self == Self::FinalBounds {
            return model;
        }
        model.effect(true, 0, Scalar::I32(17));
        model.charge(s(21), 1);
        model.effect(false, 0, Scalar::I32(17));
        model.charge(s(22), 5);
        model
    }
}

fn effect_inputs() -> [Effects; 4] {
    [
        Effects::Success,
        Effects::RhsBounds,
        Effects::IndexOverflow,
        Effects::FinalBounds,
    ]
}

#[test]
fn native_array_effect_models_keep_rhs_snapshot_and_failure_origins_explicit() {
    let (sources, s) = raw::context();
    for case in effect_inputs() {
        let model = case.model(&s);
        let text = module(case.build(&s), &sources, false, model.fuel());
        assert!(text.contains(&escaped("E0606")));
        assert_eq!(
            text.contains(&escaped("E0604")),
            case == Effects::IndexOverflow
        );
        if case == Effects::FinalBounds {
            assert_eq!(model.expected(model.fuel() - 1), Err(Failure::Fuel(s(20))));
            assert_eq!(model.expected(model.fuel()), Err(Failure::Bounds(s(20))));
        }
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7 and one serialized native execution slot"]
fn native_array_effects_use_source_free_elf_and_every_fuel() {
    let scratch = Scratch::new();
    let (sources, s) = raw::context();
    let mut processes = 0;
    for (i, case) in effect_inputs().into_iter().enumerate() {
        let model = case.model(&s);
        processes += run_input(
            &scratch,
            &format!("array-effect-{i}"),
            &sources,
            || case.build(&s),
            &model,
            &(0..=model.fuel()).collect::<Vec<_>>(),
        );
    }
    assert_eq!(processes, 229);
    eprintln!("array effects: 4 input cases, 8 compiled artifacts, {processes} ELF executions; 4 additional preserved guarded production modules. Intermediate effect traces are reference observations; native execution proves final results and exact failure diagnostics");
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7 and one serialized native execution slot"]
fn native_arrays_extreme_payloads_and_production_guarded_wrappers_use_real_llvm() {
    let scratch = Scratch::new();
    let (sources, s) = raw::context();
    let mut inputs = 0;
    let mut processes = 0;
    for access in [Access::Read, Access::Write] {
        for index in [0, 1] {
            let case = Core {
                ty: hir::Ty::I32,
                n: 2,
                index,
                access,
                extremes: true,
            };
            let model = case.model(&s);
            processes += run_input(
                &scratch,
                &format!("array-extreme-{inputs}"),
                &sources,
                || case.build(&s),
                &model,
                &(0..=model.fuel()).collect::<Vec<_>>(),
            );
            inputs += 1;
        }
    }
    let mut wrappers = 0;
    for (i, case) in [
        Core {
            ty: hir::Ty::I32,
            n: 2,
            index: 1,
            access: Access::Read,
            extremes: false,
        },
        Core {
            ty: hir::Ty::Unit,
            n: 0,
            index: 0,
            access: Access::Write,
            extremes: false,
        },
    ]
    .into_iter()
    .enumerate()
    {
        let model = case.model(&s);
        for fuel in [model.fuel() - 1, model.fuel(), plan::MAX_FUEL] {
            let production = module(case.build(&s), &sources, true, fuel);
            let binary = scratch.compile(&production, &format!("array-production-{i}-{fuel}"));
            check_process(&scratch, &binary, &[], model.expected(fuel), &sources);
            wrappers += 1;
            processes += 1;
        }
    }
    assert_eq!((inputs, wrappers), (4, 6));
    assert_eq!(processes, 112);
    eprintln!("array extreme payloads and guarded wrappers: 4 new input cases plus 2 reused core inputs, 14 compiled artifacts, {processes} ELF executions; 4 additional preserved guarded production modules");
}
