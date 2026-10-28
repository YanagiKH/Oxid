//! Native vector dispatch from the authenticated canonical suffix. No source
//! integer, descriptor shape or coincident function signature supplies a tag.
use super::super::vector_type::{self, VectorTy};
use super::*;
use crate::frontend::builtin_catalog::vector::{Function, Operation};

const NAME_BYTES: usize = 8 * usize::BITS as usize + 128;
#[allow(dead_code)]
struct EmitterCarriers {
    key: Function,
    key_return: Option<Function>,
    types: [hir::Ty; 3],
    tags: [u32; 3],
    operands: [Operand; 2],
    strings: [String; 10],
    captures: [FormatCallsiteBacking<10>; 2],
    range_capture: FormatCallsiteBacking<8>,
    writes: [std::fmt::Result; 4],
    indexed_return: plan::native_vector_layout::IndexedAccessResult,
}
pub(super) const fn control_bytes() -> usize {
    size_of::<EmitterCarriers>() + 10 * NAME_BYTES + range_control_bytes()
}
#[allow(dead_code)]
struct RangeCarriers {
    element: hir::Ty,
    stride: usize,
    backing: FormatCallsiteBacking<9>,
    write: std::fmt::Result,
}
pub(super) const fn range_control_bytes() -> usize {
    size_of::<RangeCarriers>()
}

// Actual C adapter delivery carrier plus the explicit synchronous argument,
// lookup and helper-return roles. These are normalized controls, not an RSS or
// native-machine-stack claim. Caller scalar/result/loan storage is separate.
#[allow(dead_code)]
#[repr(C)]
struct Delivery {
    remaining: u64,
    transferred: [u8; 16],
    scalar: u64,
    status: i32,
    result_bytes: usize,
}
#[allow(dead_code)]
#[repr(C)]
struct OperationFrame {
    operation: i32,
    element: u32,
    value: *mut (),
    count: i32,
    scalar: *const u8,
    source: *const u8,
    source_count: i32,
    fuel: *mut u64,
    result: *mut (),
    return_cost: u64,
    delivery: Delivery,
    width: usize,
    scalar_bytes: usize,
    source_bytes: usize,
    hosted_budget: u64,
    returned_status: i32,
}
#[allow(dead_code)]
#[repr(C)]
struct ViewFrame {
    value: *const (),
    element: u32,
    start: i32,
    end: i32,
    subrange: i32,
    pointer: *mut (),
    length: *mut i32,
    bytes: *mut u8,
    count: i32,
    status: i32,
}
#[allow(dead_code)]
#[repr(C)]
struct IndexFrame {
    value: *const (),
    element: u32,
    ordinal: i32,
    pointer: *mut (),
    bytes: *mut u8,
    status: i32,
}
#[allow(dead_code)]
#[repr(C)]
struct LengthFrame {
    value: *const (),
    element: u32,
    result: *mut (),
    status: i32,
    length: u64,
}
#[allow(dead_code)]
#[repr(C)]
struct HeapOverlapFrame {
    pointer: *const (),
    length: usize,
    cursor: *const (),
    charge: u64,
}
#[allow(dead_code)]
#[repr(C)]
struct OverlapFrame {
    a: *const (),
    a_length: usize,
    b: *const (),
    b_length: usize,
    left: usize,
    right: usize,
}
#[allow(dead_code)]
#[repr(C)]
struct HostedFrame {
    operation: i32,
    element: u32,
    value: *mut (),
    count: i32,
    scalar: *const u8,
    source: *const u8,
    source_count: i32,
    fuel: *mut u64,
    result: *mut (),
    return_cost: u64,
    output_bytes: usize,
    width: usize,
    scalar_bytes: usize,
    source_bytes: usize,
    budget: u64,
}
#[allow(dead_code)]
#[repr(C)]
struct ResultSizeFrame {
    operation: i32,
    returned: usize,
}
#[allow(dead_code)]
#[repr(C, align(16))]
struct CoreFrame {
    heap: *mut (),
    value: *mut (),
    element: u32,
    required: i32,
    mutation: i32,
    scratch: *const u8,
    scratch_bytes: usize,
    fuel: *mut u64,
    old: *mut (),
    status: i32,
    capacity: i32,
    grows: i32,
    writes: usize,
    copy_bytes: usize,
    validation: u64,
    work: u64,
    allocation_bytes: usize,
    pending: *mut (),
    header_value: [u8; 48],
    cursor: *mut (),
    found: *mut (),
    character: u32,
    width: usize,
}
#[allow(dead_code)]
#[repr(C)]
struct RuntimeCarriers {
    operation: OperationFrame,
    view: ViewFrame,
    index: IndexFrame,
    length: LengthFrame,
    heap_overlap: HeapOverlapFrame,
    overlaps: [OverlapFrame; 2],
    hosted: HostedFrame,
    result_size: ResultSizeFrame,
    core: CoreFrame,
}
pub(super) const RUNTIME_BYTES: usize = size_of::<RuntimeCarriers>();

pub(super) fn schemas(plan: &ExecutionPlan<'_>, out: &mut Emission) {
    let declarations = plan.witness().declarations();
    if !plan.uses_record_vectors() {
        return;
    }
    let count = declarations.records().len();
    writeln!(
        out,
        "@__oxid_record_vector_schema_count = constant i32 {count}, align 4"
    )
    .unwrap();
    for record in declarations.records() {
        if out.exceeded {
            return;
        }
        out.ordinary_visits += 1;
        if !record.is_copy() {
            continue;
        }
        let id = record.id();
        let leaves = declarations
            .aggregate_width(AggregateTy::Record(id))
            .expect("checked Copy record leaves");
        writeln!(
            out,
            "@__oxid_record_vector_leaves_{} = private constant [{leaves} x {{ i32, i32, i32 }}] [",
            id.0
        )
        .unwrap();
        let mut dense = 0;
        let mut separator = "";
        for leaf in declarations
            .leaves(AggregateTy::Record(id))
            .expect("checked Copy record leaves")
        {
            if out.exceeded {
                return;
            }
            out.ordinary_visits += 1;
            let element = vector_type::element(leaf.ty);
            writeln!(
                out,
                "{separator}{{ i32, i32, i32 }} {{ i32 {dense}, i32 {}, i32 {} }}",
                leaf.offset,
                element.tag()
            )
            .unwrap();
            separator = ", ";
            dense += element.size();
        }
        writeln!(out, "], align 4").unwrap();
        assert_eq!(
            dense,
            declarations
                .record_packed_bytes(id)
                .expect("checked record stride")
        );
    }
    writeln!(
        out,
        "@__oxid_record_vector_schemas = constant [{count} x {{ i32, i32, i32, i32, ptr }}] ["
    )
    .unwrap();
    let mut separator = "";
    for record in declarations.records() {
        if out.exceeded {
            return;
        }
        out.ordinary_visits += 1;
        if record.is_copy() {
            let id = record.id();
            let tag = 0x8000_0000u32 | id.0 as u32;
            let stride = declarations
                .record_packed_bytes(id)
                .expect("checked Copy record stride");
            let leaves = declarations
                .aggregate_width(AggregateTy::Record(id))
                .expect("checked Copy record width");
            writeln!(out, "{separator}{{ i32, i32, i32, i32, ptr }} {{ i32 {tag}, i32 {stride}, i32 {}, i32 {leaves}, ptr @__oxid_record_vector_leaves_{} }}", record.layout().size(), id.0).unwrap();
        } else {
            writeln!(
                out,
                "{separator}{{ i32, i32, i32, i32, ptr }} zeroinitializer"
            )
            .unwrap();
        }
        separator = ", ";
    }
    writeln!(out, "], align 8").unwrap();
}

pub(super) fn borrow_splits(plan: &ExecutionPlan<'_>, f: &RawOwnedFunction, loan: LoanId) -> bool {
    let declaration = &f.loans[loan.0];
    if declaration.projection.is_empty() {
        matches!(
            indexed_base(f, declaration.authority),
            BorrowedTy::Exact(AggregateTy::Vector(_) | AggregateTy::RecordVector(_))
        )
    } else {
        matches!(
            projection(plan, f, declaration.authority, &declaration.projection).0,
            ValueTy::Owned(AggregateTy::Vector(_) | AggregateTy::RecordVector(_))
        )
    }
}

pub(super) fn delivery(plan: &ExecutionPlan<'_>, id: hir::DefId) -> bool {
    plan.witness().vector_operation(id).is_some()
}
fn return_cost(plan: &ExecutionPlan<'_>, id: hir::DefId) -> usize {
    assert!(delivery(plan, id), "authenticated vector primitive");
    let f = &plan.witness().functions()[id.0];
    let end = f.blocks[0]
        .terminator
        .as_ref()
        .expect("canonical vector return");
    plan.terminator_cost(id, &end.kind)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn operation(
    storage: &NativeFunctionStorage<'_, '_>,
    name: &str,
    key: Function,
    destination: CallResult,
    diagnostics: &Diagnostics,
    span: Span,
    out: &mut Emission,
) {
    let plan = storage.execution();
    let f = &plan.witness().functions()[storage.id().0];
    assert_eq!(
        plan.witness().vector_operation(f.id),
        Some(key),
        "canonical vector function/key"
    );
    let tag = vector_type::checked(VectorTy::function(key), plan.witness().declarations())
        .expect("checked vector element")
        .tag();
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
    let count = if matches!(
        key.operation(),
        Operation::TryReserve | Operation::TryResize | Operation::Truncate
    ) {
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
    // Native scalar slots already contain each initialized typed byte prefix:
    // signed/narrow integers, bool/unit, chars, floats and 128-bit integers.
    // Passing that prefix avoids an unpriced alloca and a by-value C scalar.
    let scalar = if key.record_id().is_some()
        && matches!(key.operation(), Operation::TryPush | Operation::TryResize)
    {
        let owner = "%o0";
        writeln!(out, "  %{name}_pack_status = call i32 @__oxid_record_vector_pack(i32 {tag}, ptr {owner}, ptr %record_vector_scratch)").unwrap();
        experimental::status(
            out,
            diagnostics,
            &format!("{name}_pack"),
            &format!("%{name}_pack_status"),
            "packed_ok",
            span,
            false,
        );
        "%record_vector_scratch"
    } else {
        match key.operation() {
            Operation::TryPush => "%s0",
            Operation::TryResize => "%s1",
            _ => "null",
        }
    };
    let (source, length) = if key.operation() == Operation::TryExtend {
        (
            base_pointer(
                out,
                f,
                &format!("{name}_source"),
                AccessBase::Parameter(ReferenceParamId(1)),
            ),
            index_length(
                out,
                f,
                &format!("{name}_source_extent"),
                AccessBase::Parameter(ReferenceParamId(1)),
            ),
        )
    } else {
        ("null".to_owned(), "0".to_owned())
    };
    let result = match destination {
        CallResult::Owned(_) => "%result".to_owned(),
        CallResult::Scalar(local) => format!("%s{}", local.0),
    };
    let code = match key.operation() {
        Operation::New => 0,
        Operation::Len => 1,
        Operation::Capacity => 2,
        Operation::TryReserve => 3,
        Operation::TryPush => 4,
        Operation::TryExtend => 5,
        Operation::TryResize => 6,
        Operation::Truncate => 7,
        Operation::Take => 8,
    };
    let cost = return_cost(plan, f.id);
    let helper = if out.policy.hosted() {
        "__oxid_vector_operation_hosted"
    } else {
        "__oxid_vector_operation"
    };
    writeln!(out, "  %{name}_status = call i32 @{helper}(i32 {code}, i32 {tag}, ptr {pointer}, i32 {count}, ptr {scalar}, ptr {source}, i32 {length}, ptr %fuel, ptr {result}, i64 {cost})").unwrap();
    experimental::status(
        out,
        diagnostics,
        name,
        &format!("%{name}_status"),
        "operation_ok",
        span,
        true,
    );
}

pub(super) fn index_pointer(
    plan: &ExecutionPlan<'_>,
    f: &RawOwnedFunction,
    name: &str,
    statement: &OwnedStatement,
    diagnostics: &Diagnostics,
    out: &mut Emission,
) -> Option<(hir::Ty, String)> {
    let indexed =
        plan::native_vector_layout::indexed(plan.witness().declarations(), f, &statement.kind)
            .expect("admitted vector indexing")?;
    let (base, operand, element, offset) = indexed;
    let pointer = base_pointer(out, f, &format!("{name}_vector"), base);
    let pointer = match offset {
        Some(offset) => field_pointer(out, &format!("{name}_projection"), &pointer, offset),
        None => pointer,
    };
    let ordinal = load_operand(out, f, &format!("{name}_index"), operand);
    let tag = super::super::vector_type::element(element).tag();
    writeln!(out, "  %{name}_status = call i32 @__oxid_vector_index(ptr {pointer}, i32 {tag}, i32 {ordinal}, ptr %vector_index_pointer)").unwrap();
    experimental::status(
        out,
        diagnostics,
        name,
        &format!("%{name}_status"),
        "bounds_ok",
        plan::instruction_span(statement),
        false,
    );
    writeln!(
        out,
        "  %{name}_ptr = load ptr, ptr %vector_index_pointer, align 8"
    )
    .unwrap();
    Some((element, format!("%{name}_ptr")))
}
#[allow(clippy::too_many_arguments)] // Existing destination, checked type, and failure context.
pub(super) fn length(
    out: &mut Emission,
    f: &RawOwnedFunction,
    name: &str,
    base: AccessBase,
    offset: Option<usize>,
    element: VectorTy,
    destination: LocalId,
    diagnostics: &Diagnostics,
    span: Span,
) {
    let pointer = base_pointer(out, f, &format!("{name}_vector"), base);
    let pointer = match offset {
        Some(offset) => field_pointer(out, &format!("{name}_projection"), &pointer, offset),
        None => pointer,
    };
    let tag = match element {
        VectorTy::Scalar(ty) => vector_type::element(ty).tag(),
        VectorTy::Record(id) => 0x8000_0000u32 | id.0 as u32,
    };
    writeln!(
        out,
        "  %{name}_status = call i32 @__oxid_vector_length(ptr {pointer}, i32 {tag}, ptr %s{})",
        destination.0
    )
    .unwrap();
    experimental::status(
        out,
        diagnostics,
        name,
        &format!("%{name}_status"),
        "vector_ok",
        span,
        false,
    );
}

#[allow(clippy::too_many_arguments)] // Existing checked loan and emission context.
pub(super) fn borrow(
    storage: &NativeFunctionStorage<'_, '_>,
    name: &str,
    loan: LoanId,
    pointer: &str,
    element: VectorTy,
    range: Option<(Operand, Operand)>,
    diagnostics: &Diagnostics,
    span: Span,
    out: &mut Emission,
) {
    let f = storage.raw();
    let declaration = &f.loans[loan.0];
    let tag = match element {
        VectorTy::Scalar(ty) => vector_type::element(ty).tag(),
        VectorTy::Record(id) => 0x8000_0000u32 | id.0 as u32,
    };
    if declaration.referent() == BorrowedTy::Exact(element.aggregate()) {
        assert!(range.is_none(), "exact vector loan has no range");
        writeln!(
            out,
            "  %{name}_status = call i32 @__oxid_vector_valid(ptr {pointer}, i32 {tag})"
        )
        .unwrap();
        experimental::status(
            out,
            diagnostics,
            name,
            &format!("%{name}_status"),
            "bounds_ok",
            span,
            false,
        );
        writeln!(
            out,
            "  store ptr {pointer}, ptr %r{}, align 8",
            storage.loan_slot(loan)
        )
        .unwrap();
        return;
    }
    assert_eq!(
        declaration.referent(),
        element.slice(),
        "typed vector slice"
    );
    let (start, end) = range.map_or_else(
        || ("0".to_owned(), "0".to_owned()),
        |(start, end)| {
            (
                load_operand(out, f, &format!("{name}_start"), start),
                load_operand(out, f, &format!("{name}_end"), end),
            )
        },
    );
    writeln!(out, "  %{name}_status = call i32 @__oxid_vector_view(ptr {pointer}, i32 {tag}, i32 {start}, i32 {end}, i32 {}, ptr %r{}, ptr %ll{})", i32::from(range.is_some()), storage.loan_slot(loan), loan.0).unwrap();
    experimental::status(
        out,
        diagnostics,
        name,
        &format!("%{name}_status"),
        "bounds_ok",
        span,
        false,
    );
}

/// The checked record tag selects both dense stride and padded owner map.
/// Whole-vector descriptors and already-proven slice ranges remain distinct.
pub(super) fn record_index(
    storage: &NativeFunctionStorage<'_, '_>,
    name: &str,
    statement: &OwnedStatement,
    diagnostics: &Diagnostics,
    out: &mut Emission,
) {
    let plan = storage.execution();
    let f = storage.raw();
    let (base, path, operand, owner, reading) = match &statement.kind {
        OwnedInstruction::CopyIndexInitialize {
            destination,
            base,
            path,
            index,
        } => (*base, path, *index, *destination, true),
        OwnedInstruction::CopyIndexWrite {
            base,
            path,
            index,
            source,
        } => (*base, path, *index, *source, false),
        _ => unreachable!("checked record vector index"),
    };
    let AggregateTy::Record(record) = f.owners[owner.0].aggregate() else {
        unreachable!("checked record element owner");
    };
    let tag = 0x8000_0000u32 | record.0 as u32;
    let span = plan::instruction_span(statement);
    let pointer = base_pointer(out, f, &format!("{name}_vector"), base);
    let pointer = if path.is_empty() {
        pointer
    } else {
        let (actual, offset) = projection(plan, f, base, path);
        assert_eq!(actual, ValueTy::Owned(AggregateTy::RecordVector(record)));
        field_pointer(out, &format!("{name}_projection"), &pointer, offset)
    };
    let ordinal = load_operand(out, f, &format!("{name}_ordinal"), operand);
    if path.is_empty() && indexed_base(f, base) == BorrowedTy::RecordSlice(record) {
        let length = index_length(out, f, &format!("{name}_extent"), base);
        writeln!(out, "  %{name}_index_status = call i32 @__oxid_record_vector_index_slice(ptr {pointer}, i32 {tag}, i32 {length}, i32 {ordinal}, ptr %vector_index_pointer)").unwrap();
    } else {
        writeln!(out, "  %{name}_index_status = call i32 @__oxid_vector_index(ptr {pointer}, i32 {tag}, i32 {ordinal}, ptr %vector_index_pointer)").unwrap();
    }
    experimental::status(
        out,
        diagnostics,
        &format!("{name}_index"),
        &format!("%{name}_index_status"),
        "bounds_ok",
        span,
        false,
    );
    writeln!(
        out,
        "  %{name}_element = load ptr, ptr %vector_index_pointer, align 8"
    )
    .unwrap();
    if reading {
        writeln!(out, "  %{name}_status = call i32 @__oxid_record_vector_unpack(i32 {tag}, ptr %{name}_element, ptr %o{})", owner.0).unwrap();
    } else {
        writeln!(out, "  %{name}_pack_status = call i32 @__oxid_record_vector_pack(i32 {tag}, ptr %o{}, ptr %record_vector_scratch)", owner.0).unwrap();
        experimental::status(
            out,
            diagnostics,
            &format!("{name}_pack"),
            &format!("%{name}_pack_status"),
            "packed_ok",
            span,
            false,
        );
        writeln!(out, "  %{name}_status = call i32 @__oxid_record_vector_write(i32 {tag}, ptr %record_vector_scratch, ptr %{name}_element)").unwrap();
    }
    experimental::status(
        out,
        diagnostics,
        name,
        &format!("%{name}_status"),
        "aggregate_ok",
        span,
        false,
    );
}

// Named physical controls for the record-only C helpers. Catalog payload is
// read-only module data; these structures describe synchronous local roles.
#[allow(dead_code)]
#[repr(C)]
struct RecordSchemaFrame {
    element: u32,
    ordinal: u32,
    schema: *const (),
}
#[allow(dead_code)]
#[repr(C)]
struct RecordLayoutFrame {
    schema: *const (),
    dense: usize,
    owner: usize,
    index: u32,
    leaf: *const (),
    width: usize,
}
#[allow(dead_code)]
#[repr(C)]
struct RecordCanonicalFrame {
    element: u32,
    bytes: *const u8,
    length: usize,
    width: usize,
    schema: *const (),
    offset: usize,
    index: u32,
    leaf: *const (),
}
#[allow(dead_code)]
#[repr(C)]
struct RecordSourceFrame {
    heap: *mut (),
    element: u32,
    source: *const u8,
    length: usize,
    width: usize,
    node: *mut (),
    charge: u64,
    header: *const (),
    checked_charge: usize,
    first: usize,
    selected: usize,
    initialized: usize,
}
#[allow(dead_code)]
#[repr(C)]
struct RecordCellFrame {
    pointer: *const u8,
    element: u32,
    width: usize,
    node: *mut (),
    header: *const (),
    descriptor: [u8; 16],
    first: usize,
    selected: usize,
}
#[allow(dead_code)]
#[repr(C)]
struct RecordTransferFrame {
    element: u32,
    source: *const u8,
    destination: *mut u8,
    pack: i32,
    schema: *const (),
    input_bytes: usize,
    output_bytes: usize,
    indices: [u32; 2],
    leaves: [*const (); 2],
    offsets: [usize; 3],
}
#[allow(dead_code)]
#[repr(C)]
struct RecordWriteFrame {
    element: u32,
    packed: *const u8,
    destination: *mut u8,
    schema: *const (),
}
#[allow(dead_code)]
#[repr(C)]
struct RecordIndexSliceFrame {
    source: *const u8,
    element: u32,
    length: i32,
    ordinal: i32,
    pointer: *mut (),
    schema: *const (),
    selected: *mut u8,
}
#[allow(dead_code)]
struct RecordRuntimeCarriers {
    schemas: [RecordSchemaFrame; 3],
    layout: RecordLayoutFrame,
    canonical: RecordCanonicalFrame,
    source: RecordSourceFrame,
    cell: RecordCellFrame,
    transfer: RecordTransferFrame,
    write: RecordWriteFrame,
    slice: RecordIndexSliceFrame,
}
pub(super) const RECORD_RUNTIME_BYTES: usize = size_of::<RecordRuntimeCarriers>();
#[allow(dead_code)]
struct RecordEmitterCarriers {
    schema_rows: [std::slice::Iter<'static, RecordDecl>; 3],
    scalar_leaves: [ScalarLeaves<'static>; 2],
    schema_types: [VectorTy; 3],
    schema_ids: [RecordId; 3],
    cursor: usize,
    record_index_tuple: (AccessBase, &'static [FieldId], Operand, OwnerPlaceId, bool),
    index_strings: [String; 9],
    index_format: [FormatCallsiteBacking<8>; 3],
    schema_format: [FormatCallsiteBacking<8>; 3],
    lookup_results: [plan::native_vector_layout::IndexedAccessResult; 2],
    writes: [std::fmt::Result; 8],
}
pub(super) const fn record_control_bytes() -> usize {
    size_of::<RecordEmitterCarriers>() + 9 * NAME_BYTES
}
