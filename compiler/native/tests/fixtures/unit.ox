fn sink(value: ()) -> () { let ignored = value; return; }
fn main() -> () { sink(()); return; }
