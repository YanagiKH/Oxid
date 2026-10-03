use super::*;
use crate::frontend::{ast, lexer, parser};

fn table() -> Declarations {
    let mut sources = SourceMap::new();
    let file = sources.add("array-layout.ox".into(), "record".into());
    let span = sources.get(file).span(0, 6);
    let records = [
        (0, vec![]),
        (1, vec![hir::Ty::I32]),
        (2, vec![hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit]),
        (3, vec![hir::Ty::I32]),
    ]
    .into_iter()
    .map(|(id, elements)| RawRecordDecl {
        id: RecordId(id),
        span,
        fields: elements
            .into_iter()
            .enumerate()
            .map(|(index, ty)| RawFieldDecl {
                id: FieldId {
                    record: RecordId(id),
                    index,
                },
                ty: ParameterTy::Value(ValueTy::Scalar(ty)),
                span,
            })
            .collect(),
    })
    .collect::<Vec<_>>();
    Declarations::check(&records, &sources).unwrap()
}

fn array(element: hir::Ty, length: usize) -> AggregateTy {
    AggregateTy::FixedArray(FixedArrayTy::check(element, length).unwrap())
}

#[test]
fn every_scalar_length_has_independently_calculated_layout_and_width() {
    let declarations = table();
    let mut cases = 0;
    for (element, stride) in [(hir::Ty::Bool, 1), (hir::Ty::I32, 4), (hir::Ty::Unit, 1)] {
        for length in 0..=1024 {
            let fixed = FixedArrayTy::check(element, length).unwrap();
            assert_eq!(fixed.element(), element);
            assert_eq!(fixed.length(), length);
            assert_eq!(fixed.stride(), stride);
            let ty = AggregateTy::FixedArray(fixed);
            let layout = declarations.aggregate_layout(ty).unwrap();
            // Independent closed form: positive N cells, otherwise one aligned sentinel.
            assert_eq!(
                layout.size(),
                if length == 0 { stride } else { length * stride }
            );
            assert_eq!(layout.align(), stride);
            assert_eq!(
                declarations.aggregate_width(ty),
                Ok(if length == 0 { 1 } else { length })
            );
            assert_eq!(
                declarations.same_aggregate_type(ty, array(element, length)),
                Ok(())
            );
            cases += 1;
        }
    }
    assert_eq!(cases, 3075);
}

#[test]
fn excess_length_rejects_before_narrowing_including_machine_maximum() {
    for element in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit] {
        assert!(FixedArrayTy::check(element, 1024).is_ok());
        for length in [1025, 65_536, usize::MAX] {
            assert_eq!(
                FixedArrayTy::check(element, length),
                Err(DeclarationError::ResourceLimit("fixed array length"))
            );
        }
    }
}

#[test]
fn structural_identity_is_element_and_length_not_layout_or_declaration() {
    let declarations = table();
    let types = [
        array(hir::Ty::Bool, 4),
        array(hir::Ty::Unit, 4),
        array(hir::Ty::I32, 1),
        AggregateTy::Record(RecordId(1)),
        AggregateTy::Record(RecordId(3)),
    ];
    for (i, actual) in types.into_iter().enumerate() {
        assert_eq!(declarations.aggregate_layout(actual).unwrap().size(), 4);
        for (j, expected) in types.into_iter().enumerate() {
            assert_eq!(
                declarations.same_aggregate_type(actual, expected),
                if i == j {
                    Ok(())
                } else {
                    Err(DeclarationError::TypeMismatch)
                }
            );
        }
    }
    for element in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit] {
        assert_eq!(
            declarations.same_aggregate_type(array(element, 0), array(element, 1)),
            Err(DeclarationError::TypeMismatch)
        );
    }
    for (left, right) in [
        (hir::Ty::Bool, hir::Ty::I32),
        (hir::Ty::Bool, hir::Ty::Unit),
        (hir::Ty::I32, hir::Ty::Unit),
    ] {
        assert_eq!(
            declarations.same_aggregate_type(array(left, 0), array(right, 0)),
            Err(DeclarationError::TypeMismatch)
        );
    }
}

#[test]
fn all_aggregate_queries_preserve_invalid_nominal_id_and_error_precedence() {
    let declarations = table();
    let valid = array(hir::Ty::Bool, 0);
    for id in [RecordId(4), RecordId(usize::MAX)] {
        let bad = AggregateTy::Record(id);
        let error = DeclarationError::InvalidRecordId(id);
        assert_eq!(declarations.check_aggregate_type(bad), Err(error));
        assert_eq!(declarations.aggregate_layout(bad), Err(error));
        assert_eq!(declarations.aggregate_width(bad), Err(error));
        assert_eq!(declarations.same_aggregate_type(bad, bad), Err(error));
        assert_eq!(declarations.same_aggregate_type(valid, bad), Err(error));
        assert_eq!(declarations.same_aggregate_type(bad, valid), Err(error));
    }
    assert_eq!(
        declarations.same_aggregate_type(
            AggregateTy::Record(RecordId(4)),
            AggregateTy::Record(RecordId(usize::MAX))
        ),
        Err(DeclarationError::InvalidRecordId(RecordId(4)))
    );
    assert_eq!(
        declarations.same_aggregate_type(
            AggregateTy::Record(RecordId(usize::MAX)),
            AggregateTy::Record(RecordId(4))
        ),
        Err(DeclarationError::InvalidRecordId(RecordId(usize::MAX)))
    );
}

#[test]
fn aggregate_record_queries_keep_nominal_width_and_padded_layout() {
    let declarations = table();
    for (id, size, alignment, width) in [(0, 1, 1, 1), (1, 4, 4, 1), (2, 12, 4, 3), (3, 4, 4, 1)] {
        let aggregate = AggregateTy::Record(RecordId(id));
        let layout = declarations.aggregate_layout(aggregate).unwrap();
        assert_eq!((layout.size(), layout.align()), (size, alignment));
        assert_eq!(declarations.aggregate_width(aggregate), Ok(width));
    }
    assert_eq!(declarations.usage().layout_bytes, 21);
}

#[test]
fn repeated_array_queries_do_not_create_declarations_or_retained_payload() {
    let declarations = table();
    let usage = declarations.usage();
    let capacities = (
        declarations.records.capacity(),
        declarations.fields.capacity(),
    );
    for _ in 0..10_000 {
        assert_eq!(
            declarations
                .aggregate_layout(array(hir::Ty::I32, 1024))
                .unwrap()
                .size(),
            4096
        );
        assert_eq!(declarations.aggregate_width(array(hir::Ty::Unit, 0)), Ok(1));
    }
    assert_eq!(declarations.usage(), usage);
    assert_eq!(
        (
            declarations.records.capacity(),
            declarations.fields.capacity()
        ),
        capacities
    );
}

#[test]
fn array_groundwork_does_not_open_source_syntax() {
    for text in [
        "fn main()->(){let a=[1];return;}",
        "fn f(a:[i32;1])->(){return;}",
        "fn f(a:&[i32;1])->(){return;}",
        "fn main()->(){let a:[i32;0]=[];return;}",
        "fn main()->i32{let a=1;return a[0];}",
        "fn main()->(){let mut a=1;a[0]=2;return;}",
        "fn main()->i32{let a=1;return a.len();}",
    ] {
        let mut sources = SourceMap::new();
        let file = sources.add("array-closed.ox".into(), text.into());
        let file = sources.get(file);
        let tokens = lexer::lex(file).unwrap();
        assert!(
            parser::parse(file, tokens).is_err(),
            "unexpected admission: {text}"
        );
    }
}

#[test]
fn array_seam_reports_representation_without_widening_existing_carriers() {
    macro_rules! sizes { ($($ty:ty),* $(,)?) => { $(println!("layout {} {}", stringify!($ty), size_of::<$ty>());)* }; }
    sizes!(
        FixedArrayTy,
        AggregateTy,
        RecordId,
        FieldId,
        ValueTy,
        ParameterTy,
        RawRecordDecl,
        RawFieldDecl,
        RecordDecl,
        FieldDecl,
        DeclarationError,
        Declarations,
        ast::Program,
        ast::Function,
        ast::BodyBlock,
        ast::ItemId,
        ast::Expr,
        ast::Stmt,
        ast::TypeSyntax
    );
    #[cfg(target_pointer_width = "64")]
    {
        assert_eq!(size_of::<FixedArrayTy>(), 4);
        assert_eq!(size_of::<AggregateTy>(), 16);
        assert_eq!(size_of::<RecordId>(), 8);
        assert_eq!(size_of::<ValueTy>(), 16);
        assert_eq!(size_of::<ParameterTy>(), 24);
        assert_eq!(size_of::<RecordDecl>(), 64);
        assert_eq!(size_of::<FieldDecl>(), 56);
        assert_eq!(
            size_of::<ast::Function>() + size_of::<ast::BodyBlock>() + size_of::<ast::ItemId>(),
            288
        );
    }
}
