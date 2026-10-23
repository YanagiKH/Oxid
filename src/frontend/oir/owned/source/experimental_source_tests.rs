//! Ordinary source acceptance for the experimental resource, through the same
//! checked source owner used by the public compiler and both consumers.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use super::super::super::binary_io;
use super::super::super::byte_buffer;
use super::*;
use crate::frontend::project::{ProjectLimits, ProjectSources};
use std::sync::atomic::{AtomicUsize, Ordering};
#[path = "io_delivery_source_tests.rs"]
mod io_delivery;
#[path = "process_arguments_tests.rs"]
mod process_arguments;
static NEXT: AtomicUsize = AtomicUsize::new(0);

fn project(text: &str) -> ProjectSources {
    let path = std::env::temp_dir().join(format!(
        "oxid-bytebuf-source-{}-{}.ox",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, text).unwrap();
    let result = ProjectSources::load_typed(path.to_str().unwrap(), ProjectLimits::default());
    std::fs::remove_file(path).unwrap();
    result.unwrap()
}

const IMPORTS: &str = "use alloc::bytes::ByteBuf; use alloc::bytes::GrowStatus;
use alloc::bytes::new; use alloc::bytes::len; use alloc::bytes::capacity;
use alloc::bytes::try_reserve; use alloc::bytes::try_push; use alloc::bytes::try_extend;
use alloc::bytes::try_resize; use alloc::bytes::truncate;
fn byte(n:i32)->u8{return n.to_u8_checked();}";

#[test]
fn bytebuf_source_minimal_no_status_imports() {
    let project = project(
        "use alloc::bytes::new; use alloc::bytes::len; fn main()->i32{let b=new();return len(&b);}",
    );
    let (program, entry) = check_enum_source(SourceOwner::project(&project)).unwrap();
    assert_eq!(
        program.run(entry, project.sources()).unwrap(),
        Scalar::I32(0)
    );
    program.native_module(entry, project.sources()).unwrap();
}

#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn bytebuf_source_empty_binary_array_views_have_no_transport_effect() {
    let project = project("use std::io::read_some;use std::io::write_some;
use std::io::ReadSomeStatus;use std::io::WriteSomeStatus;
fn main()->i32{let mut a:[u8;0]=[];let r=read_some(&mut a);match r{
ReadSomeStatus::Data(n)=>{if n!=0{return 1;}},ReadSomeStatus::Eof=>{return 2;},ReadSomeStatus::Error(e)=>{return 3;},}
let w=write_some(&a[0..0]);match w{WriteSomeStatus::Written(n)=>{return n;},WriteSomeStatus::Error(e)=>{return 4;},}}");
    let (program, entry) = check_enum_source(SourceOwner::project(&project)).unwrap();
    let (result, script) = binary_io::testing::capture(Default::default(), || {
        program.run_process(entry.unwrap(), project.sources())
    });
    assert_eq!(result.unwrap(), Scalar::I32(0));
    assert_eq!((script.read_calls, script.write_calls), (0, 0));
    program
        .native_process_module(entry.unwrap(), project.sources())
        .unwrap();
    assert_eq!(
        program.run(entry, project.sources()).unwrap_err().code,
        "E0609"
    );
}

fn accepted(body: &str, expected: i32) {
    accepted_with_imports(IMPORTS, body, expected);
}

fn accepted_with_imports(imports: &str, body: &str, expected: i32) {
    let project = project(&format!("{imports}\n{body}"));
    let (program, entry) = check_enum_source(SourceOwner::project(&project)).unwrap();
    let sources = project.sources();
    for _ in 0..10 {
        let (value, observed) =
            byte_buffer::testing::capture(Default::default(), || program.run(entry, sources));
        assert_eq!(value.unwrap(), Scalar::I32(expected));
        assert_eq!(observed.before_cleanup.live_allocations, 0);
        assert_eq!(observed.after_cleanup.live_bytes, 0);
        assert_eq!(
            observed.after_cleanup.allocations,
            observed.after_cleanup.releases
        );
    }
    let module = program.native_module(entry, sources).unwrap();
    assert!(module.contains("@__oxid_bytebuf_operation"));
    assert!(module.contains("@__oxid_bytebuf_cleanup"));
    assert!(module.contains("@__oxid_bytebuf_finish"));
}

#[test]
fn bytebuf_source_growth_failure_preserves_old_value_and_reclaims_staging() {
    let body = "fn first(p:&[u8])->i32{let value=p[0];return value.to_i32();}
fn main()->i32{let mut b=new();try_push(&mut b,byte(255));let old=capacity(&b);
let result=try_reserve(&mut b,8);match result{GrowStatus::AllocationFailed=>{
return first(&b)+len(&b)+capacity(&b)-old;},GrowStatus::Ok=>{return 0;},
GrowStatus::CapacityOverflow=>{return 1;},GrowStatus::MemoryLimit=>{return 2;},}}";
    let project = project(&format!("{IMPORTS}{body}"));
    let (program, entry) = check_enum_source(SourceOwner::project(&project)).unwrap();
    for boundary in [
        byte_buffer::Boundary::Allocate,
        byte_buffer::Boundary::Initialize,
        byte_buffer::Boundary::Commit,
    ] {
        let control = byte_buffer::testing::Control {
            fault: Some((boundary, 1)),
            ..Default::default()
        };
        let (result, observed) =
            byte_buffer::testing::capture(control, || program.run(entry, project.sources()));
        assert_eq!(result.unwrap(), Scalar::I32(256));
        assert!(observed.fault_applied);
        assert_eq!(observed.before_cleanup.live_bytes, 0);
        assert_eq!(
            observed.after_cleanup.allocations,
            observed.after_cleanup.releases
        );
    }
    for body in [
        "struct Staged{bytes:ByteBuf,n:i32} fn main()->i32{let s=Staged{bytes:make(),n:fail()};return 0;}",
        "fn consume(b:ByteBuf,n:i32)->i32{return n;} fn main()->i32{return consume(make(),fail());}",
        "fn main()->i32{let mut b=make();let zero=0;try_resize(&mut b,-1,zero.to_u8_checked());return 0;}",
    ] {
        let project = self::project(&format!("{IMPORTS}fn make()->ByteBuf{{let mut b=new();try_push(&mut b,byte(7));return b;}} fn fail()->i32{{let z=0;return 1/z;}}{body}"));
        let (program, entry) = check_enum_source(SourceOwner::project(&project)).unwrap();
        let (result, observed) = byte_buffer::testing::capture(Default::default(), || program.run(entry, project.sources()));
        assert!(result.is_err());
        assert_eq!(observed.before_cleanup.live_allocations, 1);
        assert_eq!(observed.after_cleanup.live_allocations, 0);
        assert_eq!(observed.after_cleanup.allocations, observed.after_cleanup.releases);
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn application() -> Vec<crate::frontend::project::packages::PackageSpec> {
    use crate::frontend::project::packages::{PackageDependency, PackageSpec};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("apps/byte-reverse");
    ["app", "binary", "reverse"]
        .into_iter()
        .enumerate()
        .map(|(index, name)| {
            let root = root.join(name).canonicalize().unwrap();
            PackageSpec {
                entry: root.join(if index == 0 {
                    "src/main.ox"
                } else {
                    "src/lib.ox"
                }),
                root,
                dependencies: if index == 0 {
                    vec![
                        PackageDependency {
                            alias: "binary".into(),
                            package: 1,
                        },
                        PackageDependency {
                            alias: "reversal".into(),
                            package: 2,
                        },
                    ]
                } else {
                    vec![]
                },
            }
        })
        .collect()
}

#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn bytebuf_source_binary_loops_short_interrupt_eof_error_zero_and_growth() {
    let specs = application();
    let project = crate::frontend::project::packages::PackageSources::load(
        &specs,
        None,
        ProjectLimits::default(),
    )
    .unwrap();
    let limits = IndexLimits::default();
    let program = crate::frontend::oir::project::check_package_executable(
        &project,
        limits,
        &WorkMeter::new(limits.work),
        &mut Allocator::default(),
    )
    .unwrap();
    let module = program.native_process_module().unwrap();
    assert!(module.contains("@__oxid_bytebuf_view"));
    assert!(!module.contains("call i32 @__oxid_print_i32"));
    if let Some(directory) = std::env::var_os("OXID_BYTEBUF_IR_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("oxrev.ll"), &module).unwrap();
    }
    let data: Vec<_> = (0..8199).map(|n| ((n * 73 + 19) % 256) as u8).collect();
    for run in 0..10 {
        let mut script = binary_io::testing::Script::default();
        script.reads.push_back(Err(4));
        script
            .reads
            .extend(data.chunks(1024).map(|chunk| Ok(chunk.to_vec())));
        script.reads.push_back(Ok(vec![]));
        script.writes = [Ok(1), Err(4), Ok(2), Ok(7)].into();
        let ((result, io), ledger) = byte_buffer::testing::capture(Default::default(), || {
            binary_io::testing::capture(script, || program.run_process())
        });
        assert_eq!(result.unwrap(), 0);
        assert_eq!(io.output, data.iter().rev().copied().collect::<Vec<_>>());
        assert!(io.reads.is_empty() && io.writes.is_empty());
        assert_eq!(ledger.before_cleanup.live_allocations, 0);
        assert_eq!(ledger.after_cleanup.allocations, 6); // reusable chunk + five destination growths
        assert_eq!(ledger.after_cleanup.releases, 6);
        assert_eq!(ledger.after_cleanup.peak_bytes, 25_696);
        if run == 0 {
            println!(
                "oxrev8199 ledger={ledger:?} reads={} writes={}",
                io.read_calls, io.write_calls
            );
        }
    }
    for (reads, writes, expected) in [
        (vec![Err(9)], vec![], 74),
        (vec![Ok(vec![0, 128, 255]), Ok(vec![])], vec![Err(32)], 74),
        (vec![Ok(vec![0, 128, 255]), Ok(vec![])], vec![Ok(0)], 74),
        (vec![Ok(vec![])], vec![], 0),
    ] {
        let script = binary_io::testing::Script {
            reads: reads.into(),
            writes: writes.into(),
            ..Default::default()
        };
        let ((result, io), ledger) = byte_buffer::testing::capture(Default::default(), || {
            binary_io::testing::capture(script, || program.run_process())
        });
        assert_eq!(result.unwrap(), expected);
        assert!(io.output.is_empty());
        assert_eq!(ledger.before_cleanup.live_bytes, 0);
    }
    for (boundary, attempts) in [
        (byte_buffer::Boundary::Allocate, 6),
        (byte_buffer::Boundary::Initialize, 6),
        (byte_buffer::Boundary::Commit, 10),
    ] {
        for skip in 0..attempts {
            let mut script = binary_io::testing::Script::default();
            script
                .reads
                .extend(data.chunks(1024).map(|chunk| Ok(chunk.to_vec())));
            script.reads.push_back(Ok(vec![]));
            let control = byte_buffer::testing::Control {
                fault: Some((boundary, skip)),
                ..Default::default()
            };
            let ((result, io), ledger) = byte_buffer::testing::capture(control, || {
                binary_io::testing::capture(script, || program.run_process())
            });
            assert_eq!(result.unwrap(), 71, "{boundary:?}/{skip}");
            assert!(ledger.fault_applied);
            assert!(io.output.is_empty());
            assert_eq!(ledger.before_cleanup.live_allocations, 0);
            assert_eq!(
                ledger.after_cleanup.allocations,
                ledger.after_cleanup.releases
            );
        }
    }
}

#[test]
fn bytebuf_source_bounds_noops_endpoint_order_and_peak_admission() {
    accepted(
        "fn point(order:&mut [i32;1],n:i32)->i32{order[0]=order[0]*10+n;return n;}
fn size(p:&[u8])->i32{return p.len();}
fn main()->i32{let mut b=new();try_resize(&mut b,4,byte(7));let mut order=[0];
let n=size(&b[point(&mut order,1)..point(&mut order,3)]);return order[0]+n;}",
        15,
    );
    for expression in [
        "size(&b[-1..0])",
        "size(&b[2..1])",
        "size(&b[0..5])",
        "size(&b[2147483647..2147483647])",
    ] {
        let project = project(&format!("{IMPORTS}fn size(p:&[u8])->i32{{return p.len();}}fn main()->i32{{let mut b=new();try_resize(&mut b,4,byte(7));return {expression};}}"));
        let (program, entry) = check_enum_source(SourceOwner::project(&project)).unwrap();
        let (result, ledger) = byte_buffer::testing::capture(Default::default(), || {
            program.run(entry, project.sources())
        });
        assert_eq!(result.unwrap_err().code, "E0606");
        assert_eq!(ledger.after_cleanup.live_bytes, 0);
        assert_eq!(
            ledger.after_cleanup.allocations,
            ledger.after_cleanup.releases
        );
        program.native_module(entry, project.sources()).unwrap();
    }
    let project = project(&format!("{IMPORTS}fn main()->i32{{let mut b=new();try_push(&mut b,byte(7));let c=capacity(&b);let status=try_reserve(&mut b,8);match status{{GrowStatus::Ok=>{{return 0;}},GrowStatus::MemoryLimit=>{{return len(&b)+capacity(&b)-c;}},GrowStatus::AllocationFailed=>{{return 2;}},GrowStatus::CapacityOverflow=>{{return 3;}},}}}}"));
    let (program, entry) = check_enum_source(SourceOwner::project(&project)).unwrap();
    for (limit, expected) in [(87, 1), (88, 0)] {
        let control = byte_buffer::testing::Control {
            limit: Some(limit),
            ..Default::default()
        };
        let (result, ledger) =
            byte_buffer::testing::capture(control, || program.run(entry, project.sources()));
        assert_eq!(result.unwrap(), Scalar::I32(expected));
        assert_eq!(ledger.before_cleanup.live_bytes, 0);
        assert!(ledger.after_cleanup.peak_bytes <= limit);
    }
    let project = self::project(&format!("{IMPORTS}fn main()->i32{{let mut b=new();let empty=new();try_reserve(&mut b,0);try_extend(&mut b,&empty);try_resize(&mut b,0,byte(7));truncate(&mut b,99);return len(&b)+capacity(&b);}}"));
    let (program, entry) = check_enum_source(SourceOwner::project(&project)).unwrap();
    let (result, ledger) = byte_buffer::testing::capture(
        byte_buffer::testing::Control {
            limit: Some(0),
            ..Default::default()
        },
        || program.run(entry, project.sources()),
    );
    assert_eq!(result.unwrap(), Scalar::I32(0));
    assert_eq!(ledger.after_cleanup.allocations, 0);
}

#[test]
fn bytebuf_source_rejects_conflicting_staged_views_moves_and_wrong_signatures() {
    for body in [
        "fn main()->i32{let b=new();let c=b;return len(&b);}",
        "fn main()->i32{let mut b=new();try_reserve(&mut b,32);try_extend(&mut b,&b);return 0;}",
        "fn main()->i32{let mut b=new();try_extend(&mut b,&b[0..0]);return 0;}",
        "fn hold(p:&[u8],b:ByteBuf)->i32{return p.len();} fn main()->i32{let b=new();return hold(&b,b);}",
        "fn hold(p:&[u8],s:GrowStatus)->i32{return p.len();} fn main()->i32{let mut b=new();return hold(&b,try_reserve(&mut b,8));}",
        "fn main()->i32{let b=new();try_push(&b,byte(1));return 0;}",
        "fn main()->i32{let mut b=new();try_resize(&mut b,true,byte(1));return 0;}",
        "struct ByteBuf{x:i32} fn main()->i32{let b=ByteBuf{x:1};return len(&b);}",
    ] {
        let project = project(&format!("{IMPORTS}{body}"));
        let errors = check_enum_source(SourceOwner::project(&project)).unwrap_err();
        assert!(!errors.is_empty());
        assert!(errors.iter().all(|error| error.code != "E0500"), "{body}: {errors:?}");
    }
    let project = project(&format!("{IMPORTS}fn descend(b:ByteBuf,n:i32)->i32{{if n==0{{return 0;}}return descend(b,n-1);}}fn main()->i32{{let mut b=new();try_push(&mut b,byte(7));return descend(b,5);}}"));
    let (program, entry) = check_enum_source(SourceOwner::project(&project)).unwrap();
    let (result, ledger) = byte_buffer::testing::capture(Default::default(), || {
        execute::run_limits(
            &program.witness,
            entry,
            execute::Limits {
                frames: 3,
                ..Default::default()
            },
        )
    });
    assert!(
        matches!(
            result,
            Err(execute::OwnedRunFailure::Scalar(
                super::super::super::super::RunFailure::Frames(_)
            ))
        ),
        "{result:?}"
    );
    assert_eq!(ledger.before_cleanup.live_allocations, 1);
    assert_eq!(ledger.after_cleanup.live_allocations, 0);
}

#[test]
fn bytebuf_source_rejects_stored_escaping_views_and_user_resource_spelling() {
    for text in [
        "use alloc::bytes::new; fn main()->i32{let b=new();let view=&b;return 0;}",
        "use alloc::bytes::ByteBuf; fn escape(b:&ByteBuf)->&[u8]{return b;}",
        "use alloc::bytes::ByteBuf; struct Stored{view:&ByteBuf} fn main()->i32{return 0;}",
    ] {
        let path = std::env::temp_dir().join(format!(
            "oxid-bytebuf-reject-{}-{}.ox",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, text).unwrap();
        let loaded = ProjectSources::load_typed(path.to_str().unwrap(), ProjectLimits::default());
        std::fs::remove_file(path).unwrap();
        let errors = match loaded {
            Err(failure) => failure.diagnostics,
            Ok(project) => check_enum_source(SourceOwner::project(&project)).unwrap_err(),
        };
        assert!(
            errors.iter().all(|error| error.code != "E0500"),
            "{text}: {errors:?}"
        );
    }
    let project = project("use alloc::bytes::len; struct ByteBuf{x:i32} fn main()->i32{let b=ByteBuf{x:7};return len(&b);}");
    let errors = check_enum_source(SourceOwner::project(&project)).unwrap_err();
    assert!(
        errors.iter().any(|error| error.code == "E0300"),
        "{errors:?}"
    );
}

#[test]
fn bytebuf_source_operations_views_and_relay_both_consumers() {
    accepted(
        "fn relay(b:ByteBuf)->ByteBuf{return b;}
fn score(p:&[u8])->i32{let a=p[0];let b=p[1];return a.to_i32()+b.to_i32()+p.len();}
fn fill(p:&mut [u8])->(){p[0]=byte(7);return;}
fn main()->i32{let mut b=new();try_push(&mut b,byte(128));try_push(&mut b,byte(255));
try_resize(&mut b,4,byte(9));fill(&mut b[2..3]);let a=[byte(0),byte(1)];
try_extend(&mut b,&a[0..2]);b=relay(b);truncate(&mut b,4);
return score(&b[0..2])+len(&b);}",
        389,
    );
}

#[test]
fn bytebuf_source_active_nested_resources_replacement_discard_and_exits() {
    accepted("struct Pair{a:ByteBuf,b:ByteBuf} struct Boxed{pair:Pair}
enum Inner{Bytes(ByteBuf),Record(Boxed),Empty} enum Outer{Nested(Inner),Number(i32)}
fn make(n:i32)->ByteBuf{let mut b=new();try_resize(&mut b,n,byte(n));return b;}
fn relay(b:ByteBuf)->ByteBuf{return b;}
fn take(o:Outer)->i32{match o{Outer::Nested(i)=>{match i{
Inner::Bytes(b)=>{let mut m=b;m=relay(m);return len(&m);},
Inner::Record(r)=>{let mut m=r;try_push(&mut m.pair.a,byte(9));return len(&m.pair.a)+len(&m.pair.b);},
Inner::Empty=>{return 0;},}},Outer::Number(n)=>{return n;},}}
fn main()->i32{make(3);let mut b=make(4);b=make(5);b=relay(b);
let mut n=0;while n<3{let local=make(7);n=n+1;if n==1{continue;}break;}
let r=Boxed{pair:Pair{a:make(2),b:make(3)}};
return take(Outer::Nested(Inner::Record(r)))+take(Outer::Nested(Inner::Bytes(b)))+take(Outer::Nested(Inner::Empty))+take(Outer::Number(1));}",12);
}

#[test]
fn bytebuf_source_forwarded_record_views_shrink_regrow_and_zero_range() {
    accepted("struct Holder{bytes:ByteBuf}
fn first(p:&[u8])->i32{if p.len()==0{return 0;}let b=p[0];return b.to_i32();}
fn forward(p:&[u8])->i32{return first(&*p[1..2]);}
fn member(h:&Holder)->i32{return forward(&*h.bytes[1..4]);}
fn main()->i32{let mut h=Holder{bytes:new()};try_resize(&mut h.bytes,4,byte(7));
truncate(&mut h.bytes,1);try_resize(&mut h.bytes,4,byte(9));
try_reserve(&mut h.bytes,0);let empty=new();try_extend(&mut h.bytes,&empty);return member(&h)+first(&h.bytes[4..4]);}",9);
}

#[test]
fn bytebuf_source_reference_parameters_preserve_identity_loans_and_views() {
    accepted_with_imports(
        &IMPORTS.replace(
            "use alloc::bytes::ByteBuf;",
            "use alloc::bytes::ByteBuf as Buffer;",
        ),
        "struct Holder{prefix:i32,bytes:Buffer}
fn clear(p:&mut Buffer)->(){truncate(&mut *p,0);return;}
fn count(p:&Buffer)->i32{return len(&*p);}
fn push(p:&mut Buffer)->(){try_push(&mut *p,byte(9));return;}
fn view(p:&mut [u8])->i32{return p.len();}
fn end(p:&mut Buffer)->i32{return view(&mut *p[count(&*p)..count(&*p)]);}
fn extend(p:&mut Buffer,s:&[u8])->GrowStatus{return try_extend(&mut *p,&*s);}
fn main()->i32{let mut h=Holder{prefix:42,bytes:new()};try_resize(&mut h.bytes,3,byte(7));
let empty=end(&mut h.bytes);clear(&mut h.bytes);push(&mut h.bytes);
let a=[byte(1),byte(2)];extend(&mut h.bytes,&a);
return empty+count(&h.bytes)+end(&mut h.bytes);}",
        3,
    );
}

#[test]
fn bytebuf_source_reference_parameters_keep_authority_and_borrow_rejections() {
    for text in [
        "use alloc::bytes::len;struct ByteBuf{x:i32}fn fake(p:&ByteBuf)->i32{return len(&*p);}fn main()->i32{return 0;}",
        "use alloc::bytes::ByteBuf;use alloc::bytes::truncate;fn clear(p:&ByteBuf)->(){truncate(&mut *p,0);return;}fn main()->i32{return 0;}",
        "use alloc::bytes::GrowStatus;fn invalid(p:&GrowStatus)->i32{return 0;}fn main()->i32{return 0;}",
        "use alloc::bytes::ByteBuf as Buffer;fn invalid(p:&crate::Buffer)->i32{return 0;}fn main()->i32{return 0;}",
        "use alloc::bytes::ByteBuf;use alloc::bytes::GrowStatus;use alloc::bytes::new;use alloc::bytes::try_reserve;
fn hold(p:&ByteBuf,s:GrowStatus)->i32{return 0;}
fn grow(p:&mut ByteBuf)->GrowStatus{return try_reserve(&mut *p,1);}
fn main()->i32{let mut b=new();return hold(&b,grow(&mut b));}",
    ] {
        let project = project(text);
        let errors = check_enum_source(SourceOwner::project(&project)).unwrap_err();
        assert!(!errors.is_empty());
        assert!(errors.iter().all(|error| error.code != "E0500"), "{text}: {errors:?}");
    }
}

#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn file_source_exact_memory_boundary_refuses_before_truncation() {
    use std::os::unix::ffi::OsStrExt;
    let target = std::env::temp_dir().join(format!(
        "oxid-file-memory-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let literal = target
        .as_os_str()
        .as_bytes()
        .iter()
        .map(|byte| format!("\\x{byte:02X}"))
        .collect::<String>();
    let project = project(&format!(
        r#"
use std::fs::OpenResult; use std::fs::create;
fn main()->i32{{let path=b"{literal}";let result=create(&path);
match result{{OpenResult::Opened(file)=>{{return 7;}},OpenResult::Error(code)=>{{return -1;}},
OpenResult::MemoryLimit=>{{return -2;}},OpenResult::AllocationFailed=>{{return -3;}},}}}}
"#
    ));
    let (program, entry) = check_enum_source(SourceOwner::project(&project)).unwrap();
    let probe = |bytes| {
        std::fs::write(&target, b"untouched").unwrap();
        let (result, ledger) = byte_buffer::testing::capture(Default::default(), || {
            execute::run_limits(
                &program.witness,
                entry,
                execute::Limits {
                    frames: 2,
                    bytes,
                    ..Default::default()
                },
            )
        });
        assert_eq!(ledger.after_cleanup.live_bytes, 0);
        assert_eq!(
            ledger.after_cleanup.allocations,
            ledger.after_cleanup.releases
        );
        match result {
            Ok(value) => {
                assert_eq!(value, Scalar::I32(7));
                assert!(std::fs::read(&target).unwrap().is_empty());
                true
            }
            Err(error) => {
                let diagnostic = error.diagnostic(project.sources());
                assert_eq!(
                    (diagnostic.code, diagnostic.stage),
                    ("E0605", "oir-owned-run")
                );
                assert_eq!(ledger.after_cleanup.allocations, 0);
                assert_eq!(std::fs::read(&target).unwrap(), b"untouched");
                false
            }
        }
    };
    let (mut lower, mut upper) = (0, 1024 * 1024);
    assert!(probe(upper));
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        if probe(middle) {
            upper = middle;
        } else {
            lower = middle + 1;
        }
    }
    assert!(lower > 0);
    assert!(!probe(lower - 1));
    assert!(probe(lower));
    std::fs::remove_file(target).unwrap();
}
