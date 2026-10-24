//! A checked program selects its complete scalar storage representation once.
//! The narrow representation and its resource costs remain unchanged.
use super::*;

pub(super) trait StoredValue: Copy + std::fmt::Debug + Eq + From<Scalar> {
    const WIDE: bool;
    const QUAD: bool = false;
    fn narrow(self) -> Option<Scalar>;
    fn wide(self) -> Option<i64>;
    fn from_wide(value: i64) -> Option<Self>;
    fn integer(self) -> Option<(hir::Ty, u64)>;
    fn from_integer(ty: hir::Ty, bits: u64) -> Option<Self>;
    fn integer128(self) -> Option<(hir::Ty, u128)> {
        self.integer().map(|(ty, bits)| {
            (
                ty,
                if ty.signed_integer() {
                    bits as i64 as i128 as u128
                } else {
                    u128::from(bits)
                },
            )
        })
    }
    fn from_integer128(ty: hir::Ty, bits: u128) -> Option<Self> {
        let value = Self::from_integer(ty, bits as u64)?;
        (value.integer128() == Some((ty, bits))).then_some(value)
    }
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
    fn integer(self) -> Option<(hir::Ty, u64)> {
        match self {
            Self::I32(value) => Some((hir::Ty::I32, value as i64 as u64)),
            Self::U8(value) => Some((hir::Ty::U8, u64::from(value))),
            Self::Bool(_) | Self::Unit => None,
        }
    }
    fn from_integer(ty: hir::Ty, bits: u64) -> Option<Self> {
        match ty {
            hir::Ty::I32 => i32::try_from(bits as i64).ok().map(Self::I32),
            hir::Ty::U8 => u8::try_from(bits).ok().map(Self::U8),
            _ => None,
        }
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
    fn integer(self) -> Option<(hir::Ty, u64)> {
        RuntimeScalar::integer(self)
    }
    fn from_integer(ty: hir::Ty, bits: u64) -> Option<Self> {
        RuntimeScalar::from_integer(ty, bits)
    }
    fn ty(self) -> hir::Ty {
        match self {
            Self::Narrow(value) => value.ty(),
            Self::I64(_) => hir::Ty::I64,
            Self::I8(_) => hir::Ty::I8,
            Self::I16(_) => hir::Ty::I16,
            Self::U16(_) => hir::Ty::U16,
            Self::U32(_) => hir::Ty::U32,
            Self::U64(_) => hir::Ty::U64,
            Self::Isize(_) => hir::Ty::Isize,
            Self::Usize(_) => hir::Ty::Usize,
        }
    }
    fn decode(bytes: &[u8], offset: usize, ty: hir::Ty, span: Span) -> Result<Self> {
        if matches!(ty, hir::Ty::I128 | hir::Ty::U128) {
            return Err(bad("quad value in legacy storage", span));
        }
        if ty.extended_integer().is_some() {
            let width = scalar_size(ty);
            let selected = offset
                .checked_add(width)
                .and_then(|end| bytes.get(offset..end))
                .ok_or_else(|| bad("payload range", span))?;
            let mut encoded = [0u8; 8];
            encoded[..width].copy_from_slice(selected);
            let raw = u64::from_le_bytes(encoded);
            let bits = if ty.signed_integer() {
                let shift = 64 - width * 8;
                (((raw << shift) as i64) >> shift) as u64
            } else {
                raw
            };
            return RuntimeScalar::from_integer(ty, bits)
                .ok_or_else(|| bad("integer payload", span));
        }
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
            value @ (Self::I8(_)
            | Self::I16(_)
            | Self::U16(_)
            | Self::U32(_)
            | Self::U64(_)
            | Self::Isize(_)
            | Self::Usize(_)) => {
                let (ty, bits) =
                    RuntimeScalar::integer(value).ok_or_else(|| bad("integer payload", span))?;
                let width = scalar_size(ty);
                let selected = offset
                    .checked_add(width)
                    .and_then(|end| bytes.get_mut(offset..end))
                    .ok_or_else(|| bad("payload range", span))?;
                selected.copy_from_slice(&bits.to_le_bytes()[..width]);
                Ok(())
            }
        }
    }
}

impl StoredValue for RuntimeValue {
    const WIDE: bool = true;
    const QUAD: bool = true;
    fn narrow(self) -> Option<Scalar> {
        self.legacy()?.narrow()
    }
    fn wide(self) -> Option<i64> {
        <RuntimeScalar as StoredValue>::wide(self.legacy()?)
    }
    fn from_wide(value: i64) -> Option<Self> {
        Some(Self::Scalar(RuntimeScalar::I64(value)))
    }
    fn integer(self) -> Option<(hir::Ty, u64)> {
        self.legacy()?.integer()
    }
    fn from_integer(ty: hir::Ty, bits: u64) -> Option<Self> {
        RuntimeScalar::from_integer(ty, bits).map(Self::Scalar)
    }
    fn integer128(self) -> Option<(hir::Ty, u128)> {
        RuntimeValue::integer128(self)
    }
    fn from_integer128(ty: hir::Ty, bits: u128) -> Option<Self> {
        RuntimeValue::from_integer128(ty, bits)
    }
    fn ty(self) -> hir::Ty {
        match self {
            Self::Scalar(value) => <RuntimeScalar as StoredValue>::ty(value),
            Self::I128(_) => hir::Ty::I128,
            Self::U128(_) => hir::Ty::U128,
        }
    }
    fn decode(bytes: &[u8], offset: usize, ty: hir::Ty, span: Span) -> Result<Self> {
        if !matches!(ty, hir::Ty::I128 | hir::Ty::U128) {
            return <RuntimeScalar as StoredValue>::decode(bytes, offset, ty, span)
                .map(Self::Scalar);
        }
        let selected = offset
            .checked_add(16)
            .and_then(|end| bytes.get(offset..end))
            .ok_or_else(|| bad("payload range", span))?;
        let encoded: [u8; 16] = selected
            .try_into()
            .map_err(|_| bad("quad payload width", span))?;
        RuntimeValue::from_integer128(ty, u128::from_le_bytes(encoded))
            .ok_or_else(|| bad("quad payload", span))
    }
    fn encode(self, bytes: &mut [u8], offset: usize, span: Span) -> Result<()> {
        match self {
            Self::Scalar(value) => {
                <RuntimeScalar as StoredValue>::encode(value, bytes, offset, span)
            }
            value => {
                let (_, bits) = value
                    .integer128()
                    .ok_or_else(|| bad("quad payload type", span))?;
                let selected = offset
                    .checked_add(16)
                    .and_then(|end| bytes.get_mut(offset..end))
                    .ok_or_else(|| bad("payload range", span))?;
                selected.copy_from_slice(&bits.to_le_bytes());
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
    if left.ty().extended_integer().is_some() {
        return integers::comparison(left, right, op, span);
    }
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
struct WideControlCarriers<V: StoredValue = RuntimeScalar> {
    read_result: Result<V>,
    left_read_result: Result<V>,
    right_read_result: Result<V>,
    left_operand: V,
    right_operand: V,
    assigned_value: V,
    write_argument: V,
    write_conversion: V,
    field_return: Result<V>,
    leaf_return: Result<V>,
    element_return: Result<V>,
    selected_enum: StoredEnumValue<V>,
    enum_return: Result<StoredEnumValue<V>>,
    enum_binding: Option<V>,
    return_value: Option<V>,
    return_result: Result<Option<V>>,
    execute_return: Result<V>,
    policy_return: Result<V>,
    selected_left: Option<i64>,
    selected_right: Option<i64>,
    selected_integer: i64,
    checked_integer: Option<i64>,
    checked_result: Result<V>,
    integer_to_i32: std::result::Result<i32, std::num::TryFromIntError>,
    integer_to_u8: std::result::Result<u8, std::num::TryFromIntError>,
    codec_bytes: [u8; 8],
    decoded_integer: i64,
    copy_cell: Result<(ScalarLeaf, V)>,
    selected_copy_cell: (ScalarLeaf, V),
}
pub(super) fn control_bytes<V: StoredValue>() -> usize {
    if V::WIDE {
        size_of::<WideControlCarriers<V>>()
    } else {
        0
    }
}

const _: () = assert!(size_of::<Scalar>() == 8 && size_of::<Option<Scalar>>() == 8);
const _: () = assert!(size_of::<RuntimeScalar>() == 16 && size_of::<Option<RuntimeScalar>>() == 16);
