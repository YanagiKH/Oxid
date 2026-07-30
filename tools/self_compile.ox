use "../stdlib/prelude.ox";

fn main() {
    print "Oxid self-compile";
    print self_compile_summary("oxid", "0.7.0", "src/main.ox");
    print self_compile_boundary_note();
    print self_compile_command_list();
}