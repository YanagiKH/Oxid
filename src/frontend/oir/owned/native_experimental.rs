//! Fixed native primitive and call-only byte-view emission. Every identity and
//! parameter/result slot comes from the sealed witness's canonical descriptor.
use super::*;
use crate::frontend::builtin_catalog::experimental::Function;

// Matches the private C adapter's asserted inline control carrier. No logical
// owner/activation cells, heap allocations or ordinary-program layout changes.
pub(super) const BINARY_DELIVERY_BYTES: usize = 16;

#[allow(clippy::too_many_arguments)] // Complete preflighted failure labels; no retained state.
pub(super) fn status(
    out: &mut Emission,
    diagnostics: &Diagnostics,
    name: &str,
    status: &str,
    suffix: &str,
    span: Span,
    operation: bool,
) {
    writeln!(out, "  switch i32 {status}, label %{name}_resource_error [\n    i32 0, label %{name}_{suffix}\n    i32 -1, label %{name}_bounds_error").unwrap();
    if operation {
        writeln!(
            out,
            "    i32 -2, label %{name}_fuel_error\n    i32 -4, label %{name}_identity_error"
        )
        .unwrap();
    }
    writeln!(out, "  ]\n{name}_bounds_error:").unwrap();
    emit_failure(out, diagnostics, FailureKind::Bounds, span);
    if operation {
        writeln!(out, "{name}_fuel_error:").unwrap();
        emit_failure(out, diagnostics, FailureKind::Fuel, span);
        writeln!(out, "{name}_identity_error:").unwrap();
        emit_failure(out, diagnostics, FailureKind::AllocationIdentity, span);
    }
    writeln!(out, "{name}_resource_error:").unwrap();
    emit_failure(out, diagnostics, FailureKind::ResourceDescriptor, span);
    writeln!(out, "{name}_{suffix}:").unwrap();
}

#[allow(clippy::too_many_arguments)] // Closed instruction and existing emission context.
pub(super) fn operation(
    storage: &NativeFunctionStorage<'_, '_>,
    name: &str,
    operation: Function,
    destination: CallResult,
    diagnostics: &Diagnostics,
    span: Span,
    out: &mut Emission,
) {
    let f = &storage.execution().witness().functions()[storage.id().0];
    let pointer = if operation == Function::New {
        "null".to_owned()
    } else {
        base_pointer(
            out,
            &format!("{name}_buffer"),
            AccessBase::Parameter(ReferenceParamId(0)),
        )
    };
    let scalar = |out: &mut Emission, ordinal: usize, label: &str| {
        load_operand(
            out,
            f,
            &format!("{name}_{label}"),
            Operand {
                local: LocalId(ordinal),
                span,
            },
        )
    };
    let size = match operation {
        Function::Reserve | Function::Resize | Function::Truncate => scalar(out, 0, "size"),
        Function::ReadSome | Function::WriteSome => index_length(
            out,
            f,
            &format!("{name}_extent"),
            AccessBase::Parameter(ReferenceParamId(0)),
        ),
        _ => "0".to_owned(),
    };
    let fill = match operation {
        Function::Push | Function::Resize => {
            let byte = scalar(out, usize::from(operation == Function::Resize), "byte");
            writeln!(out, "  %{name}_fill = zext i8 {byte} to i32").unwrap();
            format!("%{name}_fill")
        }
        _ => "0".to_owned(),
    };
    let (source, length) = if operation == Function::Extend {
        (
            base_pointer(
                out,
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
        CallResult::Owned(owner) => format!("%o{}", owner.0),
        CallResult::Scalar(local) => format!("%s{}", local.0),
    };
    let code = match operation {
        Function::New => 0,
        Function::Len => 1,
        Function::Capacity => 2,
        Function::Reserve => 3,
        Function::Push => 4,
        Function::Extend => 5,
        Function::Resize => 6,
        Function::Truncate => 7,
        Function::ReadSome => 8,
        Function::WriteSome => 9,
        Function::Take => 10,
    };
    if matches!(operation, Function::ReadSome | Function::WriteSome) {
        let cost = storage
            .execution()
            .binary_delivery_cost(f.id)
            .expect("sealed binary delivery");
        // %result is the caller's preadmitted owner, including ignored and
        // nested-call results. The local owner/layout remains unchanged.
        writeln!(out, "  %{name}_status = call i32 @__oxid_binary_delivery(i32 {code}, ptr {pointer}, i32 {size}, ptr %fuel, ptr %result, i64 {cost})").unwrap();
    } else {
        writeln!(out, "  %{name}_status = call i32 @__oxid_bytebuf_operation(i32 {code}, ptr {pointer}, i32 {size}, i32 {fill}, ptr {source}, i32 {length}, ptr %fuel, ptr {result})").unwrap();
    }
    status(
        out,
        diagnostics,
        name,
        &format!("%{name}_status"),
        "operation_ok",
        span,
        true,
    );
}

pub(super) fn resource_borrow(
    plan: &ExecutionPlan<'_>,
    f: &RawOwnedFunction,
    loan: LoanId,
) -> bool {
    let declaration = &f.loans[loan.0];
    if declaration.projection.is_empty() {
        indexed_base(f, declaration.authority) == BorrowedTy::Exact(AggregateTy::ByteBuf)
    } else {
        projection(plan, f, declaration.authority, &declaration.projection).0
            == ValueTy::Owned(AggregateTy::ByteBuf)
    }
}
#[allow(clippy::too_many_arguments)] // Closed loan, endpoint operands and existing emission context.
pub(super) fn borrow(
    storage: &NativeFunctionStorage<'_, '_>,
    name: &str,
    loan: LoanId,
    range: Option<(Operand, Operand)>,
    diagnostics: &Diagnostics,
    span: Span,
    out: &mut Emission,
) {
    let plan = storage.execution();
    let f = &plan.witness().functions()[storage.id().0];
    let declaration = &f.loans[loan.0];
    let base = base_pointer(out, name, declaration.authority);
    let (pointer, actual) = if declaration.projection.is_empty() {
        (base, indexed_base(f, declaration.authority))
    } else {
        out.borrow_projection_visits += declaration.projection.len();
        let (value, offset) = projection(plan, f, declaration.authority, &declaration.projection);
        let ValueTy::Owned(aggregate) = value else {
            unreachable!("checked byte projection")
        };
        (
            field_pointer(out, &format!("{name}_projection"), &base, offset),
            BorrowedTy::Exact(aggregate),
        )
    };
    let (start, end) = range.map_or_else(
        || ("0".to_owned(), "0".to_owned()),
        |(start, end)| {
            (
                load_operand(out, f, &format!("{name}_start"), start),
                load_operand(out, f, &format!("{name}_end"), end),
            )
        },
    );
    if actual == BorrowedTy::Exact(AggregateTy::ByteBuf) {
        if declaration.referent() == BorrowedTy::Exact(AggregateTy::ByteBuf) {
            writeln!(
                out,
                "  store ptr {pointer}, ptr %r{}, align 8",
                storage.loan_slot(loan)
            )
            .unwrap();
        } else {
            writeln!(out, "  %{name}_status = call i32 @__oxid_bytebuf_view(ptr {pointer}, i32 {start}, i32 {end}, i32 {}, ptr %r{}, ptr %ll{})", i32::from(range.is_some()), storage.loan_slot(loan), loan.0).unwrap();
            status(
                out,
                diagnostics,
                name,
                &format!("%{name}_status"),
                "bounds_ok",
                span,
                false,
            );
        }
        return;
    }
    let length = match actual {
        BorrowedTy::Exact(AggregateTy::FixedArray(array)) => array.length().to_string(),
        BorrowedTy::ScalarSlice(hir::Ty::U8) => {
            index_length(out, f, &format!("{name}_extent"), declaration.authority)
        }
        _ => unreachable!("checked byte range"),
    };
    writeln!(out, "  %{name}_start_ok = icmp sge i32 {start}, 0\n  %{name}_ordered = icmp sle i32 {start}, {end}\n  %{name}_end_ok = icmp sle i32 {end}, {length}\n  %{name}_prefix_ok = and i1 %{name}_start_ok, %{name}_ordered\n  %{name}_range_ok = and i1 %{name}_prefix_ok, %{name}_end_ok\n  br i1 %{name}_range_ok, label %{name}_bounds_ok, label %{name}_bounds_error\n{name}_bounds_error:").unwrap();
    emit_failure(out, diagnostics, FailureKind::Bounds, span);
    writeln!(out, "{name}_bounds_ok:\n  %{name}_start64 = zext i32 {start} to i64\n  %{name}_pointer = getelementptr i8, ptr {pointer}, i64 %{name}_start64\n  %{name}_range_length = sub i32 {end}, {start}\n  store ptr %{name}_pointer, ptr %r{}, align 8\n  store i32 %{name}_range_length, ptr %ll{}, align 4", storage.loan_slot(loan), loan.0).unwrap();
}
