//! Independent source-operation schedules for explicit loop-transfer edges.
use super::*;
use crate::frontend::{lexer, parser};

fn raw(text: &str) -> (SourceMap, Program) {
    let mut sources = SourceMap::new();
    let id = sources.add("transfer-雪\n\t.ox".into(), text.into());
    let source = sources.get(id);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let typed = typeck::check(hir::resolve(source, &ast).unwrap()).unwrap();
    (sources, lower::lower(&typed).unwrap())
}

struct Witness {
    text: String,
    charges: Vec<(usize, (usize, usize))>,
    outcome: Result<Scalar, (usize, usize)>,
    slots: usize,
}
impl Witness {
    fn expected(&self, sources: &SourceMap, fuel: usize) -> Result<Scalar, RunFailure> {
        let file = sources.get(super::super::source::SourceFileId(0));
        let mut remaining = fuel;
        for &(cost, (start, end)) in &self.charges {
            if remaining < cost {
                return Err(RunFailure::Fuel(file.span(start, end)));
            }
            remaining -= cost;
        }
        self.outcome
            .map_err(|(start, end)| RunFailure::Overflow(file.span(start, end)))
    }
}
fn witness(which: usize) -> Witness {
    let main = match which {
        0 => "fn main()->i32 { while true { break /* 雪 */ ; } return 7; }",
        1 => "fn main()->i32 { let mut n=0; while n<2 { n=n+1; continue /* 雪 */ ; } return n; }",
        _ => "fn main()->i32 { while true { 2147483647+1; break; } return 0; }",
    };
    // Force guarded lowering even for an otherwise acyclic break-only main.
    let text = format!("{main}\nfn unused()->() {{ while false {{ continue; }} return; }}");
    let at = |needle: &str| {
        let start = text.find(needle).unwrap();
        (start, start + needle.len())
    };
    let while_span = (
        text.find("while").unwrap(),
        text.find("} return").unwrap() + 1,
    );
    let slots = match which {
        0 => 2,
        1 => 9,
        _ => 5,
    };
    let mut charges = vec![(1 + slots, at("main"))];
    let outcome = if which == 0 {
        charges.extend([
            (1, while_span),
            (1, at("true")),
            (1, while_span),
            (1, at("break /* 雪 */ ;")),
            (1, at("7")),
            (1, at("return 7;")),
        ]);
        Ok(Scalar::I32(7))
    } else if which == 1 {
        charges.extend([(1, at("0")), (1, at("let mut n=0;")), (1, while_span)]);
        let condition = text.find("n<2").unwrap();
        let rhs = text.find("n+1").unwrap();
        for iteration in 0..3 {
            charges.extend([
                (1, (condition, condition + 1)),
                (1, (condition + 2, condition + 3)),
                (1, (condition, condition + 3)),
                (1, while_span),
            ]);
            if iteration != 2 {
                charges.extend([
                    (1, (rhs, rhs + 1)),
                    (1, (rhs + 2, rhs + 3)),
                    (1, (rhs, rhs + 3)),
                    (1, at("n=n+1;")),
                    (1, at("continue /* 雪 */ ;")),
                ]);
            }
        }
        let ret = text.find("return n;").unwrap();
        charges.extend([(1, (ret + 7, ret + 8)), (1, at("return n;"))]);
        Ok(Scalar::I32(2))
    } else {
        let rhs = text.find("2147483647+1").unwrap();
        charges.extend([
            (1, while_span),
            (1, at("true")),
            (1, while_span),
            (1, (rhs, rhs + 10)),
            (1, (rhs + 11, rhs + 12)),
            (1, (rhs, rhs + 12)),
        ]);
        Err((rhs + 10, rhs + 11))
    };
    assert_eq!(
        charges.iter().map(|x| x.0).sum::<usize>(),
        [9, 37, 12][which]
    );
    Witness {
        text,
        charges,
        outcome,
        slots,
    }
}

#[test]
fn every_loop_transfer_fuel_cutoff_has_an_independent_full_origin() {
    for which in 0..3 {
        let w = witness(which);
        let (sources, raw) = raw(&w.text);
        assert_eq!(raw.functions[0].slot_count(), w.slots);
        let verified = verify::verify(raw, &sources).unwrap();
        for fuel in 0..=[9, 37, 12][which] {
            assert_eq!(
                execute::run_with_fuel(&verified, hir::DefId(0), fuel),
                w.expected(&sources, fuel),
                "witness {which}, fuel {fuel}"
            );
        }
    }
}

fn shapes() -> Vec<(&'static str, Scalar)> {
    vec![
        ("fn main()->i32 { while true { break; } return 7; }",Scalar::I32(7)),
        ("fn main()->i32 { let mut n=0; while n<3 { n=n+1; continue; } return n; }",Scalar::I32(3)),
        ("fn main()->i32 { let mut n=0; while n<3 { n=n+1; if n==2 { return 7; } else { continue; } } return 0; }",Scalar::I32(7)),
        ("fn main()->i32 { let mut n=0; let mut sum=0; while n<4 { n=n+1; let mut j=0; while true { j=j+1; if j==2 { break; } sum=sum+n; } if n<3 { continue; } sum=sum+10; } return sum; }",Scalar::I32(30)),
        ("fn main()->i32 { let mut n=0; while (n+1)<4 && yes(n) { n=n+1; continue; } return n; } fn yes(n:i32)->bool { return n<3; }",Scalar::I32(3)),
        ("fn main()->bool { let mut b=true; while b && (true || bad()) { b=false; continue; } return b; } fn bad()->bool { return (2147483647+1)==0; }",Scalar::Bool(false)),
        ("fn main()->i32 { let mut n=0; while n<3 { n=n+1; if n>0 { break; } 2147483647+1; } return n; }",Scalar::I32(1)),
        ("fn main()->() { while true { break; } return; } fn unused()->() { while false { continue; } return; }",Scalar::Unit),
    ]
}

#[test]
fn native_guarding_depends_on_real_backedges_not_while_spelling() {
    for (text,guarded) in [
        ("fn main()->() { while true { break; } return; }",false),
        ("fn main()->() { while true { if true { return; } else { break; } } return; }",false),
        ("fn main()->() { while false { continue; } return; }",true),
        ("fn main()->() { while false { if true { break; } } return; }",true),
        ("fn main()->() { while true { break; } return; } fn unused()->() { while false { continue; } return; }",true),
    ] {
        let (sources,p)=raw(text);
        let p=verify::verify(p,&sources).unwrap();
        let module=p.native_module(Some(hir::DefId(0)),&sources).unwrap();
        assert_eq!(module.contains("%fuel"),guarded,"{text}");
    }
}

#[test]
fn transfer_shapes_keep_results_with_nonzero_entry_block_permutations() {
    for (text, expected) in shapes() {
        let (sources, raw) = raw(text);
        for reversed in [false, true] {
            let mut p = raw.clone();
            if reversed {
                let n = p.functions[0].blocks.len();
                cfg_tests::permute(&mut p, &(0..n).rev().collect::<Vec<_>>());
                assert_ne!(p.functions[0].entry, BlockId(0));
            }
            assert_eq!(
                verify::verify(p, &sources)
                    .unwrap()
                    .run(Some(hir::DefId(0))),
                Ok(expected)
            );
        }
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7; explicitly run in the native CI job"]
fn loop_control_uses_real_llvm() {
    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = Scratch(std::env::temp_dir().join(format!(
        "oxid-transfer-native-{}-{stamp}",
        std::process::id()
    )));
    std::fs::create_dir(&root.0).unwrap();
    let mut artifacts = 0;
    let mut run = |module: &str, expected: Result<Scalar, RunFailure>, sources: &SourceMap| {
        let path = root.0.join(format!("artifact-{artifacts}"));
        crate::frontend::native::compile(module, path.to_str().unwrap()).unwrap();
        let result = std::process::Command::new(path)
            .env_clear()
            .env("PATH", root.0.join("no-tools"))
            .output()
            .unwrap();
        match expected {
            Ok(value) => {
                assert_eq!(result.status.code(), Some(0));
                assert!(result.stderr.is_empty());
                assert_eq!(result.stdout, format!("{value}\n").as_bytes());
            }
            Err(error) => {
                assert_eq!(result.status.code(), Some(1));
                assert!(result.stdout.is_empty());
                assert_eq!(
                    result.stderr,
                    error.diagnostic(sources).render_human(sources).as_bytes()
                );
            }
        }
        artifacts += 1;
    };
    for which in 0..3 {
        let w = witness(which);
        let (sources, raw) = raw(&w.text);
        let p = verify::verify(raw, &sources).unwrap();
        for fuel in 0..=[9, 37, 12][which] {
            let expected = w.expected(&sources, fuel);
            assert_eq!(execute::run_with_fuel(&p, hir::DefId(0), fuel), expected);
            let module = p
                .native_module_with_fuel(hir::DefId(0), &sources, fuel)
                .unwrap();
            run(&module, expected, &sources);
        }
    }
    for (text, expected) in shapes() {
        let (sources, raw) = raw(text);
        for reversed in [false, true] {
            let mut p = raw.clone();
            if reversed {
                let n = p.functions[0].blocks.len();
                cfg_tests::permute(&mut p, &(0..n).rev().collect::<Vec<_>>());
            }
            let p = verify::verify(p, &sources).unwrap();
            let module = p.native_module(Some(hir::DefId(0)), &sources).unwrap();
            run(&module, Ok(expected), &sources);
        }
    }
    assert_eq!(artifacts, 77); // 61 independent fuel/error cuts + 16 permuted shapes.
}
