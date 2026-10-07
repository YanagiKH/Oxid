// Exact ASCII keyword spellings from src/frontend/lexer.rs.
// Each table row stores three base-128 chunks (at most four ASCII bytes each)
// and size + 16 * token kind. All chunks are below 2^28; no hash is used.
// The caller preflights ASCII. Exact size and all three chunks prevent prefixes
// and near misses from matching. Literal and binding owners remain included in native inventory.
pub fn classify(codes: &[i32], start: i32, end: i32) -> i32 {
    let size = end - start;
    if size < 2 || size > 11 { return 2; }
    let first = chunk(&*codes, start, end);
    let second = chunk(&*codes, start + 4, end);
    let third = chunk(&*codes, start + 8, end);
    let first_column = [13166, 13542, 14322, 1800164, 1931749, 1850082, 1782516, 1800948, 1685490, 1880806, 1603428, 213596645, 245152485, 228325360, 230423397, 213629677, 222001260, 245266533, 232617580, 207401697, 251278572, 215512691, 230191602, 209450867, 230193763, 205323502, 205385961, 245149929, 243087733, 240745077, 222001263, 247183841, 213793381, 209450868, 230191602];
    let second_column = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 107, 101, 101, 111, 116, 104, 99, 116, 116, 12788, 14702, 14708, 13157, 14702, 222018277, 234355061];
    let third_column = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1782515];
    let metadata_column = [82, 274, 722, 115, 131, 147, 195, 211, 723, 723, 723, 308, 324, 724, 724, 724, 724, 724, 724, 245, 293, 341, 725, 725, 725, 725, 725, 725, 102, 230, 726, 726, 726, 264, 731];
    let mut row = 0;
    while row < 35 {
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
        value = value * 128 + codes[cursor];
        cursor = cursor + 1;
    }
    return value;
}
