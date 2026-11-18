//! Direct finite-record vector calls selected by the exact sealed suffix key.
use super::*;
use crate::frontend::builtin_catalog::owned_vector::{Function, Operation};

const NAME_BYTES: usize = 8 * usize::BITS as usize + 128;
#[allow(dead_code)]
struct EmitterCarriers {
    keys: [Function; 3],
    key_results: [Option<Function>; 2],
    names: [String; 12],
    views: [owned_record_codec::bounds::CheckedElementView<'static>; 3],
    elements: [owned_record_codec::Element<'static>; 3],
    element_results:
        [Result<owned_record_codec::Element<'static>, owned_record_codec::bounds::ShapeFailure>; 2],
    indices: [usize; 8],
    destinations: [CallResult; 2],
    formatting: [FormatCallsiteBacking<8>; 4],
    writes: [std::fmt::Result; 4],
    // Shared resource/composition calls select only these authenticated roots.
    roots: [OwnedVectorRoot; 2],
    selected_roots: [Option<OwnedVectorRoot>; 2],
    slots: [OwnedVectorRootSlot; 2],
    selected_slots: [Option<OwnedVectorRootSlot>; 2],
    storage_keys: [u32; 2],
}
#[allow(dead_code)]
struct ElementBorrowEmitterCarriers {
    plan: *const (),
    function: &'static RawOwnedFunction,
    loan: LoanId,
    index: Operand,
    record: RecordId,
    nominal: Option<RecordId>,
    element: owned_record_codec::Element<'static>,
    checked: Result<owned_record_codec::Element<'static>, owned_record_codec::bounds::ShapeFailure>,
    view: Option<owned_record_codec::bounds::CheckedElementView<'static>>,
    key: u32,
    names: [String; 5],
    formatting: [FormatCallsiteBacking<8>; 5],
    writes: [std::fmt::Result; 5],
    selected_slot: usize,
}
pub(super) const fn element_borrow_emitter_bytes() -> usize {
    size_of::<ElementBorrowEmitterCarriers>() + 5 * NAME_BYTES
}
#[allow(dead_code)]
#[repr(C)]
struct ElementBorrowRuntimeControls {
    heap: *mut (),
    descriptor: *const (),
    fuel: *mut u64,
    output: *mut *mut u8,
    schema: *const (),
    node: *mut (),
    record: u32,
    index: i32,
    controls: [(*const (), usize); 3],
    work: u64,
    remaining: u64,
    status: i32,
    offset: usize,
}
pub(super) const ELEMENT_BORROW_RUNTIME_BYTES: usize = size_of::<ElementBorrowRuntimeControls>();

pub(super) const fn control_bytes() -> usize {
    size_of::<EmitterCarriers>()
        + 12 * NAME_BYTES
        + owned_vector_schemas::control_bytes()
        + owned_record_codec::bounds::element_control_bytes()
}

// Logical payload of the synchronous C control bank. These named roles cover
// the caller, range/registry admission, staged growth and codec walks; they do
// not describe compiler-generated machine stack frames or allocator metadata.
#[allow(dead_code)]
#[repr(C)]
struct CodecFrame {
    ty: u32,
    next: u32,
    offset: usize,
}
#[allow(dead_code)]
#[repr(C)]
struct CodecCells {
    bytes: *const u8,
    length: usize,
    depth: usize,
    frames: [CodecFrame; 65],
}
#[allow(dead_code)]
#[repr(C)]
struct RuntimeControls {
    // Caller plus validation, canonical copy and release helper transports.
    cells: [CodecCells; 4],
    ranges: [(*const (), usize); 3],
    heap_and_nodes: [*mut (); 8],
    source_and_output: [*mut (); 4],
    schema_and_type: [*const (); 4],
    record_and_status: [u32; 8],
    selected_enum_ordinal_and_kind: [u32; 2],
    work_and_sizes: [u64; 24],
    identity_heap_sort: [usize; 12],
    descriptor_copies: [[u64; 2]; 3],
}
pub(super) const RUNTIME_BYTES: usize = size_of::<RuntimeControls>();

pub(super) fn fuel_scope(out: &mut Emission) {
    if out.policy.hosted() {
        // Hosted execution has no cumulative language-fuel counter. Reuse its
        // already admitted eight-byte transport as a fresh operation budget.
        out.write_str("  store i64 -1, ptr %fuel, align 8\n")
            .unwrap();
    }
}

pub(super) fn delivery<W: PlanWitness>(plan: &ExecutionPlan<'_, W>, id: hir::DefId) -> bool {
    plan.witness().owned_vector_operation(id).is_some()
}

pub(super) fn declarations(out: &mut Emission) {
    out.write_str(
        "declare ptr @__oxid_owned_vector_heap()\n\
declare i32 @oxid_owned_vector_new(i32, ptr)\n\
declare i32 @oxid_owned_vector_validate(ptr, ptr, i32, ptr)\n\
declare i32 @oxid_owned_vector_validate_maintenance(ptr, ptr, i32)\n\
declare i32 @oxid_owned_vector_len(ptr, ptr, i32, ptr, ptr)\n\
declare i32 @oxid_owned_vector_capacity(ptr, ptr, i32, ptr, ptr)\n\
declare i32 @oxid_owned_vector_reserve(ptr, ptr, i32, i32, ptr)\n\
declare i32 @oxid_owned_vector_push_reserved(ptr, ptr, i32, ptr, i64, ptr)\n\
declare i32 @oxid_owned_vector_pop_last(ptr, ptr, i32, ptr, i64, ptr)\n\
declare i32 @oxid_owned_vector_truncate(ptr, ptr, i32, i32, ptr)\n\
declare i32 @oxid_owned_vector_take(ptr, ptr, i32, ptr, ptr)\n\
declare i32 @oxid_owned_vector_release(ptr, ptr, i32)\n",
    )
    .unwrap();
}

pub(super) fn element_borrow_declaration(out: &mut Emission) {
    out.write_str("declare i32 @oxid_owned_vector_borrow_record(ptr, ptr, i32, i32, ptr, ptr)\n")
        .unwrap();
}

pub(super) fn borrow_record<W: PlanWitness>(
    storage: &NativeFunctionStorage<'_, '_, W>,
    name: &str,
    loan: LoanId,
    index: Operand,
    diagnostics: &Diagnostics,
    span: Span,
    out: &mut Emission,
) {
    let plan = storage.execution();
    let f = storage.raw();
    let record = shape::local_record_element(f, loan).expect("verified local owned record element");
    let element = plan
        .witness()
        .elements()
        .expect("sealed owning bounds")
        .element(record)
        .expect("checked record element");
    let key = element.storage_key();
    let pointer = base_pointer(out, f, &format!("{name}_vector"), f.loans[loan.0].authority);
    let ordinal = load_operand(out, f, &format!("{name}_index"), index);
    fuel_scope(out);
    writeln!(out, "  %{name}_heap = call ptr @__oxid_owned_vector_heap()").unwrap();
    writeln!(out, "  %{name}_status = call i32 @oxid_owned_vector_borrow_record(ptr %{name}_heap, ptr {pointer}, i32 {key}, i32 {ordinal}, ptr %fuel, ptr %vector_index_pointer)").unwrap();
    experimental::status(
        out,
        diagnostics,
        name,
        &format!("%{name}_status"),
        "bounds_ok",
        span,
        true,
    );
    writeln!(out, "  %{name}_element = load ptr, ptr %vector_index_pointer, align 8\n  store ptr %{name}_element, ptr %r{}, align 8", storage.loan_slot(loan)).unwrap();
}

#[allow(clippy::too_many_arguments)]
pub(super) fn operation<W: PlanWitness>(
    storage: &NativeFunctionStorage<'_, '_, W>,
    name: &str,
    key: Function,
    destination: CallResult,
    diagnostics: &Diagnostics,
    span: Span,
    out: &mut Emission,
) {
    let plan = storage.execution();
    let f = storage.raw();
    assert_eq!(plan.witness().owned_vector_operation(f.id), Some(key));
    let view = plan
        .witness()
        .elements()
        .expect("sealed owned vector bounds");
    let element = view
        .element_root(key.element())
        .expect("sealed finite record element");
    let cost = plan
        .owned_vector_delivery_cost(f.id, key)
        .expect("canonical owned vector delivery");
    // This is compiler-owned disjoint frame/fuel storage. All fallible fuel
    // guards precede a consuming operation and its immediate return publication.
    emit_guard(out, diagnostics, &format!("{name}_delivery"), cost, span);
    fuel_scope(out);
    writeln!(out, "  %{name}_heap = call ptr @__oxid_owned_vector_heap()").unwrap();
    let pointer = if key.operation() == Operation::New {
        "null".to_owned()
    } else {
        base_pointer(
            out,
            f,
            &format!("{name}_vector"),
            AccessBase::Parameter(ReferenceParamId(0)),
        )
    };
    let result = match destination {
        CallResult::Owned(_) => "%result".to_owned(),
        CallResult::Scalar(local) => format!("%s{}", local.0),
    };
    let count = if matches!(key.operation(), Operation::TryReserve | Operation::Truncate) {
        load_operand(
            out,
            f,
            &format!("{name}_count"),
            Operand {
                local: LocalId(0),
                span,
            },
        )
    } else {
        "0".to_owned()
    };
    let record = element.storage_key();
    let stride = element.size();
    match key.operation() {
        Operation::New => writeln!(out, "  %{name}_status = call i32 @oxid_owned_vector_new(i32 {record}, ptr {result})").unwrap(),
        Operation::Len | Operation::Capacity => {
            let function = if key.operation() == Operation::Len { "len" } else { "capacity" };
            writeln!(out, "  %{name}_status = call i32 @oxid_owned_vector_{function}(ptr %{name}_heap, ptr {pointer}, i32 {record}, ptr %fuel, ptr {result})").unwrap();
        }
        Operation::TryReserve | Operation::Truncate => {
            let function = if key.operation() == Operation::TryReserve { "reserve" } else { "truncate" };
            writeln!(out, "  %{name}_status = call i32 @oxid_owned_vector_{function}(ptr %{name}_heap, ptr {pointer}, i32 {record}, i32 {count}, ptr %fuel)").unwrap();
        }
        Operation::PushReserved => writeln!(out, "  %{name}_status = call i32 @oxid_owned_vector_push_reserved(ptr %{name}_heap, ptr {pointer}, i32 {record}, ptr %o0, i64 {stride}, ptr %fuel)").unwrap(),
        Operation::PopLast => writeln!(out, "  %{name}_status = call i32 @oxid_owned_vector_pop_last(ptr %{name}_heap, ptr {pointer}, i32 {record}, ptr {result}, i64 {stride}, ptr %fuel)").unwrap(),
        Operation::Take => writeln!(out, "  %{name}_status = call i32 @oxid_owned_vector_take(ptr %{name}_heap, ptr {pointer}, i32 {record}, ptr %fuel, ptr {result})").unwrap(),
    }
    let checked_status = if key.operation() == Operation::TryReserve {
        writeln!(out, "  %{name}_reserve_result = icmp sge i32 %{name}_status, 0\n  %{name}_checked_status = select i1 %{name}_reserve_result, i32 0, i32 %{name}_status").unwrap();
        format!("%{name}_checked_status")
    } else {
        format!("%{name}_status")
    };
    experimental::status(
        out,
        diagnostics,
        name,
        &checked_status,
        "operation_ok",
        span,
        true,
    );
    match key.operation() {
        Operation::TryReserve => {
            writeln!(out, "  store i32 %{name}_status, ptr {result}, align 1").unwrap()
        }
        Operation::PushReserved | Operation::Truncate => {
            writeln!(out, "  store i64 0, ptr {result}, align 8").unwrap()
        }
        _ => (),
    }
}
