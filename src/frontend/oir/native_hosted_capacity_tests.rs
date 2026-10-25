use super::tests::verified_with_sources;
use super::*;

fn text(functions: usize, padding: usize) -> String {
    let mut source = String::new();
    for index in 0..functions {
        let name = if index == 0 {
            "main".to_owned()
        } else {
            format!("f{index}")
        };
        source.push_str(&format!(
            "fn {name}()->i32{{{}return 7;}}",
            "0;".repeat(padding)
        ));
    }
    source
}

#[test]
fn scalar_hosted_function_limit_is_1024_and_bounded_is_256() {
    for hosted in [false, true] {
        let maximum = if hosted {
            MAX_HOSTED_FUNCTIONS
        } else {
            MAX_FUNCTIONS
        };
        let (program, _) = verified_with_sources(&text(maximum, 0));
        program.admit_with_execution(hosted).unwrap();
        let (program, _) = verified_with_sources(&text(maximum + 1, 0));
        let error = program.admit_with_execution(hosted).unwrap_err();
        assert_eq!(error.code, "E0700");
        assert_eq!(
            error.message,
            format!("native preview function count limit exceeded ({maximum})")
        );
    }
}

#[test]
fn scalar_hosted_32768_locals_are_inclusive_and_function_slots_remain_256() {
    for hosted in [false, true] {
        let maximum = if hosted {
            MAX_HOSTED_LOCALS
        } else {
            MAX_NATIVE_LOCALS
        };
        let count = maximum / MAX_FUNCTION_LOCALS;
        let source = text(count, MAX_FUNCTION_LOCALS - 1);
        let (program, _) = verified_with_sources(&source);
        assert_eq!(
            program
                .program
                .functions
                .iter()
                .map(Function::slot_count)
                .sum::<usize>(),
            maximum
        );
        program.admit_with_execution(hosted).unwrap();
        let (program, _) = verified_with_sources(&(source + "fn extra()->i32{return 0;}"));
        let error = program.admit_with_execution(hosted).unwrap_err();
        assert!(error
            .message
            .contains(&format!("aggregate locals limit exceeded ({maximum})")));
        let (program, _) = verified_with_sources(&text(1, MAX_FUNCTION_LOCALS));
        let error = program.admit_with_execution(hosted).unwrap_err();
        assert!(error
            .message
            .contains("locals per function limit exceeded (256)"));
    }
}

#[test]
fn scalar_hosted_block_limit_is_16384_and_bounded_is_4096() {
    for hosted in [false, true] {
        let maximum = if hosted {
            MAX_HOSTED_BLOCKS
        } else {
            MAX_NATIVE_BLOCKS
        };
        let full = maximum / 121;
        let remainder = maximum % 121;
        assert_eq!((remainder - 1) % 3, 0);
        let mut source = String::new();
        for index in 0..=full {
            let name = if index == 0 {
                "main".to_owned()
            } else {
                format!("f{index}")
            };
            let branches = if index == full {
                (remainder - 1) / 3
            } else {
                40
            };
            source.push_str(&format!(
                "fn {name}()->i32{{{}return 7;}}",
                "if true{}else{}".repeat(branches)
            ));
        }
        let (program, _) = verified_with_sources(&source);
        assert_eq!(
            program
                .program
                .functions
                .iter()
                .map(|f| f.blocks.len())
                .sum::<usize>(),
            maximum
        );
        program.admit_with_execution(hosted).unwrap();
        let (program, _) = verified_with_sources(&(source + "fn extra()->i32{return 0;}"));
        let error = program.admit_with_execution(hosted).unwrap_err();
        assert!(error
            .message
            .contains(&format!("aggregate blocks limit exceeded ({maximum})")));
    }
}
