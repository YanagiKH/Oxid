use alloc::bytes::ByteBuf;
use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::try_resize;
use crate::options::Options;
use crate::tensor::Error;
use crate::tensor::TensorResult;
use crate::tensor::model::Model;
use crate::tensor::model::update_count;
use crate::tensor::codec::checksum;
use crate::tensor::codec::get32;
use crate::tensor::codec::put32;

#[derive(Copy)]
pub struct Progress {
    pub epochs: i32,
    pub rows: i32,
    pub size: i32,
    pub start: i32,
    pub seed: u32,
    pub shuffled: bool,
    pub random_state: u32,
    pub dataset_checksum: u32,
}

pub struct Saved { pub model: Model, pub progress: Progress, pub slot: i32 }

enum Loaded { Found(Saved), Unavailable, Error(Error) }

fn allocate(length: i32) -> TensorResult<ByteBuf> {
    let mut bytes = new();
    let grown = try_resize(&mut bytes, length, 0u8);
    match grown {
        GrowStatus::Ok => { return TensorResult::Ok(bytes); },
        GrowStatus::CapacityOverflow => { return TensorResult::<ByteBuf>::Err(Error::SizeOverflow); },
        GrowStatus::MemoryLimit => { return TensorResult::<ByteBuf>::Err(Error::MemoryLimit); },
        GrowStatus::AllocationFailed => { return TensorResult::<ByteBuf>::Err(Error::AllocationFailed); },
    }
}

fn copy(source: &[u8], destination: &mut [u8]) -> () {
    for index in 0..source.len() { destination[index] = source[index]; }
    return;
}

fn frame(model: &[u8], progress: Progress) -> TensorResult<ByteBuf> {
    if model.len() > 16777216 - 44 { return TensorResult::<ByteBuf>::Err(Error::SizeOverflow); }
    let end = 40 + model.len();
    let allocated = allocate(end + 4);
    match allocated {
        TensorResult::Ok(bytes) => {
            let mut output = bytes;
            let magic = b"OXSTR001";
            copy(&magic, &mut output[0..8]);
            let epochs = progress.epochs;
            let rows = progress.rows;
            let size = progress.size;
            let start = progress.start;
            put32(&mut output, 8, epochs.to_u32_checked());
            put32(&mut output, 12, rows.to_u32_checked());
            put32(&mut output, 16, size.to_u32_checked());
            put32(&mut output, 20, start.to_u32_checked());
            put32(&mut output, 24, progress.seed);
            if progress.shuffled { put32(&mut output, 28, 1u32); }
            put32(&mut output, 32, progress.random_state);
            put32(&mut output, 36, progress.dataset_checksum);
            copy(&*model, &mut output[40..end]);
            let digest = checksum(&output, end);
            put32(&mut output, end, digest);
            return TensorResult::Ok(output);
        },
        TensorResult::Err(error) => { return TensorResult::<ByteBuf>::Err(error); },
    }
}

pub fn save(path: &[u8], model: &Model, progress: Progress) -> TensorResult<()> {
    let encoded = crate::tensor::model::checkpoint::encode(&*model);
    match encoded {
        TensorResult::Ok(bytes) => {
            let packed = frame(&bytes, progress);
            match packed {
                TensorResult::Ok(payload) => { return crate::checkpoint_files::save_bytes(&*path, &payload); },
                TensorResult::Err(error) => { return TensorResult::<()>::Err(error); },
            }
        },
        TensorResult::Err(error) => { return TensorResult::<()>::Err(error); },
    }
}

fn unpack(bytes: &[u8], slot: i32) -> Loaded {
    if bytes.len() <= 44 { return Loaded::Unavailable; }
    let magic = b"OXSTR001";
    for index in 0..8 { if bytes[index] != magic[index] { return Loaded::Unavailable; } }
    let end = bytes.len() - 4;
    if get32(&*bytes, end) != checksum(&*bytes, end) { return Loaded::Unavailable; }
    let epochs = get32(&*bytes, 8);
    let rows = get32(&*bytes, 12);
    let size = get32(&*bytes, 16);
    let start = get32(&*bytes, 20);
    let seed = get32(&*bytes, 24);
    let shuffled = get32(&*bytes, 28);
    let random_state = get32(&*bytes, 32);
    if epochs > 2147483647u32 || rows == 0u32 || rows > 2147483647u32
        || size == 0u32 || size > 2147483647u32 || start > 2147483647u32
        || shuffled > 1u32 || random_state == 0u32 { return Loaded::Unavailable; }
    let decoded = crate::tensor::model::checkpoint::decode(&*bytes[40..end]);
    match decoded {
        TensorResult::Ok(model) => {
            let batches = (rows - 1u32) / size + 1u32;
            if update_count(&model) != epochs.to_u64_checked() * batches.to_u64_checked() { return Loaded::Unavailable; }
            let progress = Progress {
                epochs: epochs.to_i32_checked(), rows: rows.to_i32_checked(), size: size.to_i32_checked(),
                start: start.to_i32_checked(), seed: seed, shuffled: shuffled == 1u32,
                random_state: random_state, dataset_checksum: get32(&*bytes, 36),
            };
            return Loaded::Found(Saved { model: model, progress: progress, slot: slot });
        },
        TensorResult::Err(error) => { return read_error(error); },
    }
}

fn read_error(error: Error) -> Loaded {
    let checked = crate::checkpoint_files::recoverable(error);
    match checked {
        TensorResult::Ok(done) => { return Loaded::Unavailable; },
        TensorResult::Err(cause) => { return Loaded::Error(cause); },
    }
}

fn load(path: &[u8], slot: i32) -> Loaded {
    let read = crate::checkpoint_files::load_bytes(&*path);
    match read {
        TensorResult::Ok(bytes) => { return unpack(&bytes, slot); },
        TensorResult::Err(error) => { return read_error(error); },
    }
}

pub fn latest(options: &Options) -> TensorResult<Saved> {
    let first = load(&*options.first, 0);
    match first {
        Loaded::Error(error) => { return TensorResult::<Saved>::Err(error); },
        Loaded::Unavailable => {
            let second = load(&*options.second, 1);
            match second {
                Loaded::Found(value) => { return TensorResult::Ok(value); },
                Loaded::Unavailable => { return TensorResult::<Saved>::Err(Error::InvalidCheckpoint); },
                Loaded::Error(error) => { return TensorResult::<Saved>::Err(error); },
            }
        },
        Loaded::Found(value) => {
            let second = load(&*options.second, 1);
            match second {
                Loaded::Found(other) => {
                    if other.progress.epochs > value.progress.epochs { return TensorResult::Ok(other); }
                    return TensorResult::Ok(value);
                },
                Loaded::Unavailable => { return TensorResult::Ok(value); },
                Loaded::Error(error) => { return TensorResult::<Saved>::Err(error); },
            }
        },
    }
}

pub fn empty(options: &Options) -> TensorResult<()> {
    let first = crate::checkpoint_files::exists(&*options.first);
    match first {
        TensorResult::Ok(found) => { if found { return TensorResult::<()>::Err(Error::InvalidCheckpoint); } },
        TensorResult::Err(error) => { return TensorResult::<()>::Err(error); },
    }
    let second = crate::checkpoint_files::exists(&*options.second);
    match second {
        TensorResult::Ok(found) => {
            if found { return TensorResult::<()>::Err(Error::InvalidCheckpoint); }
            return TensorResult::Ok(());
        },
        TensorResult::Err(error) => { return TensorResult::<()>::Err(error); },
    }
}
