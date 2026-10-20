use alloc::bytes::GrowStatus;
use alloc::bytes::new as new_bytes;
use alloc::bytes::try_extend;
use alloc::bytes::truncate as truncate_bytes;
use csv::Reader;
use csv::FieldInfo;
use csv::Next;
use csv::CopyResult;
use csv::new_reader;
use csv::next_field;
use csv::copy_current_field;
use sum::new_summary;
use sum::summarize;
use sum::render;

// Independent contracts for the frozen `csv_records` reader and for the shared
// oxcsvsum summary helper. Every expected field, flag, offset, size and report
// below is a fixed hand-written literal; none is produced by the code under
// test. A nonzero test result names the first failing check.

fn grew(status: GrowStatus) -> bool {
    match status {
        GrowStatus::Ok => { return true; },
        GrowStatus::CapacityOverflow => { return false; },
        GrowStatus::AllocationFailed => { return false; },
        GrowStatus::MemoryLimit => { return false; },
    }
}

fn same(actual: &[u8], expected: &[u8]) -> bool {
    if actual.len() != expected.len() {
        return false;
    }
    let mut index = 0;
    while index < actual.len() {
        if actual[index] != expected[index] {
            return false;
        }
        index = index + 1;
    }
    return true;
}

fn all_are(actual: &[u8], value: i32) -> bool {
    let mut index = 0;
    while index < actual.len() {
        let current = actual[index];
        if current.to_i32() != value {
            return false;
        }
        index = index + 1;
    }
    return true;
}

// Reader owning a copy of input. A refused copy leaves an empty owner, which
// every case expecting a first Field or Malformed result detects.
fn open(input: &[u8]) -> Reader {
    let mut bytes = new_bytes();
    if !grew(try_extend(&mut bytes, &*input)) {
        truncate_bytes(&mut bytes, 0);
    }
    return new_reader(bytes);
}

// Changes only the caller's FieldInfo value; the reader is never given it.
fn forge(info: &mut FieldInfo, decoded_len: i32, record_end: bool) -> () {
    info.decoded_len = decoded_len;
    info.record_end = record_end;
    return;
}

// Copies the current field into 32 bytes of 0xAA: exact prefix, intact suffix.
fn copies(reader: &Reader, expected: &[u8]) -> bool {
    let mut output = b"\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA";
    let copied = copy_current_field(&*reader, &mut output);
    match copied {
        CopyResult::Written(count) => {
            if count != expected.len() {
                return false;
            }
            if !same(&output[0..count], &*expected) {
                return false;
            }
            return all_are(&output[count..32], 170);
        },
        CopyResult::BufferTooSmall(required) => { return false; },
        CopyResult::NoCurrent => { return false; },
    }
}

fn written(reader: &Reader, output: &mut [u8], expected: i32) -> bool {
    let copied = copy_current_field(&*reader, &mut *output);
    match copied {
        CopyResult::Written(count) => { return count == expected; },
        CopyResult::BufferTooSmall(required) => { return false; },
        CopyResult::NoCurrent => { return false; },
    }
}

fn refused(reader: &Reader, output: &mut [u8], expected: i32) -> bool {
    let copied = copy_current_field(&*reader, &mut *output);
    match copied {
        CopyResult::Written(count) => { return false; },
        CopyResult::BufferTooSmall(required) => { return required == expected; },
        CopyResult::NoCurrent => { return false; },
    }
}

// NoCurrent leaves all four 0x5A bytes unchanged.
fn no_current(reader: &Reader) -> bool {
    let mut output = b"\x5A\x5A\x5A\x5A";
    let copied = copy_current_field(&*reader, &mut output);
    match copied {
        CopyResult::Written(count) => { return false; },
        CopyResult::BufferTooSmall(required) => { return false; },
        CopyResult::NoCurrent => { return all_are(&output, 90); },
    }
}

// Advances once; checks only the returned metadata, without copying.
fn expect_info(reader: &mut Reader, decoded_len: i32, record_end: bool) -> bool {
    let next = next_field(&mut *reader);
    match next {
        Next::Field(info) => { return info.decoded_len == decoded_len && info.record_end == record_end; },
        Next::End => { return false; },
        Next::Malformed(offset) => { return false; },
    }
}

// Advances once; checks metadata, then copies the decoded field.
fn expect_field(reader: &mut Reader, expected: &[u8], record_end: bool) -> bool {
    if !expect_info(&mut *reader, expected.len(), record_end) {
        return false;
    }
    return copies(&*reader, &*expected);
}

// Advances once to End; End also invalidates any current field.
fn expect_end(reader: &mut Reader) -> bool {
    let next = next_field(&mut *reader);
    match next {
        Next::Field(info) => { return false; },
        Next::End => { return no_current(&*reader); },
        Next::Malformed(offset) => { return false; },
    }
}

fn expect_malformed(reader: &mut Reader, expected: i32) -> bool {
    let next = next_field(&mut *reader);
    match next {
        Next::Field(info) => { return false; },
        Next::End => { return false; },
        Next::Malformed(offset) => {
            if offset != expected {
                return false;
            }
            return no_current(&*reader);
        },
    }
}

// End and Malformed are sticky: three identical results each.
fn ended(reader: &mut Reader) -> bool {
    if !expect_end(&mut *reader) {
        return false;
    }
    if !expect_end(&mut *reader) {
        return false;
    }
    return expect_end(&mut *reader);
}

fn stuck(reader: &mut Reader, offset: i32) -> bool {
    if !expect_malformed(&mut *reader, offset) {
        return false;
    }
    if !expect_malformed(&mut *reader, offset) {
        return false;
    }
    return expect_malformed(&mut *reader, offset);
}

fn one_field(input: &[u8], expected: &[u8]) -> bool {
    let mut reader = open(&*input);
    if !expect_field(&mut reader, &*expected, true) {
        return false;
    }
    return ended(&mut reader);
}

// Two fields; the second always has record_end true.
fn two_fields(input: &[u8], first: &[u8], first_end: bool, second: &[u8]) -> bool {
    let mut reader = open(&*input);
    if !expect_field(&mut reader, &*first, first_end) {
        return false;
    }
    if !expect_field(&mut reader, &*second, true) {
        return false;
    }
    return ended(&mut reader);
}

fn malformed_at(input: &[u8], offset: i32) -> bool {
    let mut reader = open(&*input);
    if !no_current(&reader) {
        return false;
    }
    return stuck(&mut reader, offset);
}

fn malformed_after(input: &[u8], first: &[u8], first_end: bool, offset: i32) -> bool {
    let mut reader = open(&*input);
    if !expect_field(&mut reader, &*first, first_end) {
        return false;
    }
    return stuck(&mut reader, offset);
}

fn summary_reports(input: &[u8], expected: &[u8]) -> bool {
    let mut bytes = new_bytes();
    if !grew(try_extend(&mut bytes, &*input)) {
        return false;
    }
    let mut summary = new_summary();
    if summarize(bytes, &mut summary) != 0 {
        return false;
    }
    let mut output = b"\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA";
    let length = render(&summary, &mut output);
    if length != expected.len() {
        return false;
    }
    if !same(&output[0..length], &*expected) {
        return false;
    }
    return all_are(&output[length..24], 170);
}

// A rejected input leaves no report: render refuses and changes nothing.
fn summary_rejects(input: &[u8], status: i32) -> bool {
    let mut bytes = new_bytes();
    if !grew(try_extend(&mut bytes, &*input)) {
        return false;
    }
    let mut summary = new_summary();
    if summarize(bytes, &mut summary) != status {
        return false;
    }
    let mut output = b"\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA";
    if render(&summary, &mut output) != -1 {
        return false;
    }
    return all_are(&output, 170);
}

// Package README dialect table, row by row.
fn test_dialect_table() -> i32 {
    let in_empty = b"";
    let in_comma = b",";
    let in_a = b"a";
    let in_a_comma = b"a,";
    let in_lf = b"\n";
    let in_lf_lf = b"\n\n";
    let in_a_lf = b"a\n";
    let in_a_comma_lf = b"a,\n";
    let in_quoted_empty = b"\"\"";
    let in_quoted_comma = b"\"a,b\",c";
    let in_doubled = b"\"a\"\"b\"";
    let want_a = b"a";
    let want_empty = b"";
    let want_blank = b"";
    let want_a_comma_b = b"a,b";
    let want_c = b"c";
    let want_a_quote_b = b"a\"b";
    let mut nothing = open(&in_empty);
    if !no_current(&nothing) { return 101; }
    if !ended(&mut nothing) { return 102; }
    if !two_fields(&in_comma, &want_empty, false, &want_blank) { return 103; }
    if !one_field(&in_a, &want_a) { return 104; }
    if !two_fields(&in_a_comma, &want_a, false, &want_empty) { return 105; }
    if !one_field(&in_lf, &want_empty) { return 106; }
    if !two_fields(&in_lf_lf, &want_empty, true, &want_blank) { return 107; }
    if !one_field(&in_a_lf, &want_a) { return 108; }
    if !two_fields(&in_a_comma_lf, &want_a, false, &want_empty) { return 109; }
    if !one_field(&in_quoted_empty, &want_empty) { return 110; }
    if !two_fields(&in_quoted_comma, &want_a_comma_b, false, &want_c) { return 111; }
    if !one_field(&in_doubled, &want_a_quote_b) { return 112; }
    return 0;
}

// CRLF records, comma followed by CRLF/EOF, and closing-quote separators.
fn test_crlf_and_trailing_comma_boundaries() -> i32 {
    let in_crlf_records = b"a\r\nb\r\n";
    let in_crlf = b"\r\n";
    let in_crlf_crlf = b"\r\n\r\n";
    let in_comma_crlf = b"a,\r\n";
    let in_comma_crlf_more = b"a,\r\nb";
    let in_quoted_comma_crlf = b"\"a\",\r\n";
    let in_quoted_crlf = b"\"a\"\r\n";
    let in_two_commas = b",,";
    let in_quoted_pair = b"\"\",\"\"";
    let in_quoted_comma_eof = b"\"a\",";
    let want_a = b"a";
    let want_b = b"b";
    let want_empty = b"";
    let want_blank = b"";
    let want_none = b"";
    if !two_fields(&in_crlf_records, &want_a, true, &want_b) { return 201; }
    if !one_field(&in_crlf, &want_empty) { return 202; }
    if !two_fields(&in_crlf_crlf, &want_empty, true, &want_blank) { return 203; }
    if !two_fields(&in_comma_crlf, &want_a, false, &want_empty) { return 204; }
    let mut more = open(&in_comma_crlf_more);
    if !expect_field(&mut more, &want_a, false) { return 205; }
    if !expect_field(&mut more, &want_empty, true) { return 206; }
    if !expect_field(&mut more, &want_b, true) { return 207; }
    if !ended(&mut more) { return 208; }
    if !two_fields(&in_quoted_comma_crlf, &want_a, false, &want_empty) { return 209; }
    if !one_field(&in_quoted_crlf, &want_a) { return 210; }
    let mut commas = open(&in_two_commas);
    if !expect_field(&mut commas, &want_empty, false) { return 211; }
    if !expect_field(&mut commas, &want_blank, false) { return 212; }
    if !expect_field(&mut commas, &want_none, true) { return 213; }
    if !ended(&mut commas) { return 214; }
    if !two_fields(&in_quoted_pair, &want_empty, false, &want_blank) { return 215; }
    if !two_fields(&in_quoted_comma_eof, &want_a, false, &want_empty) { return 216; }
    return 0;
}

// Quoted payload keeps comma, LF, CRLF, bare CR, NUL, backslash and bytes
// that are not UTF-8; unquoted fields keep NUL/non-UTF-8 bytes too; spaces
// are never trimmed and multiple doubled quotes each decode to one quote.
fn test_quoted_payload_bytes() -> i32 {
    let in_line_endings = b"\"a,b\nc\r\nd\re\"";
    let in_binary = b"\"\0\\\xFF\xC0\",x";
    let in_unquoted_binary = b"a\\b,\0\xFF";
    let in_escapes = b"\"x\"\"y\"\"z\"";
    let in_spaces = b"\" a \", b ";
    let in_bare_cr = b"\"\r\",1";
    let want_line_endings = b"a,b\nc\r\nd\re";
    let want_binary = b"\0\\\xFF\xC0";
    let want_x = b"x";
    let want_backslash = b"a\\b";
    let want_nul_ff = b"\0\xFF";
    let want_escapes = b"x\"y\"z";
    let want_spaced_a = b" a ";
    let want_spaced_b = b" b ";
    let want_cr = b"\r";
    let want_one = b"1";
    if !one_field(&in_line_endings, &want_line_endings) { return 301; }
    if !two_fields(&in_binary, &want_binary, false, &want_x) { return 302; }
    if !two_fields(&in_unquoted_binary, &want_backslash, false, &want_nul_ff) { return 303; }
    if !one_field(&in_escapes, &want_escapes) { return 304; }
    if !two_fields(&in_spaces, &want_spaced_a, false, &want_spaced_b) { return 305; }
    if !two_fields(&in_bare_cr, &want_cr, false, &want_one) { return 306; }
    return 0;
}

// Runs of 1 through 5 quote bytes as a whole document, plus runs that open
// and close a quoted field around content.
fn test_quote_runs_one_to_five() -> i32 {
    let in_one = b"\"";
    let in_two = b"\"\"";
    let in_three = b"\"\"\"";
    let in_four = b"\"\"\"\"";
    let in_five = b"\"\"\"\"\"";
    let in_wrapped = b"\"\"\"a\"\"\"";
    let in_trailing = b"\"a\"\"\"\"\"";
    let want_empty = b"";
    let want_quote = b"\"";
    let want_wrapped = b"\"a\"";
    let want_trailing = b"a\"\"";
    if !malformed_at(&in_one, 0) { return 401; }
    if !one_field(&in_two, &want_empty) { return 402; }
    if !malformed_at(&in_three, 0) { return 403; }
    if !one_field(&in_four, &want_quote) { return 404; }
    if !malformed_at(&in_five, 0) { return 405; }
    if !one_field(&in_wrapped, &want_wrapped) { return 406; }
    if !one_field(&in_trailing, &want_trailing) { return 407; }
    return 0;
}

// Exact zero-based offsets from the README rules; every error is sticky and
// leaves no current field. The separator is validated before a field is
// reported, so `a\rX` never returns `a`.
fn test_malformed_offsets_are_sticky() -> i32 {
    let in_quote_inside = b"ab\"c";
    let in_after_close = b"\"a\"x";
    let in_bare_cr = b"a\rX";
    let in_late_open = b"x,\"a";
    let in_cr_eof = b"a\r";
    let in_cr_only = b"\r";
    let in_comma_cr = b",\r";
    let in_close_cr_eof = b"\"a\"\r";
    let in_close_cr_x = b"\"a\"\rX";
    let in_close_space = b"\"a\" ,b";
    let in_second_quote = b"a,b\"c";
    let in_close_letter = b"\"a\"b";
    let in_escaped_then_letter = b"\"\"\"\"a";
    let in_open_pair_eof = b"\"a\"\"";
    let in_late_record = b"a,1\nb,\"2";
    let want_x = b"x";
    let want_empty = b"";
    let want_a = b"a";
    let want_one = b"1";
    let want_b = b"b";
    if !malformed_at(&in_quote_inside, 2) { return 501; }
    if !malformed_at(&in_after_close, 3) { return 502; }
    if !malformed_at(&in_bare_cr, 1) { return 503; }
    if !malformed_after(&in_late_open, &want_x, false, 2) { return 504; }
    if !malformed_at(&in_cr_eof, 1) { return 505; }
    if !malformed_at(&in_cr_only, 0) { return 506; }
    if !malformed_after(&in_comma_cr, &want_empty, false, 1) { return 507; }
    if !malformed_at(&in_close_cr_eof, 3) { return 508; }
    if !malformed_at(&in_close_cr_x, 3) { return 509; }
    if !malformed_at(&in_close_space, 3) { return 510; }
    if !malformed_after(&in_second_quote, &want_a, false, 3) { return 511; }
    if !malformed_at(&in_close_letter, 3) { return 512; }
    if !malformed_at(&in_escaped_then_letter, 4) { return 513; }
    if !malformed_at(&in_open_pair_eof, 0) { return 514; }
    let mut late = open(&in_late_record);
    if !expect_field(&mut late, &want_a, false) { return 515; }
    if !expect_field(&mut late, &want_one, true) { return 516; }
    if !expect_field(&mut late, &want_b, false) { return 517; }
    if !stuck(&mut late, 6) { return 518; }
    return 0;
}

// Empty, undersized, exact and oversized destinations. Refusals change no
// byte; a successful copy writes exactly the decoded prefix.
fn test_copy_destinations() -> i32 {
    let in_plain = b"abc,de";
    let in_doubled = b"\"a\"\"b\",";
    let want_abc = b"abc";
    let want_de = b"de";
    let want_a_quote_b = b"a\"b";
    let mut reader = open(&in_plain);
    if !expect_info(&mut reader, 3, false) { return 601; }
    let mut zero_room = b"";
    if !refused(&reader, &mut zero_room, 3) { return 602; }
    let mut small = b"\x11\x22";
    let small_before = b"\x11\x22";
    if !refused(&reader, &mut small, 3) { return 603; }
    if !same(&small, &small_before) { return 604; }
    if !refused(&reader, &mut small, 3) { return 605; }
    if !same(&small, &small_before) { return 606; }
    let mut exact = b"\x33\x44\x55";
    if !written(&reader, &mut exact, 3) { return 607; }
    if !same(&exact, &want_abc) { return 608; }
    let mut large = b"\x66\x66\x66\x66\x66";
    let large_after = b"abc\x66\x66";
    if !written(&reader, &mut large, 3) { return 609; }
    if !same(&large, &large_after) { return 610; }
    if !copies(&reader, &want_abc) { return 611; }
    if !expect_field(&mut reader, &want_de, true) { return 612; }
    if !ended(&mut reader) { return 613; }
    let mut quoted = open(&in_doubled);
    if !expect_info(&mut quoted, 3, false) { return 614; }
    let mut short = b"\x01\x02";
    let short_before = b"\x01\x02";
    if !refused(&quoted, &mut short, 3) { return 615; }
    if !same(&short, &short_before) { return 616; }
    let mut fit = b"\x09\x09\x09";
    if !written(&quoted, &mut fit, 3) { return 617; }
    if !same(&fit, &want_a_quote_b) { return 618; }
    if !expect_info(&mut quoted, 0, true) { return 619; }
    let mut no_room = b"";
    if !written(&quoted, &mut no_room, 0) { return 620; }
    let mut spare = b"\x07\x08";
    let spare_before = b"\x07\x08";
    if !written(&quoted, &mut spare, 0) { return 621; }
    if !same(&spare, &spare_before) { return 622; }
    if !ended(&mut quoted) { return 623; }
    return 0;
}

// Current-field lifecycle: none before the first field, repeated copies,
// advancing without copying, a final EOF field copyable until the next call,
// and invalidation by End and by Malformed.
fn test_current_field_lifecycle() -> i32 {
    let in_empty = b"";
    let in_three = b"p,q,r";
    let in_final = b"x,yz";
    let in_broken = b"a,\"b";
    let want_r = b"r";
    let want_x = b"x";
    let want_yz = b"yz";
    let want_a = b"a";
    let mut nothing = open(&in_empty);
    if !no_current(&nothing) { return 701; }
    if !expect_end(&mut nothing) { return 702; }
    if !no_current(&nothing) { return 703; }
    let mut three = open(&in_three);
    if !no_current(&three) { return 704; }
    if !expect_info(&mut three, 1, false) { return 705; }
    if !expect_info(&mut three, 1, false) { return 706; }
    if !expect_field(&mut three, &want_r, true) { return 707; }
    if !copies(&three, &want_r) { return 708; }
    if !ended(&mut three) { return 709; }
    let mut last = open(&in_final);
    if !expect_field(&mut last, &want_x, false) { return 710; }
    if !expect_field(&mut last, &want_yz, true) { return 711; }
    if !copies(&last, &want_yz) { return 712; }
    if !copies(&last, &want_yz) { return 713; }
    if !ended(&mut last) { return 714; }
    let mut broken = open(&in_broken);
    if !expect_field(&mut broken, &want_a, false) { return 715; }
    if !copies(&broken, &want_a) { return 716; }
    if !stuck(&mut broken, 2) { return 717; }
    return 0;
}

// FieldInfo is informational: changing or retaining it cannot select another
// range or change the size copy_current_field requires.
fn test_metadata_cannot_select_or_size() -> i32 {
    let in_plain = b"abc,de";
    let want_abc = b"abc";
    let want_de = b"de";
    let mut reader = open(&in_plain);
    let first = next_field(&mut reader);
    match first {
        Next::Field(info) => {
            let mut kept = info;
            if kept.decoded_len != 3 || kept.record_end { return 801; }
            forge(&mut kept, 0, true);
            if kept.decoded_len != 0 || !kept.record_end { return 802; }
            let mut tiny = b"\x77\x77";
            if !refused(&reader, &mut tiny, 3) { return 803; }
            if !copies(&reader, &want_abc) { return 804; }
            forge(&mut kept, 2, true);
            if !copies(&reader, &want_abc) { return 805; }
            if !expect_info(&mut reader, 2, true) { return 806; }
            forge(&mut kept, 3, false);
            let mut one = b"\x77";
            if !refused(&reader, &mut one, 2) { return 807; }
            if !copies(&reader, &want_de) { return 808; }
            forge(&mut kept, 64, false);
            if !copies(&reader, &want_de) { return 809; }
            if !ended(&mut reader) { return 810; }
            if kept.decoded_len != 64 || kept.record_end { return 811; }
            return 0;
        },
        Next::End => { return 812; },
        Next::Malformed(offset) => { return 813; },
    }
}

// Shared oxcsvsum helper: handoff success rows and small extra examples.
fn test_summary_fixed_reports() -> i32 {
    let in_empty = b"";
    let in_single = b"a,1";
    let in_two = b"a,1\nb,-2\n";
    let in_quoted = b"\"a,b\",\"+00012\"\r\nx,-2";
    let in_label_lf = b"\"a\nb\",3\n";
    let in_extremes = b"\"a\"\"b\",-2147483648\nx,2147483647";
    let in_empty_label = b",0";
    let in_zeroes = b"a,0000000000000000000001";
    let in_unicode = b"\"caf\xC3\xA9, \xE6\x9D\xB1\xE4\xBA\xAC\",7\n\xE2\x82\xAC,-3";
    let in_nul_label = b"\0,5\n\"\0\",-1";
    let in_quoted_negative = b"a,\"-5\"";
    let in_minimum = b"a,-2147483648";
    let want_zero = b"0 0\n";
    let want_one_one = b"1 1\n";
    let want_two_minus_one = b"2 -1\n";
    let want_two_ten = b"2 10\n";
    let want_one_three = b"1 3\n";
    let want_extremes = b"2 -1\n";
    let want_one_zero = b"1 0\n";
    let want_zeroes = b"1 1\n";
    let want_unicode = b"2 4\n";
    let want_nul = b"2 4\n";
    let want_negative = b"1 -5\n";
    let want_minimum = b"1 -2147483648\n";
    if !summary_reports(&in_empty, &want_zero) { return 901; }
    if !summary_reports(&in_single, &want_one_one) { return 902; }
    if !summary_reports(&in_two, &want_two_minus_one) { return 903; }
    if !summary_reports(&in_quoted, &want_two_ten) { return 904; }
    if !summary_reports(&in_label_lf, &want_one_three) { return 905; }
    if !summary_reports(&in_extremes, &want_extremes) { return 906; }
    if !summary_reports(&in_empty_label, &want_one_zero) { return 907; }
    if !summary_reports(&in_zeroes, &want_zeroes) { return 908; }
    if !summary_reports(&in_unicode, &want_unicode) { return 909; }
    if !summary_reports(&in_nul_label, &want_nul) { return 910; }
    if !summary_reports(&in_quoted_negative, &want_negative) { return 911; }
    if !summary_reports(&in_minimum, &want_minimum) { return 912; }
    return 0;
}

// Rejections return the fixed status and leave nothing to report, including
// intermediate overflow that a later value would bring back into range.
fn test_summary_rejections_emit_no_report() -> i32 {
    let in_blank = b"\n";
    let in_label_only = b"a";
    let in_three_fields = b"a,1,";
    let in_empty_value = b"a,";
    let in_space = b"a, 1";
    let in_too_large = b"a,2147483648";
    let in_late_quote = b"a,1\nb,\"2";
    let in_invalid_utf8 = b"a\xFF,1";
    let in_truncated_utf8 = b"\xE2\x82,1";
    let in_decoded_quote = b"a,\"1\"\"2\"";
    let in_header = b"name,value\nb,2";
    let in_late_csv = b"a,1\nb,2\nc\"d,3";
    let in_sum_high = b"a,2147483647\nb,1";
    let in_sum_low = b"a,-2147483648\nb,-1";
    let in_sum_back = b"a,2147483647\nb,1\nc,-1";
    let in_low_back = b"a,-2147483648\nb,-1\nc,1";
    if !summary_rejects(&in_blank, 65) { return 1001; }
    if !summary_rejects(&in_label_only, 65) { return 1002; }
    if !summary_rejects(&in_three_fields, 65) { return 1003; }
    if !summary_rejects(&in_empty_value, 65) { return 1004; }
    if !summary_rejects(&in_space, 65) { return 1005; }
    if !summary_rejects(&in_too_large, 65) { return 1006; }
    if !summary_rejects(&in_late_quote, 65) { return 1007; }
    if !summary_rejects(&in_invalid_utf8, 65) { return 1008; }
    if !summary_rejects(&in_truncated_utf8, 65) { return 1009; }
    if !summary_rejects(&in_decoded_quote, 65) { return 1010; }
    if !summary_rejects(&in_header, 65) { return 1011; }
    if !summary_rejects(&in_late_csv, 65) { return 1012; }
    if !summary_rejects(&in_sum_high, 70) { return 1013; }
    if !summary_rejects(&in_sum_low, 70) { return 1014; }
    if !summary_rejects(&in_sum_back, 70) { return 1015; }
    if !summary_rejects(&in_low_back, 70) { return 1016; }
    return 0;
}

// render refuses before summarize completes; a second summarize is a
// lifecycle error (70) that keeps the completed report.
fn test_summary_lifecycle() -> i32 {
    let in_first = b"a,4";
    let in_second = b"b,5";
    let want_first = b"1 4\n";
    let mut summary = new_summary();
    let mut early = b"\x5A\x5A\x5A\x5A";
    if render(&summary, &mut early) != -1 { return 1101; }
    if !all_are(&early, 90) { return 1102; }
    let mut first = new_bytes();
    if !grew(try_extend(&mut first, &in_first)) { return 1103; }
    if summarize(first, &mut summary) != 0 { return 1104; }
    let mut second = new_bytes();
    if !grew(try_extend(&mut second, &in_second)) { return 1105; }
    if summarize(second, &mut summary) != 70 { return 1106; }
    let mut output = b"\x5A\x5A\x5A\x5A\x5A\x5A";
    let length = render(&summary, &mut output);
    if length != 4 { return 1107; }
    if !same(&output[0..4], &want_first) { return 1108; }
    if !all_are(&output[4..6], 90) { return 1109; }
    return 0;
}
