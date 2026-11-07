use alloc::vec::Vec;use alloc::vec::new;use alloc::vec::try_push;fn main()->i32{let mut values=new::<i32>();let wrong=1u8;try_push(&mut values,wrong);return 0;}
