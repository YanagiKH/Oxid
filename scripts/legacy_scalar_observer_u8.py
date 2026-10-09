#!/usr/bin/env python3
"""Exact, reversible u8-era wiring for frozen legacy scalar observer wrappers.

Canonical frontend bytes and the historical observer fixtures are never edited.
Conversion is explicitly outside the legacy domain, not projected as a scalar.
Unrecognized wrapper revisions or enum additions must fail closed.
"""
import hashlib

SCHEMA = "legacy-scalar-observer-u8-compatibility-1"
PINS = {
    "parser/frontend_mod.rs": "01572fab9c8cfc133db8418f4c767f9078f34be95d08bb456b43ed94e46bc1a1",
    "parser/main.rs": "384d007dab5a0562a66eac087d33ac54b2712dff5be06960dc692687519b7704",
    "parser/oir_mod.rs": "a41583a4ae61fabc62fc798687f20d281cea896a7f1c4fcb2736d42effb4dc3b",
    "static/frontend_mod.rs": "ec0b88c14793809db7683cc178cf40e0f51926d66e3b750418d1f4552823fd03",
    "static/main.rs": "08e1b7ef3763af12520523870babc3e432d8fad0e1982017c04579e0a2ede225",
    "static/oir_mod.rs": "a41583a4ae61fabc62fc798687f20d281cea896a7f1c4fcb2736d42effb4dc3b",
    "static/static_observer.rs": "a06a3b5ed11ed47d1f2b81b1fa779f3f032c77ef1b8ebbc009ce924a6f6d7057",
}
DERIVED_PINS = {
    "parser/frontend_mod.rs": "60af59217a767ed80c36a8ae35cb4d90dfc5066c01379f0614eb0f1987566d90",
    "parser/main.rs": "384d007dab5a0562a66eac087d33ac54b2712dff5be06960dc692687519b7704",
    "parser/oir_mod.rs": "a41583a4ae61fabc62fc798687f20d281cea896a7f1c4fcb2736d42effb4dc3b",
    "static/frontend_mod.rs": "4ef32a6840fb6ed3216f739cfe0c7c06efd5a00f01f5733b78e3b294bc498b33",
    "static/main.rs": "08e1b7ef3763af12520523870babc3e432d8fad0e1982017c04579e0a2ede225",
    "static/oir_mod.rs": "a41583a4ae61fabc62fc798687f20d281cea896a7f1c4fcb2736d42effb4dc3b",
    "static/static_observer.rs": "ab372ffdb7d03d39df6170a0cbc8584fcaabb6331157a1bf631c617c2b6671bf",
}

# Inspect actual type nodes only; a value/function named u8 stays in-domain.
U8_TYPE_BOUNDARY = """fn outside_u8_type(program: &ast::Program, source: &source::SourceFile) -> Option<(&'static str, source::Span)> {
    let check = |ty: &ast::TypeSyntax| match ty.kind {
        ast::TypeSyntaxKind::Name(ast::ItemPath::Unqualified(name))
            if source.text_at(name) == "u8" => Some(("u8_type", ty.span)),
        _ => None,
    };
    for function in &program.functions {
        for param in &function.params {
            if let Some(outside) = check(&param.ty) { return Some(outside); }
        }
        if let Some(outside) = check(&function.result) { return Some(outside); }
        for block in &function.blocks {
            for statement in &block.body {
                if let ast::StmtKind::Let { annotation: Some(ty), .. } = &statement.kind {
                    if let Some(outside) = check(ty) { return Some(outside); }
                }
            }
        }
    }
    None
}

"""

AST_EDITS = (
    (
        '            ExprKind::QualifiedValue { .. } => "qualified_value",',
        '            ExprKind::Conversion { .. } => "conversion",\n            ExprKind::QualifiedValue { .. } => "qualified_value",',
    ),
    (
        '        | ExprKind::QualifiedValue { .. }\n',
        '        | ExprKind::Conversion { .. }\n        | ExprKind::QualifiedValue { .. }\n',
    ),
)
HIR_EDITS = (
    (
        '    if let Some((family, at)) = super::outside_subset(&ast) {',
        '    if let Some((family, at)) = super::outside_subset(&ast).or_else(|| outside_u8_type(&ast, source)) {',
    ),
    (
        'fn internal_failure(message: &str) -> ! {',
        U8_TYPE_BOUNDARY + 'fn internal_failure(message: &str) -> ! {',
    ),
    (
        '    match &expression.kind {\n',
        '    match &expression.kind {\n        Conversion { .. } => unreachable!("conversion passed legacy scalar admission"),\n',
    ),
)


def adapt_wrapper(kind, name, original):
    key = kind + "/" + name
    if key not in PINS:
        raise ValueError("unrecognized legacy observer wrapper: " + key)
    if hashlib.sha256(original).hexdigest() != PINS[key]:
        raise ValueError("frozen legacy observer wrapper identity changed: " + key)
    edits = HIR_EDITS if name == "static_observer.rs" else AST_EDITS if name == "frontend_mod.rs" else ()
    data = original
    for before, after in edits:
        before, after = before.encode(), after.encode()
        if data.count(before) != 1 or after in data:
            raise ValueError("legacy observer adaptation context changed: " + key)
        data = data.replace(before, after)
    restored = data
    for before, after in reversed(edits):
        if restored.count(after.encode()) != 1:
            raise ValueError("legacy observer adaptation is not reversible: " + key)
        restored = restored.replace(after.encode(), before.encode())
    if restored != original:
        raise ValueError("legacy observer adaptation changed frozen bytes: " + key)
    if hashlib.sha256(data).hexdigest() != DERIVED_PINS[key]:
        raise ValueError("legacy observer derived wrapper identity changed: " + key)
    return data
