use super::*;
// Append only inside cfg(test) array_observe module in the isolated observer.
// These are instrumentation-ledger unit inputs, not language executions.
#[test]
fn independent_unit2c_payload_ledger_preflights_before_poison() {
    for (retained, success) in [
        (MAX_PAYLOAD_BYTES - 4, true),
        (MAX_PAYLOAD_BYTES - 3, false),
        (usize::MAX, false),
    ] {
        let mut observer = Observer {
            enabled: true,
            payload_bytes: retained,
            control: ObservationControl {
                poison_destinations: true,
                ..ObservationControl::default()
            },
            ..Observer::default()
        };
        let mut payload = [0x71u8; 4];
        let before = observer.begin(
            &mut payload,
            0..4,
            OwnerKey::default(),
            UNINITIALIZED,
            StorageObservationKind::Construction,
            true,
        );
        if success {
            assert!(before.is_some());
            assert_eq!(payload, [0xa5; 4]);
            payload.fill(0);
            observer.end(&payload, before);
            assert_eq!(observer.payload_bytes, MAX_PAYLOAD_BYTES);
            assert_eq!(observer.storage.len(), 1);
            assert!(!observer.truncated);
        } else {
            assert!(before.is_none());
            assert_eq!(payload, [0x71; 4]);
            assert!(observer.storage.is_empty());
            assert!(observer.truncated);
        }
        assert!(!observer.allocation_fault_applied);
    }
    eprintln!("independent observer synthetic payload-ledger inputs=3");
}
