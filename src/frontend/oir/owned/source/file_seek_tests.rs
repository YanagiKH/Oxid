//! Seek is an exact canonical function, not permission attached to its spelling.
use super::*;
use crate::frontend::builtin_catalog::experimental::Function;

const VALID: &str = "use std::fs::File;use std::fs::FileStatus;use std::fs::seek_absolute as seek;fn position(file:&mut File,at:i64)->FileStatus{return seek(&mut *file,at);}fn main()->i32{return 42;}";

#[test]
fn file_seek_raw_signature_operation_and_function_identity_remain_independent_checks() {
    crate::frontend::oir::owned::source::record_pattern_tests::with_typed(
        VALID,
        |typed, sources| {
            use crate::frontend::oir::owned::source::{association, lower};
            let good = lower::lower(typed).unwrap();
            let witness = verified::verify_owned(good, sources).unwrap();
            let id = witness
                .experimental_function(Function::SeekAbsolute)
                .unwrap();
            for mutation in 0..5 {
                let mut raw = lower::lower(typed).unwrap();
                let function = &mut raw.functions[id.0];
                match mutation {
                    0 => function.locals[0].ty = hir::Ty::I32,
                    1 => function.parameters.swap(0, 1),
                    2 => function.references[0].kind = BorrowKind::Shared,
                    3 => {
                        let statement = function.blocks[0]
                            .statements
                            .iter_mut()
                            .find(|s| matches!(s.kind, OwnedInstruction::Experimental { .. }))
                            .unwrap();
                        let OwnedInstruction::Experimental { operation, .. } = &mut statement.kind
                        else {
                            unreachable!()
                        };
                        *operation = Function::Close;
                    }
                    4 => {
                        function.references[0].referent =
                            BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::ByteBuf)).unwrap()
                    }
                    _ => unreachable!(),
                }
                assert!(
                    verified::verify_owned(raw, sources).is_err(),
                    "mutation {mutation}"
                );
            }
            association::associate(lower::lower(typed).unwrap(), typed).unwrap();
        },
    );
}

#[test]
#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn file_seek_source_requires_exclusive_file_and_exact_i64_without_alias_or_move_escape() {
    const DECLARATIONS: &str =
        "use std::fs::File;use std::fs::FileStatus;use std::fs::seek_absolute as seek;";
    for body in [
        "fn f(file:&File)->FileStatus{return seek(&*file,0i64);}fn main()->i32{return 0;}",
        "fn f(file:&mut File)->FileStatus{return seek(&mut *file,0);}fn main()->i32{return 0;}",
        "use alloc::bytes::new;fn main()->i32{let mut b=new();seek(&mut b,0i64);return 0;}",
        "fn offset(file:&File)->i64{return 0i64;}fn f(file:&mut File)->FileStatus{return seek(&mut *file,offset(&*file));}fn main()->i32{return 0;}",
        "fn f(file:File)->i32{let other=file;let mut changed=file;seek(&mut changed,0i64);return 0;}fn main()->i32{return 0;}",
    ] {
        let loaded = project(&format!("{DECLARATIONS}{body}"));
        let errors = check_enum_source(SourceOwner::project(&loaded)).unwrap_err();
        assert!(!errors.is_empty(), "{body}");
        assert!(errors.iter().all(|e| e.code != "E0500"), "{body}: {errors:?}");
    }
    let loaded = project(VALID);
    let (program, entry) = check_enum_source(SourceOwner::project(&loaded)).unwrap();
    let plan = plan::ExecutionPlan::build(&program.witness).unwrap();
    assert!(plan.has_file_seek());
    let module = program.native_module(entry, loaded.sources()).unwrap();
    assert!(module.contains("declare i32 @__oxid_file_seek_absolute(ptr, i64, ptr, ptr, i64)"));
    assert!(module.contains("call i32 @__oxid_file_seek_absolute("));
    let old = project("use std::fs::File;use std::fs::FileStatus;use std::fs::close;fn f(file:&mut File)->FileStatus{return close(&mut *file);}fn main()->i32{return 42;}");
    let (program, entry) = check_enum_source(SourceOwner::project(&old)).unwrap();
    assert!(!plan::ExecutionPlan::build(&program.witness)
        .unwrap()
        .has_file_seek());
    assert!(!program
        .native_module(entry, old.sources())
        .unwrap()
        .contains("__oxid_file_seek_absolute"));
}
