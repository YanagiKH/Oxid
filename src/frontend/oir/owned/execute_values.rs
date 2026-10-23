//! A checked program selects its complete scalar storage representation once.
//! The narrow representation and its resource costs remain unchanged.
use super::*;

pub(super) trait StoredValue: Copy + std::fmt::Debug + Eq + From<Scalar> {
    const WIDE: bool;
    fn narrow(self) -> Option<Scalar>;
    fn wide(self) -> Option<i64>;
    fn from_wide(value: i64) -> Option<Self>;
    fn ty(self) -> hir::Ty;
    fn decode(bytes: &[u8], offset: usize, ty: hir::Ty, span: Span) -> Result<Self>;
    fn encode(self, bytes: &mut [u8], offset: usize, span: Span) -> Result<()>;
}

impl StoredValue for Scalar {
    const WIDE: bool = false;
    fn narrow(self) -> Option<Scalar> {
        Some(self)
    }
    fn wide(self) -> Option<i64> {
        None
    }
    fn from_wide(_: i64) -> Option<Self> {
        None
    }
    fn ty(self) -> hir::Ty {
        Scalar::ty(self)
    }
    fn decode(bytes: &[u8], offset: usize, ty: hir::Ty, span: Span) -> Result<Self> {
        decode(bytes, offset, ty, span)
    }
    fn encode(self, bytes: &mut [u8], offset: usize, span: Span) -> Result<()> {
        encode(bytes, offset, self, span)
    }
}

impl StoredValue for RuntimeScalar {
    const WIDE: bool = true;
    fn narrow(self) -> Option<Scalar> {
        RuntimeScalar::narrow(self)
    }
    fn wide(self) -> Option<i64> {
        if let Self::I64(value) = self {
            Some(value)
        } else {
            None
        }
    }
    fn from_wide(value: i64) -> Option<Self> {
        Some(Self::I64(value))
    }
    fn ty(self) -> hir::Ty {
        match self {
            Self::Narrow(value) => value.ty(),
            Self::I64(_) => hir::Ty::I64,
        }
    }
    fn decode(bytes: &[u8], offset: usize, ty: hir::Ty, span: Span) -> Result<Self> {
        if ty != hir::Ty::I64 {
            return decode(bytes, offset, ty, span).map(Self::Narrow);
        }
        let selected = offset
            .checked_add(8)
            .and_then(|end| bytes.get(offset..end))
            .ok_or_else(|| bad("payload range", span))?;
        Ok(Self::I64(i64::from_le_bytes(
            selected
                .try_into()
                .map_err(|_| bad("payload width", span))?,
        )))
    }
    fn encode(self, bytes: &mut [u8], offset: usize, span: Span) -> Result<()> {
        match self {
            Self::Narrow(value) => encode(bytes, offset, value, span),
            Self::I64(value) => {
                let selected = offset
                    .checked_add(8)
                    .and_then(|end| bytes.get_mut(offset..end))
                    .ok_or_else(|| bad("payload range", span))?;
                selected.copy_from_slice(&value.to_le_bytes());
                Ok(())
            }
        }
    }
}

pub(super) fn checked_wide<V: StoredValue>(value: i64, span: Span) -> Result<V> {
    V::from_wide(value).ok_or_else(|| bad("wide value in narrow execution", span))
}

pub(super) fn comparison<V: StoredValue>(
    left: V,
    right: V,
    op: hir::ComparisonOp,
    span: Span,
) -> Result<bool> {
    let ordering = match (left.wide(), right.wide()) {
        (Some(a), Some(b)) => a.cmp(&b),
        (None, None) => match (left.narrow(), right.narrow()) {
            (Some(Scalar::I32(a)), Some(Scalar::I32(b))) => a.cmp(&b),
            (Some(Scalar::U8(a)), Some(Scalar::U8(b))) => a.cmp(&b),
            (Some(Scalar::Bool(a)), Some(Scalar::Bool(b))) => {
                if !matches!(op, hir::ComparisonOp::Equal | hir::ComparisonOp::NotEqual) {
                    return Err(bad("boolean ordering", span));
                }
                a.cmp(&b)
            }
            _ => return Err(bad("comparison type", span)),
        },
        _ => return Err(bad("comparison type", span)),
    };
    Ok(match op {
        hir::ComparisonOp::Equal => ordering.is_eq(),
        hir::ComparisonOp::NotEqual => !ordering.is_eq(),
        hir::ComparisonOp::Less => ordering.is_lt(),
        hir::ComparisonOp::LessEqual => !ordering.is_gt(),
        hir::ComparisonOp::Greater => ordering.is_gt(),
        hir::ComparisonOp::GreaterEqual => !ordering.is_lt(),
    })
}

// New full-width transports are paid only by the full-width interpreter. Frame
// slots/snapshots and aggregate payloads have separate exact physical pricing.
#[allow(dead_code)]
struct WideControlCarriers {
    read_result: Result<RuntimeScalar>,
    left_read_result: Result<RuntimeScalar>,
    right_read_result: Result<RuntimeScalar>,
    left_operand: RuntimeScalar,
    right_operand: RuntimeScalar,
    assigned_value: RuntimeScalar,
    write_argument: RuntimeScalar,
    write_conversion: RuntimeScalar,
    field_return: Result<RuntimeScalar>,
    leaf_return: Result<RuntimeScalar>,
    element_return: Result<RuntimeScalar>,
    selected_enum: StoredEnumValue<RuntimeScalar>,
    enum_return: Result<StoredEnumValue<RuntimeScalar>>,
    enum_binding: Option<RuntimeScalar>,
    return_value: Option<RuntimeScalar>,
    return_result: Result<Option<RuntimeScalar>>,
    execute_return: Result<RuntimeScalar>,
    policy_return: Result<RuntimeScalar>,
    selected_left: Option<i64>,
    selected_right: Option<i64>,
    selected_integer: i64,
    checked_integer: Option<i64>,
    checked_result: Result<RuntimeScalar>,
    integer_to_i32: std::result::Result<i32, std::num::TryFromIntError>,
    integer_to_u8: std::result::Result<u8, std::num::TryFromIntError>,
    codec_bytes: [u8; 8],
    decoded_integer: i64,
    copy_cell: Result<(ScalarLeaf, RuntimeScalar)>,
    selected_copy_cell: (ScalarLeaf, RuntimeScalar),
}
pub(super) fn control_bytes<V: StoredValue>() -> usize {
    if V::WIDE {
        size_of::<WideControlCarriers>()
    } else {
        0
    }
}

const _: () = assert!(size_of::<Scalar>() == 8 && size_of::<Option<Scalar>>() == 8);
const _: () = assert!(size_of::<RuntimeScalar>() == 16 && size_of::<Option<RuntimeScalar>>() == 16);
