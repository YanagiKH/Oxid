//! Acceptance through the typed loader and the same source/project facades as the CLI.
//! The historical loader below is used only for zero-enum compatibility inputs.
//! No private enum pipeline or candidate enum owner supplies acceptance evidence.
use super::{
    ast,
    declaration_index::{IndexLimits, Observation, WorkMeter},
    diagnostic::Diagnostic,
    format, hir, lexer,
    oir::{self, project::ProjectRoute, CheckedSourceProgram, Scalar},
    project::{budget::Allocator, ProjectLimits, ProjectSources, SyntaxFlavor},
    source::SourceFileId,
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new(files: &[(&str, &str)]) -> Self {
        let path = std::env::temp_dir().join(format!(
            "oxid-public-enums-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        for (name, text) in files {
            fs::write(path.join(name), text).unwrap();
        }
        Self(path)
    }
    fn entry(&self) -> String {
        self.0.join("main.ox").to_str().unwrap().to_owned()
    }
    fn load(&self) -> ProjectSources {
        ProjectSources::load_typed(&self.entry(), ProjectLimits::default()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

// This is the public driver's existing dispatch, not a second enum admission path.
fn checked(project: &ProjectSources) -> Result<CheckedSourceProgram<'_>, Vec<Diagnostic>> {
    match project.syntax_flavor() {
        SyntaxFlavor::OriginalSingleFile => {
            let (source, ast) = project.original_file().unwrap();
            oir::check_source(source, ast, project.sources())
        }
        SyntaxFlavor::ProjectSyntax => oir::project::check_project_executable(
            project,
            IndexLimits::default(),
            &WorkMeter::default(),
            &mut Allocator::default(),
        ),
    }
}

fn reject(text: &str, code: &str, stage: &str) {
    let fixture = Fixture::new(&[("main.ox", text)]);
    let errors = match ProjectSources::load_typed(&fixture.entry(), ProjectLimits::default()) {
        Ok(project) => checked(&project).unwrap_err(),
        Err(failure) => failure.diagnostics,
    };
    assert!(
        errors
            .iter()
            .any(|error| (error.code, error.stage) == (code, stage)),
        "{text}: {errors:?}"
    );
    assert!(
        errors.iter().all(|error| error.code != "E0500"),
        "{errors:?}"
    );
}

// Exact UTF-8 sources from the independent d6249e7 fixture document. These
// literals and expectations are frozen; formatting controls use derived copies.
// SHA256: 95bbfb95b733bc272e2b11e6772b66a74bf14507f4a86a7f3e9258d4c88d2220
const TINY_RELAY_TAKE: &str = "enum E{N,V(i32)} fn relay(x:E)->E{return x;} fn take(x:E)->i32{match x{E::N=>{return 0;},E::V(v)=>{return v;},}} fn main()->i32{return take(relay(E::V(7)));}";
// SHA256: 37b027516464e4346fdce2abf1670c60e98834cda0632a31eca8af550086b74a
const TINY_WRITTEN_ORDER: &str = "enum E{N,V(i32)} fn relay(x:E)->E{return x;} fn take(x:E)->i32{match x{E::V(v)=>{return v;},E::N=>{return 0;},}} fn main()->i32{return take(relay(E::V(7)));}";
// SHA256: ba050a2d911ec4a5c0efce3dc80c48437e3ebc20829f9d5aae2c821085e006ac
const SINGLETON_NULLARY: &str =
    "enum E{Only} fn main()->i32{let x=E::Only;match x{E::Only=>{return 5;}}}";
// SHA256: bab59d2e88e75bf2cdbcb28d47d73c01c86e0c563df14b9ef4b3f1aa9976dd24
const SINGLETON_BOOL: &str =
    "enum E{B(bool)} fn main()->bool{let x=E::B(true);match x{E::B(v)=>{return v;}}}";
// SHA256: 1e5af9f0276ed35d618fdcae6376ce8c55d908782a8c0a8af365dc87340ea21a
const SINGLETON_UNIT: &str =
    "enum E{U(())} fn main()->(){let x=E::U(());match x{E::U(v)=>{return v;}}}";
// SHA256: e91ede8c0927ff7837ab545bd4c0ce17cbf1217550cad538ce872d680740cbaf
const NESTED_FIRST_ARM: &str = "enum E{N,V(i32)} fn take(x:E,y:E)->i32{match x{E::N=>{match y{E::N=>{return 1;},E::V(v)=>{return v;}}},E::V(a)=>{match y{E::N=>{return a;},E::V(b)=>{return a+b;}}}}} fn main()->i32{return take(E::N,E::V(9));}";
// SHA256: e914e897f7790913a7cf212ce29d9bd500cf6bcc27c0a6e2b2abf8d9f7269165
const MIXED_JOIN_REPLACEMENT: &str = "enum E{N,V(i32)} fn choose(flag:bool)->i32{let mut x=E::N;if flag{match x{E::N=>{},E::V(v)=>{}}}x=E::V(7);match x{E::N=>{return 0;},E::V(out)=>{return out;}}} fn main()->i32{return choose(true)+choose(false);}";
// SHA256: c90d885f54ad6f58e47df6d09fc6de89cdeacb4e40110db835179d47e19fae25
const MOVED_REMATCH: &str = "enum E{N,V(i32)} fn main()->i32{let x=E::N;match x{E::N=>{},E::V(v)=>{}}match x{E::N=>{return 0;},E::V(w)=>{return w;}}}";
// SHA256: 789c58e8c5ce6212b65d4acfe82158b9f9c742bb0413269c221bb719e5946eb6
const UNRESTORED_LOOP: &str =
    "enum E{N} fn main()->i32{let x=E::N;let mut n=0;while n<2{match x{E::N=>{n=n+1;}}}return n;}";
// SHA256: e4ea630d6ba096bf17c42d6d936a216e35141bfacff1fe19b12589f7e3b15b2d
const ENUM_ENTRY_RESULT: &str = "enum E{N} fn main()->E{return E::N;}";

#[cfg(target_os = "linux")]
const SCANNER_MAIN: &str = include_str!("../../tests/fixtures/bounded_enum_scanner/main.ox");
#[cfg(target_os = "linux")]
const SCANNER: &str = include_str!("../../tests/fixtures/bounded_enum_scanner/scanner.ox");

#[test]
fn bounded_enum_public_frozen_original_and_project_results() {
    for (text, expected, functions, main) in [
        (TINY_RELAY_TAKE, Scalar::I32(7), 3, 2),
        (TINY_WRITTEN_ORDER, Scalar::I32(7), 3, 2),
        (SINGLETON_NULLARY, Scalar::I32(5), 1, 0),
        (SINGLETON_BOOL, Scalar::Bool(true), 1, 0),
        (SINGLETON_UNIT, Scalar::Unit, 1, 0),
        (NESTED_FIRST_ARM, Scalar::I32(9), 2, 1),
        (MIXED_JOIN_REPLACEMENT, Scalar::I32(14), 2, 1),
    ] {
        let fixture = Fixture::new(&[("main.ox", text)]);
        let project = fixture.load();
        assert_eq!(project.syntax_flavor(), SyntaxFlavor::OriginalSingleFile);
        let original = checked(&project).unwrap();
        assert_eq!(original.route(), ProjectRoute::Owned);
        assert_eq!(original.entry(), Some(hir::DefId(main)));
        assert_eq!(original.function_count(), functions);
        assert_eq!(original.run().unwrap(), expected, "{text}");
        let linked = oir::project::check_project_executable(
            &project,
            IndexLimits::default(),
            &WorkMeter::default(),
            &mut Allocator::default(),
        )
        .unwrap();
        assert_eq!(linked.entry(), original.entry());
        assert_eq!(linked.function_count(), functions);
        assert_eq!(linked.run().unwrap(), expected, "{text}");
    }
}

#[cfg(target_os = "linux")]
#[test]
fn bounded_enum_public_scanner_uses_original_files_aliases_and_private_cursor() {
    let fixture = Fixture::new(&[("main.ox", SCANNER_MAIN), ("scanner.ox", SCANNER)]);
    let project = fixture.load();
    assert_eq!(project.syntax_flavor(), SyntaxFlavor::ProjectSyntax);
    assert_eq!(project.sources().files().len(), 2);
    assert_eq!(project.sources().get(SourceFileId(0)).text(), SCANNER_MAIN);
    assert_eq!(project.sources().get(SourceFileId(1)).text(), SCANNER);
    let result = checked(&project).unwrap();
    assert_eq!(result.route(), ProjectRoute::Owned);
    assert_eq!(result.entry(), Some(hir::DefId(0)));
    assert_eq!(result.function_count(), 3);
    assert_eq!(result.run().unwrap(), Scalar::I32(115));

    // The cursor's private field is legal inside scanner.ox, not at the root.
    fs::write(
        fixture.0.join("main.ox"),
        "mod scanner; fn main()->i32{let c=crate::scanner::new_cursor();return c.position;}",
    )
    .unwrap();
    let denied = fixture.load();
    let errors = checked(&denied).unwrap_err();
    assert_eq!((errors[0].code, errors[0].stage), ("E0206", "type"));
    assert_eq!(
        denied.sources().text(errors[0].primary.unwrap()),
        "position"
    );
}

#[test]
fn bounded_enum_public_consumption_reports_the_whole_match_origin() {
    for (text, mark) in [
        (
            MOVED_REMATCH,
            "match x{E::N=>{return 0;},E::V(w)=>{return w;}}",
        ),
        (UNRESTORED_LOOP, "match x{E::N=>{n=n+1;}}"),
    ] {
        let fixture = Fixture::new(&[("main.ox", text)]);
        let project = fixture.load();
        // Type-only checking intentionally does not perform raw ownership proof.
        oir::project::check_project_candidate(
            &project,
            IndexLimits::default(),
            &WorkMeter::default(),
            &mut Allocator::default(),
        )
        .unwrap();
        let errors = checked(&project).unwrap_err();
        assert_eq!((errors[0].code, errors[0].stage), ("E0310", "ownership"));
        let span = errors[0].primary.unwrap();
        assert_eq!(span.file, SourceFileId(0));
        assert_eq!(
            (span.start, span.end),
            (
                text.rfind(mark).unwrap(),
                text.rfind(mark).unwrap() + mark.len()
            )
        );
        assert_eq!(project.sources().text(span), mark);
    }

    let restored = "enum E{N} fn main()->i32{let mut x=E::N;let mut n=0;while n<2{match x{E::N=>{n=n+1;}}x=E::N;continue;}match x{E::N=>{return n;}}}";
    let fixture = Fixture::new(&[("main.ox", restored)]);
    let project = fixture.load();
    assert_eq!(checked(&project).unwrap().run().unwrap(), Scalar::I32(2));
}

#[test]
fn bounded_enum_public_nominal_payload_and_unused_arm_checks() {
    for (text, stage) in [
        ("enum E{V(i32)} fn main()->i32{let x=E::V(true);return 0;}", "type"),
        ("enum E{N} fn main()->i32{let x=E::N(1);return 0;}", "resolve"),
        ("enum E{V(i32)} fn main()->i32{let x=E::V;return 0;}", "resolve"),
        ("enum E{A,B} enum F{A,B} fn take(x:E)->i32{match x{F::A=>{return 0;},F::B=>{return 1;}}} fn main()->i32{return 0;}", "type"),
        ("enum E{A,B} fn take(x:E)->i32{match x{E::A=>{return 0;},E::A=>{return 1;}}} fn main()->i32{return 0;}", "resolve"),
        ("enum E{A,B} fn take(x:E)->i32{match x{E::A=>{return 0;}}} fn main()->i32{return 0;}", "resolve"),
    ] {
        reject(text, "E0300", stage);
    }
}

#[test]
fn bounded_enum_public_unsupported_borrow_containment_array_and_patterns() {
    for (text, code, stage) in [
        ("enum E{N} fn f(e:&E)->(){return;}", "E0202", "resolve"),
        ("enum E{N} struct R{e:E}", "E0300", "resolve"),
        ("enum E{N} fn f(a:[E;1])->(){return;}", "E0101", "parse"),
        (
            "enum E{N} fn main()->bool{let a=E::N;let b=E::N;return a==b;}",
            "E0300",
            "type",
        ),
        (
            "enum E{N} fn f(e:E)->(){match e{E::N if true=>{return;}}}",
            "E0100",
            "parse",
        ),
        (
            "enum E{V(i32)} fn f(e:E)->(){match e{E::V(_)=>{return;}}}",
            "E0100",
            "parse",
        ),
        (
            "enum E{N} fn f(e:E)->i32{return match e{E::N=>{return 1;}};}",
            "E0101",
            "parse",
        ),
    ] {
        reject(text, code, stage);
    }
}

#[test]
fn bounded_enum_public_formatter_preserves_comments_crlf_and_runtime() {
    let text = format!(
        "// enum match => :: 雪\r\n{}\r\n",
        TINY_RELAY_TAKE.replace(
            "E::V(v)=>",
            "E /* type */ :: /* variant */ V(v) /* before */ => /* after */"
        )
    );
    let fixture = Fixture::new(&[("main.ox", &text)]);
    let original = fixture.load();
    assert_eq!(checked(&original).unwrap().run().unwrap(), Scalar::I32(7));
    let file = original.sources().get(SourceFileId(0));
    let formatted = format::format_source(file).unwrap();
    assert!(formatted.contains("=>"));
    assert!(!formatted.contains("= >"));
    for comment in [
        "// enum match => :: 雪",
        "/* type */",
        "/* variant */",
        "/* before */",
        "/* after */",
    ] {
        assert_eq!(formatted.matches(comment).count(), 1);
    }
    fs::write(fixture.0.join("main.ox"), &formatted).unwrap();
    let again = fixture.load();
    let second = again.sources().get(SourceFileId(0));
    assert_eq!(format::format_source(second).unwrap(), formatted);
    assert_eq!(checked(&again).unwrap().run().unwrap(), Scalar::I32(7));

    // Compare every non-whitespace lexeme, including comment bytes. Line-ending
    // normalization is formatting; token and comment spelling must be lossless.
    let projection = |file: &super::source::SourceFile| {
        lexer::lex(file)
            .unwrap()
            .into_iter()
            .filter_map(|token| {
                let spelling = file.text_at(token.span);
                if token.kind == lexer::Kind::Trivia && spelling.chars().all(char::is_whitespace) {
                    None
                } else {
                    Some((token.kind, spelling.to_owned()))
                }
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(projection(file), projection(second));

    let borrowed = "enum E{N} struct R{n:i32} fn get(p:&R)->i32{return p.n;} fn forward(p:&R)->i32{return crate::get(&*p);} fn main()->i32{let r=R{n:7};return crate::forward(&r);}";
    fs::write(fixture.0.join("main.ox"), borrowed).unwrap();
    let project = fixture.load();
    let formatted = format::format_source(project.sources().get(SourceFileId(0))).unwrap();
    assert!(formatted.contains("crate::get(&*p)"));
    fs::write(fixture.0.join("main.ox"), &formatted).unwrap();
    let project = fixture.load();
    assert_eq!(checked(&project).unwrap().run().unwrap(), Scalar::I32(7));
    assert_eq!(
        format::format_source(project.sources().get(SourceFileId(0))).unwrap(),
        formatted
    );
}

#[test]
fn bounded_enum_public_entry_checks_remain_consumer_checks() {
    for (text, expected_entry) in [
        (ENUM_ENTRY_RESULT, Some(hir::DefId(0))),
        (
            "enum E{N} fn main(x:E)->i32{match x{E::N=>{return 7;}}}",
            Some(hir::DefId(0)),
        ),
        ("enum E{N} fn helper()->E{return E::N;}", None),
    ] {
        let fixture = Fixture::new(&[("main.ox", text)]);
        let project = fixture.load();
        let types = oir::project::check_project_candidate(
            &project,
            IndexLimits::default(),
            &WorkMeter::default(),
            &mut Allocator::default(),
        )
        .unwrap();
        assert_eq!(types.root_main(), expected_entry);
        let program = checked(&project).unwrap();
        assert_eq!(program.entry(), expected_entry);
        let run = program.run().unwrap_err();
        let native = program.native_module().unwrap_err();
        assert_eq!((run.code, run.stage), ("E0600", "oir-run"));
        assert_eq!((native.code, native.stage), ("E0700", "native-admission"));
        for error in [run, native] {
            if expected_entry.is_some() {
                assert_eq!(project.sources().text(error.primary.unwrap()), "main");
            } else {
                assert!(error.primary.is_none());
            }
        }
    }
}

#[test]
fn bounded_enum_public_original_source_identity_cannot_be_substituted() {
    let fixture = Fixture::new(&[("main.ox", TINY_RELAY_TAKE)]);
    let first = fixture.load();
    let second = fixture.load();
    let (file, ast) = first.original_file().unwrap();
    let (other_file, other_ast) = second.original_file().unwrap();
    assert_eq!(file.text(), other_file.text());
    for errors in [
        oir::check_source(file, ast, second.sources()).unwrap_err(),
        oir::check_source(file, other_ast, first.sources()).unwrap_err(),
    ] {
        assert_eq!(
            (errors[0].code, errors[0].stage),
            ("E0500", "resolve-project")
        );
        assert!(errors[0].primary.is_none());
        assert!(errors[0].secondary.is_empty());
    }
    assert_eq!(checked(&first).unwrap().run().unwrap(), Scalar::I32(7));
}

#[test]
fn bounded_enum_public_unused_declaration_selects_owned_without_changing_result() {
    let scalar = "fn value()->i32{return 7;} fn main()->i32{return crate::value();}";
    for (text, route) in [
        (scalar.to_owned(), ProjectRoute::Scalar),
        (format!("enum Unused{{N}} {scalar}"), ProjectRoute::Owned),
    ] {
        let fixture = Fixture::new(&[("main.ox", &text)]);
        let project = fixture.load();
        let checked = checked(&project).unwrap();
        assert_eq!(checked.route(), route);
        assert_eq!(checked.entry(), Some(hir::DefId(1)));
        assert_eq!(checked.run().unwrap(), Scalar::I32(7));
    }
}

#[test]
fn bounded_enum_public_zero_enum_calls_preserve_old_work_order_and_inputs() {
    for (text, expected, route, targets) in [
        ("fn get()->i32{return 7;} fn main()->i32{return crate::get();}", Some(7), ProjectRoute::Scalar, vec!["crate::get"]),
        ("fn first()->i32{return 1;} fn pair(a:i32,b:i32)->i32{return a+b;} fn main()->i32{return crate::pair(crate::first(),crate::first());}", Some(2), ProjectRoute::Scalar, vec!["crate::pair", "crate::first", "crate::first"]),
        ("struct R{x:i32} fn get(p:&R)->i32{return p.x;} fn forward(p:&R)->i32{return crate::get(&*p);} fn main()->i32{let r=R{x:7};return crate::forward(&r);}", Some(7), ProjectRoute::Owned, vec!["crate::get", "crate::forward"]),
        ("struct R{x:i32} fn tick(p:&mut R)->i32{p.x=p.x+1;return p.x;} fn pair(a:i32,b:i32)->i32{return a*10+b;} fn main()->i32{let mut r=R{x:0};return crate::pair(crate::tick(&mut r),crate::tick(&mut r));}", Some(12), ProjectRoute::Owned, vec!["crate::pair", "crate::tick", "crate::tick"]),
        ("fn main()->i32{return crate::missing(unknown_argument);}", None, ProjectRoute::Scalar, vec![]),
        ("fn get(a:i32,b:i32)->i32{return a+b;} fn main()->i32{return crate::get(first_missing,second_missing);}", None, ProjectRoute::Scalar, vec!["crate::get"]),
    ] {
        let fixture = Fixture::new(&[("main.ox", text)]);
        // Existing historical parser adapter: arrays enabled, enums closed.
        // Neither side is synthesized by mutating or relabeling an AST.
        let old = ProjectSources::load_array_candidate(
            &fixture.entry(), ProjectLimits::default(), &mut Allocator::default(),
        ).unwrap();
        let public = fixture.load();
        let old_ast = old.try_file_ast(SourceFileId(0)).unwrap();
        let public_ast = public.try_file_ast(SourceFileId(0)).unwrap();
        assert_eq!(old.sources().get(SourceFileId(0)).text(), text);
        assert_eq!(public.sources().get(SourceFileId(0)).text(), text);
        assert_eq!(old.sources().get(SourceFileId(0)).path(), public.sources().get(SourceFileId(0)).path());
        assert!(old_ast.enums.is_empty() && public_ast.enums.is_empty());
        assert!(old_ast.expressions.iter().any(|expression| matches!(expression.kind, ast::ExprKind::Call { callee: ast::ItemPath::Absolute(_), .. })));
        assert!(!old_ast.expressions.iter().any(|expression| matches!(expression.kind, ast::ExprKind::QualifiedValue { .. })));
        assert!(public_ast.expressions.iter().any(|expression| matches!(expression.kind, ast::ExprKind::QualifiedValue { .. })));

        let snapshot = |project: &ProjectSources, limit| {
            let work = WorkMeter::new(limit);
            work.enable_observation();
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(256).unwrap();
            let result = oir::project::check_project_executable(
                project, IndexLimits::default(), &work, &mut allocator,
            );
            let success = result.is_ok();
            let output = match result {
                Ok(program) => {
                    assert_eq!(program.route(), route);
                    assert_eq!(program.run().unwrap(), Scalar::I32(expected.unwrap()));
                    // Exact module bytes compare existing lower order and runtime
                    // inputs. This does not claim a new public fuel observation.
                    format!("{:?}:{:?}:{}:{}", program.route(), program.entry(), program.function_count(), program.native_module().unwrap())
                }
                Err(errors) => errors.iter().map(|error| error.render_json(project.sources())).collect::<String>(),
            };
            let callees = work.observations.borrow().iter().filter_map(|event| {
                if let Observation::Target { operation: "callee", origin, .. } = event {
                    Some(project.sources().text(*origin).to_owned())
                } else { None }
            }).collect::<Vec<_>>();
            // Whole-project admission includes the index's mandatory work
            // preflight, which may exceed actual completed work. Recover that
            // existing floor from its observations rather than padding work.
            let events = work.events.borrow();
            let preflight_end = events.iter().rposition(|event| event.operation == "preflight visit");
            let preflight_work: u64 = preflight_end.map_or(0, |end| events[..=end].iter().map(|event| event.units).sum());
            let mandatory = work.observations.borrow().iter().find_map(|event| {
                if let Observation::Plan(plan) = event { Some(preflight_work + plan.build_work) } else { None }
            }).unwrap_or(work.used());
            let boundary = work.used().max(mandatory);
            assert!(!allocator.observer_trace_overflow);
            (
                work.used(), success, output, callees,
                format!("{:?}", work.events.borrow()),
                format!("{:?}", work.observations.borrow()),
                format!("{:?}", allocator.trace), allocator.attempts, boundary,
            )
        };
        let baseline = snapshot(&old, IndexLimits::default().work);
        assert_eq!(baseline, snapshot(&public, IndexLimits::default().work), "{text}");
        assert_eq!(baseline.1, expected.is_some());
        assert_eq!(baseline.3, targets, "callee query count and source order: {text}");
        if expected.is_none() {
            let name = if text.contains("crate::missing") { "missing" } else { "first_missing" };
            assert!(baseline.2.contains("\"code\":\"E0200\""));
            assert!(baseline.2.contains(name));
            assert!(!baseline.2.contains("second_missing"));
            if name == "missing" { assert!(!baseline.2.contains("unknown_argument")); }
        }
        for limit in [baseline.8.saturating_sub(1), baseline.8, baseline.8 + 1] {
            let historical = snapshot(&old, limit);
            assert_eq!(historical, snapshot(&public, limit), "limit={limit}: {text}");
            if expected.is_some() {
                assert_eq!(historical.1, limit >= baseline.8, "limit={limit}: {text}");
                if limit < baseline.8 { assert!(historical.2.contains("E0400")); }
            }
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
fn bounded_enum_public_private_callee_precedes_bad_argument() {
    for child in [
        "fn hidden(x:i32)->i32{return x;}",
        "struct R{x:i32} fn hidden(x:i32)->i32{return x;}",
    ] {
        let fixture = Fixture::new(&[
            (
                "main.ox",
                "mod child; fn main()->i32{return crate::child::hidden(unknown_argument);}",
            ),
            ("child.ox", child),
        ]);
        let old = ProjectSources::load_array_candidate(
            &fixture.entry(),
            ProjectLimits::default(),
            &mut Allocator::default(),
        )
        .unwrap();
        let public = fixture.load();
        let old_errors = checked(&old).unwrap_err();
        let errors = checked(&public).unwrap_err();
        assert_eq!((errors[0].code, errors[0].stage), ("E0206", "resolve"));
        assert_eq!(public.sources().text(errors[0].primary.unwrap()), "hidden");
        assert_eq!(old_errors.len(), errors.len());
        for (old_error, error) in old_errors.iter().zip(&errors) {
            assert_eq!(
                old_error.render_json(old.sources()),
                error.render_json(public.sources())
            );
        }
    }
}
