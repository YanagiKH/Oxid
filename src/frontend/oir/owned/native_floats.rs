//! IEEE operations use ordinary LLVM floating-point instructions, without fast math.
use super::*;
fn canonical(out: &mut Emission, name: &str, target: hir::Ty, value: &str) -> String {
    let (integer, bits) = if target == hir::Ty::F32 {
        ("i32", "2143289344")
    } else {
        ("i64", "9221120237041090560")
    };
    writeln!(out, "  %{name}_nan = fcmp uno {} {value}, {value}\n  %{name}_canonical = bitcast {integer} {bits} to {}\n  %{name}_value = select i1 %{name}_nan, {} %{name}_canonical, {} {value}", ty(target), ty(target), ty(target), ty(target)).unwrap();
    format!("%{name}_value")
}
pub(super) fn arithmetic(
    out: &mut Emission,
    name: &str,
    target: hir::Ty,
    operation: hir::ArithmeticOp,
    a: &str,
    b: &str,
) -> String {
    let op = match operation {
        hir::ArithmeticOp::Add => "fadd",
        hir::ArithmeticOp::Subtract => "fsub",
        hir::ArithmeticOp::Multiply => "fmul",
        hir::ArithmeticOp::Divide => "fdiv",
        hir::ArithmeticOp::Remainder => "frem",
    };
    writeln!(out, "  %{name}_raw = {op} {} {a}, {b}", ty(target)).unwrap();
    canonical(out, name, target, &format!("%{name}_raw"))
}
pub(super) fn convert(
    out: &mut Emission,
    name: &str,
    input: hir::Ty,
    target: hir::Ty,
    operation: hir::ConversionOp,
    value: &str,
) -> String {
    if input == target {
        return value.to_string();
    }
    let op = match operation {
        hir::ConversionOp::FloatFromBits(_) | hir::ConversionOp::FloatToBits => "bitcast",
        hir::ConversionOp::ToFloat(_) if input.floating() => {
            if target == hir::Ty::F32 {
                "fptrunc"
            } else {
                "fpext"
            }
        }
        hir::ConversionOp::ToFloat(_) => {
            if input.signed_integer() {
                "sitofp"
            } else {
                "uitofp"
            }
        }
        _ => unreachable!("verified float conversion"),
    };
    writeln!(
        out,
        "  %{name}_raw = {op} {} {value} to {}",
        ty(input),
        ty(target)
    )
    .unwrap();
    if input.floating() && target.floating() {
        canonical(out, name, target, &format!("%{name}_raw"))
    } else {
        format!("%{name}_raw")
    }
}
