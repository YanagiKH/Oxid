//! Ordinary scalar emission observations only; private Emit remains denied.
//! Source/AST/checked owners are established before the allocation interval.
use crate::frontend::{
    lexer,
    oir::{owned, source::check_source},
    parser,
    source::SourceMap,
};

#[test]
fn checked_hir_import_inherited_emit_output_and_cleanup() {
    let rich = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import/rich-source.txt"
    ));
    for (label, text) in [
        ("rich", rich),
        ("division", "fn main()->i32{return 7/2%2;}"),
        (
            "guarded",
            "fn main()->i32{let mut n=0;while n<2{n=n+1;}return n;}",
        ),
        (
            "unused-loop",
            "fn f()->(){while true{}return;}fn main()->i32{return 1;}",
        ),
        ("merge", "fn main()->bool{return (1+2==3)&&true;}"),
        ("unit", "fn main()->(){return;}"),
    ] {
        assert!(text.is_ascii() && text.len() <= 128);
        let mut sources = SourceMap::new();
        let path = if label == "division" {
            "路徑\t\\\"".repeat(256)
        } else {
            format!("ordinary-emit-{label}.ox")
        };
        let path_bytes = path.len();
        let id = sources.add(path, text.into());
        let source = sources.get(id);
        let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
        let checked = check_source(source, &ast, &sources).unwrap();
        let mut outcome = None;
        let retained = owned::hir_import_measure_allocations(|| {
            outcome = Some(checked.native_module());
        });
        let module = outcome.unwrap().unwrap();
        assert_eq!(module.len(), module.capacity());
        assert_eq!(retained.2, isize::try_from(module.capacity()).unwrap());
        let mut same = false;
        let cleaned = owned::hir_import_measure_allocations(|| {
            let repeated = checked.native_module().unwrap();
            same = repeated == module;
            drop(repeated);
        });
        assert!(same);
        assert_eq!(cleaned.2, 0);
        assert!(owned::hir_import_allocation_observers_idle());
        println!("HIR_INHERITED_EMIT {label} path_bytes={path_bytes} text_bytes={} capacity={} retained={retained:?} cleaned={cleaned:?}", module.len(), module.capacity());
        drop(checked);
        assert!(module.contains("define i32 @main"));
        drop(module);
    }
}
