use shared::number;
pub struct Record { n:i32 }
pub fn make()->Record { return Record { n:number() }; }
pub fn value(r:&Record)->i32 { return r.n; }
