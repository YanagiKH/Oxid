use "../stdlib/frontend/bytecode.ox";

fn representative_instruction_stream() {
    return [
        {op: "const", value: 41},
        {op: "store", name: "answer"},
        {op: "load", name: "answer"},
        {op: "const", value: 1},
        {op: "add"},
        {op: "print"},
        {op: "halt"}
    ];
}

fn representative_bytecode() {
    return "OXBC|1|AST|1\n0|CONST|41\n1|STORE|\"answer\"\n2|LOAD|\"answer\"\n3|CONST|1\n4|ADD|null\n5|PRINT|null\n6|HALT|null\n";
}

fn main() {
    const manifest = canonical_compiler_manifest();
    assert(manifest.schema == 1, "compiler manifest schema must be version 1");
    assert(manifest.providers.emitter == "oxid", "the Oxid emitter must own bytecode emission");
    assert(manifest.providers.lexer == "stage0", "the lexer must remain on stage0 until parity");
    assert(manifest.providers.parser == "stage0", "the parser must remain on stage0 until parity");
    assert(manifest.providers.diagnostics == "stage0", "diagnostics must remain on stage0 until parity");
    assert(manifest.providers.modules == "stage0", "module resolution must remain on stage0 until parity");
    assert(manifest.parity_gate == "byte-for-byte", "compiler provider switches require byte-for-byte parity");
    assert(manifest.parity_required_before_switch, "compiler provider switches require a parity gate");

    const emitted = emit_bytecode(representative_instruction_stream());
    assert(emitted == representative_bytecode(), "representative bytecode is not deterministic");
    print emitted;
    print canonical_compiler_manifest_json();
}
