//! Checked full-width arithmetic; LLVM never evaluates an invalid sdiv/srem.
use super::*;

pub(super) fn widen(out: &mut Emission, name: &str, input: hir::Ty, operand: &str) -> String {
    let extension = if input == hir::Ty::I32 {
        "sext"
    } else {
        "zext"
    };
    writeln!(
        out,
        "  %{name}_value = {extension} {} {operand} to i64",
        ty(input)
    )
    .unwrap();
    format!("%{name}_value")
}

pub(super) fn narrow(
    out: &mut Emission,
    diagnostics: &Diagnostics,
    name: &str,
    target: hir::Ty,
    operand: &str,
    span: Span,
) -> String {
    let (minimum, maximum, failure) = match target {
        hir::Ty::I32 => ("-2147483648", "2147483647", FailureKind::I64ToI32Range),
        hir::Ty::U8 => ("0", "255", FailureKind::I64ToU8Range),
        _ => unreachable!("verified narrowing destination"),
    };
    writeln!(out, "  %{name}_negative = icmp slt i64 {operand}, {minimum}\n  %{name}_large = icmp sgt i64 {operand}, {maximum}\n  %{name}_invalid = or i1 %{name}_negative, %{name}_large\n  br i1 %{name}_invalid, label %{name}_conversion_error, label %{name}_conversion_ok\n{name}_conversion_error:").unwrap();
    emit_failure(out, diagnostics, failure, span);
    writeln!(
        out,
        "{name}_conversion_ok:\n  %{name}_value = trunc i64 {operand} to {}",
        ty(target)
    )
    .unwrap();
    format!("%{name}_value")
}

pub(super) fn negate(
    out: &mut Emission,
    diagnostics: &Diagnostics,
    name: &str,
    operand: &str,
    span: Span,
) -> String {
    writeln!(out, "  %{name}_checked = call {{ i64, i1 }} @llvm.ssub.with.overflow.i64(i64 0, i64 {operand})\n  %{name}_overflow = extractvalue {{ i64, i1 }} %{name}_checked, 1\n  br i1 %{name}_overflow, label %{name}_checked_error, label %{name}_checked_ok\n{name}_checked_error:").unwrap();
    emit_failure(out, diagnostics, FailureKind::WideOverflow, span);
    writeln!(
        out,
        "{name}_checked_ok:\n  %{name}_value = extractvalue {{ i64, i1 }} %{name}_checked, 0"
    )
    .unwrap();
    format!("%{name}_value")
}

#[allow(clippy::too_many_arguments)] // A single checked scalar instruction's complete operands and origin.
pub(super) fn arithmetic(
    out: &mut Emission,
    diagnostics: &Diagnostics,
    name: &str,
    op: hir::ArithmeticOp,
    left: &str,
    right: &str,
    span: Span,
) -> String {
    match op {
        hir::ArithmeticOp::Divide | hir::ArithmeticOp::Remainder => {
            writeln!(out, "  %{name}_zero = icmp eq i64 {right}, 0\n  br i1 %{name}_zero, label %{name}_division_error, label %{name}_nonzero\n{name}_division_error:").unwrap();
            emit_failure(out, diagnostics, FailureKind::WideDivisionByZero, span);
            writeln!(out, "{name}_nonzero:\n  %{name}_min = icmp eq i64 {left}, -9223372036854775808\n  %{name}_negative_one = icmp eq i64 {right}, -1\n  %{name}_overflow = and i1 %{name}_min, %{name}_negative_one").unwrap();
        }
        _ => {
            let intrinsic = match op {
                hir::ArithmeticOp::Add => "sadd",
                hir::ArithmeticOp::Subtract => "ssub",
                hir::ArithmeticOp::Multiply => "smul",
                _ => unreachable!("checked intrinsic"),
            };
            writeln!(out, "  %{name}_checked = call {{ i64, i1 }} @llvm.{intrinsic}.with.overflow.i64(i64 {left}, i64 {right})\n  %{name}_overflow = extractvalue {{ i64, i1 }} %{name}_checked, 1").unwrap();
        }
    }
    writeln!(out, "  br i1 %{name}_overflow, label %{name}_checked_error, label %{name}_checked_ok\n{name}_checked_error:").unwrap();
    emit_failure(out, diagnostics, FailureKind::WideOverflow, span);
    writeln!(out, "{name}_checked_ok:").unwrap();
    match op {
        hir::ArithmeticOp::Divide | hir::ArithmeticOp::Remainder => {
            let instruction = if op == hir::ArithmeticOp::Divide {
                "sdiv"
            } else {
                "srem"
            };
            writeln!(out, "  %{name}_value = {instruction} i64 {left}, {right}").unwrap();
        }
        _ => writeln!(
            out,
            "  %{name}_value = extractvalue {{ i64, i1 }} %{name}_checked, 0"
        )
        .unwrap(),
    }
    format!("%{name}_value")
}
