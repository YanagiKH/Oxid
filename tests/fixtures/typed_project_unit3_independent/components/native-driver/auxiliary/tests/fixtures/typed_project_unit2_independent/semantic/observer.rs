//! Reviewer-only passive observation adapter. Reads a source-only TSV queue.
//! It never opens an expected/model artifact and never manufactures an index.
use crate::frontend::{ast, declaration_index as index, diagnostic::Diagnostic, hir, lexer, oir, parser, project, source, typeck};
use index::{Observation, WorkMeter};
use oir::owned_types::{BorrowKind, FieldId, ParameterTy, ValueTy};
use source::{SourceMap, Span};
use std::{fmt::Write as _, io::Write as _, path::Path, time::{SystemTime, UNIX_EPOCH}};

fn q(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""), '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"), '\r' => out.push_str("\\r"), '\t' => out.push_str("\\t"),
            c if c < '\u{20}' => write!(&mut out, "\\u{:04x}", c as u32).unwrap(),
            c => out.push(c),
        }
    }
    out.push('"'); out
}
fn object(fields: Vec<(&str, String)>) -> String {
    format!("{{{}}}", fields.into_iter().map(|(k,v)|format!("{}:{}",q(k),v)).collect::<Vec<_>>().join(","))
}
fn array(values: impl IntoIterator<Item=String>) -> String { format!("[{}]", values.into_iter().collect::<Vec<_>>().join(",")) }
fn number(value: usize) -> String { value.to_string() }
fn optional(value: Option<usize>) -> String { value.map_or_else(||"null".into(), number) }
fn span(value: Span) -> String { object(vec![("file",number(value.file.0)),("start",number(value.start)),("end",number(value.end))]) }
fn field(value: FieldId) -> String { object(vec![("record",number(value.record.0)),("index",number(value.index))]) }
fn value_type(value: ValueTy) -> String {
    match value {
        ValueTy::Owned(record) => object(vec![("kind",q("owned")),("record",number(record.0))]),
        ValueTy::Scalar(ty) => object(vec![("kind",q("scalar")),("scalar",q(&format!("{ty:?}")))]),
    }
}
fn parameter_type(value: ParameterTy) -> String {
    match value {
        ParameterTy::Value(value) => value_type(value),
        ParameterTy::Reference { record, kind } => object(vec![("kind",q(match kind { BorrowKind::Shared=>"shared",BorrowKind::Exclusive=>"exclusive"})),("record",number(record.0))]),
    }
}
fn sources(map: &SourceMap) -> String {
    array(map.files().iter().map(|f| object(vec![
        ("file",number(f.span(0,0).file.0)),("path",q(f.path())),("text",q(f.text())),("bytes",number(f.text().len())),
    ])))
}
fn diagnostics(errors: &[Diagnostic], map: &SourceMap) -> String {
    array(errors.iter().map(|d| d.render_json(map)))
}
fn human(errors: &[Diagnostic], map: &SourceMap) -> String {
    q(&errors.iter().map(|d| d.render_human(map)).collect::<String>())
}
fn observations(work: &WorkMeter, map: &SourceMap) -> String {
    array(work.observations.borrow().iter().map(|event| {
        let mut row: Vec<(&str,String)> = Vec::new();
        match event {
            Observation::Route {owned} => {row.extend([("event",q("route")),("owned",owned.to_string())]);}
            Observation::SignatureStart {function,origin} => {row.extend([("event",q("signature-start")),("function",number(function.0)),("origin",span(*origin))]);}
            Observation::RecordStart {record,origin} => {row.extend([("event",q("record-start")),("record",number(record.0)),("origin",span(*origin))]);}
            Observation::Phase(phase) => { row.push(("event",q("phase")));row.push(("phase",q(phase))); }
            Observation::Plan(plan) => { row.push(("event",q("plan")));row.push(("retained",plan.retained.to_string()));row.push(("scratch",plan.scratch.to_string()));row.push(("build_work",plan.build_work.to_string())); }
            Observation::Original {kind,id,module,name,local} => {
                row.extend([("event",q("original")),("kind",q(kind)),("id",number(*id)),("module",number(module.0)),("name",span(*name)),("local",number(*local))]);
            }
            Observation::Import {id,module,alias,committed,ty,value,aliases,seen} => {
                row.extend([("event",q("import")),("id",number(*id)),("module",number(module.0)),("alias",span(*alias)),("committed",committed.to_string()),("type_target",optional(ty.map(|r|r.0))),("value_target",optional(value.map(|d|d.0)))]);
                row.push(("aliases",array(aliases.iter().map(|a|object(vec![
                    ("module",number(a.module.0)),("alias",span(a.alias)),("type_target",optional(a.ty.map(|r|r.0))),("value_target",optional(a.value.map(|d|d.0))),("type_first",optional(a.type_first)),("value_first",optional(a.value_first)),
                ])))));
                row.push(("seen",array(seen.iter().map(|a|object(vec![
                    ("module",number(a.module.0)),("group",number(a.group)),("type_first",optional(a.type_first)),("value_first",optional(a.value_first)),
                ])))));
            }
            Observation::Frozen {root_main} => { row.extend([("event",q("frozen")),("root_main",optional(root_main.map(|d|d.0)))]); }
            Observation::Target {operation,origin,kind,id} => { row.extend([("event",q("target")),("operation",q(operation)),("origin",span(*origin)),("kind",q(kind)),("id",number(*id))]); }
            Observation::Expression {function,origin,ty,field: selected} => { row.extend([("event",q("expression")),("function",number(function.0)),("origin",span(*origin)),("type",value_type(*ty)),("field",selected.map_or_else(||"null".into(),field))]); }
            Observation::Binding {function,origin,ty} => { row.extend([("event",q("binding")),("function",number(function.0)),("origin",span(*origin)),("type",parameter_type(*ty))]); }
            Observation::Diagnostic {phase,diagnostic} => { row.extend([("event",q("diagnostic")),("phase",q(phase)),("diagnostic",diagnostic.render_json(map))]); }
            Observation::Projection {operation,function,origin,field: selected,ty} => { row.extend([("event",q("projection")),("operation",q(operation)),("function",number(function.0)),("origin",span(*origin)),("field",field(*selected)),("type",value_type(*ty))]); }
            Observation::BorrowArgument {function,origin,binding,ty} => { row.extend([("event",q("borrow-argument")),("function",number(function.0)),("origin",span(*origin)),("binding",number(*binding)),("type",parameter_type(*ty))]); }
            Observation::SortEmit {width,position,original} => { row.extend([("event",q("sort-emit")),("width",number(*width)),("position",number(*position)),("original",number(*original as usize))]); }
            // Any unexpected interface expansion is retained, never silently dropped.
            #[allow(unreachable_patterns)]
            other => { row.extend([("event",q("unmapped")),("raw",q(&format!("{other:?}")))]); }
        }
        object(row)
    }))
}
fn ast_functions(program: &ast::Program) -> Vec<String> {
    program.functions.iter().enumerate().map(|(local,f)|object(vec![
        ("file",number(f.name.file.0)),("local",number(local)),("name",span(f.name)),("end",span(f.end)),
    ])).collect()
}
fn project_run(entry: &str, check: bool) -> String {
    match project::ProjectSources::load_project_candidate(entry, project::ProjectLimits::default()) {
        Err(failure) => object(vec![("load_ok","false".into()),("sources",sources(&failure.sources)),("diagnostics",diagnostics(&failure.diagnostics,&failure.sources)),("human",human(&failure.diagnostics,&failure.sources)),("usage",q(&format!("{:?}",failure.usage)))]),
        Ok(project) => {
            let mut rows = vec![("load_ok","true".into()),("sources",sources(project.sources())),("syntax_flavor",q(&format!("{:?}",project.syntax_flavor()))),("usage",q(&format!("{:?}",project.usage())))];
            rows.push(("modules",array(project.modules().iter().enumerate().map(|(id,m)|object(vec![
                ("id",number(id)),("file",number(m.file.0)),("parent",optional(m.parent.map(|m|m.0))),
                ("declaration",m.declaration.map_or_else(||"null".into(),span)),("relative_path",q(&m.relative_path)),("depth",number(m.depth)),
            ])))));
            rows.push(("ast_functions",array(project.modules().iter().flat_map(|m| ast_functions(project.try_file_ast(m.file).expect("real loaded AST"))))));
            if check {
                let work = WorkMeter::default(); work.enable_observation();
                let result = oir::project::check_project_candidate(&project,index::IndexLimits::default(),&work,&mut project::budget::Allocator::default());
                match result {
                    Ok(checked) => rows.push(("check",object(vec![("success","true".into()),("route",q(&format!("{:?}",checked.route()))),("functions",number(checked.functions())),("records",number(checked.records())),("root_main",optional(checked.root_main().map(|d|d.0)))]))),
                    Err(errors) => { rows.push(("check",object(vec![("success","false".into())])));rows.push(("diagnostics",diagnostics(&errors,project.sources())));rows.push(("human",human(&errors,project.sources()))); }
                }
                rows.push(("observations",observations(&work,project.sources())));
                rows.push(("work_used",work.used().to_string()));
            } else { rows.push(("diagnostics","[]".into())); }
            object(rows)
        }
    }
}
fn parser_run(entry: &str, mode: parser::SourceMode) -> String {
    let mut map=SourceMap::new(); let file=map.add(entry.to_string(),std::fs::read_to_string(entry).unwrap());let source=map.get(file);
    let result=lexer::lex(source).map_err(|e|vec![*e]).and_then(|tokens|parser::parse_with_mode(source,tokens,mode));
    let mut rows=vec![("sources",sources(&map))];
    match result { Ok(program)=>{rows.push(("parser_ok","true".into()));rows.push(("diagnostics","[]".into()));rows.push(("ast_functions",array(ast_functions(&program))));}, Err(errors)=>{rows.push(("parser_ok","false".into()));rows.push(("diagnostics",diagnostics(&errors,&map)));rows.push(("human",human(&errors,&map)));} }
    object(rows)
}
fn legacy_run(entry: &str, owned: bool) -> String {
    let mut map=SourceMap::new();let id=map.add(entry.to_string(),std::fs::read_to_string(entry).unwrap());let source=map.get(id);
    let program=parser::parse_with_mode(source,lexer::lex(source).expect("legacy fixture lexes"),parser::SourceMode::OwnedCandidate).expect("legacy fixture parses");
    let work=WorkMeter::default();work.enable_observation();let mut allocator=project::budget::Allocator::default();
    let result=if owned {
        oir::owned::source::resolve::resolve_observed(source,&program,&work,&mut allocator)
            .and_then(|resolved|oir::owned::source::typeck::check(resolved).map(|_|()))
    } else {
        hir::resolve_observed(source,&program,&work,&mut allocator).and_then(|resolved|{
            work.phase("type");
            typeck::check(resolved).map(|_|()).inspect_err(|errors|for error in errors {work.record_error(error)})
        })
    };
    let errors=result.err().unwrap_or_default();
    let work_events=array(work.events.borrow().iter().map(|e|object(vec![("operation",q(e.operation)),("origin",span(e.origin)),("units",e.units.to_string())])));
    object(vec![("sources",sources(&map)),("api",q(if owned {"owned"} else {"scalar"})),("diagnostics",diagnostics(&errors,&map)),("human",human(&errors,&map)),
        ("ast_functions",array(ast_functions(&program))),("observations",observations(&work,&map)),("work_used",work.used().to_string()),
        ("work_events",work_events),
    ])
}

// Test-only program-level observations of Loader::read_source entry. This is
// not an OS syscall/open trace and makes no claim about lower-level I/O.
thread_local! {
    static SOURCE_READ_ENTRIES: std::cell::RefCell<Vec<(Vec<u8>, String, Option<Span>)>> = const { std::cell::RefCell::new(Vec::new()) };
}
pub(in crate::frontend) fn record_source_read(path: &Path, display: &str, origin: Option<Span>) {
    SOURCE_READ_ENTRIES.with(|entries| entries.borrow_mut().push((path.as_os_str().as_encoded_bytes().to_vec(), display.to_owned(), origin)));
}
fn clear_source_read_entries() {
    SOURCE_READ_ENTRIES.with(|entries| entries.borrow_mut().clear());
}
fn source_read_entries_json() -> String {
    SOURCE_READ_ENTRIES.with(|entries| array(entries.borrow().iter().map(|(path, display, at)|object(vec![
        ("native_path_hex", q(&path.iter().map(|b| format!("{b:02x}")).collect::<String>())), ("display", q(display)), ("origin", at.map_or_else(||"null".into(),span)),
    ]))))
}

fn time_ns() -> u128 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() }

#[test]
fn observe_source_queue() {
    let queue=std::fs::read_to_string(std::env::var("OXID_OBSERVER_QUEUE").expect("source-only queue required")).unwrap();
    let mut output=std::io::BufWriter::new(std::fs::File::create(std::env::var("OXID_OBSERVER_OUTPUT").expect("isolated output path required")).unwrap());
    for line in queue.lines() {
        let pieces:Vec<_>=line.split('\t').collect();assert_eq!(pieces.len(),3);
        let (case,mode,entry)=(pieces[0],pieces[1],pieces[2]);assert!(Path::new(entry).is_file());
        clear_source_read_entries();
        let start=time_ns();
        let result=match mode {
            "project"=>project_run(entry,true), "project-parser"=>project_run(entry,false),
            "parser-ProjectCandidate"=>parser_run(entry,parser::SourceMode::ProjectCandidate),
            "parser-OwnedCandidate"=>parser_run(entry,parser::SourceMode::OwnedCandidate),
            "parser-ModuleCandidate"=>parser_run(entry,parser::SourceMode::ModuleCandidate),
            "legacy-scalar"=>legacy_run(entry,false), "legacy-owned"=>legacy_run(entry,true),
            _=>panic!("unrecognized source-only queue mode {mode}"),
        };
        writeln!(output,"{}",object(vec![("case",q(case)),("mode",q(mode)),("started_ns",start.to_string()),("finished_ns",time_ns().to_string()),("raw",result),("read_source_entries",source_read_entries_json())])).unwrap();
        output.flush().unwrap();
    }
}
