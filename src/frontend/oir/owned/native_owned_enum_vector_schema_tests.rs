use super::*;
use owned_record_codec::bounds::CheckedElements;

fn schema_declarations() -> Declarations {
    let mut sources = SourceMap::new();
    let file = sources.add("native-enum-vector.ox".into(), "enum schema".into());
    let span = sources.get(file).span(0, 4);
    let record = RawRecordDecl {
        id: RecordId(0),
        is_copy: false,
        span,
        fields: (0..2)
            .map(|index| RawFieldDecl {
                id: FieldId {
                    record: RecordId(0),
                    index,
                },
                ty: ParameterTy::Value(ValueTy::Scalar(hir::Ty::I128)),
                span,
            })
            .collect(),
    };
    let enumerations = [
        vec![
            None,
            Some(ValueTy::Owned(AggregateTy::ByteBuf)),
            Some(ValueTy::Scalar(hir::Ty::I128)),
        ],
        vec![Some(ValueTy::Owned(AggregateTy::OwnedEnumVector(EnumId(
            0,
        ))))],
    ]
    .into_iter()
    .enumerate()
    .map(|(enumeration, payloads)| RawEnumDecl {
        id: EnumId(enumeration),
        span,
        variants: payloads
            .into_iter()
            .enumerate()
            .map(|(index, payload)| RawVariantDecl {
                id: VariantId {
                    enumeration: EnumId(enumeration),
                    index,
                },
                payload: payload.map(ParameterTy::Value),
                span,
            })
            .collect(),
    })
    .collect::<Vec<_>>();
    Declarations::check_combined_owned_vectors(&[record], &enumerations, &sources).unwrap()
}

#[test]
fn native_owned_enum_vector_schemas_authenticate_distinct_roots_offsets_and_ineligible_sentinels() {
    let declarations = schema_declarations();
    let bounds = CheckedElements::check(&declarations, plan::MAX_PLAN_BYTES).unwrap();
    let view = bounds.view();
    let element = view.element_root(OwnedVectorRoot::Enum(EnumId(0))).unwrap();
    assert_eq!(
        (
            element.size(),
            element.identity_capacity(),
            element.traversal_work(1)
        ),
        (32, 1, Some(3))
    );
    assert!(view.element_root(OwnedVectorRoot::Enum(EnumId(1))).is_err());
    let mut counted = Emission::count(MAX_IR_BYTES);
    owned_vector_schemas::schemas(&declarations, &view, &mut counted).unwrap();
    assert!(!counted.exceeded);
    let mut rendered = Emission::count(counted.len);
    rendered.text = Some(String::with_capacity(counted.len));
    owned_vector_schemas::schemas(&declarations, &view, &mut rendered).unwrap();
    assert!(!rendered.exceeded);
    assert_eq!(
        (rendered.len, rendered.ordinary_visits),
        (counted.len, counted.ordinary_visits)
    );
    let text = rendered.text.unwrap();
    assert_eq!(text.len(), counted.len);
    assert!(text
        .lines()
        .any(|line| line == "@__oxid_owned_enum_vectors = constant i1 true"));
    assert!(text.contains("@__oxid_owned_enum_count = constant i32 2, align 4"));
    assert!(text.contains("@__oxid_owned_records = constant [1 x"));
    assert!(text.contains("@__oxid_owned_enums = constant [2 x"));
    assert!(text.contains("i32 0, i32 0, i64 32, i64 5, i64 0"));
    assert!(text.contains("i32 2147483648, i32 1, i64 32, i64 3, i64 1"));
    assert!(text.contains("i32 2147483649, i32 4294967295, i64 0, i64 0, i64 0"));
    assert!(text.contains("i32 4294967295, i32 0"));
    assert!(text.contains("i32 16 }"));
    let mut short = Emission::count(text.len() - 1);
    owned_vector_schemas::schemas(&declarations, &view, &mut short).unwrap();
    assert!(short.exceeded && short.text.is_none());
    let foreign = schema_declarations();
    let mut output = Emission::count(MAX_IR_BYTES);
    assert!(owned_vector_schemas::schemas(&foreign, &view, &mut output).is_err());
    assert_eq!((output.len, output.ordinary_visits), (0, 0));
    if let Some(path) = std::env::var_os("OXID_TEST_OWNED_ENUM_SCHEMA_OUTPUT") {
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();
        std::io::Write::write_all(&mut output, text.as_bytes()).unwrap();
    }
}
