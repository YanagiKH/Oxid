use alloc::vec::Vec;use alloc::vec::new;use alloc::vec::len;use alloc::vec::capacity;
use alloc::vec::try_reserve;use alloc::vec::try_push;use alloc::vec::try_extend;
use alloc::vec::try_resize;use alloc::vec::truncate;use alloc::vec::take;
struct Holder{prefix:i32,values:Vec<i32>}
fn score(p:&[i32])->i32{let a=p[0];let b=p[1];let c=p[2];return a+b+c+p.len();}
fn forward(p:&[i32])->i32{return score(&*p[0..3]);}
fn main()->i32{
let mut v=new::<i32>();try_push(&mut v,7);let extra=[11,13];try_extend(&mut v,&extra);
try_resize(&mut v,5,17);v[2]=19;
let mut h=Holder{prefix:99,values:take(&mut v)};
truncate(&mut h.values,4);try_reserve(&mut h.values,20);
return h.values[0]+forward(&h.values[1..4])+len(&h.values)+capacity(&h.values)+len(&v)+capacity(&v);
}
