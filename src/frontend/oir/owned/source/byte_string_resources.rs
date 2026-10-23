//! Explicit leaf-decoding and lowering storage, separate from byte backing and
//! inherited carrier growth. These schemas are never compiler frames or values;
//! their conservative sum is not a machine-stack or process-memory claim.
use super::super::*;
use super::resolver_storage::PaidStorage;
use crate::frontend::byte_strings::{Decoder, Failure, Reason};
use std::mem::size_of;

#[allow(dead_code)]
#[repr(C)]
struct DecoderPassRoles {
    input: &'static str,
    constructed: Decoder<'static>,
    returned: Decoder<'static>,
    into_iter: Decoder<'static>,
    active: Decoder<'static>,
    receiver: &'static mut Decoder<'static>,
    byte_return: Option<Result<u8, Failure>>,
    selected: Result<u8, Failure>,
    next_return: Option<Result<u8, Failure>>,
    caller_next: Option<Result<u8, Failure>>,
    starts: [usize; 4],
    optional_bytes: [Option<u8>; 3],
    selected_bytes: [u8; 3],
    hex_values: [u32; 2],
    hex_returns: [Result<u32, Failure>; 2],
    escape_return: Result<u8, Failure>,
    scalar: u32,
    digits: i32,
    scalar_candidate: Option<char>,
    scalar_value: char,
    encoded_utf8: &'static mut str,
    failure_bounds: [usize; 4],
    failure_reason: Reason,
    failure_constructed: Failure,
    failure_return: Failure,
}

#[allow(dead_code)]
#[repr(C)]
struct ByteStringResolveRoles {
    contents: &'static Span,
    expected: &'static u16,
    input: &'static str,
    raw_length: usize,
    converted_length: Result<u64, std::num::TryFromIntError>,
    converted_option: Option<u64>,
    raw_u64: u64,
    arithmetic: [Option<u64>; 3],
    helper_return: Option<u64>,
    mapped_work_return: Result<u64, Box<Diagnostic>>,
    selected_work: u64,
    work_return: Result<(), Box<Diagnostic>>,
    length_pass: DecoderPassRoles,
    fill_pass: DecoderPassRoles,
    length_counter: i32,
    length_return: Result<u16, Failure>,
    mapped_length_return: Result<u16, Box<Diagnostic>>,
    actual: u16,
    matches: bool,
    requested: usize,
    selected_paid: Option<&'static mut PaidStorage>,
    mapped_byte: Result<u8, Box<Diagnostic>>,
    decoded_byte: u8,
    room_return: Result<(), Box<Diagnostic>>,
}

#[allow(dead_code)]
#[repr(C)]
struct ByteStringLowerRoles {
    bytes: &'static Vec<u8>,
    span: Span,
    type_return: ValueTy,
    array: FixedArrayTy,
    predicates: [bool; 3],
    owner_return: Result<OwnerPlaceId, OwnedFailure>,
    owner: OwnerPlaceId,
    count_return: Result<usize, OwnedFailure>,
    values: Vec<Operand>,
    reserve_header: Vec<Operand>,
    reserve_return: Result<Vec<Operand>, OwnedFailure>,
    cursor: std::slice::Iter<'static, u8>,
    next: Option<&'static u8>,
    byte: u8,
    local_return: Result<LocalId, OwnedFailure>,
    local: LocalId,
    rvalues: [Rvalue; 2],
    operands: [Operand; 2],
    instructions: [OwnedInstruction; 2],
    unit_returns: [Result<(), OwnedFailure>; 8],
}

pub(super) const fn fixed_bytes() -> usize {
    // Both decoder passes are included even though they run sequentially.
    // Their pending arrays are inside Decoder. Byte backing, retained HIR,
    // array literal headers, generic resolver Vec returns, raw outputs and
    // existing scheduler stacks are already priced by their own ledgers.
    size_of::<ByteStringResolveRoles>() + size_of::<ByteStringLowerRoles>()
}

/// Independent inherited-role deltas, rather than a recreated predecessor plan.
#[cfg(test)]
pub(super) const fn inherited_fixed_growth() -> usize {
    // Plan returns10; resolver fixed37; denied return3; observation9;
    // association2; production entry10; pipeline3 machine words.
    (10 + 37 + 3 + 9 + 2 + 10 + 3) * size_of::<usize>()
}
#[cfg(test)]
pub(super) const fn inherited_function_growth() -> usize {
    // Three independent HirCounts roles in FunctionCarriers.
    3 * size_of::<usize>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, offset_of};

    #[test]
    fn byte_string_leaf_storage_covers_every_named_role_and_padding() {
        macro_rules! layout {
            ($schema:ty; $($field:ident : $ty:ty),+ $(,)?) => {{
                let mut offset: usize = 0;
                let mut alignment = 1;
                $(
                    alignment = alignment.max(align_of::<$ty>());
                    offset = offset.div_ceil(align_of::<$ty>()) * align_of::<$ty>();
                    assert_eq!(offset_of!($schema, $field), offset, stringify!($field));
                    offset += size_of::<$ty>();
                )+
                assert_eq!(align_of::<$schema>(), alignment);
                assert_eq!(size_of::<$schema>(), offset.div_ceil(alignment) * alignment);
            }};
        }
        layout!(DecoderPassRoles;
            input: &'static str, constructed: Decoder<'static>, returned: Decoder<'static>,
            into_iter: Decoder<'static>, active: Decoder<'static>, receiver: &'static mut Decoder<'static>,
            byte_return: Option<Result<u8, Failure>>, selected: Result<u8, Failure>,
            next_return: Option<Result<u8, Failure>>, caller_next: Option<Result<u8, Failure>>,
            starts: [usize; 4], optional_bytes: [Option<u8>; 3], selected_bytes: [u8; 3],
            hex_values: [u32; 2], hex_returns: [Result<u32, Failure>; 2], escape_return: Result<u8, Failure>,
            scalar: u32, digits: i32, scalar_candidate: Option<char>, scalar_value: char,
            encoded_utf8: &'static mut str, failure_bounds: [usize; 4], failure_reason: Reason,
            failure_constructed: Failure, failure_return: Failure);
        layout!(ByteStringResolveRoles;
            contents: &'static Span, expected: &'static u16, input: &'static str, raw_length: usize,
            converted_length: Result<u64, std::num::TryFromIntError>, converted_option: Option<u64>, raw_u64: u64,
            arithmetic: [Option<u64>; 3], helper_return: Option<u64>, mapped_work_return: Result<u64, Box<Diagnostic>>,
            selected_work: u64, work_return: Result<(), Box<Diagnostic>>,
            length_pass: DecoderPassRoles, fill_pass: DecoderPassRoles, length_counter: i32,
            length_return: Result<u16, Failure>, mapped_length_return: Result<u16, Box<Diagnostic>>,
            actual: u16, matches: bool, requested: usize, selected_paid: Option<&'static mut PaidStorage>,
            mapped_byte: Result<u8, Box<Diagnostic>>, decoded_byte: u8,
            room_return: Result<(), Box<Diagnostic>>);
        layout!(ByteStringLowerRoles;
            bytes: &'static Vec<u8>, span: Span, type_return: ValueTy, array: FixedArrayTy,
            predicates: [bool; 3], owner_return: Result<OwnerPlaceId, OwnedFailure>, owner: OwnerPlaceId,
            count_return: Result<usize, OwnedFailure>, values: Vec<Operand>, reserve_header: Vec<Operand>,
            reserve_return: Result<Vec<Operand>, OwnedFailure>, cursor: std::slice::Iter<'static, u8>,
            next: Option<&'static u8>, byte: u8, local_return: Result<LocalId, OwnedFailure>, local: LocalId,
            rvalues: [Rvalue; 2], operands: [Operand; 2], instructions: [OwnedInstruction; 2],
            unit_returns: [Result<(), OwnedFailure>; 8]);
        let sum = size_of::<ByteStringResolveRoles>() + size_of::<ByteStringLowerRoles>();
        assert_eq!(fixed_bytes(), sum);
        assert!(sum > 0);
        #[cfg(target_pointer_width = "64")]
        assert_eq!(
            (inherited_fixed_growth(), inherited_function_growth()),
            (592, 24)
        );
        println!("BYTE_STRING_LEAF_STORAGE resolver={} lower={} fixed={} inherited_fixed_growth={} inherited_function_growth={}",
            size_of::<ByteStringResolveRoles>(), size_of::<ByteStringLowerRoles>(), sum,
            inherited_fixed_growth(), inherited_function_growth());
    }
}
