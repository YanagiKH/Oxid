use super::*;
use crate::frontend::declaration_index::{self, IndexLimits, SourceOwner, WorkMeter};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new(text: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "oxid-builtin-carrier-{}-{}.ox",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, text).unwrap();
        Self(path)
    }
    fn load(&self) -> ProjectSources {
        ProjectSources::load_builtin_candidate(
            self.0.to_str().unwrap(),
            ProjectLimits::default(),
            &mut Allocator::default(),
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_file(&self.0).unwrap();
    }
}
fn reject_before_allocation(project: &ProjectSources) {
    for collect in [
        declaration_index::collect_builtin_candidate,
        declaration_index::collect_originals,
    ] {
        let mut allocator = Allocator::default();
        assert!(collect(
            SourceOwner::project(project),
            IndexLimits::default(),
            &WorkMeter::default(),
            &mut allocator
        )
        .is_err());
        assert_eq!(allocator.attempts, 0);
    }
}

#[test]
fn builtin_current_project_routes_unused_imports_through_owned_validation() {
    for text in [
        "use std::io::ReadStatus as S; fn main()->i32{return 7;}",
        "use std::io::read_stdin as input; fn main()->i32{return 7;}",
    ] {
        let fixture = Fixture::new(text);
        let project =
            ProjectSources::load_typed(fixture.0.to_str().unwrap(), ProjectLimits::default())
                .unwrap();
        assert!(project.uses_owned_syntax());
        assert!(SourceOwner::project(&project)
            .owned(&WorkMeter::default())
            .unwrap());
        assert!(ProjectSources::load_typed_closed_std(
            fixture.0.to_str().unwrap(),
            ProjectLimits::default()
        )
        .is_err());
    }
}

#[test]
fn builtin_owned_classifier_preserves_pre_stdin_work_for_crate_imports() {
    for text in [
        "",
        "use crate::std::read_stdin; use crate::std::ReadStatus;",
    ] {
        let fixture = Fixture::new(text);
        let project =
            ProjectSources::load_typed(fixture.0.to_str().unwrap(), ProjectLimits::default())
                .unwrap();
        assert!(!project.programs[0].uses_std_imports());
        let work = WorkMeter::default();
        assert!(!SourceOwner::project(&project).owned(&work).unwrap());
        // Pre-stdin classification visits this one module; imports were not
        // inspected, and this fixture has no declaration/body/expression edges.
        assert_eq!(work.used(), 1);
    }
    let fixture = Fixture::new("use std::io::ReadStatus;");
    let project = fixture.load();
    let work = WorkMeter::default();
    assert!(SourceOwner::project(&project).owned(&work).unwrap());
    assert_eq!(work.used(), 2);
}
#[test]
fn builtin_import_paths_are_checked_and_ordinary_item_paths_remain_crate_only() {
    let fixture = Fixture::new("use std::io::ReadStatus as S; fn main()->(){return;}");
    let project = fixture.load();
    let owner = SourceOwner::project(&project);
    let import = project.programs[0].imports[0];
    let path = QualifiedPathRef {
        file: SourceFileId(0),
        path: import.path,
    };
    assert_eq!(owner.import_path(path).unwrap().root(), ast::PathRoot::Std);
    let item = ItemPathRef {
        file: path.file,
        path: ast::ItemPath::Absolute(path.path),
    };
    assert!(owner.path_span(item).is_err());
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let index = declaration_index::collect_builtin_candidate(
        owner,
        IndexLimits::default(),
        &work,
        &mut allocator,
    )
    .unwrap()
    .finish(&work, &mut allocator)
    .unwrap();
    assert!(index
        .query(&work)
        .qualified_value_endpoint(ModuleId(0), path)
        .is_err());
}
#[test]
fn builtin_std_paths_reused_as_types_calls_variants_or_detached_rows_fail_closed() {
    let fixture = Fixture::new("use std::io::ReadStatus as S; fn main()->(){0;}");
    for mutation in 0..5 {
        let mut project = fixture.load();
        let program = &mut project.programs[0];
        let path = program.imports[0].path;
        match mutation {
            0 => {
                program.functions[0].result.kind =
                    ast::TypeSyntaxKind::Name(ast::ItemPath::Absolute(path))
            }
            1 => {
                program.expressions[0].kind = ast::ExprKind::Call {
                    callee: ast::ItemPath::Absolute(path),
                    args: Vec::new(),
                }
            }
            2 => program.expressions[0].kind = ast::ExprKind::QualifiedValue { path, args: None },
            3 => {
                program.imports.clear();
                program
                    .items
                    .retain(|item| !matches!(item, ast::ItemId::Import(_)));
            }
            4 => program.paths[path.0].root = ast::PathRoot::Crate,
            _ => unreachable!(),
        }
        reject_before_allocation(&project);
    }
    let fixture = Fixture::new("use std::io::ReadStatus as S; fn f(s:S)->(){match s{S::Full=>{}}}");
    let mut project = fixture.load();
    let program = &mut project.programs[0];
    let path = program.imports[0].path;
    for statement in &mut program.functions[0].blocks[0].body {
        if let ast::StmtKind::Match { arms, .. } = &mut statement.kind {
            arms[0].variant = path;
        }
    }
    reject_before_allocation(&project);
}
#[test]
fn builtin_forged_std_root_cannot_authorize_a_crate_token() {
    let fixture = Fixture::new("use crate::io::ReadStatus; fn main()->(){return;}");
    let mut project = fixture.load();
    project.programs[0].paths[0].root = ast::PathRoot::Std;
    reject_before_allocation(&project);
}

#[test]
fn builtin_reused_import_with_detached_row_cannot_pass_equal_counts() {
    let fixture = Fixture::new(
        "use std::io::ReadStatus as S; use std::io::read_stdin as input; fn main()->(){return;}",
    );
    let mut project = fixture.load();
    let first = project.programs[0].imports[0].path;
    project.programs[0].imports[1].path = first;
    reject_before_allocation(&project);
}
#[test]
fn builtin_project_retained_carriers_are_measured() {
    use std::mem::{align_of, size_of};
    println!("builtin-project-layout root={}/{} path={} import={} program={} builder={} policy={} project={} usage={} load-result={} qualified-handle={}",size_of::<ast::PathRoot>(),align_of::<ast::PathRoot>(),size_of::<ast::QualifiedPath>(),size_of::<ast::ImportDecl>(),size_of::<ast::Program>(),size_of::<SourceSetBuilder<'_>>(),size_of::<ProjectEnumSyntax>(),size_of::<ProjectSources>(),size_of::<SourceUsage>(),size_of::<Result<ProjectSources,LoadFailure>>(),size_of::<QualifiedPathRef>());
}
