//! Closed outcome ABI shared by both verified native emitters. Compare the full
//! signed i32 before widening the predicate; no exit-status truncation or stdout.
use super::hir::Ty;

pub(super) fn return_ir(result: Ty) -> &'static str {
    match result {
        Ty::Bool => "  %failed = xor i1 %value, true\n  %status = zext i1 %failed to i32\n  ret i32 %status\n}\n",
        Ty::I32 => "  %failed = icmp ne i32 %value, 0\n  %status = zext i1 %failed to i32\n  ret i32 %status\n}\n",
        Ty::Unit => "  ret i32 0\n}\n",
        Ty::U8 => unreachable!("byte test entry rejected before emission"),
    }
}
