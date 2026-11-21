use crate::option::Option;

/// A successful value or an error.
pub enum Result<T, E> {
    Ok(T),
    Err(E),
}

/// Take the successful value or use the supplied fallback.
pub fn unwrap_or<T, E>(value: Result<T, E>, fallback: T) -> T {
    match value {
        Result::Ok(present) => { return present; },
        Result::Err(error) => { return fallback; },
    }
}

/// Preserve the successful value, if any.
pub fn ok<T, E>(value: Result<T, E>) -> Option<T> {
    match value {
        Result::Ok(present) => { return Option::<T>::Some(present); },
        Result::Err(error) => { return Option::<T>::None; },
    }
}

/// Preserve the error, if any.
pub fn err<T, E>(value: Result<T, E>) -> Option<E> {
    match value {
        Result::Ok(present) => { return Option::<E>::None; },
        Result::Err(error) => { return Option::<E>::Some(error); },
    }
}
