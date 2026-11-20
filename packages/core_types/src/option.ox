use crate::result::Result;

/// A value that may be absent.
pub enum Option<T> {
    None,
    Some(T),
}

/// Take the present value or use the supplied fallback.
pub fn unwrap_or<T>(value: Option<T>, fallback: T) -> T {
    match value {
        Option::None => { return fallback; },
        Option::Some(present) => { return present; },
    }
}

/// Convert absence into the supplied error.
pub fn ok_or<T, E>(value: Option<T>, error: E) -> Result<T, E> {
    match value {
        Option::None => { return Result::<T, E>::Err(error); },
        Option::Some(present) => { return Result::<T, E>::Ok(present); },
    }
}
