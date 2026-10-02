use super::*;
use crate::frontend::{
    oir::project::{check_project_candidate, ProjectRoute},
    project::ProjectLimits,
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering as AtomicOrdering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new(files: &[(&str, &str)]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-unit2-index-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, AtomicOrdering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        for (name, text) in files {
            let path = root.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        Self(root)
    }
    fn load(&self) -> ProjectSources {
        ProjectSources::load_project_candidate(
            self.0.join("main.ox").to_str().unwrap(),
            ProjectLimits::default(),
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
type Summary = (ProjectRoute, usize, usize, Option<DefId>);
fn run(files: &[(&str, &str)]) -> (Result<Summary, Vec<Diagnostic>>, WorkMeter) {
    let fixture = Fixture::new(files);
    let sources = fixture.load();
    let work = WorkMeter::default();
    work.enable_observation();
    let result = check_project_candidate(
        &sources,
        IndexLimits::default(),
        &work,
        &mut Allocator::default(),
    )
    .map(|p| (p.route(), p.functions(), p.records(), p.root_main()));
    (result, work)
}

#[test]
fn scalar_project_uses_real_global_calls_and_root_original_entry() {
    let(result,work)=run(&[("main.ox","mod a; use crate::a::f as g; fn helper() -> i32 { return g(); } fn main() -> i32 { return helper(); }"),("a.ox","pub fn f() -> i32 { return 7; }")]);
    assert_eq!(
        result.unwrap(),
        (ProjectRoute::Scalar, 3, 0, Some(DefId(1)))
    );
    assert!(work.observations.borrow().iter().any(|e| matches!(
        e,
        Observation::Target {
            operation: "callee",
            id: 2,
            ..
        }
    )));
}
#[test]
fn dense_per_kind_ids_precede_child_recursion() {
    let fixture = Fixture::new(&[
        (
            "main.ox",
            "fn r0()->(){return;} mod a; fn r1()->(){return;} mod b;",
        ),
        ("a.ox", "fn a0()->(){return;} mod c; fn a1()->(){return;}"),
        ("a/c.ox", "fn c0()->(){return;}"),
        ("b.ox", "fn b0()->(){return;}"),
    ]);
    let sources = fixture.load();
    let work = WorkMeter::default();
    let mut alloc = Allocator::default();
    let facts = collect_originals(
        SourceOwner::project(&sources),
        IndexLimits::default(),
        &work,
        &mut alloc,
    )
    .unwrap();
    let index = facts.finish(&work, &mut alloc).unwrap();
    for (id, (file, local)) in [(0, 0), (0, 1), (1, 0), (1, 1), (2, 0), (3, 0)]
        .into_iter()
        .enumerate()
    {
        let (key, _) = index.function(DefId(id)).unwrap();
        assert_eq!((key.file.0, key.index), (file, local));
        assert_eq!(index.def_for(key).unwrap(), DefId(id));
    }
}
#[test]
fn paired_import_failure_rolls_back_both_target_lanes() {
    let (result, work) = run(&[
        (
            "main.ox",
            "mod a; use crate::a::X as bool; use crate::a::X as Y; fn main()->i32{return Y();}",
        ),
        ("a.ox", "pub struct X {} pub fn X()->i32{return 1;}"),
    ]);
    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, "E0202");
    let events = work.observations.borrow();
    let imports: Vec<_> = events
        .iter()
        .filter_map(|e| {
            if let Observation::Import {
                committed,
                ty,
                value,
                ..
            } = e
            {
                Some((*committed, *ty, *value))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        imports,
        vec![
            (false, None, None),
            (true, Some(RecordId(0)), Some(DefId(1)))
        ]
    );
    assert!(!events
        .iter()
        .any(|e| matches!(e, Observation::Frozen { .. })));
}
#[test]
fn owned_inferred_read_and_rhs_first_write_use_requester_permission() {
    let child = "pub struct C { n:i32 } pub fn make()->C{return C{n:1};}";
    for (body, code, stage) in [
        ("let c=crate::a::make(); return c.n;", "E0206", "type"),
        (
            "let mut c=crate::a::make(); c.n=true+1; return 0;",
            "E0300",
            "type",
        ),
        (
            "let mut c=crate::a::make(); c.n=true; return 0;",
            "E0206",
            "type",
        ),
    ] {
        let root = format!("mod a; fn main()->i32{{{body}}}");
        let (result, _) = run(&[("main.ox", &root), ("a.ox", child)]);
        let error = &result.unwrap_err()[0];
        assert_eq!((error.code, error.stage), (code, stage));
    }
}
#[test]
fn construction_privacy_precedes_initializer_name_and_value_resolution() {
    let (result, _) = run(&[
        (
            "main.ox",
            "mod a; fn main()->(){let c=crate::a::C{wrong:missing};return;}",
        ),
        ("a.ox", "pub struct C{n:i32}"),
    ]);
    let errors = result.unwrap_err();
    assert_eq!((errors[0].code, errors[0].stage), ("E0206", "resolve"));
    assert_eq!(errors[0].secondary[0].0.file, SourceFileId(1));
}
#[test]
fn signature_exposure_includes_external_and_canonical_names_preserve_identity() {
    let (result, _) = run(&[("main.ox", "struct C{} pub fn make()->C{return C{};}")]);
    assert_eq!(result.unwrap_err()[0].code, "E0207");
    let (result, _) = run(&[
        (
            "main.ox",
            "mod l; mod r; fn main()->(){let c:crate::l::C=crate::r::make();return;}",
        ),
        ("l.ox", "pub struct C{}"),
        ("r.ox", "pub struct C{} pub fn make()->C{return C{};}"),
    ]);
    assert_eq!(
        result.unwrap_err()[0].message,
        "type mismatch: expected crate::l::C, found crate::r::C"
    );
}
#[test]
fn wrong_owner_and_stale_source_are_internal_failures() {
    let mut map = super::super::source::SourceMap::new();
    let a = map.add("a".into(), "fn a()->(){return;}".into());
    let b = map.add("b".into(), "fn b()->(){return;}".into());
    let ast =
        super::super::parser::parse(map.get(a), super::super::lexer::lex(map.get(a)).unwrap())
            .unwrap();
    assert_eq!(
        SourceOwner::original(map.get(b), &ast, SourceView::Map(&map))
            .unwrap_err()
            .code,
        "E0500"
    );
    let fixture = Fixture::new(&[
        ("main.ox", "mod a; fn main()->(){return;}"),
        ("a.ox", "pub fn f()->(){return;}"),
    ]);
    let p = fixture.load();
    let work = WorkMeter::default();
    let mut alloc = Allocator::default();
    let index = collect_originals(
        SourceOwner::project(&p),
        IndexLimits::default(),
        &work,
        &mut alloc,
    )
    .unwrap()
    .finish(&work, &mut alloc)
    .unwrap();
    let span = p.try_file_ast(SourceFileId(1)).unwrap().functions[0].name;
    assert_eq!(
        index
            .query(&work)
            .callee(
                ModuleId(0),
                ItemPathRef {
                    file: SourceFileId(1),
                    path: ast::ItemPath::Unqualified(span)
                },
                false
            )
            .unwrap_err()
            .code,
        "E0500"
    );
}
#[test]
fn exact_space_caps_and_real_reserve_failures_precede_allocated_index() {
    let fixture = Fixture::new(&[("main.ox", "fn main()->(){return;}")]);
    let p = fixture.load();
    let work = WorkMeter::default();
    let mut alloc = Allocator::default();
    let facts = collect_originals(
        SourceOwner::project(&p),
        IndexLimits::default(),
        &work,
        &mut alloc,
    )
    .unwrap();
    let plan = facts.plan();
    let attempts = alloc.attempts;
    assert_eq!(attempts, 14);
    for (limits, ok) in [
        (
            IndexLimits {
                retained: plan.retained,
                scratch: plan.scratch,
                ..IndexLimits::default()
            },
            true,
        ),
        (
            IndexLimits {
                retained: plan.retained - 1,
                ..IndexLimits::default()
            },
            false,
        ),
        (
            IndexLimits {
                scratch: plan.scratch - 1,
                ..IndexLimits::default()
            },
            false,
        ),
    ] {
        let mut allocator = Allocator::default();
        let result = collect_originals(
            SourceOwner::project(&p),
            limits,
            &WorkMeter::default(),
            &mut allocator,
        );
        assert_eq!(result.is_ok(), ok);
        if !ok {
            assert_eq!(allocator.attempts, 0);
        }
    }
    for failure in 1..=attempts {
        let mut allocator = Allocator {
            fail_at: Some(failure),
            ..Allocator::default()
        };
        let error = collect_originals(
            SourceOwner::project(&p),
            IndexLimits::default(),
            &WorkMeter::default(),
            &mut allocator,
        )
        .unwrap_err();
        assert_eq!(error.code, "E0400");
        assert_eq!(allocator.attempts, failure);
        assert!(!allocator.trace.last().unwrap().success);
    }
    let error = IndexPlan::calculate(
        Counts {
            modules: 1,
            originals: u64::MAX,
            ..Counts::default()
        },
        size_of::<DeclarationIndex<'_>>(),
        FIXED_SCRATCH,
        IndexLimits {
            retained: 0,
            scratch: 0,
            work: 0,
        },
        p.sources().get(SourceFileId(0)).span(0, 0),
    )
    .unwrap_err();
    assert_eq!(error.message, "declaration index count overflow");
}

#[test]
fn measured_flat_rows_and_fixed_state() {
    println!("OriginalRow={} FunctionRow={} RecordRow={} FieldRow={} ModuleRow={} ImportRow={} AliasCell={} SeenCell={} Index={} Facts={} Plan={} Counts={} Scratch={} Fixed={}",size_of::<OriginalRow>(),size_of::<FunctionRow>(),size_of::<RecordRow>(),size_of::<FieldRow>(),size_of::<ModuleRow>(),size_of::<ImportRow>(),size_of::<AliasCell>(),size_of::<SeenCell>(),size_of::<DeclarationIndex<'_>>(),size_of::<DeclarationFacts<'_>>(),size_of::<IndexPlan>(),size_of::<Counts>(),size_of::<Scratch>(),FIXED_SCRATCH);
    const { assert!(FIXED_SCRATCH <= 4096) };
}

#[test]
fn access_work_exhaustion_uses_each_actual_request_site() {
    let fixture = Fixture::new(&[
        (
            "main.ox",
            "mod a; mod b; fn root(c:crate::a::C)->i32{return c.n;}",
        ),
        ("a.ox", "pub struct C{n:i32}"),
        (
            "b.ox",
            "fn one(c:crate::a::C)->i32{return c.n;} fn two(c:crate::a::C)->i32{return c.n;}",
        ),
    ]);
    let project = fixture.load();
    let sites: Vec<_> = [0, 2]
        .into_iter()
        .flat_map(|file| {
            project
                .try_file_ast(SourceFileId(file))
                .unwrap()
                .expressions
                .iter()
                .filter_map(move |expr| {
                    if let ast::ExprKind::FieldRead { field, .. } = expr.kind {
                        Some((ModuleId(file), field))
                    } else {
                        None
                    }
                })
        })
        .collect();
    assert_eq!(sites.len(), 3);
    for (requester, site) in sites {
        let work = WorkMeter::default();
        let mut allocator = Allocator::default();
        let frozen = collect_originals(
            SourceOwner::project(&project),
            IndexLimits::default(),
            &work,
            &mut allocator,
        )
        .unwrap()
        .finish(&work, &mut allocator)
        .unwrap();
        work.restrict(work.used());
        let error = frozen
            .query(&work)
            .field_access(
                requester,
                FieldId {
                    record: RecordId(0),
                    index: 0,
                },
                site,
            )
            .unwrap_err();
        assert_eq!(
            (error.code, error.stage, error.primary),
            ("E0400", "resolve-project", Some(site))
        );
    }
}

#[test]
fn exact_work_query_cap_and_one_less_have_distinct_results() {
    let fixture = Fixture::new(&[(
        "main.ox",
        "fn main()->i32{return helper();} fn helper()->i32{return 1;}",
    )]);
    let sources = fixture.load();
    let at = sources.try_file_ast(SourceFileId(0)).unwrap().functions[1].name;
    let measure = WorkMeter::default();
    let mut allocator = Allocator::default();
    let index = collect_originals(
        SourceOwner::project(&sources),
        IndexLimits::default(),
        &measure,
        &mut allocator,
    )
    .unwrap()
    .finish(&measure, &mut allocator)
    .unwrap();
    let before = measure.used();
    assert_eq!(
        index
            .query(&measure)
            .callee(
                ModuleId(0),
                ItemPathRef {
                    file: at.file,
                    path: ast::ItemPath::Unqualified(at)
                },
                true
            )
            .unwrap(),
        DefId(1)
    );
    let cost = measure.used() - before;
    for (excess, ok) in [(0, true), (1, false)] {
        let work = WorkMeter::default();
        let mut allocator = Allocator::default();
        let index = collect_originals(
            SourceOwner::project(&sources),
            IndexLimits::default(),
            &work,
            &mut allocator,
        )
        .unwrap()
        .finish(&work, &mut allocator)
        .unwrap();
        work.restrict(work.used() + cost - excess);
        let result = index.query(&work).callee(
            ModuleId(0),
            ItemPathRef {
                file: at.file,
                path: ast::ItemPath::Unqualified(at),
            },
            true,
        );
        assert_eq!(result.is_ok(), ok);
        if let Err(error) = result {
            assert_eq!((error.code, error.primary), ("E0400", Some(at)));
        }
    }
}

#[test]
fn builtin_type_shortcut_rejects_cross_file_name_origins() {
    let fixture = Fixture::new(&[
        ("main.ox", "mod a; fn main()->(){return;}"),
        ("a.ox", "pub fn f()->bool{return true;}"),
    ]);
    let project = fixture.load();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let index = collect_originals(
        SourceOwner::project(&project),
        IndexLimits::default(),
        &work,
        &mut allocator,
    )
    .unwrap()
    .finish(&work, &mut allocator)
    .unwrap();
    let root = project.try_file_ast(SourceFileId(0)).unwrap().functions[0].result;
    let foreign = project.try_file_ast(SourceFileId(1)).unwrap().functions[0].result;
    let error = index
        .query(&work)
        .value_type(
            ModuleId(0),
            ast::TypeSyntax {
                span: root.span,
                kind: foreign.kind,
            },
            TypeContext::Value,
        )
        .unwrap_err();
    assert_eq!(error.code, "E0500");
}

#[test]
fn passive_trace_distinguishes_lookup_from_completed_constructor_and_borrow() {
    let (result, work) = run(&[
        (
            "main.ox",
            "mod a; fn main()->(){let c=crate::a::C{n:missing};return;}",
        ),
        ("a.ox", "pub struct C{n:i32}"),
    ]);
    assert_eq!(result.unwrap_err()[0].code, "E0206");
    assert!(work.observations.borrow().iter().any(|event| matches!(
        event,
        Observation::Target {
            operation: "constructor-type",
            origin: Span {
                file: SourceFileId(0),
                ..
            },
            ..
        }
    )));
    assert!(!work.observations.borrow().iter().any(|event| matches!(
        event,
        Observation::Target {
            operation: "constructor-resolved",
            origin: Span {
                file: SourceFileId(0),
                ..
            },
            ..
        }
    )));
    let (result, work) = run(&[(
        "main.ox",
        "struct C{} fn take(x:&C)->(){return;} fn main()->(){let s=C{};take(&s);return;}",
    )]);
    result.unwrap();
    assert!(work.observations.borrow().iter().any(|event| matches!(
        event,
        Observation::BorrowArgument {
            function: DefId(1),
            binding: 0,
            ty: super::super::oir::owned_types::ParameterTy::Reference {
                record: RecordId(0),
                ..
            },
            ..
        }
    )));
}

#[test]
fn qualified_wrong_namespace_reports_only_the_terminal_name() {
    for (root, child, code) in [
        (
            "mod m; fn f(x:crate::m::X)->(){return;}",
            "pub fn X()->(){return;}",
            "E0202",
        ),
        (
            "mod m; fn main()->i32{return crate::m::X();}",
            "pub struct X{}",
            "E0200",
        ),
    ] {
        let (result, _) = run(&[("main.ox", root), ("m.ox", child)]);
        let errors = result.unwrap_err();
        let start = root.find('X').unwrap();
        assert_eq!(
            (errors[0].code, errors[0].primary),
            (
                code,
                Some(Span {
                    file: SourceFileId(0),
                    start,
                    end: start + 1
                })
            )
        );
    }
}

#[test]
fn reference_exposure_reports_nominal_referent_without_borrow_prefix() {
    for reference in ["&T", "&mut T"] {
        let child = format!("struct T{{}} pub fn f(x:{reference})->(){{return;}}");
        let (result, _) = run(&[("main.ox", "mod a;"), ("a.ox", &child)]);
        let errors = result.unwrap_err();
        let start = child.find(reference).unwrap() + reference.len() - 1;
        assert_eq!(
            (errors[0].code, errors[0].primary),
            (
                "E0207",
                Some(Span {
                    file: SourceFileId(1),
                    start,
                    end: start + 1
                })
            )
        );
    }
}
