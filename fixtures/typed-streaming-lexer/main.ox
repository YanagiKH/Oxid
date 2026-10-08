mod data;mod keywords;mod scanner;mod frame;
use std::io::read_stdin;use std::io::ReadStatus;
fn main()->i32 {
 let request=crate::frame::request();if request.status!=0{return request.status;}
 let magic=[76,88,83,49];let magicstatus=crate::frame::write(&magic);if magicstatus!=0{return magicstatus;}
 let keys=crate::keywords::make_keywords();let mut codes=crate::data::zeros();let classes=crate::data::class_table();
 let mut s=crate::scanner::State{mode:0,start:0,pos:0,size:0,first:0,second:0,third:0,count:0,base:0,emitted:0,limit:request.limit,diagnostic:0,end:0,out:crate::data::output_buffer(),outused:0};let mut eof=false;
 while !eof {let mut used=128;let result=read_stdin(&mut codes);match result{ReadStatus::Full=>{},ReadStatus::Eof(n)=>{used=n;eof=true;},ReadStatus::IoError=>{return 74;},}
 let base=s.pos;let next=base+used;if next>request.length{return 64;}if eof && next!=request.length{return 64;}
 s.base=base;let header=[69,used,base%256,base/256%256,base/65536%256,base/16777216];let hs=crate::frame::write(&header);if hs!=0{return hs;}let ws=crate::frame::write(&codes);if ws!=0{return ws;}
 if s.diagnostic==0{let status=crate::scanner::scan(&codes,used,&classes,&mut s,&keys);if status!=0 && status!=65{return status;}}
 if s.diagnostic!=0{if !crate::frame::ascii(&codes,used){return 64;}}
 s.pos=next;if !eof{let ps=crate::frame::partial(&mut s);if ps!=0{return ps;}}
 }
 if s.diagnostic==0{let status=crate::scanner::finish(&mut s,&keys);if status!=0 && status!=65{return status;}}
 let ps=crate::frame::partial(&mut s);if ps!=0{return ps;}return crate::frame::terminal(&s);
}
