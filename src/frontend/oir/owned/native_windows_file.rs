//! Denial over an already verified witness, never a source spelling or producer.
use super::*;
use crate::frontend::builtin_catalog::{experimental, BuiltinSet};

#[allow(dead_code)]
struct ControlCarriers {
    witness: &'static RawOwnedProgram,
    witness_argument: &'static (),
    entry_argument: hir::DefId,
    entry_function: Option<&'static RawOwnedFunction>,
    function_iterator: std::slice::Iter<'static, RawOwnedFunction>,
    selected_function: &'static RawOwnedFunction,
    local_iterator: std::slice::Iter<'static, LocalDecl>,
    selected_local: &'static LocalDecl,
    owner_iterator: std::slice::Iter<'static, OwnerDecl>,
    selected_owner: &'static OwnerDecl,
    reference_iterator: std::slice::Iter<'static, ReferenceDecl>,
    selected_reference: &'static ReferenceDecl,
    enum_iterator: std::slice::Iter<'static, RawEnumDecl>,
    selected_enum: &'static RawEnumDecl,
    allowed_enumerations: [Option<EnumId>; 4],
    enumeration_rank: Option<usize>,
    enumeration_base: Option<usize>,
    enumeration_index: Option<usize>,
    enumeration_iterator: std::iter::Enumerate<std::array::IntoIter<experimental::Enumeration, 4>>,
    selected_enumeration: (usize, experimental::Enumeration),
    enumeration_argument: &'static [Option<EnumId>; 4],
    enum_identity: EnumId,
    enum_comparisons: [Option<EnumId>; 4],
    operation_iterator: std::array::IntoIter<experimental::Function, 20>,
    operation: experimental::Function,
    operation_identity: Option<hir::DefId>,
    aggregate_argument: AggregateTy,
    value_argument: ValueTy,
    reference_argument: BorrowedTy,
    scalar_argument: hir::Ty,
    accepted: bool,
    scalar_returns: [bool; 2],
    aggregate_returns: [bool; 3],
}
pub(super) const fn control_bytes() -> usize {
    size_of::<ControlCarriers>() + crate::frontend::windows_native::file_routing_control_bytes()
}

pub(super) fn allowed<W: PlanWitness>(witness: &W, entry: hir::DefId) -> bool {
    let raw = witness.raw_program();
    let Some(root) = witness.functions().get(entry.0) else {
        return false;
    };
    if !matches!(
        root.result,
        ValueTy::Scalar(hir::Ty::Bool | hir::Ty::I32 | hir::Ty::Unit)
    ) {
        return false;
    }
    if !raw.records.is_empty()
        || !raw.vector_functions.is_empty()
        || witness.uses_owned_vectors()
        || raw.builtins.legacy() != BuiltinSet::None
    {
        return false;
    }
    let mut has_file = false;
    for operation in experimental::Function::ALL {
        if witness.experimental_function(operation).is_some() {
            if !operation.is_file() {
                return false;
            }
            has_file = true;
        }
    }
    if !has_file {
        return false;
    }
    let mut enumerations = [None; 4];
    for (position, item) in [
        experimental::Enumeration::Read,
        experimental::Enumeration::Write,
        experimental::Enumeration::Open,
        experimental::Enumeration::File,
    ]
    .into_iter()
    .enumerate()
    {
        if let Some(rank) = raw.builtins.experimental_enum_rank(item) {
            let Some(base) = raw.enums.len().checked_sub(raw.builtins.extra_enums()) else {
                return false;
            };
            let Some(index) = base.checked_add(rank) else {
                return false;
            };
            enumerations[position] = Some(EnumId(index));
        }
    }
    for enumeration in &raw.enums {
        if !canonical_enum(enumeration.id, &enumerations) {
            return false;
        }
    }
    for function in witness.functions() {
        if !match function.result {
            ValueTy::Scalar(ty) => scalar(ty),
            ValueTy::Owned(ty) => aggregate(ty, &enumerations),
        } {
            return false;
        }
        for local in &function.locals {
            if !scalar(local.ty) {
                return false;
            }
        }
        for owner in &function.owners {
            if !aggregate(owner.aggregate(), &enumerations) {
                return false;
            }
        }
        for reference in &function.references {
            if !match reference.referent() {
                BorrowedTy::Exact(ty) => aggregate(ty, &enumerations),
                BorrowedTy::ScalarSlice(ty) => ty == hir::Ty::U8,
                BorrowedTy::RecordSlice(_) => false,
            } {
                return false;
            }
        }
    }
    true
}
fn aggregate(ty: AggregateTy, enumerations: &[Option<EnumId>; 4]) -> bool {
    match ty {
        AggregateTy::File => true,
        AggregateTy::FixedArray(array) => array.element() == hir::Ty::U8,
        AggregateTy::Enum(id) => canonical_enum(id, enumerations),
        _ => false,
    }
}
fn canonical_enum(id: EnumId, allowed: &[Option<EnumId>; 4]) -> bool {
    Some(id) == allowed[0]
        || Some(id) == allowed[1]
        || Some(id) == allowed[2]
        || Some(id) == allowed[3]
}
fn scalar(ty: hir::Ty) -> bool {
    matches!(
        ty,
        hir::Ty::Bool | hir::Ty::I32 | hir::Ty::Unit | hir::Ty::U8 | hir::Ty::I64
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::oir::owned::source::resource_fixtures::{checked, checked_enum_project};

    const API: &str = "use std::fs::File;use std::fs::FileStatus;use std::fs::close;fn shutdown(file:&mut File)->FileStatus{return close(&mut *file);}";

    #[test]
    fn native_file_denial_uses_actual_verified_types_and_canonical_identities() {
        let accepted = checked_enum_project(&format!(
            "{API}fn main()->i32{{let bytes=b\"bytes\";if bytes[0]==98u8{{return 42;}}return 0;}}"
        ));
        assert!(allowed(&accepted.witness, accepted.entry));
        for declaration in [
            "struct Other{value:i32}",
            "enum Other{Value}",
            "use alloc::bytes::new;fn other()->(){let bytes=new();return;}",
            "use alloc::vec::new;fn other()->(){let values=new::<i32>();return;}",
            "fn other(value:f64)->f64{return value;}",
            "fn other(bytes:&[i32])->i32{return bytes[0];}",
            "fn other()->i32{let bytes:[i32;1]=[42];return bytes[0];}",
        ] {
            let rejected =
                checked_enum_project(&format!("{API}{declaration}fn main()->i32{{return 42;}}"));
            assert!(!allowed(&rejected.witness, rejected.entry), "{declaration}");
        }
        let lookalike = checked("struct File{}fn main()->i32{let file=File{};return 42;}");
        assert!(!allowed(&lookalike.witness, lookalike.entry));
        let wide_entry = checked_enum_project(&format!("{API}fn main()->i64{{return 42i64;}}"));
        assert!(!allowed(&wide_entry.witness, wide_entry.entry));
    }

    #[cfg(all(
        target_os = "windows",
        target_env = "msvc",
        target_arch = "x86_64",
        target_pointer_width = "64",
        target_endian = "little"
    ))]
    #[test]
    fn windows_file_control_metadata_exact_limit_and_one_less_precede_emission() {
        fn inventory(bank: &ControlCarriers) {
            let ControlCarriers {
                witness,
                witness_argument,
                entry_argument,
                entry_function,
                function_iterator,
                selected_function,
                local_iterator,
                selected_local,
                owner_iterator,
                selected_owner,
                reference_iterator,
                selected_reference,
                enum_iterator,
                selected_enum,
                allowed_enumerations,
                enumeration_rank,
                enumeration_base,
                enumeration_index,
                enumeration_iterator,
                selected_enumeration,
                enumeration_argument,
                enum_identity,
                enum_comparisons,
                operation_iterator,
                operation,
                operation_identity,
                aggregate_argument,
                value_argument,
                reference_argument,
                scalar_argument,
                accepted,
                scalar_returns,
                aggregate_returns,
            } = bank;
            let _ = (
                witness,
                witness_argument,
                entry_argument,
                entry_function,
                function_iterator,
                selected_function,
                local_iterator,
                selected_local,
                owner_iterator,
                selected_owner,
                reference_iterator,
                selected_reference,
                enum_iterator,
                selected_enum,
                allowed_enumerations,
                enumeration_rank,
                enumeration_base,
                enumeration_index,
                enumeration_iterator,
                selected_enumeration,
                enumeration_argument,
                enum_identity,
                enum_comparisons,
                operation_iterator,
                operation,
                operation_identity,
                aggregate_argument,
                value_argument,
                reference_argument,
                scalar_argument,
                accepted,
                scalar_returns,
                aggregate_returns,
            );
        }
        let _: fn(&ControlCarriers) = inventory;
        println!(
            "Windows native File witness scan={} align={} routing={} total={}",
            size_of::<ControlCarriers>(),
            std::mem::align_of::<ControlCarriers>(),
            crate::frontend::windows_native::file_routing_control_bytes(),
            control_bytes()
        );
        let checked = checked_enum_project(&format!("{API}fn main()->i32{{return 42;}}"));
        let compile = |limits| {
            let mut accounting = Accounting::default();
            let result = native_module_policy_accounted_with_selection::<_, reachability::Full>(
                &checked.witness,
                Some(checked.entry),
                checked.project.sources(),
                plan::MAX_FUEL,
                limits,
                NativeEntryPolicy::TypedResult,
                &mut accounting,
                None,
                false,
            );
            (result, accounting.metrics)
        };
        let limits = Limits::for_policy(NativeEntryPolicy::TypedResult);
        let (result, observed) = compile(limits);
        assert!(result.is_ok());
        assert!(observed.emitter_transient_bound >= control_bytes());
        let (exact, _) = compile(Limits {
            metadata_bytes: observed.metadata_peak,
            ..limits
        });
        assert!(exact.is_ok());
        let (short, refused) = compile(Limits {
            metadata_bytes: observed.metadata_peak - 1,
            ..limits
        });
        assert_eq!(short.unwrap_err().code, "E0700");
        assert_eq!(refused.count_bytes, 0);
        assert_eq!(refused.render_bytes, 0);
    }
}
