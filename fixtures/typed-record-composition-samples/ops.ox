use crate::model::Batch;

pub fn relay(batch: Batch) -> Batch {
    return batch;
}

fn increment(batch: &mut Batch) -> () {
    let mut index = 0;
    while index < batch.samples.len() {
        batch.samples[index] = batch.samples[index] + 1;
        batch.meta.completed = batch.meta.completed + 1;
        index = index + 1;
    }
    return;
}

pub fn forward(batch: &mut Batch) -> () {
    increment(&mut *batch);
    return;
}

pub fn inspect(batch: &Batch) -> i32 {
    return batch.meta.completed * 100 + batch.samples[0] * 10 + batch.samples[2];
}
