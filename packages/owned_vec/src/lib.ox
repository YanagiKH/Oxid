use alloc::vec::Vec;
use alloc::vec::GrowStatus;
use alloc::vec::len;
use alloc::vec::try_reserve;
use alloc::vec::push_reserved;
use alloc::vec::pop_last;
use alloc::vec::take;

pub fn transfer<T>(values: Vec<T>) -> Vec<T> {
    return values;
}

pub fn detach<T>(values: &mut Vec<T>) -> Vec<T> {
    return take(&mut *values);
}

pub fn discard<T>(values: Vec<T>) -> () {
    return;
}

pub struct PushFailure<T> {
    pub value: T,
    pub status: GrowStatus,
}

pub enum PushResult<T> {
    Pushed,
    Failed(PushFailure<T>),
}

pub enum PopResult<T> {
    Empty,
    Value(T),
}

pub fn try_push<T>(values: &mut Vec<T>, value: T) -> PushResult<T> {
    let status = try_reserve(&mut *values, 1);
    match status {
        GrowStatus::Ok => {
            push_reserved(&mut *values, value);
            return PushResult::<T>::Pushed;
        },
        GrowStatus::CapacityOverflow => {
            return PushResult::<T>::Failed(PushFailure::<T> {
                value: value,
                status: GrowStatus::CapacityOverflow,
            });
        },
        GrowStatus::AllocationFailed => {
            return PushResult::<T>::Failed(PushFailure::<T> {
                value: value,
                status: GrowStatus::AllocationFailed,
            });
        },
        GrowStatus::MemoryLimit => {
            return PushResult::<T>::Failed(PushFailure::<T> {
                value: value,
                status: GrowStatus::MemoryLimit,
            });
        },
    }
}

pub fn pop<T>(values: &mut Vec<T>) -> PopResult<T> {
    if len(&*values) == 0 {
        return PopResult::<T>::Empty;
    }
    return PopResult::<T>::Value(pop_last(&mut *values));
}
