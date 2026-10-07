//! Visibility and JSON observation adapter only; canonical files are unmodified.
mod ast;
mod builtin_catalog;
mod declaration_index;
mod diagnostic;
mod hir;
mod lexer;
mod oir;
mod owned_diagnostic;
mod parser;
mod project;
mod source;

use ast::*;
use source::Span;

pub(crate) fn observe(text: String) {
    let mut sources = source::SourceMap::new();
    let mut allocator = project::budget::Allocator::default();
    let id = sources
        .try_add("stdin.ox".into(), text, &mut allocator)
        .expect("bounded observer source allocation failed");
    let tokens = match lexer::lex_with_limit(sources.get(id), lexer::MAX_TOKENS) {
        Ok(tokens) => tokens,
        Err(diagnostic) => {
            println!(
                "{{\"status\":\"lexical_diagnostic\",\"diagnostic\":{}}}",
                diagnostic.render_json(&sources)
            );
            return;
        }
    };
    let parsed = parser::parse_typed_counted(
        sources.get(id),
        tokens,
        parser::SourceMode::ProjectCandidate,
        parser::MAX_NODES,
        &mut allocator,
        &mut parser::SyntaxStorage::default(),
    );
    match parsed {
        Ok((program, _counted_nodes)) => {
            if let Some((family, at)) = outside_subset(&program) {
                print!("{{\"status\":\"outside_subset\",\"projection\":\"successful_ast_outside_subset\",\"family\":\"{family}\",\"span\":");
                span(at);
                println!("}}");
                return;
            }
            print!("{{\"status\":\"ok\",\"ast\":");
            program_json(&program);
            println!("}}");
        }
        Err(diagnostics) => {
            // Intentionally expose only the canonical first diagnostic, with
            // no inference about recovery, discarded syntax or later errors.
            let first = diagnostics
                .first()
                .expect("canonical parser returned no diagnostic");
            println!(
                "{{\"status\":\"diagnostic\",\"projection\":\"first_parser_diagnostic\",\"diagnostic\":{}}}",
                first.render_json(&sources)
            );
        }
    }
}

fn outside_type(ty: &TypeSyntax) -> Option<(&'static str, Span)> {
    let family = match ty.kind {
        TypeSyntaxKind::Name(ItemPath::Unqualified(_)) | TypeSyntaxKind::Unit => return None,
        TypeSyntaxKind::Name(ItemPath::Absolute(_)) => "qualified_type",
        TypeSyntaxKind::Reference { .. } => "reference_type",
        TypeSyntaxKind::Array(_) => "array_type",
        TypeSyntaxKind::SliceReference { .. } => "slice_reference_type",
        TypeSyntaxKind::ArrayReference { .. } => "array_reference_type",
    };
    Some((family, ty.span))
}

fn outside_subset(program: &Program) -> Option<(&'static str, Span)> {
    // This is an AST projection boundary after successful canonical parsing,
    // not the bounded consumer's grammar-site refusal/recovery algorithm.
    for item in &program.items {
        match *item {
            ItemId::Function(_) => {}
            ItemId::Module(id) => return Some(("module", program.modules[id].span)),
            ItemId::Import(id) => return Some(("import", program.imports[id].span)),
            ItemId::Struct(id) => return Some(("record", program.records[id].span)),
            ItemId::Enum(id) => return Some(("enum", program.enums[id].span)),
        }
    }
    for function in &program.functions {
        for param in &function.params {
            if let Some(outside) = outside_type(&param.ty) {
                return Some(outside);
            }
        }
        if let Some(outside) = outside_type(&function.result) {
            return Some(outside);
        }
        for block in &function.blocks {
            for statement in &block.body {
                match &statement.kind {
                    StmtKind::Let { annotation, .. } => {
                        if let Some(outside) = annotation.as_ref().and_then(outside_type) {
                            return Some(outside);
                        }
                    }
                    StmtKind::FieldAssign { .. } => {
                        return Some(("field_assignment", statement.span))
                    }
                    StmtKind::IndexAssign { .. } => {
                        return Some(("index_assignment", statement.span))
                    }
                    StmtKind::Match { .. } => return Some(("match", statement.span)),
                    StmtKind::Assign { .. }
                    | StmtKind::Expr(_)
                    | StmtKind::Return(_)
                    | StmtKind::Break
                    | StmtKind::Continue
                    | StmtKind::While { .. }
                    | StmtKind::If { .. } => {}
                }
            }
        }
    }
    for expression in &program.expressions {
        let family = match &expression.kind {
            ExprKind::Call { callee, args } => {
                if matches!(callee, ItemPath::Absolute(_)) {
                    return Some(("qualified_call", expression.span));
                }
                for arg in args {
                    if let Argument::Borrow { span, .. } = arg {
                        return Some(("borrow_argument", *span));
                    }
                }
                continue;
            }
            ExprKind::QualifiedValue { .. } => "qualified_value",
            ExprKind::StructLiteral { .. } => "record_literal",
            ExprKind::FieldRead { .. } => "field_access",
            ExprKind::ArrayLiteral { .. } => "array_literal",
            ExprKind::IndexRead { .. } => "indexing",
            ExprKind::ArrayLength { .. } => "array_length",
            ExprKind::Negate { .. }
            | ExprKind::Not { .. }
            | ExprKind::Logical { .. }
            | ExprKind::Comparison { .. }
            | ExprKind::Bool(_)
            | ExprKind::Number { .. }
            | ExprKind::Unit
            | ExprKind::Name(_)
            | ExprKind::Group(_)
            | ExprKind::Arithmetic { .. } => continue,
        };
        return Some((family, expression.span));
    }
    // Scalar syntax has no retained path arena. Check this explicitly instead
    // of silently discarding possible future producers of qualified paths.
    if let Some(path) = program.paths.first() {
        return Some(("qualified_path", path.span));
    }
    if let Some(segment) = program.path_segments.first() {
        return Some(("qualified_path_segment", *segment));
    }
    None
}

fn comma(index: usize) {
    if index != 0 {
        print!(",");
    }
}

fn span(value: Span) {
    print!(
        "{{\"file_id\":{},\"start\":{},\"end\":{}}}",
        value.file.0, value.start, value.end
    );
}

fn optional_span(value: Option<Span>) {
    match value {
        Some(value) => span(value),
        None => print!("null"),
    }
}

fn optional_expression(value: Option<ExprId>) {
    match value {
        Some(value) => print!("{}", value.0),
        None => print!("null"),
    }
}

fn type_json(ty: &TypeSyntax) {
    match ty.kind {
        TypeSyntaxKind::Name(ItemPath::Unqualified(name)) => {
            print!("{{\"kind\":\"TypeName\",\"span\":");
            span(ty.span);
            print!(",\"name\":");
            span(name);
        }
        TypeSyntaxKind::Unit => {
            print!("{{\"kind\":\"TypeUnit\",\"span\":");
            span(ty.span);
        }
        _ => unreachable!("outside-subset type passed projection check"),
    }
    print!("}}");
}

fn statement_json(statement: &Stmt) {
    print!("{{\"span\":");
    span(statement.span);
    match &statement.kind {
        StmtKind::Let {
            mutable,
            name,
            annotation,
            init,
        } => {
            print!(",\"kind\":\"Let\",\"mutable\":{mutable},\"name\":");
            span(*name);
            print!(",\"annotation\":");
            match annotation {
                Some(ty) => type_json(ty),
                None => print!("null"),
            }
            print!(",\"init\":{}", init.0);
        }
        StmtKind::Assign {
            name,
            operator_span,
            value,
        } => {
            print!(",\"kind\":\"Assign\",\"name\":");
            span(*name);
            print!(",\"operator_span\":");
            span(*operator_span);
            print!(",\"value\":{}", value.0);
        }
        StmtKind::Expr(value) => print!(",\"kind\":\"Expr\",\"value\":{}", value.0),
        StmtKind::Return(value) => {
            print!(",\"kind\":\"Return\",\"value\":");
            optional_expression(*value);
        }
        StmtKind::Break => print!(",\"kind\":\"Break\""),
        StmtKind::Continue => print!(",\"kind\":\"Continue\""),
        StmtKind::While { condition, body } => print!(
            ",\"kind\":\"While\",\"condition\":{},\"body\":{}",
            condition.0, body.0
        ),
        StmtKind::If {
            condition,
            then_block,
            else_block,
        } => {
            print!(
                ",\"kind\":\"If\",\"condition\":{},\"then_block\":{},\"else_block\":",
                condition.0, then_block.0
            );
            match else_block {
                Some(block) => print!("{}", block.0),
                None => print!("null"),
            }
        }
        StmtKind::FieldAssign { .. } | StmtKind::IndexAssign { .. } | StmtKind::Match { .. } => {
            unreachable!("outside-subset statement passed projection check")
        }
    }
    print!("}}");
}

fn expression_json(index: usize, expression: &Expr) {
    print!("{{\"id\":{index},\"span\":");
    span(expression.span);
    match &expression.kind {
        ExprKind::Negate {
            operand,
            operator_span,
        }
        | ExprKind::Not {
            operand,
            operator_span,
        } => {
            let name = if matches!(expression.kind, ExprKind::Negate { .. }) {
                "Negate"
            } else {
                "Not"
            };
            print!(
                ",\"kind\":\"{name}\",\"operand\":{},\"operator_span\":",
                operand.0
            );
            span(*operator_span);
        }
        ExprKind::Logical {
            op,
            left,
            right,
            operator_span,
        } => {
            print!(",\"kind\":\"Logical\",\"op\":\"{op:?}\",\"left\":{},\"right\":{},\"operator_span\":", left.0, right.0);
            span(*operator_span);
        }
        ExprKind::Comparison {
            op,
            left,
            right,
            operator_span,
        } => {
            print!(",\"kind\":\"Comparison\",\"op\":\"{op:?}\",\"left\":{},\"right\":{},\"operator_span\":", left.0, right.0);
            span(*operator_span);
        }
        ExprKind::Arithmetic {
            op,
            left,
            right,
            operator_span,
        } => {
            print!(",\"kind\":\"Arithmetic\",\"op\":\"{op:?}\",\"left\":{},\"right\":{},\"operator_span\":", left.0, right.0);
            span(*operator_span);
        }
        ExprKind::Bool(value) => print!(",\"kind\":\"Bool\",\"value\":{value}"),
        ExprKind::Number { digits, negative } => {
            print!(",\"kind\":\"Number\",\"digits\":");
            span(*digits);
            print!(",\"negative\":{negative}");
        }
        ExprKind::Unit => print!(",\"kind\":\"Unit\""),
        ExprKind::Name(name) => {
            print!(",\"kind\":\"Name\",\"name\":");
            span(*name);
        }
        ExprKind::Group(operand) => print!(",\"kind\":\"Group\",\"operand\":{}", operand.0),
        ExprKind::Call {
            callee: ItemPath::Unqualified(callee),
            args,
        } => {
            print!(",\"kind\":\"Call\",\"callee\":");
            span(*callee);
            print!(",\"args\":[");
            for (index, argument) in args.iter().enumerate() {
                comma(index);
                match argument {
                    Argument::Value(value) => print!("{}", value.0),
                    Argument::Borrow { .. } => {
                        unreachable!("outside-subset argument passed projection check")
                    }
                }
            }
            print!("]");
        }
        ExprKind::Call {
            callee: ItemPath::Absolute(_),
            ..
        }
        | ExprKind::QualifiedValue { .. }
        | ExprKind::StructLiteral { .. }
        | ExprKind::FieldRead { .. }
        | ExprKind::ArrayLiteral { .. }
        | ExprKind::IndexRead { .. }
        | ExprKind::ArrayLength { .. } => {
            unreachable!("outside-subset expression passed projection check")
        }
    }
    print!("}}");
}

fn program_json(program: &Program) {
    print!("{{\"tokens\":[");
    for (index, token) in program.tokens.iter().enumerate() {
        comma(index);
        print!(
            "{{\"id\":{},\"kind\":\"{:?}\",\"file_id\":{},\"start\":{},\"end\":{}}}",
            token.kind as usize + 1,
            token.kind,
            token.span.file.0,
            token.span.start,
            token.span.end
        );
    }
    print!("],\"items\":[");
    for (index, item) in program.items.iter().enumerate() {
        comma(index);
        match item {
            ItemId::Function(id) => print!("{{\"kind\":\"Function\",\"id\":{id}}}"),
            _ => unreachable!("outside-subset item passed projection check"),
        }
    }
    print!("],\"functions\":[");
    for (index, function) in program.functions.iter().enumerate() {
        comma(index);
        print!("{{\"id\":{index},\"public\":");
        optional_span(function.public);
        print!(",\"name\":");
        span(function.name);
        print!(",\"params\":[");
        for (index, param) in function.params.iter().enumerate() {
            comma(index);
            print!("{{\"name\":");
            span(param.name);
            print!(",\"ty\":");
            type_json(&param.ty);
            print!("}}");
        }
        print!("],\"result\":");
        type_json(&function.result);
        print!(",\"body\":{},\"blocks\":[", function.body.0);
        for (index, block) in function.blocks.iter().enumerate() {
            comma(index);
            print!("{{\"id\":{index},\"span\":");
            span(block.span);
            print!(",\"end\":");
            span(block.end);
            print!(",\"body\":[");
            for (index, statement) in block.body.iter().enumerate() {
                comma(index);
                statement_json(statement);
            }
            print!("]}}");
        }
        print!("],\"end\":");
        span(function.end);
        print!("}}");
    }
    print!("],\"expressions\":[");
    for (index, expression) in program.expressions.iter().enumerate() {
        comma(index);
        expression_json(index, expression);
    }
    print!("]}}");
}
