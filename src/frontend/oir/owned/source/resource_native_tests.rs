//! Real LLVM controls for resource preservation before a replacement commits.
use super::super::*;
use super::{association, resolve};
use crate::frontend::{
    build_config::{NativeBuildSettings, NativeOptimization},
    declaration_index::{self, IndexLimits, SourceOwner, WorkMeter},
    project::{budget::Allocator, ProjectLimits, ProjectSources},
};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf, process::Command};

const SOURCE: &str = "use alloc::bytes::ByteBuf;use alloc::bytes::new;use alloc::bytes::len;use alloc::bytes::try_push;
struct R{bytes:ByteBuf,later:bool}
fn byte(n:i32)->u8{return n.to_u8_checked();}
fn make(n:i32)->R{let mut b=new();try_push(&mut b,byte(n));return R{bytes:b,later:true};}
fn main()->i32{let mut old=make(7);let next=make(9);old=next;return len(&old.bytes);}";

fn checked() -> (ProjectSources, verified::VerifiedOwnedProgram, hir::DefId) {
    let path =
        std::env::temp_dir().join(format!("oxid-resource-replace-{}.ox", std::process::id()));
    fs::write(&path, SOURCE).unwrap();
    let project =
        ProjectSources::load_typed(path.to_str().unwrap(), ProjectLimits::default()).unwrap();
    fs::remove_file(&path).unwrap();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let facts = declaration_index::collect_originals(
        SourceOwner::project(&project),
        IndexLimits::default(),
        &work,
        &mut allocator,
    )
    .unwrap();
    let index = facts.finish(&work, &mut allocator).unwrap();
    let typed = resolve::type_enum_source(&index, &work, &mut allocator).unwrap();
    let entry = typed.entry().unwrap();
    let witness =
        verified::verify_associated(association::lower_and_associate(&typed).unwrap()).unwrap();
    drop(typed);
    drop(index);
    (project, witness, entry)
}

#[test]
#[ignore = "requires LLVM 19.1.7; executes resource replacement faults at O0/O2"]
fn bytebuf_native_replacement_validates_before_releasing_the_previous_value() {
    let (project, witness, entry) = checked();
    let sources = project.sources();
    let function = &witness.functions()[entry.0];
    let sites: Vec<_> = function
        .blocks
        .iter()
        .enumerate()
        .flat_map(|(block, body)| {
            body.statements
                .iter()
                .enumerate()
                .filter_map(move |(index, statement)| {
                    if let OwnedInstruction::Replace {
                        source,
                        destination,
                    } = statement.kind
                    {
                        Some((block, index, statement.span, source, destination))
                    } else {
                        None
                    }
                })
        })
        .collect();
    assert_eq!(sites.len(), 1);
    let (block, index, span, source, destination) = sites[0];
    let AggregateTy::Record(record) = function.owners[source.0].aggregate() else {
        panic!("record replacement")
    };
    assert_eq!(
        witness
            .declarations()
            .fields(record)
            .unwrap()
            .iter()
            .map(|f| f.offset())
            .collect::<Vec<_>>(),
        [0, 16]
    );
    let name = format!("f{}_b{block}_i{index}", entry.0);
    let needle = format!(
        "  %{name}_incoming_validate0_ptr = getelementptr i8, ptr %o{}, i64 16\n",
        source.0
    );
    let observed = native::run_array_observed(&witness, Some(entry), sources, Default::default());
    let mut module = observed
        .result
        .unwrap_or_else(|error| panic!("{error:?}; native metrics: {:?}", observed.metrics));
    assert_eq!(module.matches(&needle).count(), 1);
    for suffix in ["validate0_error", "admit_resource0_error"] {
        let marker = format!("{name}_incoming_{suffix}:\n");
        assert_eq!(module.matches(&marker).count(), 1);
        let observed = format!("{name}_observe_{suffix}");
        let observe = format!("  %{observed}_len_ptr = getelementptr i8, ptr %o{}, i64 8\n  %{observed}_len = load i32, ptr %{observed}_len_ptr, align 4\n  %{observed}_print_len = call i32 @__oxid_print_i32(i32 %{observed}_len)\n  %{observed}_valid = call i32 @__oxid_bytebuf_valid(ptr %o{})\n  %{observed}_print_valid = call i32 @__oxid_print_i32(i32 %{observed}_valid)\n  %{observed}_id = load i64, ptr %o{}, align 8\n  %{observed}_nonempty = icmp ne i64 %{observed}_id, 0\n  %{observed}_id_flag = zext i1 %{observed}_nonempty to i32\n  %{observed}_print_id = call i32 @__oxid_print_i32(i32 %{observed}_id_flag)\n", destination.0, destination.0, destination.0);
        module = module.replacen(&marker, &format!("{marker}{observe}"), 1);
    }
    let root = std::env::temp_dir().join(format!("oxid-resource-native-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let run = root.join("run");
    fs::create_dir(&run).unwrap();
    let evidence = std::env::var_os("OXID_OWNED_NATIVE_EVIDENCE").map(PathBuf::from);
    if let Some(path) = &evidence {
        fs::create_dir_all(path).unwrap();
        fs::write(path.join("bytebuf-replacement-source.ox"), SOURCE).unwrap();
    }
    for optimization in [NativeOptimization::O0, NativeOptimization::O2] {
        for control in ["valid", "late-bool", "descriptor", "early-release-control"] {
            let mut injected = format!("  %test_bool = getelementptr i8, ptr %o{}, i64 16\n  store i8 {}, ptr %test_bool, align 1\n", source.0, if matches!(control, "late-bool" | "early-release-control") { 2 } else { 1 });
            if control == "descriptor" {
                injected.push_str(&format!("  %test_length = getelementptr i8, ptr %o{}, i64 8\n  store i32 -1, ptr %test_length, align 4\n", source.0));
            }
            if control == "early-release-control" {
                injected.push_str(&format!(
                    "  call void @__oxid_bytebuf_drop(ptr %o{})\n",
                    destination.0
                ));
            }
            let harness = module.replacen(&needle, &format!("{injected}{needle}"), 1);
            let case = format!("bytebuf-replace-{optimization:?}-{control}");
            let binary = root.join(&case);
            crate::frontend::native::compile_with_settings(
                &harness,
                binary.to_str().unwrap(),
                &NativeBuildSettings::with_optimization(false, optimization),
            )
            .unwrap();
            let output = Command::new(&binary)
                .current_dir(&run)
                .env_clear()
                .output()
                .unwrap();
            if let Some(path) = &evidence {
                fs::copy(&binary, path.join(format!("{case}.elf"))).unwrap();
                for (suffix, bytes) in [
                    ("ll", harness.as_bytes()),
                    ("stdout", output.stdout.as_slice()),
                    ("stderr", output.stderr.as_slice()),
                ] {
                    fs::write(path.join(format!("{case}.{suffix}")), bytes).unwrap();
                }
                fs::write(
                    path.join(format!("{case}.status")),
                    output
                        .status
                        .code()
                        .map_or_else(|| "signal\n".into(), |status| format!("{status}\n")),
                )
                .unwrap();
                fs::write(path.join(format!("{case}.run.txt")), format!("source-free; env_clear; argv=[]; optimization={optimization:?}\nLLVM sha256={:x}\nELF sha256={:x}\nstdout sha256={:x}\nstderr sha256={:x}\n", Sha256::digest(harness.as_bytes()), Sha256::digest(fs::read(&binary).unwrap()), Sha256::digest(&output.stdout), Sha256::digest(&output.stderr))).unwrap();
            }
            if control == "valid" {
                assert_eq!(
                    (output.status.code(), output.stdout, output.stderr),
                    (Some(0), b"1\n".to_vec(), vec![])
                );
            } else {
                assert_eq!(output.status.code(), Some(1));
                assert_eq!(
                    output.stdout,
                    if control == "early-release-control" {
                        b"0\n0\n0\n"
                    } else {
                        b"1\n0\n1\n"
                    }
                );
                let invariant = if control == "descriptor" {
                    "resource descriptor"
                } else {
                    "enum payload"
                };
                assert_eq!(
                    output.stderr,
                    execute::OwnedRunFailure::Invariant(invariant, Some(span))
                        .diagnostic(sources)
                        .render_human(sources)
                        .as_bytes()
                );
            }
            assert_eq!(fs::read_dir(&run).unwrap().count(), 0);
        }
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "requires LLVM 19.1.7; executes fixed byte ranges at O0/O2"]
fn bytebuf_native_fixed_byte_ranges_preserve_bounds_and_projected_authority() {
    let cases = [
        ("exact", "fn score(p:&[u8])->i32{let b=p[0];return b.to_i32()+p.len();}
fn exact(p:&[u8;4])->i32{return score(&*p[1..3]);}
fn main()->i32{let a=[byte(0),byte(128),byte(255),byte(7)];return exact(&a)+score(&a[2..3]);}", Some(386)),
        ("record", "struct H{prefix:i32,bytes:[u8;4]}
fn score(p:&[u8])->i32{let b=p[0];return b.to_i32()+p.len();}
fn forward(p:&[u8])->i32{return score(&*p[1..2]);}
fn member(h:&H)->i32{return forward(&*h.bytes[1..4]);}
fn main()->i32{let h=H{prefix:42,bytes:[byte(0),byte(128),byte(255),byte(7)]};return member(&h);}", Some(256)),
        ("empty", "fn size(p:&[u8])->i32{return p.len();}
fn forward(p:&[u8])->i32{return size(&*p[0..0]);}
fn main()->i32{let a=[byte(255)];let empty:[u8;0]=[];return forward(&a[1..1])+size(&empty[0..0]);}", Some(0)),
        ("exclusive", "struct H{bytes:[u8;4]}
fn edit(p:&mut [u8])->(){p[0]=byte(255);return;}
fn member(h:&mut H)->(){edit(&mut *h.bytes[1..3]);return;}
fn main()->i32{let mut h=H{bytes:[byte(0),byte(128),byte(255),byte(7)]};member(&mut h);let a=h.bytes[0];let b=h.bytes[1];let c=h.bytes[2];let d=h.bytes[3];return a.to_i32()+b.to_i32()+c.to_i32()+d.to_i32();}", Some(517)),
        ("bounds", "fn size(p:&[u8])->i32{return p.len();}
fn main()->i32{let a=[byte(255),byte(128)];return size(&a[2..1]);}", None),
    ];
    let root = std::env::temp_dir().join(format!("oxid-byte-ranges-native-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let run = root.join("run");
    fs::create_dir(&run).unwrap();
    let evidence = std::env::var_os("OXID_OWNED_NATIVE_EVIDENCE").map(PathBuf::from);
    if let Some(path) = &evidence {
        fs::create_dir_all(path).unwrap();
    }
    for (name, body, expected) in cases {
        let text = format!("fn byte(n:i32)->u8{{return n.to_u8_checked();}}\n{body}");
        let source_path = root.join(format!("{name}.ox"));
        fs::write(&source_path, &text).unwrap();
        let project =
            ProjectSources::load_typed(source_path.to_str().unwrap(), ProjectLimits::default())
                .unwrap();
        fs::remove_file(source_path).unwrap();
        let work = WorkMeter::default();
        let mut allocator = Allocator::default();
        let index = declaration_index::collect_originals(
            SourceOwner::project(&project),
            IndexLimits::default(),
            &work,
            &mut allocator,
        )
        .unwrap()
        .finish(&work, &mut allocator)
        .unwrap();
        let typed = resolve::type_enum_source(&index, &work, &mut allocator).unwrap();
        let entry = typed.entry().unwrap();
        let witness =
            verified::verify_associated(association::lower_and_associate(&typed).unwrap()).unwrap();
        let reference = execute::run(&witness, Some(entry));
        let (status, stdout, stderr) = match expected {
            Some(value) => {
                assert_eq!(reference.unwrap(), Scalar::I32(value), "{name}");
                (0, format!("{value}\n").into_bytes(), vec![])
            }
            None => {
                let error = reference.unwrap_err().diagnostic(project.sources());
                assert_eq!(error.code, "E0606");
                (
                    1,
                    vec![],
                    error.render_human(project.sources()).into_bytes(),
                )
            }
        };
        let module = native::run_array_observed(
            &witness,
            Some(entry),
            project.sources(),
            Default::default(),
        )
        .result
        .unwrap();
        if let Some(path) = &evidence {
            fs::write(path.join(format!("bytebuf-range-{name}.ox")), &text).unwrap();
        }
        for optimization in [NativeOptimization::O0, NativeOptimization::O2] {
            let case = format!("bytebuf-range-{name}-{optimization:?}");
            let binary = root.join(&case);
            crate::frontend::native::compile_with_settings(
                &module,
                binary.to_str().unwrap(),
                &NativeBuildSettings::with_optimization(false, optimization),
            )
            .unwrap();
            let output = Command::new(&binary)
                .current_dir(&run)
                .env_clear()
                .output()
                .unwrap();
            if let Some(path) = &evidence {
                fs::copy(&binary, path.join(format!("{case}.elf"))).unwrap();
                for (suffix, bytes) in [
                    ("ll", module.as_bytes()),
                    ("stdout", output.stdout.as_slice()),
                    ("stderr", output.stderr.as_slice()),
                ] {
                    fs::write(path.join(format!("{case}.{suffix}")), bytes).unwrap();
                }
                fs::write(
                    path.join(format!("{case}.status")),
                    output
                        .status
                        .code()
                        .map_or_else(|| "signal\n".into(), |value| format!("{value}\n")),
                )
                .unwrap();
                fs::write(path.join(format!("{case}.run.txt")),format!("source-free; env_clear; argv=[]; optimization={optimization:?}\nLLVM sha256={:x}\nELF sha256={:x}\nstdout sha256={:x}\nstderr sha256={:x}\n",Sha256::digest(module.as_bytes()),Sha256::digest(fs::read(&binary).unwrap()),Sha256::digest(&output.stdout),Sha256::digest(&output.stderr))).unwrap();
            }
            assert_eq!(
                (output.status.code(), output.stdout, output.stderr),
                (Some(status), stdout.clone(), stderr.clone()),
                "{case}"
            );
            assert_eq!(fs::read_dir(&run).unwrap().count(), 0);
        }
    }
    fs::remove_dir_all(root).unwrap();
}
