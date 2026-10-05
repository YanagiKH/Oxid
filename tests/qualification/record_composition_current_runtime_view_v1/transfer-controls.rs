#[cfg(test)]
mod record_transfer_controls {
    use super::*;
    use crate::frontend::unit3_observer as journal;
    fn machine<'p, 'w>(plan: &'p ExecutionPlan<'w>) -> Machine<'p, 'w> {
        let frame = Frame::allocate(plan, hir::DefId(0), 1, None).unwrap();
        Machine { plan, frames: vec![frame], limits: Limits::default(), fuel: 1000,
            next_activation: 2, live_slots: 0, live_cells: 0, live_bytes: 0, header_bytes: 0,
            events: vec![], observer: array_observe::Observer::default() }
    }
    fn key(owner: u64) -> OwnerKey { OwnerKey { frame: 0, activation: 1, owner, generation: 1 } }
    #[test]
    fn record_transfer_one_success_no_duplicates_and_failed_stores_no_event() {
        let (sources, raw, _) = super::super::consumer_fixtures::owned_relay();
        let witness = verified::verify_owned(raw, &sources).unwrap();
        let plan = ExecutionPlan::build(&witness).unwrap();
        let mut m = machine(&plan); let span = witness.functions()[0].span;
        let source = m.owner_extent(key(0), span).unwrap();
        let destination = m.owner_extent(key(1), span).unwrap();
        encode(&mut m.frames[0].payload, source.start, Scalar::I32(73), span).unwrap();
        journal::record_transfer_control_start();
        m.transfer_payload(key(0), key(1), span).unwrap();
        let rows = journal::record_transfer_control_rows();
        assert_eq!(rows.len(), 2, "one payload write followed by existing transfer callback");
        assert_eq!(rows[0].0, "payload_write"); assert_eq!(rows[1].0, "existing_owned_event");
        assert!(rows[1].1.starts_with("Transfer("));
        assert!(rows[0].1.contains("Some([0, 0, 0, 0]), Some([73, 0, 0, 0])"));
        let before = m.frames[0].payload.clone();
        assert!(m.store_leaf(key(1), ScalarLeaf { offset: 0, ty: hir::Ty::I32 }, Scalar::Bool(true), span).is_err());
        assert_eq!(m.frames[0].payload, before); assert_eq!(journal::record_transfer_control_rows(), rows);
        m.frames[0].payload.truncate(destination.end - 1);
        let before = m.frames[0].payload.clone();
        assert!(m.transfer_payload(key(0), key(1), span).is_err());
        assert_eq!(m.frames[0].payload, before); assert_eq!(journal::record_transfer_control_rows(), rows);
        journal::record_transfer_control_end();
    }
    #[test]
    fn record_transfer_empty_sentinel_emits_no_field_write() {
        let (sources, mut raw, _) = super::super::consumer_fixtures::empty_record();
        let second = raw.functions[0].owners[0].clone(); raw.functions[0].owners.push(second);
        let span = raw.functions[0].span;
        raw.functions[0].blocks[0].statements.insert(0, super::super::consumer_fixtures::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(1)), span));
        raw.functions[0].blocks[0].statements.insert(1, super::super::consumer_fixtures::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(1)), span));
        let witness = verified::verify_owned(raw, &sources).unwrap();
        let plan = ExecutionPlan::build(&witness).unwrap();
        let mut m = machine(&plan); let span = witness.functions()[0].span;
        journal::record_transfer_control_start(); m.transfer_payload(key(0), key(1), span).unwrap();
        let rows = journal::record_transfer_control_rows();
        assert_eq!(rows.len(), 1); assert_eq!(rows[0].0, "existing_owned_event");
        assert!(rows[0].1.starts_with("Transfer(")); journal::record_transfer_control_end();
    }
    #[test]
    fn record_transfer_mixed_fields_keep_order_width_and_padding() {
        let (sources, raw, _) = super::super::consumer_fixtures::mixed_relay(2);
        let witness = verified::verify_owned(raw, &sources).unwrap();
        let plan = ExecutionPlan::build(&witness).unwrap();
        let mut m = machine(&plan); let span = witness.functions()[0].span;
        let source = m.owner_extent(key(0), span).unwrap();
        let destination = m.owner_extent(key(1), span).unwrap();
        m.frames[0].payload[destination.clone()].fill(0xa5);
        encode(&mut m.frames[0].payload, source.start, Scalar::Bool(true), span).unwrap();
        encode(&mut m.frames[0].payload, source.start + 1, Scalar::Unit, span).unwrap();
        encode(&mut m.frames[0].payload, source.start + 4, Scalar::I32(-99), span).unwrap();
        journal::record_transfer_control_start(); m.transfer_payload(key(0), key(1), span).unwrap();
        let rows = journal::record_transfer_control_rows(); assert_eq!(rows.len(), 4);
        for (index, row) in rows[..3].iter().enumerate() {
            assert_eq!(row.0, "payload_write"); assert!(row.1.contains(&format!("index: {index}")));
        }
        assert!(rows[0].1.contains("Some([165]), Some([1])"));
        assert!(rows[1].1.contains("Some([165]), Some([0])"));
        assert!(rows[2].1.contains("Some([165, 165, 165, 165]), Some([157, 255, 255, 255])"));
        assert_eq!(rows[3].0, "existing_owned_event"); assert!(rows[3].1.starts_with("Transfer("));
        assert_eq!(&m.frames[0].payload[destination.start + 2..destination.start + 4], &[0xa5, 0xa5]);
        journal::record_transfer_control_end();
    }

}
