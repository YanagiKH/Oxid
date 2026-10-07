use super::super::{CELLS, COLUMN_STARTS, OPA_BYTES, SUCCESS_BYTES};
use super::*;
use crate::frontend::{
    declaration_index::SourceOwner,
    lexer, parser,
    project::{ProjectLimits, ProjectSources},
    source::{SourceMap, SourceView},
};

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/checked_hir_import/rich-source.txt"
));
const WIRE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/checked_hir_import/rich-success.bin"
));
macro_rules! scalar_fixture {
    ($name:literal, $kinds:expr) => {
        (
            $name,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/checked_hir_import/scalar-",
                $name,
                "-source.txt"
            )),
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/checked_hir_import/scalar-",
                $name,
                "-success.bin"
            )) as &[u8],
            $kinds,
        )
    };
}
const SCALAR_FIXTURES: &[(&str, &str, &[u8], &[u8])] = &[
    scalar_fixture!(
        "arithmetic",
        &[1, 2, 3, 5, 10, 15, 19, 21, 22, 24, 25, 26, 27, 28]
    ),
    scalar_fixture!("boolean", &[1, 2, 3, 5, 10, 16, 19, 21, 23, 35, 36]),
    scalar_fixture!(
        "comparison",
        &[1, 2, 3, 5, 10, 19, 29, 30, 31, 32, 33, 34, 35]
    ),
    scalar_fixture!("unit", &[1, 2, 4, 5, 6, 9, 10, 18, 19]),
    scalar_fixture!("loop", &[1, 2, 3, 4, 5, 10, 11, 12, 13, 14, 19]),
    scalar_fixture!("assignment", &[1, 2, 3, 5, 7, 8, 10, 15, 19]),
];
fn parsed(text: &str) -> (SourceMap, ast::Program) {
    let mut sources = SourceMap::new();
    let id = sources.add("opa-comparison.ox".into(), text.into());
    let source = sources.get(id);
    let program = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    (sources, program)
}
fn owner<'s>(sources: &'s SourceMap, program: &'s ast::Program) -> SourceOwner<'s> {
    let file = sources.get(crate::frontend::source::SourceFileId(0));
    SourceOwner::original(file, program, SourceView::Map(sources)).unwrap()
}
fn write_word(bytes: &mut [u8], column: usize, cell: usize, value: u32) {
    for (plane, byte) in value.to_le_bytes().into_iter().enumerate() {
        bytes[COLUMN_STARTS[column] + plane * CELLS + cell] = byte;
    }
}
fn encode_row(bytes: &mut [u8], reference: u8, row: Row) {
    let cell = usize::from(reference - 1);
    write_word(
        bytes,
        0,
        cell,
        u32::from(row.kind)
            | (u32::from(row.start) << 6)
            | (u32::from(row.end) << 14)
            | (u32::from(row.next) << 22),
    );
    write_word(bytes, 1, cell, u32::from(row.a) | (u32::from(row.b) << 8));
    write_word(bytes, 2, cell, u32::from(row.c) | (u32::from(row.d) << 8));
}
#[test]
fn checked_hir_import_source_opa_actual_producer_rows_match() {
    let (sources, program) = parsed(SOURCE);
    let bound = BoundObservation::bind(owner(&sources, &program), SOURCE.as_bytes(), WIRE).unwrap();
    let compared = compare(&bound).unwrap();
    assert_eq!(compared.function_row(0).unwrap(), 1);
    for (index, expression) in program.expressions.iter().enumerate() {
        let row = compared
            .row(compared.expr_row(ast::ExprId(index)).unwrap())
            .unwrap();
        assert_eq!(
            (usize::from(row.start), usize::from(row.end)),
            (expression.span.start, expression.span.end)
        );
    }
    let mut blocks = 0;
    for row in 1..=bound.wire.rows {
        if compared.row(row).unwrap().kind == 5 {
            let (function, block) = compared.block_location(row).unwrap();
            assert!(program.functions[function].blocks.get(block.0).is_some());
            blocks += 1;
        }
    }
    assert_eq!(blocks, 5);
    assert!(compared.expr_row(ast::ExprId(MAX_ROWS)).is_err());
    assert!(compared.block_location(0).is_err());
    assert!(compared.block_location(1).is_err());
    assert!(compared.function_row(MAX_ROWS).is_err());
    assert!(compared.visits() <= MAX_WORK);
}
#[test]
fn checked_hir_import_source_opa_scalar_producer_grammar_matches() {
    let mut covered = [false; 37];
    for &(name, text, bytes, expected_kinds) in SCALAR_FIXTURES {
        assert!(text.is_ascii() && text.len() <= MAX_ROWS, "{name}");
        let (sources, program) = parsed(text);
        // Original scalar ownership is available on every ordinary test host.
        // These controls need neither project admission nor canonical HIR.
        let bound = BoundObservation::bind(owner(&sources, &program), text.as_bytes(), bytes)
            .unwrap_or_else(|error| panic!("{name}: {error:?}"));
        let compared = compare(&bound).unwrap_or_else(|error| panic!("{name}: {error:?}"));
        let mut kinds = [false; 37];
        for reference in 1..=bound.wire.rows {
            let row = compared.row(reference).unwrap();
            kinds[usize::from(row.kind)] = true;
            covered[usize::from(row.kind)] = true;
        }
        for kind in 1..=36 {
            assert_eq!(
                kinds[usize::from(kind)],
                expected_kinds.contains(&kind),
                "{name}: row kind {kind}"
            );
        }
        assert_eq!(compared.function_row(0).unwrap(), 1, "{name}");
        for (index, expression) in program.expressions.iter().enumerate() {
            let row = compared
                .row(compared.expr_row(ast::ExprId(index)).unwrap())
                .unwrap();
            assert_eq!(
                (usize::from(row.start), usize::from(row.end)),
                (expression.span.start, expression.span.end),
                "{name}: expression {index}"
            );
        }
        assert!(compared.visits() <= MAX_WORK, "{name}");
    }
    let (sources, program) = parsed(SOURCE);
    let bound = BoundObservation::bind(owner(&sources, &program), SOURCE.as_bytes(), WIRE).unwrap();
    let compared = compare(&bound).unwrap();
    for reference in 1..=bound.wire.rows {
        covered[usize::from(compared.row(reference).unwrap().kind)] = true;
    }
    assert!(covered[1..].iter().all(|present| *present));
}
#[test]
fn checked_hir_import_source_opa_scalar_producer_changed_row_kinds_reject() {
    for &(name, text, bytes, _) in SCALAR_FIXTURES {
        let (sources, program) = parsed(text);
        let wire = Wire::decode(bytes, text.len()).unwrap();
        for reference in 1..=wire.rows {
            let mut changed = bytes.to_vec();
            let mut row = Row::read(wire, reference).unwrap();
            row.kind = row.kind % 36 + 1;
            encode_row(&mut changed, reference, row);
            let bound =
                BoundObservation::bind(owner(&sources, &program), text.as_bytes(), &changed)
                    .unwrap();
            assert!(compare(&bound).is_err(), "{name}: row {reference}");
        }
    }
}
#[test]
fn checked_hir_import_source_opa_bounded_split_trivia_tape_rejects() {
    for &(name, text, bytes, _) in SCALAR_FIXTURES
        .iter()
        .filter(|fixture| matches!(fixture.0, "unit" | "assignment"))
    {
        let (sources, mut program) = parsed(text);
        {
            let bound =
                BoundObservation::bind(owner(&sources, &program), text.as_bytes(), bytes).unwrap();
            assert!(compare(&bound).is_ok(), "{name}");
        }
        let index = program
            .tokens
            .iter()
            .position(|token| token.kind == Kind::Trivia && token.span.end - token.span.start > 1)
            .unwrap();
        let mut second = program.tokens[index];
        let split = second.span.start + 1;
        program.tokens[index].span.end = split;
        second.span.start = split;
        program.tokens.insert(index + 1, second);
        // Preserve the complete byte coverage and stay below the tape bound;
        // rejection must distinguish real lexer trivia extents from split ones.
        assert!(program.tokens.len() <= MAX_ROWS + 1);
        let bound =
            BoundObservation::bind(owner(&sources, &program), text.as_bytes(), bytes).unwrap();
        assert!(compare(&bound).is_err(), "{name}");
    }
}
#[test]
fn checked_hir_import_source_opa_public_producer_matches_project() {
    let text = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import/public-source.txt"
    ));
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import/public-success.bin"
    ));
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!("oxid-opa-{}-{stamp}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(directory.clone());
    let path = directory.join("main.ox");
    std::fs::write(&path, text).unwrap();
    let project =
        ProjectSources::load_typed(path.to_str().unwrap(), ProjectLimits::default()).unwrap();
    let bound =
        BoundObservation::bind(SourceOwner::project(&project), text.as_bytes(), bytes).unwrap();
    assert!(compare(&bound).is_ok());
}
#[test]
fn checked_hir_import_source_opa_every_active_bit_is_compared() {
    let (sources, program) = parsed(SOURCE);
    for cell in 0..usize::from(WIRE[8]) {
        for column in 0..3 {
            for bit in 0..32 {
                let mut bytes = WIRE.to_vec();
                bytes[COLUMN_STARTS[column] + (bit / 8) * CELLS + cell] ^= 1 << (bit % 8);
                let bound =
                    BoundObservation::bind(owner(&sources, &program), SOURCE.as_bytes(), &bytes)
                        .unwrap();
                assert!(
                    compare(&bound).is_err(),
                    "cell={cell} column={column} bit={bit}"
                );
            }
        }
    }
}
#[test]
fn checked_hir_import_source_opa_repaired_row_permutations_reject() {
    let (sources, program) = parsed(SOURCE);
    let wire = Wire::decode(WIRE, SOURCE.len()).unwrap();
    for (left, right) in [(1, 2), (6, 7), (9, 10), (16, 18), (22, 23)] {
        let remap = |row| {
            if row == left {
                right
            } else if row == right {
                left
            } else {
                row
            }
        };
        let mut bytes = WIRE.to_vec();
        bytes[9] = remap(bytes[9]);
        for reference in 1..=wire.rows {
            let mut row = Row::read(wire, reference).unwrap();
            row.next = remap(row.next);
            match row.kind {
                1 => {
                    row.b = remap(row.b);
                    row.c = remap(row.c);
                    row.d = remap(row.d);
                }
                2 | 9 | 10 | 21..=23 => row.a = remap(row.a),
                5 | 20 => row.b = remap(row.b),
                6 | 7 => {
                    row.b = remap(row.b);
                    row.c = remap(row.c);
                }
                8 => row.c = remap(row.c),
                13 => {
                    row.a = remap(row.a);
                    row.b = remap(row.b);
                    row.c = remap(row.c);
                }
                14 | 24..=36 => {
                    row.a = remap(row.a);
                    row.b = remap(row.b);
                }
                _ => (),
            }
            encode_row(&mut bytes, remap(reference), row);
        }
        let bound =
            BoundObservation::bind(owner(&sources, &program), SOURCE.as_bytes(), &bytes).unwrap();
        assert!(compare(&bound).is_err(), "repaired swap {left}/{right}");
    }
}
#[test]
fn checked_hir_import_source_opa_independent_ast_mutations_reject() {
    let mutations: &[fn(&mut ast::Program)] = &[
        |p| p.tokens[0].kind = Kind::Ident,
        |p| p.tokens[0].span.end += 1,
        |p| p.tokens.last_mut().unwrap().span.start -= 1,
        |p| p.functions[0].name.start += 1,
        |p| p.functions[0].params[0].name.end += 1,
        |p| p.functions[0].params[0].ty.span.end += 1,
        |p| p.functions[0].result.kind = ast::TypeSyntaxKind::Unit,
        |p| p.functions[0].end.start -= 1,
        |p| p.functions[1].body = ast::BodyBlockId(1),
        |p| p.functions[1].blocks[0].span.start += 1,
        |p| p.functions[1].blocks[0].body[0].span.end -= 1,
        |p| p.expressions[0].span.file.0 += 1,
        |p| p.expressions[0].span.end += 1,
        |p| p.expressions.swap(0, 1),
        |p| p.items.swap(0, 1),
        |p| {
            p.expressions.push(ast::Expr {
                kind: ast::ExprKind::Unit,
                span: p.expressions[0].span,
            })
        },
        |p| {
            let ast::StmtKind::Return(value) = &mut p.functions[0].blocks[0].body[0].kind else {
                panic!()
            };
            *value = None;
        },
        |p| {
            let ast::StmtKind::Let { mutable, .. } = &mut p.functions[1].blocks[0].body[0].kind
            else {
                panic!()
            };
            *mutable = false;
        },
        |p| {
            for e in &mut p.expressions {
                if let ast::ExprKind::Arithmetic { op, .. } = &mut e.kind {
                    *op = ast::ArithmeticOp::Multiply;
                    return;
                }
            }
            panic!();
        },
        |p| {
            for e in &mut p.expressions {
                if let ast::ExprKind::Call { args, .. } = &mut e.kind {
                    args.clear();
                    return;
                }
            }
            panic!();
        },
    ];
    for (index, mutate) in mutations.iter().enumerate() {
        let (sources, mut program) = parsed(SOURCE);
        mutate(&mut program);
        let bound =
            BoundObservation::bind(owner(&sources, &program), SOURCE.as_bytes(), WIRE).unwrap();
        assert!(compare(&bound).is_err(), "AST mutation {index}");
    }
    let changed = SOURCE.replace("n+1", "n*1");
    let (sources, program) = parsed(&changed);
    let bound =
        BoundObservation::bind(owner(&sources, &program), changed.as_bytes(), WIRE).unwrap();
    assert!(compare(&bound).is_err());
    assert!(BoundObservation::bind(owner(&sources, &program), SOURCE.as_bytes(), WIRE).is_err());
}
#[test]
fn checked_hir_import_source_opa_empty_and_full_trivia_domain() {
    for text in [
        "".to_owned(),
        " ".repeat(128),
        "/*//*/\t\n//line\n\x0b\x0c".to_owned(),
    ] {
        let (sources, program) = parsed(&text);
        let mut bytes = [0; SUCCESS_BYTES];
        bytes[..4].copy_from_slice(b"OPA1");
        bytes[10] = text.len() as u8;
        bytes[OPA_BYTES..OPA_BYTES + 4].copy_from_slice(b"STF1");
        let bound =
            BoundObservation::bind(owner(&sources, &program), text.as_bytes(), &bytes).unwrap();
        assert!(compare(&bound).is_ok());
    }
}
#[test]
fn checked_hir_import_source_opa_complete_layouts() {
    use std::mem::{align_of, size_of};
    macro_rules! layout {
        ($name:literal, $ty:ty) => {
            println!(
                "HIR_IMPORT_AST {} size={} align={}",
                $name,
                size_of::<$ty>(),
                align_of::<$ty>()
            );
        };
    }
    layout!("row", Row);
    layout!("row-result", Result<Row, Boundary>);
    layout!("event", Event);
    layout!("scratch", Scratch);
    layout!("walker", Walker<'static, 'static, 'static>);
    layout!("compared", ComparedSyntax<'static, 'static, 'static>);
    layout!(
        "compared-result",
        Result<ComparedSyntax<'static, 'static, 'static>, Boundary>
    );
    assert_eq!(SCRATCH_BYTES, size_of::<Scratch>());
    assert!(
        SCRATCH_BYTES >= size_of::<[Event; MAX_EVENTS]>() + 2 * MAX_ROWS + 6 * size_of::<usize>()
    );
}
