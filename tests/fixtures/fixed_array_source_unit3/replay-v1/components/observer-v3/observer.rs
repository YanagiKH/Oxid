//! Independent, source-only Unit3A observer. This file is test-only.
use super::oir::owned_types::{AggregateTy, BorrowKind, ParameterTy, ValueTy};
use super::{ast, declaration_index as di, diagnostic, hir, project, source};
use std::{
    fmt::{self, Write},
    path::Path,
};

const REVIEWED_SOURCE: &str = "aace15bc61711ce9f7731c435f5618eb9b6a1118da4f226e9172b4b053891624";
const MAX_ROWS: usize = 200_000;
const MAX_BYTES: usize = 16 * 1024 * 1024;
struct Counter(usize);
impl Write for Counter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0 = self.0.checked_add(s.len()).ok_or(fmt::Error)?;
        Ok(())
    }
}
struct Output {
    text: String,
    rows: usize,
    row_limit: usize,
    byte_limit: usize,
}
impl Output {
    fn new(rows: usize, bytes: usize) -> Self {
        Self {
            text: String::new(),
            rows: 0,
            row_limit: rows.min(MAX_ROWS),
            byte_limit: bytes.min(MAX_BYTES),
        }
    }
    fn row(&mut self, a: fmt::Arguments<'_>) -> Result<(), String> {
        let mut count = Counter(0);
        count.write_fmt(a).map_err(|_| "observer byte overflow")?;
        let bytes = self
            .text
            .len()
            .checked_add(count.0)
            .and_then(|x| x.checked_add(1))
            .ok_or("observer byte overflow")?;
        let rows = self.rows.checked_add(1).ok_or("observer row overflow")?;
        if bytes > self.byte_limit || rows > self.row_limit {
            return Err("incomplete-observation: observer limit exceeded".into());
        }
        self.text
            .try_reserve_exact(count.0 + 1)
            .map_err(|_| "incomplete-observation: allocation failed")?;
        self.text
            .write_fmt(a)
            .map_err(|_| "observer formatting failed")?;
        self.text.push('\n');
        self.rows = rows;
        Ok(())
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
struct JDebug(super::lexer::Kind);
impl fmt::Display for JDebug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\"{:?}\"", self.0)
    }
}
struct JLine<'a>(&'a str);
impl fmt::Display for JLine<'_> {
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
        f.write_str("\\n\"")
    }
}
struct S(source::Span);
impl fmt::Display for S {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}, {}, {}]", self.0.file.0, self.0.start, self.0.end)
    }
}
struct E<'a> {
    e: &'a ast::Expr,
    is_target: bool,
}
impl fmt::Display for E<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.e.kind {
            ast::ExprKind::ArrayLiteral { elements } => {
                f.write_str("\"kind\":\"ArrayLiteral\",\"elements\":[")?;
                for (i, id) in elements.iter().enumerate() {
                    if i > 0 {
                        f.write_char(',')?
                    }
                    write!(f, "{}", id.0)?;
                }
                write!(f, "],\"element_count\":{}", elements.len())
            }
            ast::ExprKind::IndexRead { base, index } => write!(
                f,
                "\"kind\":\"IndexRead\",\"base\":{},\"index\":{},\"role\":{}",
                S(*base),
                index.0,
                J(if self.is_target {
                    "store-target-wrapper"
                } else {
                    "value"
                })
            ),
            ast::ExprKind::ArrayLength { base } => {
                write!(f, "\"kind\":\"ArrayLength\",\"base\":{}", S(*base))
            }
            ast::ExprKind::Group(child) => write!(f, "\"kind\":\"Group\",\"child\":{}", child.0),
            ast::ExprKind::FieldRead { base, field } => write!(
                f,
                "\"kind\":\"FieldRead\",\"base\":{},\"field\":{}",
                S(*base),
                S(*field)
            ),
            ast::ExprKind::Call { args, .. } => {
                write!(f, "\"kind\":\"Call\",\"arguments\":{}", args.len())
            }
            _ => {
                let kind = match self.e.kind {
                    ast::ExprKind::Bool(_) => "Bool",
                    ast::ExprKind::Unit => "Unit",
                    ast::ExprKind::Number { .. } => "Number",
                    ast::ExprKind::Name(_) => "Name",
                    ast::ExprKind::Not { .. } => "Not",
                    ast::ExprKind::Logical { .. } => "Logical",
                    ast::ExprKind::Comparison { .. } => "Comparison",
                    ast::ExprKind::Arithmetic { .. } => "Arithmetic",
                    ast::ExprKind::StructLiteral { .. } => "StructLiteral",
                    _ => unreachable!(),
                };
                write!(f, "\"kind\":{}", J(kind))
            }
        }
    }
}
fn scalar(t: hir::Ty) -> &'static str {
    match t {
        hir::Ty::Bool => "bool",
        hir::Ty::I32 => "i32",
        hir::Ty::Unit => "unit",
    }
}
struct V<'a, 's> {
    ty: ValueTy,
    index: &'a di::DeclarationIndex<'s>,
}
impl fmt::Display for V<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.ty {
            ValueTy::Scalar(t) => write!(f, "{{\"tag\":\"scalar\",\"name\":{}}}", J(scalar(t))),
            ValueTy::Owned(AggregateTy::FixedArray(a)) => write!(
                f,
                "{{\"tag\":\"fixed_array\",\"element\":{},\"length\":{}}}",
                J(scalar(a.element())),
                a.length()
            ),
            ValueTy::Owned(AggregateTy::Record(r)) => {
                let (k, m) = self.index.record(r).map_err(|_| fmt::Error)?;
                let p = self.index.sources().ast(m).map_err(|_| fmt::Error)?;
                write!(
                    f,
                    "{{\"tag\":\"record\",\"declaration\":{},\"record_id\":{}}}",
                    S(p.records[k.index].name),
                    r.0
                )
            }
        }
    }
}
struct P<'a, 's> {
    ty: ParameterTy,
    index: &'a di::DeclarationIndex<'s>,
}
impl fmt::Display for P<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.ty {
            ParameterTy::Value(ty) => write!(
                f,
                "{}",
                V {
                    ty,
                    index: self.index
                }
            ),
            ParameterTy::Reference { aggregate, kind } => write!(
                f,
                "{{\"tag\":\"reference\",\"kind\":{},\"aggregate\":{}}}",
                J(match kind {
                    BorrowKind::Shared => "shared",
                    BorrowKind::Exclusive => "exclusive",
                }),
                V {
                    ty: ValueTy::Owned(aggregate),
                    index: self.index
                }
            ),
        }
    }
}
fn paths_and_sources(
    out: &mut Output,
    map: &source::SourceMap,
    root: &Path,
) -> Result<source::SourceMap, String> {
    let mut display = source::SourceMap::new();
    let mut display_bytes = 0usize;
    for (i, f) in map.files().iter().enumerate() {
        let relative = Path::new(f.path())
            .strip_prefix(root)
            .map_err(|_| "source outside fixture")?
            .to_str()
            .ok_or("path utf8")?;
        out.row(format_args!("{{\"row\":\"source\",\"file\":{},\"identity\":{},\"path\":{},\"original_path\":{},\"text\":{}}}",i,f.identity(),J(relative),J(f.path()),J(f.text())))?;
        display_bytes = display_bytes
            .checked_add(relative.len())
            .and_then(|n| n.checked_add(f.text().len()))
            .ok_or("display map overflow")?;
        if display_bytes > MAX_BYTES {
            return Err("incomplete-observation: display map bytes".into());
        }
        display.add(relative.into(), f.text().into());
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
        for s in d.primary.iter().chain(d.secondary.iter().map(|x| &x.0)) {
            if map.try_text(*s).is_none() {
                return Err("invalid diagnostic span".into());
            }
        }
        // Production renderer is used only after original-map validation. Presentation map preserves IDs/bytes.
        let mut bound = 4096usize;
        for text in [d.code, d.stage, d.message.as_str()]
            .into_iter()
            .chain(d.notes.iter().map(String::as_str))
            .chain(d.secondary.iter().map(|x| x.1.as_str()))
            .chain(display.files().iter().map(|x| x.path()))
        {
            bound = bound
                .checked_add(text.len().checked_mul(36).ok_or("render overflow")?)
                .ok_or("render overflow")?;
        }
        if bound > out.byte_limit.saturating_sub(out.text.len()) {
            return Err("incomplete-observation: diagnostic preflight".into());
        }
        let h = d.render_human(display);
        let j = d.render_json(display);
        out.row(format_args!(
            "{{\"row\":\"diagnostic\",\"human\":{},\"json_line\":{},\"diagnostic\":{}}}",
            J(&h),
            JLine(&j),
            j
        ))?;
    }
    Ok(())
}
fn type_use(
    out: &mut Output,
    index: &di::DeclarationIndex<'_>,
    work: &di::WorkMeter,
    module: project::ModuleId,
    ty: ast::TypeSyntax,
    parameter: bool,
    site: &str,
    values: &mut Vec<(source::Span, ValueTy)>,
) -> Result<(), String> {
    if let ast::TypeSyntaxKind::Array(array) | ast::TypeSyntaxKind::ArrayReference { array, .. } =
        ty.kind
    {
        let (kind, mutable) = match ty.kind {
            ast::TypeSyntaxKind::Array(_) => ("Array", false),
            ast::TypeSyntaxKind::ArrayReference { mutable, .. } => ("ArrayReference", mutable),
            _ => unreachable!(),
        };
        let element = match array.element {
            ast::ScalarTypeSyntax::Bool => "bool",
            ast::ScalarTypeSyntax::I32 => "i32",
            ast::ScalarTypeSyntax::Unit => "unit",
        };
        out.row(format_args!("{{\"row\":\"type_syntax\",\"kind\":{},\"site\":{},\"origin\":{},\"element\":{},\"length\":{},\"mutable\":{}}}", J(kind), J(site), S(ty.span), J(element), array.length, mutable))?;
    }
    if parameter {
        let p = index
            .query(work)
            .parameter_type(module, ty)
            .map_err(|e| format!("type query: {e:?}"))?;
        out.row(format_args!("{{\"row\":\"query\",\"query_kind\":\"parameter\",\"site\":{},\"origin\":{},\"value\":{}}}",J(site),S(ty.span),P{ty:p,index}))?;
    }
    if !matches!(
        ty.kind,
        ast::TypeSyntaxKind::Reference { .. } | ast::TypeSyntaxKind::ArrayReference { .. }
    ) {
        let v = index
            .query(work)
            .value_type(module, ty, di::TypeContext::Value)
            .map_err(|e| format!("type query: {e:?}"))?;
        out.row(format_args!(
            "{{\"row\":\"query\",\"query_kind\":\"value\",\"site\":{},\"origin\":{},\"value\":{}}}",
            J(site),
            S(ty.span),
            V { ty: v, index }
        ))?;
        if matches!(v, ValueTy::Owned(_)) {
            if values
                .len()
                .checked_add(1)
                .and_then(|n| n.checked_mul(std::mem::size_of::<(source::Span, ValueTy)>()))
                .is_none_or(|n| n > MAX_BYTES)
                || values.len() >= MAX_ROWS
            {
                return Err("incomplete-observation: value rows".into());
            }
            values.try_reserve_exact(1).map_err(|_| "value reserve")?;
            values.push((ty.span, v));
        }
    }
    Ok(())
}
fn reservations(out: &mut Output, allocator: &project::budget::Allocator) -> Result<(), String> {
    for e in &allocator.trace {
        if e.kind == "array literal elements" {
            out.row(format_args!("{{\"row\":\"reservation\",\"kind\":{},\"length\":{},\"element_bytes\":{},\"success\":{}}}",J(e.kind),e.length,e.element_bytes,e.success))?;
        }
    }
    Ok(())
}
fn observe(
    root: &Path,
    scope: &str,
    row_limit: usize,
    byte_limit: usize,
) -> Result<String, String> {
    if !matches!(scope, "single" | "project") {
        return Err("unknown scope".into());
    }
    let mut out = Output::new(row_limit, byte_limit);
    let (rl, bl) = (out.row_limit, out.byte_limit);
    out.row(format_args!("{{\"row\":\"header\",\"schema\":\"oxid-array-source-observation-v1\",\"scope\":{},\"reviewed_source_archive\":{},\"row_limit\":{},\"byte_limit\":{}}}",J(scope),J(REVIEWED_SOURCE),rl,bl))?;
    let mut allocator = project::budget::Allocator::default();
    let loaded = project::ProjectSources::load_array_candidate(
        root.join("main.ox").to_str().ok_or("path utf8")?,
        project::ProjectLimits::default(),
        &mut allocator,
    );
    let p = match loaded {
        Ok(p) => p,
        Err(f) => {
            let display = paths_and_sources(&mut out, &f.sources, root)?;
            diagnostics(&mut out, &f.sources, &display, &f.diagnostics)?;
            reservations(&mut out, &f.allocator)?;
            let (rows_before, bytes_before) = (out.rows, out.text.len());
            out.row(format_args!("{{\"row\":\"complete\",\"complete\":true,\"phase\":\"diagnostic\",\"rows_before_footer\":{},\"bytes_before_footer\":{}}}",rows_before,bytes_before))?;
            return Ok(out.text);
        }
    };
    if scope == "single" && p.modules().len() != 1 {
        return Err("single scope loaded multiple files".into());
    }
    let _display = paths_and_sources(&mut out, p.sources(), root)?;
    let work = di::WorkMeter::default();
    let owner = di::SourceOwner::project(&p);
    let owned = owner.owned(&work).map_err(|e| format!("selector: {e:?}"))?;
    let facts = di::collect_originals(owner, di::IndexLimits::default(), &work, &mut allocator)
        .map_err(|e| format!("collection: {e:?}"))?;
    let index = facts
        .finish(&work, &mut allocator)
        .map_err(|e| format!("index: {e:?}"))?;
    let mut values = Vec::new();
    let mut elements = 0usize;
    let mut targets = 0usize;
    let mut exprs = 0usize;
    let mut array_nodes = 0usize;
    let mut visits = 0usize;
    let mut declared_records = 0usize;
    for (i, module) in p.modules().iter().enumerate() {
        let ast = p.try_file_ast(module.file).ok_or("missing AST")?;
        let file = p.sources().get(module.file);
        declared_records += ast.records.len();
        if !ast.belongs_to(file)
            || !ast.validate_spans_and_ids_counted(|s| {
                visits += 1;
                s.is_none_or(|s| s.file == module.file && file.try_text(s).is_some())
            })
        {
            return Err("invalid AST".into());
        }
        out.row(format_args!(
            "{{\"row\":\"selector\",\"file\":{},\"owned\":{}}}",
            i,
            ast.uses_owned_syntax(file)
        ))?;
        for token in &ast.tokens {
            out.row(format_args!(
                "{{\"row\":\"token\",\"kind\":{},\"span\":{}}}",
                JDebug(token.kind),
                S(token.span)
            ))?;
        }
        let mut target_flags = Vec::new();
        if ast
            .expressions
            .len()
            .checked_mul(std::mem::size_of::<bool>())
            .is_none_or(|n| n > MAX_BYTES)
        {
            return Err("incomplete-observation: target scratch".into());
        }
        target_flags
            .try_reserve_exact(ast.expressions.len())
            .map_err(|_| "target scratch reserve")?;
        target_flags.resize(ast.expressions.len(), false);
        for (fi, function) in ast.functions.iter().enumerate() {
            out.row(format_args!(
                "{{\"row\":\"function\",\"file\":{},\"index\":{},\"name\":{},\"parameters\":{}}}",
                i,
                fi,
                S(function.name),
                function.params.len()
            ))?;
            type_use(
                &mut out,
                &index,
                &work,
                project::ModuleId(i),
                function.result,
                false,
                "result",
                &mut values,
            )?;
            for param in &function.params {
                type_use(
                    &mut out,
                    &index,
                    &work,
                    project::ModuleId(i),
                    param.ty,
                    true,
                    "parameter",
                    &mut values,
                )?;
            }
            for (bi, block) in function.blocks.iter().enumerate() {
                for (si, stmt) in block.body.iter().enumerate() {
                    match stmt.kind {
                        ast::StmtKind::Let {
                            annotation: Some(ty),
                            ..
                        } => type_use(
                            &mut out,
                            &index,
                            &work,
                            project::ModuleId(i),
                            ty,
                            false,
                            "annotation",
                            &mut values,
                        )?,
                        ast::StmtKind::IndexAssign {
                            target,
                            operator_span,
                            value,
                        } => {
                            target_flags[target.0] = true;
                            targets += 1;
                            array_nodes += 1;
                            out.row(format_args!("{{\"row\":\"statement\",\"file\":{},\"function\":{},\"block\":{},\"index\":{},\"kind\":\"IndexAssign\",\"origin\":{},\"target\":{},\"operator\":{},\"value\":{}}}",i,fi,bi,si,S(stmt.span),target.0,S(operator_span),value.0))?;
                        }
                        _ => (),
                    }
                }
            }
        }
        for (eid, e) in ast.expressions.iter().enumerate() {
            exprs += 1;
            let is_target = target_flags[eid];
            match &e.kind {
                ast::ExprKind::ArrayLiteral { elements: ids } => {
                    elements += ids.len();
                    array_nodes += 1
                }
                ast::ExprKind::IndexRead { .. } | ast::ExprKind::ArrayLength { .. } => {
                    array_nodes += 1
                }
                _ => (),
            }
            out.row(format_args!(
                "{{\"row\":\"expression\",\"file\":{},\"id\":{},\"origin\":{}, {}}}",
                i,
                eid,
                S(e.span),
                E { e, is_target }
            ))?;
        }
    }
    for (a, (sa, va)) in values.iter().enumerate() {
        for (sb, vb) in values.iter().skip(a) {
            out.row(format_args!(
                "{{\"row\":\"equality\",\"a\":{},\"b\":{},\"equal\":{}}}",
                S(*sa),
                S(*sb),
                va == vb
            ))?;
        }
    }
    reservations(&mut out, &allocator)?;
    let entry = index.root_original_main().map(|id| {
        let (k, m) = index.function(id).unwrap();
        index.sources().ast(m).unwrap().functions[k.index].name
    });
    let entry = entry
        .map(|s| format!("{}", S(s)))
        .unwrap_or_else(|| "null".into());
    let inv = p.inventory().ok_or("inventory overflow")?;
    out.row(format_args!("{{\"row\":\"summary\",\"owned\":{},\"modules\":{},\"entry\":{},\"array_literal_elements\":{},\"array_store_target_wrappers\":{},\"ast_expression_count\":{},\"array_node_count\":{},\"ast_payload\":{},\"syntax_nodes\":{},\"non_eof_tokens\":{},\"source_bytes\":{},\"validation_visits\":{},\"work\":{},\"records\":{},\"declared_records\":{}}}",owned,p.modules().len(),entry,elements,targets,exprs,array_nodes,inv.ast_payload,p.usage().syntax_nodes,p.usage().non_eof_tokens,p.usage().source_bytes,visits,work.used(),index.record_count(),declared_records))?;
    let (rows_before, bytes_before) = (out.rows, out.text.len());
    out.row(format_args!("{{\"row\":\"complete\",\"complete\":true,\"phase\":\"source-types\",\"rows_before_footer\":{},\"bytes_before_footer\":{}}}",rows_before,bytes_before))?;
    Ok(out.text)
}
#[test]
#[ignore]
fn unit3a_independent_observe_requests() {
    let request = std::env::var("OXID_UNIT3A_REQUEST").unwrap();
    let output = std::env::var("OXID_UNIT3A_OUTPUT").unwrap();
    const MAX_REQUEST_BYTES: usize = 256 * 1024;
    let file = std::fs::File::open(request).unwrap();
    let mut bounded = std::io::Read::take(file, (MAX_REQUEST_BYTES + 1) as u64);
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(MAX_REQUEST_BYTES + 1).unwrap();
    std::io::Read::read_to_end(&mut bounded, &mut bytes).unwrap();
    assert!(
        bytes.len() < MAX_REQUEST_BYTES,
        "observer request size limit"
    );
    let requests = String::from_utf8(bytes).unwrap();
    let mut seen = std::collections::BTreeSet::new();
    std::fs::create_dir(&output).expect("observer output directory must be absent");
    for line in requests.lines() {
        let v: Vec<_> = line.split('\t').collect();
        assert_eq!(v.len(), 5);
        assert!(seen.insert(v[0]));
        assert!(v[0].bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'));
        let result = observe(
            Path::new(v[2]),
            v[1],
            v[3].parse().unwrap(),
            v[4].parse().unwrap(),
        );
        let (suffix, data) = match result {
            Ok(data) => ("jsonl", data),
            Err(error) => ("error", error),
        };
        let path = Path::new(&output).join(format!("{}.{}", v[0], suffix));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .expect("observer artifact must be new");
        std::io::Write::write_all(&mut file, data.as_bytes()).unwrap();
    }
}
