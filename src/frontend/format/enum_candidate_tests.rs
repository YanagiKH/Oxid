//! Private enum formatting; ordinary format_source remains closed.
use super::*;
use crate::frontend::source::SourceFileId;

fn source(text: &str) -> SourceMap {
    let mut sources = SourceMap::new();
    sources.add("enum-format.ox".into(), text.into());
    sources
}
fn formatted(text: &str) -> String {
    let sources = source(text);
    format_enum_candidate_observed(sources.get(SourceFileId(0)), &mut Allocator::default())
        .0
        .unwrap()
}

#[test]
fn enum_formatter_private_roundtrip_keeps_public_gate_closed() {
    let text = "enum E{V(i32),Z}fn f(e:E)->(){match e{E::V(v)=>{crate::m::f(v,&mut *r);},E::Z=>{E::Z();},}}";
    let expected = "enum E { V(i32), Z } fn f(e: E) -> () { match e { E::V(v) => { crate::m::f(v, &mut *r); }, E::Z => { E::Z(); }, } }\n";
    let output = formatted(text);
    assert_eq!(output, expected);
    assert_eq!(formatted(&output), output);
    let sources = source(text);
    assert!(format_source(sources.get(SourceFileId(0))).is_err());
    let (_, metrics) =
        format_enum_candidate_observed(sources.get(SourceFileId(0)), &mut Allocator::default());
    assert_eq!(metrics.parse_calls, 2);
    assert_eq!(metrics.output_heap, output.len());
    assert_eq!(metrics.roles_heap, text.len());
    assert!(metrics.source_owner_heap > metrics.output_heap);
    assert!(metrics.phase_peak_heap_bound >= metrics.emit_live_heap);
    assert!(metrics.phase_peak_heap_bound >= metrics.reparse_live_heap);
}
