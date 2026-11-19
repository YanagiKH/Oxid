//! Shared concrete source for local owning-record element references.
#![allow(dead_code)]
pub const IMPORTS: &str = r#"
use alloc::bytes::ByteBuf;
use alloc::bytes::new as bytes_new;
use alloc::bytes::try_push as bytes_push;
use alloc::bytes::try_extend as bytes_extend;
use alloc::bytes::len as bytes_len;
use alloc::vec::Vec;
use alloc::vec::new;
use alloc::vec::len;
use alloc::vec::capacity;
use alloc::vec::try_reserve;
use alloc::vec::push_reserved;
use alloc::vec::pop_last;
use alloc::vec::truncate;
use alloc::vec::take;
use alloc::vec::try_push as copy_push;
use alloc::vec::try_extend as copy_extend;
"#;
pub const SIMPLE: &str = r#"
struct Entry { value:i32, bytes:ByteBuf }
fn read(item:&Entry)->i32 { return item.value; }
fn bump(item:&mut Entry)->() { item.value=item.value+1; return; }
fn edit(values:&mut Vec<Entry>)->() {
    let item=&mut *values[1];
    item.value=40;
    bump(&mut *item);
    let forwarded=&mut *item;
    bump(&mut *forwarded);
    return;
}
fn once(ticks:&mut[i32])->i32 { ticks[0]=ticks[0]+1; return 0; }
fn main()->i32 {
    let mut values=new::<Entry>();
    try_reserve(&mut values,2);
    push_reserved(&mut values,Entry{value:7,bytes:bytes_new()});
    push_reserved(&mut values,Entry{value:8,bytes:bytes_new()});
    let mut ticks=[0];
    let mut selected=0;
    if true {
        let first=&values[once(&mut ticks)];
        let second=&values[selected];
        selected=1;
        let forwarded=&*first;
        if read(&*forwarded)!=7 || read(&*second)!=7 { return -1; }
        if ticks[0]!=1 || selected!=1 { return -2; }
    }
    edit(&mut values);
    if true {
        let last=&values[1];
        if read(&*last)!=42 { return -3; }
    }
    try_reserve(&mut values,9);
    let moved=pop_last(&mut values);
    return moved.value;
}
"#;
pub const CHILDREN: &str = r#"
#[derive(Copy)] struct Point { value:i32 }
struct Child { count:i32, bytes:ByteBuf }
struct Entry { value:i32, child:Child, numbers:Vec<i32>, points:Vec<Point>, bytes:[u8;17], source:[i32;9] }
fn first(bytes:&[u8])->u8 { return bytes[0]; }
fn last(bytes:&[u8])->u8 { return bytes[bytes.len()-1]; }
fn mutate(item:&mut Entry)->() {
    item.value=item.value+1;
    item.child.count=11;
    bytes_push(&mut *item.child.bytes,42u8);
    copy_push(&mut *item.numbers,3);
    item.numbers[0]=7;
    copy_push(&mut *item.points,Point{value:9});
    item.points[0]=Point{value:13};
    item.bytes[0]=26u8;
    bytes_extend(&mut *item.child.bytes,&*item.bytes);
    copy_extend(&mut *item.numbers,&*item.source);
    if bytes_len(&*item.child.bytes)!=18 || first(&*item.child.bytes)!=42u8 || last(&*item.child.bytes)!=255u8 { return; }
    return;
}
fn read(item:&Entry)->i32 {
    let point=item.points[0];
    if item.child.count!=11 || item.numbers[0]!=7 || point.value!=13 { return -4; }
    if item.numbers.len()!=10 || item.numbers[9]!=9 || item.points.len()!=1 || first(&*item.bytes)!=26u8 { return -5; }
    if bytes_len(&*item.child.bytes)!=18 || last(&*item.child.bytes)!=255u8 { return -9; }
    if first(&*item.child.bytes[0..1])!=42u8 { return -6; }
    return item.value+2;
}
fn make()->Entry { return Entry{value:39,child:Child{count:0,bytes:bytes_new()},numbers:new::<i32>(),points:new::<Point>(),bytes:[0u8,13u8,10u8,26u8,1u8,2u8,3u8,4u8,5u8,6u8,7u8,8u8,9u8,10u8,11u8,12u8,255u8],source:[1,2,3,4,5,6,7,8,9]}; }
fn main()->i32 {
    let mut values=new::<Entry>();
    try_reserve(&mut values,1);
    push_reserved(&mut values,make());
    if true {
        let item=&mut values[0];
        mutate(&mut *item);
        let child=&*item;
        if read(&*child)!=42 { return -7; }
    }
    try_reserve(&mut values,9);
    push_reserved(&mut values,make());
    truncate(&mut values,1);
    if true { let item=&values[0]; if read(&*item)!=42 { return -8; } }
    let moved=pop_last(&mut values);
    return read(&moved);
}
"#;
pub const BASE: &str = r#"
struct Entry { value:i32, bytes:ByteBuf }
fn read(item:&Entry)->i32 { return item.value; }
fn consume(values:Vec<Entry>)->() { return; }
fn main()->i32 {
    let mut values=new::<Entry>();
    try_reserve(&mut values,2);
    push_reserved(&mut values,Entry{value:42,bytes:bytes_new()});
    push_reserved(&mut values,Entry{value:7,bytes:bytes_new()});
    if true {
        let item=&values[0];
        __ACTION__
        if read(&*item)!=42 { return -1; }
    }
    return 42;
}
"#;
pub const LOOP: &str = r#"
struct Entry { value:i32, bytes:ByteBuf }
fn main()->i32 {
    let mut values=new::<Entry>();try_reserve(&mut values,1);
    push_reserved(&mut values,Entry{value:0,bytes:bytes_new()});
    let mut i=0;
    while i<8 {
        let item=&mut values[0];
        item.value=item.value+1;
        i=i+1;
        if i<7 { continue; }
        if i==7 { break; }
    }
    try_reserve(&mut values,9);
    let item=pop_last(&mut values);
    return item.value+35;
}
"#;

#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
pub const FILE: &str = r#"
use std::fs::File;
use std::fs::OpenResult;
use std::fs::FileStatus;
use std::fs::create;
use std::fs::write_some as file_write;
use std::fs::sync_all;
use std::fs::close;
use std::io::WriteSomeStatus;
struct Entry { file:File, bytes:[u8;6] }
fn write(item:&mut Entry)->i32 {
    item.bytes[0]=42u8;
    let result=file_write(&mut *item.file,&*item.bytes);
    match result { WriteSomeStatus::Written(count)=>{if count!=6{return -1;}}, WriteSomeStatus::Error(code)=>{return -2;}, }
    let synced=sync_all(&mut *item.file);
    match synced { FileStatus::Ok=>{}, FileStatus::Error(code)=>{return -3;}, }
    let closed=close(&mut *item.file);
    match closed { FileStatus::Ok=>{}, FileStatus::Error(code)=>{return -4;}, }
    return 42;
}
fn main()->i32 {
    let path=b__ELEMENT_FILE_PATH__;
    let opened=create(&path);
    match opened {
        OpenResult::Opened(file)=>{
            let mut values=new::<Entry>();try_reserve(&mut values,1);
            push_reserved(&mut values,Entry{file:file,bytes:[99u8,0u8,13u8,10u8,26u8,255u8]});
            if true { let item=&mut values[0]; if write(&mut *item)!=42 {return -5;} }
            try_reserve(&mut values,2);
            truncate(&mut values,0);
            return 42;
        },
        OpenResult::Error(code)=>{return -6;}, OpenResult::MemoryLimit=>{return -7;}, OpenResult::AllocationFailed=>{return -8;},
    }
}
"#;

pub const REALLOCATING_INDEX: &str = r#"
struct Entry { value:i32, bytes:ByteBuf }
fn choose(values:&mut Vec<Entry>)->i32 { try_reserve(&mut *values,17); return len(&*values)-1; }
fn main()->i32 {
    let mut values=new::<Entry>();try_reserve(&mut values,2);
    push_reserved(&mut values,Entry{value:7,bytes:bytes_new()});
    push_reserved(&mut values,Entry{value:42,bytes:bytes_new()});
    let item=&values[choose(&mut values)];
    return item.value;
}
"#;
pub const GENERIC_RECORD: &str = r#"
struct Envelope<T> { value:T, bytes:ByteBuf }
fn read(item:&Envelope<i32>)->i32 { return item.value; }
fn main()->i32 {
    let mut values=new::<Envelope<i32>>();try_reserve(&mut values,1);
    push_reserved(&mut values,Envelope::<i32>{value:7,bytes:bytes_new()});
    if true { let item:&mut Envelope<i32> = &mut values[0]; item.value=42; }
    if true { let item:&Envelope<i32> = &values[0]; if read(&*item)!=42 {return -1;} }
    let owned=pop_last(&mut values);return owned.value;
}
"#;

pub const EMPTY_RECORD: &str = r#"
struct Empty {}
fn inspect(item:&Empty)->i32 { return 42; }
fn main()->i32 {
    let mut values=new::<Empty>();try_reserve(&mut values,1);push_reserved(&mut values,Empty{});
    let item=&values[0];return inspect(&*item);
}
"#;

pub const EMPTY_END_SLICES: &str = r#"
struct Entry { bytes:[u8;4] }
fn empty(bytes:&[u8])->i32 { return bytes.len(); }
fn forward(bytes:&[u8])->i32 { return empty(&*bytes[0..0]); }
fn forward_mut(bytes:&mut[u8])->i32 { return forward(&*bytes[0..0]); }
fn inspect(item:&Entry)->i32 {
    if forward(&*item.bytes[0..0])!=0 {return -1;}
    return forward(&*item.bytes[4..4]);
}
fn edit(item:&mut Entry)->i32 { return forward_mut(&mut *item.bytes[4..4]); }
fn main()->i32 {
    let mut values=new::<Entry>();try_reserve(&mut values,1);push_reserved(&mut values,Entry{bytes:[0u8,13u8,10u8,26u8]});
    if true {let item=&values[0];if inspect(&*item)!=0{return -2;}}
    if true {let item=&mut values[0];if edit(&mut *item)!=0{return -3;}}
    return 42;
}
"#;
