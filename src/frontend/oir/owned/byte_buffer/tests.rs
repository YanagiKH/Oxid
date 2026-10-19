use super::*;

fn fuel() -> usize {
    1_000_000
}
fn fill(heap: &mut Heap, value: &mut Descriptor, bytes: &[u8]) {
    assert_eq!(
        heap.try_extend(value, bytes, &mut fuel()),
        Ok(GrowStatus::Ok)
    );
}
fn snapshot(heap: &Heap, value: &Descriptor) -> (u64, i32, i32, Vec<u8>) {
    (
        value.identity,
        value.length,
        value.capacity,
        heap.bytes(value).unwrap().to_vec(),
    )
}

#[test]
fn bytebuf_dynamic_growth_roundtrip_and_exact_accounting() {
    let mut heap = Heap::default();
    let mut b = Descriptor::empty();
    let input: Vec<_> = (0..8199).map(|n| ((n * 73 + 19) & 255) as u8).collect();
    for chunk in input.chunks(1024) {
        fill(&mut heap, &mut b, chunk);
    }
    assert_eq!(heap.bytes(&b).unwrap(), input);
    assert!(heap.ledger().allocations >= 5);
    assert_eq!(heap.ledger().live_allocations, 1);
    assert_eq!(heap.ledger().live_bytes, b.capacity as u64 + 32);
    assert_eq!(heap.ledger().peak_bytes, 8192 + 16384 + 64);
    heap.range_mut(&b, 0, 8199).unwrap().reverse();
    assert!(heap.bytes(&b).unwrap().iter().eq(input.iter().rev()));
    heap.release(&mut b).unwrap();
    assert_eq!(b, Descriptor::empty());
    assert_eq!(heap.ledger().live_bytes, 0);
    assert_eq!(heap.ledger().allocations, heap.ledger().releases);
}

#[test]
fn bytebuf_noop_operations_allocate_nothing_even_with_zero_budget_and_fuel() {
    let mut heap = Heap {
        limit: 0,
        ..Heap::default()
    };
    let mut b = Descriptor::empty();
    heap.fault = Some((Boundary::Allocate, 0));
    assert_eq!(heap.try_reserve(&mut b, 0, &mut 0), Ok(GrowStatus::Ok));
    assert_eq!(heap.try_extend(&mut b, &[], &mut 0), Ok(GrowStatus::Ok));
    assert_eq!(heap.try_resize(&mut b, 0, 99, &mut 0), Ok(GrowStatus::Ok));
    heap.truncate(&mut b, 10).unwrap();
    assert_eq!(heap.ledger(), Ledger::default());
    assert_eq!(heap.range(&b, 0, 0).unwrap(), &[]);
    assert_eq!(heap.range_mut(&b, 0, 0).unwrap(), &[]);
    assert_eq!(heap.fault, Some((Boundary::Allocate, 0)));
}

#[test]
fn bytebuf_all_allocation_boundaries_rollback_empty_and_existing_values() {
    for boundary in [Boundary::Allocate, Boundary::Initialize, Boundary::Commit] {
        for initial in [&[][..], &[0, 128, 255][..]] {
            let mut heap = Heap::default();
            let mut b = Descriptor::empty();
            fill(&mut heap, &mut b, initial);
            let old = snapshot(&heap, &b);
            let ledger = heap.ledger();
            heap.fault = Some((boundary, 0));
            assert_eq!(
                heap.try_resize(&mut b, 100, 71, &mut fuel()),
                Ok(GrowStatus::AllocationFailed)
            );
            assert_eq!(snapshot(&heap, &b), old);
            assert_eq!(heap.ledger().live_bytes, ledger.live_bytes);
            assert_eq!(heap.ledger().live_allocations, ledger.live_allocations);
            assert_eq!(
                heap.ledger().allocations - heap.ledger().releases,
                heap.ledger().live_allocations
            );
            assert_eq!(
                heap.try_resize(&mut b, 100, 71, &mut fuel()),
                Ok(GrowStatus::Ok)
            );
            assert_eq!(&heap.bytes(&b).unwrap()[..initial.len()], initial);
            assert!(heap.bytes(&b).unwrap()[initial.len()..]
                .iter()
                .all(|&b| b == 71));
            heap.cleanup();
            assert_eq!(heap.ledger().live_bytes, 0);
            assert_eq!(heap.ledger().allocations, heap.ledger().releases);
        }
    }
}

#[test]
fn bytebuf_spare_capacity_mutation_failure_preserves_initialized_bytes() {
    let mut heap = Heap::default();
    let mut b = Descriptor::empty();
    fill(&mut heap, &mut b, &[1, 2, 3]);
    let old = snapshot(&heap, &b);
    heap.fault = Some((Boundary::Commit, 0));
    assert_eq!(
        heap.try_push(&mut b, 4, &mut fuel()),
        Ok(GrowStatus::AllocationFailed)
    );
    assert_eq!(snapshot(&heap, &b), old);
    heap.fault = Some((Boundary::Commit, 0));
    assert_eq!(
        heap.try_resize(&mut b, 1, 9, &mut 0),
        Ok(GrowStatus::AllocationFailed)
    );
    assert_eq!(snapshot(&heap, &b), old);
}

#[test]
fn bytebuf_growth_budget_includes_old_new_and_two_headers() {
    let mut heap = Heap {
        limit: 87,
        ..Heap::default()
    };
    let mut b = Descriptor::empty();
    fill(&mut heap, &mut b, &[11; 8]); // 8 + 32
    let old = snapshot(&heap, &b);
    assert_eq!(
        heap.try_push(&mut b, 12, &mut 0),
        Ok(GrowStatus::MemoryLimit)
    );
    assert_eq!(snapshot(&heap, &b), old);
    heap.limit = 88; // (8+32) + (16+32)
    assert_eq!(heap.try_push(&mut b, 12, &mut 8), Err(Failure::Fuel));
    assert_eq!(snapshot(&heap, &b), old);
    let mut exact_fuel = 9;
    assert_eq!(
        heap.try_push(&mut b, 12, &mut exact_fuel),
        Ok(GrowStatus::Ok)
    );
    assert_eq!(exact_fuel, 0);
    assert_eq!(heap.ledger().peak_bytes, 88);
    assert_eq!(heap.ledger().live_bytes, 48);
    heap.release(&mut b).unwrap(); // no fuel needed for physical release
    assert_eq!(heap.ledger().live_bytes, 0);
}

#[test]
fn bytebuf_signed_bounds_and_capacity_arithmetic_have_defined_precedence() {
    let mut heap = Heap {
        limit: 0,
        ..Heap::default()
    };
    let mut b = Descriptor::empty();
    assert_eq!(heap.try_reserve(&mut b, -1, &mut 0), Err(Failure::Bounds));
    assert_eq!(heap.try_resize(&mut b, -1, 0, &mut 0), Err(Failure::Bounds));
    assert_eq!(heap.truncate(&mut b, -1), Err(Failure::Bounds));
    for (start, end) in [(-1, 0), (0, -1), (1, 0), (0, 1), (i32::MAX, i32::MAX)] {
        assert_eq!(heap.range(&b, start, end), Err(Failure::Bounds));
        assert_eq!(heap.range_mut(&b, start, end), Err(Failure::Bounds));
    }
    assert_eq!(growth_capacity(1 << 30, (1 << 30) + 1), Ok(i32::MAX));
    assert_eq!(growth_capacity(i32::MAX, i32::MAX), Ok(i32::MAX));
    assert_eq!(growth_capacity(-1, 0), Err(Failure::Bounds));
    heap.limit = LIMIT;
    fill(&mut heap, &mut b, &[7]);
    assert_eq!(
        heap.try_reserve(&mut b, i32::MAX, &mut 0),
        Ok(GrowStatus::CapacityOverflow)
    );
    assert_eq!(heap.bytes(&b).unwrap(), &[7]);
}

#[test]
fn bytebuf_shrink_regrow_overwrites_old_tail_and_never_exposes_capacity() {
    let mut heap = Heap::default();
    let mut b = Descriptor::empty();
    fill(&mut heap, &mut b, &[0, 128, 255, 1, 2]);
    assert_eq!(heap.range(&b, 0, b.capacity), Err(Failure::Bounds));
    heap.truncate(&mut b, 2).unwrap();
    let capacity = b.capacity;
    let ledger = heap.ledger();
    assert_eq!(heap.try_resize(&mut b, 6, 7, &mut 4), Ok(GrowStatus::Ok));
    assert_eq!(heap.bytes(&b).unwrap(), &[0, 128, 7, 7, 7, 7]);
    assert_eq!(b.capacity, capacity);
    assert_eq!(heap.ledger(), ledger);
    assert_eq!(heap.try_resize(&mut b, 1, 42, &mut 0), Ok(GrowStatus::Ok));
    assert_eq!(heap.bytes(&b).unwrap(), &[0]);
    assert_eq!(heap.try_reserve(&mut b, 0, &mut 0), Ok(GrowStatus::Ok));
    assert_eq!(heap.try_extend(&mut b, &[], &mut 0), Ok(GrowStatus::Ok));
    assert_eq!(heap.try_resize(&mut b, 1, 42, &mut 0), Ok(GrowStatus::Ok));
    assert_eq!(heap.range(&b, 1, 1).unwrap(), &[]);
}

#[test]
fn bytebuf_stale_generation_and_forged_initialized_extent_reject() {
    let mut heap = Heap::default();
    let mut b = Descriptor::empty();
    fill(&mut heap, &mut b, &[42]);
    let stale = Descriptor {
        identity: b.identity,
        length: b.length,
        capacity: b.capacity,
    };
    let forged = Descriptor {
        identity: b.identity,
        length: b.capacity,
        capacity: b.capacity,
    };
    assert_eq!(heap.bytes(&forged), Err(Failure::Descriptor));
    heap.release(&mut b).unwrap();
    fill(&mut heap, &mut b, &[42]);
    assert_ne!(b.identity, stale.identity);
    assert_eq!(heap.bytes(&stale), Err(Failure::Descriptor));
    assert_eq!(heap.bytes(&b).unwrap(), &[42]);
    heap.next_identity = u64::MAX;
    assert_eq!(
        heap.try_reserve(&mut b, 100, &mut 0),
        Err(Failure::IdentityExhausted)
    );
    assert_eq!(heap.bytes(&b).unwrap(), &[42]);
}

#[test]
fn bytebuf_fatal_cleanup_and_repeated_executions_release_every_node() {
    for _ in 0..100 {
        let mut heap = Heap::default();
        let mut first = Descriptor::empty();
        let mut second = Descriptor::empty();
        let mut third = Descriptor::empty();
        fill(&mut heap, &mut first, &[1]);
        fill(&mut heap, &mut second, &[2; 32]);
        fill(&mut heap, &mut third, &[3; 128]);
        heap.release(&mut second).unwrap(); // interior unlink
        assert_eq!(heap.bytes(&first).unwrap(), &[1]);
        assert_eq!(heap.bytes(&third).unwrap(), &[3; 128]);
        assert_eq!(
            heap.try_resize(&mut first, 100, 0, &mut 0),
            Err(Failure::Fuel)
        );
        heap.cleanup(); // fatal path has no descriptor or fuel dependency
        assert_eq!(heap.ledger().live_bytes, 0);
        assert_eq!(heap.ledger().live_allocations, 0);
        assert_eq!(heap.ledger().allocations, heap.ledger().releases);
        heap.cleanup(); // idempotent execution teardown
    }
}

#[test]
fn bytebuf_runtime_storage_is_separate_from_normalized_heap_admission() {
    assert_eq!(std::mem::size_of::<Header>(), 32);
    assert_eq!(std::mem::align_of::<Header>(), 8);
    assert_eq!(std::mem::size_of::<Descriptor>(), 16);
    assert_eq!(std::mem::size_of::<Ledger>(), 40);
    println!("bytebuf context={} header=32 descriptor=16 normalized_per_allocation=capacity+32 allocator_overhead=unmeasured",
        std::mem::size_of::<Heap>());
}
