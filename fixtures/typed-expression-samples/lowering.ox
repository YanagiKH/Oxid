pub enum LowerResult {
    Lowered(i32),
    InvalidArena(i32),
}

pub fn lower(arena: &crate::arena::Arena, root: i32, code: &mut crate::stack_code::Code) -> LowerResult {
    // Invalid input preserves every output field, including a prior program.
    let invalid = crate::evaluator::validate(&*arena, root);
    if invalid != -2 {
        return LowerResult::InvalidArena(invalid);
    }
    // Unpublish before writing. Only a complete new prefix receives a count.
    code.count = 0;
    let mut index = 0;
    while index < arena.count {
        code.opcode[index] = arena.kind[index];
        code.operand[index] = arena.value[index];
        index = index + 1;
    }
    code.count = arena.count;
    return LowerResult::Lowered(code.count);
}
