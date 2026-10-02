//! Focused linked-production checks; independent qualification uses separate fixtures.
use super::*;
use crate::frontend::project::ProjectLimits;
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
            "oxid-unit3-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        for (name, text) in files {
            fs::write(path.join(name), text).unwrap();
        }
        Self(path)
    }
    fn load(&self) -> ProjectSources {
        ProjectSources::load_project_candidate(
            self.0.join("app.ox").to_str().unwrap(),
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
fn checked(project: &ProjectSources) -> Result<CheckedSourceProgram<'_>, Vec<Diagnostic>> {
    check_project_executable_candidate(
        project,
        IndexLimits::default(),
        &WorkMeter::default(),
        &mut Allocator::default(),
    )
}
#[test]
fn scalar_project_uses_original_root_entry_and_global_calls() {
    let fixture = Fixture::new(&[
        ("app.ox", "mod child; use crate::child::sum as add; fn helper()->i32{return 91;} fn main()->i32{return add(2,3);}"),
        ("child.ox", "pub fn sum(a:i32,b:i32)->i32{return a+b;} fn main(x:i32)->i32{return x;}"),
    ]);
    let project = fixture.load();
    let result = checked(&project).unwrap();
    assert_eq!(result.entry(), Some(hir::DefId(1)));
    assert_eq!(result.route(), ProjectRoute::Scalar);
    assert_eq!(result.function_count(), 4);
    assert_eq!(result.run().unwrap(), Scalar::I32(5));
    assert!(result.native_module().unwrap().contains("define i32 @main"));
    let counts = super::super::source::association::last_usage().unwrap();
    assert_eq!(counts.count, counts.validation);
    assert_eq!(counts.validation.declarations, 4);
    assert_eq!(counts.dimensions, 1);
    assert!(
        counts.validation.declarations + counts.validation.spans
            <= 17 * project.usage().syntax_nodes
    );
}
#[test]
fn owned_project_links_nominals_and_preserves_nonzero_entry() {
    let fixture = Fixture::new(&[
        ("app.ox", "mod child; use crate::child::make; use crate::child::bump; use crate::child::finish; fn helper()->i32{return 91;} fn main()->i32{let mut c=make(); bump(&mut c); return finish(c);}"),
        ("child.ox", "pub struct C{n:i32} pub fn make()->C{return C{n:4};} pub fn bump(c:&mut C)->(){c.n=c.n+1;return;} pub fn finish(c:C)->i32{return c.n;} fn main(c:C)->C{return c;}"),
    ]);
    let project = fixture.load();
    let result = checked(&project).unwrap();
    assert_eq!(result.entry(), Some(hir::DefId(1)));
    assert_eq!(result.route(), ProjectRoute::Owned);
    assert_eq!(result.function_count(), 6);
    assert_eq!(result.run().unwrap(), Scalar::I32(5));
    assert!(result.native_module().unwrap().contains("define i32 @main"));
    let counts = super::super::source::association::last_usage().unwrap();
    assert_eq!(counts.count, counts.validation);
    assert_eq!(counts.validation.declarations, 8);
    assert_eq!(counts.dimensions, 3);
}
#[test]
fn type_depth_stays_type_only_and_executable_checks_unused_ownership() {
    let fixture = Fixture::new(&[
        ("app.ox", "mod child;"),
        ("child.ox", "struct C{} fn unused(c:C)->(){c;c;return;}"),
    ]);
    let project = fixture.load();
    assert!(check_project_candidate(
        &project,
        IndexLimits::default(),
        &WorkMeter::default(),
        &mut Allocator::default()
    )
    .is_ok());
    let errors = checked(&project).unwrap_err();
    assert_eq!(errors[0].code, "E0310");
    assert_eq!(errors[0].primary.unwrap().file.0, 1);
}
#[test]
fn child_or_imported_main_never_supplies_entry() {
    for owned in [false, true] {
        let fixture = Fixture::new(&[
            ("app.ox", "mod child; use crate::child::main;"),
            (
                "child.ox",
                if owned {
                    "struct C{} pub fn main()->i32{return 9;}"
                } else {
                    "pub fn main()->i32{return 9;}"
                },
            ),
        ]);
        let project = fixture.load();
        let result = checked(&project).unwrap();
        assert_eq!(result.entry(), None);
        assert_eq!(result.run().unwrap_err().code, "E0600");
        assert_eq!(result.native_module().unwrap_err().code, "E0700");
    }
}
#[test]
fn private_linking_does_not_enable_public_module_syntax() {
    let fixture = Fixture::new(&[
        ("app.ox", "mod child; fn main()->i32{return 0;}"),
        ("child.ox", ""),
    ]);
    let failure = ProjectSources::load_original(
        fixture.0.join("app.ox").to_str().unwrap(),
        ProjectLimits::default(),
    )
    .unwrap_err();
    assert_eq!(failure.diagnostics[0].stage, "parse");
}

#[test]
fn project_depths_share_one_namespace_schedule_and_work_total() {
    for child in [
        "pub fn f()->i32{return 3;}",
        "struct C{} pub fn f()->i32{return 3;}",
    ] {
        let fixture = Fixture::new(&[
            (
                "app.ox",
                "mod child; fn main()->i32{return crate::child::f();}",
            ),
            ("child.ox", child),
        ]);
        let project = fixture.load();
        let typed_work = WorkMeter::default();
        typed_work.enable_observation();
        let executable_work = WorkMeter::default();
        executable_work.enable_observation();
        let typed = check_project_candidate(
            &project,
            IndexLimits::default(),
            &typed_work,
            &mut Allocator::default(),
        )
        .unwrap();
        let executable = check_project_executable_candidate(
            &project,
            IndexLimits::default(),
            &executable_work,
            &mut Allocator::default(),
        )
        .unwrap();
        assert_eq!(typed.functions(), executable.function_count());
        assert_eq!(typed.route(), executable.route());
        assert_eq!(typed.root_main(), executable.entry());
        assert_eq!(typed_work.used(), executable_work.used());
        assert_eq!(
            format!("{:?}", typed_work.observations.borrow()),
            format!("{:?}", executable_work.observations.borrow())
        );
    }
}
