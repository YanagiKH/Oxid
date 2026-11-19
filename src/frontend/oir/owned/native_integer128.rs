//! Portable part-based helpers avoid target compiler-rt 128-bit ABI imports.
use super::*;

fn parts(out: &mut Emission, name: &str, value: &str) {
    writeln!(out, "  %{name}_low = trunc i128 {value} to i64\n  %{name}_shift = lshr i128 {value}, 64\n  %{name}_high = trunc i128 %{name}_shift to i64").unwrap();
}

fn result(out: &mut Emission, name: &str, first: usize) {
    writeln!(out, "  %{name}_result_low = load i64, ptr %integer128_word{first}, align 8\n  %{name}_result_high = load i64, ptr %integer128_word{}, align 8\n  %{name}_wide_low = zext i64 %{name}_result_low to i128\n  %{name}_wide_high = zext i64 %{name}_result_high to i128\n  %{name}_shifted_high = shl i128 %{name}_wide_high, 64\n  %{name}_magnitude = or i128 %{name}_wide_low, %{name}_shifted_high", first + 1).unwrap();
}

#[allow(clippy::too_many_arguments)]
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
    let signed = kind.signed_integer();
    if op == hir::ArithmeticOp::Multiply {
        parts(out, &format!("{name}_left"), left);
        parts(out, &format!("{name}_right"), right);
        writeln!(out, "  %{name}_status = call i32 @__oxid_checked_mul128_parts(i64 %{name}_left_low, i64 %{name}_left_high, i64 %{name}_right_low, i64 %{name}_right_high, i32 {}, ptr %integer128_word0, ptr %integer128_word1)\n  %{name}_overflow = icmp ne i32 %{name}_status, 0\n  br i1 %{name}_overflow, label %{name}_checked_error, label %{name}_checked_ok\n{name}_checked_error:", u8::from(signed)).unwrap();
        emit_failure(out, diagnostics, FailureKind::integer_overflow(kind), span);
        writeln!(out, "{name}_checked_ok:").unwrap();
        result(out, name, 0);
        return format!("%{name}_magnitude");
    }
    debug_assert!(matches!(
        op,
        hir::ArithmeticOp::Divide | hir::ArithmeticOp::Remainder
    ));
    writeln!(out, "  %{name}_zero = icmp eq i128 {right}, 0\n  br i1 %{name}_zero, label %{name}_division_error, label %{name}_nonzero\n{name}_division_error:").unwrap();
    emit_failure(
        out,
        diagnostics,
        FailureKind::integer_division_by_zero(kind),
        span,
    );
    writeln!(out, "{name}_nonzero:").unwrap();
    if signed {
        writeln!(out, "  %{name}_minimum = icmp eq i128 {left}, {}\n  %{name}_negative_one = icmp eq i128 {right}, -1\n  %{name}_overflow = and i1 %{name}_minimum, %{name}_negative_one\n  br i1 %{name}_overflow, label %{name}_checked_error, label %{name}_division_ready\n{name}_checked_error:", i128::MIN).unwrap();
        emit_failure(out, diagnostics, FailureKind::integer_overflow(kind), span);
        writeln!(out, "{name}_division_ready:\n  %{name}_left_negative = icmp slt i128 {left}, 0\n  %{name}_right_negative = icmp slt i128 {right}, 0\n  %{name}_left_negated = sub i128 0, {left}\n  %{name}_right_negated = sub i128 0, {right}\n  %{name}_left_abs = select i1 %{name}_left_negative, i128 %{name}_left_negated, i128 {left}\n  %{name}_right_abs = select i1 %{name}_right_negative, i128 %{name}_right_negated, i128 {right}").unwrap();
        parts(out, &format!("{name}_left"), &format!("%{name}_left_abs"));
        parts(out, &format!("{name}_right"), &format!("%{name}_right_abs"));
    } else {
        parts(out, &format!("{name}_left"), left);
        parts(out, &format!("{name}_right"), right);
    }
    writeln!(out, "  %{name}_status = call i32 @__oxid_udivmod128_parts(i64 %{name}_left_low, i64 %{name}_left_high, i64 %{name}_right_low, i64 %{name}_right_high, ptr %integer128_word0, ptr %integer128_word1, ptr %integer128_word2, ptr %integer128_word3)\n  %{name}_helper_failed = icmp ne i32 %{name}_status, 0\n  br i1 %{name}_helper_failed, label %{name}_division_error, label %{name}_checked_ok\n{name}_checked_ok:").unwrap();
    result(
        out,
        name,
        if op == hir::ArithmeticOp::Divide {
            0
        } else {
            2
        },
    );
    if signed {
        let negative = if op == hir::ArithmeticOp::Divide {
            writeln!(
                out,
                "  %{name}_negative = xor i1 %{name}_left_negative, %{name}_right_negative"
            )
            .unwrap();
            format!("%{name}_negative")
        } else {
            format!("%{name}_left_negative")
        };
        writeln!(out, "  %{name}_negated = sub i128 0, %{name}_magnitude\n  %{name}_value = select i1 {negative}, i128 %{name}_negated, i128 %{name}_magnitude").unwrap();
        format!("%{name}_value")
    } else {
        format!("%{name}_magnitude")
    }
}
