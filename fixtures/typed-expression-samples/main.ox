mod arena;
mod scanner;

fn main() -> i32 {
    let mut nodes = crate::arena::new_arena();
    let root = crate::arena::append(&mut nodes, 1, 39, -1, -1);
    return nodes.value[root];
}
