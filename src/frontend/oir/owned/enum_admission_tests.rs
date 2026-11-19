use super::consumer_fixtures as f;
use super::*;
use std::mem::size_of;

// Independent footprint models for the frozen pre-vector raw header and the
// vector successor. These byte arrays model catalog representation only; they
// are not builtin authority and never enter production admission or execution.
#[repr(C)]
#[allow(dead_code)]
struct PreVectorRawHeader {
    builtins: [u8; 5],
    enums: Vec<RawEnumDecl>,
    records: Vec<RawRecordDecl>,
    functions: Vec<RawOwnedFunction>,
}
#[repr(C)]
#[allow(dead_code)]
struct VectorRawHeader {
    builtins: [u8; 9],
    vector_functions: Vec<crate::frontend::builtin_catalog::vector::Function>,
    enums: Vec<RawEnumDecl>,
    records: Vec<RawRecordDecl>,
    functions: Vec<RawOwnedFunction>,
}

// One new Vec header plus the independently modeled catalog padding growth.
// Old literal measurements below remain visible and acquire this term once per
// program, including empty catalogs. Function/event/work ledgers do not grow.
pub(super) const VECTOR_RAW_HEADER_GROWTH: usize =
    size_of::<VectorRawHeader>() - size_of::<PreVectorRawHeader>();

#[test]
fn vector_raw_header_growth_is_one_descriptor_header_and_catalog_padding() {
    let header = size_of::<Vec<()>>();
    let alignment = std::mem::align_of::<Vec<()>>();
    let padded_catalog = |bytes: usize| bytes.div_ceil(alignment) * alignment;
    assert_eq!(
        size_of::<PreVectorRawHeader>(),
        3 * header + padded_catalog(5)
    );
    assert_eq!(size_of::<VectorRawHeader>(), 4 * header + padded_catalog(9));
    assert_eq!(size_of::<BuiltinOrigins>(), 9);
    assert_eq!(std::mem::align_of::<BuiltinOrigins>(), 1);
    assert_eq!(size_of::<RawOwnedProgram>(), size_of::<VectorRawHeader>());
    assert_eq!(
        VECTOR_RAW_HEADER_GROWTH,
        header + padded_catalog(9) - padded_catalog(5)
    );
    #[cfg(target_pointer_width = "64")]
    {
        assert_eq!(header, 24);
        assert_eq!(padded_catalog(5), 8);
        assert_eq!(padded_catalog(9), 16);
        assert_eq!(size_of::<PreVectorRawHeader>(), 80);
        assert_eq!(size_of::<RawOwnedProgram>(), 112);
        assert_eq!(VECTOR_RAW_HEADER_GROWTH, 32);
    }
}

fn enumeration(span: Span) -> RawEnumDecl {
    RawEnumDecl {
        id: EnumId(0),
        span,
        variants: vec![RawVariantDecl {
            id: VariantId {
                enumeration: EnumId(0),
                index: 0,
            },
            payload: None,
            span,
        }],
    }
}
fn raw(span: Span) -> RawOwnedProgram {
    let mut function = f::function(0, ValueTy::Scalar(hir::Ty::Unit), span);
    function.locals.push(f::scalar(hir::Ty::Unit, span));
    function.blocks.push(OwnedBlock {
        merge: None,
        span,
        statements: vec![f::assign(0, Rvalue::Unit, span)],
        terminator: f::end(OwnedTerminatorKind::ReturnScalar(f::operand(0, span)), span),
    });
    RawOwnedProgram {
        vector_functions: Vec::new(),
        builtins: BuiltinOrigins::None,
        enums: vec![],
        records: vec![],
        functions: vec![function],
    }
}

#[test]
fn bounded_enum_declarations_and_consumers_require_the_complete_raw_proof() {
    let (sources, span) = f::context();
    let span = span(0);
    let make = || {
        let mut p = raw(span);
        p.enums.push(enumeration(span));
        p
    };
    let witness = verify_owned(make(), &sources).unwrap();
    assert_eq!(
        execute::run(&witness, Some(hir::DefId(0))),
        Ok(Scalar::Unit)
    );
    assert!(verified::probe_array_validation(&make(), &sources, budget::Limits::DEFAULT).is_ok());
    assert_eq!(
        verified::probe_array_reference(
            make(),
            &sources,
            budget::Limits::DEFAULT,
            Some(hir::DefId(0)),
            execute::Limits::default(),
            execute::ObservationControl::default(),
        )
        .unwrap()
        .result,
        Ok(Scalar::Unit)
    );
    assert!(verified::probe_array_native(
        make(),
        &sources,
        budget::Limits::DEFAULT,
        Some(hir::DefId(0)),
        &sources,
        native::NativeControl::default(),
    )
    .unwrap()
    .result
    .is_ok());
    // Declaration presence is not authority: all witness paths still reject
    // forged nominal members before either consumer can run.
    let malformed = || {
        let mut p = make();
        p.enums[0].variants[0].id.index = 1;
        p
    };
    assert!(verify_owned(malformed(), &sources).is_err());
    assert!(verified::probe_array_reference(
        malformed(),
        &sources,
        budget::Limits::DEFAULT,
        Some(hir::DefId(0)),
        execute::Limits::default(),
        execute::ObservationControl::default(),
    )
    .is_err());
    assert!(verified::probe_array_native(
        malformed(),
        &sources,
        budget::Limits::DEFAULT,
        Some(hir::DefId(0)),
        &sources,
        native::NativeControl::default(),
    )
    .is_err());
}

#[test]
fn bounded_enum_consumers_do_not_open_source_declaration_or_match_syntax() {
    use crate::frontend::{lexer, parser};
    for text in [
        "enum Token { End } fn main()->(){return;}",
        "pub enum Token { Integer(i32), End } fn main()->(){return;}",
        "fn main()->(){match token { Token::End => {return;} }}",
    ] {
        let mut sources = SourceMap::new();
        let file = sources.add("enum-source-closed.ox".into(), text.into());
        let file = sources.get(file);
        for mode in [
            parser::SourceMode::ScalarOnly,
            parser::SourceMode::OwnedCandidate,
            parser::SourceMode::ModuleCandidate,
            parser::SourceMode::ProjectCandidate,
        ] {
            assert!(
                parser::parse_with_mode(file, lexer::lex(file).unwrap(), mode).is_err(),
                "{text}"
            );
        }
    }
}

#[test]
fn bounded_enum_raw_undeclared_equal_owner_and_result_ids_reject() {
    let (sources, span) = f::context();
    let span = span(0);
    for id in [0, 4096, u32::MAX as usize] {
        for result in [false, true] {
            let mut p = raw(span);
            let ty = AggregateTy::Enum(EnumId(id));
            p.functions[0].owners.push(OwnerDecl {
                aggregate: AggregateSlot::try_from_aggregate(ty).unwrap(),
                kind: OwnerKind::Local { mutable: true },
                span,
            });
            if result {
                p.functions[0].result = ValueTy::Owned(ty);
            }
            assert_eq!(
                verify_owned(p, &sources).unwrap_err().kind,
                OwnedFailureKind::Malformed(Malformed::Declaration(
                    DeclarationError::InvalidEnumId(EnumId(id))
                ))
            );
        }
    }
}

#[test]
fn bounded_enum_raw_inventory_charges_rows_members_and_empty_header() {
    let (_, span) = f::context();
    let span = span(0);
    let mut p = raw(span);
    let empty = budget::preflight(&p, budget::Limits::DEFAULT).unwrap();
    assert_eq!(
        empty.metadata_bytes,
        (size_of::<Vec<RawEnumDecl>>() + size_of::<usize>() + size_of::<Vec<MatchDecl>>())
            + VECTOR_RAW_HEADER_GROWTH
    );
    p.enums.push(enumeration(span));
    let usage = budget::fail_allocation_after(0, || budget::preflight(&p, budget::Limits::DEFAULT))
        .unwrap();
    assert_eq!(
        usage.metadata_bytes,
        (size_of::<Vec<RawEnumDecl>>() + size_of::<usize>() + size_of::<Vec<MatchDecl>>())
            + VECTOR_RAW_HEADER_GROWTH
            + size_of::<RawEnumDecl>()
            + size_of::<RawVariantDecl>()
    );
    assert_eq!(usage.expanded_events, 2);
    assert_eq!(usage.work, 8);
    for selector in 0..3 {
        let mut limits = budget::Limits::DEFAULT;
        match selector {
            0 => limits.metadata = usage.metadata_bytes,
            1 => limits.events = usage.expanded_events,
            _ => limits.work = usage.work,
        }
        assert!(budget::preflight(&p, limits).is_ok());
        match selector {
            0 => limits.metadata -= 1,
            1 => limits.events -= 1,
            _ => limits.work -= 1,
        }
        assert!(budget::preflight(&p, limits).is_err());
    }
    assert!(budget::preflight(
        &raw(span),
        budget::Limits {
            metadata: (size_of::<Vec<RawEnumDecl>>()
                + size_of::<usize>()
                + size_of::<Vec<MatchDecl>>())
                + VECTOR_RAW_HEADER_GROWTH
                - 1,
            ..budget::Limits::DEFAULT
        }
    )
    .is_err());
    #[cfg(target_pointer_width = "64")]
    assert_eq!(size_of::<RawOwnedProgram>(), 80 + VECTOR_RAW_HEADER_GROWTH);
}
