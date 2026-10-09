//! RFC0031 declaration/type controls. No consumer result supplies an expectation.
use super::*;
use crate::frontend::oir::owned_types::DeclarationError;
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
fn byte_storage_source_unused_nested_and_empty_record_fields_stay_closed() {
    for n in [0, 1, 1024] {
        let ty = format!("[u8;{n}]");
        for declarations in [
            format!("struct Unused {{ bytes:{ty} }}"),
            format!("struct Outer {{ inner:Inner }} struct Inner {{ bytes:{ty} }}"),
            format!("struct Inner {{ bytes:{ty} }} struct Outer {{ inner:Inner }}"),
        ] {
            let text = format!("{declarations} fn main()->i32{{return 0;}}");
            let errors = check(&text).unwrap_err();
            assert_eq!(errors.len(), 1);
            let error = &errors[0];
            assert_eq!(
                (error.code, error.stage, error.message.as_str()),
                (
                    "E0202",
                    "resolve",
                    "u8 array record fields are not supported"
                )
            );
            let start = text.find(&ty).unwrap();
            assert_eq!(
                error.primary.map(|s| (s.start, s.end)),
                Some((start, start + ty.len()))
            );
            assert!(error.secondary.is_empty());
        }
    }
    let errors = check("struct Unused { byte:u8 } fn main()->i32{return 0;}").unwrap_err();
    assert_eq!(
        (errors[0].code, errors[0].stage, errors[0].message.as_str()),
        ("E0202", "resolve", "u8 record fields are not supported")
    );
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
    for element in ["bool", "i32", "()"] {
        let text = format!("struct R{{a:[{element};0]}} fn take(a:[u8;0])->[u8;0]{{return a;}} fn read(a:&[u8])->i32{{return a.len();}} fn exact(a:&[u8;0])->i32{{return a.len();}} fn main()->i32{{let a:[u8;0]=[];return read(&a)+exact(&a);}}");
        assert!(check(&text).is_ok(), "{element}");
    }
}

#[test]
fn byte_storage_imported_record_fence_uses_complete_child_type_origin() {
    use crate::frontend::project::{ProjectLimits, ProjectSources};
    use std::{
        fs,
        sync::atomic::{AtomicUsize, Ordering},
    };
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let directory = std::env::temp_dir().join(format!(
        "oxid-byte-type-project-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&directory).unwrap();
    let root = directory.join("main.ox");
    fs::write(&root, "mod child; use crate::child::Inner; struct Outer { inner:Inner } fn main()->i32{return 0;}").unwrap();
    for n in [0, 1, 1024] {
        let field_type = format!("[u8;{n}]");
        let text = format!("pub struct Inner {{ pub bytes:{field_type} }}");
        fs::write(directory.join("child.ox"), &text).unwrap();
        let project =
            ProjectSources::load_typed(root.to_str().unwrap(), ProjectLimits::default()).unwrap();
        assert!(project.uses_owned_syntax());
        let errors = resolve_sources(SourceOwner::project(&project)).unwrap_err();
        assert_eq!(errors.len(), 1);
        let error = &errors[0];
        assert_eq!(
            (error.code, error.stage, error.message.as_str()),
            (
                "E0202",
                "resolve",
                "u8 array record fields are not supported"
            )
        );
        let primary = error.primary.unwrap();
        assert_eq!(primary.file, crate::frontend::source::SourceFileId(1));
        let start = text.find(&field_type).unwrap();
        assert_eq!(
            (primary.start, primary.end),
            (start, start + field_type.len())
        );
        assert!(error.secondary.is_empty());
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn byte_storage_new_fence_format_backing_and_failure_roles_are_bounded() {
    use std::mem::{align_of, size_of};
    let mut sources = SourceMap::new();
    let file = sources.add("byte-error.ox".into(), "[u8;0]".into());
    let span = sources.get(file).span(0, 6);
    let ty = ValueTy::Owned(AggregateTy::FixedArray(
        FixedArrayTy::check(Ty::U8, 0).unwrap(),
    ));
    let literal = format_args!("u8 array record fields are not supported");
    // The new branch passes a static literal: there is no interpolated
    // argument array or source-sized name backing to add to fmt::Arguments.
    assert_eq!(
        literal.as_str(),
        Some("u8 array record fields are not supported")
    );
    let (error, metrics) =
        owned_diagnostic::measure_formatting(|| record_field_type(ty, span).unwrap_err());
    assert_eq!(metrics.allocations, 1);
    assert_eq!(
        metrics.copied_bytes,
        "u8 array record fields are not supported".len()
    );
    assert_eq!(metrics.displayed_names, 0);
    assert_eq!(
        error.message.capacity(),
        owned_diagnostic::MAX_MESSAGE_BYTES
    );
    assert_eq!(error.secondary.capacity(), 0);
    assert_eq!(error.notes.capacity(), 0);
    println!("BYTE-STORAGE-LANE-A record-fence-error message_len={} message_capacity={} label_capacity={} note_capacity={} bounded_reservations={} static_format_argument_backing=0", error.message.len(), error.message.capacity(), error.secondary.capacity(), error.notes.capacity(), metrics.allocations);
    for fail_after in [0, 1] {
        let (error, metrics) = owned_diagnostic::measure_formatting(|| {
            owned_diagnostic::fail_allocation_after(fail_after, || {
                record_field_type(ty, span).unwrap_err()
            })
        });
        assert_eq!(metrics.allocations, 1);
        assert_eq!(error.primary, Some(span));
        assert_eq!(error.stage, "resolve");
        assert_eq!(error.code, if fail_after == 0 { "E0400" } else { "E0202" });
    }
    macro_rules! role {
        ($label:literal,$ty:ty) => {
            println!(
                "BYTE-STORAGE-LANE-A role {} size={} align={}",
                $label,
                size_of::<$ty>(),
                align_of::<$ty>()
            );
        };
    }
    // These are conservative simultaneous repository-owned role inventories,
    // not a compiler-generated stack or process-RSS measurement. Arguments,
    // returned Result and caller-selected span are accounted separately.
    role!(
        "record field caller type/type-span/field-span/selected-span/result",
        (ValueTy, Span, Span, Span, Result<ValueTy, Box<Diagnostic>>)
    );
    role!(
        "record fence type/span/match-array/format/result",
        (
            ValueTy,
            Span,
            FixedArrayTy,
            std::fmt::Arguments<'static>,
            Result<ValueTy, Box<Diagnostic>>
        )
    );
    role!(
        "source error code/format/span/boxed-return",
        (
            &'static str,
            std::fmt::Arguments<'static>,
            Span,
            Box<Diagnostic>
        )
    );
    role!(
        "array query arguments/matched-element/checked-return/caller-return",
        (
            ast::FixedArraySyntax,
            Span,
            Ty,
            Result<FixedArrayTy, DeclarationError>,
            Result<FixedArrayTy, Box<Diagnostic>>
        )
    );
    role!(
        "literal inherited scalar-inference/current/expected/array/checked-return",
        (
            Option<Ty>,
            ValueTy,
            Ty,
            FixedArrayTy,
            Result<FixedArrayTy, DeclarationError>
        )
    );
    for (name, size, align) in owned_diagnostic::u8_error_phase_layouts() {
        println!("BYTE-STORAGE-LANE-A diagnostic-role {name} size={size} align={align}");
    }
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
