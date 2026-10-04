//! Source-only bounded observations. No count/lower/ownership/execute calls.
use super::{hir::*, resolve, typeck};
use crate::frontend::{ast, declaration_index as di, diagnostic, project, source};
use std::{
    fmt::{self, Write},
    path::Path,
};
const MAX_ROWS: usize = 200_000;
const MAX_BYTES: usize = 16 * 1024 * 1024;
const TRACE_ROWS: usize = 200_000;
const DESCRIPTOR_OVERLAP: usize =
    256 * 1024 + 1 + 2 * 4096 + 128 + 16 * std::mem::size_of::<String>();
const CATEGORIES: [&str; 20] = [
    "header",
    "source",
    "phase",
    "selector",
    "ast_inventory",
    "record",
    "field",
    "signature",
    "parameter",
    "function",
    "binding",
    "block",
    "expression",
    "statement",
    "typed_expression",
    "typed_binding",
    "typed_block",
    "reservation",
    "diagnostic",
    "summary",
];
#[derive(Default)]
struct Counter {
    bytes: usize,
    units: usize,
    string: bool,
    escape: bool,
    atom: bool,
}
impl Write for Counter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.bytes = self.bytes.checked_add(s.len()).ok_or(fmt::Error)?;
        for b in s.bytes() {
            if self.string {
                if self.escape {
                    self.escape = false
                } else if b == b'\\' {
                    self.escape = true
                } else if b == b'"' {
                    self.string = false
                }
                continue;
            }
            match b {
                b'"' => {
                    self.units = self.units.checked_add(1).ok_or(fmt::Error)?;
                    self.string = true;
                    self.atom = false
                }
                b'{' | b'[' => {
                    self.units = self.units.checked_add(1).ok_or(fmt::Error)?;
                    self.atom = false
                }
                b'}' | b']' | b',' | b':' | b' ' | b'\n' | b'\r' | b'\t' => self.atom = false,
                _ => {
                    if !self.atom {
                        self.units = self.units.checked_add(1).ok_or(fmt::Error)?;
                    }
                    self.atom = true
                }
            }
        }
        Ok(())
    }
}
struct Output {
    text: String,
    rows: usize,
    units: usize,
    rows_limit: usize,
    bytes_limit: usize,
    counts: [usize; 20],
    auxiliary: usize,
    auxiliary_peak: usize,
    diagnostic_render_peak: usize,
}
impl Output {
    fn new(rows: usize, bytes: usize) -> Self {
        Self {
            text: String::new(),
            rows: 0,
            units: 0,
            rows_limit: rows.min(MAX_ROWS),
            bytes_limit: bytes.min(MAX_BYTES),
            counts: [0; 20],
            auxiliary: 0,
            auxiliary_peak: 0,
            diagnostic_render_peak: 0,
        }
    }
    fn row(&mut self, cat: usize, args: fmt::Arguments<'_>) -> Result<(), String> {
        let mut c = Counter::default();
        c.write_fmt(args).map_err(|_| "observer counter overflow")?;
        let bytes = self
            .text
            .len()
            .checked_add(c.bytes)
            .and_then(|x| x.checked_add(1))
            .ok_or("observer byte overflow")?;
        let units = self
            .units
            .checked_add(c.units)
            .ok_or("observer units overflow")?;
        let rows = self.rows.checked_add(1).ok_or("observer row overflow")?;
        if bytes > self.bytes_limit || units > self.rows_limit {
            return Err("incomplete-observation: observer limit exceeded".into());
        }
        self.text
            .try_reserve_exact(c.bytes + 1)
            .map_err(|_| "incomplete-observation: output allocation failed")?;
        self.text
            .write_fmt(args)
            .map_err(|_| "observer formatting failed")?;
        self.text.push('\n');
        self.rows = rows;
        self.units = units;
        if cat < 20 {
            self.counts[cat] = self.counts[cat].checked_add(1).ok_or("category overflow")?;
        }
        Ok(())
    }
    fn phase(&mut self, name: &str, status: &str) -> Result<(), String> {
        self.row(
            2,
            format_args!(
                "{{\"row\":\"phase\",\"phase\":{},\"status\":{}}}",
                J(name),
                J(status)
            ),
        )
    }
    fn aux(&mut self, n: usize) -> Result<(), String> {
        self.auxiliary = self.auxiliary.checked_add(n).ok_or("auxiliary overflow")?;
        self.auxiliary_peak = self.auxiliary_peak.max(self.auxiliary);
        if self.auxiliary > MAX_BYTES {
            return Err("incomplete-observation: auxiliary limit".into());
        }
        Ok(())
    }
    fn render_peak(&mut self, bytes: usize) -> Result<(), String> {
        let peak = self
            .auxiliary
            .checked_add(bytes)
            .ok_or("render overlap overflow")?;
        if peak > MAX_BYTES {
            return Err("incomplete-observation: render overlap limit".into());
        }
        self.auxiliary_peak = self.auxiliary_peak.max(peak);
        self.diagnostic_render_peak = self.diagnostic_render_peak.max(bytes);
        Ok(())
    }
    fn finish(mut self, outcome: &str, ds: usize) -> Result<String, String> {
        let rows = self.rows;
        let bytes = self.text.len();
        let units = self.units;
        let counts = self.counts;
        let aux = self.auxiliary;
        let peak = self.auxiliary_peak;
        let render_peak = self.diagnostic_render_peak;
        self.row(20,format_args!("{{\"row\":\"complete\",\"complete\":true,\"outcome\":{},\"rows_before_footer\":{},\"bytes_before_footer\":{},\"semantic_units_before_footer\":{},\"category_counts\":{},\"diagnostics\":{},\"auxiliary_requested_bytes\":{},\"auxiliary_peak_requested_bytes\":{},\"diagnostic_render_bound_bytes\":{},\"descriptor_overlap_bound_bytes\":{}}}",J(outcome),rows,bytes,units,Counts(&counts),ds,aux,peak,render_peak,DESCRIPTOR_OVERLAP))?;
        Ok(self.text)
    }
}
struct Counts<'a>(&'a [usize; 20]);
impl fmt::Display for Counts<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_char('{')?;
        for (i, n) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_char(',')?
            }
            write!(f, "{}:{}", J(CATEGORIES[i]), n)?
        }
        f.write_char('}')
    }
}
struct J<'a>(&'a str);
impl fmt::Display for J<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_char('"')?;
        for c in self.0.chars() {
            match c {
                '"' => f.write_str("\\\"")?,
                '\\' => f.write_str("\\\\")?,
                '\n' => f.write_str("\\n")?,
                '\r' => f.write_str("\\r")?,
                '\t' => f.write_str("\\t")?,
                c if c.is_control() => write!(f, "\\u{:04x}", c as u32)?,
                c => f.write_char(c)?,
            }
        }
        f.write_char('"')
    }
}
struct S<'a>(source::Span, &'a source::SourceMap);
impl fmt::Display for S<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.1.try_text(self.0).is_none() {
            return Err(fmt::Error);
        }
        write!(f, "[{}, {}, {}]", self.0.file.0, self.0.start, self.0.end)
    }
}
struct OS<'a>(Option<source::Span>, &'a source::SourceMap);
impl fmt::Display for OS<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(s) => write!(f, "{}", S(s, self.1)),
            None => f.write_str("null"),
        }
    }
}
struct OptU(Option<usize>);
impl fmt::Display for OptU {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(n) => write!(f, "{n}"),
            None => f.write_str("null"),
        }
    }
}
fn scalar(t: Ty) -> &'static str {
    match t {
        Ty::Bool => "bool",
        Ty::I32 => "i32",
        Ty::Unit => "unit",
    }
}
struct V<'a, 's>(ValueTy, &'a di::DeclarationIndex<'s>, &'a source::SourceMap);
impl fmt::Display for V<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            ValueTy::Scalar(t) => write!(f, "{{\"tag\":\"scalar\",\"scalar\":{}}}", J(scalar(t))),
            ValueTy::Owned(AggregateTy::FixedArray(a)) => write!(
                f,
                "{{\"tag\":\"fixed_array\",\"element\":{},\"length\":{}}}",
                J(scalar(a.element())),
                a.length()
            ),
            ValueTy::Owned(AggregateTy::Record(r)) => {
                let (k, m) = self.1.record(r).map_err(|_| fmt::Error)?;
                let a = self.1.sources().ast(m).map_err(|_| fmt::Error)?;
                write!(
                    f,
                    "{{\"tag\":\"record\",\"record_id\":{},\"declaration\":{}}}",
                    r.0,
                    S(a.records[k.index].name, self.2)
                )
            }
        }
    }
}
struct P<'a, 's>(
    ParameterTy,
    &'a di::DeclarationIndex<'s>,
    &'a source::SourceMap,
);
impl fmt::Display for P<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            ParameterTy::Value(v) => write!(
                f,
                "{{\"tag\":\"value\",\"value\":{}}}",
                V(v, self.1, self.2)
            ),
            ParameterTy::Reference { aggregate, kind } => write!(
                f,
                "{{\"tag\":\"reference\",\"kind\":{},\"aggregate\":{}}}",
                J(match kind {
                    BorrowKind::Shared => "shared",
                    BorrowKind::Exclusive => "exclusive",
                }),
                V(ValueTy::Owned(aggregate), self.1, self.2)
            ),
        }
    }
}
struct OV<'a, 's>(
    Option<ValueTy>,
    &'a di::DeclarationIndex<'s>,
    &'a source::SourceMap,
);
impl fmt::Display for OV<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(v) => write!(f, "{}", V(v, self.1, self.2)),
            None => f.write_str("null"),
        }
    }
}
struct Ids<'a>(&'a [ExprId]);
impl fmt::Display for Ids<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_char('[')?;
        for (i, id) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_char(',')?
            }
            write!(f, "{}", id.0)?
        }
        f.write_char(']')
    }
}
struct E<'a, 's>(
    &'a ExprKind,
    &'a di::DeclarationIndex<'s>,
    &'a source::SourceMap,
);
impl fmt::Display for E<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let map = self.2;
        match self.0{
 ExprKind::Bool(v)=>write!(f,"\"kind\":\"Bool\",\"value\":{v}"),ExprKind::I32(v)=>write!(f,"\"kind\":\"I32\",\"value\":{v}"),ExprKind::Unit=>f.write_str("\"kind\":\"Unit\""),ExprKind::Binding(id)=>write!(f,"\"kind\":\"Binding\",\"binding\":{}",id.0),ExprKind::Group(id)=>write!(f,"\"kind\":\"Group\",\"child\":{}",id.0),
 ExprKind::ArrayLiteral{elements}=>write!(f,"\"kind\":\"ArrayLiteral\",\"elements\":{}",Ids(elements)),
 ExprKind::IndexRead{base,base_span,index}=>write!(f,"\"kind\":\"IndexRead\",\"base\":{},\"base_origin\":{},\"index\":{}",base.0,S(*base_span,map),index.0),
 ExprKind::ArrayLength{base,base_span}=>write!(f,"\"kind\":\"ArrayLength\",\"base\":{},\"base_origin\":{}",base.0,S(*base_span,map)),
 ExprKind::FieldRead{base,base_span,field_span}=>write!(f,"\"kind\":\"FieldRead\",\"base\":{},\"base_origin\":{},\"field_origin\":{}",base.0,S(*base_span,map),S(*field_span,map)),
 ExprKind::Not{operand,operator_span}=>write!(f,"\"kind\":\"Not\",\"operand\":{},\"operator_origin\":{}",operand.0,S(*operator_span,map)),
 ExprKind::Logical{op,left,right,operator_span}=>write!(f,"\"kind\":\"Logical\",\"operator\":\"{op:?}\",\"left\":{},\"right\":{},\"operator_origin\":{}",left.0,right.0,S(*operator_span,map)),
 ExprKind::Comparison{op,left,right,operator_span}=>write!(f,"\"kind\":\"Comparison\",\"operator\":\"{op:?}\",\"left\":{},\"right\":{},\"operator_origin\":{}",left.0,right.0,S(*operator_span,map)),
 ExprKind::Arithmetic{op,left,right,operator_span}=>write!(f,"\"kind\":\"Arithmetic\",\"operator\":\"{op:?}\",\"left\":{},\"right\":{},\"operator_origin\":{}",left.0,right.0,S(*operator_span,map)),
 ExprKind::Call{target,args}=>{let(k,m)=self.1.function(*target).map_err(|_|fmt::Error)?;let a=self.1.sources().ast(m).map_err(|_|fmt::Error)?;write!(f,"\"kind\":\"Call\",\"target\":{},\"declaration\":{},\"arguments\":[",target.0,S(a.functions[k.index].name,map))?;for(i,arg)in args.iter().enumerate(){if i>0{f.write_char(',')?}match arg{Argument::Value(id)=>write!(f,"{{\"kind\":\"value\",\"expression\":{}}}",id.0)?,Argument::Borrow{kind,place,span,name_span,star_span}=>{let(b,forwarded)=match place{BorrowPlace::Owner(b)=>(b,false),BorrowPlace::Forwarded(b)=>(b,true)};write!(f,"{{\"kind\":\"borrow\",\"permission\":{},\"binding\":{},\"forwarded\":{},\"origin\":{},\"name_origin\":{},\"star_origin\":{}}}",J(match kind{BorrowKind::Shared=>"shared",BorrowKind::Exclusive=>"exclusive"}),b.0,forwarded,S(*span,map),S(*name_span,map),OS(*star_span,map))?}}}f.write_char(']')},
 ExprKind::StructLiteral{record,fields}=>{write!(f,"\"kind\":\"StructLiteral\",\"record\":{},\"fields\":[",record.0)?;for(i,x)in fields.iter().enumerate(){if i>0{f.write_char(',')?}write!(f,"{{\"record\":{},\"field\":{},\"value\":{},\"origin\":{}}}",x.field.record.0,x.field.index,x.value.0,S(x.span,map))?}f.write_char(']')}
}
    }
}
struct ST<'a>(&'a StmtKind, &'a source::SourceMap);
impl fmt::Display for ST<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let m = self.1;
        match self.0{
 StmtKind::Let{binding,init}=>write!(f,"\"kind\":\"Let\",\"binding\":{},\"init\":{},\"roots\":[{}]",binding.0,init.0,init.0),
 StmtKind::Assign{binding,target_span,operator_span,value}=>write!(f,"\"kind\":\"Assign\",\"binding\":{},\"target_origin\":{},\"operator_origin\":{},\"value\":{},\"roots\":[{}]",binding.0,S(*target_span,m),S(*operator_span,m),value.0,value.0),
 StmtKind::IndexAssign{base,base_span,target_span,operator_span,value,index}=>write!(f,"\"kind\":\"IndexAssign\",\"base\":{},\"base_origin\":{},\"target_origin\":{},\"operator_origin\":{},\"value\":{},\"index\":{},\"roots\":[{},{}]",base.0,S(*base_span,m),S(*target_span,m),S(*operator_span,m),value.0,index.0,value.0,index.0),
 StmtKind::FieldAssign{base,base_span,field_span,target_span,operator_span,value}=>write!(f,"\"kind\":\"FieldAssign\",\"base\":{},\"base_origin\":{},\"field_origin\":{},\"target_origin\":{},\"operator_origin\":{},\"value\":{},\"roots\":[{}]",base.0,S(*base_span,m),S(*field_span,m),S(*target_span,m),S(*operator_span,m),value.0,value.0),
 StmtKind::Expr(id)=>write!(f,"\"kind\":\"Expr\",\"value\":{},\"roots\":[{}]",id.0,id.0),StmtKind::Return(id)=>{write!(f,"\"kind\":\"Return\",\"value\":{},\"roots\":[",OptU(id.map(|x|x.0)))?;if let Some(id)=id{write!(f,"{}",id.0)?}f.write_char(']')},
 StmtKind::Break{target}=>write!(f,"\"kind\":\"Break\",\"target\":{},\"roots\":[]",target.0),StmtKind::Continue{target}=>write!(f,"\"kind\":\"Continue\",\"target\":{},\"roots\":[]",target.0),
 StmtKind::While{loop_id,condition,body}=>write!(f,"\"kind\":\"While\",\"loop_id\":{},\"condition\":{},\"body\":{},\"roots\":[{}]",loop_id.0,condition.0,body.0,condition.0),
 StmtKind::If{condition,then_block,else_block}=>write!(f,"\"kind\":\"If\",\"condition\":{},\"then\":{},\"else\":{},\"roots\":[{}]",condition.0,then_block.0,OptU(else_block.map(|x|x.0)),condition.0)
}
    }
}
struct ProjectionRow(Option<Projection>);
impl fmt::Display for ProjectionRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Some(p) = self.0 else {
            return f.write_str("null");
        };
        let (b, mode) = match p.base {
            AccessBase::Owner(b) => (b, "owner"),
            AccessBase::Reference { binding, kind } => (
                binding,
                match kind {
                    BorrowKind::Shared => "shared",
                    BorrowKind::Exclusive => "exclusive",
                },
            ),
        };
        write!(
            f,
            "{{\"binding\":{},\"mode\":{},\"record\":{},\"field\":{}}}",
            b.0,
            J(mode),
            p.field.record.0,
            p.field.index
        )
    }
}
fn loader_phases(out: &mut Output, a: &project::budget::Allocator) -> Result<(), String> {
    for (name, slot) in [("lex", 0), ("parse", 2)] {
        let attempted = a.observer_loader_phases[slot];
        let completed = a.observer_loader_phases[slot + 1];
        let status = if attempted == 0 {
            "not-attempted"
        } else if completed == attempted {
            "completed"
        } else {
            "failed"
        };
        out.row(2, format_args!("{{\"row\":\"phase\",\"phase\":{},\"status\":{},\"attempted_files\":{},\"completed_files\":{}}}", J(name), J(status), attempted, completed))?;
    }
    Ok(())
}
fn checked_add(a: &mut usize, n: usize) -> Result<(), String> {
    *a = a.checked_add(n).ok_or("observer inventory overflow")?;
    Ok(())
}
fn copied_string(s: &str) -> Result<String, String> {
    let mut out = String::new();
    out.try_reserve_exact(s.len())
        .map_err(|_| "display allocation")?;
    out.push_str(s);
    Ok(out)
}
fn sources(
    out: &mut Output,
    map: &source::SourceMap,
    root: &Path,
) -> Result<source::SourceMap, String> {
    let mut display = source::SourceMap::new();
    let mut display_alloc = project::budget::Allocator::default();
    display_alloc.observer_trace_bound(0)?;
    for (i, f) in map.files().iter().enumerate() {
        let relative = Path::new(f.path())
            .strip_prefix(root)
            .map_err(|_| "source outside fixture")?
            .to_str()
            .ok_or("path utf8")?;
        out.row(1,format_args!("{{\"row\":\"source\",\"file\":{},\"identity\":{},\"role\":{},\"path\":{},\"original_path\":{},\"bytes\":{},\"text\":{}}}",i,f.identity(),J(if i==0{"root"}else{"child"}),J(relative),J(f.path()),f.text().len(),J(f.text())))?;
        let lines = f
            .text()
            .bytes()
            .filter(|b| *b == b'\n')
            .count()
            .checked_add(1)
            .ok_or("line overflow")?;
        let payload = relative
            .len()
            .checked_add(f.text().len())
            .and_then(|n| n.checked_add(lines.checked_mul(std::mem::size_of::<usize>())?))
            .and_then(|n| n.checked_add(std::mem::size_of::<source::SourceFile>()))
            .ok_or("display overflow")?;
        out.aux(payload)?;
        display
            .try_add(
                copied_string(relative)?,
                copied_string(f.text())?,
                &mut display_alloc,
            )
            .map_err(|_| "display map reserve")?;
    }
    Ok(display)
}
fn diagnostics(
    out: &mut Output,
    map: &source::SourceMap,
    display: &source::SourceMap,
    ds: &[diagnostic::Diagnostic],
) -> Result<(), String> {
    for d in ds {
        if d.secondary.len() > 2 || d.notes.len() > 2 {
            return Err("incomplete-observation: diagnostic shape bound".into());
        }
        for s in d.primary.iter().chain(d.secondary.iter().map(|x| &x.0)) {
            if map.try_text(*s).is_none() {
                return Err("invalid diagnostic span".into());
            }
        }
        let mut bound = 16_384usize;
        for s in [d.code, d.stage, d.message.as_str()]
            .into_iter()
            .chain(d.notes.iter().map(String::as_str))
            .chain(d.secondary.iter().map(|x| x.1.as_str()))
            .chain(display.files().iter().map(|x| x.path()))
        {
            bound = bound
                .checked_add(s.len().checked_mul(128).ok_or("render overflow")?)
                .ok_or("render overflow")?;
        }
        if bound > out.bytes_limit.saturating_sub(out.text.len())
            || bound > MAX_BYTES.saturating_sub(out.auxiliary)
        {
            return Err("incomplete-observation: diagnostic preflight".into());
        }
        out.render_peak(bound)?;
        let h = d.render_human(display);
        let mut j = d.render_json(display);
        j.try_reserve_exact(1).map_err(|_| "json line allocation")?;
        j.push('\n');
        out.row(
            18,
            format_args!(
                "{{\"row\":\"diagnostic\",\"human\":{},\"json_line\":{},\"diagnostic\":{}}}",
                J(&h),
                J(&j),
                j.trim_end_matches('\n')
            ),
        )?;
    }
    Ok(())
}
fn reservations(out: &mut Output, a: &project::budget::Allocator) -> Result<(), String> {
    if a.observer_trace_overflow {
        return Err("incomplete-observation: trace limit".into());
    }
    let mut cumulative = 0usize;
    for (ordinal, e) in a.trace.iter().enumerate() {
        if matches!(e.kind, "array HIR elements" | "array literal elements") {
            if e.kind == "array HIR elements" {
                checked_add(&mut cumulative, e.length)?
            }
            out.row(17,format_args!("{{\"row\":\"reservation\",\"attempt\":{},\"kind\":{},\"length\":{},\"element_bytes\":{},\"success\":{},\"cumulative_hir_entries\":{}}}",ordinal+1,J(e.kind),e.length,e.element_bytes,e.success,cumulative))?;
        }
    }
    Ok(())
}
fn resolved(
    out: &mut Output,
    p: &resolve::ResolvedOwnedProgram<'_>,
    map: &source::SourceMap,
) -> Result<(usize, usize), String> {
    if p.functions().len() != p.signatures().len() {
        return Err("incomplete-observation: function/signature count mismatch".into());
    }
    let index = p.index();
    if p.functions().len() != index.function_count() || p.records().len() != index.record_count() {
        return Err("incomplete-observation: declaration coverage mismatch".into());
    }
    let mut es = 0usize;
    let mut elements = 0usize;
    for r in p.records() {
        out.row(5,format_args!("{{\"row\":\"record\",\"id\":{},\"declaration\":{},\"origin\":{},\"end\":{},\"fields\":{}}}",r.id.0,S(r.name_span,map),S(r.span,map),S(r.end,map),r.fields.len()))?;
        for x in &r.fields {
            out.row(6,format_args!("{{\"row\":\"field\",\"record\":{},\"id\":{},\"declaration\":{},\"origin\":{},\"scalar\":{}}}",x.id.record.0,x.id.index,S(x.name_span,map),S(x.span,map),J(scalar(x.ty))))?;
        }
    }
    for (f, s) in p.functions().iter().zip(p.signatures()) {
        let (k, m) = index.function(f.id).map_err(|_| "function identity")?;
        let ast = index.sources().ast(m).map_err(|_| "module identity")?;
        out.row(7,format_args!("{{\"row\":\"signature\",\"function\":{},\"module\":{},\"local\":{},\"declaration\":{},\"origin\":{},\"result\":{},\"parameters\":{}}}",f.id.0,m.0,k.index,S(ast.functions[k.index].name,map),S(s.span,map),V(s.result,index,map),s.params.len()))?;
        for (i, t) in s.params.iter().enumerate() {
            out.row(
                8,
                format_args!(
                    "{{\"row\":\"parameter\",\"function\":{},\"position\":{},\"type\":{}}}",
                    f.id.0,
                    i,
                    P(*t, index, map)
                ),
            )?
        }
        out.row(9,format_args!("{{\"row\":\"function\",\"id\":{},\"body\":{},\"end\":{},\"bindings\":{},\"expressions\":{},\"blocks\":{}}}",f.id.0,f.body.0,S(f.end,map),f.bindings.len(),f.expressions.len(),f.blocks.len()))?;
        for (i, b) in f.bindings.iter().enumerate() {
            out.row(10,format_args!("{{\"row\":\"binding\",\"function\":{},\"id\":{},\"origin\":{},\"annotation\":{},\"mutable\":{},\"scope\":{},\"parameter_position\":{}}}",f.id.0,i,S(b.span,map),OV(b.annotation,index,map),b.mutable,b.scope.0,OptU(b.parameter_position)))?
        }
        for (i, b) in f.blocks.iter().enumerate() {
            out.row(11,format_args!("{{\"row\":\"block\",\"function\":{},\"id\":{},\"origin\":{},\"end\":{},\"statements\":{}}}",f.id.0,i,S(b.span,map),S(b.end,map),b.body.len()))?;
            for (j, s) in b.body.iter().enumerate() {
                out.row(13,format_args!("{{\"row\":\"statement\",\"function\":{},\"block\":{},\"position\":{},\"origin\":{}, {}}}",f.id.0,i,j,S(s.span,map),ST(&s.kind,map)))?
            }
        }
        for (i, e) in f.expressions.iter().enumerate() {
            checked_add(&mut es, 1)?;
            if let ExprKind::ArrayLiteral { elements: e } = &e.kind {
                checked_add(&mut elements, e.len())?
            }
            out.row(
                12,
                format_args!(
                    "{{\"row\":\"expression\",\"function\":{},\"id\":{},\"origin\":{}, {}}}",
                    f.id.0,
                    i,
                    S(e.span, map),
                    E(&e.kind, index, map)
                ),
            )?;
        }
    }
    Ok((es, elements))
}
fn typed(
    out: &mut Output,
    p: &typeck::TypedOwnedProgram<'_>,
    map: &source::SourceMap,
) -> Result<(), String> {
    let index = p.index();
    for t in p.functions() {
        let f = t.hir();
        for i in 0..f.expressions.len() {
            out.row(14,format_args!("{{\"row\":\"typed_expression\",\"function\":{},\"id\":{},\"type\":{},\"projection\":{}}}",f.id.0,i,V(t.expression_ty(ExprId(i)),index,map),ProjectionRow(t.expression_projection(ExprId(i)))))?
        }
        for i in 0..f.bindings.len() {
            out.row(
                15,
                format_args!(
                    "{{\"row\":\"typed_binding\",\"function\":{},\"id\":{},\"type\":{}}}",
                    f.id.0,
                    i,
                    P(t.binding_ty(BindingId(i)), index, map)
                ),
            )?
        }
        for (i, b) in f.blocks.iter().enumerate() {
            out.row(
                16,
                format_args!(
                    "{{\"row\":\"typed_block\",\"function\":{},\"id\":{},\"flow\":\"{:?}\"}}",
                    f.id.0,
                    i,
                    t.block_flow(BodyBlockId(i))
                ),
            )?;
            for j in 0..b.body.len() {
                out.row(16,format_args!("{{\"row\":\"typed_statement\",\"function\":{},\"block\":{},\"position\":{},\"projection\":{}}}",f.id.0,i,j,ProjectionRow(t.statement_projection(BodyBlockId(i),j))))?
            }
        }
    }
    Ok(())
}
fn observe(
    id: &str,
    root: &Path,
    scope: &str,
    rows: usize,
    bytes: usize,
    limits: di::IndexLimits,
) -> Result<String, String> {
    if !matches!(scope, "single" | "project") {
        return Err("unknown scope".into());
    }
    let mut out = Output::new(rows, bytes);
    let d = di::IndexLimits::default();
    let limits = di::IndexLimits {
        retained: limits.retained.min(d.retained),
        scratch: limits.scratch.min(d.scratch),
        work: limits.work.min(d.work),
    };
    let (row_limit, byte_limit) = (out.rows_limit, out.bytes_limit);
    out.row(0,format_args!("{{\"row\":\"header\",\"schema\":\"oxid-array-source-typing-observation-v1\",\"case\":{},\"base\":\"2e84c9de9d9b02b62283fff1902cf1a840254ac4\",\"core_manifest_sha256\":\"f44586df897fb27b0b47684f32108f1c6d24eede7b852730308a48939992c101\",\"scope\":{},\"path\":\"private-owned-test-seam\",\"admission\":\"ObserveArrayTypes\",\"row_limit\":{},\"byte_limit\":{},\"retained_limit\":{},\"scratch_limit\":{},\"work_limit\":{},\"semantic_metric\":\"json-values-and-keys\",\"work_observation_enabled\":false}}",J(id),J(scope),row_limit,byte_limit,limits.retained,limits.scratch,limits.work))?;
    let mut a = project::budget::Allocator::default();
    out.aux(DESCRIPTOR_OVERLAP)?;
    out.aux(
        TRACE_ROWS
            .checked_mul(std::mem::size_of::<project::budget::ReserveEvent>())
            .ok_or("trace payload overflow")?,
    )?;
    a.observer_trace_bound(TRACE_ROWS)?;
    out.phase("load", "attempted")?;
    let loaded = project::ProjectSources::load_array_candidate(
        root.join("main.ox").to_str().ok_or("path utf8")?,
        project::ProjectLimits::default(),
        &mut a,
    );
    let p = match loaded {
        Ok(p) => p,
        Err(f) => {
            out.phase("load", "failed")?;
            loader_phases(&mut out, &f.allocator)?;
            out.phase("provenance", "not-attempted")?;
            for phase in ["select", "index", "resolve", "type"] {
                out.phase(phase, "not-attempted")?
            }
            let display = sources(&mut out, &f.sources, root)?;
            diagnostics(&mut out, &f.sources, &display, &f.diagnostics)?;
            reservations(&mut out, &f.allocator)?;
            return out.finish("diagnostic-complete", f.diagnostics.len());
        }
    };
    if scope == "single" && p.modules().len() != 1 {
        return Err("single scope loaded multiple files".into());
    }
    out.phase("load", "completed")?;
    loader_phases(&mut out, &a)?;
    let display = sources(&mut out, p.sources(), root)?;
    let work = di::WorkMeter::new(limits.work);
    let owner = di::SourceOwner::project(&p);
    out.phase("select", "attempted")?;
    let owned = owner.owned(&work).map_err(|_| "selector failed")?;
    out.phase("select", "completed")?;
    let mut ast_expressions = 0usize;
    let mut ast_elements = 0usize;
    let mut targets = 0usize;
    out.phase("provenance", "attempted")?;
    for (mi, module) in p.modules().iter().enumerate() {
        let ast = p.try_file_ast(module.file).ok_or("missing AST")?;
        let file = p.sources().get(module.file);
        if !ast.belongs_to(file)
            || !ast.validate_spans_and_ids_counted(|s| s.is_none_or(|s| file.try_text(s).is_some()))
        {
            return Err("invalid AST".into());
        }
        out.row(
            3,
            format_args!(
                "{{\"row\":\"selector\",\"module\":{},\"file\":{},\"owned\":{}}}",
                mi,
                module.file.0,
                ast.uses_owned_syntax(file)
            ),
        )?;
        checked_add(&mut ast_expressions, ast.expressions.len())?;
        for e in &ast.expressions {
            if let ast::ExprKind::ArrayLiteral { elements } = &e.kind {
                checked_add(&mut ast_elements, elements.len())?
            }
        }
        for f in &ast.functions {
            for b in &f.blocks {
                for s in &b.body {
                    if matches!(s.kind, ast::StmtKind::IndexAssign { .. }) {
                        checked_add(&mut targets, 1)?
                    }
                }
            }
        }
    }
    out.phase("provenance", "completed")?;
    out.row(4,format_args!("{{\"row\":\"ast_inventory\",\"ast_expression_count\":{},\"E_ast\":{},\"target_wrappers\":{},\"owned\":{},\"modules\":{}}}",ast_expressions,ast_elements,targets,owned,p.modules().len()))?;
    out.phase("index", "attempted")?;
    out.phase("resolve", "attempted")?;
    let r = match resolve::resolve_array_types(owner, limits, &work, &mut a) {
        Ok(r) => r,
        Err(ds) => {
            let reached = work.observer_phase();
            out.phase(
                "index",
                if matches!(
                    reached,
                    "record-fields" | "signatures" | "exposure" | "body-resolution" | "type"
                ) {
                    "completed"
                } else {
                    "not-completed"
                },
            )?;
            out.phase("resolve", "failed")?;
            out.phase("type", "not-attempted")?;
            diagnostics(&mut out, p.sources(), &display, &ds)?;
            reservations(&mut out, &a)?;
            out.row(19,format_args!("{{\"row\":\"summary\",\"work\":{},\"work_phase\":{},\"allocator_attempts\":{}}}",work.used(),J(reached),a.attempts))?;
            return out.finish("diagnostic-complete", ds.len());
        }
    };
    out.phase("index", "completed")?;
    let declaration_counts = (r.functions().len(), r.signatures().len(), r.records().len());
    let entry = r.entry().map(|x| x.0);
    let resolved_executable = r.admission().executable();
    let mut typed_executable = "null";
    let (es, elements) = resolved(&mut out, &r, p.sources())?;
    out.phase("resolve", "completed")?;
    out.phase("type", "attempted")?;
    let result = typeck::check(r);
    let mut ds_count = 0;
    let outcome = match result {
        Ok(t) => {
            typed_executable = if t.admission().executable() {
                "true"
            } else {
                "false"
            };
            typed(&mut out, &t, p.sources())?;
            out.phase("type", "completed")?;
            "typed-success"
        }
        Err(ds) => {
            ds_count = ds.len();
            out.phase("type", "failed")?;
            diagnostics(&mut out, p.sources(), &display, &ds)?;
            "diagnostic-complete"
        }
    };
    reservations(&mut out, &a)?;
    out.row(19,format_args!("{{\"row\":\"summary\",\"entry\":{},\"hir_expression_count\":{},\"E_hir\":{},\"ast_expression_count\":{},\"E_ast\":{},\"target_wrappers\":{},\"work\":{},\"allocator_attempts\":{},\"trace_rows\":{},\"trace_row_bytes\":{},\"work_observation_rows\":{},\"resolved_executable\":{},\"typed_executable\":{},\"functions\":{},\"signatures\":{},\"records\":{}}}",OptU(entry),es,elements,ast_expressions,ast_elements,targets,work.used(),a.attempts,a.trace.len(),std::mem::size_of::<project::budget::ReserveEvent>(),work.observations.borrow().len(),resolved_executable,typed_executable,declaration_counts.0,declaration_counts.1,declaration_counts.2))?;
    out.finish(outcome, ds_count)
}
#[test]
#[ignore]
fn array_typing_observe_requests() {
    const REQUEST_BYTES: usize = 256 * 1024;
    let request = std::env::var("OXID_ARRAY_TYPING_REQUEST").unwrap();
    let output = std::env::var("OXID_ARRAY_TYPING_OUTPUT").unwrap();
    assert!(request.len() <= 4096 && output.len() <= 4096);
    let file = std::fs::File::open(request).unwrap();
    let mut read = std::io::Read::take(file, (REQUEST_BYTES + 1) as u64);
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(REQUEST_BYTES + 1).unwrap();
    std::io::Read::read_to_end(&mut read, &mut bytes).unwrap();
    assert!(bytes.len() <= REQUEST_BYTES);
    let requests = String::from_utf8(bytes).unwrap();
    assert!(requests.lines().count() <= 1024);
    std::fs::create_dir(&output).expect("output directory must be absent");
    for (i, line) in requests.lines().enumerate() {
        let mut fields = line.split('\t');
        let id = fields.next().unwrap();
        let scope = fields.next().unwrap();
        let path = fields.next().unwrap();
        let rows = fields.next().unwrap().parse().unwrap();
        let bytes = fields.next().unwrap().parse().unwrap();
        let retained = fields.next().unwrap().parse().unwrap();
        let scratch = fields.next().unwrap().parse().unwrap();
        let work = fields.next().unwrap().parse().unwrap();
        assert!(fields.next().is_none());
        assert!(
            !id.is_empty()
                && id.len() <= 128
                && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        );
        assert!(path.len() <= 4096);
        assert!(!requests
            .lines()
            .take(i)
            .any(|l| l.split('\t').next() == Some(id)));
        let result = observe(
            id,
            Path::new(path),
            scope,
            rows,
            bytes,
            di::IndexLimits {
                retained,
                scratch,
                work,
            },
        );
        let (suffix, data) = match result {
            Ok(d) => ("jsonl", d),
            Err(e) => ("error", e),
        };
        let path = Path::new(&output).join(format!("{id}.{suffix}"));
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();
        std::io::Write::write_all(&mut f, data.as_bytes()).unwrap();
    }
}
