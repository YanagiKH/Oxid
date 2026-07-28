use "../stdlib/prelude.ox";

fn main() {
    print diag_message("note", "examples/diagnostics_preview.ox", 1, 1, "preview only", "replace with real parsing rules later");
    print diag_frame("examples/diagnostics_preview.ox", 2, 4, "print(1 + )");
}
