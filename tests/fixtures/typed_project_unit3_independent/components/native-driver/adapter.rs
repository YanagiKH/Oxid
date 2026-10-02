//! Private qualification adapter. Its inputs contain no expected outcomes.
use super::*;
use std::{cell::Cell, fs, io::Write, path::{Path, PathBuf}};
thread_local! { static FUEL: Cell<Option<usize>> = const { Cell::new(None) }; }
pub(in crate::frontend) fn fuel(default: usize) -> usize {
    FUEL.with(|value| value.get().unwrap_or(default).min(default))
}
fn receipt() -> PathBuf { PathBuf::from(std::env::var_os("OXID_UNIT3_NATIVE_RECEIPT").expect("receipt directory")) }
fn append(name: &str, line: &str) {
    let mut file=fs::OpenOptions::new().create(true).append(true).open(receipt().join(name)).expect("receipt append");
    writeln!(file,"{line}").expect("receipt write");
}
pub(in crate::frontend) fn record_module(module: &str) {
    fs::write(receipt().join("actual.ll"), module).expect("actual emitted IR receipt");
}
pub(in crate::frontend) fn record_tool(tool: &Path, args: &[&std::ffi::OsStr], cwd: &Path) {
    let args=args.iter().map(|v|json_string(&v.to_string_lossy())).collect::<Vec<_>>().join(",");
    append("tools.jsonl", &format!("{{\"tool\":{},\"argv\":[{}],\"cwd\":{}}}",json_string(&tool.to_string_lossy()),args,json_string(&cwd.to_string_lossy())));
}
struct Restore { visible: PathBuf, hidden: PathBuf }
impl Drop for Restore {
    fn drop(&mut self) {
        fs::rename(&self.hidden,&self.visible).expect("restore fixture root");
        append("source-state.jsonl", "{\"phase\":\"restored\",\"restored\":true}");
    }
}
fn hide_loaded(entry: &str, project: &ProjectSources) -> Restore {
    let visible=PathBuf::from(std::env::var_os("OXID_UNIT3_NATIVE_FIXTURE_ROOT").expect("fixture root"));
    assert!(Path::new(entry).starts_with(&visible));
    let hidden=visible.with_file_name(format!("{}.unavailable",visible.file_name().unwrap().to_string_lossy()));
    assert!(!hidden.exists(),"never overwrite an old unavailable fixture");
    fs::rename(&visible,&hidden).expect("hide loaded fixture root");
    let restore=Restore{visible,hidden};
    assert!(!Path::new(entry).exists());
    let mut files=Vec::new();
    // SourceMap iteration records original display paths, without rereading text.
    for source in project.sources().files() {
        let display=source.path();
        assert!(!Path::new(display).exists());
        files.push(json_string(display));
    }
    append("source-state.jsonl", &format!("{{\"phase\":\"after_load_before_check\",\"all_source_paths_absent\":true,\"files\":[{}]}}",files.join(",")));
    restore
}
fn candidate(args: &mut Vec<String>) -> i32 {
    let (path,json,operation,output)=match options::route(args) {
        Route::TypedCheck{path,json}=>(path,json,Operation::Check,None),
        Route::TypedRun{path,json}=>(path,json,Operation::Run,None),
        Route::TypedCompile{path,json,output}=>(path,json,Operation::Compile,Some(output)),
        Route::Error{..}=>return dispatch(args).expect("typed validation error"),
        Route::Legacy(_)=>panic!("private adapter requires typed argv"),
    };
    let project=match ProjectSources::load_project_candidate(&path,ProjectLimits::default()) {
        Ok(value)=>value,
        Err(failure)=>return report(&failure.sources,failure.diagnostics,json,Summary::empty(operation)),
    };
    let _restore=hide_loaded(&path,&project);
    let work=super::super::declaration_index::WorkMeter::default();
    let mut allocator=super::super::project::budget::Allocator::default();
    let executable=oir::project::check_project_executable_candidate(&project,super::super::declaration_index::IndexLimits::default(),&work,&mut allocator);
    process_loaded(&project,json,operation,output.as_deref(),executable)
}
pub(in crate::frontend) fn main() -> i32 {
    let receipt=receipt(); assert!(receipt.is_dir());
    assert!(!receipt.join("tools.jsonl").exists());
    fs::write(receipt.join("tools.jsonl"),b"").expect("empty tool receipt");
    let fuel=std::env::var("OXID_UNIT3_NATIVE_FUEL").ok().map(|s|s.parse::<usize>().expect("fuel integer"));
    FUEL.with(|value|value.set(fuel));
    let mut args=std::env::args().skip(1).collect::<Vec<_>>();
    if std::env::var_os("OXID_UNIT3_PUBLIC").is_some() {
        dispatch(&mut args).expect("public typed route")
    } else {candidate(&mut args)}
}
