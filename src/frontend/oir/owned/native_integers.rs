//! LLVM operations for exact signed and unsigned integer widths.
use super::*;

pub(super) fn literal(kind: hir::Ty, bits: u64) -> String {
    if kind.signed_integer() {
        (bits as i64).to_string()
    } else {
        bits.to_string()
    }
}

pub(super) fn negate(
    out: &mut Emission,
    diagnostics: &Diagnostics,
    name: &str,
    kind: hir::Ty,
    operand: &str,
    span: Span,
) -> String {
    let t = ty(kind);
    writeln!(out, "  %{name}_checked = call {{ {t}, i1 }} @llvm.ssub.with.overflow.{t}({t} 0, {t} {operand})\n  %{name}_overflow = extractvalue {{ {t}, i1 }} %{name}_checked, 1\n  br i1 %{name}_overflow, label %{name}_checked_error, label %{name}_checked_ok\n{name}_checked_error:").unwrap();
    emit_failure(out, diagnostics, FailureKind::integer_overflow(kind), span);
    writeln!(
        out,
        "{name}_checked_ok:\n  %{name}_value = extractvalue {{ {t}, i1 }} %{name}_checked, 0"
    )
    .unwrap();
    format!("%{name}_value")
}

#[allow(clippy::too_many_arguments)] // One verified arithmetic instruction and its source origin.
pub(super) fn arithmetic(
    out: &mut Emission,
    diagnostics: &Diagnostics,
    name: &str,
    kind: hir::Ty,
    op: hir::ArithmeticOp,
    left: &str,
    right: &str,
    span: Span,
) -> String {
    if kind.quad_integer()
        && matches!(
            op,
            hir::ArithmeticOp::Multiply | hir::ArithmeticOp::Divide | hir::ArithmeticOp::Remainder
        )
    {
        return integer128::arithmetic(out, diagnostics, name, kind, op, left, right, span);
    }
    let t = ty(kind);
    let signed = kind.signed_integer();
    if matches!(op, hir::ArithmeticOp::Divide | hir::ArithmeticOp::Remainder) {
        writeln!(out, "  %{name}_zero = icmp eq {t} {right}, 0\n  br i1 %{name}_zero, label %{name}_division_error, label %{name}_nonzero\n{name}_division_error:").unwrap();
        emit_failure(
            out,
            diagnostics,
            FailureKind::integer_division_by_zero(kind),
            span,
        );
        writeln!(out, "{name}_nonzero:").unwrap();
        if signed {
            let minimum = -(1i128 << (kind.integer_bits().expect("integer width") - 1));
            writeln!(out, "  %{name}_minimum = icmp eq {t} {left}, {minimum}\n  %{name}_negative_one = icmp eq {t} {right}, -1\n  %{name}_overflow = and i1 %{name}_minimum, %{name}_negative_one\n  br i1 %{name}_overflow, label %{name}_checked_error, label %{name}_checked_ok\n{name}_checked_error:").unwrap();
            emit_failure(out, diagnostics, FailureKind::integer_overflow(kind), span);
        } else {
            writeln!(out, "  br label %{name}_checked_ok").unwrap();
        }
        let instruction = match (signed, op) {
            (true, hir::ArithmeticOp::Divide) => "sdiv",
            (true, hir::ArithmeticOp::Remainder) => "srem",
            (false, hir::ArithmeticOp::Divide) => "udiv",
            (false, hir::ArithmeticOp::Remainder) => "urem",
            _ => unreachable!("division opcode"),
        };
        writeln!(
            out,
            "{name}_checked_ok:\n  %{name}_value = {instruction} {t} {left}, {right}"
        )
        .unwrap();
    } else {
        let operation = match op {
            hir::ArithmeticOp::Add => "add",
            hir::ArithmeticOp::Subtract => "sub",
            hir::ArithmeticOp::Multiply => "mul",
            _ => unreachable!("overflow arithmetic opcode"),
        };
        let prefix = if signed { "s" } else { "u" };
        writeln!(out, "  %{name}_checked = call {{ {t}, i1 }} @llvm.{prefix}{operation}.with.overflow.{t}({t} {left}, {t} {right})\n  %{name}_overflow = extractvalue {{ {t}, i1 }} %{name}_checked, 1\n  br i1 %{name}_overflow, label %{name}_checked_error, label %{name}_checked_ok\n{name}_checked_error:").unwrap();
        emit_failure(out, diagnostics, FailureKind::integer_overflow(kind), span);
        writeln!(
            out,
            "{name}_checked_ok:\n  %{name}_value = extractvalue {{ {t}, i1 }} %{name}_checked, 0"
        )
        .unwrap();
    }
    format!("%{name}_value")
}

#[allow(clippy::too_many_arguments)] // A checked conversion's complete input/output types and origin.
pub(super) fn cast(
    out: &mut Emission,
    diagnostics: &Diagnostics,
    name: &str,
    input: hir::Ty,
    target: hir::Ty,
    operand: &str,
    span: Span,
) -> String {
    let source_width = input.integer_bits().expect("integer source");
    let target_width = target.integer_bits().expect("integer target");
    let t = ty(input);
    let source_signed = input.signed_integer();
    let target_signed = target.signed_integer();
    let low = if source_signed {
        if !target_signed {
            Some(0i128)
        } else if target_width < source_width {
            Some(-(1i128 << (target_width - 1)))
        } else {
            None
        }
    } else {
        None
    };
    let high = if target_width < source_width
        || (target_width == source_width && target_signed && !source_signed)
    {
        Some((1u128 << (target_width - u8::from(target_signed))) - 1)
    } else {
        None
    };
    if let Some(minimum) = low {
        writeln!(out, "  %{name}_low = icmp slt {t} {operand}, {minimum}").unwrap();
    }
    if let Some(maximum) = high {
        let predicate = if source_signed { "sgt" } else { "ugt" };
        writeln!(
            out,
            "  %{name}_high = icmp {predicate} {t} {operand}, {maximum}"
        )
        .unwrap();
    }
    let condition = match (low, high) {
        (Some(_), Some(_)) => {
            writeln!(out, "  %{name}_invalid = or i1 %{name}_low, %{name}_high").unwrap();
            format!("%{name}_invalid")
        }
        (Some(_), None) => format!("%{name}_low"),
        (None, Some(_)) => format!("%{name}_high"),
        (None, None) => "false".to_string(),
    };
    writeln!(out, "  br i1 {condition}, label %{name}_conversion_error, label %{name}_conversion_ok\n{name}_conversion_error:").unwrap();
    emit_failure(
        out,
        diagnostics,
        FailureKind::integer_range(input, target),
        span,
    );
    writeln!(out, "{name}_conversion_ok:").unwrap();
    if source_width == target_width {
        return operand.to_string();
    }
    let conversion = if target_width < source_width {
        "trunc"
    } else if source_signed {
        "sext"
    } else {
        "zext"
    };
    writeln!(
        out,
        "  %{name}_value = {conversion} {t} {operand} to {}",
        ty(target)
    )
    .unwrap();
    format!("%{name}_value")
}
