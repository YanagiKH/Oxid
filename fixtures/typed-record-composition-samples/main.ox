mod model;
mod ops;
use crate::model::Meta;
use crate::model::Batch;

fn main() -> i32 {
    let meta = Meta { completed: 0 };
    let samples = [1, 2, 3];
    let mut batch = crate::ops::relay(Batch { samples: samples, meta: meta });
    crate::ops::forward(&mut batch);
    return crate::ops::inspect(&batch);
}
