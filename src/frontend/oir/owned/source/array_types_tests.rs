//! Focused private types-only controls. Sources and language diagnostics are
//! copied from the independently frozen contracts-v2 authority, never from a
//! candidate observation. This is not an executable array qualification.
use super::{budget, diagnostic, hir::*, lower, program, resolve, typeck};
use crate::frontend::{
    ast,
    declaration_index::{IndexLimits, SourceOwner, WorkMeter},
    diagnostic::Diagnostic,
    lexer, parser,
    project::budget::Allocator,
    source::{SourceMap, SourceView},
};

fn parsed(text: &str) -> (SourceMap, ast::Program) {
    let mut sources = SourceMap::new();
    let id = sources.add("main.ox".into(), text.into());
    let file = sources.get(id);
    let (ast, _) = parser::parse_counted_with_arrays(
        file,
        lexer::lex(file).unwrap(),
        parser::SourceMode::OwnedCandidate,
        parser::MAX_NODES,
        &mut Allocator::default(),
        parser::ArraySyntaxPolicy::Candidate,
    )
    .unwrap();
    (sources, ast)
}
fn resolve_types<'s>(
    sources: &'s SourceMap,
    ast: &'s ast::Program,
    work: &'s WorkMeter,
    allocator: &mut Allocator,
) -> Result<resolve::ResolvedOwnedProgram<'s>, Vec<Diagnostic>> {
    let source = sources.get(crate::frontend::source::SourceFileId(0));
    let owner = SourceOwner::original(source, ast, SourceView::Map(sources)).unwrap();
    resolve::resolve_array_types(owner, IndexLimits::default(), work, allocator)
}

#[test]
fn unit3b1_frozen_language_diagnostics_keep_all_29_renderings() {
    let cases = [
        (
            r#"read-index-internal-type-first"#,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-index-internal-type-first/main.ox")),
            r#"error[E0300] (type): type mismatch: expected i32, found bool
  --> main.ox:1:40
"#,
            r#"{"schema_version":1,"edition":"typed-preview","kind":"diagnostic","severity":"error","code":"E0300","stage":"type","message":"type mismatch: expected i32, found bool","primary":{"file_id":0,"path":"main.ox","start":39,"end":43,"line":1,"column":40,"end_line":1,"end_column":44},"secondary":[],"notes":[]}
"#,
        ),
        (
            r#"read-base-kind-before-index-kind"#,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-base-kind-before-index-kind/main.ox")),
            r#"error[E0305] (type): array access requires an array binding
  --> main.ox:1:38
  ::: main.ox:1:24: binding declared here
"#,
            r#"{"schema_version":1,"edition":"typed-preview","kind":"diagnostic","severity":"error","code":"E0305","stage":"type","message":"array access requires an array binding","primary":{"file_id":0,"path":"main.ox","start":37,"end":44,"line":1,"column":38,"end_line":1,"end_column":45},"secondary":[{"span":{"file_id":0,"path":"main.ox","start":23,"end":24,"line":1,"column":24,"end_line":1,"end_column":25},"message":"binding declared here"}],"notes":[]}
"#,
        ),
        (
            r#"read-array-bad-index"#,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-array-bad-index/main.ox")),
            r#"error[E0300] (type): type mismatch: expected i32, found bool
  --> main.ox:1:42
"#,
            r#"{"schema_version":1,"edition":"typed-preview","kind":"diagnostic","severity":"error","code":"E0300","stage":"type","message":"type mismatch: expected i32, found bool","primary":{"file_id":0,"path":"main.ox","start":41,"end":45,"line":1,"column":42,"end_line":1,"end_column":46},"secondary":[],"notes":[]}
"#,
        ),
        (
            r#"read-unknown-base-first"#,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-unknown-base-first/main.ox")),
            r#"error[E0200] (resolve): unknown local `missing`
  --> main.ox:1:27
"#,
            r#"{"schema_version":1,"edition":"typed-preview","kind":"diagnostic","severity":"error","code":"E0200","stage":"resolve","message":"unknown local `missing`","primary":{"file_id":0,"path":"main.ox","start":26,"end":33,"line":1,"column":27,"end_line":1,"end_column":34},"secondary":[],"notes":[]}
"#,
        ),
        (
            r#"read-index-resolution-before-base-type"#,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-index-resolution-before-base-type/main.ox")),
            r#"error[E0200] (resolve): unknown direct function `unknown`
  --> main.ox:1:40
"#,
            r#"{"schema_version":1,"edition":"typed-preview","kind":"diagnostic","severity":"error","code":"E0200","stage":"resolve","message":"unknown direct function `unknown`","primary":{"file_id":0,"path":"main.ox","start":39,"end":46,"line":1,"column":40,"end_line":1,"end_column":47},"secondary":[],"notes":[]}
"#,
        ),
        (
            r#"read-index-literal-range-before-base-type"#,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-index-literal-range-before-base-type/main.ox")),
            r#"error[E0203] (resolve): decimal literal is outside the i32 range [-2147483648, 2147483647]
  --> main.ox:1:40
"#,
            r#"{"schema_version":1,"edition":"typed-preview","kind":"diagnostic","severity":"error","code":"E0203","stage":"resolve","message":"decimal literal is outside the i32 range [-2147483648, 2147483647]","primary":{"file_id":0,"path":"main.ox","start":39,"end":49,"line":1,"column":40,"end_line":1,"end_column":50},"secondary":[],"notes":[]}
"#,
        ),
        (
            r#"write-rhs-internal-type-first"#,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-rhs-internal-type-first/main.ox")),
            r#"error[E0300] (type): type mismatch: expected i32, found bool
  --> main.ox:1:42
"#,
            r#"{"schema_version":1,"edition":"typed-preview","kind":"diagnostic","severity":"error","code":"E0300","stage":"type","message":"type mismatch: expected i32, found bool","primary":{"file_id":0,"path":"main.ox","start":41,"end":45,"line":1,"column":42,"end_line":1,"end_column":46},"secondary":[],"notes":[]}
"#,
        ),
        (
            r#"write-index-internal-type-before-base"#,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-index-internal-type-before-base/main.ox")),
            r#"error[E0300] (type): type mismatch: expected i32, found bool
  --> main.ox:1:33
"#,
            r#"{"schema_version":1,"edition":"typed-preview","kind":"diagnostic","severity":"error","code":"E0300","stage":"type","message":"type mismatch: expected i32, found bool","primary":{"file_id":0,"path":"main.ox","start":32,"end":36,"line":1,"column":33,"end_line":1,"end_column":37},"secondary":[],"notes":[]}
"#,
        ),
        (
            r#"write-base-kind-before-index-kind"#,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-base-kind-before-index-kind/main.ox")),
            r#"error[E0305] (type): array access requires an array binding
  --> main.ox:1:31
  ::: main.ox:1:24: binding declared here
"#,
            r#"{"schema_version":1,"edition":"typed-preview","kind":"diagnostic","severity":"error","code":"E0305","stage":"type","message":"array access requires an array binding","primary":{"file_id":0,"path":"main.ox","start":30,"end":37,"line":1,"column":31,"end_line":1,"end_column":38},"secondary":[{"span":{"file_id":0,"path":"main.ox","start":23,"end":24,"line":1,"column":24,"end_line":1,"end_column":25},"message":"binding declared here"}],"notes":[]}
"#,
        ),
        (
            r#"write-index-kind-before-rhs-element"#,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-index-kind-before-rhs-element/main.ox")),
            r#"error[E0300] (type): type mismatch: expected i32, found bool
  --> main.ox:1:35
"#,
            r#"{"schema_version":1,"edition":"typed-preview","kind":"diagnostic","severity":"error","code":"E0300","stage":"type","message":"type mismatch: expected i32, found bool","primary":{"file_id":0,"path":"main.ox","start":34,"end":38,"line":1,"column":35,"end_line":1,"end_column":39},"secondary":[],"notes":[]}
"#,
        ),
        (
            r#"write-element-before-mutability"#,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-element-before-mutability/main.ox")),
            r#"error[E0300] (type): type mismatch: expected i32, found bool
  --> main.ox:1:40
"#,
            r#"{"schema_version":1,"edition":"typed-preview","kind":"diagnostic","severity":"error","code":"E0300","stage":"type","message":"type mismatch: expected i32, found bool","primary":{"file_id":0,"path":"main.ox","start":39,"end":44,"line":1,"column":40,"end_line":1,"end_column":45},"secondary":[],"notes":[]}
"#,
        ),
        (
            r#"write-owner-mutability-last"#,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-owner-mutability-last/main.ox")),
            r#"error[E0304] (type): operation requires a mutable owned binding
  --> main.ox:1:33
  ::: main.ox:1:24: immutable binding declared here
"#,
            r#"{"schema_version":1,"edition":"typed-preview","kind":"diagnostic","severity":"error","code":"E0304","stage":"type","message":"operation requires a mutable owned binding","primary":{"file_id":0,"path":"main.ox","start":32,"end":36,"line":1,"column":33,"end_line":1,"end_column":37},"secondary":[{"span":{"file_id":0,"path":"main.ox","start":23,"end":24,"line":1,"column":24,"end_line":1,"end_column":25},"message":"immutable binding declared here"}],"notes":[]}
"#,
        ),
        (
            r#"empty-no-context"#,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-no-context/main.ox")),
            r#"error[E0300] (type): empty array literal requires an explicit array annotation on its local initializer
  --> main.ox:1:29
"#,
            r#"{"schema_version":1,"edition":"typed-preview","kind":"diagnostic","severity":"error","code":"E0300","stage":"type","message":"empty array literal requires an explicit array annotation on its local initializer","primary":{"file_id":0,"path":"main.ox","start":28,"end":30,"line":1,"column":29,"end_line":1,"end_column":31},"secondary":[],"notes":[]}
"#,
        ),
        (
            r#"empty-nonzero-annotation"#,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-nonzero-annotation/main.ox")),
            r#"error[E0300] (type): type mismatch: expected [i32; 1], found [i32; 0]
  --> main.ox:1:38
  ::: main.ox:1:24: binding declared here
"#,
            r#"{"schema_version":1,"edition":"typed-preview","kind":"diagnostic","severity":"error","code":"E0300","stage":"type","message":"type mismatch: expected [i32; 1], found [i32; 0]","primary":{"file_id":0,"path":"main.ox","start":37,"end":41,"line":1,"column":38,"end_line":1,"end_column":42},"secondary":[{"span":{"file_id":0,"path":"main.ox","start":23,"end":24,"line":1,"column":24,"end_line":1,"end_column":25},"message":"binding declared here"}],"notes":[]}
"#,
        ),
        (
            r#"nested-empty-does-not-inherit-context"#,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/nested-empty-does-not-inherit-context/main.ox")),
            r#"error[E0300] (type): empty array literal requires an explicit array annotation on its local initializer
  --> main.ox:1:39
"#,
            r#"{"schema_version":1,"edition":"typed-preview","kind":"diagnostic","severity":"error","code":"E0300","stage":"type","message":"empty array literal requires an explicit array annotation on its local initializer","primary":{"file_id":0,"path":"main.ox","start":38,"end":40,"line":1,"column":39,"end_line":1,"end_column":41},"secondary":[],"notes":[]}
"#,
        ),
        (
            r#"utf8-read-primary"#,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/utf8-read-primary/main.ox")),
            r#"error[E0305] (type): array access requires an array binding
  --> main.ox:2:38
  ::: main.ox:2:24: binding declared here
"#,
            r#"{"schema_version":1,"edition":"typed-preview","kind":"diagnostic","severity":"error","code":"E0305","stage":"type","message":"array access requires an array binding","primary":{"file_id":0,"path":"main.ox","start":50,"end":57,"line":2,"column":38,"end_line":2,"end_column":45},"secondary":[{"span":{"file_id":0,"path":"main.ox","start":36,"end":37,"line":2,"column":24,"end_line":2,"end_column":25},"message":"binding declared here"}],"notes":[]}
"#,
        ),
        (
            r#"write-resolve-base-before-both-operands"#,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-resolve-base-before-both-operands/main.ox")),
            r#"error[E0200] (resolve): unknown local `missing`
  --> main.ox:1:20
"#,
            r#"{"schema_version":1,"edition":"typed-preview","kind":"diagnostic","severity":"error","code":"E0200","stage":"resolve","message":"unknown local `missing`","primary":{"file_id":0,"path":"main.ox","start":19,"end":26,"line":1,"column":20,"end_line":1,"end_column":27},"secondary":[],"notes":[]}
"#,
        ),
        (
            r#"write-resolve-rhs-before-index"#,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-resolve-rhs-before-index/main.ox")),
            r#"error[E0200] (resolve): unknown direct function `absent`
  --> main.ox:1:52
"#,
            r#"{"schema_version":1,"edition":"typed-preview","kind":"diagnostic","severity":"error","code":"E0200","stage":"resolve","message":"unknown direct function `absent`","primary":{"file_id":0,"path":"main.ox","start":51,"end":57,"line":1,"column":52,"end_line":1,"end_column":58},"secondary":[],"notes":[]}
"#,
        ),
        (
            r#"write-whole-resolution-before-rhs-typing"#,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-whole-resolution-before-rhs-typing/main.ox")),
            r#"error[E0200] (resolve): unknown direct function `unknown`
  --> main.ox:1:39
"#,
            r#"{"schema_version":1,"edition":"typed-preview","kind":"diagnostic","severity":"error","code":"E0200","stage":"resolve","message":"unknown direct function `unknown`","primary":{"file_id":0,"path":"main.ox","start":38,"end":45,"line":1,"column":39,"end_line":1,"end_column":46},"secondary":[],"notes":[]}
"#,
        ),
        (
            r#"write-both-subtree-errors-rhs-wins"#,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-both-subtree-errors-rhs-wins/main.ox")),
            r#"error[E0300] (type): type mismatch: expected i32, found bool
  --> main.ox:1:46
"#,
            r#"{"schema_version":1,"edition":"typed-preview","kind":"diagnostic","severity":"error","code":"E0300","stage":"type","message":"type mismatch: expected i32, found bool","primary":{"file_id":0,"path":"main.ox","start":45,"end":49,"line":1,"column":46,"end_line":1,"end_column":50},"secondary":[],"notes":[]}
"#,
        ),
        (
            r#"whole-program-resolution-before-earlier-function-type"#,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/whole-program-resolution-before-earlier-function-type/main.ox")),
            r#"error[E0200] (resolve): unknown direct function `unknown`
  --> main.ox:2:27
"#,
            r#"{"schema_version":1,"edition":"typed-preview","kind":"diagnostic","severity":"error","code":"E0200","stage":"resolve","message":"unknown direct function `unknown`","primary":{"file_id":0,"path":"main.ox","start":77,"end":84,"line":2,"column":27,"end_line":2,"end_column":34},"secondary":[],"notes":[]}
"#,
        ),
        (
            r#"write-one-conflict-element"#,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-one-conflict-element/main.ox")),
            r#"error[E0300] (type): type mismatch: expected i32, found bool
  --> main.ox:1:44
"#,
            r#"{"schema_version":1,"edition":"typed-preview","kind":"diagnostic","severity":"error","code":"E0300","stage":"type","message":"type mismatch: expected i32, found bool","primary":{"file_id":0,"path":"main.ox","start":43,"end":48,"line":1,"column":44,"end_line":1,"end_column":49},"secondary":[],"notes":[]}
"#,
        ),
        (
            r#"write-one-conflict-index"#,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-one-conflict-index/main.ox")),
            r#"error[E0300] (type): type mismatch: expected i32, found bool
  --> main.ox:1:39
"#,
            r#"{"schema_version":1,"edition":"typed-preview","kind":"diagnostic","severity":"error","code":"E0300","stage":"type","message":"type mismatch: expected i32, found bool","primary":{"file_id":0,"path":"main.ox","start":38,"end":42,"line":1,"column":39,"end_line":1,"end_column":43},"secondary":[],"notes":[]}
"#,
        ),
        (
            r#"empty-scalar-context"#,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-scalar-context/main.ox")),
            r#"error[E0300] (type): empty array literal requires an explicit array annotation on its local initializer
  --> main.ox:1:34
"#,
            r#"{"schema_version":1,"edition":"typed-preview","kind":"diagnostic","severity":"error","code":"E0300","stage":"type","message":"empty array literal requires an explicit array annotation on its local initializer","primary":{"file_id":0,"path":"main.ox","start":33,"end":35,"line":1,"column":34,"end_line":1,"end_column":36},"secondary":[],"notes":[]}
"#,
        ),
        (
            r#"empty-call-context-excluded"#,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-call-context-excluded/main.ox")),
            r#"error[E0300] (type): empty array literal requires an explicit array annotation on its local initializer
  --> main.ox:2:32
"#,
            r#"{"schema_version":1,"edition":"typed-preview","kind":"diagnostic","severity":"error","code":"E0300","stage":"type","message":"empty array literal requires an explicit array annotation on its local initializer","primary":{"file_id":0,"path":"main.ox","start":73,"end":75,"line":2,"column":32,"end_line":2,"end_column":34},"secondary":[],"notes":[]}
"#,
        ),
        (
            r#"empty-return-context-excluded"#,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-return-context-excluded/main.ox")),
            r#"error[E0300] (type): empty array literal requires an explicit array annotation on its local initializer
  --> main.ox:1:33
"#,
            r#"{"schema_version":1,"edition":"typed-preview","kind":"diagnostic","severity":"error","code":"E0300","stage":"type","message":"empty array literal requires an explicit array annotation on its local initializer","primary":{"file_id":0,"path":"main.ox","start":32,"end":34,"line":1,"column":33,"end_line":1,"end_column":35},"secondary":[],"notes":[]}
"#,
        ),
        (
            r#"empty-reassignment-context-excluded"#,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-reassignment-context-excluded/main.ox")),
            r#"error[E0300] (type): empty array literal requires an explicit array annotation on its local initializer
  --> main.ox:1:50
"#,
            r#"{"schema_version":1,"edition":"typed-preview","kind":"diagnostic","severity":"error","code":"E0300","stage":"type","message":"empty array literal requires an explicit array annotation on its local initializer","primary":{"file_id":0,"path":"main.ox","start":49,"end":51,"line":1,"column":50,"end_line":1,"end_column":52},"secondary":[],"notes":[]}
"#,
        ),
        (
            r#"literal-first-heterogeneous-element"#,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/literal-first-heterogeneous-element/main.ox")),
            r#"error[E0300] (type): type mismatch: expected i32, found bool
  --> main.ox:1:32
"#,
            r#"{"schema_version":1,"edition":"typed-preview","kind":"diagnostic","severity":"error","code":"E0300","stage":"type","message":"type mismatch: expected i32, found bool","primary":{"file_id":0,"path":"main.ox","start":31,"end":35,"line":1,"column":32,"end_line":1,"end_column":36},"secondary":[],"notes":[]}
"#,
        ),
        (
            r#"literal-nested-nonempty-is-nonscalar"#,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/literal-nested-nonempty-is-nonscalar/main.ox")),
            r#"error[E0300] (type): array elements must have scalar bool, i32 or () type
  --> main.ox:1:29
"#,
            r#"{"schema_version":1,"edition":"typed-preview","kind":"diagnostic","severity":"error","code":"E0300","stage":"type","message":"array elements must have scalar bool, i32 or () type","primary":{"file_id":0,"path":"main.ox","start":28,"end":31,"line":1,"column":29,"end_line":1,"end_column":32},"secondary":[],"notes":[]}
"#,
        ),
    ];
    assert_eq!(cases.len(), 29);
    for (name, text, human, json) in cases {
        let (sources, ast) = parsed(text);
        let work = WorkMeter::default();
        let mut allocator = Allocator::default();
        let result = resolve_types(&sources, &ast, &work, &mut allocator).and_then(typeck::check);
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1, "{name}");
        assert_eq!(errors[0].render_human(&sources), human, "{name}");
        assert_eq!(
            format!("{}\n", errors[0].render_json(&sources)),
            json,
            "{name}"
        );
        assert!(!work.observing());
        assert!(work.events.borrow().is_empty());
        assert!(work.observations.borrow().is_empty());
    }
}

const GROUPED: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/grouped-complete-access-and-index/main.ox"));
#[test]
fn unit3b1_grouped_store_has_rhs_first_and_no_target_read() {
    let (sources, ast) = parsed(GROUPED);
    assert_eq!(ast.expressions.len(), 11);
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let resolved = resolve_types(&sources, &ast, &work, &mut allocator).unwrap();
    let f = &resolved.functions()[0];
    assert_eq!(f.expressions.len(), 10);
    let spellings: Vec<_> = f
        .expressions
        .iter()
        .map(|e| resolved.text(e.span))
        .collect();
    assert_eq!(
        spellings,
        [
            "5",
            "7",
            "[5, 7]",
            "1",
            "a[1]",
            "(a[1])",
            "0",
            "(0)",
            "a.len()",
            "(a.len())"
        ]
    );
    let StmtKind::IndexAssign {
        value,
        index,
        base,
        target_span,
        base_span,
        operator_span,
    } = f.blocks[0].body[1].kind
    else {
        panic!("store");
    };
    assert_eq!((value, index, base), (ExprId(5), ExprId(7), BindingId(0)));
    assert_eq!(resolved.text(target_span), "a[(0)]");
    assert_eq!(resolved.text(base_span), "a");
    assert_eq!(resolved.text(operator_span), "=");
    let typed = typeck::check(resolved).unwrap();
    let view = typed.functions().next().unwrap();
    for id in [0, 1, 3, 4, 5, 6, 7, 8, 9] {
        assert_eq!(view.expression_ty(ExprId(id)), ValueTy::Scalar(Ty::I32));
    }
    assert_eq!(
        view.expression_ty(ExprId(2)),
        ValueTy::Owned(AggregateTy::FixedArray(
            FixedArrayTy::check(Ty::I32, 2).unwrap()
        ))
    );
    let trace: Vec<_> = allocator
        .trace
        .iter()
        .filter(|e| e.kind == "array HIR elements")
        .collect();
    assert_eq!(trace.len(), 1);
    assert_eq!(
        (trace[0].length, trace[0].element_bytes, trace[0].success),
        (2, 8, true)
    );
}

#[test]
fn unit3b1_zero_and_one_scalar_types_fill_complete_group_slots() {
    let cases = [
        (
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-bool-length-0/main.ox")),
            Ty::Bool,
            0,
        ),
        (
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-bool-length-1/main.ox")),
            Ty::Bool,
            1,
        ),
        (
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-i32-length-0/main.ox")),
            Ty::I32,
            0,
        ),
        (
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-i32-length-1/main.ox")),
            Ty::I32,
            1,
        ),
        (
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-unit-length-0/main.ox")),
            Ty::Unit,
            0,
        ),
        (
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-unit-length-1/main.ox")),
            Ty::Unit,
            1,
        ),
    ];
    for (text, element, length) in cases {
        let (sources, ast) = parsed(text);
        let work = WorkMeter::default();
        let mut allocator = Allocator::default();
        let typed =
            typeck::check(resolve_types(&sources, &ast, &work, &mut allocator).unwrap()).unwrap();
        let view = typed.functions().next().unwrap();
        let expected = ValueTy::Owned(AggregateTy::FixedArray(
            FixedArrayTy::check(element, length).unwrap(),
        ));
        assert_eq!(view.binding_ty(BindingId(0)), ParameterTy::Value(expected));
        for (id, expr) in view.hir().expressions.iter().enumerate() {
            if matches!(
                expr.kind,
                ExprKind::ArrayLiteral { .. } | ExprKind::Group(_)
            ) {
                assert_eq!(view.expression_ty(ExprId(id)), expected);
            }
        }
        let literal = allocator
            .trace
            .iter()
            .find(|e| e.kind == "array HIR elements")
            .unwrap();
        assert_eq!(literal.length, length);
        assert!(literal.success);
    }
}

#[test]
fn unit3b1_exact_literal_reservations_fail_at_real_fallible_requests() {
    let cases = [
        GROUPED,
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-unit-length-0/main.ox")),
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/literal-nested-nonempty-is-nonscalar/main.ox")),
    ];
    for text in cases {
        let (sources, ast) = parsed(text);
        let work = WorkMeter::default();
        let mut baseline = Allocator::default();
        let resolved = resolve_types(&sources, &ast, &work, &mut baseline).unwrap();
        let literal_spans: Vec<_> = ast
            .expressions
            .iter()
            .filter_map(|expr| {
                matches!(expr.kind, ast::ExprKind::ArrayLiteral { .. }).then_some(expr.span)
            })
            .collect();
        // Reservation order is before children, unlike append-time HIR IDs.
        let mut ordered_spans = literal_spans;
        ordered_spans.sort_by_key(|span| span.start);
        let attempts: Vec<_> = baseline
            .trace
            .iter()
            .enumerate()
            .filter(|(_, e)| e.kind == "array HIR elements")
            .map(|(i, e)| (i + 1, e.length))
            .collect();
        assert_eq!(attempts.len(), ordered_spans.len());
        drop(resolved);
        for ((attempt, length), span) in attempts.into_iter().zip(ordered_spans) {
            let work = WorkMeter::default();
            let mut allocator = Allocator {
                fail_at: Some(attempt),
                ..Allocator::default()
            };
            let errors = resolve_types(&sources, &ast, &work, &mut allocator).unwrap_err();
            let e = &errors[0];
            assert_eq!(
                (e.code, e.stage, e.message.as_str(), e.primary),
                (
                    "E0400",
                    "resolve",
                    "array HIR allocation failed",
                    Some(span)
                )
            );
            assert!(e.notes.is_empty() && e.secondary.is_empty());
            let event = allocator.trace.last().unwrap();
            assert_eq!(
                (event.kind, event.length, event.element_bytes, event.success),
                (
                    "array HIR elements",
                    length,
                    std::mem::size_of::<ExprId>(),
                    false
                )
            );
        }
    }
}

#[test]
fn unit3b1_added_work_and_inclusive_limit_share_the_existing_meter() {
    let (sources, ast) = parsed(GROUPED);
    let measured = WorkMeter::default();
    // Exactly this fixed 86-byte fixture bounds passive log retention.
    measured.enable_observation();
    let typed =
        typeck::check(resolve_types(&sources, &ast, &measured, &mut Allocator::default()).unwrap())
            .unwrap();
    assert_eq!(typed.functions().len(), 1);
    let added: u64 = measured
        .events
        .borrow()
        .iter()
        .filter(|e| e.operation.starts_with("array "))
        .map(|e| e.units)
        .sum();
    assert_eq!(added, 16);
    println!(
        "unit3b1 grouped work total={} added={added}",
        measured.used()
    );
    const PREDECESSOR_GROUPED_WORK: u64 = 247;
    // One module and one original function: two module phases + three row phases.
    const RFC0030_RESERVATION_SCAN: u64 = 2 + 3;
    assert_eq!(
        measured.used(),
        PREDECESSOR_GROUPED_WORK + RFC0030_RESERVATION_SCAN
    );
    let limited = WorkMeter::new(247);
    let errors = resolve_types(&sources, &ast, &limited, &mut Allocator::default()).unwrap_err();
    assert_eq!(
        (errors[0].code, errors[0].stage, errors[0].message.as_str()),
        (
            "E0400",
            "resolve-project",
            "declaration index mandatory build work limit exceeded"
        )
    );
    assert_eq!(limited.used(), 196);
    println!("CONTROL {{\"schema\":\"oxid-array-types-work-rfc0030\",\"kind\":\"work\",\"reservation_scan_delta\":5,\"seam\":\"private-owned-without-selector\",\"case_id\":\"grouped-complete-access-and-index\",\"limit\":247,\"used\":{},\"added_units\":0,\"success\":false,\"predecessor_ledger_sha256\":\"65258c079d5e0131685170fc37e2ee1997f913613c2c8cf57f9f6dac5ee34592\",\"diagnostic\":{},\"human\":{},\"json_line\":{}}}", limited.used(), errors[0].render_json(&sources), crate::frontend::diagnostic::json_string(&errors[0].render_human(&sources)), crate::frontend::diagnostic::json_string(&format!("{}\n", errors[0].render_json(&sources))));

    // A small source's actual total may be below the unchanged conservative
    // index-build admission estimate. Use the frozen wide source for boundary
    // calibration, and independently check its source-derived added total.
    let (sources, ast) = parsed(WIDE);
    let measured = WorkMeter::default();
    measured.enable_observation();
    let typed =
        typeck::check(resolve_types(&sources, &ast, &measured, &mut Allocator::default()).unwrap())
            .unwrap();
    assert_eq!(typed.functions().len(), 1);
    let added: u64 = measured
        .events
        .borrow()
        .iter()
        .filter(|e| e.operation.starts_with("array "))
        .map(|e| e.units)
        .sum();
    assert_eq!(added, 2053);
    // Independently source-derived before this boundary assertion: ledger
    // SHA256 65258c079d5e0131685170fc37e2ee1997f913613c2c8cf57f9f6dac5ee34592.
    // This helper does not invoke the separately charged whole-project selector.
    const PREDECESSOR_WIDE_WORK: u64 = 10_369;
    let exact = PREDECESSOR_WIDE_WORK + RFC0030_RESERVATION_SCAN;
    assert_eq!(measured.used(), exact);
    for (limit, succeeds) in [(exact, true), (exact - 1, false)] {
        let work = WorkMeter::new(limit);
        work.enable_observation(); // The same fixed 2,097-byte control bounds retention.
        let result =
            resolve_types(&sources, &ast, &work, &mut Allocator::default()).and_then(typeck::check);
        assert_eq!(result.is_ok(), succeeds);
        let success = result.is_ok();
        let observed_added: u64 = work
            .events
            .borrow()
            .iter()
            .filter(|e| e.operation.starts_with("array "))
            .map(|e| e.units)
            .sum();
        let rendered = match &result {
            Ok(_) => "\"diagnostic\":null,\"human\":null,\"json_line\":null".into(),
            Err(errors) => format!(
                "\"diagnostic\":{},\"human\":{},\"json_line\":{}",
                errors[0].render_json(&sources),
                crate::frontend::diagnostic::json_string(&errors[0].render_human(&sources)),
                crate::frontend::diagnostic::json_string(&format!(
                    "{}\n",
                    errors[0].render_json(&sources)
                ))
            ),
        };
        println!("CONTROL {{\"schema\":\"oxid-array-types-work-rfc0030\",\"kind\":\"work\",\"reservation_scan_delta\":5,\"seam\":\"private-owned-without-selector\",\"case_id\":\"literal-length-max-trailing-comma\",\"limit\":{limit},\"used\":{},\"added_units\":{observed_added},\"success\":{success},\"predecessor_ledger_sha256\":\"65258c079d5e0131685170fc37e2ee1997f913613c2c8cf57f9f6dac5ee34592\",{rendered}}}", work.used());
        if let Err(errors) = result {
            assert_eq!(
                (errors[0].code, errors[0].stage),
                ("E0400", "resolve-project")
            );
            assert_eq!(errors[0].message, "declaration index work limit exceeded");
            let file = sources.get(crate::frontend::source::SourceFileId(0));
            assert_eq!(file.text_at(errors[0].primary.unwrap()), "a.len()");
        }
        assert_eq!(work.used(), limit);
    }
    println!("unit3b1 wide work boundary calibration total={exact} added={added}");
}

fn assert_fence(error: &Diagnostic, expected: crate::frontend::source::Span) {
    assert_eq!(
        (
            error.code,
            error.stage,
            error.message.as_str(),
            error.primary
        ),
        (
            "E0500",
            "oir-owned-lower",
            "internal compiler error: owned invariant violation",
            Some(expected)
        )
    );
    assert!(error.notes.is_empty() && error.secondary.is_empty());
}
fn guard_typed(typed: &typeck::TypedOwnedProgram<'_>, sources: &SourceMap) {
    let eof = typed.index().sources().eof();
    budget::reset_guard_counts();
    budget::fail_allocation_after(0, || {
        let errors = program::check_typed(typed).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_fence(&errors[0], eof);
    });
    assert_eq!(budget::guard_counts(), [0; 7]);
    for preflight in [true, false] {
        budget::reset_guard_counts();
        let error = budget::fail_allocation_after(0, || {
            if preflight {
                budget::preflight(typed, budget::Limits::DEFAULT).unwrap_err()
            } else {
                lower::lower(typed).unwrap_err()
            }
        });
        assert_fence(&diagnostic::lower(&error, sources), eof);
        assert_eq!(budget::guard_counts(), [0; 7]);
    }
    for view in typed.functions() {
        for per_block in [false, true] {
            let mut blocks = [usize::MAX; 3];
            budget::reset_guard_counts();
            let error = budget::fail_allocation_after(0, || {
                lower::count_function(&view, per_block.then_some(&mut blocks[..])).unwrap_err()
            });
            assert_fence(&diagnostic::lower(&error, sources), view.signature().span);
            assert_eq!(blocks, [usize::MAX; 3]);
            assert_eq!(budget::guard_counts(), [0; 7]);
        }
        budget::reset_guard_counts();
        let error = budget::fail_allocation_after(0, || {
            lower::check_array_type_emission_fence(&view).unwrap_err()
        });
        assert_fence(&diagnostic::lower(&error, sources), view.signature().span);
        assert_eq!(budget::guard_counts(), [0; 7]);
    }
}
#[test]
fn unit3b1_admission_fences_precede_all_lower_work_even_without_arrays_or_functions() {
    // Explicit private-owned seam; these need not select owned in production.
    let cases = [
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/fixtures/guard-empty/main.ox")),
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/fixtures/guard-record-only/main.ox")),
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/fixtures/guard-array-free/main.ox")),
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/structural-identities-pairwise-distinct/main.ox")),
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/fixtures/reference-access-modes/main.ox")),
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-unit-length-0/main.ox")),
    ];
    for text in cases {
        let (sources, ast) = parsed(text);
        let work = WorkMeter::default();
        let typed =
            typeck::check(resolve_types(&sources, &ast, &work, &mut Allocator::default()).unwrap())
                .unwrap();
        guard_typed(&typed, &sources);
        // This view forces the guard before SourceView::Map discrimination.
        let source = sources.get(crate::frontend::source::SourceFileId(0));
        let single = SourceOwner::original(source, &ast, SourceView::Single(source)).unwrap();
        let typed = typeck::check(
            resolve::resolve_array_types(
                single,
                IndexLimits::default(),
                &work,
                &mut Allocator::default(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_fence(
            &program::check_typed(&typed).unwrap_err()[0],
            typed.index().sources().eof(),
        );
    }
}
#[test]
fn unit3b1_reference_access_retains_modes_without_ownership_qualification() {
    let (sources, ast) = parsed(
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/fixtures/reference-access-modes/main.ox")),
    );
    let work = WorkMeter::default();
    let typed =
        typeck::check(resolve_types(&sources, &ast, &work, &mut Allocator::default()).unwrap())
            .unwrap();
    let array = AggregateTy::FixedArray(FixedArrayTy::check(Ty::I32, 1).unwrap());
    for (view, kind) in typed
        .functions()
        .take(2)
        .zip([BorrowKind::Shared, BorrowKind::Exclusive])
    {
        assert_eq!(
            view.binding_ty(BindingId(0)),
            ParameterTy::Reference {
                referent: BorrowedTy::Exact(array),
                kind
            }
        );
        for id in 0..view.hir().expressions.len() {
            assert_eq!(view.expression_ty(ExprId(id)), ValueTy::Scalar(Ty::I32));
        }
    }
    guard_typed(&typed, &sources);
}
#[test]
fn unit3b1_wide_literal_retains_exact_scalar_slots_without_deep_recursion() {
    let (sources, ast) = parsed(
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/literal-length-max-trailing-comma/main.ox")),
    );
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let typed =
        typeck::check(resolve_types(&sources, &ast, &work, &mut allocator).unwrap()).unwrap();
    let view = typed.functions().next().unwrap();
    let (id, elements) = view
        .hir()
        .expressions
        .iter()
        .enumerate()
        .find_map(|(id, e)| match &e.kind {
            ExprKind::ArrayLiteral { elements } => Some((id, elements)),
            _ => None,
        })
        .unwrap();
    assert_eq!(elements.len(), 1024);
    assert_eq!(
        view.expression_ty(ExprId(id)),
        ValueTy::Owned(AggregateTy::FixedArray(
            FixedArrayTy::check(Ty::I32, 1024).unwrap()
        ))
    );
    for element in elements {
        assert_eq!(view.expression_ty(*element), ValueTy::Scalar(Ty::I32));
    }
    let events: Vec<_> = allocator
        .trace
        .iter()
        .filter(|e| e.kind == "array HIR elements")
        .collect();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].length, 1024);
}

const WIDE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/literal-length-max-trailing-comma/main.ox"));

#[cfg(target_os = "linux")]
#[test]
fn unit3b1_project_root_fences_and_cumulative_literal_inventory() {
    use crate::frontend::project::{ProjectLimits, ProjectSources};
    for (name, expected_elements) in [
        ("guard-project-root-eof", 0),
        ("reserve-across-modules", 3),
        ("cross-file-array-identity", 1),
    ] {
        let authority = if name == "cross-file-array-identity" {
            "contracts-v2"
        } else {
            "typing-contracts-v1"
        };
        let path = format!(
            "{}/tests/fixtures/fixed_array_source_unit3/{authority}/fixtures/{name}/main.ox",
            env!("CARGO_MANIFEST_DIR")
        );
        let project = ProjectSources::load_array_candidate(
            &path,
            ProjectLimits::default(),
            &mut Allocator::default(),
        )
        .unwrap();
        let owner = SourceOwner::project(&project);
        let work = WorkMeter::default();
        let mut allocator = Allocator::default();
        let resolved =
            resolve::resolve_array_types(owner, IndexLimits::default(), &work, &mut allocator)
                .unwrap();
        let elements: usize = resolved
            .functions()
            .iter()
            .flat_map(|f| &f.expressions)
            .map(|e| match &e.kind {
                ExprKind::ArrayLiteral { elements } => elements.len(),
                _ => 0,
            })
            .sum();
        assert_eq!(elements, expected_elements, "{name}");
        let requested: usize = allocator
            .trace
            .iter()
            .filter(|e| e.kind == "array HIR elements")
            .map(|e| e.length)
            .sum();
        assert_eq!(requested, elements, "{name}");
        if name == "reserve-across-modules" {
            let lengths: Vec<_> = allocator
                .trace
                .iter()
                .filter(|e| e.kind == "array HIR elements")
                .map(|e| e.length)
                .collect();
            assert_eq!(lengths, [1, 2]);
        }
        let typed = typeck::check(resolved).unwrap();
        assert_eq!(typed.index().sources().eof().file.0, 0);
        guard_typed(&typed, project.sources());
    }
}
