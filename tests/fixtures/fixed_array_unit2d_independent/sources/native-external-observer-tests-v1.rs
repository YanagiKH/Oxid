#[test]
#[ignore = "independent Unit2D physical storage and mutation ELF gate"]
fn independent_unit2d_external_storage_observer_llvm() {
    let directory = std::path::PathBuf::from(std::env::var_os("OXID_UNIT2D_EXTERNAL_OBSERVERS").expect("frozen reviewer observer directory"));
    let manifest = std::fs::read_to_string(directory.join("manifest.tsv")).unwrap();
    let mut lines = manifest.lines();
    assert_eq!(lines.next(),Some("case\tmodule\tstatus\tstdout_hex\tstderr_hex"));
    let decode = |text: &str| -> Vec<u8> {
        assert!(text.len().is_multiple_of(2));
        (0..text.len()).step_by(2).map(|i|u8::from_str_radix(&text[i..i+2],16).unwrap()).collect()
    };
    let scratch = Scratch::new();
    let mut cases = 0;
    for line in lines {
        let cells: Vec<_> = line.split('\t').collect();
        assert_eq!(cells.len(),5);
        assert!(!cells[1].contains("..") && !std::path::Path::new(cells[1]).is_absolute());
        let module = std::fs::read_to_string(directory.join(cells[1])).unwrap();
        let name = format!("physical-{}",cells[0]);
        let binary = scratch.compile(&module,&name);
        let result = independent_run(&scratch,&binary,&name,&[]);
        assert_result(result,&decode(cells[3]),&decode(cells[4]),cells[2].parse().unwrap());
        cases += 1;
    }
    assert!(cases>0);
    eprintln!("independent physical/sensitivity ELF cases={cases}; exact manifest expectation checked");
}
