//! LLVM bit operations guard shift counts before constructing a shift value.
use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn binary(
    out: &mut Emission,
    diagnostics: &Diagnostics,
    name: &str,
    op: hir::BitwiseOp,
    kind: hir::Ty,
    count_kind: hir::Ty,
    left: &str,
    right: &str,
    span: Span,
) -> String {
    let t = ty(kind);
    let instruction = match op {
        hir::BitwiseOp::And => "and",
        hir::BitwiseOp::Or => "or",
        hir::BitwiseOp::Xor => "xor",
        hir::BitwiseOp::ShiftLeft => "shl",
        hir::BitwiseOp::ShiftRight if kind.signed_integer() => "ashr",
        hir::BitwiseOp::ShiftRight => "lshr",
    };
    let count = if op.shift() {
        let count_t = ty(count_kind);
        let width = kind.integer_bits().expect("verified bitwise width");
        writeln!(
            out,
            "  %{name}_count_large = icmp uge {count_t} {right}, {width}"
        )
        .unwrap();
        let invalid = if count_kind.signed_integer() {
            writeln!(out, "  %{name}_count_negative = icmp slt {count_t} {right}, 0\n  %{name}_count_invalid = or i1 %{name}_count_negative, %{name}_count_large").unwrap();
            format!("%{name}_count_invalid")
        } else {
            format!("%{name}_count_large")
        };
        writeln!(out, "  br i1 {invalid}, label %{name}_shift_error, label %{name}_checked_ok\n{name}_shift_error:").unwrap();
        emit_failure(out, diagnostics, FailureKind::BitShift, span);
        writeln!(out, "{name}_checked_ok:").unwrap();
        let count_width = count_kind
            .integer_bits()
            .expect("verified shift count type");
        if count_width == width {
            right.to_string()
        } else {
            let conversion = if count_width < width { "zext" } else { "trunc" };
            writeln!(
                out,
                "  %{name}_count = {conversion} {count_t} {right} to {t}"
            )
            .unwrap();
            format!("%{name}_count")
        }
    } else {
        right.to_string()
    };
    writeln!(out, "  %{name}_value = {instruction} {t} {left}, {count}").unwrap();
    format!("%{name}_value")
}
