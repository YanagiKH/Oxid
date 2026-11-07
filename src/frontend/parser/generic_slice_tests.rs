//! Syntax-only controls. Named elements receive no semantic type admission.
use super::*;
use crate::frontend::{
    lexer,
    source::{SourceFileId, SourceMap, Span},
};

fn slice_sources(text: &str) -> SourceMap {
    let mut sources = SourceMap::new();
    sources.add("generic-slices.ox".into(), text.into());
    sources
}

fn parse_slices(
    sources: &SourceMap,
    mode: SourceMode,
    allocator: &mut Allocator,
) -> Result<Program, Vec<Diagnostic>> {
    let source = sources.get(SourceFileId(0));
    parse_typed_counted(
        source,
        lexer::lex(source).unwrap(),
        mode,
        MAX_NODES,
        allocator,
        &mut Default::default(),
    )
    .map(|(program, _)| program)
}

fn parsed(sources: &SourceMap) -> Program {
    parse_slices(
        sources,
        SourceMode::ProjectCandidate,
        &mut Allocator::default(),
    )
    .unwrap()
}

fn validate_named(
    program: &Program,
    source: &SourceFile,
    inspect: impl FnMut(Option<Span>) -> bool,
) -> bool {
    program.validate_spans_and_ids_counted_with_features(
        Some(source),
        inspect,
        &mut None,
        StdImportPolicy::Enabled,
        &mut false,
        &mut crate::frontend::ast::SyntaxFeatures::default(),
    )
}

fn valid_named(program: &Program, source: &SourceFile) -> bool {
    validate_named(program, source, |at| {
        at.is_none_or(|at| source.try_text(at).is_some())
    })
}

#[test]
fn generic_slice_parameters_preserve_mutability_comments_and_element_origins() {
    let sources = slice_sources(
        "fn use_slices<T:Copy>(shared:& /* open */ [ /* type */ T ],exclusive:&mut [T])->(){return;}",
    );
    let source = sources.get(SourceFileId(0));
    for mode in [SourceMode::OwnedCandidate, SourceMode::ProjectCandidate] {
        let program = parse_slices(&sources, mode, &mut Allocator::default()).unwrap();
        assert!(program.belongs_to(source));
        assert!(valid_named(&program, source));
        assert!(program.uses_owned_syntax(source));
        for (parameter, expected_mutability, expected_text) in [
            (
                &program.functions[0].params[0],
                false,
                "& /* open */ [ /* type */ T ]",
            ),
            (&program.functions[0].params[1], true, "&mut [T]"),
        ] {
            let TypeSyntaxKind::GenericSliceReference { mutable, element } = parameter.ty.kind
            else {
                panic!("expected a named slice parameter");
            };
            assert_eq!(mutable, expected_mutability);
            let ItemPath::Unqualified(element) = element else {
                panic!("expected the original type parameter name");
            };
            assert_eq!(source.text_at(element), "T");
            assert_eq!(source.text_at(parameter.ty.span), expected_text);
            assert_eq!(element.file, parameter.ty.span.file);
            assert!(parameter.ty.span.start < element.start && element.end < parameter.ty.span.end);
        }
    }
}

#[test]
fn named_and_absolute_slice_elements_are_syntax_without_owned_type_authority() {
    let sources = slice_sources(
        "use crate::types::Element as Alias;fn f(a:&[Alias],b:&mut[crate::types::Element],c:&[Owned])->(){return;}",
    );
    let source = sources.get(SourceFileId(0));
    let program = parsed(&sources);
    assert!(program.belongs_to(source));
    assert!(valid_named(&program, source));
    let parameters = &program.functions[0].params;
    let TypeSyntaxKind::GenericSliceReference {
        element: ItemPath::Unqualified(alias),
        ..
    } = parameters[0].ty.kind
    else {
        panic!("expected an alias spelling");
    };
    assert_eq!(source.text_at(alias), "Alias");
    let TypeSyntaxKind::GenericSliceReference {
        mutable: true,
        element: ItemPath::Absolute(id),
    } = parameters[1].ty.kind
    else {
        panic!("expected an absolute element path");
    };
    assert_eq!(program.paths[id.0].root, PathRoot::Crate);
    assert_eq!(
        source.text_at(program.paths[id.0].span),
        "crate::types::Element"
    );
    assert_eq!(
        program
            .path_segments(id)
            .unwrap()
            .iter()
            .map(|at| source.text_at(*at))
            .collect::<Vec<_>>(),
        ["crate", "types", "Element"]
    );
    // The parser intentionally does not decide whether Alias/Owned is a scalar.
    // The semantic resolver must reject an owned element or unknown name.
    assert!(matches!(
        parameters[2].ty.kind,
        TypeSyntaxKind::GenericSliceReference { .. }
    ));
}

#[test]
fn concrete_slice_parameters_keep_their_original_scalar_variants() {
    for (spelling, element) in [
        ("bool", ArrayElementTypeSyntax::Bool),
        ("i32", ArrayElementTypeSyntax::I32),
        ("u8", ArrayElementTypeSyntax::U8),
        ("()", ArrayElementTypeSyntax::Unit),
        ("i64", ArrayElementTypeSyntax::I64),
        ("i8", ArrayElementTypeSyntax::Integer(IntegerTypeSyntax::I8)),
        (
            "i16",
            ArrayElementTypeSyntax::Integer(IntegerTypeSyntax::I16),
        ),
        (
            "u16",
            ArrayElementTypeSyntax::Integer(IntegerTypeSyntax::U16),
        ),
        (
            "u32",
            ArrayElementTypeSyntax::Integer(IntegerTypeSyntax::U32),
        ),
        (
            "u64",
            ArrayElementTypeSyntax::Integer(IntegerTypeSyntax::U64),
        ),
        (
            "isize",
            ArrayElementTypeSyntax::Integer(IntegerTypeSyntax::Isize),
        ),
        (
            "usize",
            ArrayElementTypeSyntax::Integer(IntegerTypeSyntax::Usize),
        ),
        (
            "i128",
            ArrayElementTypeSyntax::Integer(IntegerTypeSyntax::I128),
        ),
        (
            "u128",
            ArrayElementTypeSyntax::Integer(IntegerTypeSyntax::U128),
        ),
        ("f32", ArrayElementTypeSyntax::Float(FloatTypeSyntax::F32)),
        ("f64", ArrayElementTypeSyntax::Float(FloatTypeSyntax::F64)),
        ("char", ArrayElementTypeSyntax::Char),
    ] {
        let sources = slice_sources(&format!(
            "fn f(a:& /* open */ [ {spelling} /* close */ ],b:&mut\t[\r\n{spelling}])->(){{return;}}"
        ));
        let source = sources.get(SourceFileId(0));
        let program = parsed(&sources);
        assert!(program.belongs_to(source));
        assert!(program.validate_spans_and_ids(|at| source.try_text(at).is_some()));
        let mut features = crate::frontend::ast::SyntaxFeatures::default();
        assert!(
            program.validate_spans_and_ids_counted_with_features(
                Some(source),
                |at| at.is_none_or(|at| source.try_text(at).is_some()),
                &mut None,
                StdImportPolicy::Enabled,
                &mut false,
                &mut features,
            ),
            "{spelling}"
        );
        assert!(features.source_slices, "{spelling}");
        assert!(!features.generic_slices, "{spelling}");
        assert!(!program.has_parsed_named_slice(), "{spelling}");
        for (position, mutable) in [false, true].into_iter().enumerate() {
            assert_eq!(
                program.functions[0].params[position].ty.kind,
                TypeSyntaxKind::SliceReference { mutable, element },
                "{spelling}"
            );
        }
    }
}

#[test]
fn named_slices_do_not_extend_fixed_arrays_or_reference_storage_positions() {
    for text in [
        "fn f<T:Copy>(a:[T;2])->(){return;}",
        "fn f<T:Copy>(a:&[T;2])->(){return;}",
        "fn f<T:Copy>(a:&mut[T;2])->(){return;}",
        "fn f(a:[crate::types::Element;2])->(){return;}",
        "fn f(a:&[crate::types::Element;2])->(){return;}",
        "fn f<T:Copy>()->&[T]{return;}",
        "struct Stored<T:Copy>{field:&[T]}",
        "fn f<T:Copy>()->(){let value:&[T]=missing;return;}",
        "fn f<T:Copy>()->(){let value:&mut[T]=missing;return;}",
        "fn f()->(){let value:&[crate::types::Element]=missing;return;}",
        "fn f(value:Holder<&[T]>)->(){return;}",
    ] {
        let sources = slice_sources(text);
        let errors = parse_slices(
            &sources,
            SourceMode::ProjectCandidate,
            &mut Allocator::default(),
        )
        .unwrap_err();
        assert_eq!(errors[0].stage, "parse", "{text}: {errors:?}");
        assert!(
            matches!(errors[0].code, "E0100" | "E0101"),
            "{text}: {errors:?}"
        );
    }
    let sources = slice_sources("fn f(a:&[i32;2],b:&mut[u8;0])->[();0]{return [];}");
    let program = parsed(&sources);
    assert_eq!(
        program.functions[0].params[0].ty.kind,
        TypeSyntaxKind::ArrayReference {
            mutable: false,
            array: FixedArraySyntax {
                element: ArrayElementTypeSyntax::I32,
                length: 2
            },
        }
    );
    assert_eq!(
        program.functions[0].params[1].ty.kind,
        TypeSyntaxKind::ArrayReference {
            mutable: true,
            array: FixedArraySyntax {
                element: ArrayElementTypeSyntax::U8,
                length: 0
            },
        }
    );
}

#[test]
fn malformed_named_slice_brackets_and_paths_report_parse_errors() {
    for (ty, code) in [
        ("&[T", "E0100"),
        ("&mut[T", "E0100"),
        ("&[T U]", "E0100"),
        ("&[T,]", "E0100"),
        ("&[T<>]", "E0100"),
        ("&[[T]]", "E0101"),
        ("&[]", "E0101"),
        ("&[T::Member]", "E0101"),
    ] {
        let sources = slice_sources(&format!("fn f(value:{ty})->(){{return;}}"));
        let errors = parse_slices(
            &sources,
            SourceMode::ProjectCandidate,
            &mut Allocator::default(),
        )
        .unwrap_err();
        assert_eq!(
            (errors[0].code, errors[0].stage),
            (code, "parse"),
            "{ty}: {errors:?}"
        );
        assert!(sources.try_text(errors[0].primary.unwrap()).is_some());
    }
}

#[test]
fn legacy_and_dormant_array_grammars_preserve_their_original_diagnostics() {
    let sources = slice_sources("fn f(value:&[T])->(){return;}");
    let source = sources.get(SourceFileId(0));
    let legacy = parse_with_mode(
        source,
        lexer::lex(source).unwrap(),
        SourceMode::OwnedCandidate,
    )
    .unwrap_err();
    assert_eq!((legacy[0].code, legacy[0].stage), ("E0101", "parse"));
    assert_eq!(legacy[0].message, "unsupported typed-preview construct `[`");
    assert_eq!(source.text_at(legacy[0].primary.unwrap()), "[");
    let dormant = parse_counted_with_arrays(
        source,
        lexer::lex(source).unwrap(),
        SourceMode::OwnedCandidate,
        MAX_NODES,
        &mut Allocator::default(),
        ArraySyntaxPolicy::Candidate,
    )
    .unwrap_err();
    assert_eq!((dormant[0].code, dormant[0].stage), ("E0101", "parse"));
    assert_eq!(
        dormant[0].message,
        "unsupported typed-preview construct `T`"
    );
    assert_eq!(source.text_at(dormant[0].primary.unwrap()), "T");
}

#[test]
fn named_slice_path_ids_spans_and_mutability_cannot_forge_source_identity() {
    let sources = slice_sources("fn f<T:Copy>(a:&[T],b:&[crate::types::Element])->(){return;}");
    let source = sources.get(SourceFileId(0));
    for mutation in 0..7 {
        let mut program = parsed(&sources);
        let ty = program.functions[0].params[0].ty;
        match mutation {
            0 => {
                program.functions[0].params[0].ty.kind = TypeSyntaxKind::GenericSliceReference {
                    mutable: false,
                    element: ItemPath::Absolute(PathId(usize::MAX)),
                }
            }
            1 => {
                program.functions[0].params[0].ty.kind = TypeSyntaxKind::GenericSliceReference {
                    mutable: false,
                    element: ItemPath::Unqualified(Span {
                        file: SourceFileId(1),
                        start: ty.span.start + 2,
                        end: ty.span.start + 3,
                    }),
                }
            }
            2 => {
                program.functions[0].params[0].ty.kind = TypeSyntaxKind::GenericSliceReference {
                    mutable: false,
                    element: ItemPath::Unqualified(Span {
                        start: ty.span.start + 2,
                        end: ty.span.start + 2,
                        ..ty.span
                    }),
                }
            }
            3 => {
                program.functions[0].params[0].ty.kind = TypeSyntaxKind::GenericSliceReference {
                    mutable: false,
                    element: ItemPath::Unqualified(program.functions[0].name),
                }
            }
            4 => {
                program.functions[0].params[0].ty.kind = TypeSyntaxKind::GenericSliceReference {
                    mutable: true,
                    element: match ty.kind {
                        TypeSyntaxKind::GenericSliceReference { element, .. } => element,
                        _ => unreachable!(),
                    },
                }
            }
            5 => program.paths[0].root = PathRoot::Std,
            6 => program.path_segments[1] = program.functions[0].name,
            _ => unreachable!(),
        }
        // Cheap parser provenance is not a second type authentication pass.
        assert!(program.belongs_to(source), "mutation {mutation}");
        assert!(!valid_named(&program, source), "mutation {mutation}");
    }
    let sources = slice_sources("fn f(a:&[i32])->(){return;}");
    let source = sources.get(SourceFileId(0));
    let mut program = parsed(&sources);
    let start = source.text().find("i32").unwrap();
    program.functions[0].params[0].ty.kind = TypeSyntaxKind::GenericSliceReference {
        mutable: false,
        element: ItemPath::Unqualified(source.span(start, start + 3)),
    };
    assert!(program.belongs_to(source));
    assert!(
        !valid_named(&program, source),
        "concrete slice representation was relabelled"
    );
}

#[test]
fn named_slice_ast_cannot_be_reused_in_stored_or_returned_type_positions() {
    let sources = slice_sources(
        "struct Record{value:i32}fn f(a:&[Element])->i32{let value:i32=1;return value;}",
    );
    let source = sources.get(SourceFileId(0));
    for location in 0..3 {
        let mut program = parsed(&sources);
        let ty = program.functions[0].params[0].ty;
        match location {
            0 => program.records[0].fields[0].ty = ty,
            1 => program.functions[0].result = ty,
            2 => {
                let StmtKind::Let { annotation, .. } =
                    &mut program.functions[0].blocks[0].body[0].kind
                else {
                    panic!("expected the annotated local");
                };
                *annotation = Some(ty);
            }
            _ => unreachable!(),
        }
        assert!(!valid_named(&program, source), "location {location}");
    }
}

#[test]
fn absolute_named_slice_parsing_remains_fallible_at_every_reservation() {
    let sources = slice_sources("fn f(value:&[crate::types::Element])->(){return;}");
    let mut baseline = Allocator::default();
    parse_slices(&sources, SourceMode::ProjectCandidate, &mut baseline).unwrap();
    assert!(baseline.attempts > 0);
    for failure in 1..=baseline.attempts {
        let mut allocator = Allocator {
            fail_at: Some(failure),
            ..Default::default()
        };
        let errors =
            parse_slices(&sources, SourceMode::ProjectCandidate, &mut allocator).unwrap_err();
        assert_eq!(errors[0].code, "E0400", "reservation {failure}: {errors:?}");
        assert_eq!(allocator.attempts, failure);
    }
}

#[test]
fn named_slice_authentication_is_source_bound_and_every_inspection_can_refuse() {
    let text = "fn f<T:Copy>(a:& /* trivia */ [T],b:&mut[crate::types::Element])->(){return;}";
    let sources = slice_sources(text);
    let source = sources.get(SourceFileId(0));
    let program = parsed(&sources);
    let mut total = 0;
    let mut observed_spans = Vec::new();
    assert!(validate_named(&program, source, |at| {
        total += 1;
        if let Some(at) = at {
            observed_spans.push(at);
        }
        at.is_none_or(|at| source.try_text(at).is_some())
    }));
    let trivia = program
        .tokens
        .iter()
        .find(|token| source.try_text(token.span) == Some("/* trivia */"))
        .unwrap()
        .span;
    // The token-table visit and the new source-authentication visit are distinct.
    assert_eq!(
        observed_spans
            .iter()
            .filter(|&&span| span == trivia)
            .count(),
        2
    );
    for allowance in 0..total {
        let mut attempted = 0;
        assert!(
            !validate_named(&program, source, |at| {
                attempted += 1;
                attempted <= allowance && at.is_none_or(|at| source.try_text(at).is_some())
            }),
            "accepted with {allowance}/{total} paid inspections"
        );
        assert_eq!(attempted, allowance + 1, "continued after refusal");
    }
    assert!(
        !program.validate_spans_and_ids(|at| source.try_text(at).is_some()),
        "compatibility validation cannot authorize new source-bearing syntax"
    );
    let other = slice_sources(text);
    assert!(
        !valid_named(&program, other.get(SourceFileId(0))),
        "matching bytes in another source owner do not grant authority"
    );
}

#[test]
fn forged_trivia_and_omitted_tokens_cannot_erase_source_mutability() {
    for mutation in 0..2 {
        let sources = slice_sources("fn f<T:Copy>(a:&mut[T])->(){return;}");
        let source = sources.get(SourceFileId(0));
        let mut program = parsed(&sources);
        let TypeSyntaxKind::GenericSliceReference { mutable, .. } =
            &mut program.functions[0].params[0].ty.kind
        else {
            unreachable!()
        };
        *mutable = false;
        let token = program
            .tokens
            .iter()
            .position(|token| token.kind == lexer::Kind::Mut)
            .unwrap();
        if mutation == 0 {
            program.tokens[token].kind = lexer::Kind::Trivia;
        } else {
            program.tokens.remove(token);
        }
        assert!(program.belongs_to(source));
        assert!(!valid_named(&program, source), "mutation {mutation}");
    }
}

#[test]
fn closed_validation_stays_legacy_and_source_slice_authentication_is_paid() {
    for text in [
        "fn main()->i32{return 3;}",
        "fn f(a:&[i32],b:&mut[u8])->(){return;}",
        "fn f(a:&[i16])->(){return;}fn main()->i32{let a=[1i16,2i16];f(&a[0..1]);return 0;}",
    ] {
        let sources = slice_sources(text);
        let source = sources.get(SourceFileId(0));
        let program = parsed(&sources);
        let mut old = 0;
        assert!(program.validate_spans_and_ids_counted(|at| {
            old += 1;
            at.is_none_or(|at| source.try_text(at).is_some())
        }));
        let mut counted = 0;
        let mut features = crate::frontend::ast::SyntaxFeatures::default();
        assert!(program.validate_spans_and_ids_counted_with_features(
            Some(source),
            |at| {
                counted += 1;
                at.is_none_or(|at| source.try_text(at).is_some())
            },
            &mut None,
            StdImportPolicy::Enabled,
            &mut false,
            &mut features,
        ));
        if text.contains("&[") {
            assert!(counted > old, "source slice authentication was not paid");
            assert!(features.source_slices);
        } else {
            assert_eq!(counted, old);
            assert!(!features.source_slices);
        }
        assert!(!features.generic_slices);
        assert_eq!(features.element_ranges, text.contains("[0..1]"));
    }
}

#[test]
fn source_slice_authentication_refuses_same_width_element_and_mutability_forgery() {
    for (spelling, replacement) in [
        (
            "i32",
            ArrayElementTypeSyntax::Integer(IntegerTypeSyntax::U32),
        ),
        ("u32", ArrayElementTypeSyntax::I32),
        (
            "i64",
            ArrayElementTypeSyntax::Integer(IntegerTypeSyntax::U64),
        ),
        ("f32", ArrayElementTypeSyntax::I32),
        ("u8", ArrayElementTypeSyntax::Integer(IntegerTypeSyntax::I8)),
    ] {
        let sources = slice_sources(&format!("fn f(a:&[{spelling}])->(){{return;}}"));
        let source = sources.get(SourceFileId(0));
        let mut program = parsed(&sources);
        program.functions[0].params[0].ty.kind = TypeSyntaxKind::SliceReference {
            mutable: false,
            element: replacement,
        };
        assert!(program.belongs_to(source));
        assert!(
            program.validate_spans_and_ids(|at| source.try_text(at).is_some()),
            "source-free compatibility wrapper changed"
        );
        assert!(
            !valid_named(&program, source),
            "relabelled {spelling} as {replacement:?}"
        );
    }
    for mutable in [false, true] {
        let qualifier = if mutable { "mut" } else { "" };
        let sources = slice_sources(&format!("fn f(a:&{qualifier}[i32])->(){{return;}}"));
        let source = sources.get(SourceFileId(0));
        let mut program = parsed(&sources);
        program.functions[0].params[0].ty.kind = TypeSyntaxKind::SliceReference {
            mutable: !mutable,
            element: ArrayElementTypeSyntax::I32,
        };
        assert!(program.belongs_to(source));
        assert!(!valid_named(&program, source), "mutability {mutable}");
    }
}

#[test]
fn primitive_source_slice_authentication_stops_at_every_unpaid_inspection() {
    let sources = slice_sources("fn f(a:& /* gap */ [i32],b:&mut[( /* unit */ )])->(){return;}");
    let source = sources.get(SourceFileId(0));
    let program = parsed(&sources);
    let mut total = 0;
    assert!(validate_named(&program, source, |at| {
        total += 1;
        at.is_none_or(|at| source.try_text(at).is_some())
    }));
    for allowance in 0..total {
        let mut attempted = 0;
        assert!(
            !validate_named(&program, source, |at| {
                attempted += 1;
                attempted <= allowance && at.is_none_or(|at| source.try_text(at).is_some())
            }),
            "accepted with {allowance}/{total} paid inspections"
        );
        assert_eq!(attempted, allowance + 1, "continued after refusal");
    }
}

#[test]
fn parsed_named_slice_fact_is_independent_of_mutable_parameter_kinds() {
    for text in [
        "fn f(p:&[T])->(){return;}",
        "fn f(p:&[T],q:&mut[T])->(){return;}",
        "fn f<T:Copy>(p:&[T])->(){return;}",
    ] {
        let sources = slice_sources(text);
        let source = sources.get(SourceFileId(0));
        let mut program = parsed(&sources);
        assert!(program.has_parsed_named_slice(), "{text}");
        for parameter in &mut program.functions[0].params {
            parameter.ty.kind = TypeSyntaxKind::Unit;
        }
        assert!(program.belongs_to(source));
        assert!(program.has_parsed_named_slice(), "{text}");
        program.functions[0].params.clear();
        assert!(program.has_parsed_named_slice(), "{text}");
    }
    for text in [
        "fn f()->(){return;}",
        "fn f(p:&T)->(){return;}",
        "fn f(p:&mut T)->(){return;}",
        "fn f(p:&[i32;1])->(){return;}",
        "fn f(p:&mut[i32;1])->(){return;}",
        "fn f<T:Copy>(p:&T)->(){return;}",
    ] {
        let sources = slice_sources(text);
        let program = parsed(&sources);
        assert!(!program.has_parsed_named_slice(), "{text}");
    }
}
