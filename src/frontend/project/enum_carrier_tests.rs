//! Internal carrier controls, with the historical closed policy kept explicit.
use super::*;
use crate::frontend::{
    declaration_index::{self, IndexLimits, SourceOwner, WorkMeter},
    hir,
    source::SourceView,
};
use std::mem::{align_of, size_of};

const TEXT: &str = "/* E::V E::W bool payload */ fn f()->(){return;}";

fn fixture(text: &str) -> ProjectSources {
    let mut sources = SourceMap::new();
    let file = sources.add("carrier.ox".into(), text.into());
    let source = sources.get(file);
    let (program, nodes) = parser::parse_counted(
        source,
        lexer::lex(source).unwrap(),
        parser::SourceMode::ProjectCandidate,
        parser::MAX_NODES,
        &mut Allocator::default(),
    )
    .unwrap();
    let flavor = if program.uses_project_syntax() {
        SyntaxFlavor::ProjectSyntax
    } else {
        SyntaxFlavor::OriginalSingleFile
    };
    let usage = SourceUsage {
        source_bytes: text.len(),
        non_eof_tokens: program.tokens.len() - 1,
        syntax_nodes: nodes,
        line_starts: source.line_count(),
        modules: 1,
        ..SourceUsage::default()
    };
    ProjectSources {
        sources,
        programs: vec![program],
        modules: vec![ModuleHeader {
            file,
            parent: None,
            declaration: None,
            public: None,
            depth: 0,
            relative_path: "carrier.ox".into(),
            canonical_path: None,
        }],
        canonical_root: None,
        usage,
        syntax_flavor: flavor,
    }
}
fn span(project: &ProjectSources, spelling: &str) -> Span {
    let source = project.sources.get(SourceFileId(0));
    let start = source.text().find(spelling).unwrap();
    source.span(start, start + spelling.len())
}
fn local_path(project: &mut ProjectSources, spelling: &str) -> ast::PathId {
    let at = span(project, spelling);
    let program = &mut project.programs[0];
    let id = ast::PathId(program.paths.len());
    program.paths.push(ast::QualifiedPath {
        span: at,
        segment_start: program.path_segments.len(),
        segment_len: 2,
        root: ast::PathRoot::LocalType,
    });
    program.path_segments.extend([
        Span {
            end: at.start + 1,
            ..at
        },
        Span {
            start: at.end - 1,
            ..at
        },
    ]);
    id
}
fn declaration(project: &mut ProjectSources, count: usize) {
    let at = span(project, "E::V");
    let payload = span(project, "bool");
    let program = &mut project.programs[0];
    program.items.push(ast::ItemId::Enum(program.enums.len()));
    program.enums.push(ast::EnumDecl {
        public: None,
        name: Span {
            end: at.start + 1,
            ..at
        },
        variants: (0..count)
            .map(|index| ast::EnumVariantSyntax {
                name: Span {
                    start: at.end - 1,
                    ..at
                },
                payload: (index != 0).then_some(ast::EnumPayloadSyntax {
                    kind: ast::ScalarTypeSyntax::Bool,
                    span: payload,
                }),
                span: at,
            })
            .collect(),
        span: at,
        end: at,
    });
}
fn qualified(project: &mut ProjectSources, path: ast::PathId, args: Option<Vec<ast::Argument>>) {
    let program = &mut project.programs[0];
    program.expressions.push(ast::Expr {
        span: program.paths[path.0].span,
        kind: ast::ExprKind::QualifiedValue { path, args },
    });
}
fn matching(project: &mut ProjectSources, path: ast::PathId) {
    let at = span(project, "payload");
    let function = &mut project.programs[0].functions[0];
    let body = ast::BodyBlockId(function.blocks.len());
    function.blocks.push(ast::BodyBlock {
        body: Vec::new(),
        span: at,
        end: at,
    });
    function.blocks[0].body[0].kind = ast::StmtKind::Match {
        scrutinee: at,
        arms: vec![ast::MatchArmSyntax {
            variant: path,
            binding: Some(at),
            body,
            span: at,
        }],
    };
}
fn valid(project: &ProjectSources) -> bool {
    project.programs[0].validate_spans_and_ids(|at| project.try_text(at).is_some())
}

#[test]
fn enum_carrier_actual_layouts_preserve_hot_enclosures() {
    macro_rules! report {
        ($($ty:ty),+ $(,)?) => { $(
            println!("c1a-layout {} bytes={} align={}", stringify!($ty), size_of::<$ty>(), align_of::<$ty>());
        )+ };
    }
    report!(
        lexer::Kind,
        lexer::Token,
        Span,
        ast::PathId,
        ast::ItemPath,
        ast::QualifiedPath,
        ast::Program,
        ast::ExprKind,
        ast::Expr,
        Option<ast::Expr>,
        ast::StmtKind,
        ast::Stmt,
        ast::Function,
        ast::BodyBlock,
        ast::Param,
        ast::Argument,
        ast::TypeSyntaxKind,
        ast::TypeSyntax,
        ast::EnumPayloadSyntax,
        Option<ast::EnumPayloadSyntax>,
        ast::EnumVariantSyntax,
        ast::EnumDecl,
        ast::MatchArmSyntax,
        ast::ItemId,
        QualifiedPathRef,
        ProjectSources,
        SourceUsage,
        Inventory,
        ModuleHeader,
        SourceSetBuilder<'_>,
    );
    #[cfg(target_pointer_width = "64")]
    {
        assert_eq!(
            (size_of::<ast::ExprKind>(), size_of::<ast::Expr>()),
            (64, 88)
        );
        assert_eq!(
            (size_of::<ast::StmtKind>(), size_of::<ast::Stmt>()),
            (112, 136)
        );
        assert_eq!(
            (size_of::<ast::ItemPath>(), size_of::<ast::QualifiedPath>()),
            (32, 40)
        );
        assert_eq!(size_of::<ast::Program>(), 272);
        assert_eq!(
            (size_of::<ast::Function>(), size_of::<ast::TypeSyntax>()),
            (200, 64)
        );
    }
}

#[test]
fn enum_carrier_legacy_parser_stays_closed_and_formatter_accepts_syntax() {
    for text in [
        "enum E { V } fn f()->(){return;}",
        "fn f()->(){E::V;return;}",
        "fn f()->(){match e { E::V => {return;} }}",
    ] {
        let mut sources = SourceMap::new();
        let id = sources.add("closed.ox".into(), text.into());
        let source = sources.get(id);
        for mode in [
            parser::SourceMode::OwnedCandidate,
            parser::SourceMode::ProjectCandidate,
        ] {
            assert!(parser::parse_with_mode(source, lexer::lex(source).unwrap(), mode).is_err());
        }
        assert!(crate::frontend::format::format_source(source).is_ok());
    }
    for text in [TEXT, "fn f()->i32{return crate::f();}"] {
        let project = fixture(text);
        let program = &project.programs[0];
        assert!(program.enums.is_empty());
        let mut feature = None;
        assert!(program.validate_spans_and_ids_counted_with_enum_syntax(
            |at| at.is_none_or(|at| project.try_text(at).is_some()),
            &mut feature,
        ));
        assert!(feature.is_none());
        assert!(!program
            .expressions
            .iter()
            .any(|expression| matches!(expression.kind, ast::ExprKind::QualifiedValue { .. })));
        assert!(valid(&project));
    }
}

#[test]
fn enum_carrier_local_paths_are_not_project_syntax_or_absolute_authority() {
    let mut project = fixture(TEXT);
    let path = local_path(&mut project, "E::V");
    qualified(&mut project, path, None);
    assert!(valid(&project));
    assert!(!project.programs[0].uses_project_syntax());
    assert!(project.uses_owned_syntax());
    let owner = SourceOwner::project(&project);
    assert!(owner.owned(&WorkMeter::default()).unwrap());
    let handle = QualifiedPathRef {
        file: SourceFileId(0),
        path,
    };
    let view = owner.qualified_path(handle).unwrap();
    assert_eq!(view.root(), ast::PathRoot::LocalType);
    assert_eq!(view.span(), span(&project, "E::V"));
    assert_eq!(view.segments().len(), 2);
    assert!(owner
        .path_span(ItemPathRef {
            file: SourceFileId(0),
            path: ast::ItemPath::Absolute(path)
        })
        .is_err());
    // A forged root cannot turn the E spelling into crate authority.
    project.programs[0].paths[path.0].root = ast::PathRoot::Crate;
    let owner = SourceOwner::project(&project);
    assert!(owner.qualified_path(handle).is_err());
    assert!(owner
        .path_span(ItemPathRef {
            file: SourceFileId(0),
            path: ast::ItemPath::Absolute(path)
        })
        .is_err());
}

#[test]
fn enum_carrier_absolute_function_shape_remains_owned_route_neutral() {
    let mut project = fixture("fn f()->i32{return crate::f();}");
    let program = &mut project.programs[0];
    let expression = &mut program.expressions[0];
    let ast::ExprKind::Call {
        callee: ast::ItemPath::Absolute(path),
        args,
    } = std::mem::replace(&mut expression.kind, ast::ExprKind::Unit)
    else {
        panic!("fixture is an absolute call")
    };
    expression.kind = ast::ExprKind::QualifiedValue {
        path,
        args: Some(args),
    };
    assert!(valid(&project));
    assert!(!project.uses_owned_syntax());
    assert!(!SourceOwner::project(&project)
        .owned(&WorkMeter::default())
        .unwrap());
    let mut feature = None;
    assert!(project.programs[0]
        .validate_spans_and_ids_counted_with_enum_syntax(|_| true, &mut feature,));
    assert!(feature.is_some());
}

#[test]
fn enum_carrier_closed_collection_and_candidate_producers_reject_new_forms() {
    for kind in 0..3 {
        let mut project = fixture(TEXT);
        match kind {
            0 => declaration(&mut project, 1),
            1 => {
                let path = local_path(&mut project, "E::V");
                qualified(&mut project, path, None);
            }
            _ => {
                let path = local_path(&mut project, "E::V");
                matching(&mut project, path);
            }
        }
        assert!(valid(&project));
        let work = WorkMeter::default();
        let mut allocator = Allocator::default();
        let error = declaration_index::collect_closed(
            SourceOwner::project(&project),
            IndexLimits::default(),
            &work,
            &mut allocator,
        )
        .unwrap_err();
        assert_eq!(
            (error.code, error.message.as_str()),
            ("E0101", "enum source syntax is unavailable")
        );
        assert_eq!(allocator.attempts, 0);
        let source = project.sources.get(SourceFileId(0));
        let owner = SourceOwner::original(source, &project.programs[0], SourceView::Single(source))
            .unwrap();
        assert!(owner.owned(&WorkMeter::default()).unwrap());
        assert_eq!(
            hir::resolve(source, &project.programs[0]).unwrap_err()[0].code,
            "E0101"
        );
        // Fabricated carriers are only negative controls. The explicit private
        // collection policy preserves its marker at direct producer seams.
        let work = WorkMeter::default();
        let mut allocator = Allocator::default();
        let facts = declaration_index::collect_enum_candidate(
            SourceOwner::project(&project),
            IndexLimits::default(),
            &work,
            &mut allocator,
        )
        .unwrap();
        assert_eq!(
            hir::original_signatures(&facts, &work).unwrap_err()[0].code,
            "E0101"
        );
        let index = facts.finish(&work, &mut allocator).unwrap();
        assert_eq!(
            hir::resolve_project(&index, &work).unwrap_err()[0].code,
            "E0101"
        );
        assert_eq!(
            hir::resolve_bodies(&index, &work, Vec::new()).unwrap_err()[0].code,
            "E0101"
        );
    }
}

#[test]
fn enum_carrier_structural_negative_controls() {
    for count in [0, 1, 256, 257] {
        let mut project = fixture(TEXT);
        declaration(&mut project, count);
        assert_eq!(valid(&project), (1..=256).contains(&count));
    }
    let mut project = fixture(TEXT);
    declaration(&mut project, 2);
    project.programs[0].items.push(ast::ItemId::Enum(0));
    assert!(!valid(&project));
    project.programs[0].items.pop();
    project.programs[0].enums[0].variants[1]
        .payload
        .as_mut()
        .unwrap()
        .span
        .file = SourceFileId(1);
    assert!(!valid(&project));

    for corruption in 0..8 {
        let mut project = fixture(TEXT);
        let path = local_path(&mut project, "E::V");
        matching(&mut project, path);
        assert!(valid(&project));
        match corruption {
            0 => project.programs[0].paths[0].segment_start = usize::MAX,
            1 => project.programs[0].paths[0].segment_len = 3,
            2 => project.programs[0].path_segments[1].start = 0,
            3 => {
                local_path(&mut project, "E::V");
            }
            _ => {
                let ast::StmtKind::Match { arms, .. } =
                    &mut project.programs[0].functions[0].blocks[0].body[0].kind
                else {
                    unreachable!()
                };
                match corruption {
                    4 => arms[0].variant = ast::PathId(1),
                    5 => arms[0].body = ast::BodyBlockId(0),
                    6 => arms[0].body = ast::BodyBlockId(2),
                    _ => arms[0].binding.as_mut().unwrap().file = SourceFileId(1),
                }
            }
        }
        assert!(!valid(&project), "corruption {corruption}");
    }
    let mut project = fixture(TEXT);
    let path = local_path(&mut project, "E::V");
    qualified(
        &mut project,
        path,
        Some(vec![ast::Argument::Value(ast::ExprId(0))]),
    );
    assert!(!valid(&project), "self/forward expression edge");
}

#[test]
fn enum_carrier_inventory_counts_nested_payloads_once() {
    let mut project = fixture(TEXT);
    let baseline = project.inventory().unwrap();
    declaration(&mut project, 3);
    let path = local_path(&mut project, "E::V");
    matching(&mut project, path);
    let at = span(&project, "bool");
    project.programs[0].expressions.push(ast::Expr {
        kind: ast::ExprKind::Bool(true),
        span: at,
    });
    qualified(
        &mut project,
        path,
        Some(vec![ast::Argument::Value(ast::ExprId(0))]),
    );
    assert!(valid(&project));
    let expected = size_of::<ast::EnumDecl>()
        + 3 * size_of::<ast::EnumVariantSyntax>()
        + size_of::<ast::ItemId>()
        + size_of::<ast::QualifiedPath>()
        + 2 * size_of::<Span>()
        + size_of::<ast::BodyBlock>()
        + size_of::<ast::MatchArmSyntax>()
        + 2 * size_of::<ast::Expr>()
        + size_of::<ast::Argument>();
    let inventory = project.inventory().unwrap();
    assert_eq!(inventory.ast_payload - baseline.ast_payload, expected);
    assert_eq!(inventory.ast_headers, size_of::<ast::Program>());
    assert_eq!(inventory.tokens, baseline.tokens);
    // Spare capacity is distinct from logical inventory; never infer it from len.
    let program = &mut project.programs[0];
    program.enums.reserve_exact(7);
    program.enums[0].variants.reserve_exact(13);
    let capacity_bytes = program.enums.capacity() * size_of::<ast::EnumDecl>()
        + program.enums[0].variants.capacity() * size_of::<ast::EnumVariantSyntax>();
    assert!(capacity_bytes > size_of::<ast::EnumDecl>() + 3 * size_of::<ast::EnumVariantSyntax>());
    println!("c1a-capacity enum-and-variant-bytes={capacity_bytes}");
    assert_eq!(
        project.inventory().unwrap().ast_payload,
        inventory.ast_payload
    );
}
