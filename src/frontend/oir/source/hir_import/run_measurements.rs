//! Inherited reference-engine observations through the ordinary source facade.
//! The private importer Run gate remains denied; these are not imported runs.
use crate::frontend::{
    lexer,
    oir::{execute, source::check_source, Scalar},
    parser,
    source::SourceMap,
};

#[test]
fn checked_hir_import_inherited_run_layouts_and_capacities() {
    println!("HIR_INHERITED_RUN_LAYOUT {:?}", execute::measurement::layout());
    let rich = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import/rich-source.txt"
    ));
    for (label, text, expected) in [
        ("rich-call", rich, Ok(Scalar::I32(1))),
        ("two-arguments", "fn f(a:i32,b:i32)->i32{return a+b;}fn main()->i32{return f(3,4);}", Ok(Scalar::I32(7))),
        ("bool", "fn main()->bool{return true;}", Ok(Scalar::Bool(true))),
        ("unit", "fn main()->(){return;}", Ok(Scalar::Unit)),
        ("overflow", "fn main()->i32{return 2147483647+1;}", Err("E0604")),
        ("division", "fn main()->i32{return 1/0;}", Err("E0607")),
        ("fuel", "fn main()->(){while true{}return;}", Err("E0601")),
        ("frames", "fn f()->i32{return f();}fn main()->i32{return f();}", Err("E0602")),
        ("library", "fn f()->i32{return 1;}", Err("E0600")),
        ("arity", "fn main(x:i32)->i32{return x;}", Err("E0600")),
    ] {
        assert!(text.is_ascii() && text.len() <= 128);
        let mut sources = SourceMap::new();
        let id = sources.add(format!("ordinary-run-{label}.ox"), text.into());
        let source = sources.get(id);
        let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
        let checked = check_source(source, &ast, &sources).unwrap();
        let guard = execute::measurement::begin();
        let result = checked.run();
        let observed = guard.finish();
        assert!(!observed.observation_overflowed);
        assert_eq!(result.as_ref().map(|value| *value).map_err(|error| error.code), expected, "{label}");
        println!("HIR_INHERITED_RUN {label} {observed:?}");
        drop(result);
        drop(checked);
    }
}
