struct R{values:[i32;4]}
fn score(p:&[i32])->i32{let a=p[0];let b=p[1];return a+b+p.len();}
fn projected(r:&R)->i32{return score(&*r.values[1..3]);}
fn forward(p:&[i32])->i32{return score(&*p[1..3]);}
fn edit(p:&mut[i32])->(){p[0]=77;return;}
fn main()->i32{let mut a=[10,20,30,40];edit(&mut a[2..3]);let r=R{values:[1,2,3,4]};
return score(&a[1..3])+forward(&a)+projected(&r);}
