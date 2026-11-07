use alloc::vec::new;use alloc::vec::try_resize;
fn tail(p:&[i32])->i32{return p[0]+p.len();}
fn forward(p:&[i32])->i32{return tail(&*p[2048..2049]);}
fn main()->i32{let mut v=new::<i32>();try_resize(&mut v,2049,31);return forward(&v);}
