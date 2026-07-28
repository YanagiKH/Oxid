use "../stdlib/prelude.ox";

fn main() {
    print repeat("ox", 2);
    print quote("Oxid");
    print pad_left("7", 3, "0");
    print join_lines(["a", "b", "c"], ", ");
}
