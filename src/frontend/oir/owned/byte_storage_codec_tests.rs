//! RFC0031 byte codecs are full-octet transports, independently of admission.
use super::*;

#[test]
fn byte_storage_codecs_round_trip_every_octet_without_touching_neighbors() {
    let (_, at) = super::super::consumer_fixtures::context();
    let span = at(0);
    assert_eq!(scalar_size(hir::Ty::U8), 1);
    for value in 0..=u8::MAX {
        for offset in 1..=4 {
            let mut bytes = [0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc];
            let before = bytes;
            encode(&mut bytes, offset, Scalar::U8(value), span).unwrap();
            assert_eq!(
                decode(&bytes, offset, hir::Ty::U8, span),
                Ok(Scalar::U8(value))
            );
            assert_eq!(bytes[offset], value);
            assert_eq!(&bytes[..offset], &before[..offset]);
            assert_eq!(&bytes[offset + 1..], &before[offset + 1..]);
        }
        assert_eq!(
            decode(&[value], 0, hir::Ty::U8, span),
            Ok(Scalar::U8(value))
        );
        assert_eq!(decode(&[value], 0, hir::Ty::Bool, span).is_ok(), value < 2);
        assert_eq!(decode(&[value], 0, hir::Ty::Unit, span).is_ok(), value == 0);
    }
}

#[test]
fn byte_storage_codecs_check_offset_and_extent_before_writing() {
    let (_, at) = super::super::consumer_fixtures::context();
    let span = at(0);
    for offset in [2, usize::MAX] {
        let mut bytes = [0x80, 0xff];
        assert_eq!(
            encode(&mut bytes, offset, Scalar::U8(7), span),
            Err(bad("payload range", span))
        );
        assert_eq!(
            decode(&bytes, offset, hir::Ty::U8, span),
            Err(bad("payload range", span))
        );
        assert_eq!(bytes, [0x80, 0xff]);
    }
    assert_eq!(scalar_offset(7..8, 0, hir::Ty::U8, span), Ok(7));
    assert_eq!(
        scalar_offset(7..8, 1, hir::Ty::U8, span),
        Err(bad("leaf payload range", span))
    );
    assert_eq!(
        scalar_offset(usize::MAX..usize::MAX, 1, hir::Ty::U8, span),
        Err(bad("leaf offset overflow", span))
    );
}

#[test]
fn byte_storage_views_rederive_whole_owner_identity_before_offsets() {
    let text = "struct R{b:[bool;1]}fn len(p:&[u8])->i32{return p.len();}fn main()->i32{let x=255;let b=x.to_u8_checked();let a=[b];let same=[b];let zero:[u8;0]=[];let control=[true];let r=R{b:[true]};return len(&a);}";
    source::byte_storage_tests::with_raw(text, |_, typed, raw| {
        let witness =
            verified::verify_associated(source::association::associate(raw, typed).unwrap())
                .unwrap();
        let plan = ExecutionPlan::build(&witness).unwrap();
        let entry = typed.entry().unwrap();
        let f = &witness.functions()[entry.0];
        let span = f.span;
        let mut machine = Machine {
            plan: &plan,
            frames: Vec::new(),
            limits: Limits::default(),
            fuel: plan::MAX_FUEL,
            next_activation: 1,
            live_slots: 0,
            live_cells: 0,
            live_bytes: 0,
            header_bytes: 0,
            events: Vec::new(),
            observer: array_observe::Observer::default(),
        };
        machine.install(Frame::allocate(&plan, entry, 1, None).unwrap());
        // Trusted source contains exactly one call, after every owner is ready.
        for statement in &f.blocks[0].statements {
            machine
                .statement(0, &statement.kind, statement.span)
                .unwrap();
        }
        let handle = machine.handle(0, LoanId(0), span).unwrap();
        assert!(machine.validate_handle(handle, Access::Read, span).is_ok());
        assert!(machine.checked_handle_view(handle, span).is_ok());
        for offset in [1, u64::MAX] {
            let mut forged = handle;
            forged.view.offset = offset;
            assert_eq!(
                machine.validate_handle(forged, Access::Read, span),
                Err(bad("loan view mismatch", span))
            );
            assert_eq!(
                machine.checked_handle_view(forged, span),
                Err(bad("view root type", span))
            );
        }
        for element in [hir::Ty::Bool, hir::Ty::Unit, hir::Ty::U8] {
            for length in [0, 1, 2] {
                if element == hir::Ty::U8 && length == 1 {
                    continue;
                }
                let mut forged = handle;
                forged.view.aggregate = Some(
                    AggregateSlot::try_from_aggregate(AggregateTy::FixedArray(
                        FixedArrayTy::check(element, length).unwrap(),
                    ))
                    .unwrap(),
                );
                assert_eq!(
                    machine.checked_handle_view(forged, span),
                    Err(bad("view root type", span))
                );
            }
        }
        for (mutation, expected) in [
            (0, "stale owner activation"),
            (1, "stale owner generation"),
            (2, "stale loan activation"),
            (3, "stale loan instance"),
        ] {
            let mut forged = handle;
            match mutation {
                0 => forged.root.activation += 1,
                1 => forged.root.generation += 1,
                2 => forged.permission.activation += 1,
                _ => forged.permission.instance += 1,
            }
            assert_eq!(
                machine.validate_handle(forged, Access::Read, span),
                Err(bad(expected, span))
            );
        }
        let root = f
            .owners
            .iter()
            .enumerate()
            .find(|(_, o)| {
                matches!(o.aggregate(), AggregateTy::Record(_))
                    && matches!(o.kind, OwnerKind::Local { .. })
            })
            .map(|(i, _)| machine.owner_key(0, OwnerPlaceId(i), span).unwrap())
            .unwrap();
        let mut projected = handle;
        projected.root = root;
        // A byte descriptor cannot manufacture a record projection, even when
        // the requested offset and physical byte size happen to fit.
        assert!(machine.checked_handle_view(projected, span).is_err());
    });
}

#[test]
fn byte_storage_resource_width_and_payload_are_exact_before_frame_allocation() {
    for length in [0usize, 1, 1024] {
        // Only the nonempty version needs scalar expressions: literal128,
        // immutable binding, receiver snapshot, conversion, immutable binding,
        // N copied elements and final len. Thus S=N+6, versus S=1 at N=0.
        let elements = (0..length).map(|_| "b").collect::<Vec<_>>().join(",");
        let prefix = if length == 0 {
            ""
        } else {
            "let x=128;let b=x.to_u8_checked();"
        };
        let text =
            format!("fn main()->i32{{{prefix}let a:[u8;{length}]=[{elements}];return a.len();}}");
        source::byte_storage_tests::with_raw(&text, |_, typed, raw| {
            let witness =
                verified::verify_associated(source::association::associate(raw, typed).unwrap())
                    .unwrap();
            let entry = typed.entry().unwrap();
            let p = ExecutionPlan::build(&witness).unwrap();
            let width = length.max(1);
            let scalars = if length == 0 { 1 } else { length + 6 };
            let cells = scalars + 2 * width + 8;
            let payload = 2 * width;
            let physical =
                scalars * size_of::<Option<Scalar>>() + payload + 2 * size_of::<OwnerRuntime>();
            let usage = p.function(entry).usage();
            assert_eq!(
                (
                    usage.scalar_slots,
                    usage.owner_cells,
                    usage.payload_bytes,
                    usage.expanded_cells,
                    usage.reference_bytes
                ),
                (scalars, 2 * width, payload, cells, physical)
            );
            let bytes = size_of::<Frame>() + size_of::<Scalar>() + physical;
            let limits = Limits {
                frames: 1,
                cells,
                bytes,
                ..Default::default()
            };
            assert_eq!(
                run_limits(&witness, Some(entry), limits),
                Ok(Scalar::I32(length as i32))
            );
            for (short, name) in [
                (
                    Limits {
                        cells: cells - 1,
                        ..limits
                    },
                    "live expanded cells",
                ),
                (
                    Limits {
                        bytes: bytes - 1,
                        ..limits
                    },
                    "live requested bytes",
                ),
            ] {
                // Build the plan outside the injected scope. No allocation can
                // be attempted before the failed activation preflight.
                let result =
                    plan::fail_allocation_after(0, || execute_plan(&p, entry, short, None));
                assert_eq!(
                    result,
                    Err(OwnedRunFailure::Resource(plan::AdmissionFailure::new(
                        name,
                        Some(witness.functions()[entry.0].span)
                    )))
                );
            }
            // Independent allocation topology: one frame-header reserve plus
            // Frame::allocate's seven filled arrays (slots, snapshots, payload,
            // owners, references, loans, calls). reserve invokes its test point
            // even for an empty vector. No instruction in this single-frame
            // literal/length fixture allocates after activation.
            const ALLOCATION_SITES: usize = 1 + 7;
            for fail in 0..ALLOCATION_SITES {
                let allocation =
                    plan::fail_allocation_after(fail, || execute_plan(&p, entry, limits, None));
                assert!(
                    matches!(
                        allocation,
                        Err(OwnedRunFailure::Resource(plan::AdmissionFailure {
                            name: "injected owned allocation failure",
                            ..
                        }))
                    ),
                    "length={length}, allocation={fail}: {allocation:?}"
                );
            }
            assert_eq!(
                plan::fail_allocation_after(ALLOCATION_SITES, || {
                    execute_plan(&p, entry, limits, None)
                }),
                Ok(Scalar::I32(length as i32))
            );
        });
    }
}

#[test]
fn byte_storage_dynamic_reborrows_preserve_root_view_and_parent_permissions() {
    for (parent, child) in [
        (BorrowKind::Shared, BorrowKind::Shared),
        (BorrowKind::Exclusive, BorrowKind::Shared),
        (BorrowKind::Exclusive, BorrowKind::Exclusive),
    ] {
        let parent_mut = if parent == BorrowKind::Exclusive {
            "mut "
        } else {
            ""
        };
        let child_mut = if child == BorrowKind::Exclusive {
            "mut "
        } else {
            ""
        };
        let text = format!("fn child(p:&{child_mut}[u8])->i32{{return p.len();}}fn relay(p:&{parent_mut}[u8])->i32{{return child(&{child_mut}*p);}}fn main()->i32{{let x=128;let b=x.to_u8_checked();let mut a=[b];let other=[b];return relay(&{parent_mut}a);}}");
        source::byte_storage_tests::with_raw(&text, |_, typed, raw| {
            let witness =
                verified::verify_associated(source::association::associate(raw, typed).unwrap())
                    .unwrap();
            let p = ExecutionPlan::build(&witness).unwrap();
            let entry = typed.entry().unwrap();
            let f = &witness.functions()[entry.0];
            let span = f.span;
            let mut machine = Machine {
                plan: &p,
                frames: Vec::new(),
                limits: Limits::default(),
                fuel: plan::MAX_FUEL,
                next_activation: 1,
                live_slots: 0,
                live_cells: 0,
                live_bytes: 0,
                header_bytes: 0,
                events: Vec::new(),
                observer: array_observe::Observer::default(),
            };
            machine.install(Frame::allocate(&p, entry, 1, None).unwrap());
            for statement in &f.blocks[0].statements {
                machine
                    .statement(0, &statement.kind, plan::instruction_span(statement))
                    .unwrap();
            }
            let end = f.blocks[0].terminator.as_ref().unwrap();
            let OwnedTerminatorKind::Invoke { call, continuation } = end.kind else {
                unreachable!()
            };
            machine.dispatch(0, call, continuation, end.span).unwrap();
            assert_eq!(machine.frames.len(), 2);
            let relay = &witness.functions()[1];
            for statement in &relay.blocks[0].statements {
                machine
                    .statement(1, &statement.kind, plan::instruction_span(statement))
                    .unwrap();
            }
            let parent_handle = machine.frames[1].references[0];
            let child_handle = machine.handle(1, LoanId(0), span).unwrap();
            assert_eq!(child_handle.root, parent_handle.root);
            assert_eq!(child_handle.view, parent_handle.view);
            assert_eq!(
                machine.loan(child_handle.permission, span).unwrap().parent,
                parent_handle.permission
            );
            assert!(machine.checked_handle_view(child_handle, span).is_ok());
            assert_eq!(
                machine.release_preflight(parent_handle.permission, span),
                Err(bad("release with active child", span))
            );
            assert_eq!(
                machine
                    .validate_handle(parent_handle, Access::Read, span)
                    .is_ok(),
                child == BorrowKind::Shared
            );
            assert_eq!(
                machine.validate_handle(parent_handle, Access::Write, span),
                Err(bad("suspended parent permission", span))
            );
            assert_eq!(
                machine
                    .validate_handle(child_handle, Access::Write, span)
                    .is_ok(),
                child == BorrowKind::Exclusive
            );

            // A same-type, same-length neighboring owner is still another root.
            let other = f
                .owners
                .iter()
                .enumerate()
                .filter(|(_, o)| matches!(o.kind, OwnerKind::Local { .. }))
                .map(|(i, _)| machine.owner_key(0, OwnerPlaceId(i), span).unwrap())
                .find(|root| *root != parent_handle.root)
                .unwrap();
            let mut forged = child_handle;
            forged.root = other;
            assert_eq!(
                machine.validate_handle(forged, Access::Read, span),
                Err(bad("loan root mismatch", span))
            );
            let mut forged = child_handle;
            forged.view.offset = 1;
            assert_eq!(
                machine.validate_handle(forged, Access::Read, span),
                Err(bad("loan view mismatch", span))
            );
            let saved = machine.frames[1].loans[0].view;
            machine.frames[1].loans[0].view.offset = 1;
            assert_eq!(
                machine.validate_handle(child_handle, Access::Read, span),
                Err(bad("loan view mismatch", span))
            );
            machine.frames[1].loans[0].view = saved;

            machine.release(child_handle.permission, span).unwrap();
            assert_eq!(
                machine.validate_handle(child_handle, Access::Read, span),
                Err(bad("stale loan instance", span))
            );
            assert!(machine
                .validate_handle(parent_handle, Access::Read, span)
                .is_ok());
            if parent == BorrowKind::Exclusive {
                assert!(machine
                    .validate_handle(parent_handle, Access::Write, span)
                    .is_ok());
                assert!(machine
                    .validate_handle(parent_handle, Access::Borrow(BorrowKind::Exclusive), span)
                    .is_ok());
            } else {
                assert_eq!(
                    machine.validate_handle(parent_handle, Access::Write, span),
                    Err(bad("shared permission upgrade", span))
                );
                assert_eq!(
                    machine.validate_handle(
                        parent_handle,
                        Access::Borrow(BorrowKind::Exclusive),
                        span
                    ),
                    Err(bad("shared permission upgrade", span))
                );
            }
            machine.release(parent_handle.permission, span).unwrap();
            assert!(machine
                .base(
                    0,
                    AccessBase::Owner(OwnerPlaceId(parent_handle.root.owner as usize)),
                    Access::Write,
                    span
                )
                .is_ok());
        });
    }
}

#[test]
fn byte_storage_reference_frame_ceiling_is_exact_for_byte_slice_reborrows() {
    // main occupies one frame. down(1022) through down(0) occupies 1023 more;
    // tail-call elimination is not part of the reference contract. Every call
    // explicitly reborrows the complete byte owner, including its empty case.
    for length in [0usize, 1, 1024] {
        let prefix = if length == 0 {
            ""
        } else {
            "let x=128;let b=x.to_u8_checked();"
        };
        let elements = (0..length).map(|_| "b").collect::<Vec<_>>().join(",");
        let text = format!("fn down(a:&[u8],n:i32)->i32{{if n==0{{return a.len();}}return down(&*a,n-1);}}fn main()->i32{{{prefix}let a:[u8;{length}]=[{elements}];return down(&a,1022);}}");
        source::byte_storage_tests::with_raw(&text, |_, typed, raw| {
            let witness =
                verified::verify_associated(source::association::associate(raw, typed).unwrap())
                    .unwrap();
            let entry = typed.entry().unwrap();
            let p = ExecutionPlan::build(&witness).unwrap();
            let exact = Limits {
                frames: 1024,
                ..Default::default()
            };
            assert_eq!(
                execute_plan(&p, entry, exact, None),
                Ok(Scalar::I32(length as i32))
            );
            let short = Limits {
                frames: 1023,
                ..exact
            };
            assert!(matches!(
                execute_plan(&p, entry, short, None),
                Err(OwnedRunFailure::Scalar(RunFailure::Frames(_)))
            ));
            // Same fixed seven arrays for each activation, plus one header
            // reserve. Exercise the final activation site and the exact full
            // topology count; the small-frame control sweeps each distinct site.
            const SITES: usize = 1 + 7 * 1024;
            assert!(matches!(
                plan::fail_allocation_after(SITES - 1, || execute_plan(&p, entry, exact, None)),
                Err(OwnedRunFailure::Resource(plan::AdmissionFailure {
                    name: "injected owned allocation failure",
                    ..
                }))
            ));
            assert_eq!(
                plan::fail_allocation_after(SITES, || execute_plan(&p, entry, exact, None)),
                Ok(Scalar::I32(length as i32))
            );
        });
    }
}

#[test]
fn byte_storage_maximum_owned_and_borrowed_arguments_use_full_scratch() {
    for borrowed in [false, true] {
        let params = (0..256)
            .map(|i| format!("p{i}:{}", if borrowed { "&[u8]" } else { "[u8;0]" }))
            .collect::<Vec<_>>()
            .join(",");
        let (bindings, args) = if borrowed {
            (
                "let a:[u8;0]=[];".into(),
                (0..256).map(|_| "&a").collect::<Vec<_>>().join(","),
            )
        } else {
            (
                (0..256)
                    .map(|i| format!("let a{i}:[u8;0]=[];"))
                    .collect::<String>(),
                (0..256)
                    .map(|i| format!("a{i}"))
                    .collect::<Vec<_>>()
                    .join(","),
            )
        };
        let text = format!("fn many({params})->i32{{return p255.len();}}fn main()->i32{{{bindings}return many({args});}}");
        source::byte_storage_tests::with_raw(&text, |_, typed, raw| {
            let witness =
                verified::verify_associated(source::association::associate(raw, typed).unwrap())
                    .unwrap();
            let entry = typed.entry().unwrap();
            let p = ExecutionPlan::build(&witness).unwrap();
            // Arguments occupy scalar-width staging positions even when the
            // value itself is an owner/view. Byte payload size cannot discount
            // the full 256-position transport scratch.
            assert_eq!(p.function(entry).usage().arguments, 256);
            // Source census: main has one call-result scalar and 256 staging
            // slots. Shared-borrow form has two owners and256 loans; owned form
            // has four owners per argument (literal,local,name-move,staged).
            // many has one len scalar and256 references or parameter owners.
            let (root_owners, child_owners, loans, references) = if borrowed {
                (2usize, 0usize, 256usize, 256usize)
            } else {
                (4 * 256, 256, 0, 0)
            };
            let root_cells = 1 + 256 + 5 * root_owners + 14 * loans + 2;
            let child_cells = 1 + 5 * child_owners + 10 * references;
            let root_bytes = 257 * size_of::<Option<Scalar>>()
                + root_owners
                + root_owners * size_of::<OwnerRuntime>()
                + loans * size_of::<LoanRuntime>()
                + size_of::<CallRuntime>();
            let child_bytes = size_of::<Option<Scalar>>()
                + child_owners
                + child_owners * size_of::<OwnerRuntime>()
                + references * size_of::<ReferenceHandle>();
            assert_eq!(p.function(entry).usage().expanded_cells, root_cells);
            assert_eq!(p.function(entry).usage().reference_bytes, root_bytes);
            let limits = Limits {
                frames: 2,
                cells: root_cells + child_cells,
                bytes: 2 * size_of::<Frame>() + size_of::<Scalar>() + root_bytes + child_bytes,
                ..Default::default()
            };
            for (short, name) in [
                (
                    Limits {
                        cells: limits.cells - 1,
                        ..limits
                    },
                    "live expanded cells",
                ),
                (
                    Limits {
                        bytes: limits.bytes - 1,
                        ..limits
                    },
                    "live requested bytes",
                ),
            ] {
                // Root uses8 allocation points; child admission must refuse
                // before its first vector allocation at point8.
                let error = plan::fail_allocation_after(8, || execute_plan(&p, entry, short, None));
                assert!(
                    matches!(error, Err(OwnedRunFailure::Resource(plan::AdmissionFailure { name: actual, .. })) if actual == name),
                    "{error:?}"
                );
            }
            // main plus one callee: one header + two seven-vector frames.
            const SITES: usize = 1 + 2 * 7;
            for fail in 0..SITES {
                let result =
                    plan::fail_allocation_after(fail, || execute_plan(&p, entry, limits, None));
                assert!(
                    matches!(
                        result,
                        Err(OwnedRunFailure::Resource(plan::AdmissionFailure {
                            name: "injected owned allocation failure",
                            ..
                        }))
                    ),
                    "borrowed={borrowed}, site={fail}: {result:?}"
                );
            }
            assert_eq!(
                plan::fail_allocation_after(SITES, || execute_plan(&p, entry, limits, None)),
                Ok(Scalar::I32(0))
            );
        });
    }
}
