use crate::tensor::TensorResult;
use crate::tensor::data::Dataset;
use crate::tensor::data::Sampler;

fn steps(data: &Dataset, ready: Sampler) -> i32 {
    let mut sampler = ready;
    for index in 0..2 {
        let next = crate::tensor::data::next_batch(&*data, &mut sampler);
        match next {
            TensorResult::Ok(batch) => {
                let mut expected = 2;
                if index == 1 { expected = 1; }
                if crate::tensor::row_count(&batch.inputs) != expected || crate::tensor::row_count(&batch.targets) != expected { return 1; }
            },
            TensorResult::Err(error) => { return 2; }
        }
    }
    if crate::tensor::data::epoch(&sampler) != 1u64 || crate::tensor::data::position(&sampler) != 0 { return 3; }
    let encoded = crate::tensor::data::sampler_checkpoint::encode(&sampler);
    match encoded {
        TensorResult::Ok(bytes) => {
            let decoded = crate::tensor::data::sampler_checkpoint::decode(&bytes);
            match decoded { TensorResult::Ok(restored) => { if crate::tensor::data::batch_count(&restored) != 2u64 { return 4; } return 0; }, TensorResult::Err(error) => { return 5; } }
        },
        TensorResult::Err(error) => { return 6; }
    }
}
fn run(data: Dataset) -> i32 {
    let made = crate::tensor::data::initialize_sampler(&data, 2, true, 1u32);
    match made { TensorResult::Ok(sampler) => { return steps(&data, sampler); }, TensorResult::Err(error) => { return 7; } }
}
fn main() -> i32 {
    let x = [0.0, 1.0, 2.0];
    let y = [3.0, 4.0, 5.0];
    let first = crate::tensor::from_slice(3, 1, &x);
    match first {
        TensorResult::Ok(inputs) => {
            let second = crate::tensor::from_slice(3, 1, &y);
            match second {
                TensorResult::Ok(targets) => {
                    let made = crate::tensor::data::create(inputs, targets);
                    match made { TensorResult::Ok(data) => { return run(data); }, TensorResult::Err(error) => { return 8; } }
                },
                TensorResult::Err(error) => { return 9; }
            }
        },
        TensorResult::Err(error) => { return 10; }
    }
}
