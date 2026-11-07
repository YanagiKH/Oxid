//! Independent reference execution oracles for typed vectors and slices.
use super::super::super::consumer_fixtures as fixture;
use super::*;
use crate::frontend::oir::owned::source::resource_fixtures::checked_enum_project;

#[test]
fn vector_reference_range_words_are_elements_above_the_fixed_array_limit() {
    let (_, at) = fixture::context();
    let span = at(0);
    assert_eq!(
        packed_range(4096, 8192, span).unwrap(),
        0x0000_1000_0000_1000
    );
    assert_eq!(
        unpacked_range(0x0000_1000_0000_1000, span).unwrap(),
        (4096, 8192)
    );
    assert_eq!(
        selected_range(4096, 8192, Some((3, 7)), span).unwrap(),
        (4099, 4103)
    );
    assert_eq!(
        packed_range(4099, 4103, span).unwrap(),
        0x0000_1003_0000_0004
    );
    assert_eq!(
        selected_range(4096, 8192, Some((4096, 4096)), span).unwrap(),
        (8192, 8192)
    );
    for range in [(-1, 0), (7, 3), (0, 4097)] {
        assert!(matches!(
            selected_range(4096, 8192, Some(range), span),
            Err(OwnedRunFailure::Bounds(_))
        ));
    }
    assert!(matches!(
        unpacked_range(u64::MAX, span),
        Err(OwnedRunFailure::Bounds(_))
    ));
    assert!(matches!(
        unpacked_range(0x7fff_ffff_0000_0001, span),
        Err(OwnedRunFailure::Bounds(_))
    ));
}

#[test]
fn vector_reference_all_scalar_scratch_uses_actual_stored_value_codecs() {
    let (_, at) = fixture::context();
    let span = at(0);
    let cases: &[(hir::Ty, usize, &[u8])] = &[
        (hir::Ty::Bool, 1, &[1]),
        (hir::Ty::Unit, 1, &[0]),
        (hir::Ty::I8, 1, &[0x80]),
        (hir::Ty::U8, 1, &[0xff]),
        (hir::Ty::I16, 2, &[0, 0x80]),
        (hir::Ty::U16, 2, &[0x34, 0xab]),
        (hir::Ty::I32, 4, &[0, 0, 0, 0x80]),
        (hir::Ty::U32, 4, &[0xff; 4]),
        (hir::Ty::I64, 8, &[0, 0, 0, 0, 0, 0, 0, 0x80]),
        (hir::Ty::U64, 8, &[0xff; 8]),
        (hir::Ty::Isize, 8, &[0, 0, 0, 0, 0, 0, 0, 0x80]),
        (hir::Ty::Usize, 8, &[0xff; 8]),
        (hir::Ty::F32, 4, &[0x45, 0x23, 0xc1, 0x7f]),
        (hir::Ty::F64, 8, &[0x42, 0, 0, 0, 0, 0, 0xf8, 0x7f]),
        (hir::Ty::Char, 4, &[0xff, 0xff, 0x10, 0]),
        (
            hir::Ty::I128,
            16,
            &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x80],
        ),
        (hir::Ty::U128, 16, &[0xff; 16]),
    ];
    for &(ty, width, bytes) in cases {
        let value = <RuntimeValue as StoredValue>::decode(bytes, 0, ty, span).unwrap();
        assert_eq!(value.ty(), ty);
        let mut scratch = [0xa5; 16];
        value.encode(&mut scratch, 0, span).unwrap();
        assert_eq!(&scratch[..width], bytes);
        assert!(scratch[width..].iter().all(|byte| *byte == 0xa5));
    }
    for (ty, bytes) in [
        (hir::Ty::Bool, &[2][..]),
        (hir::Ty::Unit, &[1][..]),
        (hir::Ty::Char, &[0, 0xd8, 0, 0][..]),
        (hir::Ty::Char, &[0, 0, 0x11, 0][..]),
    ] {
        assert!(<RuntimeValue as StoredValue>::decode(bytes, 0, ty, span).is_err());
    }
}

#[test]
fn vector_reference_float_scratch_preserves_all_supplied_bits() {
    let (_, at) = fixture::context();
    let span = at(0);
    for bits in [
        0u64,
        0x8000_0000_0000_0000,
        0x7ff0_0000_0000_0000,
        0xfff0_0000_0000_0000,
        0x7ff8_0000_0000_0042,
        0x7ff0_0000_0000_0001,
    ] {
        let bytes = bits.to_le_bytes();
        let value = <RuntimeValue as StoredValue>::decode(&bytes, 0, hir::Ty::F64, span).unwrap();
        let mut scratch = [0xa5; 16];
        value.encode(&mut scratch, 0, span).unwrap();
        assert_eq!(&scratch[..8], &bytes);
        assert_eq!(&scratch[8..], &[0xa5; 8]);
    }
    for bits in [
        0u32,
        0x8000_0000,
        0x7f80_0000,
        0xff80_0000,
        0x7fc0_0042,
        0x7f80_0001,
    ] {
        let bytes = bits.to_le_bytes();
        let value = <RuntimeValue as StoredValue>::decode(&bytes, 0, hir::Ty::F32, span).unwrap();
        let mut scratch = [0xa5; 16];
        value.encode(&mut scratch, 0, span).unwrap();
        assert_eq!(&scratch[..4], &bytes);
        assert_eq!(&scratch[4..], &[0xa5; 12]);
    }
}

#[test]
fn vector_reference_real_source_all_nine_ops_projected_record_and_forwarded_slice() {
    let checked = checked_enum_project(
        r#"
use alloc::vec::Vec;use alloc::vec::new;use alloc::vec::len;use alloc::vec::capacity;
use alloc::vec::try_reserve;use alloc::vec::try_push;use alloc::vec::try_extend;
use alloc::vec::try_resize;use alloc::vec::truncate;use alloc::vec::take;
struct Holder{prefix:i32,values:Vec<i32>}
fn score(p:&[i32])->i32{let a=p[0];let b=p[1];let c=p[2];return a+b+c+p.len();}
fn forward(p:&[i32])->i32{return score(&*p[0..3]);}
fn main()->i32{
let mut v=new::<i32>();try_push(&mut v,7);let extra=[11,13];try_extend(&mut v,&extra);
try_resize(&mut v,5,17);v[2]=19;
let mut h=Holder{prefix:99,values:take(&mut v)};
truncate(&mut h.values,4);try_reserve(&mut h.values,20);
return h.values[0]+forward(&h.values[1..4])+len(&h.values)+capacity(&h.values)+len(&v)+capacity(&v);
}"#,
    );
    // [7,11,19,17], capacity24; score=11+19+17+3=50; empty moved source.
    assert_eq!(
        super::super::run(&checked.witness, Some(checked.entry)).unwrap(),
        Scalar::I32(85)
    );
}

#[test]
fn vector_reference_real_source_range_beyond_1024_retains_dynamic_storage() {
    let checked = checked_enum_project(
        r#"
use alloc::vec::new;use alloc::vec::try_resize;
fn tail(p:&[i32])->i32{return p[0]+p.len();}
fn forward(p:&[i32])->i32{return tail(&*p[2048..2049]);}
fn main()->i32{let mut v=new::<i32>();try_resize(&mut v,2049,31);return forward(&v);}
"#,
    );
    assert_eq!(
        super::super::run(&checked.witness, Some(checked.entry)).unwrap(),
        Scalar::I32(32)
    );
}

#[test]
fn scalar_reference_fixed_array_ranges_are_typed_without_vector_imports() {
    let checked = checked_enum_project(
        r#"
struct R{values:[i32;4]}
fn score(p:&[i32])->i32{let a=p[0];let b=p[1];return a+b+p.len();}
fn projected(r:&R)->i32{return score(&*r.values[1..3]);}
fn forward(p:&[i32])->i32{return score(&*p[1..3]);}
fn edit(p:&mut[i32])->(){p[0]=77;return;}
fn main()->i32{let mut a=[10,20,30,40];edit(&mut a[2..3]);let r=R{values:[1,2,3,4]};
return score(&a[1..3])+forward(&a)+projected(&r);}
"#,
    );
    // Two [20,77] views score99 each; projected [2,3] scores7.
    assert_eq!(
        super::super::run(&checked.witness, Some(checked.entry)).unwrap(),
        Scalar::I32(205)
    );
}
