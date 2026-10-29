//! Stream immutable C-runtime type facts through the ordinary bounded LLVM
//! count/render sink. Producing globals grants no executable witness.
use super::*;
use crate::frontend::oir::owned::owned_record_codec::bounds::CheckedElementView;

#[derive(Clone, Copy)]
struct Counts {
    records: usize,
    enums: usize,
    fields: usize,
    variants: usize,
    nominal: usize,
    types: usize,
}

#[allow(dead_code)]
struct SchemaCarriers {
    declarations: [&'static Declarations; 3],
    bounds: [&'static CheckedElementView<'static>; 2],
    views: [CheckedElementView<'static>; 3],
    outputs: [&'static mut Emission; 2],
    counts: [Counts; 3],
    count_results: [Result<Counts, Box<Diagnostic>>; 2],
    type_rows: [TypeRow; 3],
    type_results: [Result<TypeRow, Box<Diagnostic>>; 2],
    values: [ValueTy; 3],
    identifiers: [Result<u32, Box<Diagnostic>>; 3],
    indices: [usize; 16],
    products: [Option<usize>; 4],
    format: [FormatCallsiteBacking<8>; 2],
    writes: [std::fmt::Result; 4],
    elements:
        [Result<owned_record_codec::Element<'static>, owned_record_codec::bounds::ShapeFailure>; 2],
    record_rows: [std::slice::Iter<'static, RecordDecl>; 2],
    enum_rows: [std::slice::Iter<'static, crate::frontend::oir::owned_types::EnumDecl>; 2],
    field_rows: [std::slice::Iter<'static, crate::frontend::oir::owned_types::FieldDecl>; 2],
    variant_rows: [std::slice::Iter<'static, crate::frontend::oir::owned_types::VariantDecl>; 2],
}
pub(super) const fn control_bytes() -> usize {
    size_of::<SchemaCarriers>()
}
impl Counts {
    fn check(declarations: &Declarations) -> Result<Self, Box<Diagnostic>> {
        let records = declarations.records().len();
        let enums = declarations.enums().enums().len();
        let fields = declarations.usage().fields;
        let variants = declarations.enums().usage().variants;
        let nominal = add(records, enums)?;
        let types = add(add(nominal, fields)?, variants)?;
        u32::try_from(types)
            .map_err(|_| reject("owned vector native schema count overflow", None))?;
        Ok(Self {
            records,
            enums,
            fields,
            variants,
            nominal,
            types,
        })
    }
    fn type_id(self, value: ValueTy, leaf: usize) -> Result<u32, Box<Diagnostic>> {
        let id = match value {
            ValueTy::Owned(AggregateTy::Record(id)) => id.0,
            ValueTy::Owned(AggregateTy::Enum(id)) => add(self.records, id.0)?,
            _ => add(self.nominal, leaf)?,
        };
        u32::try_from(id).map_err(|_| reject("owned vector native type ordinal overflow", None))
    }
}
fn declaration_error(_: impl std::fmt::Debug) -> Box<Diagnostic> {
    reject(
        "owned vector native schema requires checked declarations",
        None,
    )
}
type TypeRow = (u32, u32, u32, u32, u64);
fn leaf_row(declarations: &Declarations, value: ValueTy) -> Result<TypeRow, Box<Diagnostic>> {
    let (kind, detail, count, bytes) = match value {
        ValueTy::Scalar(ty) => (
            1,
            vector_type::element(ty).tag(),
            0,
            Layout::scalar(ty).size(),
        ),
        ValueTy::Owned(AggregateTy::ByteBuf) => (4, 0, 0, 16),
        ValueTy::Owned(AggregateTy::File) => (5, 0, 0, 16),
        ValueTy::Owned(AggregateTy::Vector(ty)) => (6, vector_type::element(ty).tag(), 0, 16),
        ValueTy::Owned(AggregateTy::RecordVector(record)) => (
            6,
            0x8000_0000 | u32::try_from(record.0).map_err(declaration_error)?,
            0,
            16,
        ),
        ValueTy::Owned(aggregate @ AggregateTy::FixedArray(array)) => (
            7,
            vector_type::element(array.element()).tag(),
            u32::try_from(array.length()).map_err(declaration_error)?,
            declarations
                .aggregate_layout(aggregate)
                .map_err(declaration_error)?
                .size(),
        ),
        // Nominal values reference the earlier shared type row. Ineligible
        // nested owned-vector declarations never acquire a resource leaf.
        ValueTy::Owned(
            AggregateTy::Record(_) | AggregateTy::Enum(_) | AggregateTy::OwnedRecordVector(_),
        ) => (0, 0, 0, 0),
    };
    Ok((kind, detail, 0, count, bytes as u64))
}
fn type_row(out: &mut Emission, separator: &mut &'static str, row: TypeRow) {
    if out.exceeded {
        return;
    }
    let (kind, detail, first, count, bytes) = row;
    out.ordinary_visits += 1;
    writeln!(out, "{separator}{{ i32, i32, i32, i32, i64 }} {{ i32 {kind}, i32 {detail}, i32 {first}, i32 {count}, i64 {bytes} }}").unwrap();
    *separator = ", ";
}
fn member_row(
    out: &mut Emission,
    separator: &mut &'static str,
    ty: u32,
    offset: usize,
) -> Result<(), Box<Diagnostic>> {
    if out.exceeded {
        return Ok(());
    }
    let offset = u32::try_from(offset).map_err(declaration_error)?;
    out.ordinary_visits += 1;
    writeln!(
        out,
        "{separator}{{ i32, i32 }} {{ i32 {ty}, i32 {offset} }}"
    )
    .unwrap();
    *separator = ", ";
    Ok(())
}

pub(super) fn schemas(
    declarations: &Declarations,
    bounds: &CheckedElementView<'_>,
    out: &mut Emission,
) -> Result<(), Box<Diagnostic>> {
    if !bounds.belongs_to(declarations) {
        return Err(reject(
            "owned vector native schema belongs to a different declaration facade",
            None,
        ));
    }
    let counts = Counts::check(declarations)?;
    for (name, count) in [
        ("type", counts.types),
        ("field", counts.fields),
        ("variant", counts.variants),
        ("record", counts.records),
    ] {
        if out.exceeded {
            return Ok(());
        }
        out.ordinary_visits += 1;
        writeln!(
            out,
            "@__oxid_owned_{name}_count = constant i32 {count}, align 4"
        )
        .unwrap();
    }
    writeln!(
        out,
        "@__oxid_owned_types = constant [{} x {{ i32, i32, i32, i32, i64 }}] [",
        counts.types
    )
    .unwrap();
    let mut separator = "";
    for record in declarations.records() {
        if out.exceeded {
            return Ok(());
        }
        out.ordinary_visits += 1;
        let fields = declarations
            .fields(record.id())
            .map_err(declaration_error)?;
        type_row(
            out,
            &mut separator,
            (
                2,
                0,
                u32::try_from(record.field_start()).map_err(declaration_error)?,
                u32::try_from(fields.len()).map_err(declaration_error)?,
                record.layout().size() as u64,
            ),
        );
    }
    let mut first = 0usize;
    for enumeration in declarations.enums().enums() {
        if out.exceeded {
            return Ok(());
        }
        out.ordinary_visits += 1;
        let variants = declarations
            .enums()
            .variants(enumeration.id())
            .map_err(declaration_error)?;
        type_row(
            out,
            &mut separator,
            (
                3,
                0,
                u32::try_from(first).map_err(declaration_error)?,
                u32::try_from(variants.len()).map_err(declaration_error)?,
                enumeration.layout().size() as u64,
            ),
        );
        first = add(first, variants.len())?;
    }
    for record in declarations.records() {
        if out.exceeded {
            return Ok(());
        }
        out.ordinary_visits += 1;
        for field in declarations
            .fields(record.id())
            .map_err(declaration_error)?
        {
            if out.exceeded {
                return Ok(());
            }
            type_row(
                out,
                &mut separator,
                leaf_row(declarations, field.value_ty())?,
            );
        }
    }
    for enumeration in declarations.enums().enums() {
        if out.exceeded {
            return Ok(());
        }
        out.ordinary_visits += 1;
        for variant in declarations
            .enums()
            .variants(enumeration.id())
            .map_err(declaration_error)?
        {
            if out.exceeded {
                return Ok(());
            }
            type_row(
                out,
                &mut separator,
                match variant.value_payload() {
                    Some(value) => leaf_row(declarations, value)?,
                    None => (0, 0, 0, 0, 0),
                },
            );
        }
    }
    writeln!(out, "], align 8").unwrap();
    writeln!(
        out,
        "@__oxid_owned_fields = constant [{} x {{ i32, i32 }}] [",
        counts.fields
    )
    .unwrap();
    separator = "";
    let mut ordinal = 0usize;
    for record in declarations.records() {
        if out.exceeded {
            return Ok(());
        }
        out.ordinary_visits += 1;
        for field in declarations
            .fields(record.id())
            .map_err(declaration_error)?
        {
            if out.exceeded {
                return Ok(());
            }
            member_row(
                out,
                &mut separator,
                counts.type_id(field.value_ty(), ordinal)?,
                field.offset(),
            )?;
            ordinal = add(ordinal, 1)?;
        }
    }
    writeln!(out, "], align 4").unwrap();
    writeln!(
        out,
        "@__oxid_owned_variants = constant [{} x {{ i32, i32 }}] [",
        counts.variants
    )
    .unwrap();
    separator = "";
    ordinal = 0;
    for enumeration in declarations.enums().enums() {
        if out.exceeded {
            return Ok(());
        }
        out.ordinary_visits += 1;
        for variant in declarations
            .enums()
            .variants(enumeration.id())
            .map_err(declaration_error)?
        {
            if out.exceeded {
                return Ok(());
            }
            let (ty, offset) = match variant.value_payload() {
                Some(value) => (
                    counts.type_id(value, add(counts.fields, ordinal)?)?,
                    variant
                        .payload_offset()
                        .ok_or_else(|| declaration_error("payload offset"))?,
                ),
                None => (u32::MAX, 0),
            };
            member_row(out, &mut separator, ty, offset)?;
            ordinal = add(ordinal, 1)?;
        }
    }
    writeln!(out, "], align 4").unwrap();
    writeln!(
        out,
        "@__oxid_owned_records = constant [{} x {{ i32, i32, i64, i64, i64 }}] [",
        counts.records
    )
    .unwrap();
    separator = "";
    for record in declarations.records() {
        if out.exceeded {
            return Ok(());
        }
        out.ordinary_visits += 1;
        let id = u32::try_from(record.id().0).map_err(declaration_error)?;
        let (root, stride, visits, resources) = match bounds.element(record.id()) {
            Ok(element) => (
                id,
                element.size(),
                element
                    .traversal_work(1)
                    .ok_or_else(|| declaration_error("visits"))?,
                element.identity_capacity(),
            ),
            Err(_) => (u32::MAX, 0, 0, 0),
        };
        writeln!(out, "{separator}{{ i32, i32, i64, i64, i64 }} {{ i32 {id}, i32 {root}, i64 {stride}, i64 {visits}, i64 {resources} }}").unwrap();
        separator = ", ";
    }
    writeln!(out, "], align 8").unwrap();
    Ok(())
}
