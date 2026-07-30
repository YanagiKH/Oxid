use "../stdlib/prelude.ox";

fn main() {
    print "Oxid bootstrap";
    print bootstrap_summary("oxid", "0.7.0", "src/main.ox");
    print bootstrap_boundary_note();
    print bootstrap_command_list();
}