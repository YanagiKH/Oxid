//! Candidate arrays use the production source owner and cumulative loader limits.
use super::*;
use crate::frontend::declaration_index::{SourceOwner, WorkMeter};
use std::sync::atomic::{AtomicUsize, Ordering};

static SERIAL: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "oxid-unit3a-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, path: &str, text: &str) {
        std::fs::write(self.0.join(path), text).unwrap();
    }
    #[allow(clippy::result_large_err)]
    fn load(&self, limits: ProjectLimits) -> Result<ProjectSources, LoadFailure> {
        ProjectSources::load_array_candidate(
            self.0.join("main.ox").to_str().unwrap(),
            limits,
            &mut Allocator::default(),
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn unit3a_actual_literal_elements_are_inventoried_with_target_wrappers() {
    let f = Fixture::new();
    f.write("main.ox", "fn f()->(){[1,2];a[0]=1;}");
    let p = f.load(ProjectLimits::default()).unwrap();
    let a = p.try_file_ast(SourceFileId(0)).unwrap();
    assert_eq!(a.expressions.len(), 6);
    assert_eq!(p.usage().syntax_nodes, 9);
    let expected = size_of::<ast::Function>()
        + size_of::<ast::BodyBlock>()
        + size_of::<ast::ItemId>()
        + 2 * size_of::<ast::Stmt>()
        + 6 * size_of::<ast::Expr>()
        + 2 * size_of::<ast::ExprId>();
    assert_eq!(p.inventory().unwrap().ast_payload, expected);
    assert!(expected <= 288 * p.usage().syntax_nodes);
    assert!(p.uses_owned_syntax());
    assert!(SourceOwner::project(&p)
        .owned(&WorkMeter::default())
        .unwrap());
}

#[test]
fn unit3a_both_selectors_cover_every_form_and_ignore_comment_text() {
    for text in [
        "fn unused(a:[bool;0])->(){}",
        "fn unused(a:&mut [i32;1])->(){}",
        "fn unused()->[();0]{}",
        "fn unused()->(){let a:[i32;1]=0;}",
        "fn unused()->(){[];}",
        "fn unused()->(){a[0];}",
        "fn unused()->(){a.len();}",
        "fn unused()->(){a[0]=1;}",
    ] {
        let f = Fixture::new();
        f.write("main.ox", text);
        let p = f.load(ProjectLimits::default()).unwrap();
        assert!(p.uses_owned_syntax(), "{text}");
        let meter = WorkMeter::default();
        assert!(SourceOwner::project(&p).owned(&meter).unwrap(), "{text}");
        assert!(meter.used() > 0);
        for program in &p.programs {
            assert!(program.validate_spans_and_ids(|span| p.try_text(span).is_some()));
        }
    }
    let f = Fixture::new();
    f.write(
        "main.ox",
        "// [1] a[0]=1; a.len();\nfn main()->i32{return 0;}",
    );
    let p = f.load(ProjectLimits::default()).unwrap();
    assert!(!p.uses_owned_syntax());
    assert!(!SourceOwner::project(&p)
        .owned(&WorkMeter::default())
        .unwrap());
}

#[cfg(target_os = "linux")]
#[test]
fn unit3a_child_only_syntax_keeps_cumulative_source_token_node_admission() {
    let f = Fixture::new();
    f.write("main.ox", "mod child; fn main()->i32{return 0;}");
    f.write("child.ox", "fn unused(a:[i32;1024])->(){}");
    let p = f.load(ProjectLimits::default()).unwrap();
    assert_eq!(p.usage().modules, 2);
    assert!(p.uses_owned_syntax());
    assert!(SourceOwner::project(&p)
        .owned(&WorkMeter::default())
        .unwrap());
    let usage = p.usage();
    for limits in [
        ProjectLimits {
            source_bytes: usage.source_bytes,
            ..ProjectLimits::default()
        },
        ProjectLimits {
            tokens: usage.non_eof_tokens,
            ..ProjectLimits::default()
        },
        ProjectLimits {
            nodes: usage.syntax_nodes,
            ..ProjectLimits::default()
        },
    ] {
        assert!(f.load(limits).is_ok());
    }
    for limits in [
        ProjectLimits {
            source_bytes: usage.source_bytes - 1,
            ..ProjectLimits::default()
        },
        ProjectLimits {
            tokens: usage.non_eof_tokens - 1,
            ..ProjectLimits::default()
        },
        ProjectLimits {
            nodes: usage.syntax_nodes - 1,
            ..ProjectLimits::default()
        },
    ] {
        let failure = f.load(limits).unwrap_err();
        assert_eq!(failure.diagnostics[0].code, "E0400");
    }
    let public = ProjectSources::load_typed(
        f.0.join("main.ox").to_str().unwrap(),
        ProjectLimits::default(),
    )
    .unwrap();
    assert!(public.uses_owned_syntax());
    assert_eq!(public.usage(), p.usage());
}

#[test]
fn unit3a_remeasure_enclosing_rows_and_disjoint_ast_envelope() {
    macro_rules! sizes { ($($ty:ty),* $(,)?) => { $(println!("layout {} {}", stringify!($ty), size_of::<$ty>());)* }; }
    sizes!(
        ast::Program,
        ast::Function,
        ast::Param,
        ast::TypeSyntax,
        ast::TypeSyntaxKind,
        ast::FixedArraySyntax,
        ast::ScalarTypeSyntax,
        Option<ast::TypeSyntax>,
        ast::Expr,
        ast::ExprKind,
        ast::ExprId,
        Vec<ast::ExprId>,
        Option<ast::Expr>,
        ast::Stmt,
        ast::StmtKind,
        ast::Argument,
        ast::BodyBlock,
        ProjectSources,
        SourceSetBuilder<'_>
    );
    #[cfg(target_pointer_width = "64")]
    {
        assert_eq!(
            (
                size_of::<ast::Expr>(),
                size_of::<ast::Stmt>(),
                size_of::<ast::TypeSyntax>()
            ),
            (88, 136, 64)
        );
        assert_eq!(
            (
                size_of::<ast::Param>(),
                size_of::<ast::Function>(),
                size_of::<ast::Program>()
            ),
            (88, 200, 272)
        );
        assert_eq!(
            size_of::<ast::Function>() + size_of::<ast::BodyBlock>() + size_of::<ast::ItemId>(),
            288
        );
    }
    for text in [
        "fn f()->(){}".repeat(100),
        format!("fn f()->(){{[{}];}}", "0,".repeat(1024)),
        "fn f()->(){a[0]=[1,2];if true {[1];} else {[];}}".into(),
        "fn f(a:[bool;0],b:&mut [();1024])->[i32;1]{}".into(),
        "fn f()->(){[g(1),g(2),a[0],a.len()];}".into(),
    ] {
        let f = Fixture::new();
        f.write("main.ox", &text);
        let p = f.load(ProjectLimits::default()).unwrap();
        assert!(p.inventory().unwrap().ast_payload <= 288 * p.usage().syntax_nodes);
        let mut visits = 0;
        assert!(p.programs[0].validate_spans_and_ids_counted(|span| {
            visits += 1;
            span.is_none_or(|at| p.try_text(at).is_some())
        }));
        assert!(visits <= 11 * p.usage().syntax_nodes + 2 * p.usage().non_eof_tokens + 4);
        println!(
            "resource nodes={} ast_bytes={} validation_visits={visits}",
            p.usage().syntax_nodes,
            p.inventory().unwrap().ast_payload
        );
    }
}
