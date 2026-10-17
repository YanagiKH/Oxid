pub fn make_first_column() -> [i32; 35] { return [13166, 13542, 14322, 1603428, 1685490, 1782516, 1800164, 1800948, 1850082, 1880806, 1931749, 205323502, 205385961, 207401697, 209450867, 209450868, 213596645, 213629677, 213793381, 215512691, 222001260, 222001263, 228325360, 230191602, 230191602, 230193763, 230423397, 232617580, 240745077, 243087733, 245149929, 245152485, 245266533, 247183841, 251278572]; }
pub fn make_second_column() -> [i32; 35] { return [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 99, 116, 107, 116, 222018277, 0, 0, 14702, 101, 0, 14708, 0, 111, 234355061, 104, 0, 0, 14702, 12788, 116, 0, 0, 13157, 101]; }
pub fn make_third_column() -> [i32; 35] { return [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1782515, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]; }
pub fn make_metadata_column() -> [i32; 35] { return [82, 274, 722, 723, 723, 195, 115, 211, 147, 723, 131, 725, 725, 245, 725, 264, 308, 724, 726, 341, 724, 726, 724, 725, 731, 725, 724, 724, 230, 102, 725, 324, 724, 726, 293]; }
// Exact ASCII keyword spellings from src/frontend/lexer.rs.
// Each table row stores three base-128 chunks (at most four ASCII bytes each)
// and size + 16 * token kind. All chunks are below 2^28; no hash is used.
// The caller preflights ASCII. Exact size and all three chunks prevent prefixes
// and near misses from matching. Literal and binding owners remain included in native inventory.
pub fn classify(codes: &[i32], start: i32, end: i32, first_column: &[i32], second_column: &[i32], third_column: &[i32], metadata_column: &[i32]) -> i32 {
    let size = end - start;
    if size < 2 || size > 11 { return 2; }
    let first = chunk(&*codes, start, end);
    let second = chunk(&*codes, start + 4, end);
    let third = chunk(&*codes, start + 8, end);
    let mut row = 0;
    let mut upper = 35;
    while row < upper {
        let middle = (row + upper) / 2;
        if first_column[middle] < first { row = middle + 1; }
        else { upper = middle; }
    }
    while row < 35 && first_column[row] == first {
        if metadata_column[row] % 16 == size && first_column[row] == first && second_column[row] == second && third_column[row] == third {
            return metadata_column[row] / 16;
        }
        row = row + 1;
    }
    return 2;
}

// Pack at most four original bytes exactly, including the shorter final chunk.
// A start beyond end gives zero and never reads the source.
fn chunk(codes: &[i32], start: i32, end: i32) -> i32 {
    let mut cursor = start;
    let mut value = 0;
    while cursor < end && cursor < start + 4 {
        value = value * 128 + crate::source_bytes::at(&*codes, cursor);
        cursor = cursor + 1;
    }
    return value;
}
