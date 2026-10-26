//! Integer bit patterns with explicit shift bounds and original-width results.
use super::*;

fn mask(width: u8) -> u128 {
    if width == 128 {
        u128::MAX
    } else {
        (1u128 << width) - 1
    }
}
fn result<V: StoredValue>(ty: hir::Ty, bits: u128, span: Span) -> Result<V> {
    let width = ty
        .integer_bits()
        .ok_or_else(|| bad("bitwise result type", span))?;
    let selected = mask(width);
    let mut bits = bits & selected;
    if ty.signed_integer() && bits & (1u128 << (width - 1)) != 0 {
        bits |= !selected;
    }
    V::from_integer128(ty, bits).ok_or_else(|| bad("bitwise result storage", span))
}
pub(super) fn complement<V: StoredValue>(operand: V, span: Span) -> Result<V> {
    let (ty, bits) = operand
        .integer128()
        .ok_or_else(|| bad("bitwise complement type", span))?;
    result(ty, !bits, span)
}
pub(super) fn binary<V: StoredValue>(
    op: hir::BitwiseOp,
    left: V,
    right: V,
    span: Span,
) -> Result<V> {
    let (ty, a) = left
        .integer128()
        .ok_or_else(|| bad("bitwise left type", span))?;
    let (other, b) = right
        .integer128()
        .ok_or_else(|| bad("bitwise right type", span))?;
    let width = ty
        .integer_bits()
        .ok_or_else(|| bad("bitwise width", span))?;
    let bits = if op.shift() {
        if (other.signed_integer() && (b as i128) < 0) || b >= u128::from(width) {
            return Err(RunFailure::BitShift(span).into());
        }
        let count = b as u32;
        match op {
            hir::BitwiseOp::ShiftLeft => a << count,
            hir::BitwiseOp::ShiftRight if ty.signed_integer() => ((a as i128) >> count) as u128,
            hir::BitwiseOp::ShiftRight => (a & mask(width)) >> count,
            _ => unreachable!("checked shift opcode"),
        }
    } else {
        if ty != other {
            return Err(bad("bitwise operand types", span));
        }
        match op {
            hir::BitwiseOp::And => a & b,
            hir::BitwiseOp::Or => a | b,
            hir::BitwiseOp::Xor => a ^ b,
            _ => unreachable!("non-shift bitwise opcode"),
        }
    };
    result(ty, bits, span)
}

#[allow(dead_code)]
struct Control<V> {
    read_results: [Result<V>; 2],
    caller_values: [V; 2],
    helper_values: [V; 2],
    integer_views: [Option<(hir::Ty, u128)>; 2],
    input_bits: [u128; 2],
    selected_bits: u128,
    selected_mask: u128,
    result_argument: u128,
    normalized_result: u128,
    shift_count: u32,
    converted: Option<V>,
    returned: Result<V>,
}
pub(super) fn control_bytes<V: StoredValue>() -> usize {
    size_of::<Control<V>>()
}
