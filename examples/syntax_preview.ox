use "../stdlib/prelude.ox";

fn main() {
    print syntax_profile_name();
    print syntax_summary();
    print syntax_note("pub fn");
    print syntax_note("match");
}
