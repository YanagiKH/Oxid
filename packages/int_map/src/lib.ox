use alloc::vec::Vec;
use alloc::vec::GrowStatus;
use alloc::vec::new as new_vector;
use alloc::vec::len as vector_len;
use alloc::vec::try_push;
use alloc::vec::truncate;
use crate::types::option::Option;

#[derive(Copy)]
struct Entry<T: Copy> { key: i32, value: T }

pub struct IntMap<T: Copy> { entries: Vec<Entry<T>> }

pub fn new<T: Copy>() -> IntMap<T> {
    return IntMap::<T> { entries: new_vector::<Entry<T>>() };
}

pub fn len<T: Copy>(map: &IntMap<T>) -> i32 {
    return vector_len(&*map.entries);
}

fn lower_bound<T: Copy>(entries: &Vec<Entry<T>>, key: i32) -> i32 {
    let mut low = 0;
    let mut high = vector_len(&*entries);
    while low < high {
        let middle = low + (high - low) / 2;
        let entry = entries[middle];
        if entry.key < key { low = middle + 1; }
        else { high = middle; }
    }
    return low;
}

fn find<T: Copy>(entries: &Vec<Entry<T>>, key: i32) -> Option<T> {
    let position = lower_bound(&*entries, key);
    if position < vector_len(&*entries) {
        let Entry { key: stored_key, value: stored } = entries[position];
        if stored_key == key { return Option::<T>::Some(stored); }
    }
    return Option::<T>::None;
}

pub fn get<T: Copy>(map: &IntMap<T>, key: i32) -> Option<T> {
    return find(&*map.entries, key);
}

fn insert_entry<T: Copy>(entries: &mut Vec<Entry<T>>, key: i32, value: T) -> GrowStatus {
    let position = lower_bound(&*entries, key);
    let length = vector_len(&*entries);
    let new_entry = Entry::<T> { key: key, value: value };
    if position < length {
        let existing = entries[position];
        if existing.key == key {
            entries[position] = new_entry;
            return GrowStatus::Ok;
        }
    }
    let status = try_push(&mut *entries, new_entry);
    match status {
        GrowStatus::Ok => {},
        GrowStatus::CapacityOverflow => { return GrowStatus::CapacityOverflow; },
        GrowStatus::AllocationFailed => { return GrowStatus::AllocationFailed; },
        GrowStatus::MemoryLimit => { return GrowStatus::MemoryLimit; },
    }
    let mut at = length;
    while at > position {
        entries[at] = entries[at - 1];
        at = at - 1;
    }
    entries[position] = new_entry;
    return GrowStatus::Ok;
}

pub fn try_insert<T: Copy>(map: &mut IntMap<T>, key: i32, value: T) -> GrowStatus {
    return insert_entry(&mut *map.entries, key, value);
}

fn remove_entry<T: Copy>(entries: &mut Vec<Entry<T>>, key: i32) -> Option<T> {
    let position = lower_bound(&*entries, key);
    let length = vector_len(&*entries);
    if position == length { return Option::<T>::None; }
    let Entry { key: removed_key, value: removed } = entries[position];
    if removed_key != key { return Option::<T>::None; }
    let mut at = position;
    let last = length - 1;
    while at < last {
        entries[at] = entries[at + 1];
        at = at + 1;
    }
    truncate(&mut *entries, last);
    return Option::<T>::Some(removed);
}

pub fn remove<T: Copy>(map: &mut IntMap<T>, key: i32) -> Option<T> {
    return remove_entry(&mut *map.entries, key);
}
