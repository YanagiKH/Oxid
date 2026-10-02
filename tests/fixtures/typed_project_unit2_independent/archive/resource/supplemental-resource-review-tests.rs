// Additional hand-authored long-key tests; append before the supplemental run.
#[test]
fn reviewer_long_keys_keep_actual_byte_debits_and_stability() {
    let at=Span{file:SourceFileId(0),start:0,end:0};let long="x".repeat(65536);
    for limit in [65536,65537] {let w=WorkMeter::new(limit);let mut order=[0u32,1];let mut scratch=[0u32;2];let out=merge_sort(&mut order,&mut scratch,|_,_|compare_bytes(&long,&long,&w,at),&w,at);
        if limit==65536{check_error(&out.unwrap_err(),"declaration index work limit exceeded",at);}else{out.unwrap();assert_eq!(order,[0,1]);assert_eq!(w.used(),65537);}}
    let mut values=vec!["x".repeat(65536),format!("{}y","x".repeat(65535)),"x".repeat(65535),"x".repeat(65536),"z".into(),"a".into()];values.reverse();
    let mut order:Vec<u32>=(0..values.len() as u32).collect();let mut scratch=vec![0u32;values.len()];let w=WorkMeter::default();merge_sort(&mut order,&mut scratch,|a,b|compare_bytes(&values[a as usize],&values[b as usize],&w,at),&w,at).unwrap();let mut expected:Vec<u32>=(0..values.len() as u32).collect();expected.sort_by_key(|i|&values[*i as usize]);assert_eq!(order,expected);assert!(w.used()<=3*(values.len()+values.iter().map(|v|v.len()).sum::<usize>()) as u64);
}
#[test]
fn reviewer_normalized_long_path_comparison_charges_only_inspected_segments() {
    let name="x".repeat(65536);let root=format!("mod c; use crate::c::{name} as A; use crate :: c :: {name} as B;");let child=format!("pub fn {name}()->(){{}}");let fixture=Fixture::new(&[("root.ox",&root),("c.ox",&child)]);let p=fixture.load();let w=WorkMeter::default();let mut a=Allocator::default();let facts=collect_originals(SourceOwner::project(&p),IndexLimits::default(),&w,&mut a).unwrap();let at=p.try_file_ast(SourceFileId(0)).unwrap().imports[1].alias;
    // One comparator + six segment visits +5(crate)+1(c)+65536 terminal bytes.
    for limit in [65548,65549] {let meter=WorkMeter::new(limit);let view=facts.signature_view(&meter);let out=view.tables.target_cmp(0,1,&meter,at);if limit==65548{check_error(&out.unwrap_err(),"declaration index work limit exceeded",at);}else{assert_eq!(out.unwrap(),Ordering::Equal);assert_eq!(meter.used(),65549);}}
}
