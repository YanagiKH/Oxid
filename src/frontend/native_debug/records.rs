//! Flat numeric Copy records, with types and offsets from the checked witness.
use super::*;
use crate::frontend::oir::{DebugField, DebugRecord};
use std::mem::size_of;

// Named construction/return/caller roles for the new allocation-free bridge,
// type walk and variable-marker branch. This is a conservative payload model,
// not an assumption about the compiler's generated machine stack. Formatting
// is streamed into Output; it never creates a String or member-ID collection.
#[allow(dead_code)]
struct Formatting<const N: usize> {
    captures: [usize; N],
    argument_returns: [[usize; 2]; N],
    arguments: [[usize; 2]; N],
    template_borrow: usize,
    argument_array_borrow: usize,
    forwarded_arguments: fmt::Arguments<'static>,
}

#[allow(dead_code)]
struct RecordMetadataCarriers {
    // Include construction, return and caller slots across add/render/type
    // rendering/support lookup/facade queries, even when moves can coalesce.
    records: [Option<DebugRecords<'static>>; 8],
    record: [Option<DebugRecord<'static>>; 6],
    fields: [Option<DebugField>; 4],
    project: [&'static ProjectSources; 6],
    ast: [Option<&'static ast::Program>; 2],
    source_record: [Option<&'static ast::StructDecl>; 2],
    declaration_and_field_spans: [Span; 4],
    output: [&'static mut Output; 3],
    ids_offsets_and_counts: [usize; 24],
    range_iterators: [std::ops::Range<usize>; 3],
    scalar_type: [Option<usize>; 3],
    variable_kind: Option<(bool, &'static str)>,
    variable_type: Option<(usize, Option<&'static str>)>,
    marker_words: std::str::Split<'static, char>,
    marker_word: Option<&'static str>,
    results: [fmt::Result; 7],
    // Separate callsite construction and forwarding for composite/member
    // headers, their numeric suffixes, and the member-ID list writes.
    formatting: [Formatting<12>; 6],
}

/// Exact generated full-line grammar. In particular, embedded text in an LLVM
/// string/comment, a suffix, a tab or a second space cannot become a binding.
pub(super) fn valid_marker(text: &str) -> bool {
    let mut words = text.split(' ');
    for _ in 0..6 {
        let Some(word) = words.next() else {
            return false;
        };
        if word.is_empty()
            || !word.bytes().all(|byte| byte.is_ascii_digit())
            || (word.len() > 1 && word.starts_with('0'))
            || word.parse::<usize>().is_err()
        {
            return false;
        }
    }
    words.next() == Some("address") && words.next() == Some("%owners") && words.next().is_none()
}

pub(super) const fn control_bytes() -> usize {
    // The optional bridge/ID bookkeeping also has a physical representation
    // when no owned records are present. Charge the complete fixed model for
    // every metadata render, rather than treating a None value as zero bytes.
    size_of::<RecordMetadataCarriers>()
}

fn scalar(ty: Ty) -> Option<usize> {
    match ty {
        Ty::I8
        | Ty::U8
        | Ty::I16
        | Ty::U16
        | Ty::I32
        | Ty::U32
        | Ty::I64
        | Ty::U64
        | Ty::Isize
        | Ty::Usize
        | Ty::I128
        | Ty::U128
        | Ty::F32
        | Ty::F64 => TYPES.iter().position(|entry| entry.0 == ty),
        // Bool fields use i1 stores, unlike scalar debug slots' full-byte
        // definition. Unit, char and all aggregate/reference views are outside
        // this initial record scope; do not publish a misleading partial type.
        _ => None,
    }
}

pub(super) fn supported<'a>(
    records: Option<DebugRecords<'a>>,
    id: usize,
    project: &ProjectSources,
) -> Result<Option<DebugRecord<'a>>, fmt::Error> {
    let record = records
        .and_then(|records| records.record(id))
        .ok_or(fmt::Error)?;
    if !record.is_copy() || record.len() == 0 {
        return Ok(None);
    }
    let span = record.span();
    let program = project.try_file_ast(span.file).ok_or(fmt::Error)?;
    let declaration = program
        .records
        .binary_search_by_key(&span.start, |record| record.name.start)
        .ok()
        .and_then(|index| program.records.get(index))
        .filter(|record| record.name == span)
        .ok_or(fmt::Error)?;
    if !program.generic_parameters(span).is_empty() {
        return Ok(None);
    }
    if declaration.derive_copy.is_none() || declaration.fields.len() != record.len() {
        return Err(fmt::Error);
    }
    for index in 0..record.len() {
        let Some(field) = record.field(index) else {
            return Ok(None);
        };
        let Some(scalar) = scalar(field.ty) else {
            return Ok(None);
        };
        if field.span != declaration.fields[index].name
            || !project.sources().is_valid_span(field.span)
            || field.size.checked_mul(8) != Some(TYPES[scalar].3)
            || field
                .offset
                .checked_add(field.size)
                .is_none_or(|end| end > record.size())
            || field.offset % field.align != 0
        {
            return Err(fmt::Error);
        }
    }
    Ok(Some(record))
}

fn bits(bytes: usize) -> Result<usize, fmt::Error> {
    bytes.checked_mul(8).ok_or(fmt::Error)
}

pub(super) fn render(
    records: Option<DebugRecords<'_>>,
    project: &ProjectSources,
    first_record: usize,
    first_scalar: usize,
    out: &mut Output,
) -> fmt::Result {
    let Some(records) = records else {
        return Ok(());
    };
    for index in 0..records.len() {
        let Some(record) = supported(Some(records), index, project)? else {
            continue;
        };
        let id = first_record
            .checked_add(record.metadata_offset().ok_or(fmt::Error)?)
            .ok_or(fmt::Error)?;
        let members = id.checked_add(1).ok_or(fmt::Error)?;
        let span = record.span();
        let file = 5usize.checked_add(span.file.0).ok_or(fmt::Error)?;
        let row = project.sources().get(span.file).line_number(span.start);
        write!(
            out,
            "!{id} = distinct !DICompositeType(tag: DW_TAG_structure_type, name: "
        )?;
        string(out, project.sources().text(span))?;
        writeln!(out, ", scope: !{file}, file: !{file}, line: {row}, size: {}, align: {}, elements: !{members}, identifier: \"Oxid.record.{index}\")", bits(record.size())?, bits(record.align())?)?;
        write!(out, "!{members} = !{{")?;
        for field in 0..record.len() {
            if field != 0 {
                out.write_str(", ")?;
            }
            write!(
                out,
                "!{}",
                members
                    .checked_add(1)
                    .and_then(|id| id.checked_add(field))
                    .ok_or(fmt::Error)?
            )?;
        }
        out.write_str("}\n")?;
        for index in 0..record.len() {
            let field = record.field(index).ok_or(fmt::Error)?;
            let field_id = members
                .checked_add(1)
                .and_then(|id| id.checked_add(index))
                .ok_or(fmt::Error)?;
            let ty = first_scalar
                .checked_add(scalar(field.ty).ok_or(fmt::Error)?)
                .ok_or(fmt::Error)?;
            let row = project
                .sources()
                .get(field.span.file)
                .line_number(field.span.start);
            write!(
                out,
                "!{field_id} = !DIDerivedType(tag: DW_TAG_member, name: "
            )?;
            string(out, project.sources().text(field.span))?;
            writeln!(out, ", scope: !{id}, file: !{file}, line: {row}, baseType: !{ty}, size: {}, align: {}, offset: {})", bits(field.size)?, bits(field.align)?, bits(field.offset)?)?;
        }
    }
    Ok(())
}
