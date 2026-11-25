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
use crate::checkpoint::Progress;
use crate::checkpoint::Saved;
use std::process::termination_requested;
pub struct Summary { pub epochs: i32, pub rows: i32, pub batches: u64, pub initial_loss: f64, pub final_loss: f64 }
struct Measurement { loss: f64, digest: u32 }
fn converted(problem: StreamError) -> Error { match problem { StreamError::Tensor(cause) => { return cause; }, StreamError::Io(code) => { return Error::IoError(code); }, StreamError::InvalidHeader => { return Error::InvalidStorage; }, StreamError::Truncated => { return Error::InvalidStorage; }, StreamError::TrailingData => { return Error::InvalidStorage; }, StreamError::InvalidBatchSize => { return Error::NegativeShape; }, StreamError::Poisoned => { return Error::InvalidStorage; }, StreamError::End => { return Error::InvalidStorage; }, StreamError::InvalidStartRow => { return Error::Bounds; }, } }
fn create_model(path: &[u8], size: i32, start_row: i32) -> TensorResult<Model>{ let opened = open_at(&*path, size, start_row); match opened { StreamResult::Ok(reader) => { return crate::tensor::model::initialize(feature_count(&reader), 8, target_count(&reader), 42u32); }, StreamResult::Err(problem) => { return TensorResult::<Model>:: Err(converted(problem)); }, } }
fn count_rows(path: &[u8], size: i32, start_row: i32) -> TensorResult<i32>{ let opened = open_at(&*path, size, start_row); match opened { StreamResult::Ok(reader) => { return TensorResult::Ok(remaining(&reader)); }, StreamResult::Err(problem) => { return TensorResult::<i32>:: Err(converted(problem)); }, } }
fn interrupted() -> Error { return Error::IoError(4); }

fn evaluate(options: &Options, expected_rows: i32, model: &Model) -> TensorResult<Measurement>{
    let opened = open_at(&*options.path, options.size, options.start_row);
    match opened {
        StreamResult::Ok(value) => {
            let mut reader = value;
            if remaining(&reader) != expected_rows { return TensorResult::<Measurement>::Err(Error::ShapeMismatch); }
            if options.checkpoint_mode != 0 { crate::reader::begin_checksum(&mut reader); }
            let mut weighted = 0.0;
            while remaining(&reader) > 0 {
                if termination_requested() { return TensorResult::<Measurement>::Err(interrupted()); }
                let loaded = next(&mut reader);
                match loaded {
                    StreamResult::Ok(batch) => {
                        let measured = crate::tensor::model::loss(&*model, &batch.inputs, &batch.targets);
                        match measured {
                            TensorResult::Ok(loss) => {
                                let count = crate::tensor::row_count(&batch.inputs);
                                weighted = weighted + loss * count.to_f64();
                            },
                            TensorResult::Err(error) => { return TensorResult::<Measurement>::Err(error); },
                        }
                    },
                    StreamResult::Err(problem) => { return TensorResult::<Measurement>::Err(converted(problem)); },
                }
            }
            return TensorResult::Ok(Measurement { loss: weighted / expected_rows.to_f64(), digest: crate::reader::checksum(&reader) });
        },
        StreamResult::Err(problem) => { return TensorResult::<Measurement>::Err(converted(problem)); },
    }
}
fn train_batch(model: &mut Model, batch: &Batch) -> TensorResult<()>{
    let trained = crate::tensor::model::train_step(&mut *model, &*batch.inputs, &*batch.targets, 0.05);
    match trained {
        TensorResult::Ok(loss) => { return TensorResult::Ok(()); },
        TensorResult::Err(error) => { return TensorResult::<()>:: Err(error); },
    }
}

fn train_window(model: &mut Model, window: Batch, size: i32, seed: u32) -> TensorResult<()>{
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
                        if termination_requested() { return TensorResult::<()>::Err(interrupted()); }
                        let selected = crate::tensor::data::next_batch(&data, &mut sampler);
                        match selected {
                            TensorResult::Ok(batch) => {
                                let trained = train_batch(&mut *model, &batch);
                                match trained {
                                    TensorResult::Ok(done) => {},
                                    TensorResult::Err(error) => { return TensorResult::<()>:: Err(error); },
                                }
                            },
                            TensorResult::Err(error) => { return TensorResult::<()>:: Err(error); },
                        }
                    }
                    return TensorResult::Ok(());
                },
                TensorResult::Err(error) => { return TensorResult::<()>:: Err(error); },
            }
        },
        TensorResult::Err(error) => { return TensorResult::<()>:: Err(error); },
    }
}

fn train_chunk(model: &mut Model, chunk: Batch, options: &Options, random: &mut Random) -> TensorResult<()>{
    if !options.shuffle { return train_batch(&mut *model, &chunk); }
    let seed = crate::tensor::model::next_random(&mut *random);
    return train_window(&mut *model, chunk, options.size, seed);
}

fn epoch(options: &Options, progress: Progress, model: &mut Model, random: &mut Random) -> TensorResult<()>{
    let expected_rows = progress.rows;
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
            if remaining(&reader) != expected_rows { return TensorResult::<()>::Err(Error::ShapeMismatch); }
            if options.checkpoint_mode != 0 { crate::reader::begin_checksum(&mut reader); }
            while remaining(&reader) > 0 {
                if termination_requested() { return TensorResult::<()>::Err(interrupted()); }
                let loaded = next(&mut reader);
                match loaded {
                    StreamResult::Ok(chunk) => {
                        let trained = train_chunk(&mut *model, chunk, &*options, &mut *random);
                        match trained {
                            TensorResult::Ok(done) => {},
                            TensorResult::Err(error) => { return TensorResult::<()>::Err(error); },
                        }
                    },
                    StreamResult::Err(error) => { return TensorResult::<()>::Err(converted(error)); },
                }
            }
            if options.checkpoint_mode != 0 && crate::reader::checksum(&reader) != progress.dataset_checksum {
                return TensorResult::<()>::Err(Error::InvalidStorage);
            }
            return TensorResult::Ok(());
        },
        StreamResult::Err(error) => { return TensorResult::<()>::Err(converted(error)); },
    }
}

fn save_slot(options: &Options, slot: i32, model: &Model, progress: Progress) -> TensorResult<()> {
    if termination_requested() { return TensorResult::<()>::Err(interrupted()); }
    if slot == 0 { return crate::checkpoint::save(&*options.first, &*model, progress); }
    return crate::checkpoint::save(&*options.second, &*model, progress);
}

fn fit(options: &Options, model: &mut Model, saved: Progress, first_slot: i32, initial: f64) -> TensorResult<Summary>{
    if saved.epochs > 2147483647 - options.epochs { return TensorResult::<Summary>::Err(Error::StepOverflow); }
    let mut progress = saved;
    let mut slot = first_slot;
    let mut random = crate::tensor::model::random(progress.random_state);
    for index in 0..options.epochs {
        if termination_requested() { return TensorResult::<Summary>::Err(interrupted()); }
        let trained = epoch(&*options, progress, &mut *model, &mut random);
        match trained {
            TensorResult::Ok(done) => {},
            TensorResult::Err(error) => { return TensorResult::<Summary>:: Err(error); },
        }
        progress.epochs = progress.epochs + 1;
        if options.checkpoint_mode != 0 {
            progress.random_state = crate::tensor::model::random_state(&random);
            let written = save_slot(&*options, slot, &*model, progress);
            match written {
                TensorResult::Ok(done) => { slot = 1 - slot; },
                TensorResult::Err(error) => { return TensorResult::<Summary>::Err(error); },
            }
        }
    }
    let final_value = evaluate(&*options, progress.rows, &*model);
    match final_value {
        TensorResult::Ok(measurement) => {
            if options.checkpoint_mode != 0 && measurement.digest != progress.dataset_checksum {
                return TensorResult::<Summary>::Err(Error::InvalidStorage);
            }
            return TensorResult::Ok(Summary {
                epochs: progress.epochs, rows: progress.rows, batches: crate::tensor::model::update_count(&*model),
                initial_loss: initial, final_loss: measurement.loss,
            });
        },
        TensorResult::Err(error) => { return TensorResult::<Summary>:: Err(error); },
    }
}

fn fresh(options: &Options, rows: i32, initial: Model) -> TensorResult<Summary> {
    let mut model = initial;
    let measured = evaluate(&*options, rows, &model);
    match measured {
        TensorResult::Ok(measurement) => {
            let random = crate::tensor::model::random(options.seed);
            let progress = Progress { epochs: 0, rows: rows, size: options.size, start: options.start_row,
                seed: options.seed, shuffled: options.shuffle, random_state: crate::tensor::model::random_state(&random),
                dataset_checksum: measurement.digest };
            if options.checkpoint_mode != 0 {
                let written = save_slot(&*options, 0, &model, progress);
                match written { TensorResult::Ok(done) => {}, TensorResult::Err(error) => { return TensorResult::<Summary>::Err(error); }, }
            }
            return fit(&*options, &mut model, progress, 1, measurement.loss);
        },
        TensorResult::Err(error) => { return TensorResult::<Summary>::Err(error); },
    }
}

fn resume(options: &Options, rows: i32) -> TensorResult<Summary> {
    let loaded = crate::checkpoint::latest(&*options);
    match loaded {
        TensorResult::Ok(saved) => {
            let Saved { model: previous, progress: progress, slot: slot } = saved;
            if progress.rows != rows || progress.size != options.size || progress.start != options.start_row
                || progress.seed != options.seed || progress.shuffled != options.shuffle {
                return TensorResult::<Summary>::Err(Error::InvalidCheckpoint);
            }
            let mut model = previous;
            let measured = evaluate(&*options, rows, &model);
            match measured {
                TensorResult::Ok(measurement) => {
                    if measurement.digest != progress.dataset_checksum { return TensorResult::<Summary>::Err(Error::InvalidStorage); }
                    return fit(&*options, &mut model, progress, 1 - slot, measurement.loss);
                },
                TensorResult::Err(error) => { return TensorResult::<Summary>::Err(error); },
            }
        },
        TensorResult::Err(error) => { return TensorResult::<Summary>::Err(error); },
    }
}

pub fn run(options: &Options) -> TensorResult<Summary>{
    if options.checkpoint_mode == 1 {
        let ready = crate::checkpoint::empty(&*options);
        match ready { TensorResult::Ok(done) => {}, TensorResult::Err(error) => { return TensorResult::<Summary>::Err(error); }, }
    }
    let counted = count_rows(&*options.path, options.size, options.start_row);
    match counted {
        TensorResult::Ok(rows) => {
            if options.checkpoint_mode == 2 { return resume(&*options, rows); }
            let made = create_model(&*options.path, options.size, options.start_row);
            match made {
                TensorResult::Ok(model) => { return fresh(&*options, rows, model); },
                TensorResult::Err(error) => { return TensorResult::<Summary>:: Err(error); },
            }
        },
        TensorResult::Err(error) => { return TensorResult::<Summary>:: Err(error); },
    }
}
