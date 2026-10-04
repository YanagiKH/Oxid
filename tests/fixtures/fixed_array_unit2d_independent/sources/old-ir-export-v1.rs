// Independent old-behavior exporter. Uses the pre-existing raw fixtures and real verifier/consumer.
// This artifact is unchanged when appended to baseline and candidate native_tests.rs.
#[test]
fn independent_unit2d_export_old_ir() {
    fn empty() -> (SourceMap, RawOwnedProgram) { let (s,r,_) = fixtures::empty_record(); (s,r) }
    fn relay() -> (SourceMap, RawOwnedProgram) { let (s,r,_) = fixtures::owned_relay(); (s,r) }
    fn shared() -> (SourceMap, RawOwnedProgram) { let (s,r,_) = fixtures::shared_read(); (s,r) }
    fn empty_call() -> (SourceMap, RawOwnedProgram) { let (s,r,_) = empty_relay(); (s,r) }
    fn adjacent() -> (SourceMap, RawOwnedProgram) { let (s,r,_) = empty_adjacent_sentinels(); (s,r) }
    fn overflowing() -> (SourceMap, RawOwnedProgram) { overflow_before_field(true) }
    fn no_overflow() -> (SourceMap, RawOwnedProgram) { overflow_before_field(false) }
    fn merge_forward() -> (SourceMap, RawOwnedProgram) { looping_merge(false) }
    fn merge_reverse() -> (SourceMap, RawOwnedProgram) { looping_merge(true) }
    type Builder = fn() -> (SourceMap, RawOwnedProgram);
    let destination = std::path::PathBuf::from(std::env::var_os("OXID_UNIT2D_OLD_IR_OUTPUT").expect("explicit reviewer output directory"));
    std::fs::create_dir_all(&destination).unwrap();
    let mut rows = String::from("fixture\tguarded_added\trender_map\tllvm_bytes\tplan_bytes\troot_cells\troot_native_bytes\n");
    let builders: [(&str,Builder);9] = [("empty",empty),("relay",relay),("shared",shared),("empty-call",empty_call),("adjacent",adjacent),("overflow",overflowing),("no-overflow",no_overflow),("merge-forward",merge_forward),("merge-reverse",merge_reverse)];
    for (name,build) in builders {
        for add_guard in [false,true] {
            let (sources,mut raw) = build();
            if add_guard { append_cycle(&mut raw); }
            let witness = verified::verify_owned(raw,&sources).unwrap();
            let plan = ExecutionPlan::build(&witness).unwrap();
            let usage=plan.function(hir::DefId(0)).usage();
            for map in 0..5 {
                let mut supplied = SourceMap::new();
                match map {
                    0 => {},
                    1 => {},
                    2 => { supplied.add("short\n\t\0.ox".into(),"x".into()); },
                    3 => { supplied.add("unicode-invalid-boundary.ox".into(),"é".repeat(4096)); },
                    4 => { supplied.add("different\t\n\r\u{1b}\\\".ox".into(),"q\r\n\txe\u{301}界🦀\n".repeat(4096)); },
                    _ => unreachable!(),
                }
                let rendered = if map==0 { &sources } else { &supplied };
                let module=native_module_with_fuel(&witness,hir::DefId(0),rendered,73).unwrap();
                let stem=format!("{name}-g{}-m{map}",usize::from(add_guard));
                std::fs::write(destination.join(format!("{stem}.ll")),&module).unwrap();
                use std::fmt::Write;
                writeln!(rows,"{name}\t{add_guard}\t{map}\t{}\t{}\t{}\t{}",module.len(),plan.metadata_bytes(),usage.expanded_cells,usage.native_bytes).unwrap();
            }
        }
    }
    std::fs::write(destination.join("inventory.tsv"),rows).unwrap();
    eprintln!("independent Unit2D old IR: 9 inherited fixtures x 2 guard choices x 5 rendering maps = 90 exact modules");
}
