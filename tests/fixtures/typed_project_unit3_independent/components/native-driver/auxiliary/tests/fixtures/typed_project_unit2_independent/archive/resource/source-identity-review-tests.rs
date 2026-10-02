//! Inject under frontend::source in the copied frozen tree.
use super::*;
use std::sync::atomic::{AtomicU64,Ordering};
#[test]
fn reviewer_identity_exhaustion_and_parallel_uniqueness() {
    let c=AtomicU64::new(u64::MAX-2);
    assert_eq!(allocate_source_identity(&c),Some(u64::MAX-2));assert_eq!(allocate_source_identity(&c),Some(u64::MAX-1));
    for _ in 0..3 {assert_eq!(allocate_source_identity(&c),None);assert_eq!(c.load(Ordering::Relaxed),u64::MAX);}
    let c=std::sync::Arc::new(AtomicU64::new(1));
    let workers:Vec<_>=(0..8).map(|_|{let c=c.clone();std::thread::spawn(move||(0..1000).map(|_|allocate_source_identity(&c).unwrap()).collect::<Vec<_>>())}).collect();
    let mut ids:Vec<_>=workers.into_iter().flat_map(|t|t.join().unwrap()).collect();ids.sort_unstable();assert_eq!(ids,(1..=8000).collect::<Vec<_>>());
}
