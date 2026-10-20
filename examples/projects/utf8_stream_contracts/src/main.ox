use stream::Decoder;
use stream::Step;
use stream::Finish;
use stream::new_decoder;
use stream::push_byte;
use stream::finish;
use stat::Counts;
use stat::new_counts;
use stat::feed;
use stat::finalize;
use stat::render;

// Independent contracts for the frozen `utf8_stream` decoder and for the
// shared oxtextstat statistics helper. Every expected scalar, offset and
// report below is a fixed literal; none is computed by the decoder.
// A nonzero test result names the first failing check.

fn byte(value: i32) -> u8 {
    return value.to_u8_checked();
}

fn step_pending(decoder: &mut Decoder, value: i32) -> bool {
    let step = push_byte(&mut *decoder, byte(value));
    match step {
        Step::Pending => { return true; },
        Step::Scalar(actual) => { return false; },
        Step::Invalid(offset) => { return false; },
        Step::PositionOverflow => { return false; },
        Step::Finished => { return false; },
    }
}

fn step_scalar(decoder: &mut Decoder, value: i32, expected: i32) -> bool {
    let step = push_byte(&mut *decoder, byte(value));
    match step {
        Step::Pending => { return false; },
        Step::Scalar(actual) => { return actual == expected; },
        Step::Invalid(offset) => { return false; },
        Step::PositionOverflow => { return false; },
        Step::Finished => { return false; },
    }
}

fn step_invalid(decoder: &mut Decoder, value: i32, expected: i32) -> bool {
    let step = push_byte(&mut *decoder, byte(value));
    match step {
        Step::Pending => { return false; },
        Step::Scalar(actual) => { return false; },
        Step::Invalid(offset) => { return offset == expected; },
        Step::PositionOverflow => { return false; },
        Step::Finished => { return false; },
    }
}

fn step_finished(decoder: &mut Decoder, value: i32) -> bool {
    let step = push_byte(&mut *decoder, byte(value));
    match step {
        Step::Pending => { return false; },
        Step::Scalar(actual) => { return false; },
        Step::Invalid(offset) => { return false; },
        Step::PositionOverflow => { return false; },
        Step::Finished => { return true; },
    }
}

fn finish_complete(decoder: &mut Decoder) -> bool {
    let finished = finish(&mut *decoder);
    match finished {
        Finish::Complete => { return true; },
        Finish::Invalid(offset) => { return false; },
        Finish::PositionOverflow => { return false; },
    }
}

fn finish_invalid(decoder: &mut Decoder, expected: i32) -> bool {
    let finished = finish(&mut *decoder);
    match finished {
        Finish::Complete => { return false; },
        Finish::Invalid(offset) => { return offset == expected; },
        Finish::PositionOverflow => { return false; },
    }
}

// After Invalid(n): pushes and finishes keep returning Invalid(n).
fn sticky_invalid(decoder: &mut Decoder, expected: i32) -> bool {
    if !step_invalid(&mut *decoder, 65, expected) { return false; }
    if !step_invalid(&mut *decoder, 128, expected) { return false; }
    if !step_invalid(&mut *decoder, 194, expected) { return false; }
    if !finish_invalid(&mut *decoder, expected) { return false; }
    if !finish_invalid(&mut *decoder, expected) { return false; }
    return step_invalid(&mut *decoder, 10, expected);
}

fn decodes_two(lead: i32, second: i32, expected: i32) -> bool {
    let mut decoder = new_decoder();
    if !step_pending(&mut decoder, lead) { return false; }
    if !step_scalar(&mut decoder, second, expected) { return false; }
    return finish_complete(&mut decoder);
}

fn decodes_three(lead: i32, second: i32, third: i32, expected: i32) -> bool {
    let mut decoder = new_decoder();
    if !step_pending(&mut decoder, lead) { return false; }
    if !step_pending(&mut decoder, second) { return false; }
    if !step_scalar(&mut decoder, third, expected) { return false; }
    return finish_complete(&mut decoder);
}

fn decodes_four(lead: i32, second: i32, third: i32, fourth: i32, expected: i32) -> bool {
    let mut decoder = new_decoder();
    if !step_pending(&mut decoder, lead) { return false; }
    if !step_pending(&mut decoder, second) { return false; }
    if !step_pending(&mut decoder, third) { return false; }
    if !step_scalar(&mut decoder, fourth, expected) { return false; }
    return finish_complete(&mut decoder);
}

fn rejects_lead(lead: i32) -> bool {
    let mut decoder = new_decoder();
    if !step_invalid(&mut decoder, lead, 0) { return false; }
    return sticky_invalid(&mut decoder, 0);
}

fn rejects_second(lead: i32, second: i32) -> bool {
    let mut decoder = new_decoder();
    if !step_pending(&mut decoder, lead) { return false; }
    if !step_invalid(&mut decoder, second, 0) { return false; }
    return sticky_invalid(&mut decoder, 0);
}

fn rejects_third(lead: i32, second: i32, third: i32) -> bool {
    let mut decoder = new_decoder();
    if !step_pending(&mut decoder, lead) { return false; }
    if !step_pending(&mut decoder, second) { return false; }
    if !step_invalid(&mut decoder, third, 0) { return false; }
    return sticky_invalid(&mut decoder, 0);
}

fn rejects_fourth(lead: i32, second: i32, third: i32, fourth: i32) -> bool {
    let mut decoder = new_decoder();
    if !step_pending(&mut decoder, lead) { return false; }
    if !step_pending(&mut decoder, second) { return false; }
    if !step_pending(&mut decoder, third) { return false; }
    if !step_invalid(&mut decoder, fourth, 0) { return false; }
    return sticky_invalid(&mut decoder, 0);
}

fn test_ascii_and_nul_scalars() -> i32 {
    let mut decoder = new_decoder();
    if !step_scalar(&mut decoder, 0, 0) { return 101; }
    if !step_scalar(&mut decoder, 65, 65) { return 102; }
    if !step_scalar(&mut decoder, 10, 10) { return 103; }
    if !step_scalar(&mut decoder, 13, 13) { return 104; }
    if !step_scalar(&mut decoder, 127, 127) { return 105; }
    if !finish_complete(&mut decoder) { return 106; }
    return 0;
}

fn test_two_byte_boundaries() -> i32 {
    if !decodes_two(194, 128, 128) { return 201; }
    if !decodes_two(223, 191, 2047) { return 202; }
    if !decodes_two(195, 169, 233) { return 203; }
    if !decodes_two(194, 162, 162) { return 204; }
    return 0;
}

fn test_three_byte_boundaries() -> i32 {
    if !decodes_three(224, 160, 128, 2048) { return 301; }
    if !decodes_three(224, 160, 159, 2079) { return 302; }
    if !decodes_three(224, 191, 191, 4095) { return 303; }
    if !decodes_three(225, 128, 128, 4096) { return 304; }
    if !decodes_three(236, 191, 191, 53247) { return 305; }
    if !decodes_three(237, 128, 128, 53248) { return 306; }
    if !decodes_three(237, 128, 191, 53311) { return 307; }
    if !decodes_three(237, 159, 191, 55295) { return 308; }
    if !decodes_three(238, 128, 128, 57344) { return 309; }
    if !decodes_three(239, 191, 191, 65535) { return 310; }
    if !decodes_three(226, 130, 172, 8364) { return 311; }
    return 0;
}

fn test_four_byte_boundaries() -> i32 {
    if !decodes_four(240, 144, 128, 128, 65536) { return 401; }
    if !decodes_four(240, 144, 191, 191, 69631) { return 402; }
    if !decodes_four(240, 191, 191, 191, 262143) { return 403; }
    if !decodes_four(241, 128, 128, 128, 262144) { return 404; }
    if !decodes_four(243, 191, 191, 191, 1048575) { return 405; }
    if !decodes_four(244, 128, 128, 128, 1048576) { return 406; }
    if !decodes_four(244, 128, 191, 191, 1052671) { return 407; }
    if !decodes_four(244, 143, 191, 191, 1114111) { return 408; }
    if !decodes_four(240, 159, 152, 128, 128512) { return 409; }
    return 0;
}

fn test_bom_noncharacters_and_combining() -> i32 {
    if !decodes_three(239, 187, 191, 65279) { return 501; }
    if !decodes_three(239, 183, 144, 64976) { return 502; }
    if !decodes_three(239, 191, 190, 65534) { return 503; }
    if !decodes_four(240, 159, 191, 191, 131071) { return 504; }
    let mut decoder = new_decoder();
    if !step_scalar(&mut decoder, 101, 101) { return 505; }
    if !step_pending(&mut decoder, 204) { return 506; }
    if !step_scalar(&mut decoder, 129, 769) { return 507; }
    if !finish_complete(&mut decoder) { return 508; }
    return 0;
}

fn test_mixed_sequence_events() -> i32 {
    let mut decoder = new_decoder();
    if !step_scalar(&mut decoder, 65, 65) { return 601; }
    if !step_pending(&mut decoder, 194) { return 602; }
    if !step_scalar(&mut decoder, 162, 162) { return 603; }
    if !step_pending(&mut decoder, 226) { return 604; }
    if !step_pending(&mut decoder, 130) { return 605; }
    if !step_scalar(&mut decoder, 172, 8364) { return 606; }
    if !step_pending(&mut decoder, 240) { return 607; }
    if !step_pending(&mut decoder, 159) { return 608; }
    if !step_pending(&mut decoder, 152) { return 609; }
    if !step_scalar(&mut decoder, 128, 128512) { return 610; }
    if !step_scalar(&mut decoder, 10, 10) { return 611; }
    if !finish_complete(&mut decoder) { return 612; }
    return 0;
}

fn test_invalid_leads_and_stray_continuations() -> i32 {
    if !rejects_lead(128) { return 701; }
    if !rejects_lead(143) { return 702; }
    if !rejects_lead(191) { return 703; }
    if !rejects_lead(192) { return 704; }
    if !rejects_lead(193) { return 705; }
    if !rejects_lead(245) { return 706; }
    if !rejects_lead(247) { return 707; }
    if !rejects_lead(248) { return 708; }
    if !rejects_lead(254) { return 709; }
    if !rejects_lead(255) { return 710; }
    let mut decoder = new_decoder();
    if !step_scalar(&mut decoder, 65, 65) { return 711; }
    if !step_invalid(&mut decoder, 192, 1) { return 712; }
    if !sticky_invalid(&mut decoder, 1) { return 713; }
    let mut later = new_decoder();
    if !step_pending(&mut later, 194) { return 714; }
    if !step_scalar(&mut later, 162, 162) { return 715; }
    if !step_invalid(&mut later, 128, 2) { return 716; }
    if !sticky_invalid(&mut later, 2) { return 717; }
    return 0;
}

fn test_second_byte_restrictions() -> i32 {
    if !rejects_second(224, 128) { return 801; }
    if !rejects_second(224, 159) { return 802; }
    if !rejects_second(237, 160) { return 803; }
    if !rejects_second(237, 191) { return 804; }
    if !rejects_second(240, 128) { return 805; }
    if !rejects_second(240, 143) { return 806; }
    if !rejects_second(244, 144) { return 807; }
    if !rejects_second(244, 191) { return 808; }
    if !rejects_second(194, 127) { return 809; }
    if !rejects_second(194, 192) { return 810; }
    if !rejects_second(225, 127) { return 811; }
    if !rejects_second(225, 192) { return 812; }
    if !rejects_second(241, 127) { return 813; }
    if !rejects_second(241, 192) { return 814; }
    if !rejects_second(224, 65) { return 815; }
    let mut decoder = new_decoder();
    if !step_scalar(&mut decoder, 65, 65) { return 816; }
    if !step_scalar(&mut decoder, 66, 66) { return 817; }
    if !step_pending(&mut decoder, 224) { return 818; }
    if !step_invalid(&mut decoder, 128, 2) { return 819; }
    if !sticky_invalid(&mut decoder, 2) { return 820; }
    return 0;
}

fn test_wrong_later_continuations() -> i32 {
    if !rejects_third(226, 130, 40) { return 901; }
    if !rejects_third(226, 130, 192) { return 902; }
    if !rejects_third(224, 160, 127) { return 903; }
    if !rejects_third(237, 128, 192) { return 904; }
    if !rejects_third(240, 159, 65) { return 905; }
    if !rejects_third(244, 143, 192) { return 906; }
    if !rejects_fourth(240, 159, 152, 65) { return 907; }
    if !rejects_fourth(240, 159, 152, 192) { return 908; }
    if !rejects_fourth(244, 143, 191, 127) { return 909; }
    let mut decoder = new_decoder();
    if !step_scalar(&mut decoder, 65, 65) { return 910; }
    if !step_pending(&mut decoder, 226) { return 911; }
    if !step_invalid(&mut decoder, 40, 1) { return 912; }
    if !sticky_invalid(&mut decoder, 1) { return 913; }
    let mut later = new_decoder();
    if !step_scalar(&mut later, 65, 65) { return 914; }
    if !step_scalar(&mut later, 66, 66) { return 915; }
    if !step_scalar(&mut later, 67, 67) { return 916; }
    if !step_pending(&mut later, 240) { return 917; }
    if !step_pending(&mut later, 159) { return 918; }
    if !step_pending(&mut later, 152) { return 919; }
    if !step_invalid(&mut later, 127, 3) { return 920; }
    if !sticky_invalid(&mut later, 3) { return 921; }
    return 0;
}

fn test_incomplete_tails_at_eof() -> i32 {
    let mut two = new_decoder();
    if !step_pending(&mut two, 194) { return 1001; }
    if !finish_invalid(&mut two, 0) { return 1002; }
    let mut three_one = new_decoder();
    if !step_pending(&mut three_one, 226) { return 1003; }
    if !finish_invalid(&mut three_one, 0) { return 1004; }
    let mut three_two = new_decoder();
    if !step_pending(&mut three_two, 226) { return 1005; }
    if !step_pending(&mut three_two, 130) { return 1006; }
    if !finish_invalid(&mut three_two, 0) { return 1007; }
    let mut four_one = new_decoder();
    if !step_pending(&mut four_one, 240) { return 1008; }
    if !finish_invalid(&mut four_one, 0) { return 1009; }
    let mut four_two = new_decoder();
    if !step_pending(&mut four_two, 240) { return 1010; }
    if !step_pending(&mut four_two, 159) { return 1011; }
    if !finish_invalid(&mut four_two, 0) { return 1012; }
    let mut four_three = new_decoder();
    if !step_pending(&mut four_three, 240) { return 1013; }
    if !step_pending(&mut four_three, 159) { return 1014; }
    if !step_pending(&mut four_three, 152) { return 1015; }
    if !finish_invalid(&mut four_three, 0) { return 1016; }
    let mut prefixed = new_decoder();
    if !step_scalar(&mut prefixed, 65, 65) { return 1017; }
    if !step_pending(&mut prefixed, 240) { return 1018; }
    if !step_pending(&mut prefixed, 144) { return 1019; }
    if !finish_invalid(&mut prefixed, 1) { return 1020; }
    let mut later = new_decoder();
    if !step_scalar(&mut later, 65, 65) { return 1021; }
    if !step_pending(&mut later, 194) { return 1022; }
    if !step_scalar(&mut later, 162, 162) { return 1023; }
    if !step_pending(&mut later, 226) { return 1024; }
    if !step_pending(&mut later, 130) { return 1025; }
    if !finish_invalid(&mut later, 3) { return 1026; }
    return 0;
}

fn test_premature_eof_cannot_be_repaired() -> i32 {
    let mut decoder = new_decoder();
    if !step_scalar(&mut decoder, 65, 65) { return 1101; }
    if !step_pending(&mut decoder, 226) { return 1102; }
    if !step_pending(&mut decoder, 130) { return 1103; }
    if !finish_invalid(&mut decoder, 1) { return 1104; }
    if !step_invalid(&mut decoder, 172, 1) { return 1105; }
    if !finish_invalid(&mut decoder, 1) { return 1106; }
    if !sticky_invalid(&mut decoder, 1) { return 1107; }
    return 0;
}

fn test_successful_finish_closes_stream() -> i32 {
    let mut empty = new_decoder();
    if !finish_complete(&mut empty) { return 1201; }
    if !finish_complete(&mut empty) { return 1202; }
    if !step_finished(&mut empty, 65) { return 1203; }
    if !step_finished(&mut empty, 128) { return 1204; }
    if !finish_complete(&mut empty) { return 1205; }
    let mut decoder = new_decoder();
    if !step_scalar(&mut decoder, 65, 65) { return 1206; }
    if !finish_complete(&mut decoder) { return 1207; }
    if !step_finished(&mut decoder, 194) { return 1208; }
    if !step_finished(&mut decoder, 162) { return 1209; }
    if !finish_complete(&mut decoder) { return 1210; }
    if !step_finished(&mut decoder, 10) { return 1211; }
    return 0;
}

// ---- shared oxtextstat helper: the same feed/finalize/render as the app ----

fn same_prefix(actual: &[u8], length: i32, expected: &[u8]) -> bool {
    if length != expected.len() { return false; }
    if length > actual.len() { return false; }
    let mut index = 0;
    while index < length {
        let left = actual[index];
        let right = expected[index];
        if left != right { return false; }
        index = index + 1;
    }
    return true;
}

fn report_is(counts: &Counts, expected: &[u8]) -> bool {
    let mut output = b"\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA";
    let length = render(&*counts, &mut output);
    return same_prefix(&output, length, &*expected);
}

fn unreported(counts: &Counts) -> bool {
    let mut output = b"\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA";
    let length = render(&*counts, &mut output);
    if length != -1 { return false; }
    let mut index = 0;
    while index < output.len() {
        let current = output[index];
        if current.to_i32() != 170 { return false; }
        index = index + 1;
    }
    return true;
}

// The decoder retained by the helper is closed after one successful finalize.
fn closed(decoder: &mut Decoder) -> bool {
    if !step_finished(&mut *decoder, 65) { return false; }
    return finish_complete(&mut *decoder);
}

fn whole_reports(input: &[u8], expected: &[u8]) -> bool {
    let mut decoder = new_decoder();
    let mut counts = new_counts();
    if feed(&mut decoder, &mut counts, &*input) != 0 { return false; }
    if !unreported(&counts) { return false; }
    if finalize(&mut decoder, &mut counts) != 0 { return false; }
    if !closed(&mut decoder) { return false; }
    return report_is(&counts, &*expected);
}

fn feed_rejects(input: &[u8]) -> bool {
    let mut decoder = new_decoder();
    let mut counts = new_counts();
    if feed(&mut decoder, &mut counts, &*input) != 65 { return false; }
    if feed(&mut decoder, &mut counts, &*input) != 65 { return false; }
    if finalize(&mut decoder, &mut counts) != 65 { return false; }
    return unreported(&counts);
}

fn finalize_rejects(input: &[u8]) -> bool {
    let mut decoder = new_decoder();
    let mut counts = new_counts();
    if feed(&mut decoder, &mut counts, &*input) != 0 { return false; }
    if finalize(&mut decoder, &mut counts) != 65 { return false; }
    if finalize(&mut decoder, &mut counts) != 65 { return false; }
    return unreported(&counts);
}

// Every two-chunk partition input[0..split] + input[split..length], including
// both empty-chunk ends, with one finalize after the last chunk.
fn every_split_reports(input: &[u8], expected: &[u8]) -> i32 {
    let length = input.len();
    let mut split = 0;
    while split <= length {
        let mut decoder = new_decoder();
        let mut counts = new_counts();
        if feed(&mut decoder, &mut counts, &*input[0..split]) != 0 { return 1; }
        if !unreported(&counts) { return 2; }
        if feed(&mut decoder, &mut counts, &*input[split..length]) != 0 { return 3; }
        if finalize(&mut decoder, &mut counts) != 0 { return 4; }
        if !report_is(&counts, &*expected) { return 5; }
        split = split + 1;
    }
    return 0;
}

// Every three-chunk partition [0..first] + [first..second] + [second..length].
fn every_three_chunk_reports(input: &[u8], expected: &[u8]) -> i32 {
    let length = input.len();
    let mut first = 0;
    while first <= length {
        let mut second = first;
        while second <= length {
            let mut decoder = new_decoder();
            let mut counts = new_counts();
            if feed(&mut decoder, &mut counts, &*input[0..first]) != 0 { return 1; }
            if feed(&mut decoder, &mut counts, &*input[first..second]) != 0 { return 2; }
            if feed(&mut decoder, &mut counts, &*input[second..length]) != 0 { return 3; }
            if finalize(&mut decoder, &mut counts) != 0 { return 4; }
            if !report_is(&counts, &*expected) { return 5; }
            second = second + 1;
        }
        first = first + 1;
    }
    return 0;
}

// One-byte chunks separated by empty chunks; finalize only after the last.
fn bytewise_reports(input: &[u8], expected: &[u8]) -> bool {
    let empty = b"";
    let mut decoder = new_decoder();
    let mut counts = new_counts();
    let length = input.len();
    let mut index = 0;
    while index < length {
        if feed(&mut decoder, &mut counts, &empty) != 0 { return false; }
        let next = index + 1;
        if feed(&mut decoder, &mut counts, &*input[index..next]) != 0 { return false; }
        if !unreported(&counts) { return false; }
        index = next;
    }
    if feed(&mut decoder, &mut counts, &empty) != 0 { return false; }
    if finalize(&mut decoder, &mut counts) != 0 { return false; }
    return report_is(&counts, &*expected);
}

fn test_consumer_fixed_reports() -> i32 {
    let empty = b"";
    let letter = b"A";
    let lf = b"\n";
    let cr = b"\r";
    let crlf = b"\r\n";
    let lf_lf = b"\n\n";
    let unterminated = b"A\nB";
    let terminated = b"A\nB\n";
    let nul = b"\0";
    let bom = b"\xEF\xBB\xBF";
    let combining = b"e\xCC\x81";
    let mixed = b"A\xC2\xA2\xE2\x82\xAC\xF0\x9F\x98\x80\n";
    let report_empty = b"0 0 0\n";
    let report_one = b"1 1 1\n";
    let report_crlf = b"2 2 1\n";
    let report_lf_lf = b"2 2 2\n";
    let report_unterminated = b"3 3 2\n";
    let report_terminated = b"4 4 2\n";
    let report_bom = b"3 1 1\n";
    let report_combining = b"3 2 1\n";
    let report_mixed = b"11 5 1\n";
    if !whole_reports(&empty, &report_empty) { return 1301; }
    if !whole_reports(&letter, &report_one) { return 1302; }
    if !whole_reports(&lf, &report_one) { return 1303; }
    if !whole_reports(&cr, &report_one) { return 1304; }
    if !whole_reports(&crlf, &report_crlf) { return 1305; }
    if !whole_reports(&lf_lf, &report_lf_lf) { return 1306; }
    if !whole_reports(&unterminated, &report_unterminated) { return 1307; }
    if !whole_reports(&terminated, &report_terminated) { return 1308; }
    if !whole_reports(&nul, &report_one) { return 1309; }
    if !whole_reports(&bom, &report_bom) { return 1310; }
    if !whole_reports(&combining, &report_combining) { return 1311; }
    if !whole_reports(&mixed, &report_mixed) { return 1312; }
    return 0;
}

fn test_consumer_rejections_emit_no_report() -> i32 {
    let stray = b"\x80";
    let wrong_third = b"A\xE2(";
    let surrogate = b"\xED\xA0\x80";
    let above_max = b"\xF4\x90\x80\x80";
    let late = b"A\nB\n\xC0";
    let truncated = b"A\xF0\x90";
    let truncated_four = b"\xF0\x9F\x98";
    if !feed_rejects(&stray) { return 1401; }
    if !feed_rejects(&wrong_third) { return 1402; }
    if !feed_rejects(&surrogate) { return 1403; }
    if !feed_rejects(&above_max) { return 1404; }
    if !feed_rejects(&late) { return 1405; }
    if !finalize_rejects(&truncated) { return 1406; }
    if !finalize_rejects(&truncated_four) { return 1407; }
    return 0;
}

fn test_consumer_every_two_chunk_split() -> i32 {
    let two = b"\xC2\xA2";
    let three = b"\xE2\x82\xAC";
    let four = b"\xF0\x9F\x98\x80";
    let mixed = b"A\xC2\xA2\xE2\x82\xAC\xF0\x9F\x98\x80\n";
    let report_two = b"2 1 1\n";
    let report_three = b"3 1 1\n";
    let report_four = b"4 1 1\n";
    let report_mixed = b"11 5 1\n";
    let first = every_split_reports(&two, &report_two);
    if first != 0 { return 1500 + first; }
    let second = every_split_reports(&three, &report_three);
    if second != 0 { return 1510 + second; }
    let third = every_split_reports(&four, &report_four);
    if third != 0 { return 1520 + third; }
    let fourth = every_split_reports(&mixed, &report_mixed);
    if fourth != 0 { return 1530 + fourth; }
    return 0;
}

fn test_consumer_every_three_chunk_split() -> i32 {
    let three = b"\xE2\x82\xAC";
    let four = b"\xF0\x9F\x98\x80";
    let crlf_tail = b"\xF0\x9F\x98\x80\r\nZ";
    let report_three = b"3 1 1\n";
    let report_four = b"4 1 1\n";
    let report_crlf_tail = b"7 4 2\n";
    let first = every_three_chunk_reports(&three, &report_three);
    if first != 0 { return 1600 + first; }
    let second = every_three_chunk_reports(&four, &report_four);
    if second != 0 { return 1610 + second; }
    let third = every_three_chunk_reports(&crlf_tail, &report_crlf_tail);
    if third != 0 { return 1620 + third; }
    return 0;
}

fn test_consumer_bytewise_with_empty_chunks() -> i32 {
    let mixed = b"A\xC2\xA2\xE2\x82\xAC\xF0\x9F\x98\x80\n";
    let report_mixed = b"11 5 1\n";
    let tail = b"\xEF\xBB\xBF\0\re\xCC\x81";
    let report_tail = b"8 5 1\n";
    if !bytewise_reports(&mixed, &report_mixed) { return 1701; }
    if !bytewise_reports(&tail, &report_tail) { return 1702; }
    return 0;
}

fn test_consumer_split_errors() -> i32 {
    let wrong_head = b"A\xE2";
    let wrong_tail = b"(";
    let mut decoder = new_decoder();
    let mut counts = new_counts();
    if feed(&mut decoder, &mut counts, &wrong_head) != 0 { return 1801; }
    if feed(&mut decoder, &mut counts, &wrong_tail) != 65 { return 1802; }
    if !step_invalid(&mut decoder, 65, 1) { return 1803; }
    if finalize(&mut decoder, &mut counts) != 65 { return 1804; }
    if !unreported(&counts) { return 1805; }
    let truncated_head = b"\xF0\x9F";
    let truncated_tail = b"\x98";
    let mut later = new_decoder();
    let mut later_counts = new_counts();
    if feed(&mut later, &mut later_counts, &truncated_head) != 0 { return 1806; }
    if feed(&mut later, &mut later_counts, &truncated_tail) != 0 { return 1807; }
    if !unreported(&later_counts) { return 1808; }
    if finalize(&mut later, &mut later_counts) != 65 { return 1809; }
    if !finish_invalid(&mut later, 0) { return 1810; }
    if !unreported(&later_counts) { return 1811; }
    let valid_chunk = b"A\n";
    let invalid_chunk = b"\x80";
    let mut prefix = new_decoder();
    let mut prefix_counts = new_counts();
    if feed(&mut prefix, &mut prefix_counts, &valid_chunk) != 0 { return 1812; }
    if feed(&mut prefix, &mut prefix_counts, &invalid_chunk) != 65 { return 1813; }
    if feed(&mut prefix, &mut prefix_counts, &valid_chunk) != 65 { return 1814; }
    if !step_invalid(&mut prefix, 65, 2) { return 1815; }
    if finalize(&mut prefix, &mut prefix_counts) != 65 { return 1816; }
    if !unreported(&prefix_counts) { return 1817; }
    return 0;
}

fn test_consumer_lifecycle_and_render_bounds() -> i32 {
    let empty = b"";
    let letter = b"A";
    let report_empty = b"0 0 0\n";
    let mut decoder = new_decoder();
    let mut counts = new_counts();
    if !unreported(&counts) { return 1901; }
    if finalize(&mut decoder, &mut counts) != 0 { return 1902; }
    if !report_is(&counts, &report_empty) { return 1903; }
    if finalize(&mut decoder, &mut counts) != 70 { return 1904; }
    if feed(&mut decoder, &mut counts, &letter) != 70 { return 1905; }
    if feed(&mut decoder, &mut counts, &empty) != 70 { return 1906; }
    if !report_is(&counts, &report_empty) { return 1907; }
    if !closed(&mut decoder) { return 1908; }
    let mut short = b"\xAA\xAA\xAA\xAA\xAA";
    if render(&counts, &mut short) != -2 { return 1909; }
    let mut exact = b"\xAA\xAA\xAA\xAA\xAA\xAA";
    let length = render(&counts, &mut exact);
    if !same_prefix(&exact, length, &report_empty) { return 1910; }
    return 0;
}
