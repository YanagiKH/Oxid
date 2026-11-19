use crate::text::new_text;
use crate::text::append_text;

fn main() -> i32 {
    let mut text = new_text();
    append_text(&mut text, &text);
    return 0;
}
