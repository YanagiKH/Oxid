use crate::tensor::TensorResult;
use crate::tensor::Error;
use crate::tensor::model::Model;
use crate::tensor::model::Random;
use crate::tensor::data::Batch;
use crate::options::Options;
use crate::reader::Result as StreamResult;
use crate::reader::Error as StreamError;
use crate::reader::open_at;
use crate::reader::next;
use crate::reader::remaining;
use crate::reader::feature_count;
use crate::reader::target_count;
use crate::reader::row_count;
pub struct Summary { pub epochs: i32, pub rows: i32, pub batches: u64, pub initial_loss: f64, pub final_loss: f64 }
fn converted(problem: StreamError) -> Error { match problem { StreamError::Tensor(cause) => { return cause; }, StreamError::Io(code) => { return Error::IoError(code); }, StreamError::InvalidHeader => { return Error::InvalidStorage; }, StreamError::Truncated => { return Error::InvalidStorage; }, StreamError::TrailingData => { return Error::InvalidStorage; }, StreamError::InvalidBatchSize => { return Error::NegativeShape; }, StreamError::Poisoned => { return Error::InvalidStorage; }, StreamError::End => { return Error::InvalidStorage; }, StreamError::InvalidStartRow => { return Error::Bounds; }, } }
fn create_model(path: &[u8], size: i32, start_row: i32) -> TensorResult<Model>{ let opened = open_at(&*path, size, start_row); match opened { StreamResult::Ok(reader) => { return crate::tensor::model::initialize(feature_count(&reader), 8, target_count(&reader), 42u32); }, StreamResult::Err(problem) => { return TensorResult::<Model>:: Err(converted(problem)); }, } }
fn count_rows(path: &[u8], size: i32, start_row: i32) -> TensorResult<i32>{ let opened = open_at(&*path, size, start_row); match opened { StreamResult::Ok(reader) => { return TensorResult::Ok(remaining(&reader)); }, StreamResult::Err(problem) => { return TensorResult::<i32>:: Err(converted(problem)); }, } }
fn evaluate(path: &[u8], size: i32, start_row: i32, expected_rows: i32, model: &Model) -> TensorResult<f64>{
    let opened = open_at(&*path, size, start_row); match opened {
        StreamResult::Ok(value) => { let mut reader = value; if row_count(&reader) - start_row != expected_rows { return TensorResult::<f64>:: Err(Error::ShapeMismatch); } let mut weighted = 0.0;
            while remaining(&reader) > 0 { let loaded = next(&mut reader); match loaded { StreamResult::Ok(batch) => { let measured = crate::tensor::model::loss(&*model, &batch.inputs, &batch.targets); match measured { TensorResult::Ok(loss) => { let count = crate::tensor::row_count(&batch.inputs); let weight = count.to_f64(); weighted = weighted + loss * weight; }, TensorResult::Err(error) => { return TensorResult::<f64>:: Err(error); }, } }, StreamResult::Err(problem) => { return TensorResult::<f64>:: Err(converted(problem)); }, } }
            let count = expected_rows; return TensorResult::Ok(weighted / count.to_f64()); }, StreamResult::Err(problem) => { return TensorResult::<f64>:: Err(converted(problem)); }, }
}
fn train_batch(model: &mut Model, batch: &Batch) -> TensorResult<u64>{
    let trained = crate::tensor::model::train_step(&mut *model, &*batch.inputs, &*batch.targets, 0.05);
    match trained {
        TensorResult::Ok(loss) => { return TensorResult::Ok(1u64); },
        TensorResult::Err(error) => { return TensorResult::<u64>:: Err(error); },
    }
}

fn train_window(model: &mut Model, window: Batch, size: i32, seed: u32) -> TensorResult<u64>{
    let Batch { inputs: inputs, targets: targets } = window;
    let combined = crate::tensor::data::create(inputs, targets);
    match combined {
        TensorResult::Ok(data) => {
            let initialized = crate::tensor::data::initialize_sampler(&data, size, true, seed);
            match initialized {
                TensorResult::Ok(value) => {
                    let mut sampler = value;
                    let batches = (crate::tensor::data::row_count(&data) - 1) / size + 1;
                    for index in 0..batches {
                        let selected = crate::tensor::data::next_batch(&data, &mut sampler);
                        match selected {
                            TensorResult::Ok(batch) => {
                                let trained = train_batch(&mut *model, &batch);
                                match trained {
                                    TensorResult::Ok(count) => {},
                                    TensorResult::Err(error) => { return TensorResult::<u64>:: Err(error); },
                                }
                            },
                            TensorResult::Err(error) => { return TensorResult::<u64>:: Err(error); },
                        }
                    }
                    return TensorResult::Ok(batches.to_u64_checked());
                },
                TensorResult::Err(error) => { return TensorResult::<u64>:: Err(error); },
            }
        },
        TensorResult::Err(error) => { return TensorResult::<u64>:: Err(error); },
    }
}

fn train_chunk(model: &mut Model, chunk: Batch, options: &Options, random: &mut Random) -> TensorResult<u64>{
    if !options.shuffle { return train_batch(&mut *model, &chunk); }
    let seed = crate::tensor::model::next_random(&mut *random);
    return train_window(&mut *model, chunk, options.size, seed);
}

fn epoch(options: &Options, expected_rows: i32, model: &mut Model, random: &mut Random) -> TensorResult<u64>{
    let mut read_size = options.size;
    if options.shuffle {
        read_size = expected_rows;
        // Full shuffle windows contain eight complete batches, so any short
        // batch remains the last one in the epoch. The product stays in i32.
        if options.size <= (expected_rows - 1) / 8 { read_size = options.size * 8; }
    }
    let opened = open_at(&*options.path, read_size, options.start_row);
    match opened {
        StreamResult::Ok(value) => {
            let mut reader = value;
            if remaining(&reader) != expected_rows { return TensorResult::<u64>:: Err(Error::ShapeMismatch); }
            let mut updates = 0u64;
            while remaining(&reader) > 0 {
                let loaded = next(&mut reader);
                match loaded {
                    StreamResult::Ok(chunk) => {
                        let trained = train_chunk(&mut *model, chunk, &*options, &mut *random);
                        match trained {
                            TensorResult::Ok(count) => { updates = updates + count; },
                            TensorResult::Err(error) => { return TensorResult::<u64>:: Err(error); },
                        }
                    },
                    StreamResult::Err(error) => { return TensorResult::<u64>:: Err(converted(error)); },
                }
            }
            return TensorResult::Ok(updates);
        },
        StreamResult::Err(error) => { return TensorResult::<u64>:: Err(converted(error)); },
    }
}

fn fit(options: &Options, rows: i32, model: &mut Model, initial: f64) -> TensorResult<Summary>{
    let mut random = crate::tensor::model::random(options.seed);
    let mut batches = 0u64;
    for index in 0..options.epochs {
        let trained = epoch(&*options, rows, &mut *model, &mut random);
        match trained {
            TensorResult::Ok(updates) => { batches = batches + updates; },
            TensorResult::Err(error) => { return TensorResult::<Summary>:: Err(error); },
        }
    }
    let final_value = evaluate(&*options.path, options.size, options.start_row, rows, &*model);
    match final_value {
        TensorResult::Ok(loss) => {
            return TensorResult::Ok(Summary {
                epochs: options.epochs, rows: rows, batches: batches,
                initial_loss: initial, final_loss: loss
            });
        },
        TensorResult::Err(error) => { return TensorResult::<Summary>:: Err(error); },
    }
}

pub fn run(options: &Options) -> TensorResult<Summary>{
    let counted = count_rows(&*options.path, options.size, options.start_row);
    match counted {
        TensorResult::Ok(rows) => {
            let made = create_model(&*options.path, options.size, options.start_row);
            match made {
                TensorResult::Ok(value) => {
                    let mut model = value;
                    let measured = evaluate(&*options.path, options.size, options.start_row, rows, &model);
                    match measured {
                        TensorResult::Ok(initial) => { return fit(&*options, rows, &mut model, initial); },
                        TensorResult::Err(error) => { return TensorResult::<Summary>:: Err(error); },
                    }
                },
                TensorResult::Err(error) => { return TensorResult::<Summary>:: Err(error); },
            }
        },
        TensorResult::Err(error) => { return TensorResult::<Summary>:: Err(error); },
    }
}
