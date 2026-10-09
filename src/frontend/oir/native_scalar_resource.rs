//! RFC0030's additive native-local bank and finite source-event containment.
//!
//! This is a repository-owned named-carrier model, not stack size or RSS. It
//! sums branch-specific and sequential transports without ABI elision. Existing
//! graph/diagnostic/label owners remain inherited. Opaque standard formatting
//! frames and allocator internals remain excluded. NEW compiler-generated
//! callsite backing arrays, individual argument returns, descriptor borrows and
//! referent transports are explicitly included from qualified Rust1.99 MIR.
//! No newly retained owner is introduced.
use super::*;
use std::mem::size_of;

pub(super) const INVALID_TYPES: &str =
    "internal compiler error: invalid native scalar operation types";

/// One source event is one fixed field/index/tag/predicate/branch/move operation
/// or one copied ASCII message byte; this is not machine instructions or time.
/// The caller's closed Statement/Rvalue dispatch and helper prologue/return fit
/// 32+16 events. A local lookup has no loop: id/locals/slice/get bounds+index,
/// Option branch/map and type projection fit 16. At most three lookups execute
/// (destination + two operands); malformed IDs short-circuit or return None.
/// At most eight scalar Option/type predicates execute after closed dispatch.
/// Diagnostic construction has six fixed fields, empty Vecs and one String
/// copy: 64 fixed events plus four per ASCII byte conservatively cover all
/// repository-owned setup/copy/result roles, excluding allocation backend work.
/// Charging the error path even on success gives 440 <= inherited 4*128=512.
#[cfg(test)]
pub(super) const ADMISSION_EVENTS: usize = 32 + 3 * 16 + 8 * 4 + 16 + 64 + 4 * INVALID_TYPES.len();
#[cfg(test)]
pub(super) const ADMISSION_TARIFF: usize = 4 * 128;

/// The six inherited per-pass statement buckets remain: metadata candidate
/// lookup, checked-failure classification, exit-label reverse lookup, body
/// traversal, opcode/predicate selection and result/write setup. Each bucket
/// receives128 source events in mode_cost; finite filter/map callbacks are
/// included, not treated as free. New branch setup has only fixed reads/matches
/// and at most four output requests, well below128. Narrowing has one checked
/// failure, below division/remainder's two; diagnostic loops are priced by K/J.
/// Output processing is separately priced at64 events per template byte. There
/// is no new OIR traversal, dynamic rhs String, or source/name scan. Widening has
/// no failure or label split. Admission is separately covered by WA above.
#[cfg(test)]
pub(super) const STATEMENT_VISITS: usize = 6;
#[cfg(test)]
pub(super) const TEMPLATE_BYTES: usize = 2_048;

#[allow(dead_code)]
struct LookupRoles {
    // Caller-site transports are listed separately below; helper frame once.
    input: LocalId,
    captured_function: &'static Function,
    get_input: (&'static [LocalDecl], usize),
    get_result: Option<&'static LocalDecl>,
    map_input: Option<&'static LocalDecl>,
    mapping_closure: (), // noncapturing |decl| decl.ty
    mapping_input: &'static LocalDecl,
    mapping_result: hir::Ty,
    returned: Option<hir::Ty>,
}

#[allow(dead_code)]
struct AdmissionRoles {
    caller_and_callee: [(&'static Function, &'static Assign); 2],
    results: [Result<(), Box<Diagnostic>>; 2],
    // The actual closure capture is measured by record_closure in test builds.
    closure: &'static Function,
    destination: Option<hir::Ty>,
    valid: bool,
    // Ten syntactic local(...) call sites, although no path executes over 3.
    lookup_calls: [(&'static (), LocalId); 10],
    lookup_results: [Option<hir::Ty>; 10],
    lookup_frame: LookupRoles,
    // Sum branch-specific named bindings; never assume their stack reuse.
    conversion_operands: [Operand; 2],
    comparison_pattern: (hir::ComparisonOp, Operand, Operand),
    comparison_types: [Option<hir::Ty>; 2],
    comparison_allowed: bool,
    not_operand: Operand,
    negate_operand: Operand,
    arithmetic_operands: [Operand; 2],
    copy_operand: Operand,
    load_place: Place,
    place_get_input: (&'static [PlaceDecl], usize),
    place_get_result: Option<&'static PlaceDecl>,
    place_mapping_input: &'static PlaceDecl,
    place_mapping_result: hir::Ty,
    place_mapped_result: Option<hir::Ty>,
    diagnostic_inputs: (&'static str, &'static str, &'static str, Option<Span>),
    diagnostic_return: Box<Diagnostic>,
    // Include the new error's complete Box payload and sole heap String data.
    // String::from(&str) requests exactly len; physical rounding is excluded.
    diagnostic_payload: Diagnostic,
    diagnostic_message: [u8; INVALID_TYPES.len()],
}

type FailureInputs = (
    &'static mut Emission,
    Option<&'static GuardedDiagnostics>,
    FailureKind,
    Span,
    &'static SourceMap,
    usize,
    usize,
);

type NumericWriteInputs = (&'static mut Emission, usize, usize);

// Rust1.99.0 b940084d7 print-type-sizes: core::fmt::rt::Argument and
// ArgumentType are16/align8 on qualified Linux x86_64. Stable code cannot name
// that private type. This repr(C) two-pointer surrogate prices its full carrier;
// rerun the MIR/layout qualification when the prescribed toolchain changes.
#[repr(C)]
#[allow(dead_code)]
struct FormatArgumentCarrier {
    value: *const (),
    formatter: *const (),
}

#[allow(dead_code)]
struct GeneratedFormatRoles<const N: usize> {
    // MIR: N individual new_display return objects, followed by [Argument;N].
    // Count both in full even though one is moved into the other.
    constructor_returns: [FormatArgumentCarrier; N],
    argument_array: [FormatArgumentCarrier; N],
    // MIR: initial borrows, complete args tuple, and constructor input borrows.
    // Include the constructor call transport too, with no ABI-elision credit.
    initial_borrows: [*const (); N],
    args_tuple: [*const (); N],
    constructor_borrows: [*const (); N],
    constructor_calls: [*const (); N],
    template_borrow: *const (), // &[u8;L] is a thin borrowed static descriptor.
    argument_array_borrow: *const (), // &[Argument;N], also thin.
    arguments_constructor: (*const (), *const ()),
    arguments_return: std::fmt::Arguments<'static>,
    // The compact placeholder byte-program is immutable compiler data, not a
    // new runtime allocation or dynamic Placeholder array. Its borrow is above.
}

#[allow(dead_code)]
struct FormattingRequest<const N: usize> {
    // Complete write_fmt transport, argument object and returned fmt::Result.
    caller_and_callee: [(&'static mut Emission, std::fmt::Arguments<'static>); 2],
    arguments: std::fmt::Arguments<'static>,
    result: std::fmt::Result,
    // Three numeric referents suffice for the widest new call; count all three
    // even for the two-referent widening/value writes. Referents are borrowed.
    referents: [usize; 3],
    referent_transport: [&'static usize; 3],
    // Comparison's two inherited textual selectors are included in full too.
    text_referents: [&'static str; 2],
    generated: GeneratedFormatRoles<N>,
}

#[allow(dead_code)]
struct EmissionRoles {
    narrow_operand: Operand,
    narrow_name: Span,
    narrow_destination: usize,
    // Actual helper caller/callee transports for check, success and widening.
    numeric_calls: [NumericWriteInputs; 6],
    numeric_results: [(); 6],
    numeric_bindings: [(usize, usize); 3],
    failure_inputs: [FailureInputs; 2],
    failure_result: (),
    widen_operand: Operand,
    comparison_type: hir::Ty,
    comparison_unsigned: bool,
    comparison_selector: (hir::ComparisonOp, bool),
    // Four source format sites: checks, truncation, widening, comparison.
    checks_formatting: FormattingRequest<2>,
    trunc_formatting: FormattingRequest<2>,
    widen_formatting: FormattingRequest<2>,
    comparison_formatting: FormattingRequest<5>,
}

#[allow(dead_code)]
struct FixedPreflightRoles {
    receiver: &'static OutputMode<'static>,
    admission: &'static private_emit::Admission<'static>,
    call_and_input: [(&'static private_emit::Admission<'static>, usize); 2],
    results: [Result<(), EmitFailure>; 3],
}

#[allow(dead_code)]
struct NativeScalarSuccessorRoles {
    admission: AdmissionRoles,
    emission: EmissionRoles,
    preflight: FixedPreflightRoles,
    // named_bytes is a const sizeof, with its new caller-held usize result.
    inventory_return: usize,
}

pub(in crate::frontend::oir) const fn named_bytes() -> usize {
    size_of::<NativeScalarSuccessorRoles>()
}

#[cfg(test)]
thread_local! {
    static CALLS: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
}
#[cfg(test)]
pub(super) fn reset() {
    CALLS.with(|calls| calls.set((0, 0)));
}
#[cfg(test)]
pub(super) fn calls() -> (usize, usize) {
    CALLS.with(std::cell::Cell::get)
}
#[cfg(test)]
pub(super) fn record_closure<F>(closure: &F) {
    assert_eq!(std::mem::size_of_val(closure), size_of::<&Function>());
    CALLS.with(|calls| {
        let (assignments, lookups) = calls.get();
        calls.set((assignments + 1, lookups));
    });
}
#[cfg(test)]
pub(super) fn lookup() {
    CALLS.with(|calls| {
        let (assignments, lookups) = calls.get();
        calls.set((assignments, lookups + 1));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn u8_native_roles_and_finite_containment_are_measured() {
        assert_eq!(ADMISSION_EVENTS, 440);
        assert_eq!(ADMISSION_TARIFF, 512);
        const { assert!(ADMISSION_EVENTS <= ADMISSION_TARIFF) };
        assert_eq!(STATEMENT_VISITS, 6);
        assert_eq!(TEMPLATE_BYTES, 2048);
        assert_eq!(
            (
                size_of::<FormatArgumentCarrier>(),
                std::mem::align_of::<FormatArgumentCarrier>()
            ),
            (16, 8)
        );
        assert_eq!(size_of::<AdmissionRoles>(), 960);
        assert_eq!(size_of::<LookupRoles>(), 72);
        assert_eq!(size_of::<GeneratedFormatRoles<2>>(), 176);
        assert_eq!(size_of::<GeneratedFormatRoles<5>>(), 368);
        assert_eq!(size_of::<FormattingRequest<2>>(), 328);
        assert_eq!(size_of::<FormattingRequest<5>>(), 520);
        assert_eq!(size_of::<EmissionRoles>(), 1944);
        assert_eq!(size_of::<FixedPreflightRoles>(), 96);
        assert_eq!(named_bytes(), 960 + 1944 + 96 + 8);
        println!("U8_NATIVE_ROLE_BANK total={} admission={} lookup={} emission={} format2={} format5={} generated2={} generated5={} preflight={} fmt_arguments={} failure_inputs={} events={}/{}", named_bytes(), size_of::<AdmissionRoles>(), size_of::<LookupRoles>(), size_of::<EmissionRoles>(), size_of::<FormattingRequest<2>>(), size_of::<FormattingRequest<5>>(), size_of::<GeneratedFormatRoles<2>>(), size_of::<GeneratedFormatRoles<5>>(), size_of::<FixedPreflightRoles>(), size_of::<std::fmt::Arguments<'_>>(), size_of::<FailureInputs>(), ADMISSION_EVENTS, ADMISSION_TARIFF);
    }

    #[test]
    fn u8_native_max_numeric_templates_fit_existing_byte_envelope() {
        let mut output = Emission::default();
        write_u8_checks(&mut output, usize::MAX, usize::MAX);
        let checks = output.len;
        write_u8_value(&mut output, usize::MAX, usize::MAX);
        let narrow = output.len;
        let mut sources = SourceMap::new();
        let file = sources.add(String::new(), String::new());
        let span = sources.get(file).span(0, 0);
        emit_arithmetic_failure(
            &mut output,
            None,
            FailureKind::ByteRange,
            span,
            &sources,
            usize::MAX,
            usize::MAX,
        );
        // The diagnostic length is the remaining numeric field. Reserve its
        // full usize decimal width as well, even though this fixture is short.
        let complete_narrow = output.len + usize::MAX.to_string().len();
        assert!(complete_narrow < TEMPLATE_BYTES);
        let mut output = Emission::default();
        write_u8_widen(&mut output, usize::MAX, usize::MAX);
        assert!(output.len < TEMPLATE_BYTES);
        println!("U8_NATIVE_TEMPLATE_BOUND checks={checks} checks_and_value={narrow} narrow_with_failure_max_numeric={complete_narrow} widen={} ceiling={TEMPLATE_BYTES}", output.len);
    }

    #[test]
    fn u8_native_admission_census_covers_all_types_opcodes_and_bad_ids() {
        let (verified, _) = super::super::tests::verified_with_sources("fn main()->i32{return 0;}");
        let mut function = verified.program.functions[0].clone();
        let span = function.span;
        function.locals.resize(3, function.locals[0].clone());
        function.places.push(PlaceDecl {
            ty: hir::Ty::U8,
            span,
        });
        let mut attempts = 0;
        let mut maximum = 0;
        for output in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::U8, hir::Ty::Unit] {
            for left_ty in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::U8, hir::Ty::Unit] {
                for right_ty in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::U8, hir::Ty::Unit] {
                    function.locals[0].ty = output;
                    function.locals[1].ty = left_ty;
                    function.locals[2].ty = right_ty;
                    for bad_destination in [false, true] {
                        for bad_left in [false, true] {
                            for bad_right in [false, true] {
                                let destination =
                                    LocalId(if bad_destination { usize::MAX } else { 0 });
                                let left = Operand {
                                    local: LocalId(if bad_left { usize::MAX } else { 1 }),
                                    span,
                                };
                                let right = Operand {
                                    local: LocalId(if bad_right { usize::MAX } else { 2 }),
                                    span,
                                };
                                let mut values = vec![
                                    Rvalue::Bool(true),
                                    Rvalue::I32(0),
                                    Rvalue::Unit,
                                    Rvalue::Copy(left),
                                    Rvalue::Load(Place {
                                        id: PlaceId(if bad_left { usize::MAX } else { 0 }),
                                        span,
                                    }),
                                    Rvalue::CheckedI32ToU8 {
                                        operand: left,
                                        name_span: span,
                                        source_expr: crate::frontend::ast::ExprId(0),
                                    },
                                    Rvalue::U8ToI32 {
                                        operand: left,
                                        name_span: span,
                                        source_expr: crate::frontend::ast::ExprId(0),
                                    },
                                    Rvalue::NotBool {
                                        operand: left,
                                        operator_span: span,
                                    },
                                    Rvalue::CheckedNegateI32 {
                                        operand: left,
                                        operator_span: span,
                                    },
                                ];
                                for op in [
                                    hir::ComparisonOp::Equal,
                                    hir::ComparisonOp::NotEqual,
                                    hir::ComparisonOp::Less,
                                    hir::ComparisonOp::LessEqual,
                                    hir::ComparisonOp::Greater,
                                    hir::ComparisonOp::GreaterEqual,
                                ] {
                                    values.push(Rvalue::CompareScalar {
                                        op,
                                        left,
                                        right,
                                        operator_span: span,
                                    });
                                }
                                for op in [
                                    hir::ArithmeticOp::Add,
                                    hir::ArithmeticOp::Subtract,
                                    hir::ArithmeticOp::Multiply,
                                    hir::ArithmeticOp::Divide,
                                    hir::ArithmeticOp::Remainder,
                                ] {
                                    values.push(Rvalue::CheckedI32 {
                                        op,
                                        left,
                                        right,
                                        operator_span: span,
                                    });
                                }
                                for value in values {
                                    reset();
                                    let result = admit_assignment(
                                        &function,
                                        &Assign {
                                            destination,
                                            value,
                                            span,
                                        },
                                    );
                                    let (assignments, lookups) = calls();
                                    assert_eq!(assignments, 1);
                                    assert!((1..=3).contains(&lookups));
                                    maximum = maximum.max(lookups);
                                    if let Err(error) = result {
                                        assert_eq!(error.primary, None);
                                        assert_eq!(error.message, INVALID_TYPES);
                                        assert_eq!(error.message.capacity(), INVALID_TYPES.len());
                                        assert!(
                                            error.secondary.is_empty() && error.notes.is_empty()
                                        );
                                    }
                                    attempts += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(maximum, 3);
        assert_eq!(attempts, 10_240);
        println!("U8_NATIVE_ADMISSION_CENSUS cases={attempts} max_local_lookups={maximum} finite_event_bound={ADMISSION_EVENTS} tariff={ADMISSION_TARIFF}");
    }
}
