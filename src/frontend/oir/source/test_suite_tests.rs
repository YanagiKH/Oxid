//! Core contract tests; independent CLI acceptance lives outside this module.
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
    fn new(root: &str, child: Option<&str>) -> Self {
        let path = std::env::temp_dir().join(format!(
            "oxid-test-core-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::write(path.join("app.ox"), root).unwrap();
        if let Some(child) = child {
            fs::write(path.join("helper.ox"), child).unwrap();
        }
        Self(path)
    }
    fn load(&self) -> ProjectSources {
        ProjectSources::load_typed(
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
fn checked(project: &ProjectSources) -> Result<CheckedProjectTests<'_>, Vec<Diagnostic>> {
    let limits = IndexLimits::default();
    check_project_tests(
        project,
        limits,
        &WorkMeter::new(limits.work),
        &mut Allocator::default(),
    )
}

#[test]
fn typed_project_test_root_declarations_reuse_production_helpers_and_keep_main() {
    for prefix in ["", "struct Marker {}"] {
        let fixture = Fixture::new(&format!("{prefix} mod helper; use crate::helper::answer as production; use crate::helper::test_hidden as test_alias; fn prelude()->i32{{return 91;}} fn main()->i32{{return production();}} fn test_z()->bool{{return production()==7;}} fn test_a()->i32{{return production()-7;}}"),
            Some("pub fn answer()->i32{return 7;} pub fn test_hidden(x:i32)->i32{return x;} fn main()->bool{return false;}"));
        let project = fixture.load();
        let suite = checked(&project).unwrap();
        assert_eq!(suite.len(), 2);
        assert_eq!(suite.name(0), Some("test_z"));
        assert_eq!(suite.name(1), Some("test_a"));
        assert_eq!(suite.name(2), None);
        assert_eq!(suite.program.entry, Some(hir::DefId(1)));
        assert_eq!(suite.program.run().unwrap(), Scalar::I32(7));
        assert!(suite.run(0).unwrap());
        assert!(suite.run(1).unwrap());
        assert_eq!(suite.run(2).unwrap_err().code, "E0500");
        assert_eq!(suite.native_module(2).unwrap_err().code, "E0500");
        let original = check_project_executable_candidate(
            &project,
            IndexLimits::default(),
            &WorkMeter::default(),
            &mut Allocator::default(),
        )
        .unwrap();
        assert_eq!(
            original.native_module().unwrap(),
            suite.program.native_module().unwrap()
        );
    }
}

const OUTCOMES: &str = "fn test_true()->bool{return true;} fn test_false()->bool{return false;} fn test_unit()->(){return;} fn test_zero()->i32{return 0;} fn test_negative()->i32{return -1;} fn test_large()->i32{return 2147483647;} fn test_wrap()->i32{return 256;} fn test_negative_wrap()->i32{return -256;} fn test_min()->i32{return -2147483648;}";
const EXPECTED: [bool; 9] = [true, false, true, true, false, false, false, false, false];

#[test]
fn typed_project_test_all_outcomes_and_native_wrappers() {
    for prefix in ["", "struct Marker {}"] {
        let fixture = Fixture::new(&format!("{prefix}{OUTCOMES}"), None);
        let project = fixture.load();
        let suite = checked(&project).unwrap();
        assert_eq!(suite.program.entry, None);
        for (ordinal, expected) in EXPECTED.into_iter().enumerate() {
            assert_eq!(suite.run(ordinal).unwrap(), expected);
            let module = suite.native_module(ordinal).unwrap();
            let wrapper = module.split("define i32 @main()").nth(1).unwrap();
            let suffix = if ordinal < 2 {
                "xor i1 %value, true"
            } else if ordinal == 2 {
                "ret i32 0"
            } else {
                "icmp ne i32 %value, 0"
            };
            assert!(wrapper.contains(suffix), "{wrapper}");
            assert!(!wrapper.contains("__oxid_print"));
            assert!(!wrapper.contains("trunc"));
            assert!(!wrapper.contains("__oxid_process_setup"));
            assert!(wrapper.contains(&format!("fn_{ordinal}(")));
        }
    }
}

#[test]
fn typed_project_test_rejects_empty_bad_signatures_and_unchecked_unused_bodies() {
    for text in [
        "fn helper()->i32{return 0;}",
        "fn test_bad(x:i32)->i32{return x;}",
        "fn test_good()->bool{return true;} fn test_bad()->u8{let x=0;return x.to_u8_checked();}",
        "struct R{} fn test_bad()->R{return R{};}",
        "fn test_bad()->[i32;1]{return [0];}",
    ] {
        let fixture = Fixture::new(text, None);
        let project = fixture.load();
        let error = checked(&project).unwrap_err();
        assert_eq!(error[0].code, "E0800", "{error:?}");
    }
    let fixture = Fixture::new(
        "mod helper; use crate::helper::test_child as test_alias;",
        Some("pub fn test_child()->bool{return true;}"),
    );
    assert_eq!(checked(&fixture.load()).unwrap_err()[0].code, "E0800");
    for child in [
        "fn broken()->i32{return true;}",
        "struct R{} fn broken(r:R)->(){r;r;return;}",
    ] {
        let fixture = Fixture::new("mod helper; fn test_ok()->bool{return true;}", Some(child));
        assert!(checked(&fixture.load()).is_err());
    }
}

#[test]
fn typed_project_test_checks_import_identity_without_reading_stdin() {
    for (root, child) in [
        (
            "use std::io::read_stdin as innocent; fn test_ok()->bool{return true;}",
            None,
        ),
        (
            "mod helper; fn test_ok()->bool{return true;}",
            Some("use std::io::read_stdin as unused;"),
        ),
    ] {
        let fixture = Fixture::new(root, child);
        let project = fixture.load();
        let errors = checked(&project).unwrap_err();
        assert_eq!(errors[0].code, "E0800");
        assert!(errors[0].message.contains("read_stdin function imports"));
    }
    for text in [
        "use std::io::ReadStatus; fn test_ok()->bool{return true;}",
        "fn read_stdin()->bool{return true;} fn test_ok()->bool{return read_stdin();}",
        "// use std::io::read_stdin;\nfn test_ok()->bool{return true;}",
    ] {
        let fixture = Fixture::new(text, None);
        let project = fixture.load();
        let suite = checked(&project).unwrap();
        assert!(suite.run(0).unwrap());
        assert!(suite.native_module(0).is_ok());
    }
    let fixture = Fixture::new(
        "use std::io::write_stdout; fn test_ok()->bool{return true;}",
        None,
    );
    assert!(checked(&fixture.load()).unwrap_err()[0]
        .message
        .contains("process"));
}

#[test]
fn typed_project_test_runtime_failure_does_not_reuse_execution_state() {
    for prefix in ["", "struct Marker {}"] {
        let fixture = Fixture::new(&format!("{prefix} fn test_error()->i32{{return 1/0;}} fn test_later()->bool{{let mut x=0; while x<2{{x=x+1;}}return x==2;}} fn test_false()->bool{{return false;}}"), None);
        let project = fixture.load();
        let suite = checked(&project).unwrap();
        for _ in 0..2 {
            assert!(suite.run(0).is_err());
            assert!(suite.run(1).unwrap());
            assert!(!suite.run(2).unwrap());
        }
        let module = suite.native_module(1).unwrap();
        let wrapper = module.split("define i32 @main()").nth(1).unwrap();
        assert!(wrapper.contains("store i64 1000000, ptr %fuel"));
        assert!(wrapper.contains("xor i1 %value, true"));
        assert!(!wrapper.contains("__oxid_print"));
    }
}

#[test]
#[ignore = "requires separately installed LLVM 19.1.7 on Linux x86_64"]
fn typed_project_test_native_agrees_with_interpreter() {
    use crate::frontend::{manifest::BuildSelection, options::EntryPolicy, project_build};
    use std::os::unix::fs::MetadataExt;
    for prefix in ["", "struct Marker {}"] {
        let fixture = Fixture::new(&format!("{prefix} mod helper; fn main()->i32{{return crate::helper::answer();}} {OUTCOMES} fn test_helper()->bool{{return crate::helper::answer()==7;}} fn test_error()->i32{{return 1/0;}} fn test_later()->bool{{return true;}}"), Some("pub fn answer()->i32{return 7;}"));
        let project = fixture.load();
        let suite = checked(&project).unwrap();
        let parent = format!("{}/", fixture.0.display());
        let selection = BuildSelection {
            entry: "app.ox".into(),
            parent: parent.clone(),
            name: "app".into(),
        };
        let app = project_build::publish(
            &suite.program.native_module().unwrap(),
            &selection,
            EntryPolicy::Result,
        )
        .unwrap();
        let app_bytes = fs::read(&app.path).unwrap();
        let app_inode = fs::metadata(&app.path).unwrap().ino();
        let artifacts: Vec<_> = (0..suite.len())
            .map(|i| {
                crate::frontend::project_build::publish_test(
                    &suite.native_module(i).unwrap(),
                    &parent,
                    i,
                )
                .unwrap()
            })
            .collect();
        for (ordinal, artifact) in artifacts.iter().enumerate() {
            assert_eq!(
                artifact.run_test().unwrap(),
                suite.run(ordinal).unwrap_or(false)
            );
        }
        assert_eq!(fs::read(&app.path).unwrap(), app_bytes);
        assert_eq!(fs::metadata(&app.path).unwrap().ino(), app_inode);
        assert_eq!(suite.program.run().unwrap(), Scalar::I32(7));
        assert_eq!(app.run().unwrap(), 0);
    }
}
