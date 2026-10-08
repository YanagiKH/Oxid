pub struct State { pub mode:i32,pub start:i32,pub pos:i32,pub size:i32,pub first:i32,pub second:i32,pub third:i32,pub count:i32,pub base:i32,pub emitted:i32,pub limit:i32,pub diagnostic:i32,pub end:i32,pub out:[i32;100],pub outused:i32 }
fn flush(s:&mut State,kind:i32,start:i32,end:i32)->i32 {
 if kind!=47 && (end-start>65536 || s.count>=s.limit){s.start=start;s.end=end;s.diagnostic=3;return 65;}
 let at=s.outused;
 s.out[at]=kind;s.out[at+1]=end-s.base;
 s.outused=at+2;
 if s.outused==100{let status=crate::frame::write(&*s.out);if status!=0{return status;}s.outused=0;}
 s.count=s.count+1;return 0;
}
fn slow(codes:&[i32],used:i32,initial:i32,s:&mut State)->i32 {
 let mut i=initial;let mut mode=s.mode;let mut emitkind=0;
 if mode==4 || mode==50 {
   while i<used {let b=codes[i];if b>127{s.emitted=-64;return i;}i=i+1;
    if mode==50{mode=4;}else{if b==92{mode=50;}else{if b==34{emitkind=4;break;}}}
   }
  }else{if mode==48 {
   while i<used {let b=codes[i];if b>127{s.emitted=-64;return i;}if b==10{emitkind=1;break;}i=i+1;}
  }else{if mode==49 || mode==51 {
   while i<used {let b=codes[i];if b>127{s.emitted=-64;return i;}i=i+1;
    if mode==51 && b==47{emitkind=1;break;}if b==42{mode=51;}else{mode=49;}
   }
  }}}
 s.mode=mode;s.emitted=emitkind;return i;
}
fn ident(codes:&[i32],used:i32,classes:&[i32],initial:i32,s:&mut State,keys:&crate::keywords::Keywords)->i32 {
 let mut i=initial;let mut size=s.size;let mut first=s.first;let mut second=s.second;let mut third=s.third;
   while i<used {let b=codes[i];if b>127{s.emitted=-64;return i;}let kind=classes[b];if kind!=2 && kind!=3{break;}
    if size<4{first=first*128+b;}else{if size<8{second=second*128+b;}else{if size<11{third=third*128+b;}}}size=size+1;i=i+1;
   }

 if i<used{
  let mut kind=2;
  if size<=11{let at=(first%107+size*20)%127;let tag=keys.tag[at];
   if tag/64==size && keys.first[at]==first && keys.second[at]==second && keys.third[at]==third{kind=tag%64;}
  }
  s.emitted=kind;
 }else{s.size=size;s.first=first;s.second=second;s.third=third;s.emitted=0;}return i;
}
pub fn scan(codes:&[i32],used:i32,classes:&[i32],s:&mut State,keys:&crate::keywords::Keywords)->i32 {
 let mut i=0;let mut mode=s.mode;let mut start=s.start;let base=s.pos;

 while i<used {
  let mut emitkind=0;
  if mode==0 {
   let b=codes[i];if b>127{return 64;}mode=classes[b];start=base+i;
   if mode==2{s.size=1;s.first=b;s.second=0;s.third=0;}
   i=i+1;
   if mode>4 && mode<128{emitkind=mode;}
  }
  if emitkind==0{
  if mode==1 {
   while i<used {let b=codes[i];if b>127{return 64;}if classes[b]!=1{break;}i=i+1;}
   if i<used{emitkind=1;}
  }else{if mode==2 {
   i=ident(&*codes,used,&*classes,i,&mut *s,&*keys);emitkind=s.emitted;
   if emitkind<0{return 0-emitkind;}
  }else{if mode==3 {
   while i<used {let b=codes[i];if b>127{return 64;}let kind=classes[b];if kind!=2 && kind!=3 && b!=46{break;}i=i+1;}
   if i<used{emitkind=3;}
  }else{if mode>=128{
   if i<used{let b=codes[i];if b>127{return 64;}
    if b==mode/128%256{emitkind=mode/32768;i=i+1;if emitkind==48{mode=48;emitkind=0;}}
    else{if mode==1578923 && b==42{mode=49;i=i+1;}else{emitkind=mode%128;}}
   }
  }else{
   s.mode=mode;i=slow(&*codes,used,i,&mut *s);mode=s.mode;emitkind=s.emitted;
   if emitkind<0{return 0-emitkind;}
  }}}}
  }
  if emitkind>0{let hi=base+i; if hi-start>65536 || s.count>=s.limit{s.start=start;s.end=hi;s.diagnostic=3;return 65;}
 let at=s.outused;
 s.out[at]=emitkind;s.out[at+1]=i;
 s.outused=at+2;
 if s.outused==100{let status=crate::frame::write(&*s.out);if status!=0{return status;}s.outused=0;}
 s.count=s.count+1;
mode=0;}
 }
 s.mode=mode;s.start=start;return 0;
}
pub fn finish(s:&mut State,keys:&crate::keywords::Keywords)->i32{
 let mut kind=s.mode;if kind==4 || kind==50{s.end=s.pos;s.diagnostic=1;return 0;}if kind==49 || kind==51{s.end=s.pos;s.diagnostic=2;return 0;}
 if kind==2{kind=crate::keywords::keyword(s.size,s.first,s.second,s.third,&*keys);}if kind==48{kind=1;}
 if kind>=128{kind=kind%128;}let lo=s.start;let hi=s.pos;if kind!=0{let status=flush(&mut *s,kind,lo,hi);if status!=0{return status;}}
 s.start=s.pos;let status=flush(&mut *s,47,hi,hi);if status!=0{return status;}
 return 0;
}
