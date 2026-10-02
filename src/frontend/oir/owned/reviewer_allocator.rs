//! Test-only wiring for the two independent allocation observers.
//! Raw tests count attempts, including null results. Source tests count only
//! successful allocation calls and their live/peak requested payload bytes.
use std::alloc::{GlobalAlloc, Layout, System};

struct ReviewerAllocator;
#[global_allocator]
static ALLOCATOR: ReviewerAllocator = ReviewerAllocator;

unsafe impl GlobalAlloc for ReviewerAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        super::reviewer_origins::note_alloc();
        let result = unsafe { System.alloc(layout) };
        if !result.is_null() {
            super::source::reviewer_source::account(layout.size() as isize, true);
        }
        result
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        super::reviewer_origins::note_alloc();
        let result = unsafe { System.alloc_zeroed(layout) };
        if !result.is_null() {
            super::source::reviewer_source::account(layout.size() as isize, true);
        }
        result
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        super::source::reviewer_source::account(-(layout.size() as isize), false);
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        super::reviewer_origins::note_alloc();
        let result = unsafe { System.realloc(pointer, layout, size) };
        if !result.is_null() {
            super::source::reviewer_source::account(size as isize - layout.size() as isize, true);
        }
        result
    }
}

#[cfg(test)]
#[path = "allocator_review_controls.rs"]
mod allocator_review_controls;
