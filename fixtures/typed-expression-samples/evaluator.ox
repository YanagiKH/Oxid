pub enum EvalResult {
    Value(i32),
    InvalidArena(i32),
}

// The public entry point checks count before this pass reads any arena column.
// Return the malformed node index, or -1 when the whole postorder tree is valid.
fn validate_tree(arena: &crate::arena::Arena) -> i32 {
    let mut roots = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let mut length = 0;
    let mut index = 0;
    while index < arena.count {
        let kind = arena.kind[index];
        if kind == 1 {
            if arena.value[index] < 0 || arena.left[index] != -1 || arena.right[index] != -1 {
                return index;
            }
            if length >= 15 {
                return index;
            }
            roots[length] = index;
            length = length + 1;
        } else {
            if kind != 2 && kind != 3 {
                return index;
            }
            if arena.value[index] != 0 || length < 2 {
                return index;
            }
            let left = roots[length - 2];
            let right = roots[length - 1];
            if arena.left[index] != left || arena.right[index] != right {
                return index;
            }
            roots[length - 2] = index;
            length = length - 1;
        }
        index = index + 1;
    }
    if length != 1 {
        return arena.count - 1;
    }
    return -1;
}

// Only called after the entire tree has passed validation.
fn evaluate_valid_tree(arena: &crate::arena::Arena, root: i32) -> i32 {
    let mut values = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let mut index = 0;
    while index < arena.count {
        let kind = arena.kind[index];
        if kind == 1 {
            values[index] = arena.value[index];
        } else {
            let left = values[arena.left[index]];
            let right = values[arena.right[index]];
            if kind == 2 {
                values[index] = left + right;
            } else {
                values[index] = left * right;
            }
        }
        index = index + 1;
    }
    return values[root];
}

// -2 means valid; -1 is an invalid count/root; other values name a bad node.
pub fn validate(arena: &crate::arena::Arena, root: i32) -> i32 {
    if arena.count < 1 || arena.count > 15 {
        return -1;
    }
    if root != arena.count - 1 {
        return -1;
    }
    let invalid = validate_tree(&*arena);
    if invalid < 0 { return -2; }
    return invalid;
}

pub fn evaluate(arena: &crate::arena::Arena, root: i32) -> EvalResult {
    let invalid = validate(&*arena, root);
    if invalid != -2 {
        return EvalResult::InvalidArena(invalid);
    }
    let value = evaluate_valid_tree(&*arena, root);
    return EvalResult::Value(value);
}
