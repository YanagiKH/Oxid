//! Reviewer-owned controls; only copied review checkout is modified.
use super::*;
use crate::frontend::oir::owned::source::resource_fixtures;
use crate::frontend::source::SourceFileId;

#[test]
fn reviewer_source_resource_reserved_headers_charge_capacity_not_peak() {
    let text = "struct T{value:i32} fn relay(x:T)->T{return x;} fn read(p:&T)->i32{return p.value;} fn main()->i32{let x=relay(T{value:7});return read(&x);}";
    let case = resource_fixtures::checked(text);
    let file_id = SourceFileId(0);
    let entry = case.entry;
    let witness = case.witness;
    let sources = case.sources;
    // Hand census with physical-view R80/L112: main S2 A2 O4 P4 B16 L1
    // C2 => D320; relay O2 P2 B8 => D72; read S1 R1 => D88. The read
    // path peaks at408 dynamic bytes and2 frames; reserve3 headers anyway.
    // This expectation does not inspect a plan.
    let exact = 3 * size_of::<Frame>() + size_of::<Scalar>() + 320 + 88;
    assert_eq!(size_of::<Frame>(), 272);
    assert_eq!(size_of::<Scalar>(), 8);
    assert_eq!(exact, 1232);
    assert_eq!(
        run_limits(
            &witness,
            Some(entry),
            Limits {
                frames: 3,
                bytes: exact,
                ..Limits::default()
            }
        )
        .unwrap(),
        Scalar::I32(7)
    );
    let failure = run_limits(
        &witness,
        Some(entry),
        Limits {
            frames: 3,
            bytes: exact - 1,
            ..Limits::default()
        },
    )
    .unwrap_err();
    let start = text.find("read(&x)").unwrap();
    let expected = Span {
        file: file_id,
        start,
        end: start + "read(&x)".len(),
    };
    assert_eq!(
        failure,
        OwnedRunFailure::Resource(plan::AdmissionFailure::new(
            "live requested bytes",
            Some(expected)
        ))
    );
    assert_eq!(failure.diagnostic(&sources).code, "E0605");
}
