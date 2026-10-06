//! Closed-origin precursor: no builtin descriptor grants execution authority.
use super::*;
use std::mem::{align_of, size_of};

fn empty(builtins: BuiltinOrigins) -> RawOwnedProgram {
    RawOwnedProgram {
        builtins,
        enums: vec![],
        records: vec![],
        functions: vec![],
    }
}

#[test]
fn builtin_origins_are_denied_before_declaration_or_proof_allocation() {
    let sources = SourceMap::new();
    for origin in [BuiltinOrigins::ReadStatus, BuiltinOrigins::ReadStdin] {
        budget::fail_allocation_after(0, || {
            let error = verify_owned(empty(origin), &sources).unwrap_err();
            assert_eq!(error.kind, OwnedFailureKind::Malformed(Malformed::Binding));
            assert_eq!(error.primary.get(), None);
            assert_eq!(
                verified::probe_enum_validation(&empty(origin), &sources, budget::Limits::DEFAULT)
                    .unwrap_err()
                    .kind,
                error.kind
            );
            assert_eq!(
                verified::probe_array_validation(&empty(origin), &sources, budget::Limits::DEFAULT)
                    .unwrap_err()
                    .kind,
                error.kind
            );
        });
    }
}

#[test]
fn builtin_origin_header_padding_is_charged_at_metadata_endpoint() {
    let raw = empty(BuiltinOrigins::None);
    let usage = budget::preflight(&raw, budget::Limits::DEFAULT).unwrap();
    // Historical empty inventory includes its enum Vec header (24 bytes).
    // Add the complete padded raw-header growth, not the one-byte discriminant.
    let expected =
        size_of::<Vec<RawEnumDecl>>() + size_of::<RawOwnedProgram>() - 3 * size_of::<Vec<()>>();
    assert_eq!(usage.metadata_bytes, expected);
    let limits = budget::Limits {
        metadata: expected,
        ..budget::Limits::DEFAULT
    };
    assert!(budget::preflight(&raw, limits).is_ok());
    assert_eq!(
        budget::preflight(
            &raw,
            budget::Limits {
                metadata: expected - 1,
                ..limits
            }
        )
        .unwrap_err()
        .kind,
        OwnedFailureKind::Resource("ownership metadata")
    );
}

#[test]
fn builtin_origin_enclosing_layout_measurements() {
    macro_rules! report {
        ($($ty:ty),+ $(,)?) => {$(
            println!("builtin-layout {} bytes={} align={}", stringify!($ty), size_of::<$ty>(), align_of::<$ty>());
        )+};
    }
    report!(
        BuiltinOrigins,
        RawOwnedProgram,
        Result<RawOwnedProgram, OwnedFailure>,
        verified::VerifiedOwnedProgram,
        Result<verified::VerifiedOwnedProgram, OwnedFailure>,
        RawOwnedFunction,
        RawEnumDecl,
        OwnedInstruction,
        OwnedStatement,
        plan::FrameUsage,
    );
}
