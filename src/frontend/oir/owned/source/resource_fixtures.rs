//! Source-only consumer resource fixtures. Counts are from canonical templates,
//! not observed traces, producer preflight, or predecessor raw fixtures.
use super::super::*;
use super::{lower, resolve, typeck};
use crate::frontend::{lexer, parser, source::SourceFileId};

pub(in crate::frontend::oir::owned) struct CheckedSource {
    pub sources: SourceMap,
    pub witness: verified::VerifiedOwnedProgram,
    pub entry: hir::DefId,
}

impl CheckedSource {
    pub fn span(&self, needle: &str) -> Span {
        let file = self.sources.get(SourceFileId(0));
        let start = file.text().find(needle).expect("source-owned origin");
        file.span(start, start + needle.len())
    }

    pub fn name(&self, name: &str) -> Span {
        let declaration = self.span(&format!("fn {name}"));
        Span {
            start: declaration.start + 3,
            ..declaration
        }
    }
}

pub(in crate::frontend::oir::owned) fn checked(text: &str) -> CheckedSource {
    assert!(text.len() < crate::frontend::source::MAX_SOURCE_BYTES);
    let mut sources = SourceMap::new();
    let id = sources.add("source-resources.ox".into(), text.into());
    let file = sources.get(id);
    let tokens = lexer::lex(file).unwrap();
    assert!(tokens.len() <= lexer::MAX_TOKENS + 1);
    let ast = parser::parse_with_mode(file, tokens, parser::SourceMode::OwnedCandidate).unwrap();
    let typed = typeck::check(resolve::resolve(file, &ast).unwrap()).unwrap();
    let entry = typed.entry().unwrap();
    let raw = lower::lower(&typed).unwrap();
    let witness = verified::verify_owned(raw, &sources).unwrap();
    CheckedSource {
        sources,
        witness,
        entry,
    }
}

pub(in crate::frontend::oir::owned) fn recursive(
    countdown: usize,
    recursive_padding: usize,
    main_padding: usize,
) -> String {
    format!(
        "struct T{{value:i32}} fn recur(p:&T,n:i32)->i32{{{}if n==0{{return p.value;}}else{{return recur(&*p,n-1);}}}} fn main()->i32{{let x=T{{value:7}};{}return recur(&x,{countdown});}}",
        "0;".repeat(recursive_padding),
        "0;".repeat(main_padding),
    )
}

pub(in crate::frontend::oir::owned) const OWNER_CLASSES: &str = "struct T{value:i32} fn relay(x:T)->T{return x;} fn read(p:&T)->i32{return p.value;} fn main()->i32{let x=relay(T{value:7});return read(&x);}";

pub(in crate::frontend::oir::owned) const BATCH: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/fixtures/owned_source/batch.ox"
));

pub(in crate::frontend::oir::owned) fn scalar_owner_slots(extra: bool) -> String {
    // Field initializer + return-unit are two scalar slots; the constructor
    // temporary and binding are two owner slots. K252 therefore gives S+O256.
    format!(
        "struct T{{value:i32}} fn main()->(){{let x=T{{value:7}};{}return;}}",
        "0;".repeat(252 + usize::from(extra))
    )
}

pub(in crate::frontend::oir::owned) fn native_cell_chain(extra: bool) -> String {
    // Physical views charge R10/L14 cells (80/112 bytes), keeping X8192.
    // main: S229,A1,O2,P2,L1,C1 => 229+1+2+8+14+2=256.
    // f1..f30: S229,A1,R1,L1,C1 => 229+1+10+14+2=256 each.
    // f31: S246,R1 => 246+10=256; one extra literal adds one cell.
    // Every function's S+O is below256; depth32,63blocks,S7345,X8192.
    let mut text = format!(
        "struct T{{value:i32}} fn main()->(){{let x=T{{value:7}};{}f1(&x);return;}}",
        "0;".repeat(226),
    );
    for index in 1..31 {
        text.push_str(&format!(
            "fn f{index}(p:&T)->(){{{}f{}(&*p);return;}}",
            "0;".repeat(227),
            index + 1,
        ));
    }
    text.push_str(&format!(
        "fn f31(p:&T)->(){{{}return;}}",
        "0;".repeat(245 + usize::from(extra)),
    ));
    text
}

pub(in crate::frontend::oir::owned) fn assert_owner_classes(plan: &plan::ExecutionPlan<'_>) {
    // relay parameter/name temporary; main constructor/stage/result/binding.
    let witness = plan.witness();
    assert!(matches!(
        witness.functions()[0].owners[0].kind,
        OwnerKind::Parameter { position: 0 }
    ));
    assert!(matches!(
        witness.functions()[0].owners[1].kind,
        OwnerKind::Temporary
    ));
    for (id, expected) in [
        (
            0,
            plan::FrameUsage {
                owners: 2,
                owner_cells: 2,
                payload_bytes: 8,
                expanded_cells: 10,
                reference_bytes: 72,
                native_bytes: 8,
                ..Default::default()
            },
        ),
        (
            1,
            plan::FrameUsage {
                scalar_slots: 1,
                references: 1,
                expanded_cells: 1 + 10,
                reference_bytes: 8 + 80,
                native_bytes: 16,
                ..Default::default()
            },
        ),
        (
            2,
            plan::FrameUsage {
                scalar_slots: 2,
                arguments: 2,
                owners: 4,
                owner_cells: 4,
                payload_bytes: 16,
                loans: 1,
                calls: 2,
                expanded_cells: 2 + 2 + 4 + 4 * 4 + 14 + 2 * 2,
                reference_bytes: 4 * 8 + 16 + 4 * 32 + 112 + 2 * 16,
                native_bytes: 56,
                ..Default::default()
            },
        ),
    ] {
        assert_eq!(plan.function(hir::DefId(id)).usage(), expected);
    }
    let kinds: Vec<_> = witness.functions()[2]
        .owners
        .iter()
        .map(|o| o.kind)
        .collect();
    assert_eq!(
        kinds
            .iter()
            .filter(|k| matches!(k, OwnerKind::Temporary))
            .count(),
        1
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|k| matches!(k, OwnerKind::StagedArgument { .. }))
            .count(),
        1
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|k| matches!(k, OwnerKind::CallResult { .. }))
            .count(),
        1
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|k| matches!(k, OwnerKind::Local { mutable: false }))
            .count(),
        1
    );
}

/// Native slice qualification uses the enabled array grammar while preserving
/// the record-only helper and its unchanged source templates.
pub(in crate::frontend::oir::owned) fn checked_arrays(text: &str) -> CheckedSource {
    let mut sources = SourceMap::new();
    let file = sources.add("slice-native.ox".into(), text.into());
    let source = sources.get(file);
    let (ast, _) = parser::parse_counted_with_arrays(
        source,
        lexer::lex(source).unwrap(),
        parser::SourceMode::OwnedCandidate,
        parser::MAX_NODES,
        &mut crate::frontend::project::budget::Allocator::default(),
        parser::ArraySyntaxPolicy::Enabled,
    )
    .unwrap();
    let typed = typeck::check(resolve::resolve_in_map(source, &ast, &sources).unwrap()).unwrap();
    let entry = typed.entry().unwrap();
    let raw = lower::lower(&typed).unwrap();
    let witness = verified::verify_owned(raw, &sources).unwrap();
    CheckedSource {
        sources,
        witness,
        entry,
    }
}
