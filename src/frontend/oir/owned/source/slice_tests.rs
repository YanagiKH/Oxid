//! Whole-array scalar slice source tests, independent of native representation.
use super::{lower, program, resolve, typeck};
use crate::frontend::{
    ast,
    diagnostic::Diagnostic,
    hir::Ty,
    lexer,
    oir::{
        owned_types::{AggregateTy, BorrowedTy, FixedArrayTy},
        Scalar,
    },
    parser,
    project::budget::Allocator,
    source::{SourceFileId, SourceMap},
};

fn parsed(text: &str) -> Result<(SourceMap, ast::Program), Vec<Diagnostic>> {
    let mut sources = SourceMap::new();
    let file = sources.add("slices.ox".into(), text.into());
    let source = sources.get(file);
    let (ast, _) = parser::parse_counted_with_arrays(
        source,
        lexer::lex(source).map_err(|error| vec![*error])?,
        parser::SourceMode::OwnedCandidate,
        parser::MAX_NODES,
        &mut Allocator::default(),
        parser::ArraySyntaxPolicy::Enabled,
    )?;
    Ok((sources, ast))
}

fn run(text: &str) -> Result<Scalar, Vec<Diagnostic>> {
    let (sources, ast) = parsed(text)?;
    let (program, entry) = program::check_source(sources.get(SourceFileId(0)), &ast, &sources)?;
    program.run(entry, &sources).map_err(|error| vec![*error])
}

const SUM: &str = "fn sum(p:&[i32])->i32{let mut i=0;let mut total=0;while i<p.len(){total=total+p[i];i=i+1;}return total;}";

#[test]
fn borrowed_slice_same_sum_helper_preserves_each_actual_length() {
    let text = format!("{SUM} fn main()->i32{{let a=[10,20];let b=[50,70,162];let c:[i32;0]=[];return sum(&a)+sum(&b)+sum(&c);}}");
    assert_eq!(run(&text).unwrap(), Scalar::I32(312));
}

#[test]
fn borrowed_slice_loan_uses_target_view_and_preserves_owner_authority() {
    let text = "fn view(p:&[i32])->i32{return p.len();} fn exact(p:&[i32;2])->i32{return view(&*p);} fn main()->i32{let a=[10,20];return exact(&a)+view(&a);}";
    let (sources, ast) = parsed(text).unwrap();
    let typed = typeck::check(
        resolve::resolve_in_map(sources.get(SourceFileId(0)), &ast, &sources).unwrap(),
    )
    .unwrap();
    let raw = lower::lower(&typed).unwrap();
    let exact = BorrowedTy::Exact(AggregateTy::FixedArray(
        FixedArrayTy::check(Ty::I32, 2).unwrap(),
    ));
    assert_eq!(
        raw.functions[0].references[0].referent(),
        BorrowedTy::ScalarSlice(Ty::I32)
    );
    assert_eq!(raw.functions[1].references[0].referent(), exact);
    assert_eq!(
        raw.functions[1].loans[0].referent(),
        BorrowedTy::ScalarSlice(Ty::I32)
    );
    assert_eq!(raw.functions[2].loans[0].referent(), exact);
    assert_eq!(
        raw.functions[2].loans[1].referent(),
        BorrowedTy::ScalarSlice(Ty::I32)
    );
    assert_eq!(run(text).unwrap(), Scalar::I32(4));
}

#[test]
fn borrowed_slice_exclusive_reborrow_restores_whole_owner_access() {
    let text = format!("{SUM} fn bump(p:&mut [i32])->(){{let mut i=0;while i<p.len(){{p[i]=p[i]+1;i=i+1;}}return;}} fn relay(p:&mut [i32])->i32{{bump(&mut *p);let n=sum(&*p);p[0]=p[0]+n;return sum(&*p);}} fn main()->i32{{let mut a=[2,3];let n=relay(&mut a);return n*10+a[0];}}");
    assert_eq!(run(&text).unwrap(), Scalar::I32(150));
}

#[test]
fn borrowed_slice_bool_unit_and_zero_length_views() {
    for (text, expected) in [
        ("fn toggle(p:&mut [bool])->(){p[0]=!p[0];return;} fn read(p:&[bool])->bool{return p[0];} fn main()->bool{let mut a=[false];toggle(&mut a);return read(&a);}", Scalar::Bool(true)),
        ("fn touch(p:&mut [()])->i32{let mut i=0;while i<p.len(){p[i]=p[i];i=i+1;}return p.len();} fn main()->i32{let mut a=[(),()];let mut z:[();0]=[];return touch(&mut a)+touch(&mut z);}", Scalar::I32(2)),
        ("fn empty(p:&[bool])->i32{return p.len();} fn main()->i32{let a:[bool;0]=[];return empty(&a);}", Scalar::I32(0)),
    ] {
        assert_eq!(run(text).unwrap(), expected, "{text}");
    }
}

#[test]
fn borrowed_slice_retains_type_and_whole_root_permission_rejections() {
    for (text, code, stage) in [
        ("fn take(p:&[i32])->(){return;} fn forward(p:&[i32])->(){take(p);return;}", "E0312", "type"),
        ("fn fixed(p:&[i32;2])->(){return;} fn erased(p:&[i32])->(){fixed(&*p);return;}", "E0300", "type"),
        ("fn take(p:&[i32])->(){return;} fn main()->(){let a=[true];take(&a);return;}", "E0300", "type"),
        ("fn take(p:&[i32])->(){return;} fn main()->(){let mut a=[1];take(&mut a);return;}", "E0300", "type"),
        ("fn take(p:&mut [i32])->(){return;} fn main()->(){let a=[1];take(&mut a);return;}", "E0304", "type"),
        ("fn pair(p:&[i32],q:&mut [i32;1])->(){return;} fn main()->(){let mut a=[1];pair(&a,&mut a);return;}", "E0311", "ownership"),
        ("fn write(p:&[i32])->(){p[0]=7;return;}", "E0313", "ownership"),
        ("fn write(p:&mut [i32])->(){return;} fn forward(p:&[i32])->(){write(&mut *p);return;}", "E0313", "ownership"),
        ("fn take(p:&mut [i32],n:i32)->(){return;} fn main()->(){let mut a=[1];take(&mut a,a.len());return;}", "E0311", "ownership"),
        ("fn take(p:&[i32])->(){return;} fn main()->(){let a=[1];let b=a;take(&a);return;}", "E0310", "ownership"),
    ] {
        let errors = run(text).unwrap_err();
        assert_eq!((errors[0].code, errors[0].stage), (code, stage), "{text}: {errors:?}");
    }
}

#[test]
fn borrowed_slice_bounds_use_actual_length_and_preserve_rhs_first_failure() {
    for (text, code) in [
        ("fn read(p:&[i32])->i32{return p[p.len()];} fn main()->i32{let a=[7,8];return read(&a);}", "E0606"),
        ("fn read(p:&[i32])->i32{return p[0];} fn main()->i32{let a:[i32;0]=[];return read(&a);}", "E0606"),
        ("fn write(p:&mut [i32])->(){p[-1]=3;return;} fn main()->(){let mut a=[1];write(&mut a);return;}", "E0606"),
        ("fn write(p:&mut [i32])->(){p[1/0]=7%0;return;} fn main()->(){let mut a=[1];write(&mut a);return;}", "E0607"),
    ] {
        let errors = run(text).unwrap_err();
        assert_eq!(errors[0].code, code, "{text}: {errors:?}");
        if code == "E0607" {
            let (sources, _) = parsed(text).unwrap();
            assert_eq!(sources.get(SourceFileId(0)).text_at(errors[0].primary.unwrap()), "%");
        }
    }
}

#[test]
fn borrowed_slice_syntax_remains_parameter_only_and_scalar_only() {
    for text in [
        "fn f(p:[i32])->(){return;}",
        "fn f()->[i32]{return [];}",
        "fn f()->&[i32]{return [];}",
        "struct R{p:&[i32]}",
        "fn f()->(){let p:&[i32]=[];return;}",
        "fn f(p:&[[i32;1]])->(){return;}",
        "struct R{} fn f(p:&[R])->(){return;}",
        "fn f(p:&[&i32])->(){return;}",
        "fn f(p:&[i32])->(){p[0..1];return;}",
    ] {
        let errors = parsed(text).unwrap_err();
        assert_eq!(errors[0].stage, "parse", "{text}: {errors:?}");
    }
}
