//! Acceptance through the public typed loader and the CLI's project facade.
//! Input-consuming cases use separate explicit subprocess qualification.
use super::{
    declaration_index::{IndexLimits, WorkMeter},
    diagnostic::Diagnostic,
    oir::{self, CheckedSourceProgram, Scalar},
    project::{budget::Allocator, ProjectLimits, ProjectSources},
};
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
            "oxid-public-stdin-{}-{}.ox",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, text).unwrap();
        Self(path)
    }
    fn load(&self) -> ProjectSources {
        ProjectSources::load_typed(self.0.to_str().unwrap(), ProjectLimits::default()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_file(&self.0).unwrap();
    }
}
fn checked(project: &ProjectSources) -> Result<CheckedSourceProgram<'_>, Vec<Diagnostic>> {
    oir::project::check_project_executable(
        project,
        IndexLimits::default(),
        &WorkMeter::default(),
        &mut Allocator::default(),
    )
}

#[test]
fn bounded_stdin_public_status_only_is_portable_nominal_enum() {
    let fixture = Fixture::new("use std::io::ReadStatus as S; fn main()->i32{let s=S::Eof(11);match s{S::Eof(n)=>{return n;},S::Full=>{return 2;},S::IoError=>{return 3;},}}");
    let project = fixture.load();
    let program = checked(&project).unwrap();
    assert_eq!(program.function_count(), 1);
    assert_eq!(program.run().unwrap(), Scalar::I32(11));
}

#[test]
fn bounded_stdin_public_unused_and_zero_capacity_preserve_host_policy() {
    for (text, result) in [
        ("use std::io::read_stdin; fn main()->i32{return 7;}", 7),
        ("use std::io::read_stdin as read; use std::io::ReadStatus as S; fn main()->i32{let mut a:[i32;0]=[];let s=read(&mut a);match s{S::Eof(n)=>{return n;},S::Full=>{return 39;},S::IoError=>{return -2;},}}", 39),
    ] {
        let fixture = Fixture::new(text);
        let project = fixture.load();
        let program = checked(&project).unwrap();
        assert_eq!(program.function_count(), 2);
        #[cfg(all(target_os="linux",target_arch="x86_64",target_pointer_width="64"))]
        assert_eq!(program.run().unwrap(), Scalar::I32(result));
        #[cfg(not(all(target_os="linux",target_arch="x86_64",target_pointer_width="64")))]
        {
            let _ = result;
            let error = program.run().unwrap_err();
            assert_eq!((error.code, error.stage), ("E0608", "oir-owned-run"));
            assert_eq!(error.message, "bounded stdin execution requires Linux x86_64");
            assert_eq!((error.primary.unwrap().start,error.primary.unwrap().end), (13,23));
        }
    }
}

#[test]
#[cfg(not(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
)))]
fn bounded_stdin_public_unsupported_host_precedes_entry_activation() {
    for body in [
        "fn helper()->i32{return 0;}",
        "use std::io::ReadStatus as S; fn main()->S{return S::Full;}",
    ] {
        let fixture = Fixture::new(&format!("use std::io::read_stdin; {body}"));
        let project = fixture.load();
        let error = checked(&project).unwrap().run().unwrap_err();
        assert_eq!((error.code, error.stage), ("E0608", "oir-owned-run"));
    }
}

#[test]
fn bounded_stdin_public_borrow_and_nominal_errors_are_user_diagnostics() {
    for text in [
        "use std::io::read_stdin; fn main()->i32{let mut a=[true];let s=read_stdin(&mut a);return 0;}",
        "use std::io::read_stdin; fn main()->i32{let a=[1];let s=read_stdin(&a);return 0;}",
        "use std::io::ReadStatus as S; enum User{Eof(i32),Full,IoError} fn use_status(s:S)->(){return;} fn main()->(){use_status(User::Full);return;}",
    ] {
        let fixture = Fixture::new(text);
        let project = fixture.load();
        let errors = checked(&project).unwrap_err();
        assert!(!errors.is_empty());
        assert!(errors.iter().all(|e| e.code != "E0500"), "{errors:?}");
    }
}
