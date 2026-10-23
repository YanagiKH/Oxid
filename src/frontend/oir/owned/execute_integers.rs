//! Checked arithmetic for additional integer widths, after source verification.
use super::*;

fn bounds(ty: hir::Ty, span: Span) -> Result<(i128, i128)> {
    let width = ty.integer_bits().ok_or_else(|| bad("integer type", span))?;
    if ty.signed_integer() {
        Ok((-(1i128 << (width - 1)), (1i128 << (width - 1)) - 1))
    } else {
        Ok((0, (1i128 << width) - 1))
    }
}

pub(super) fn arithmetic<V: StoredValue>(
    op: hir::ArithmeticOp,
    left: V,
    right: V,
    span: Span,
) -> Result<V> {
    let (ty, lhs) = left
        .integer()
        .ok_or_else(|| bad("integer left operand", span))?;
    let (right_ty, rhs) = right
        .integer()
        .ok_or_else(|| bad("integer right operand", span))?;
    if ty != right_ty {
        return Err(bad("integer operand types", span));
    }
    let overflow = || RunFailure::IntegerOverflow { ty, span };
    let zero = || RunFailure::IntegerDivisionByZero { ty, span };
    let bits = if ty.signed_integer() {
        let a = i128::from(lhs as i64);
        let b = i128::from(rhs as i64);
        let (low, high) = bounds(ty, span)?;
        if matches!(op, hir::ArithmeticOp::Divide | hir::ArithmeticOp::Remainder) {
            if b == 0 {
                return Err(zero().into());
            }
            if a == low && b == -1 {
                return Err(overflow().into());
            }
        }
        let value = match op {
            hir::ArithmeticOp::Add => a + b,
            hir::ArithmeticOp::Subtract => a - b,
            hir::ArithmeticOp::Multiply => a * b,
            hir::ArithmeticOp::Divide => a / b,
            hir::ArithmeticOp::Remainder => a % b,
        };
        if value < low || value > high {
            return Err(overflow().into());
        }
        value as i64 as u64
    } else {
        let a = u128::from(lhs);
        let b = u128::from(rhs);
        if b == 0 && matches!(op, hir::ArithmeticOp::Divide | hir::ArithmeticOp::Remainder) {
            return Err(zero().into());
        }
        let value = match op {
            hir::ArithmeticOp::Add => a + b,
            hir::ArithmeticOp::Subtract => a.checked_sub(b).ok_or_else(overflow)?,
            hir::ArithmeticOp::Multiply => a * b,
            hir::ArithmeticOp::Divide => a / b,
            hir::ArithmeticOp::Remainder => a % b,
        };
        let width = ty
            .integer_bits()
            .ok_or_else(|| bad("integer width", span))?;
        if value >= 1u128 << width {
            return Err(overflow().into());
        }
        value as u64
    };
    V::from_integer(ty, bits).ok_or_else(|| bad("integer result storage", span))
}

pub(super) fn negate<V: StoredValue>(operand: V, span: Span) -> Result<V> {
    let (ty, bits) = operand
        .integer()
        .ok_or_else(|| bad("integer negate operand", span))?;
    if !ty.signed_integer() {
        return Err(bad("unsigned negate", span));
    }
    let (low, high) = bounds(ty, span)?;
    let value = -i128::from(bits as i64);
    if value < low || value > high {
        return Err(RunFailure::IntegerOverflow { ty, span }.into());
    }
    V::from_integer(ty, value as i64 as u64).ok_or_else(|| bad("integer negate storage", span))
}

pub(super) fn cast<V: StoredValue>(operand: V, target: hir::Ty, span: Span) -> Result<V> {
    let (input, bits) = operand
        .integer()
        .ok_or_else(|| bad("integer conversion operand", span))?;
    let value = if input.signed_integer() {
        i128::from(bits as i64)
    } else {
        i128::from(bits)
    };
    let (low, high) = bounds(target, span)?;
    if value < low || value > high {
        return Err(RunFailure::IntegerRange {
            input,
            target,
            span,
        }
        .into());
    }
    V::from_integer(target, value as u64).ok_or_else(|| bad("integer conversion storage", span))
}

pub(super) fn comparison<V: StoredValue>(
    left: V,
    right: V,
    op: hir::ComparisonOp,
    span: Span,
) -> Result<bool> {
    let (ty, a) = left
        .integer()
        .ok_or_else(|| bad("integer comparison left", span))?;
    let (other, b) = right
        .integer()
        .ok_or_else(|| bad("integer comparison right", span))?;
    if ty != other {
        return Err(bad("integer comparison types", span));
    }
    let order = if ty.signed_integer() {
        (a as i64).cmp(&(b as i64))
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

// Existing narrow/i64 operations do not enter these helper paths. The plan
// admits these additional transports before any extended operation executes.
#[allow(dead_code)]
struct Control<V> {
    first_read: Result<V>,
    second_read: Result<V>,
    caller_left: V,
    caller_right: V,
    helper_left: V,
    helper_right: V,
    first_integer: Option<(hir::Ty, u64)>,
    second_integer: Option<(hir::Ty, u64)>,
    first_bits: u64,
    second_bits: u64,
    signed_left: i128,
    signed_right: i128,
    signed_result: i128,
    signed_bounds: Result<(i128, i128)>,
    selected_bounds: (i128, i128),
    unsigned_left: u128,
    unsigned_right: u128,
    unsigned_result: u128,
    unsigned_subtraction: Option<u128>,
    converted_result: Option<V>,
    helper_result: Result<V>,
    comparison_result: Result<bool>,
    codec_encoded: [u8; 8],
    codec_bits: u64,
    codec_integer: Option<(hir::Ty, u64)>,
    codec_result: Result<V>,
    codec_bytes: Option<std::ops::Range<usize>>,
    codec_read: Option<&'static [u8]>,
    codec_write: Option<&'static mut [u8]>,
}
pub(super) fn control_bytes<V: StoredValue>() -> usize {
    size_of::<Control<V>>()
}
