use alloc::vec::Vec;use alloc::vec::new;use alloc::vec::try_push;fn duplicate<T:Copy>(value:T)->T{return value;}fn main()->i32{let values=new::<i32>();let copied=duplicate(values);return 0;}
