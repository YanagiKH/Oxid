//! Mathematical byte oracle, independently enumerated through typed invocation.
use super::*;
use crate::frontend::{lexer, parser};

fn compiled(text: &str) -> (SourceMap, VerifiedProgram) {
    let mut sources = SourceMap::new();
    let file = sources.add("byte-oracle.ox".into(), text.into());
    let source = sources.get(file);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let typed = typeck::check(hir::resolve(source, &ast).unwrap()).unwrap();
    let raw = lower::lower(&typed).unwrap();
    let associated = super::super::source::association::authenticate_scalar(
        raw,
        &sources,
        super::super::source::association::Declarations::Original(&ast),
    )
    .unwrap();
    let verified = verify::verify_associated(associated).unwrap();
    (sources, verified)
}

fn bounded() -> Limits {
    Limits {
        fuel: 64,
        frames: 1,
        slots: 16,
    }
}

#[test]
fn every_byte_narrows_and_widens_to_the_independent_mathematical_value() {
    let (_, narrow) = compiled("fn narrow(x:i32)->u8{return x.to_u8_checked();}");
    let (_, widen) = compiled("fn widen(x:u8)->i32{return x.to_i32();}");
    let mut seen = [false; 256];
    for value in 0i32..256 {
        let byte = u8::try_from(value).unwrap();
        assert!(!std::mem::replace(&mut seen[value as usize], true));
        assert_eq!(
            invoke(&narrow, hir::DefId(0), &[Scalar::I32(value)], bounded()),
            Ok(Scalar::U8(byte))
        );
        assert_eq!(
            invoke(&widen, hir::DefId(0), &[Scalar::U8(byte)], bounded()),
            Ok(Scalar::I32(value))
        );
    }
    assert!(seen.into_iter().all(|v| v));
}

#[test]
fn all_393216_byte_comparisons_match_independent_integer_oracle() {
    let operators = ["==", "!=", "<", "<=", ">", ">="];
    let source = operators
        .iter()
        .enumerate()
        .map(|(id, op)| format!("fn compare{id}(a:u8,b:u8)->bool{{return a {op} b;}}"))
        .collect::<Vec<_>>()
        .join(" ");
    let (_, program) = compiled(&source);
    let mut seen = vec![false; 256 * 256 * 6];
    let mut total = 0usize;
    for left in 0i32..256 {
        for right in 0i32..256 {
            let expected = [
                left == right,
                left != right,
                left < right,
                left <= right,
                left > right,
                left >= right,
            ];
            for (operator, value) in expected.into_iter().enumerate() {
                let key = ((left as usize) * 256 + right as usize) * 6 + operator;
                assert!(!std::mem::replace(&mut seen[key], true));
                total += 1;
                assert_eq!(
                    invoke(
                        &program,
                        hir::DefId(operator),
                        &[Scalar::U8(left as u8), Scalar::U8(right as u8)],
                        bounded()
                    ),
                    Ok(Scalar::Bool(value)),
                    "{left} {} {right}",
                    operators[operator]
                );
            }
        }
    }
    assert_eq!(total, 393_216);
    assert!(seen.into_iter().all(|value| value));
}

#[test]
fn narrowing_failure_is_paid_and_uses_only_intrinsic_origin() {
    let (sources, program) = compiled("fn narrow(x:i32)->u8{return x.to_u8_checked();}");
    let function = &program.program.functions[0];
    assert_eq!(function.locals.len(), 3); // parameter, read snapshot, conversion
    let read = function.blocks[0].statements[0].assignment();
    let conversion = function.blocks[0].statements[1].assignment();
    let Rvalue::CheckedI32ToU8 {
        operand, name_span, ..
    } = conversion.value
    else {
        panic!("conversion")
    };
    assert_eq!(operand.local, read.destination);
    assert!(matches!(read.value, Rvalue::Copy(_)));
    for value in [-1, 256, i32::MIN, i32::MAX] {
        // Entry + three cells = 4; snapshot = 1; conversion = 1.
        for (fuel, expected) in [
            (4, RunFailure::Fuel(read.span)),
            (5, RunFailure::Fuel(conversion.span)),
            (6, RunFailure::ByteRange(name_span)),
        ] {
            assert_eq!(
                invoke(
                    &program,
                    hir::DefId(0),
                    &[Scalar::I32(value)],
                    Limits { fuel, ..bounded() }
                ),
                Err(expected)
            );
        }
        let error = invoke(&program, hir::DefId(0), &[Scalar::I32(value)], bounded())
            .unwrap_err()
            .diagnostic(&sources);
        assert_eq!(
            (error.code, error.stage, error.message.as_str()),
            (
                "E0610",
                "oir-run",
                "checked i32 to u8 conversion out of range"
            )
        );
        assert_eq!(error.primary, Some(name_span));
    }
}

#[test]
fn unpaid_conversion_never_reads_its_operand_or_initializes_destination() {
    let (_, mut program) = compiled("fn narrow(x:i32)->u8{return x.to_u8_checked();}");
    let conversion = program.program.functions[0].blocks[0].statements[1].assignment_mut();
    let full = conversion.span;
    let Rvalue::CheckedI32ToU8 { operand, .. } = &mut conversion.value else {
        panic!("conversion")
    };
    operand.local = LocalId(usize::MAX);
    // Only this module-private adversary mutates the witness. It proves runtime
    // ordering, not a public construction or execution path for unchecked OIR.
    assert_eq!(
        invoke(
            &program,
            hir::DefId(0),
            &[Scalar::I32(256)],
            Limits {
                fuel: 5,
                ..bounded()
            }
        ),
        Err(RunFailure::Fuel(full))
    );
    assert!(matches!(
        invoke(
            &program,
            hir::DefId(0),
            &[Scalar::I32(256)],
            Limits {
                fuel: 6,
                ..bounded()
            }
        ),
        Err(RunFailure::Internal(_))
    ));
}
