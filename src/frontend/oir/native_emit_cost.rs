//! Disconnected checked body-work formula for the private Emit precursor.
//!
//! This is the conservative H = 71 + 6p proposal's finite, weighted source-event
//! model, not a CPU, elapsed-time, allocator-backend, storage, or RSS bound.
//! Inputs must be the actual immutable scan dimensions and the already-owned
//! root display path's UTF-8 byte length. Genuine association with one verified
//! source of at most 128 bytes and Result policy remain caller obligations.
//! No scan, path walk, emitter, owner, witness, allocation, or meter is used here.
//! A successful calculation grants neither native nor output admission. Its
//! complete named carrier inventory needs outside payment before entry, and its
//! body charge needs a later debit on the original meter before native entry.
use super::emit_work::{Dimensions, Failure};
use std::mem::{size_of, size_of_val};

/// Fixed work breakdown. The separately paid dimension scan is excluded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend::oir) struct Cost {
    pub(in crate::frontend::oir) admission: u64,
    pub(in crate::frontend::oir) unguarded: u64,
    pub(in crate::frontend::oir) guarded: Option<u64>,
    pub(in crate::frontend::oir) finish: u64,
    pub(in crate::frontend::oir) body: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DiagnosticCost {
    human_bytes: u64,
    render: u64,
    escape: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ModeCost {
    template_bytes: u64,
    traversal: u64,
    passes: u64,
}

fn add(left: u64, right: u64) -> Result<u64, Failure> {
    left.checked_add(right).ok_or(Failure::Overflow)
}

fn mul(left: u64, right: u64) -> Result<u64, Failure> {
    left.checked_mul(right).ok_or(Failure::Overflow)
}

fn weighted<const N: usize>(base: u64, terms: [(u64, u64); N]) -> Result<u64, Failure> {
    let mut total = base;
    for (weight, count) in terms {
        total = add(total, mul(weight, count)?)?;
    }
    Ok(total)
}

/// Only incidence relations guaranteed by the scan and verified scalar OIR.
/// These are not native caps, grammar restrictions, or proof of a CFG cycle.
fn validate(dimensions: Dimensions) -> Result<(), Failure> {
    if dimensions.functions > dimensions.blocks
        || dimensions.parameters > dimensions.locals
        || dimensions.merges > dimensions.blocks
        || dimensions.calls > dimensions.blocks
        || dimensions.calls > dimensions.edges
        || dimensions.edges > mul(2, dimensions.blocks)?
        || dimensions.arithmetic_failures > mul(2, dimensions.statements)?
        || (dimensions.maybe_cyclic && dimensions.edges == 0)
    {
        return Err(Failure::Invariant);
    }
    Ok(())
}

fn diagnostic_cost(path_utf8_len: usize) -> Result<DiagnosticCost, Failure> {
    let path = u64::try_from(path_utf8_len).map_err(|_| Failure::Overflow)?;
    let human_bytes = add(71, mul(6, path)?)?;
    let render = add(4_096, mul(128, add(path, human_bytes)?)?)?;
    let escape = mul(128, human_bytes)?;
    Ok(DiagnosticCost {
        human_bytes,
        render,
        escape,
    })
}

fn mode_cost(dimensions: Dimensions, guards: u64, diagnostics: u64) -> Result<ModeCost, Failure> {
    let template_bytes = weighted(
        2_048,
        [
            (128, dimensions.functions),
            (32, dimensions.parameters),
            (64, dimensions.places),
            (128, dimensions.blocks),
            (256, dimensions.merges),
            (2_048, dimensions.statements),
            (128, dimensions.calls),
            (32, dimensions.arguments),
            (1_024, guards),
            (256, diagnostics),
        ],
    )?;
    let traversal = weighted(
        0,
        [
            (4, dimensions.functions),
            (5, dimensions.blocks),
            (6, dimensions.statements),
            (1, dimensions.parameters),
            (1, dimensions.places),
            (1, dimensions.arguments),
            (1, dimensions.merges),
            (1, dimensions.calls),
            (1, guards),
            (1, diagnostics),
            (1, dimensions.locals),
        ],
    )?;
    let passes = mul(2, add(mul(128, traversal)?, mul(64, template_bytes)?)?)?;
    Ok(ModeCost {
        template_bytes,
        traversal,
        passes,
    })
}

/// Returns WA, Wu, optional Wg, Wfinish, and their selected body total.
/// q merely selects max(Wu, Wg); it asserts neither an actual cycle nor native
/// admission. When q is false and K is zero, even usize::MAX is an irrelevant
/// path length: no path conversion or H/R/X term is evaluated.
pub(in crate::frontend::oir) fn calculate(
    dimensions: Dimensions,
    path_utf8_len: usize,
) -> Result<Cost, Failure> {
    validate(dimensions)?;
    let admission = add(
        4_096,
        mul(
            128,
            weighted(
                0,
                [
                    (16, dimensions.functions),
                    (24, dimensions.blocks),
                    (4, dimensions.statements),
                    (8, dimensions.calls),
                    (8, dimensions.edges),
                ],
            )?,
        )?,
    )?;
    let message = if dimensions.maybe_cyclic || dimensions.arithmetic_failures != 0 {
        Some(diagnostic_cost(path_utf8_len)?)
    } else {
        None
    };
    let unguarded_mode = mode_cost(dimensions, 0, dimensions.arithmetic_failures)?;
    let unguarded = match message {
        Some(message) => add(
            unguarded_mode.passes,
            add(
                mul(mul(4, dimensions.arithmetic_failures)?, message.render)?,
                mul(mul(2, dimensions.arithmetic_failures)?, message.escape)?,
            )?,
        )?,
        None => unguarded_mode.passes,
    };
    let (guarded, finish_diagnostics) = if dimensions.maybe_cyclic {
        let guards = weighted(
            1,
            [
                (1, dimensions.merges),
                (1, dimensions.statements),
                (1, dimensions.blocks),
            ],
        )?;
        let diagnostics = add(guards, dimensions.arithmetic_failures)?;
        let guarded_mode = mode_cost(dimensions, guards, diagnostics)?;
        let message = message.ok_or(Failure::Invariant)?;
        // Rust 1.99's audited fixed tuple-key BTreeMap source-event tariff:
        // <=4D operations plus one map teardown, within 512D(D+1). This is
        // implementation-specific, not an API big-O or allocator-time claim.
        let dictionary = mul(mul(512, diagnostics)?, add(diagnostics, 1)?)?;
        let guarded = add(
            guarded_mode.passes,
            add(
                dictionary,
                add(
                    mul(mul(2, diagnostics)?, message.render)?,
                    mul(mul(2, diagnostics)?, message.escape)?,
                )?,
            )?,
        )?;
        (
            Some(guarded),
            dimensions.arithmetic_failures.max(diagnostics),
        )
    } else {
        (None, dimensions.arithmetic_failures)
    };
    let selected = match guarded {
        Some(value) => value.max(unguarded),
        None => unguarded,
    };
    let finish = add(
        4_096,
        mul(
            128,
            weighted(
                0,
                [
                    (1, dimensions.functions),
                    (1, dimensions.blocks),
                    (1, dimensions.statements),
                    (1, dimensions.locals),
                    (1, dimensions.places),
                    (1, dimensions.arguments),
                    (1, finish_diagnostics),
                ],
            )?,
        )?,
    )?;
    let body = add(add(admission, selected)?, finish)?;
    Ok(Cost {
        admission,
        unguarded,
        guarded,
        finish,
        body,
    })
}

type FormulaInputs = (Dimensions, usize);
type BinaryInputs = (u64, u64);

/// Inventory only: distinct named source roles, without assuming stack-slot
/// reuse or ABI elision. These types are never allocated or passed to emit.
#[allow(dead_code)]
struct BinaryCall {
    arguments: BinaryInputs,
    returned: Result<u64, Failure>,
    success: u64,
    failure: Failure,
}

#[allow(dead_code)]
struct ArithmeticLocals {
    inputs: BinaryInputs,
    left: u64,
    right: u64,
    checked_arguments: BinaryInputs,
    checked_result: Option<u64>,
    ok_or_arguments: (Option<u64>, Failure),
    returned: Result<u64, Failure>,
}

/// N is each actual weighted array's length, including its actual IntoIter.
/// The enclosing input tuples and every array move remain full-size objects.
#[allow(dead_code)]
struct WeightedRoles<const N: usize> {
    caller_arguments: (u64, [(u64, u64); N]),
    caller_result: Result<u64, Failure>,
    inputs: (u64, [(u64, u64); N]),
    base: u64,
    terms: [(u64, u64); N],
    total: u64,
    into_iter_argument: [(u64, u64); N],
    constructor_result: std::array::IntoIter<(u64, u64), N>,
    cursor: std::array::IntoIter<(u64, u64), N>,
    next_result: Option<(u64, u64)>,
    current_tuple: (u64, u64),
    weight: u64,
    count: u64,
    multiply: BinaryCall,
    accumulate: BinaryCall,
    returned: Result<u64, Failure>,
}

#[allow(dead_code)]
struct FormulaLocals {
    dimensions: Dimensions,
    path_utf8_len: usize,
    admission: u64,
    message: Option<DiagnosticCost>,
    unguarded_mode: ModeCost,
    unguarded_message_binding: DiagnosticCost,
    unguarded: u64,
    guards: u64,
    diagnostics: u64,
    guarded_mode: ModeCost,
    guarded_message_binding: DiagnosticCost,
    dictionary: u64,
    guarded_value: u64,
    branch_tuple: (Option<u64>, u64),
    guarded: Option<u64>,
    finish_diagnostics: u64,
    selected_value_binding: u64,
    selected: u64,
    finish: u64,
    body: u64,
}

#[allow(dead_code)]
struct DiagnosticLocals {
    caller_arguments: (usize,),
    caller_result: Result<DiagnosticCost, Failure>,
    path_utf8_len: usize,
    conversion_arguments: (usize,),
    conversion_result: Result<u64, std::num::TryFromIntError>,
    conversion_error: std::num::TryFromIntError,
    mapped_result: Result<u64, Failure>,
    path: u64,
    human_bytes: u64,
    render: u64,
    escape: u64,
    constructed: DiagnosticCost,
    returned: Result<DiagnosticCost, Failure>,
}

#[allow(dead_code)]
struct ModeLocals {
    // Both call sites have full arguments/results, even though the callee's
    // named roles are reusable. WeightedRoles separately measures T and V.
    caller_arguments: [(Dimensions, u64, u64); 2],
    caller_results: [Result<ModeCost, Failure>; 2],
    inputs: (Dimensions, u64, u64),
    dimensions: Dimensions,
    guards: u64,
    diagnostics: u64,
    template_bytes: u64,
    traversal: u64,
    passes: u64,
    constructed: ModeCost,
    returned: Result<ModeCost, Failure>,
}

fn byte_add(left: usize, right: usize) -> Result<usize, Failure> {
    left.checked_add(right).ok_or(Failure::Overflow)
}

/// Complete local named carrier inventory, disconnected and unpaid here.
/// A future caller must also measure its full enclosing mode/input/result and
/// ownership carriers and admit both storage banks before formula entry. This
/// inventory excludes inherited owner payloads, arbitrary compiler/library
/// stack internals, allocator internals, and test-only carriers. No enclosing
/// enum is replaced by a sum of just one variant's fields.
pub(in crate::frontend::oir) fn named_bytes() -> Result<usize, Failure> {
    let roles = [
        // Complete caller transport, callee input and all local bindings.
        size_of::<FormulaInputs>(),
        size_of::<FormulaInputs>(),
        size_of::<FormulaLocals>(),
        // Constructed payload, success move, callee and caller results.
        size_of::<Cost>(),
        size_of::<Cost>(),
        size_of::<Result<Cost, Failure>>(),
        size_of::<Result<Cost, Failure>>(),
        // validate caller/callee inputs, named input and complete results.
        size_of::<(Dimensions,)>(),
        size_of::<(Dimensions,)>(),
        size_of::<Dimensions>(),
        size_of::<Result<(), Failure>>(),
        size_of::<Result<(), Failure>>(),
        // Seven scalar incidence comparisons plus the q/edge condition.
        size_of::<[(u64, u64, bool); 7]>(),
        size_of::<(bool, u64, bool, bool)>(),
        size_of::<DiagnosticLocals>(),
        size_of::<ModeLocals>(),
        // Exact concrete weighted arrays: WA, G, finish, T and V.
        size_of::<WeightedRoles<5>>(),
        size_of::<WeightedRoles<3>>(),
        size_of::<WeightedRoles<7>>(),
        size_of::<WeightedRoles<10>>(),
        size_of::<WeightedRoles<11>>(),
        // Every direct add/mul call site outside weighted: diagnostic has
        // 3/3, mode has 1/3, validate has 0/2, calculate has 11/12.
        size_of::<[BinaryCall; 15]>(),
        size_of::<[BinaryCall; 20]>(),
        // Reusable add and mul helper frames, including checked Option and
        // full mapped Result. weighted's call sites are in WeightedRoles.
        size_of::<ArithmeticLocals>(),
        size_of::<ArithmeticLocals>(),
        // Both max inputs/results and guarded message extraction transport.
        size_of::<[BinaryInputs; 2]>(),
        size_of::<[u64; 2]>(),
        size_of::<(Option<DiagnosticCost>, Failure)>(),
        size_of::<Result<DiagnosticCost, Failure>>(),
        // Failure expressions: add, mul, conversion, validate, guarded
        // extraction and byte_add. Complete enclosing Results are above.
        size_of::<[Failure; 6]>(),
        // This inventory's total/current bytes, four byte_add call sites,
        // helper bindings, checked Option/ok_or and enclosing results.
        size_of::<usize>(),
        size_of::<usize>(),
        size_of::<[(usize, usize); 4]>(),
        size_of::<[Result<usize, Failure>; 4]>(),
        size_of::<(usize, usize)>(),
        size_of::<(usize, usize)>(),
        size_of::<Option<usize>>(),
        size_of::<(Option<usize>, Failure)>(),
        size_of::<Result<usize, Failure>>(),
        size_of::<Result<usize, Failure>>(),
        size_of::<Option<usize>>(),
    ];
    // The actual concrete inventory array is retained in full, with its move
    // input, constructor result, held IntoIter and complete next transport.
    let mut total = byte_add(size_of_val(&roles), size_of_val(&roles))?;
    total = byte_add(total, size_of_val(&roles.into_iter()))?;
    total = byte_add(total, size_of_val(&roles.into_iter()))?;
    for bytes in roles {
        total = byte_add(total, bytes)?;
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::declaration_index::IndexLimits;

    fn literal() -> Dimensions {
        Dimensions {
            functions: 1,
            locals: 1,
            blocks: 1,
            statements: 1,
            ..Dimensions::default()
        }
    }

    fn division() -> Dimensions {
        Dimensions {
            locals: 3,
            statements: 3,
            arithmetic_failures: 2,
            ..literal()
        }
    }

    fn simple_loop() -> Dimensions {
        Dimensions {
            functions: 1,
            locals: 2,
            blocks: 4,
            statements: 2,
            edges: 4,
            maybe_cyclic: true,
            ..Dimensions::default()
        }
    }

    fn rich() -> Dimensions {
        Dimensions {
            functions: 2,
            locals: 12,
            places: 1,
            parameters: 1,
            blocks: 9,
            statements: 11,
            merges: 0,
            calls: 2,
            arguments: 2,
            edges: 8,
            arithmetic_failures: 1,
            maybe_cyclic: true,
        }
    }

    #[test]
    fn disconnected_emit_cost_literal_path_is_never_evaluated() {
        // Independent report total 580,096 less its separately paid 4,608
        // scan. This control would fail if unnecessary H/R/X overflowed.
        let expected = Cost {
            admission: 9_728,
            unguarded: 561_152,
            guarded: None,
            finish: 4_608,
            body: 575_488,
        };
        for path in [0, 7, 1_024, 4_096, usize::MAX] {
            assert_eq!(calculate(literal(), path), Ok(expected));
        }
    }

    #[test]
    fn disconnected_emit_cost_rich_report_examples() {
        // Fixed totals from emit-work-bound-plan.md, independently derived
        // before this implementation. The scan is never inside Cost.body.
        for (path, expected_extra) in [(7, 8_721_664), (1_024, 83_182_336), (4_096, 308_101_888)] {
            let cost = calculate(rich(), path).unwrap();
            assert_eq!(cost.admission, 51_712);
            assert_eq!(cost.finish, 11_648);
            assert_eq!(cost.body.checked_add(8_064), Some(expected_extra));
            assert_eq!(
                cost.body,
                cost.admission
                    .checked_add(cost.unguarded.max(cost.guarded.unwrap()))
                    .unwrap()
                    .checked_add(cost.finish)
                    .unwrap()
            );
        }
    }

    #[test]
    fn disconnected_emit_cost_division_and_loop_report_examples() {
        for (path, expected_division_extra, expected_loop_extra) in [
            (7, 1_389_568, 2_567_296),
            (1_024, 11_803_648, 26_259_328),
            (4_096, 43_260_928, 97_824_640),
        ] {
            let division = calculate(division(), path).unwrap();
            let simple_loop = calculate(simple_loop(), path).unwrap();
            assert_eq!(division.guarded, None);
            assert!(simple_loop.guarded.is_some());
            assert_eq!(
                division.body.checked_add(4_864),
                Some(expected_division_extra)
            );
            assert_eq!(
                simple_loop.body.checked_add(5_504),
                Some(expected_loop_extra)
            );
        }
    }

    #[test]
    fn disconnected_emit_cost_keeps_conservative_q_selector() {
        // A false-positive backward-edge selector is deliberately priced in
        // the same way as a real loop; no graph or native admission is read.
        let dimensions = Dimensions {
            functions: 1,
            locals: 3,
            blocks: 4,
            statements: 3,
            merges: 2,
            edges: 4,
            maybe_cyclic: true,
            ..Dimensions::default()
        };
        let selected = calculate(dimensions, 7).unwrap();
        let only_unguarded = calculate(
            Dimensions {
                maybe_cyclic: false,
                ..dimensions
            },
            7,
        )
        .unwrap();
        assert_eq!(selected.unguarded, only_unguarded.unguarded);
        assert!(selected.guarded.unwrap() >= selected.unguarded);
        assert_eq!(only_unguarded.guarded, None);
        assert!(selected.body > only_unguarded.body);

        // Guarded does not universally dominate: many division occurrences
        // cause unguarded's repeated human renders to outweigh guarded work.
        // The formula still returns data above the work ceiling; only a later
        // original-meter debit can admit or deny execution of that body.
        let arithmetic_heavy = calculate(
            Dimensions {
                functions: 1,
                locals: 300,
                blocks: 2,
                statements: 100,
                edges: 1,
                arithmetic_failures: 200,
                maybe_cyclic: true,
                ..Dimensions::default()
            },
            65_536,
        )
        .unwrap();
        assert_eq!(arithmetic_heavy.unguarded, 67_156_410_880);
        assert_eq!(arithmetic_heavy.guarded, Some(66_196_404_736));
        assert_eq!(
            arithmetic_heavy.body,
            arithmetic_heavy
                .admission
                .checked_add(arithmetic_heavy.unguarded)
                .unwrap()
                .checked_add(arithmetic_heavy.finish)
                .unwrap()
        );
    }

    #[test]
    fn disconnected_emit_cost_uses_existing_ceiling_as_data_only() {
        // Historical rich pre-emission work, already including entry charge.
        // This arithmetic is a data oracle, not a same-meter integration test
        // or a new receipt that any native body was admitted or executed.
        let prior = 1_310_659_u64;
        let scan = 8_064_u64;
        let ceiling = IndexLimits::default().work;
        assert_eq!(ceiling, 256_000_000);
        for (path, expected_total, fits) in [
            (7, 10_032_323, true),
            (1_024, 84_492_995, true),
            (4_096, 309_412_547, false),
            (3_366, 255_964_867, true),
            (3_367, 256_038_083, false),
        ] {
            let total = prior
                .checked_add(scan)
                .unwrap()
                .checked_add(calculate(rich(), path).unwrap().body)
                .unwrap();
            assert_eq!(total, expected_total);
            assert_eq!(total <= ceiling, fits);
        }
    }

    #[test]
    fn disconnected_emit_cost_checked_overflow() {
        assert_eq!(add(u64::MAX, 1), Err(Failure::Overflow));
        assert_eq!(mul(u64::MAX, 2), Err(Failure::Overflow));
        assert_eq!(weighted(u64::MAX, [(1, 1)]), Err(Failure::Overflow));
        assert_eq!(weighted(0, [(2, u64::MAX)]), Err(Failure::Overflow));
        assert_eq!(byte_add(usize::MAX, 1), Err(Failure::Overflow));
        for dimensions in [
            Dimensions {
                locals: u64::MAX,
                ..literal()
            },
            Dimensions {
                places: u64::MAX,
                ..literal()
            },
            Dimensions {
                arguments: u64::MAX,
                ..literal()
            },
            Dimensions {
                blocks: u64::MAX,
                ..literal()
            },
            Dimensions {
                statements: u64::MAX,
                ..literal()
            },
        ] {
            assert_eq!(calculate(dimensions, 7), Err(Failure::Overflow));
        }
        // On the qualified 64-bit target, both required-path branches reject
        // overflow. 32-bit usize::MAX does not itself overflow these formulas.
        if usize::BITS == 64 {
            assert_eq!(calculate(division(), usize::MAX), Err(Failure::Overflow));
            assert_eq!(calculate(simple_loop(), usize::MAX), Err(Failure::Overflow));
        }
    }

    #[test]
    fn disconnected_emit_cost_rejects_impossible_incidence_only() {
        for dimensions in [
            Dimensions {
                functions: 2,
                ..literal()
            },
            Dimensions {
                parameters: 2,
                ..literal()
            },
            Dimensions {
                merges: 2,
                ..literal()
            },
            Dimensions {
                calls: 2,
                ..literal()
            },
            Dimensions {
                calls: 1,
                ..literal()
            },
            Dimensions {
                edges: 3,
                ..literal()
            },
            Dimensions {
                arithmetic_failures: 3,
                ..literal()
            },
            Dimensions {
                maybe_cyclic: true,
                ..literal()
            },
        ] {
            assert_eq!(calculate(dimensions, 7), Err(Failure::Invariant));
        }
        // Native admission owns its caps. The passive arithmetic does not
        // impose them and can price a scalar shape beyond the function cap.
        assert!(calculate(
            Dimensions {
                functions: 257,
                blocks: 257,
                ..Dimensions::default()
            },
            usize::MAX
        )
        .is_ok());
    }

    #[test]
    fn disconnected_emit_cost_named_layout_needs_outside_payment() {
        let named = named_bytes().unwrap();
        assert!(named > size_of::<FormulaInputs>() + size_of::<Cost>());
        assert!(named < 16 * 1024 * 1024);
        println!(
            "PRIVATE_EMIT_COST disconnected_not_yet_integrated paid_outside named={} inputs={} cost={} result={} locals={} diagnostic={} mode={} weighted5={} weighted3={} weighted7={} weighted10={} weighted11={}",
            named, size_of::<FormulaInputs>(), size_of::<Cost>(),
            size_of::<Result<Cost, Failure>>(), size_of::<FormulaLocals>(),
            size_of::<DiagnosticLocals>(), size_of::<ModeLocals>(),
            size_of::<WeightedRoles<5>>(), size_of::<WeightedRoles<3>>(),
            size_of::<WeightedRoles<7>>(), size_of::<WeightedRoles<10>>(),
            size_of::<WeightedRoles<11>>()
        );
    }
}
