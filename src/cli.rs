#[allow(clippy::items_after_test_module)]
mod runtime {
    include!("main.rs");

    pub fn entry() {
        main();
    }
}

fn main() {
    runtime::entry();
}
