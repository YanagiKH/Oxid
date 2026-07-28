use "../stdlib/prelude.ox";

fn main() {
    print command_catalog();
    print command_preview("demo", "src/main.ox");
    print command_error("src/main.ox", 12, 8, "unexpected token");
}
