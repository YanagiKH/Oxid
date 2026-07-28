use "../stdlib/prelude.ox";

fn main() {
    print "smoke";
    print clamp(12, 0, 10);
    print square(4);
    print parse_error("tests/smoke.ox", 1, 1, "smoke test placeholder", "replace with real assertions");
}
