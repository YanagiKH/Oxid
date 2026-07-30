use "../stdlib/prelude.ox";

fn main() {
    print "Oxid compile";
    print compile_summary("oxid", "0.7.0", "src/main.ox");
    print compile_goal();
    print compile_command_list();
}