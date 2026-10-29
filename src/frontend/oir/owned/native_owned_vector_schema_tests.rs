//! Actual compiler-produced globals replace the C fixture's owned-type table.
use super::*;
use owned_record_codec::bounds::CheckedElements;

fn declarations() -> Declarations {
    let mut sources = SourceMap::new();
    let file = sources.add("native-owned-schemas.ox".into(), "record schemas".into());
    let span = sources.get(file).span(0, 6);
    let records = (0..3)
        .map(|record| {
            let fields = if record < 2 {
                vec![
                    ValueTy::Scalar(hir::Ty::Bool),
                    ValueTy::Owned(AggregateTy::ByteBuf),
                    ValueTy::Scalar(hir::Ty::I32),
                ]
            } else {
                vec![ValueTy::Scalar(hir::Ty::I32)]
            };
            RawRecordDecl {
                is_copy: record == 2,
                id: RecordId(record),
                span,
                fields: fields
                    .into_iter()
                    .enumerate()
                    .map(|(index, value)| RawFieldDecl {
                        id: FieldId {
                            record: RecordId(record),
                            index,
                        },
                        ty: ParameterTy::Value(value),
                        span,
                    })
                    .collect(),
            }
        })
        .collect::<Vec<_>>();
    Declarations::check(&records, &sources).unwrap()
}
fn render(declarations: &Declarations, bounds: &CheckedElements<'_>) -> String {
    let bounds = bounds.view();
    let mut counted = Emission::count(MAX_IR_BYTES);
    owned_vector_schemas::schemas(declarations, &bounds, &mut counted).unwrap();
    assert!(!counted.exceeded);
    let mut rendered = Emission::count(counted.len);
    rendered.text = Some(String::with_capacity(counted.len));
    owned_vector_schemas::schemas(declarations, &bounds, &mut rendered).unwrap();
    assert!(!rendered.exceeded);
    assert_eq!(rendered.len, counted.len);
    assert_eq!(rendered.ordinary_visits, counted.ordinary_visits);
    let text = rendered.text.unwrap();
    assert_eq!(text.len(), counted.len);
    text
}
#[test]
fn owned_vector_schema_nominals_layouts_and_copy_sentinels_come_from_real_facade() {
    let declarations = declarations();
    let bounds = CheckedElements::check(&declarations, plan::MAX_PLAN_BYTES).unwrap();
    let text = render(&declarations, &bounds);
    assert!(text.contains("@__oxid_owned_type_count = constant i32 10"));
    assert!(text.contains("@__oxid_owned_field_count = constant i32 7"));
    assert!(text.contains("@__oxid_owned_record_count = constant i32 3"));
    assert!(text.contains("i32 0, i32 0, i64 32, i64 7, i64 1"));
    assert!(text.contains("i32 1, i32 1, i64 32, i64 7, i64 1"));
    assert!(text.contains("i32 2, i32 4294967295, i64 0, i64 0, i64 0"));
    assert!(text.contains("{ i32 4, i32 8 }"));
    assert!(text.contains("{ i32 5, i32 24 }"));
    assert!(owned_vector_schemas::control_bytes() < EMITTER_TRANSIENT_BYTES);
    if let Some(path) = std::env::var_os("OXID_TEST_OWNED_SCHEMA_OUTPUT") {
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();
        std::io::Write::write_all(&mut output, text.as_bytes()).unwrap();
    }
}
#[test]
fn owned_vector_schema_rejects_foreign_bounds_before_output_and_stops_at_ir_limit() {
    let first = declarations();
    let second = declarations();
    let bounds = CheckedElements::check(&first, plan::MAX_PLAN_BYTES).unwrap();
    let mut out = Emission::count(MAX_IR_BYTES);
    assert!(owned_vector_schemas::schemas(&second, &bounds.view(), &mut out).is_err());
    assert_eq!((out.len, out.ordinary_visits), (0, 0));
    let text = render(&first, &bounds);
    let mut short = Emission::count(text.len() - 1);
    owned_vector_schemas::schemas(&first, &bounds.view(), &mut short).unwrap();
    assert!(short.exceeded && short.len < text.len());
    assert!(short.text.is_none());
}

#[test]
fn owned_vector_schema_active_enums_files_arrays_and_resource_classes_use_real_layouts() {
    let mut sources = SourceMap::new();
    let file = sources.add("native-owned-composed.ox".into(), "record schemas".into());
    let span = sources.get(file).span(0, 6);
    let fields = [
        vec![
            ValueTy::Scalar(hir::Ty::Bool),
            ValueTy::Scalar(hir::Ty::Char),
            ValueTy::Owned(AggregateTy::Enum(EnumId(0))),
            ValueTy::Owned(AggregateTy::File),
            ValueTy::Owned(AggregateTy::Vector(hir::Ty::U32)),
            ValueTy::Owned(AggregateTy::RecordVector(RecordId(2))),
            ValueTy::Owned(AggregateTy::Record(RecordId(3))),
            ValueTy::Owned(AggregateTy::FixedArray(
                FixedArrayTy::check(hir::Ty::U16, 0).unwrap(),
            )),
        ],
        vec![ValueTy::Owned(AggregateTy::ByteBuf); 2],
        vec![ValueTy::Scalar(hir::Ty::U32)],
        vec![],
    ];
    let records = fields
        .into_iter()
        .enumerate()
        .map(|(record, fields)| RawRecordDecl {
            is_copy: record >= 2,
            id: RecordId(record),
            span,
            fields: fields
                .into_iter()
                .enumerate()
                .map(|(index, value)| RawFieldDecl {
                    id: FieldId {
                        record: RecordId(record),
                        index,
                    },
                    ty: ParameterTy::Value(value),
                    span,
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    let enums = [RawEnumDecl {
        id: EnumId(0),
        span,
        variants: [
            None,
            Some(ValueTy::Owned(AggregateTy::ByteBuf)),
            Some(ValueTy::Scalar(hir::Ty::I128)),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, value)| RawVariantDecl {
            id: VariantId {
                enumeration: EnumId(0),
                index,
            },
            payload: value.map(ParameterTy::Value),
            span,
        })
        .collect(),
    }];
    let declarations = Declarations::check_combined(&records, &enums, &sources).unwrap();
    let bounds = CheckedElements::check(&declarations, plan::MAX_PLAN_BYTES).unwrap();
    let element = bounds.element(RecordId(0)).unwrap();
    assert_eq!(element.size(), 112);
    assert_eq!(element.identity_capacity(), 4);
    let text = render(&declarations, &bounds);
    assert!(text.contains("@__oxid_owned_variant_count = constant i32 3"));
    assert!(text.contains("i32 4294967295, i32 0"));
    assert!(text.contains("i32 6, i32 2147483650"));
    if let Some(path) = std::env::var_os("OXID_TEST_COMPOSED_OWNED_SCHEMA_OUTPUT") {
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();
        std::io::Write::write_all(&mut output, text.as_bytes()).unwrap();
    }
}

#[test]
fn owned_vector_schema_unrelated_owned_vector_paths_keep_supported_elements_and_invalid_sentinels()
{
    let mut sources = SourceMap::new();
    let file = sources.add("native-owned-unrelated.ox".into(), "record schemas".into());
    let span = sources.get(file).span(0, 6);
    let vector = ValueTy::Owned(AggregateTy::OwnedRecordVector(RecordId(0)));
    let fields = [
        vec![
            ValueTy::Scalar(hir::Ty::Bool),
            ValueTy::Owned(AggregateTy::ByteBuf),
            ValueTy::Scalar(hir::Ty::I32),
        ],
        vec![vector],
        vec![ValueTy::Scalar(hir::Ty::U32)],
        vec![ValueTy::Owned(AggregateTy::Enum(EnumId(0)))],
    ];
    let records = fields
        .into_iter()
        .enumerate()
        .map(|(record, fields)| RawRecordDecl {
            is_copy: record == 2,
            id: RecordId(record),
            span,
            fields: fields
                .into_iter()
                .enumerate()
                .map(|(index, value)| RawFieldDecl {
                    id: FieldId {
                        record: RecordId(record),
                        index,
                    },
                    ty: ParameterTy::Value(value),
                    span,
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    let enums = [RawEnumDecl {
        id: EnumId(0),
        span,
        variants: [None, Some(vector)]
            .into_iter()
            .enumerate()
            .map(|(index, value)| RawVariantDecl {
                id: VariantId {
                    enumeration: EnumId(0),
                    index,
                },
                payload: value.map(ParameterTy::Value),
                span,
            })
            .collect(),
    }];
    assert!(Declarations::check_combined(&records, &enums, &sources).is_err());
    let declarations =
        Declarations::check_combined_owned_vectors(&records, &enums, &sources).unwrap();
    let bounds = CheckedElements::check(&declarations, plan::MAX_PLAN_BYTES).unwrap();
    assert_eq!(bounds.element(RecordId(0)).unwrap().size(), 32);
    for record in [1, 2, 3] {
        assert!(bounds.element(RecordId(record)).is_err());
    }
    let text = render(&declarations, &bounds);
    assert!(text.contains("@__oxid_owned_record_count = constant i32 4"));
    assert!(text.contains("i32 0, i32 0, i64 32, i64 7, i64 1"));
    for record in [1, 2, 3] {
        assert!(text.contains(&format!(
            "i32 {record}, i32 4294967295, i64 0, i64 0, i64 0"
        )));
    }
    if let Some(path) = std::env::var_os("OXID_TEST_UNRELATED_OWNED_SCHEMA_OUTPUT") {
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();
        std::io::Write::write_all(&mut output, text.as_bytes()).unwrap();
    }
}
