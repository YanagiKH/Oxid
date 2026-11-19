//! IEEE 754 operations over explicitly tagged f32/f64 bit patterns.
use super::*;
fn result<V: StoredValue>(ty: hir::Ty, bits: u64, span: Span) -> Result<V> {
    V::from_float(ty, bits).ok_or_else(|| bad("float result storage", span))
}
pub(super) fn negate<V: StoredValue>(value: V, span: Span) -> Result<V> {
    let (ty, bits) = value
        .float()
        .ok_or_else(|| bad("float negate type", span))?;
    let sign = if ty == hir::Ty::F32 {
        1u64 << 31
    } else {
        1u64 << 63
    };
    result(ty, bits ^ sign, span)
}
pub(super) fn arithmetic<V: StoredValue>(
    op: hir::ArithmeticOp,
    left: V,
    right: V,
    span: Span,
) -> Result<V> {
    let (ty, a) = left.float().ok_or_else(|| bad("float left type", span))?;
    let (other, b) = right.float().ok_or_else(|| bad("float right type", span))?;
    if ty != other {
        return Err(bad("float operand types", span));
    }
    let bits = match ty {
        hir::Ty::F32 => {
            let a = f32::from_bits(a as u32);
            let b = f32::from_bits(b as u32);
            let value = match op {
                hir::ArithmeticOp::Add => a + b,
                hir::ArithmeticOp::Subtract => a - b,
                hir::ArithmeticOp::Multiply => a * b,
                hir::ArithmeticOp::Divide => a / b,
                hir::ArithmeticOp::Remainder => a % b,
            };
            u64::from(if value.is_nan() {
                0x7fc0_0000
            } else {
                value.to_bits()
            })
        }
        hir::Ty::F64 => {
            let a = f64::from_bits(a);
            let b = f64::from_bits(b);
            let value = match op {
                hir::ArithmeticOp::Add => a + b,
                hir::ArithmeticOp::Subtract => a - b,
                hir::ArithmeticOp::Multiply => a * b,
                hir::ArithmeticOp::Divide => a / b,
                hir::ArithmeticOp::Remainder => a % b,
            };
            if value.is_nan() {
                0x7ff8_0000_0000_0000
            } else {
                value.to_bits()
            }
        }
        _ => return Err(bad("float result type", span)),
    };
    result(ty, bits, span)
}
pub(super) fn comparison<V: StoredValue>(
    left: V,
    right: V,
    op: hir::ComparisonOp,
    span: Span,
) -> Result<bool> {
    let (ty, a) = left
        .float()
        .ok_or_else(|| bad("float comparison left", span))?;
    let (other, b) = right
        .float()
        .ok_or_else(|| bad("float comparison right", span))?;
    if ty != other {
        return Err(bad("float comparison types", span));
    }
    // f32 -> f64 is exact, including infinities, zeros, and NaN classification.
    let (a, b) = if ty == hir::Ty::F32 {
        (
            f64::from(f32::from_bits(a as u32)),
            f64::from(f32::from_bits(b as u32)),
        )
    } else {
        (f64::from_bits(a), f64::from_bits(b))
    };
    Ok(match op {
        hir::ComparisonOp::Equal => a == b,
        hir::ComparisonOp::NotEqual => a != b,
        hir::ComparisonOp::Less => a < b,
        hir::ComparisonOp::LessEqual => a <= b,
        hir::ComparisonOp::Greater => a > b,
        hir::ComparisonOp::GreaterEqual => a >= b,
    })
}
#[allow(dead_code)]
struct Control<V> {
    inputs: [V; 2],
    views: [Option<(hir::Ty, u64)>; 2],
    bits: [u64; 3],
    floats: [f64; 3],
    converted: Option<V>,
    result: Result<V>,
    comparison: Result<bool>,
    conversion_integer: Option<(hir::Ty, u128)>,
    conversion_input: (hir::Ty, u128),
    conversion_output: hir::Ty,
    float32: [f32; 3],
    float64: [f64; 3],
    codec_input: [u8; 8],
    codec_range: Option<std::ops::Range<usize>>,
    codec_read: Option<&'static [u8]>,
    codec_write: Option<&'static mut [u8]>,
}
pub(super) fn control_bytes<V: StoredValue>() -> usize {
    size_of::<Control<V>>()
}

pub(super) fn convert<V: StoredValue>(
    operation: hir::ConversionOp,
    value: V,
    span: Span,
) -> Result<V> {
    let target = value
        .ty()
        .float_conversion(operation)
        .ok_or_else(|| bad("float conversion types", span))?;
    match operation {
        hir::ConversionOp::FloatSqrt => {
            let (_, bits) = value
                .float()
                .ok_or_else(|| bad("float square-root source", span))?;
            let bits = if target == hir::Ty::F32 {
                let root = f32::from_bits(bits as u32).sqrt();
                u64::from(if root.is_nan() {
                    0x7fc0_0000
                } else {
                    root.to_bits()
                })
            } else {
                let root = f64::from_bits(bits).sqrt();
                if root.is_nan() {
                    0x7ff8_0000_0000_0000
                } else {
                    root.to_bits()
                }
            };
            result(target, bits, span)
        }
        hir::ConversionOp::FloatToBits => {
            let (_, bits) = value.float().ok_or_else(|| bad("float bit source", span))?;
            V::from_integer(target, bits).ok_or_else(|| bad("float bit storage", span))
        }
        hir::ConversionOp::FloatFromBits(_) => {
            let (_, bits) = value
                .integer()
                .ok_or_else(|| bad("integer bit source", span))?;
            result(target, bits, span)
        }
        hir::ConversionOp::ToFloat(_) => {
            if target == value.ty() {
                return Ok(value);
            }
            let bits = if let Some((input, bits)) = value.float() {
                if input == hir::Ty::F32 {
                    let converted = f64::from(f32::from_bits(bits as u32));
                    if converted.is_nan() {
                        0x7ff8_0000_0000_0000
                    } else {
                        converted.to_bits()
                    }
                } else {
                    let converted = f64::from_bits(bits) as f32;
                    u64::from(if converted.is_nan() {
                        0x7fc0_0000
                    } else {
                        converted.to_bits()
                    })
                }
            } else {
                let (input, bits) = value
                    .integer128()
                    .ok_or_else(|| bad("integer float source", span))?;
                if target == hir::Ty::F32 {
                    u64::from(
                        (if input.signed_integer() {
                            (bits as i128) as f32
                        } else {
                            bits as f32
                        })
                        .to_bits(),
                    )
                } else {
                    (if input.signed_integer() {
                        (bits as i128) as f64
                    } else {
                        bits as f64
                    })
                    .to_bits()
                }
            };
            result(target, bits, span)
        }
        _ => Err(bad("float conversion opcode", span)),
    }
}
