use super::*;
use crate::frontend::builtin_catalog::experimental::{Enumeration, Function};

#[test]
fn experimental_catalog_import_identity_and_legacy_separation() {
    let source = "use std::io::ReadStatus; use std::io::WriteStatus; use std::io::read_stdin; use std::io::write_stdout; use alloc::bytes::ByteBuf; use alloc::bytes::GrowStatus; use alloc::bytes::new; use alloc::bytes::len; use alloc::bytes::capacity; use alloc::bytes::try_reserve; use alloc::bytes::try_push; use alloc::bytes::try_extend; use alloc::bytes::try_resize; use alloc::bytes::truncate; use std::io::ReadSomeStatus; use std::io::WriteSomeStatus; use std::io::read_some; use std::io::write_some; use alloc::bytes::take; use std::fs::File; use std::fs::OpenResult; use std::fs::FileStatus; use std::fs::open_read; use std::fs::create; use std::fs::read_some as file_read; use std::fs::write_some as file_write; use std::fs::sync_all; use std::fs::close; fn main()->(){return;}";
    let fixture = Fixture::new(&[("main.ox", source)]);
    let project = fixture.load_current();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let index = collect_originals(
        SourceOwner::project(&project),
        IndexLimits::default(),
        &work,
        &mut allocator,
    )
    .unwrap()
    .finish(&work, &mut allocator)
    .unwrap();
    assert_eq!((index.enum_count(), index.function_count()), (7, 20));
    assert_eq!(
        index.builtin_enum_id(BuiltinEnum::ReadStatus).unwrap(),
        EnumId(0)
    );
    assert_eq!(
        index.builtin_enum_id(BuiltinEnum::WriteStatus).unwrap(),
        EnumId(1)
    );
    assert_eq!(
        index
            .builtin_function_id(BuiltinFunction::ReadStdin)
            .unwrap(),
        DefId(1)
    );
    assert_eq!(
        index
            .builtin_function_id(BuiltinFunction::WriteStdout)
            .unwrap(),
        DefId(2)
    );
    for (ordinal, item) in Enumeration::ALL.into_iter().enumerate() {
        let id = index.experimental_enum_id(item).unwrap();
        assert_eq!(id, EnumId(2 + ordinal));
        let view = index.enum_view(id).unwrap();
        assert_eq!(view.name(), item.item().name());
        assert_eq!(view.variant_count(), item.members().len());
        for member in 0..view.variant_count() {
            let variant = view
                .variant(VariantId {
                    enumeration: id,
                    index: member,
                })
                .unwrap();
            assert_eq!(variant.name(), item.members()[member]);
            assert_eq!(
                variant.value_payload(&work).unwrap(),
                item.value_payload(member)
            );
            if !variant.has_owned_payload() {
                assert_eq!(variant.payload(), item.payload(member));
            }
        }
    }
    for (ordinal, item) in Function::ALL.into_iter().enumerate() {
        assert_eq!(
            index.experimental_function_id(item).unwrap(),
            DefId(3 + ordinal)
        );
        let signature = index.experimental_signature(item).unwrap();
        assert_eq!(
            signature.parameters(),
            item.signature(|_| EnumId(0)).parameters()
        );
    }
    assert_eq!(
        index
            .query(&work)
            .nominal_type(ModuleId(0), bound(&project, 0, 4), TypeContext::Value)
            .unwrap(),
        NominalId::ByteBuf
    );
}
