//! Nested record fields retain their root owner's call-only loan authority.
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Input(PathBuf);
impl Input {
    fn new(source: &str) -> Self {
        let input = Self(std::env::temp_dir().join(format!(
            "oxid-record-field-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        fs::create_dir(&input.0).unwrap();
        fs::write(input.0.join("main.ox"), source).unwrap();
        input
    }
    fn invoke(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .current_dir(&self.0)
            .args(args)
            .env("OXID_CACHE_DIR", self.0.join("cache"))
            .output()
            .unwrap()
    }
    fn run(source: &str, command: &str) -> std::process::Output {
        Self::new(source).invoke(&[command, "main.ox", "--edition=typed-preview"])
    }
}
impl Drop for Input {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn samples() -> [(&'static str, &'static [u8]); 14] {
    [
        (
            r#"struct Point { x:i32, y:i32 } struct Pair { prefix:u8, first:Point, second:Point, tail:i64 }
fn read(p:&Point)->i32{return p.x*10+p.y;} fn bump(p:&mut Point)->(){p.x=p.x+3; p.y=p.y+4;return;}
fn main()->i32{let mut pair=Pair{prefix:7u8,first:Point{x:1,y:2},second:Point{x:5,y:6},tail:99i64};
bump(&mut pair.first); return read(&pair.first)*100+read(&pair.second);}"#,
            b"4656\n",
        ),
        (
            r#"struct Inner { value:i32 } struct Middle { lead:i64, item:Inner, tail:i32 } struct Outer { pad:u8, middle:Middle }
fn increment(p:&mut Inner)->(){p.value=p.value+1;return;}
fn forward(p:&mut Middle)->(){increment(&mut *p.item);return;}
fn inspect(p:&Inner)->i32{return p.value;}
fn read(p:&Outer)->i32{return inspect(&*p.middle.item);}
fn main()->i32{let mut outer=Outer{pad:9u8,middle:Middle{lead:77i64,item:Inner{value:40},tail:88}};
forward(&mut outer.middle);forward(&mut outer.middle);return read(&outer);}"#,
            b"42\n",
        ),
        (
            r#"use alloc::bytes::ByteBuf;use alloc::bytes::GrowStatus;use alloc::bytes::new;use alloc::bytes::try_push;use alloc::bytes::len;
struct Vector { bytes:ByteBuf } struct Outer { pad:i64, data:Vector, tail:i32 }
fn push(v:&mut Vector)->i32{let status=try_push(&mut *v.bytes,42u8);match status{GrowStatus::Ok=>{return len(&*v.bytes);},GrowStatus::CapacityOverflow=>{return -1;},GrowStatus::AllocationFailed=>{return -2;},GrowStatus::MemoryLimit=>{return -3;},}}
fn forward(o:&mut Outer)->i32{return push(&mut *o.data);}
fn inspect(v:&Vector)->i32{return len(&*v.bytes);}
fn main()->i32{let mut outer=Outer{pad:7i64,data:Vector{bytes:new()},tail:9};let first=forward(&mut outer);let second=forward(&mut outer);return first*100+second*10+inspect(&outer.data);}"#,
            b"122\n",
        ),
        (
            r#"struct Inner{samples:[u8;4]}struct Outer{pad:i64,data:Inner,tail:bool}
fn first(p:&[u8])->i32{let value=p[0];return value.to_i32();}
fn through(p:&Inner)->i32{return first(&*p.samples[1..3]);}
fn relay(p:&Outer)->i32{return through(&*p.data);}
fn main()->i32{let outer=Outer{pad:99i64,data:Inner{samples:[11u8,22u8,33u8,44u8]},tail:true};return relay(&outer);}"#,
            b"22\n",
        ),
        (
            r#"use alloc::bytes::ByteBuf;use alloc::bytes::GrowStatus;use alloc::bytes::new;use alloc::bytes::try_push;
struct Inner{bytes:ByteBuf}struct Outer{pad:i64,data:Inner,tail:i32}
fn first(p:&[u8])->i32{let value=p[0];return value.to_i32();}
fn through(p:&Inner)->i32{return first(&*p.bytes[0..1]);}
fn relay(p:&Outer)->i32{return through(&*p.data);}
fn main()->i32{let mut bytes=new();let status=try_push(&mut bytes,42u8);match status{GrowStatus::Ok=>{},GrowStatus::CapacityOverflow=>{return -1;},GrowStatus::AllocationFailed=>{return -2;},GrowStatus::MemoryLimit=>{return -3;},}let outer=Outer{pad:99i64,data:Inner{bytes:bytes},tail:7};return relay(&outer);}"#,
            b"42\n",
        ),
        (
            r#"struct Leaf { pad:u8, flag:bool, signed:i128, wide:u128, single:f32, double:f64, scalar:char }
struct Branch { prefix:u8, target:Leaf, tail:i64 }
struct Holder { lead:i128, sibling:Leaf, branch:Branch, tail:u8 }
fn inspect(p:&Leaf)->bool {
let single=p.single;let double=p.double;
return p.pad==7u8 && p.flag && p.signed == -170141183460469231731687303715884105728i128
&& p.wide==340282366920938463463374607431768211455u128
&& single.to_bits()==2143289361u32 && double.to_bits()==18444492273895866434u64 && p.scalar=='🦀';}
fn change(p:&mut Leaf)->(){let tiny=1u32;p.pad=9u8;p.flag=false;p.signed=-1i128;p.wide=1u128;p.single=f32::from_bits(tiny);p.double=-0.0;p.scalar='中';return;}
fn forward(p:&mut Branch)->bool{if !inspect(&*p.target){return false;}change(&mut *p.target);return true;}
fn main()->i32{
let single_bits=2143289361u32;let double_bits=18444492273895866434u64;
let mut holder=Holder{lead:-123i128,sibling:Leaf{pad:2u8,flag:false,signed:3i128,wide:4u128,single:1.0f32,double:2.0,scalar:'A'},
branch:Branch{prefix:5u8,target:Leaf{pad:7u8,flag:true,signed:-170141183460469231731687303715884105728i128,wide:340282366920938463463374607431768211455u128,single:f32::from_bits(single_bits),double:f64::from_bits(double_bits),scalar:'🦀'},tail:6i64},tail:8u8};
if !forward(&mut holder.branch){return 1;}
let single=holder.branch.target.single;let double=holder.branch.target.double;
if holder.branch.target.pad!=9u8 || holder.branch.target.flag || holder.branch.target.signed != -1i128 || holder.branch.target.wide!=1u128 || single.to_bits()!=1u32 || double.to_bits()!=9223372036854775808u64 || holder.branch.target.scalar!='中'{return 2;}
if holder.lead != -123i128 || holder.sibling.pad!=2u8 || holder.sibling.flag || holder.sibling.signed!=3i128 || holder.sibling.wide!=4u128 || holder.sibling.single!=1.0f32 || holder.sibling.double!=2.0 || holder.sibling.scalar!='A' || holder.branch.prefix!=5u8 || holder.branch.tail!=6i64 || holder.tail!=8u8{return 3;}
return 0;}"#,
            b"0\n",
        ),
        (
            "struct Value{value:i32}struct Pair{left:Value,right:Value}\nfn change(a:&mut Value,b:&mut Value)->i32{a.value=a.value+10;b.value=b.value+20;return a.value*100+b.value;}\nfn main()->i32{let mut pair=Pair{left:Value{value:1},right:Value{value:2}};return change(&mut pair.left,&mut pair.right);}",
            b"1122\n",
        ),
        (
            "struct Value{value:i32}struct Pair{left:Value,right:Value}\nfn mix(a:&Value,b:&mut Value)->i32{b.value=b.value+a.value;return b.value;}\nfn main()->i32{let mut pair=Pair{left:Value{value:3},right:Value{value:4}};return mix(&pair.left,&mut pair.right);}",
            b"7\n",
        ),
        (
            "struct Leaf{value:i32}struct Branch{left:Leaf,right:Leaf}struct Outer{pad:i32,branch:Branch}\nfn swap(a:&mut Leaf,b:&mut Leaf)->(){let saved=a.value;a.value=b.value;b.value=saved;return;}\nfn forward(p:&mut Outer)->(){swap(&mut *p.branch.left,&mut *p.branch.right);return;}\nfn main()->i32{let mut outer=Outer{pad:9,branch:Branch{left:Leaf{value:7},right:Leaf{value:2}}};forward(&mut outer);return outer.branch.left.value*10+outer.branch.right.value;}",
            b"27\n",
        ),
        (
            "struct Value{value:i32}struct Pair{left:Value,right:Value}\nfn add(a:&mut Value,amount:i32)->i32{a.value=a.value+amount;return a.value;}\nfn main()->i32{let mut pair=Pair{left:Value{value:4},right:Value{value:3}};return add(&mut pair.left,pair.right.value);}",
            b"7\n",
        ),
        (
            "struct Value{value:i32}struct Pair{left:Value,right:Value}\nfn add(a:&mut Value,amount:i32)->i32{a.value=a.value+amount;return a.value;}\nfn observe(a:&mut Value)->i32{a.value=a.value+1;return a.value;}\nfn main()->i32{let mut pair=Pair{left:Value{value:4},right:Value{value:3}};return add(&mut pair.left,observe(&mut pair.right));}",
            b"8\n",
        ),
        (
            "struct Empty{}struct Pair{left:Empty,right:Empty}\nfn inspect(a:&mut Empty,b:&mut Empty)->i32{return 42;}\nfn main()->i32{let mut pair=Pair{left:Empty{},right:Empty{}};return inspect(&mut pair.left,&mut pair.right);}",
            b"42\n",
        ),
        (
            r#"use alloc::bytes::ByteBuf;use alloc::bytes::GrowStatus;use alloc::bytes::new;use alloc::bytes::try_push;use alloc::bytes::len;
struct Pair{left:ByteBuf,right:ByteBuf}
fn push(p:&mut ByteBuf,value:u8)->i32{let status=try_push(&mut *p,value);match status{GrowStatus::Ok=>{return len(&*p);},GrowStatus::CapacityOverflow=>{return -1;},GrowStatus::AllocationFailed=>{return -2;},GrowStatus::MemoryLimit=>{return -3;},}}
fn change(a:&mut ByteBuf,b:&mut ByteBuf)->i32{return push(&mut *a,11u8)*100+push(&mut *b,22u8);}
fn main()->i32{let mut pair=Pair{left:new(),right:new()};let first=change(&mut pair.left,&mut pair.right);let second=change(&mut pair.left,&mut pair.right);return first+second;}"#,
            b"303\n",
        ),
        (
            "struct Pair{left:[u8;3],right:[u8;3]}\nfn change(a:&mut [u8],b:&mut [u8])->i32{a[0]=a[0]+1u8;b[0]=b[0]+2u8;let left=a[0];let right=b[0];return left.to_i32()*100+right.to_i32();}\nfn main()->i32{let mut pair=Pair{left:[10u8,20u8,30u8],right:[30u8,40u8,50u8]};return change(&mut pair.left[1..3],&mut pair.right[0..2]);}",
            b"2132\n",
        ),
    ]
}

#[test]
fn shared_and_exclusive_record_fields_preserve_siblings_and_nominal_layout() {
    let source = samples()[0].0;
    let result = Input::run(source, "run");
    assert!(result.status.success(), "{result:?}");
    assert!(result.stderr.is_empty());
    assert_eq!(result.stdout, b"4656\n");
}

#[test]
fn projected_records_can_be_reborrowed_through_multiple_callees_and_paths() {
    let source = samples()[1].0;
    let result = Input::run(source, "run");
    assert!(result.status.success(), "{result:?}");
    assert!(result.stderr.is_empty());
    assert_eq!(result.stdout, b"42\n");
}

#[test]
fn nested_resource_views_retain_storage_through_record_reborrows() {
    let source = samples()[2].0;
    let result = Input::run(source, "run");
    assert!(result.status.success(), "{result:?}");
    assert!(result.stderr.is_empty());
    assert_eq!(result.stdout, b"122\n");
}

#[test]
fn nested_views_preserve_wide_scalars_float_bits_unicode_and_alignment() {
    let (source, expected) = samples()[5];
    let result = Input::run(source, "run");
    assert_eq!(
        (result.status.code(), result.stdout, result.stderr),
        (Some(0), expected.to_vec(), Vec::new())
    );
}

#[test]
fn field_loans_preserve_mutability_liveness_and_overlap_rejections() {
    let declarations = "struct Inner { value:i32 } struct Outer { first:Inner, second:Inner }
fn shared(p:&Inner)->i32{return p.value;}
fn exclusive(p:&mut Inner)->i32{p.value=p.value+1;return p.value;}
fn both(p:&mut Inner,q:&mut Inner)->i32{return p.value+q.value;}
fn mix(p:&Inner,q:&mut Inner)->i32{return p.value+q.value;}
fn owner(p:Outer)->i32{return p.first.value;}
fn whole(p:&mut Outer,q:&mut Inner)->i32{return p.first.value+q.value;}
fn reverse(p:&mut Inner,q:&mut Outer)->i32{return p.value+q.first.value;}
";
    for body in [
        "let outer=Outer{first:Inner{value:1},second:Inner{value:2}};return exclusive(&mut outer.first);",
        "let mut outer=Outer{first:Inner{value:1},second:Inner{value:2}};return both(&mut outer.first,&mut outer.first);",
        "let mut outer=Outer{first:Inner{value:1},second:Inner{value:2}};return mix(&outer.first,&mut outer.first);",
        "let mut outer=Outer{first:Inner{value:1},second:Inner{value:2}};return whole(&mut outer,&mut outer.first);",
        "let mut outer=Outer{first:Inner{value:1},second:Inner{value:2}};return reverse(&mut outer.first,&mut outer);",
        "let mut outer=Outer{first:Inner{value:1},second:Inner{value:2}};let moved=outer;return shared(&outer.first);",
    ] {
        let result = Input::run(&format!("{declarations} fn main()->i32{{{body}}}"), "check");
        assert!(!result.status.success(), "{result:?}");
    }
}

#[test]
fn disjoint_fields_support_exclusive_mixed_nested_and_left_to_right_access() {
    for (source, expected) in &samples()[6..] {
        for command in ["check", "run"] {
            let result = Input::run(source, command);
            assert!(result.status.success(), "{source}\n{result:?}");
            assert!(result.stderr.is_empty(), "{result:?}");
            if command == "run" {
                assert_eq!(result.stdout, *expected);
            } else {
                assert!(
                    result.stdout.starts_with(b"typed-preview check ok ("),
                    "{result:?}"
                );
            }
        }
    }
}

fn deeply_nested_sample(depth: usize) -> String {
    let mut source = String::from("struct Value{value:i32}struct Pair{left:Value,right:Value}\nfn change(a:&mut Value,b:&mut Value)->i32{a.value=a.value+10;b.value=b.value+20;return a.value*100+b.value;}\n");
    let mut ty = String::from("Pair");
    let mut value = String::from("Pair{left:Value{value:1},right:Value{value:2}}");
    for index in 0..depth {
        source.push_str(&format!("struct Layer{index}{{next:{ty}}}\n"));
        ty = format!("Layer{index}");
        value = format!("{ty}{{next:{value}}}");
        if index % 4 == 3 || index + 1 == depth {
            source.push_str(&format!("fn make{index}()->{ty}{{return {value};}}\n"));
            value = format!("make{index}()");
        }
    }
    let path = "root".to_string() + &".next".repeat(depth);
    source.push_str(&format!(
        "fn main()->i32{{let mut root={value};return change(&mut {path}.left,&mut {path}.right);}}"
    ));
    source
}

#[test]
fn distinct_fields_at_maximum_record_depth_stay_within_verifier_work_limits() {
    for depth in [1, 31, 62] {
        let source = deeply_nested_sample(depth);
        let result = Input::run(&source, "run");
        assert!(result.status.success(), "depth {depth}: {result:?}");
        assert_eq!(result.stdout, b"1122\n");
        assert!(result.stderr.is_empty());
    }
}

#[test]
fn parent_child_fields_and_whole_owner_moves_still_conflict() {
    let declarations = "struct Leaf{value:i32}struct Branch{left:Leaf,right:Leaf}struct Outer{branch:Branch}\nfn whole(a:&mut Branch,b:&mut Leaf)->i32{return b.value;}\nfn reverse(a:&mut Leaf,b:&mut Branch)->i32{return a.value;}\nfn consume(value:Outer)->i32{return value.branch.left.value;}\nfn hold(a:&mut Leaf,n:i32)->i32{return a.value+n;}\nfn hold_branch(a:&mut Branch,n:i32)->i32{return a.left.value+n;}\n";
    for expression in [
        "whole(&mut root.branch,&mut root.branch.left)",
        "reverse(&mut root.branch.left,&mut root.branch)",
        "hold(&mut root.branch.left,consume(root))",
        "hold_branch(&mut root.branch,root.branch.left.value)",
    ] {
        let source = format!("{declarations}fn main()->i32{{let mut root=Outer{{branch:Branch{{left:Leaf{{value:1}},right:Leaf{{value:2}}}}}};return {expression};}}");
        let result = Input::run(&source, "check");
        assert!(!result.status.success(), "{expression}: {result:?}");
        let error = String::from_utf8(result.stderr).unwrap();
        assert!(error.contains("E0311"), "{expression}: {error}");
    }
}

#[test]
fn slices_reborrow_from_array_and_buffer_fields_inside_projected_records() {
    for (source, expected) in &samples()[3..5] {
        let result = Input::run(source, "run");
        assert_eq!(
            (result.status.code(), result.stdout, result.stderr),
            (Some(0), expected.to_vec(), Vec::new())
        );
    }
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
#[ignore = "requires Linux x86_64 or ARM64 and LLVM 19.1.7"]
fn native_record_field_loans_match_reference_without_sources() {
    let deep = deeply_nested_sample(62);
    for policy in ["bounded", "hosted"] {
        for (source, expected) in samples()
            .into_iter()
            .chain([(deep.as_str(), &b"1122\n"[..])])
        {
            let input = Input::new(source);
            fs::write(input.0.join("oxid.toml"), format!("[project]\nname=\"record-fields\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\n[execution]\npolicy=\"{policy}\"\n")).unwrap();
            let reference = input.invoke(&["run"]);
            assert_eq!(
                (reference.status.code(), reference.stdout, reference.stderr),
                (Some(0), expected.to_vec(), Vec::new())
            );
            let deployment = input.0.join("deployment");
            fs::create_dir(&deployment).unwrap();
            for profile in ["dev", "release"] {
                let built = input.invoke(&["build", "--profile", profile]);
                assert!(built.status.success(), "{built:?}");
                fs::copy(
                    input.0.join(".oxid/bin/record-fields"),
                    deployment.join(profile),
                )
                .unwrap();
            }
            fs::remove_file(input.0.join("main.ox")).unwrap();
            fs::remove_file(input.0.join("oxid.toml")).unwrap();
            fs::remove_dir_all(input.0.join(".oxid")).unwrap();
            for profile in ["dev", "release"] {
                let actual = Command::new(deployment.join(profile))
                    .current_dir(&deployment)
                    .env_clear()
                    .output()
                    .unwrap();
                assert_eq!(
                    (actual.status.code(), actual.stdout, actual.stderr),
                    (Some(0), expected.to_vec(), Vec::new())
                );
            }
        }
    }
}
