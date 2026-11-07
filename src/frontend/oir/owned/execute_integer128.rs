//! Checked full-width arithmetic. No host integer is wider than the operands.
use super::*;

pub(super) fn arithmetic<V: StoredValue>(
    op: hir::ArithmeticOp,
    left: V,
    right: V,
    span: Span,
) -> Result<V> {
    let (ty, a) = left
        .integer128()
        .ok_or_else(|| bad("128-bit left operand", span))?;
    let (other, b) = right
        .integer128()
        .ok_or_else(|| bad("128-bit right operand", span))?;
    if ty != other || !ty.quad_integer() {
        return Err(bad("128-bit operand types", span));
    }
    if b == 0 && matches!(op, hir::ArithmeticOp::Divide | hir::ArithmeticOp::Remainder) {
        return Err(RunFailure::IntegerDivisionByZero { ty, span }.into());
    }
    let bits = if ty.signed_integer() {
        let a = a as i128;
        let b = b as i128;
        match op {
            hir::ArithmeticOp::Add => a.checked_add(b),
            hir::ArithmeticOp::Subtract => a.checked_sub(b),
            hir::ArithmeticOp::Multiply => a.checked_mul(b),
            hir::ArithmeticOp::Divide => a.checked_div(b),
            hir::ArithmeticOp::Remainder => a.checked_rem(b),
        }
        .map(|value| value as u128)
    } else {
        match op {
            hir::ArithmeticOp::Add => a.checked_add(b),
            hir::ArithmeticOp::Subtract => a.checked_sub(b),
            hir::ArithmeticOp::Multiply => a.checked_mul(b),
            hir::ArithmeticOp::Divide => a.checked_div(b),
            hir::ArithmeticOp::Remainder => a.checked_rem(b),
        }
    }
    .ok_or(RunFailure::IntegerOverflow { ty, span })?;
    V::from_integer128(ty, bits).ok_or_else(|| bad("128-bit result storage", span))
}

pub(super) fn negate<V: StoredValue>(operand: V, span: Span) -> Result<V> {
    let (ty, bits) = operand
        .integer128()
        .ok_or_else(|| bad("128-bit negate operand", span))?;
    if ty != hir::Ty::I128 {
        return Err(bad("128-bit negate type", span));
    }
    let result = (bits as i128)
        .checked_neg()
        .ok_or(RunFailure::IntegerOverflow { ty, span })?;
    V::from_integer128(ty, result as u128).ok_or_else(|| bad("128-bit negate storage", span))
}

pub(super) fn cast<V: StoredValue>(operand: V, target: hir::Ty, span: Span) -> Result<V> {
    let (input, bits) = operand
        .integer128()
        .ok_or_else(|| bad("128-bit conversion operand", span))?;
    let width = target
        .integer_bits()
        .ok_or_else(|| bad("128-bit conversion target", span))?;
    let negative = input.signed_integer() && (bits as i128) < 0;
    let magnitude = if negative {
        (bits as i128).unsigned_abs()
    } else {
        bits
    };
    let maximum = if target.signed_integer() {
        (1u128 << (width - 1)) - u128::from(!negative)
    } else if width == 128 {
        u128::MAX
    } else {
        (1u128 << width) - 1
    };
    if (negative && !target.signed_integer()) || magnitude > maximum {
        return Err(RunFailure::IntegerRange {
            input,
            target,
            span,
        }
        .into());
    }
    V::from_integer128(target, bits).ok_or_else(|| bad("128-bit conversion storage", span))
}

pub(super) fn comparison<V: StoredValue>(
    left: V,
    right: V,
    op: hir::ComparisonOp,
    span: Span,
) -> Result<bool> {
    let (ty, a) = left
        .integer128()
        .ok_or_else(|| bad("128-bit comparison left", span))?;
    let (other, b) = right
        .integer128()
        .ok_or_else(|| bad("128-bit comparison right", span))?;
    if ty != other || !ty.quad_integer() {
        return Err(bad("128-bit comparison types", span));
    }
    let order = if ty.signed_integer() {
        (a as i128).cmp(&(b as i128))
    } else {
        a.cmp(&b)
    };
    Ok(match op {
        hir::ComparisonOp::Equal => order.is_eq(),
        hir::ComparisonOp::NotEqual => !order.is_eq(),
        hir::ComparisonOp::Less => order.is_lt(),
        hir::ComparisonOp::LessEqual => !order.is_gt(),
        hir::ComparisonOp::Greater => order.is_gt(),
        hir::ComparisonOp::GreaterEqual => !order.is_lt(),
    })
}

/// Include the actual 128-bit temporaries, codec transports and generic value
/// carriers in admission before any full-width operation can run.
#[allow(dead_code)]
struct Control<V> {
    read_results: [Result<V>; 2],
    caller_values: [V; 2],
    helper_values: [V; 2],
    extracted: [Option<(hir::Ty, u128)>; 2],
    unsigned_operands: [u128; 2],
    signed_operands: [i128; 2],
    checked_signed: Option<i128>,
    checked_unsigned: Option<u128>,
    returned_bits: Option<u128>,
    conversion_magnitude: u128,
    conversion_bound: u128,
    converted: Option<V>,
    helper_result: Result<V>,
    comparison_result: Result<bool>,
    codec_buffer: [u8; 16],
    codec_bits: u128,
    codec_result: Option<V>,
    codec_range: Option<std::ops::Range<usize>>,
    codec_read: Option<&'static [u8]>,
    codec_write: Option<&'static mut [u8]>,
}
pub(super) fn control_bytes<V: StoredValue>() -> usize {
    size_of::<Control<V>>()
}
