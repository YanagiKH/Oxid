use std::io::read_stdin;use std::io::ReadStatus;use std::io::write_stdout;use std::io::WriteStatus;
pub struct Request { pub length:i32,pub limit:i32,pub status:i32 }
pub fn request()->Request {
 let mut h=[0,0,0,0,0,0,0,0,0,0,0,0];let result=read_stdin(&mut h);
 match result{ReadStatus::Full=>{},ReadStatus::Eof(n)=>{return Request{length:0,limit:0,status:64};},ReadStatus::IoError=>{return Request{length:0,limit:0,status:74};},}
 if h[0]!=76 || h[1]!=88 || h[2]!=73 || h[3]!=49 || h[7]!=0 || h[11]!=0{return Request{length:0,limit:0,status:64};}
 let length=h[4]+h[5]*256+h[6]*65536;let limit=h[8]+h[9]*256+h[10]*65536;
 if length>1048576 || limit>100000{return Request{length:0,limit:0,status:64};}
 return Request{length:length,limit:limit,status:0};
}
pub fn write(codes:&[i32])->i32 {let result=write_stdout(&*codes);match result {WriteStatus::Complete=>{return 0;},WriteStatus::InvalidInput=>{return 70;},WriteStatus::IoError(n)=>{return 74;},}}
pub fn partial(s:&mut crate::scanner::State)->i32{
 if s.outused>0{let head=[66,s.outused];let hs=write(&head);if hs!=0{return hs;}let status=write(&*s.out);if status!=0{return status;}s.outused=0;}return 0;
}

pub fn terminal(s:&crate::scanner::State)->i32 {
 if s.diagnostic!=0{let lo=s.start;let hi=s.end;let row=[68,s.diagnostic,lo%256,lo/256%256,lo/65536%256,lo/16777216,hi%256,hi/256%256,hi/65536%256,hi/16777216];return write(&row);}
 let count=s.count;let size=s.pos;let row=[83,count%256,count/256%256,count/65536%256,count/16777216,size%256,size/256%256,size/65536%256,size/16777216];return write(&row);
}
pub fn ascii(codes:&[i32],used:i32)->bool {let mut i=0;while i<used{if codes[i]>127{return false;}i=i+1;}return true;}
