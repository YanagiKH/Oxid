//! Inject under frontend::parser in the copied frozen tree.
use super::*;
use crate::frontend::source::SourceMap;
#[test]
fn reviewer_q_node_and_reserve_precedence() {
    for segments in [34usize,35] {
        let path=format!("crate::{}",vec!["a";segments-1].join("::"));
        let text=format!("fn main()->(){{return {path}();}}");
        for limit in if segments==34 { vec![37usize,36] } else { vec![37usize] } {
            let mut map=SourceMap::new();let id=map.add("q.ox".into(),text.clone());let file=map.get(id);let tokens=crate::frontend::lexer::lex(file).unwrap();let mut a=Allocator{fail_at:if segments==35 {Some(35)}else if limit==36 {Some(34)}else {None},..Allocator::default()};
            let out=parse_counted(file,tokens,SourceMode::ProjectCandidate,limit,&mut a);
            if segments==34 && limit==37 {assert_eq!(out.unwrap().1,37);assert_eq!(a.attempts,35);} else {let errors=out.unwrap_err();let e=&errors[0]; assert_eq!((e.code,e.stage),("E0400","parse"));assert_eq!(e.message,if segments==35 {"qualified path segment limit exceeded"}else{"syntax node limit exceeded"});assert_eq!(a.attempts,if segments==35 {34}else{33});}
        }
    }
}
#[test]
fn reviewer_checked_segment_overflow_has_zero_reserves() {
    let mut map=SourceMap::new();let id=map.add("overflow.ox".into(),"crate".into());let file=map.get(id);let tokens=crate::frontend::lexer::lex(file).unwrap();let mut a=Allocator::default();
    let mut p=Parser{source:file,allocator:&mut a,mode:SourceMode::ProjectCandidate,tokens,cursor:0,expressions:Vec::new(),paths:Vec::new(),path_segments:Vec::new(),heights:Vec::new(),nodes:0,node_limit:0};
    let mut count=usize::MAX;
    let error=p.path_segment(&mut count,file.span(0,5)).unwrap_err();
    assert_eq!((error.code,error.stage,error.message.as_str(),error.primary),("E0400","parse","project syntax count overflow",Some(file.span(0,5))));
    assert_eq!(count,usize::MAX);assert_eq!(a.attempts,0);
}
