//! RFC0031 declaration/type controls. No consumer result supplies an expectation.
use super::*;
use crate::frontend::oir::owned_types::EnumId;
use crate::frontend::{lexer, parser};

fn check(text: &str) -> Result<(), Vec<Diagnostic>> {
    let mut sources = SourceMap::new();
    let file = sources.add("byte-types.ox".into(), text.into());
    let source = sources.get(file);
    let (ast, _) = parser::parse_typed_counted(
        source,
        lexer::lex(source).map_err(|error| vec![*error])?,
        parser::SourceMode::ProjectCandidate,
        parser::MAX_NODES,
        &mut Allocator::default(),
        &mut parser::SyntaxStorage::default(),
    )?;
    super::super::typeck::check(resolve(source, &ast)?).map(drop)
}

#[test]
fn byte_record_source_unused_nested_and_empty_fields_are_admitted() {
    for ty in ["u8", "[u8;0]", "[u8;1]", "[u8;1024]"] {
        for declarations in [
            format!("struct Unused {{ bytes:{ty} }}"),
            format!("struct Outer {{ inner:Inner }} struct Inner {{ bytes:{ty} }}"),
            format!("struct Inner {{ bytes:{ty} }} struct Outer {{ inner:Inner }}"),
        ] {
            let text = format!("{declarations} fn main()->i32{{return 0;}}");
            assert!(check(&text).is_ok(), "{text}");
        }
    }
    let errors = check("enum E { V(u8) } fn main()->i32{return 0;}").unwrap_err();
    assert_eq!(
        (errors[0].code, errors[0].stage, errors[0].message.as_str()),
        (
            "E0100",
            "parse",
            "only bool, i32 and () enum payloads are supported"
        )
    );
}

#[test]
fn byte_storage_source_literal_inference_is_exact_and_empty_context_is_local_only() {
    const BYTE: &str = "fn byte(x:i32)->u8{return x.to_u8_checked();}";
    for body in [
        "let a:[u8;2]=[byte(0),byte(255)];return a.len();",
        "let a=[byte(0),byte(127),byte(128),byte(255)];return a.len();",
        "let a:[u8;0000]=([]);return a.len();",
        "let a:[u8;0]=[];let b=a;return b.len();",
    ] {
        assert!(
            check(&format!("{BYTE} fn main()->i32{{{body}}}")).is_ok(),
            "{body}"
        );
    }
    for (body, target, message) in [
        (
            "let a:[u8;2]=[0,255];return 0;",
            "[0,255]",
            "type mismatch: expected [u8; 2], found [i32; 2]",
        ),
        (
            "let a=[byte(0),1];return 0;",
            "1]",
            "type mismatch: expected u8, found i32",
        ),
        (
            "let a=[0,byte(255)];return 0;",
            "byte(255)",
            "type mismatch: expected i32, found u8",
        ),
        (
            "let a=[byte(0)];let i=byte(0);a[i];return 0;",
            "i]",
            "type mismatch: expected i32, found u8",
        ),
        (
            "let a=[];return 0;",
            "[]",
            "empty array literal requires an explicit array annotation on its local initializer",
        ),
    ] {
        let text = format!("{BYTE} fn main()->i32{{{body}}}");
        let errors = check(&text).unwrap_err();
        assert_eq!(
            (errors[0].code, errors[0].stage, errors[0].message.as_str()),
            ("E0300", "type", message),
            "{body}"
        );
        let start = text.rfind(target).unwrap();
        let length = target.len() - usize::from(target == "1]" || target == "i]");
        assert_eq!(
            errors[0].primary.map(|s| (s.start, s.end)),
            Some((start, start + length)),
            "{body}"
        );
    }
    for text in [
        format!("{BYTE} fn bad()->[u8;0]{{return [];}} fn main()->i32{{return 0;}}"),
        format!("{BYTE} fn f(a:[u8;0])->(){{return;}} fn main()->i32{{f([]);return 0;}}"),
        format!("{BYTE} fn main()->i32{{let mut a:[u8;0]=[];a=[];return 0;}}"),
    ] {
        let errors = check(&text).unwrap_err();
        assert_eq!((errors[0].code, errors[0].stage, errors[0].message.as_str()), ("E0300", "type", "empty array literal requires an explicit array annotation on its local initializer"));
    }
}

#[test]
fn byte_storage_source_scalar_record_controls_and_standalone_signatures_coexist() {
    for element in ["bool", "i32", "u8", "()"] {
        let text = format!("struct R{{a:[{element};0]}} fn take(a:[u8;0])->[u8;0]{{return a;}} fn read(a:&[u8])->i32{{return a.len();}} fn exact(a:&[u8;0])->i32{{return a.len();}} fn main()->i32{{let a:[u8;0]=[];return read(&a)+exact(&a);}}");
        assert!(check(&text).is_ok(), "{element}");
    }
}

#[test]
fn byte_record_imported_fields_keep_complete_child_type_origin() {
    let root = "mod child; use crate::child::Inner; struct Outer { inner:Inner } fn main()->i32{return 0;}";
    for field_type in ["u8", "[u8;0]", "[u8;1]", "[u8;1024]"] {
        let text = format!("pub struct Inner {{ pub bytes:{field_type} }}");
        for project in super::super::byte_storage_tests::module_sources(&[
            ("main.ox", root),
            ("child.ox", &text),
        ]) {
            assert!(project.uses_owned_syntax());
            let resolved = resolve_sources(SourceOwner::project(&project)).unwrap();
            let field = resolved
                .records()
                .iter()
                .flat_map(|record| &record.fields)
                .find(|field| field.span.file == crate::frontend::source::SourceFileId(1))
                .unwrap();
            assert!(project
                .sources()
                .get(field.span.file)
                .text_at(field.span)
                .contains(field_type));
            assert!(super::super::typeck::check(resolved).is_ok());
        }
    }
}

#[test]
fn byte_record_field_gate_preserves_enum_rejection() {
    let mut sources = SourceMap::new();
    let file = sources.add("byte-record.ox".into(), "field".into());
    let span = sources.get(file).span(0, 5);
    for ty in [
        ValueTy::Scalar(Ty::U8),
        ValueTy::Owned(AggregateTy::FixedArray(
            FixedArrayTy::check(Ty::U8, 0).unwrap(),
        )),
    ] {
        assert_eq!(record_field_type(ty, span).unwrap(), ty);
    }
    let error = record_field_type(ValueTy::Owned(AggregateTy::Enum(EnumId(0))), span).unwrap_err();
    assert_eq!((error.code, error.stage), ("E0300", "resolve"));
    assert_eq!(error.message, "enum values cannot be record fields");
    assert_eq!(error.primary, Some(span));
}

#[test]
fn byte_storage_store_first_error_order_matches_immutable_predecessor() {
    // The contracts-v2 write-both-subtree-errors-rhs-wins,
    // write-base-kind-before-index-kind and write-index-kind-before-rhs-element
    // fixtures fix this ordering. Exact bool-array/scalar-u8 analogues were
    // separately replayed on b5455ad before this successor was executed.
    for (text, code, message, selected, binding) in [
        (
            "fn f(b:u8)->i32{let a=[b];a[false+2]=true+1;return 0;}",
            "E0300",
            "type mismatch: expected i32, found bool",
            "true",
            None,
        ),
        (
            "fn f(b:u8)->i32{let s=b;s[true]=false;return 0;}",
            "E0305",
            "array access requires an array binding",
            "s[true]",
            Some("s=b"),
        ),
        (
            "fn f(b:u8)->i32{let a=[b];a[true]=false;return 0;}",
            "E0300",
            "type mismatch: expected i32, found bool",
            "true",
            None,
        ),
    ] {
        let errors = check(text).unwrap_err();
        assert_eq!(errors.len(), 1);
        let error = &errors[0];
        assert_eq!(
            (error.code, error.stage, error.message.as_str()),
            (code, "type", message)
        );
        let start = text.find(selected).unwrap();
        assert_eq!(
            error.primary.map(|s| (s.start, s.end)),
            Some((start, start + selected.len()))
        );
        match binding {
            None => assert!(error.secondary.is_empty()),
            Some(needle) => {
                assert_eq!(error.secondary.len(), 1);
                let start = text.find(needle).unwrap();
                assert_eq!(
                    (error.secondary[0].0.start, error.secondary[0].0.end),
                    (start, start + 1)
                );
                assert_eq!(error.secondary[0].1, "binding declared here");
            }
        }
        assert!(error.notes.is_empty());
    }
}
