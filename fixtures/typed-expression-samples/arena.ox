pub struct Arena {
    pub kind: [i32; 15],
    pub value: [i32; 15],
    pub left: [i32; 15],
    pub right: [i32; 15],
    pub count: i32,
}

pub fn new_arena() -> Arena {
    return Arena {
        kind: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        value: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        left: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        right: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        count: 0,
    };
}

pub fn append(arena: &mut Arena, kind: i32, value: i32, left: i32, right: i32) -> i32 {
    let index = arena.count;
    if index >= 15 { return -1; }
    arena.kind[index] = kind;
    arena.value[index] = value;
    arena.left[index] = left;
    arena.right[index] = right;
    arena.count = index + 1;
    return index;
}
