use decimal::ParseResult;
use decimal::FormatResult;
use decimal::parse_i32;
use decimal::format_i32;

fn byte(value: i32) -> u8 {
    return value.to_u8_checked();
}

fn same_bytes(actual: &[u8], expected: &[u8]) -> bool {
    if actual.len() != expected.len() { return false; }
    let mut offset = 0;
    while offset < actual.len() {
        if actual[offset] != expected[offset] { return false; }
        offset = offset + 1;
    }
    return true;
}

// The written prefix must equal `prefix`; every later byte must still be `fill`.
fn prefix_then_fill(buffer: &[u8], prefix: &[u8], fill: i32) -> bool {
    if prefix.len() > buffer.len() { return false; }
    let mut offset = 0;
    while offset < prefix.len() {
        if buffer[offset] != prefix[offset] { return false; }
        offset = offset + 1;
    }
    while offset < buffer.len() {
        let current = buffer[offset];
        if current.to_i32() != fill { return false; }
        offset = offset + 1;
    }
    return true;
}

fn all_bytes(buffer: &[u8], fill: i32) -> bool {
    let mut offset = 0;
    while offset < buffer.len() {
        let current = buffer[offset];
        if current.to_i32() != fill { return false; }
        offset = offset + 1;
    }
    return true;
}

fn value_is(input: &[u8], expected: i32) -> bool {
    let result = parse_i32(&*input);
    match result {
        ParseResult::Value(actual) => { return actual == expected; },
        ParseResult::Empty => { return false; },
        ParseResult::SignOnly => { return false; },
        ParseResult::InvalidDigit(index) => { return false; },
        ParseResult::Overflow(index) => { return false; },
    }
}

fn empty_is(input: &[u8]) -> bool {
    let result = parse_i32(&*input);
    match result {
        ParseResult::Value(actual) => { return false; },
        ParseResult::Empty => { return true; },
        ParseResult::SignOnly => { return false; },
        ParseResult::InvalidDigit(index) => { return false; },
        ParseResult::Overflow(index) => { return false; },
    }
}

fn sign_only_is(input: &[u8]) -> bool {
    let result = parse_i32(&*input);
    match result {
        ParseResult::Value(actual) => { return false; },
        ParseResult::Empty => { return false; },
        ParseResult::SignOnly => { return true; },
        ParseResult::InvalidDigit(index) => { return false; },
        ParseResult::Overflow(index) => { return false; },
    }
}

fn invalid_at(input: &[u8], expected: i32) -> bool {
    let result = parse_i32(&*input);
    match result {
        ParseResult::Value(actual) => { return false; },
        ParseResult::Empty => { return false; },
        ParseResult::SignOnly => { return false; },
        ParseResult::InvalidDigit(index) => { return index == expected; },
        ParseResult::Overflow(index) => { return false; },
    }
}

fn overflow_at(input: &[u8], expected: i32) -> bool {
    let result = parse_i32(&*input);
    match result {
        ParseResult::Value(actual) => { return false; },
        ParseResult::Empty => { return false; },
        ParseResult::SignOnly => { return false; },
        ParseResult::InvalidDigit(index) => { return false; },
        ParseResult::Overflow(index) => { return index == expected; },
    }
}

fn written(result: FormatResult, expected: i32) -> bool {
    match result {
        FormatResult::Written(count) => { return count == expected; },
        FormatResult::BufferTooSmall(required) => { return false; },
    }
}

fn too_small(result: FormatResult, expected: i32) -> bool {
    match result {
        FormatResult::Written(count) => { return false; },
        FormatResult::BufferTooSmall(required) => { return required == expected; },
    }
}

// Formats into twelve 0xAA sentinel bytes: the prefix is exact, the rest is kept.
fn formats_in_twelve(value: i32, expected: &[u8]) -> bool {
    let mut buffer = b"\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA";
    let result = format_i32(value, &mut buffer);
    if !written(result, expected.len()) { return false; }
    return prefix_then_fill(&buffer, &*expected, 170);
}

fn test_parse_signed_zero_and_units() -> i32 {
    let zero = b"0";
    let minus_zero = b"-0";
    let plus_zero = b"+0";
    let one = b"1";
    let minus_one = b"-1";
    let plus_one = b"+1";
    if !value_is(&zero, 0) { return 101; }
    if !value_is(&minus_zero, 0) { return 102; }
    if !value_is(&plus_zero, 0) { return 103; }
    if !value_is(&one, 1) { return 104; }
    if !value_is(&minus_one, -1) { return 105; }
    if !value_is(&plus_one, 1) { return 106; }
    return 0;
}

fn test_parse_digit_count_boundaries() -> i32 {
    let nine = b"9";
    let ten = b"10";
    let ninety_nine = b"99";
    let hundred = b"100";
    let minus_nine = b"-9";
    let minus_ten = b"-10";
    let minus_ninety_nine = b"-99";
    let minus_hundred = b"-100";
    let nine_digits = b"999999999";
    let ten_digits = b"1000000000";
    let minus_ten_digits = b"-1000000000";
    if !value_is(&nine, 9) { return 201; }
    if !value_is(&ten, 10) { return 202; }
    if !value_is(&ninety_nine, 99) { return 203; }
    if !value_is(&hundred, 100) { return 204; }
    if !value_is(&minus_nine, -9) { return 205; }
    if !value_is(&minus_ten, -10) { return 206; }
    if !value_is(&minus_ninety_nine, -99) { return 207; }
    if !value_is(&minus_hundred, -100) { return 208; }
    if !value_is(&nine_digits, 999999999) { return 209; }
    if !value_is(&ten_digits, 1000000000) { return 210; }
    if !value_is(&minus_ten_digits, -1000000000) { return 211; }
    return 0;
}

fn test_parse_extremes_and_explicit_plus() -> i32 {
    let max = b"2147483647";
    let plus_max = b"+2147483647";
    let min = b"-2147483648";
    let below_max = b"2147483646";
    let above_min = b"-2147483647";
    if !value_is(&max, 2147483647) { return 301; }
    if !value_is(&plus_max, 2147483647) { return 302; }
    if !value_is(&min, -2147483647 - 1) { return 303; }
    if !value_is(&below_max, 2147483646) { return 304; }
    if !value_is(&above_min, -2147483647) { return 305; }
    return 0;
}

fn test_parse_leading_zero_boundaries() -> i32 {
    let double_zero = b"00";
    let seven = b"007";
    let minus_seven = b"-007";
    let plus_ten = b"+010";
    let ninety_nine = b"0099";
    let minus_hundred = b"-0100";
    let ten_zeros = b"0000000000";
    let minus_zeros = b"-0000000000";
    let padded_max = b"00000000002147483647";
    let padded_min = b"-00000000002147483648";
    let plus_padded_max = b"+0002147483647";
    if !value_is(&double_zero, 0) { return 401; }
    if !value_is(&seven, 7) { return 402; }
    if !value_is(&minus_seven, -7) { return 403; }
    if !value_is(&plus_ten, 10) { return 404; }
    if !value_is(&ninety_nine, 99) { return 405; }
    if !value_is(&minus_hundred, -100) { return 406; }
    if !value_is(&ten_zeros, 0) { return 407; }
    if !value_is(&minus_zeros, 0) { return 408; }
    if !value_is(&padded_max, 2147483647) { return 409; }
    if !value_is(&padded_min, -2147483647 - 1) { return 410; }
    if !value_is(&plus_padded_max, 2147483647) { return 411; }
    return 0;
}

fn test_parse_empty_and_sign_only() -> i32 {
    let empty = b"";
    let minus = b"-";
    let plus = b"+";
    if !empty_is(&empty) { return 501; }
    if !sign_only_is(&minus) { return 502; }
    if !sign_only_is(&plus) { return 503; }
    return 0;
}

fn test_parse_extra_signs_rejected() -> i32 {
    let double_minus = b"--1";
    let plus_minus = b"+-1";
    let minus_plus = b"-+1";
    let double_plus = b"++1";
    let trailing_minus = b"1-";
    let trailing_plus = b"12+";
    let wrapped = b"-1-";
    if !invalid_at(&double_minus, 1) { return 601; }
    if !invalid_at(&plus_minus, 1) { return 602; }
    if !invalid_at(&minus_plus, 1) { return 603; }
    if !invalid_at(&double_plus, 1) { return 604; }
    if !invalid_at(&trailing_minus, 1) { return 605; }
    if !invalid_at(&trailing_plus, 2) { return 606; }
    if !invalid_at(&wrapped, 2) { return 607; }
    return 0;
}

fn test_parse_whitespace_rejected() -> i32 {
    let space = b" ";
    let leading = b" 1";
    let trailing = b"1 ";
    let after_sign = b"- 1";
    let tab = b"\t5";
    let newline = b"12\n";
    let carriage = b"7\r";
    if !invalid_at(&space, 0) { return 701; }
    if !invalid_at(&leading, 0) { return 702; }
    if !invalid_at(&trailing, 1) { return 703; }
    if !invalid_at(&after_sign, 1) { return 704; }
    if !invalid_at(&tab, 0) { return 705; }
    if !invalid_at(&newline, 2) { return 706; }
    if !invalid_at(&carriage, 1) { return 707; }
    return 0;
}

fn test_parse_underscore_point_and_comma_rejected() -> i32 {
    let grouped = b"1_000";
    let leading_underscore = b"_1";
    let trailing_underscore = b"1_";
    let fraction = b"1.5";
    let bare_fraction = b".5";
    let trailing_point = b"1.";
    let signed_fraction = b"-.5";
    let comma = b"1,000";
    if !invalid_at(&grouped, 1) { return 801; }
    if !invalid_at(&leading_underscore, 0) { return 802; }
    if !invalid_at(&trailing_underscore, 1) { return 803; }
    if !invalid_at(&fraction, 1) { return 804; }
    if !invalid_at(&bare_fraction, 0) { return 805; }
    if !invalid_at(&trailing_point, 1) { return 806; }
    if !invalid_at(&signed_fraction, 1) { return 807; }
    if !invalid_at(&comma, 1) { return 808; }
    return 0;
}

fn test_parse_nul_and_high_bytes_rejected() -> i32 {
    let nul = [byte(0)];
    let digit_nul = [byte(49), byte(0)];
    let sign_nul = [byte(45), byte(0)];
    let inner_nul = [byte(49), byte(50), byte(0), byte(51)];
    let high = [byte(255)];
    let digit_high = [byte(55), byte(255)];
    let sign_high = [byte(43), byte(255), byte(49)];
    let high_bit_zero = [byte(176)];
    let high_bit_nine = [byte(49), byte(185)];
    if !invalid_at(&nul, 0) { return 901; }
    if !invalid_at(&digit_nul, 1) { return 902; }
    if !invalid_at(&sign_nul, 1) { return 903; }
    if !invalid_at(&inner_nul, 2) { return 904; }
    if !invalid_at(&high, 0) { return 905; }
    if !invalid_at(&digit_high, 1) { return 906; }
    if !invalid_at(&sign_high, 1) { return 907; }
    if !invalid_at(&high_bit_zero, 0) { return 908; }
    if !invalid_at(&high_bit_nine, 1) { return 909; }
    return 0;
}

fn test_parse_digit_neighbors_and_letters_rejected() -> i32 {
    let slash = b"/";
    let colon = b":";
    let digit_slash = b"1/";
    let digit_colon = b"9:";
    let letter = b"a";
    let hex_prefix = b"0x10";
    let exponent = b"1e3";
    let minus_letter = b"-x";
    if !invalid_at(&slash, 0) { return 1001; }
    if !invalid_at(&colon, 0) { return 1002; }
    if !invalid_at(&digit_slash, 1) { return 1003; }
    if !invalid_at(&digit_colon, 1) { return 1004; }
    if !invalid_at(&letter, 0) { return 1005; }
    if !invalid_at(&hex_prefix, 1) { return 1006; }
    if !invalid_at(&exponent, 1) { return 1007; }
    if !invalid_at(&minus_letter, 1) { return 1008; }
    return 0;
}

fn test_parse_positive_overflow_index() -> i32 {
    let max_plus_one = b"2147483648";
    let plus_max_plus_one = b"+2147483648";
    let max_plus_three = b"2147483650";
    let three_billion = b"3000000000";
    let ten_nines = b"9999999999";
    let max_times_ten = b"21474836470";
    let padded = b"0002147483648";
    if !overflow_at(&max_plus_one, 9) { return 1101; }
    if !overflow_at(&plus_max_plus_one, 10) { return 1102; }
    if !overflow_at(&max_plus_three, 9) { return 1103; }
    if !overflow_at(&three_billion, 9) { return 1104; }
    if !overflow_at(&ten_nines, 9) { return 1105; }
    if !overflow_at(&max_times_ten, 10) { return 1106; }
    if !overflow_at(&padded, 12) { return 1107; }
    return 0;
}

fn test_parse_negative_overflow_index() -> i32 {
    let min_minus_one = b"-2147483649";
    let min_minus_two = b"-2147483650";
    let minus_three_billion = b"-3000000000";
    let minus_ten_nines = b"-9999999999";
    let min_times_ten = b"-21474836480";
    let padded = b"-0002147483649";
    if !overflow_at(&min_minus_one, 10) { return 1201; }
    if !overflow_at(&min_minus_two, 10) { return 1202; }
    if !overflow_at(&minus_three_billion, 10) { return 1203; }
    if !overflow_at(&minus_ten_nines, 10) { return 1204; }
    if !overflow_at(&min_times_ten, 11) { return 1205; }
    if !overflow_at(&padded, 13) { return 1206; }
    return 0;
}

fn test_parse_long_overflow_strings() -> i32 {
    let twenty = b"12345678901234567890";
    let minus_twenty = b"-12345678901234567890";
    let nines = b"99999999999999999999999999999999";
    let power = b"100000000000000000000";
    let minus_power = b"-100000000000000000000";
    if !overflow_at(&twenty, 10) { return 1301; }
    if !overflow_at(&minus_twenty, 11) { return 1302; }
    if !overflow_at(&nines, 9) { return 1303; }
    if !overflow_at(&power, 10) { return 1304; }
    if !overflow_at(&minus_power, 11) { return 1305; }
    return 0;
}

fn test_parse_first_failure_left_to_right() -> i32 {
    let overflow_then_letter = b"2147483648x";
    let overflow_then_space = b"-2147483649 ";
    let overflow_then_underscore = b"21474836480_";
    let long_overflow_then_letter = b"99999999999x";
    let letter_then_overflow = b"1x99999999999";
    let signed_letter_then_overflow = b"-2x147483649";
    let max_then_letter = b"2147483647x";
    let min_then_letter = b"-2147483648x";
    if !overflow_at(&overflow_then_letter, 9) { return 1401; }
    if !overflow_at(&overflow_then_space, 10) { return 1402; }
    if !overflow_at(&overflow_then_underscore, 9) { return 1403; }
    if !overflow_at(&long_overflow_then_letter, 9) { return 1404; }
    if !invalid_at(&letter_then_overflow, 1) { return 1405; }
    if !invalid_at(&signed_letter_then_overflow, 2) { return 1406; }
    if !invalid_at(&max_then_letter, 10) { return 1407; }
    if !invalid_at(&min_then_letter, 11) { return 1408; }
    return 0;
}

fn test_parse_leaves_input_unchanged() -> i32 {
    let overflow = b"2147483648x";
    let overflow_bytes = [byte(50), byte(49), byte(52), byte(55), byte(52), byte(56), byte(51), byte(54), byte(52), byte(56), byte(120)];
    if !overflow_at(&overflow, 9) { return 1501; }
    if !same_bytes(&overflow, &overflow_bytes) { return 1502; }
    let min = b"-2147483648";
    let min_bytes = [byte(45), byte(50), byte(49), byte(52), byte(55), byte(52), byte(56), byte(51), byte(54), byte(52), byte(56)];
    if !value_is(&min, -2147483647 - 1) { return 1503; }
    if !same_bytes(&min, &min_bytes) { return 1504; }
    let high = [byte(49), byte(255)];
    let high_bytes = [byte(49), byte(255)];
    if !invalid_at(&high, 1) { return 1505; }
    if !same_bytes(&high, &high_bytes) { return 1506; }
    return 0;
}

fn test_format_small_values_exact() -> i32 {
    let zero = [byte(48)];
    let one = [byte(49)];
    let minus_one = [byte(45), byte(49)];
    let nine = [byte(57)];
    let ten = [byte(49), byte(48)];
    let minus_nine = [byte(45), byte(57)];
    let minus_ten = [byte(45), byte(49), byte(48)];
    let ninety_nine = [byte(57), byte(57)];
    let hundred = [byte(49), byte(48), byte(48)];
    let minus_ninety_nine = [byte(45), byte(57), byte(57)];
    let minus_hundred = [byte(45), byte(49), byte(48), byte(48)];
    if !formats_in_twelve(0, &zero) { return 1601; }
    if !formats_in_twelve(1, &one) { return 1602; }
    if !formats_in_twelve(-1, &minus_one) { return 1603; }
    if !formats_in_twelve(9, &nine) { return 1604; }
    if !formats_in_twelve(10, &ten) { return 1605; }
    if !formats_in_twelve(-9, &minus_nine) { return 1606; }
    if !formats_in_twelve(-10, &minus_ten) { return 1607; }
    if !formats_in_twelve(99, &ninety_nine) { return 1608; }
    if !formats_in_twelve(100, &hundred) { return 1609; }
    if !formats_in_twelve(-99, &minus_ninety_nine) { return 1610; }
    if !formats_in_twelve(-100, &minus_hundred) { return 1611; }
    return 0;
}

fn test_format_extremes_exact() -> i32 {
    let max = [byte(50), byte(49), byte(52), byte(55), byte(52), byte(56), byte(51), byte(54), byte(52), byte(55)];
    let min = [byte(45), byte(50), byte(49), byte(52), byte(55), byte(52), byte(56), byte(51), byte(54), byte(52), byte(56)];
    let above_min = [byte(45), byte(50), byte(49), byte(52), byte(55), byte(52), byte(56), byte(51), byte(54), byte(52), byte(55)];
    let billion = [byte(49), byte(48), byte(48), byte(48), byte(48), byte(48), byte(48), byte(48), byte(48), byte(48)];
    let minus_billion = [byte(45), byte(49), byte(48), byte(48), byte(48), byte(48), byte(48), byte(48), byte(48), byte(48), byte(48)];
    if max.len() != 10 || min.len() != 11 { return 1701; }
    if !formats_in_twelve(2147483647, &max) { return 1702; }
    if !formats_in_twelve(-2147483647 - 1, &min) { return 1703; }
    if !formats_in_twelve(-2147483647, &above_min) { return 1704; }
    if !formats_in_twelve(1000000000, &billion) { return 1705; }
    if !formats_in_twelve(-1000000000, &minus_billion) { return 1706; }
    return 0;
}

fn test_format_zero_length_buffer() -> i32 {
    let mut empty: [u8; 0] = [];
    let zero_result = format_i32(0, &mut empty);
    if !too_small(zero_result, 1) { return 1801; }
    let minus_one_result = format_i32(-1, &mut empty);
    if !too_small(minus_one_result, 2) { return 1802; }
    let max_result = format_i32(2147483647, &mut empty);
    if !too_small(max_result, 10) { return 1803; }
    let min_result = format_i32(-2147483647 - 1, &mut empty);
    if !too_small(min_result, 11) { return 1804; }
    if empty.len() != 0 { return 1805; }
    return 0;
}

fn test_format_one_short_keeps_whole_buffer() -> i32 {
    let mut one = b"\xAA";
    let minus_one_result = format_i32(-1, &mut one);
    if !too_small(minus_one_result, 2) { return 1901; }
    if !all_bytes(&one, 170) { return 1902; }
    let ten_result = format_i32(10, &mut one);
    if !too_small(ten_result, 2) { return 1903; }
    if !all_bytes(&one, 170) { return 1904; }
    let mut three = b"\xAA\xAA\xAA";
    let hundred_result = format_i32(-100, &mut three);
    if !too_small(hundred_result, 4) { return 1905; }
    if !all_bytes(&three, 170) { return 1906; }
    let mut nine = b"\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA";
    let max_result = format_i32(2147483647, &mut nine);
    if !too_small(max_result, 10) { return 1907; }
    if !all_bytes(&nine, 170) { return 1908; }
    let mut ten = b"\xC8\xC9\xCA\xCB\xCC\xCD\xCE\xCF\xD0\xD1";
    let ten_pattern = [byte(200), byte(201), byte(202), byte(203), byte(204), byte(205), byte(206), byte(207), byte(208), byte(209)];
    let min_result = format_i32(-2147483647 - 1, &mut ten);
    if !too_small(min_result, 11) { return 1909; }
    if !same_bytes(&ten, &ten_pattern) { return 1910; }
    let mut five = b"\xAA\xAA\xAA\xAA\xAA";
    let far_result = format_i32(-2147483647 - 1, &mut five);
    if !too_small(far_result, 11) { return 1911; }
    if !all_bytes(&five, 170) { return 1912; }
    return 0;
}

fn test_format_exact_buffer() -> i32 {
    let mut one = b"\xAA";
    let zero_expected = [byte(48)];
    let zero_result = format_i32(0, &mut one);
    if !written(zero_result, 1) { return 2001; }
    if !same_bytes(&one, &zero_expected) { return 2002; }
    let mut two = b"\xAA\xAA";
    let minus_one_expected = [byte(45), byte(49)];
    let minus_one_result = format_i32(-1, &mut two);
    if !written(minus_one_result, 2) { return 2003; }
    if !same_bytes(&two, &minus_one_expected) { return 2004; }
    let mut four = b"\xAA\xAA\xAA\xAA";
    let minus_hundred_expected = [byte(45), byte(49), byte(48), byte(48)];
    let minus_hundred_result = format_i32(-100, &mut four);
    if !written(minus_hundred_result, 4) { return 2005; }
    if !same_bytes(&four, &minus_hundred_expected) { return 2006; }
    let mut ten = b"\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA";
    let max_expected = [byte(50), byte(49), byte(52), byte(55), byte(52), byte(56), byte(51), byte(54), byte(52), byte(55)];
    let max_result = format_i32(2147483647, &mut ten);
    if !written(max_result, 10) { return 2007; }
    if !same_bytes(&ten, &max_expected) { return 2008; }
    let mut eleven = b"\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA";
    let min_expected = [byte(45), byte(50), byte(49), byte(52), byte(55), byte(52), byte(56), byte(51), byte(54), byte(52), byte(56)];
    let min_result = format_i32(-2147483647 - 1, &mut eleven);
    if !written(min_result, 11) { return 2009; }
    if !same_bytes(&eleven, &min_expected) { return 2010; }
    return 0;
}

fn test_format_oversized_keeps_suffix() -> i32 {
    let mut sixteen = b"\xC0\xC1\xC2\xC3\xC4\xC5\xC6\xC7\xC8\xC9\xCA\xCB\xCC\xCD\xCE\xCF";
    let min_then_pattern = [byte(45), byte(50), byte(49), byte(52), byte(55), byte(52), byte(56), byte(51), byte(54), byte(52), byte(56), byte(203), byte(204), byte(205), byte(206), byte(207)];
    let min_result = format_i32(-2147483647 - 1, &mut sixteen);
    if !written(min_result, 11) { return 2101; }
    if !same_bytes(&sixteen, &min_then_pattern) { return 2102; }
    let mut other = b"\xC0\xC1\xC2\xC3\xC4\xC5\xC6\xC7\xC8\xC9\xCA\xCB\xCC\xCD\xCE\xCF";
    let zero_then_pattern = [byte(48), byte(193), byte(194), byte(195), byte(196), byte(197), byte(198), byte(199), byte(200), byte(201), byte(202), byte(203), byte(204), byte(205), byte(206), byte(207)];
    let zero_result = format_i32(0, &mut other);
    if !written(zero_result, 1) { return 2103; }
    if !same_bytes(&other, &zero_then_pattern) { return 2104; }
    let mut eleven = b"\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA";
    let max_then_sentinel = [byte(50), byte(49), byte(52), byte(55), byte(52), byte(56), byte(51), byte(54), byte(52), byte(55), byte(170)];
    let max_result = format_i32(2147483647, &mut eleven);
    if !written(max_result, 10) { return 2105; }
    if !same_bytes(&eleven, &max_then_sentinel) { return 2106; }
    return 0;
}

fn test_round_trip_supplement() -> i32 {
    let mut min_buffer = b"\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA";
    let min_result = format_i32(-2147483647 - 1, &mut min_buffer);
    if !written(min_result, 11) { return 2201; }
    if !value_is(&min_buffer, -2147483647 - 1) { return 2202; }
    let mut max_buffer = b"\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA";
    let max_result = format_i32(2147483647, &mut max_buffer);
    if !written(max_result, 10) { return 2203; }
    if !value_is(&max_buffer, 2147483647) { return 2204; }
    let mut small_buffer = b"\xAA\xAA";
    let small_result = format_i32(-1, &mut small_buffer);
    if !written(small_result, 2) { return 2205; }
    if !value_is(&small_buffer, -1) { return 2206; }
    return 0;
}

fn main() -> i32 {
    let signed_zero = test_parse_signed_zero_and_units();
    if signed_zero != 0 { return signed_zero; }
    let digit_count = test_parse_digit_count_boundaries();
    if digit_count != 0 { return digit_count; }
    let extremes = test_parse_extremes_and_explicit_plus();
    if extremes != 0 { return extremes; }
    let leading_zero = test_parse_leading_zero_boundaries();
    if leading_zero != 0 { return leading_zero; }
    let empty_sign = test_parse_empty_and_sign_only();
    if empty_sign != 0 { return empty_sign; }
    let extra_signs = test_parse_extra_signs_rejected();
    if extra_signs != 0 { return extra_signs; }
    let whitespace = test_parse_whitespace_rejected();
    if whitespace != 0 { return whitespace; }
    let separators = test_parse_underscore_point_and_comma_rejected();
    if separators != 0 { return separators; }
    let nul_high = test_parse_nul_and_high_bytes_rejected();
    if nul_high != 0 { return nul_high; }
    let neighbors = test_parse_digit_neighbors_and_letters_rejected();
    if neighbors != 0 { return neighbors; }
    let positive_overflow = test_parse_positive_overflow_index();
    if positive_overflow != 0 { return positive_overflow; }
    let negative_overflow = test_parse_negative_overflow_index();
    if negative_overflow != 0 { return negative_overflow; }
    let long_overflow = test_parse_long_overflow_strings();
    if long_overflow != 0 { return long_overflow; }
    let first_failure = test_parse_first_failure_left_to_right();
    if first_failure != 0 { return first_failure; }
    let unchanged = test_parse_leaves_input_unchanged();
    if unchanged != 0 { return unchanged; }
    let small_format = test_format_small_values_exact();
    if small_format != 0 { return small_format; }
    let extreme_format = test_format_extremes_exact();
    if extreme_format != 0 { return extreme_format; }
    let zero_length = test_format_zero_length_buffer();
    if zero_length != 0 { return zero_length; }
    let one_short = test_format_one_short_keeps_whole_buffer();
    if one_short != 0 { return one_short; }
    let exact = test_format_exact_buffer();
    if exact != 0 { return exact; }
    let oversized = test_format_oversized_keeps_suffix();
    if oversized != 0 { return oversized; }
    let round_trip = test_round_trip_supplement();
    if round_trip != 0 { return round_trip; }
    return 42;
}
