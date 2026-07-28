use "../stdlib/prelude.ox";

fn main() {
    print parse_error("src/main.ox", 12, 8, "unexpected token", "remove the extra operator or finish the expression");
    print runtime_error("examples/hello.ox", 4, 3, "value is not callable", "call a function or import the module that defines it");
}
