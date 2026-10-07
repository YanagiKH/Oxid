//! Read-only observations through existing canonical frontend views.
use super::{ast, declaration_index::SourceOwner, diagnostic, hir, lexer, parser, project, source, typeck};
use super::{comma, program_json, span};
use std::io::Read;

pub(crate) fn observe_static(text: String, project_source: bool) {
    let mut sources = source::SourceMap::new();
    let mut allocator = project::budget::Allocator::default();
    let id = sources.try_add("stdin.ox".into(), text, &mut allocator)
        .expect("bounded observer source allocation failed");
    let source = sources.get(id);
    let tokens = match lexer::lex_with_limit(source, lexer::MAX_TOKENS) {
        Ok(tokens) => tokens,
        Err(error) => { diagnostic_json("lexical_diagnostic", "lex", &error, &sources); return; }
    };
    let ast = match parser::parse_typed_counted(
        source, tokens, parser::SourceMode::ProjectCandidate, parser::MAX_NODES,
        &mut allocator, &mut parser::SyntaxStorage::default(),
    ) {
        Ok((ast, _)) => ast,
        Err(errors) => { diagnostic_json("diagnostic", "parse", &errors[0], &sources); return; }
    };
    if let Some((family, at)) = super::outside_subset(&ast) {
        print!("{{\"schema\":\"canonical-static-observation-1\",\"status\":\"outside_subset\",\"family\":\"{family}\",\"span\":");
        span(at);
        println!("}}");
        return;
    }
    // This is the exact production selector, not a spelling approximation or a
    // forced scalar resolver. No owned semantic implementation is linked here.
    if ast.uses_owned_syntax(source) {
        print!("{{\"schema\":\"canonical-static-observation-1\",\"status\":\"public_route_required\",\"route\":\"owned\",\"purpose\":\"diagnostic_only\",\"source_name\":\"stdin.ox\",\"ast\":");
        program_json(&ast);
        println!("}}");
        return;
    }
    if ast.uses_project_syntax() {
        if project_source {
            observe_project(source.text());
        } else {
            print!("{{\"schema\":\"canonical-static-observation-1\",\"status\":\"public_route_required\",\"route\":\"project_scalar\",\"purpose\":\"scalar_project_facts\",\"source_name\":\"stdin.ox\",\"ast\":");
            program_json(&ast);
            println!("}}");
        }
        return;
    }
    if project_source {
        internal_failure("--project-source requires parsed scalar project syntax");
    }
    // Resolve the entire program first. No typed facts survive either failure.
    let resolved = match hir::resolve(source, &ast) {
        Ok(program) => program,
        Err(errors) => { diagnostic_json("diagnostic", "resolve", &errors[0], &sources); return; }
    };
    let typed = match typeck::check(resolved) {
        Ok(program) => program,
        Err(errors) => { diagnostic_json("diagnostic", "type", &errors[0], &sources); return; }
    };
    typed_json(&typed, source, &ast, "scalar");
}

fn internal_failure(message: &str) -> ! {
    eprintln!("canonical static observer internal failure: {message}");
    std::process::exit(70);
}

fn observe_project(expected_text: &str) {
    // Only the controller's fresh stdin.ox is accepted. Check before loading,
    // then check the loader's genuine retained source again before resolution.
    let mut bytes = Vec::new();
    let file = std::fs::File::open("stdin.ox")
        .unwrap_or_else(|_| internal_failure("project source stdin.ox is unavailable"));
    file.take(129).read_to_end(&mut bytes)
        .unwrap_or_else(|_| internal_failure("project source stdin.ox cannot be read"));
    if bytes != expected_text.as_bytes() {
        internal_failure("project source stdin.ox differs from exact stdin bytes");
    }
    let project = project::ProjectSources::load_typed("stdin.ox", project::ProjectLimits::default())
        .unwrap_or_else(|_| internal_failure("canonical project loader rejected prevalidated source"));
    let sources = project.sources();
    let source = sources.get(source::SourceFileId(0));
    let ast = project.try_file_ast(source::SourceFileId(0))
        .unwrap_or_else(|| internal_failure("canonical project loader omitted root AST"));
    if source.path() != "stdin.ox" || source.text() != expected_text
        || project.modules().len() != 1 || !ast.uses_project_syntax()
        || ast.uses_owned_syntax(source) || super::outside_subset(ast).is_some()
    {
        internal_failure("canonical project loader source/grammar association drifted");
    }
    // Root-only public functions follow the real project owner and resolver.
    // No owner/index fields, source identities or syntax flags are fabricated.
    let resolved = match hir::resolve_sources(SourceOwner::project(&project)) {
        Ok(program) => program,
        Err(errors) => { diagnostic_json("diagnostic", "resolve", &errors[0], sources); return; }
    };
    let typed = match typeck::check(resolved) {
        Ok(program) => program,
        Err(errors) => { diagnostic_json("diagnostic", "type", &errors[0], sources); return; }
    };
    typed_json(&typed, source, ast, "project_scalar");
}

fn typed_json(typed: &typeck::TypedProgram, source: &source::SourceFile, ast: &ast::Program, route: &str) {
    print!("{{\"schema\":\"canonical-static-observation-1\",\"status\":\"ok\",\"route\":\"{route}\",\"ast\":");
    program_json(ast);
    print!(",\"typed_hir\":{{\"functions\":[");
    for (index, view) in typed.functions().enumerate() {
        comma(index);
        function_json(&view, source);
    }
    println!("]}}}}");
}

fn diagnostic_json(status: &str, phase: &str, error: &diagnostic::Diagnostic, sources: &source::SourceMap) {
    println!("{{\"schema\":\"canonical-static-observation-1\",\"status\":\"{status}\",\"phase\":\"{phase}\",\"diagnostic\":{}}}", error.render_json(sources));
}

fn ty_json(ty: hir::Ty) {
    print!("\"{ty}\"");
}

fn optional_id(id: Option<usize>) {
    match id { Some(id) => print!("{id}"), None => print!("null") }
}

fn function_json(view: &typeck::TypedFunction<'_>, source: &source::SourceFile) {
    let function = view.hir();
    let signature = view.signature();
    print!("{{\"id\":{},\"name\":{},\"signature\":{{\"span\":", function.id.0, diagnostic::json_string(source.text_at(signature.span)));
    span(signature.span);
    print!(",\"params\":[");
    for (index, ty) in signature.params.iter().enumerate() { comma(index); ty_json(*ty); }
    print!("],\"result\":");
    ty_json(signature.result);
    print!("}},\"body\":{},\"end\":", function.body.0);
    span(function.end);
    print!(",\"locals\":[");
    for (index, local) in function.locals.iter().enumerate() {
        comma(index);
        print!("{{\"id\":{index},\"name\":{},\"mutable\":{},\"span\":", diagnostic::json_string(source.text_at(local.span)), local.mutable);
        span(local.span);
        print!(",\"annotation\":");
        match local.annotation { Some(ty) => ty_json(ty), None => print!("null") }
        print!(",\"ty\":");
        ty_json(view.local_ty(hir::LocalId(index)));
        print!("}}");
    }
    print!("],\"expressions\":[");
    for (index, expression) in function.expressions.iter().enumerate() {
        comma(index);
        expression_json(index, expression, view.expression_ty(hir::ExprId(index)));
    }
    print!("],\"blocks\":[");
    for (index, block) in function.blocks.iter().enumerate() {
        comma(index);
        print!("{{\"id\":{index},\"span\":"); span(block.span);
        print!(",\"end\":"); span(block.end);
        print!(",\"flow\":"); flow_json(view.block_flow(hir::BodyBlockId(index)));
        print!(",\"body\":[");
        for (index, statement) in block.body.iter().enumerate() { comma(index); statement_json(statement); }
        print!("]}}");
    }
    print!("]}}");
}

fn expression_json(index: usize, expression: &hir::Expr, ty: hir::Ty) {
    use hir::ExprKind::*;
    print!("{{\"id\":{index},\"span\":"); span(expression.span);
    print!(",\"ty\":"); ty_json(ty);
    match &expression.kind {
        Bool(value) => print!(",\"kind\":\"Bool\",\"value\":{value}"),
        I32(value) => print!(",\"kind\":\"I32\",\"value\":{value}"),
        Unit => print!(",\"kind\":\"Unit\""),
        Local(id) => print!(",\"kind\":\"Local\",\"local\":{}", id.0),
        Group(id) => print!(",\"kind\":\"Group\",\"operand\":{}", id.0),
        Negate { operand, operator_span } | Not { operand, operator_span } => {
            let kind = if matches!(expression.kind, Negate { .. }) { "Negate" } else { "Not" };
            print!(",\"kind\":\"{kind}\",\"operand\":{},\"operator_span\":", operand.0); span(*operator_span);
        }
        Logical { op, left, right, operator_span } => {
            print!(",\"kind\":\"Logical\",\"op\":\"{op:?}\",\"left\":{},\"right\":{},\"operator_span\":", left.0, right.0); span(*operator_span);
        }
        Comparison { op, left, right, operator_span } => {
            print!(",\"kind\":\"Comparison\",\"op\":\"{op:?}\",\"left\":{},\"right\":{},\"operator_span\":", left.0, right.0); span(*operator_span);
        }
        Arithmetic { op, left, right, operator_span } => {
            print!(",\"kind\":\"Arithmetic\",\"op\":\"{op:?}\",\"left\":{},\"right\":{},\"operator_span\":", left.0, right.0); span(*operator_span);
        }
        Call { target, args } => {
            print!(",\"kind\":\"Call\",\"target\":{},\"args\":[", target.0);
            for (index, arg) in args.iter().enumerate() { comma(index); print!("{}", arg.0); }
            print!("]");
        }
    }
    print!("}}");
}

fn statement_json(statement: &hir::Stmt) {
    use hir::StmtKind::*;
    print!("{{\"span\":"); span(statement.span);
    match &statement.kind {
        Let { local, init } => print!(",\"kind\":\"Let\",\"local\":{},\"init\":{}", local.0, init.0),
        Assign { local, target_span, operator_span, value } => {
            print!(",\"kind\":\"Assign\",\"local\":{},\"target_span\":", local.0); span(*target_span);
            print!(",\"operator_span\":"); span(*operator_span); print!(",\"value\":{}", value.0);
        }
        Expr(value) => print!(",\"kind\":\"Expr\",\"value\":{}", value.0),
        Return(value) => { print!(",\"kind\":\"Return\",\"value\":"); optional_id(value.map(|id| id.0)); }
        Break { target } => print!(",\"kind\":\"Break\",\"target\":{}", target.0),
        Continue { target } => print!(",\"kind\":\"Continue\",\"target\":{}", target.0),
        While { loop_id, condition, body } => print!(",\"kind\":\"While\",\"loop_id\":{},\"condition\":{},\"body\":{}", loop_id.0, condition.0, body.0),
        If { condition, then_block, else_block } => {
            print!(",\"kind\":\"If\",\"condition\":{},\"then_block\":{},\"else_block\":", condition.0, then_block.0); optional_id(else_block.map(|id| id.0));
        }
    }
    print!("}}");
}

fn flow_json(flow: typeck::FlowSummary) {
    // Observe the actual private fields through their existing Debug surface.
    // Enumerate only the sixteen exact pinned representations. This does not
    // visit statements, infer exits, or recalculate any flow operation.
    let debug = format!("{flow:?}");
    let mask = (0u8..16).find(|mask| debug == format!(
        "FlowSummary {{ fallthrough: {}, returns: {}, breaks: {}, continues: {} }}",
        mask & 1 != 0, mask & 2 != 0, mask & 4 != 0, mask & 8 != 0,
    )).expect("canonical FlowSummary Debug format drifted");
    print!("{{\"mask\":{mask},\"fallthrough\":{},\"returns\":{},\"breaks\":{},\"continues\":{},\"debug\":{}}}",
        mask & 1 != 0, mask & 2 != 0, mask & 4 != 0, mask & 8 != 0, diagnostic::json_string(&debug));
}
