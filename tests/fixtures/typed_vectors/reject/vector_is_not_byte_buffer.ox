use alloc::vec::Vec;use alloc::vec::new;use alloc::vec::try_push;use alloc::bytes::ByteBuf;fn needs(values:&ByteBuf)->i32{return 0;}fn main()->i32{let values=new::<u8>();return needs(&values);}
